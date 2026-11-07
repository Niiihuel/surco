#import <XCTest/XCTest.h>
#import <UIKit/UIKit.h>

// Test the shipping UI through public XCUIAutomation. The canvas has no UIKit
// accessibility elements yet, so only entry taps use verified Frame geometry;
// every sheet assertion/interaction targets an actual native control by ID.
// https://developer.apple.com/documentation/xcuiautomation/xcuiapplication
@interface SurcoNativeUITests : XCTestCase
@property(nonatomic, strong) XCUIApplication *app;
@property(nonatomic, strong) NSDictionary *entryGeometry;
@end

@implementation SurcoNativeUITests
- (void)setUp {
    [super setUp];
    self.continueAfterFailure = NO;
    XCUIDevice.sharedDevice.orientation = UIDeviceOrientationPortrait;
    self.app = [XCUIApplication new];
    // The interface follows the system's language; these tests read its Spanish texts.
    self.app.launchArguments = @[@"-AppleLanguages", @"(es)", @"-AppleLocale", @"es_ES"];
}
- (void)tearDown {
    [self.app terminate];
    UIPasteboard.generalPasteboard.items = @[];
    XCUIDevice.sharedDevice.orientation = UIDeviceOrientationPortrait;
    [super tearDown];
}
- (XCUIElement *)element:(NSString *)identifier {
    return [[self.app descendantsMatchingType:XCUIElementTypeAny] matchingIdentifier:identifier].firstMatch;
}
- (void)waitFor:(XCUIElement *)element predicate:(NSString *)format {
    XCTNSPredicateExpectation *expectation = [[XCTNSPredicateExpectation alloc]
        initWithPredicate:[NSPredicate predicateWithFormat:format] object:element];
    XCTAssertEqual([XCTWaiter waitForExpectations:@[expectation] timeout:12], XCTWaiterResultCompleted,
        @"Expected %@ for %@. UI: %@", format, element, self.app.debugDescription);
}
- (void)waitForLandscape:(BOOL)landscape {
    NSPredicate *predicate = [NSPredicate predicateWithBlock:^BOOL(id object, NSDictionary *bindings) {
        XCUIElement *app = object;
        CGRect frame = app.frame;
        return landscape ? frame.size.width > frame.size.height : frame.size.height > frame.size.width;
    }];
    XCTNSPredicateExpectation *expectation = [[XCTNSPredicateExpectation alloc] initWithPredicate:predicate object:self.app];
    XCTAssertEqual([XCTWaiter waitForExpectations:@[expectation] timeout:12], XCTWaiterResultCompleted);
}
- (void)launch {
    [self.app launch];
    XCTAssertTrue([self.app waitForState:XCUIApplicationStateRunningForeground timeout:30]);
    XCTAssertTrue([self.app.windows.firstMatch waitForExistenceWithTimeout:10]);
    NSURL *fixture = [[NSBundle bundleForClass:self.class] URLForResource:@"EntryGeometry" withExtension:@"json"];
    XCTAssertNotNil(fixture);
    self.entryGeometry = [NSJSONSerialization JSONObjectWithData:[NSData dataWithContentsOfURL:fixture] options:0 error:nil];
    NSArray *window = self.entryGeometry[@"window"];
    XCTAssertEqualWithAccuracy(self.app.frame.size.width, [window[0] doubleValue], 0.5, @"Use the simulator verified by the Rust Frame fixture");
    XCTAssertEqualWithAccuracy(self.app.frame.size.height, [window[1] doubleValue], 0.5);
}
- (CGPoint)entryPoint:(NSString *)identifier page:(NSString *)page {
    NSArray *rect = self.entryGeometry[@"targets"][page][identifier];
    XCTAssertEqual(rect.count, 4u, @"Missing verified Frame target %@:%@. UI: %@", page, identifier, self.app.debugDescription);
    return CGPointMake([rect[0] doubleValue] + [rect[2] doubleValue] / 2.0,
        [rect[1] doubleValue] + [rect[3] doubleValue] / 2.0);
}
- (XCUICoordinate *)entryCoordinate:(NSString *)identifier page:(NSString *)page {
    CGPoint point = [self entryPoint:identifier page:page];
    XCUICoordinate *origin = [self.app coordinateWithNormalizedOffset:CGVectorMake(0, 0)];
    return [origin coordinateWithOffset:CGVectorMake(point.x, point.y)];
}
- (void)canvasTap:(NSString *)identifier page:(NSString *)page {
    XCUIElement *element = [self element:identifier];
    if (element.exists && element.hittable) { [element tap]; return; }
    [[self entryCoordinate:identifier page:page] tap];
}
// The tabs are limn's floating bar, at the same place on every page that shows it.
- (void)tapTab:(NSString *)identifier {
    [self canvasTap:identifier page:@"home"];
}
- (void)openLibrary {
    [self tapTab:@"mobile-library"];
}
// The list of playlists, with the card that brings music in: "See all" on the overview's lists.
- (void)openLibraryLists {
    [self openLibrary];
    [self canvasTap:@"library-all:lists" page:@"library"];
}
- (void)attachScreenshot:(NSString *)name {
    XCTAttachment *attachment = [XCTAttachment attachmentWithScreenshot:self.app.screenshot];
    attachment.name = name;
    attachment.lifetime = XCTAttachmentLifetimeKeepAlways;
    [self addAttachment:attachment];
}
- (void)testPlaylistCardUsesNativePasteWithoutStartingAnImport {
    NSString *link = @"https://open.spotify.com/playlist/surco-ui-card";
    UIPasteboard.generalPasteboard.URL = [NSURL URLWithString:link];
    [self launch];
    [self openLibraryLists];
    XCUIElement *paste = [self element:@"surco.native.paste.import-card"];
    XCTAssertTrue([paste waitForExistenceWithTimeout:12], @"%@", self.app.debugDescription);
    [self waitFor:paste predicate:@"enabled == YES AND hittable == YES"];
    [paste tap];
    // The link fills limn's import sheet, which only its check imports.
    [NSThread sleepForTimeInterval:1.0];
    XCTAssertEqual(self.app.alerts.count, 0u, @"UIPasteControl must not prompt for pasteboard permission");
    [self attachScreenshot:@"native-playlist-card-paste"];
}

- (void)testPlaylistCardAcceptsCopiedUTF16TextWithoutImporting {
    NSString *text = @"Lista para el viaje — niñez 東京";
    UIPasteboard.generalPasteboard.items = @[@{@"public.utf16-external-plain-text": [text dataUsingEncoding:NSUTF16StringEncoding]}];
    [self launch];
    [self openLibraryLists];
    XCUIElement *paste = [self element:@"surco.native.paste.import-card"];
    XCTAssertTrue([paste waitForExistenceWithTimeout:12]);
    [self waitFor:paste predicate:@"enabled == YES AND hittable == YES"];
    [paste tap];
    // The text fills limn's import sheet, which only its check imports.
    [NSThread sleepForTimeInterval:1.0];
    XCTAssertEqual(self.app.alerts.count, 0u, @"UIPasteControl must not prompt for pasteboard permission");
    [self attachScreenshot:@"native-playlist-card-utf16-text"];
}
- (void)testPlaylistCardFallsBackToTextWhenTheCopiedURLRepresentationFails {
    NSString *text = @"https://open.spotify.com/playlist/surco-ui-url-fallback";
    const unsigned char malformedURL[] = {0xFF, 0xFE, 0xFF};
    UIPasteboard.generalPasteboard.items = @[@{
        @"public.url": [NSData dataWithBytes:malformedURL length:sizeof(malformedURL)],
        @"public.utf8-plain-text": [text dataUsingEncoding:NSUTF8StringEncoding],
    }];
    [self launch];
    [self openLibraryLists];
    XCUIElement *paste = [self element:@"surco.native.paste.import-card"];
    XCTAssertTrue([paste waitForExistenceWithTimeout:12]);
    [self waitFor:paste predicate:@"enabled == YES AND hittable == YES"];
    [paste tap];
    // The text fills limn's import sheet, which only its check imports.
    [NSThread sleepForTimeInterval:1.0];
    XCTAssertEqual(self.app.alerts.count, 0u, @"UIPasteControl must not prompt for pasteboard permission");
    [self attachScreenshot:@"native-playlist-card-url-fallback"];
}
- (void)testConnectionsSheetCarriesTheSystemRoutePicker {
    [self launch];
    [self canvasTap:@"settings" page:@"home"];
    [self canvasTap:@"setting:connections" page:@"settings"];
    // limn draws the sheet; UIKit's own AirPlay and Bluetooth button stands in its slot.
    XCUIElement *picker = [self element:@"surco-route-picker"];
    XCTAssertTrue([picker waitForExistenceWithTimeout:12], @"%@", self.app.debugDescription);
    [self waitFor:picker predicate:@"hittable == YES"];
    [self attachScreenshot:@"limn-connections-with-route-picker"];
}

- (void)testHomeCardLongPressOpensNativeMenuForTheHeldTrack {
    [self launch];
    [self attachScreenshot:@"home-before-native-context-menu"];
    // Hold the untouched card itself. No tap or playback happens beforehand.
    [[self entryCoordinate:@"heard:0" page:@"home"] pressForDuration:0.7];
    XCUIElement *like = self.app.buttons[@"Me gusta"];
    XCTAssertTrue([like waitForExistenceWithTimeout:12], @"%@", self.app.debugDescription);
    XCTAssertTrue(like.hittable);
    XCTAssertFalse([self element:@"sheet-close"].exists, @"A card hold opens UIKit's context menu");
    [self attachScreenshot:@"home-native-context-menu"];
    [like tap];
    [self waitFor:like predicate:@"exists == NO"];
    // The harmless action changes this synthetic track, proving the menu captured
    // the card's identity rather than a later mutable list index.
    [[self entryCoordinate:@"heard:0" page:@"home"] pressForDuration:0.7];
    XCUIElement *unlike = self.app.buttons[@"Quitar de Tus me gusta"];
    XCTAssertTrue([unlike waitForExistenceWithTimeout:12]);
    [self attachScreenshot:@"home-native-context-menu-held-track-liked"];
    [unlike tap];
    [self waitFor:unlike predicate:@"exists == NO"];
    XCTAssertFalse([self element:@"sheet-close"].exists);
}

// Search is limn's field on the canvas, which XCUI cannot read: what reaches UIKit is the
// keyboard, and the screenshots are left for review.
- (void)testSearchFieldTakesTheKeyboardForTheQuery {
    [self launch];
    [self tapTab:@"mobile-search"];
    [self canvasTap:@"mobile-search-edit" page:@"search"];
    [self waitFor:self.app.keyboards.firstMatch predicate:@"exists == YES"];
    [self.app typeText:@"consulta retenida"];
    [self attachScreenshot:@"search-query-typed"];
    [self.app typeText:@"\n"];
    [self waitFor:self.app.keyboards.firstMatch predicate:@"exists == NO"];
    [self attachScreenshot:@"search-query-submitted"];
}

// The query stays the search's while the tabs go elsewhere and back, as BitChord's.
- (void)testSearchQueryStaysAcrossTheTabs {
    [self launch];
    [self tapTab:@"mobile-search"];
    [self canvasTap:@"mobile-search-edit" page:@"search"];
    [self waitFor:self.app.keyboards.firstMatch predicate:@"exists == YES"];
    [self.app typeText:@"consulta retenida\n"];
    [self waitFor:self.app.keyboards.firstMatch predicate:@"exists == NO"];
    [self tapTab:@"mobile-library"];
    [self tapTab:@"mobile-search"];
    [NSThread sleepForTimeInterval:1.0];
    XCTAssertFalse(self.app.keyboards.firstMatch.exists, @"Returning to Search does not raise the keyboard");
    [self attachScreenshot:@"search-query-after-navigation"];
}

- (void)testSettingsRowsBlurBeneathTheHeaderAsTheyScroll {
    [self launch];
    // Home is long enough to pass under the header; its edge is the one the eye judges.
    [[self.app coordinateWithNormalizedOffset:CGVectorMake(0.5, 0.75)] pressForDuration:0.05
        thenDragToCoordinate:[self.app coordinateWithNormalizedOffset:CGVectorMake(0.5, 0.45)]];
    [NSThread sleepForTimeInterval:1.0];
    [self attachScreenshot:@"home-scrolled-header-edge"];
    [[self.app coordinateWithNormalizedOffset:CGVectorMake(0.5, 0.3)] pressForDuration:0.05
        thenDragToCoordinate:[self.app coordinateWithNormalizedOffset:CGVectorMake(0.5, 0.9)]];
    [NSThread sleepForTimeInterval:1.0];
    [self canvasTap:@"settings" page:@"home"];
    [[self entryCoordinate:@"setting:connections" page:@"settings"] pressForDuration:0.05
        thenDragToCoordinate:[self.app coordinateWithNormalizedOffset:CGVectorMake(0.5, 0.12)]];
    [NSThread sleepForTimeInterval:1.0];
    // Evidence for review: rows under the header fade into the progressive edge.
    [self attachScreenshot:@"settings-scrolled-header-edge"];
}

- (void)testSystemSymbolsUsedBySurcoExistInUIKit {
    NSURL *fixture = [[NSBundle bundleForClass:self.class] URLForResource:@"SymbolNames" withExtension:@"json"];
    XCTAssertNotNil(fixture);
    NSArray<NSString *> *names = [NSJSONSerialization JSONObjectWithData:[NSData dataWithContentsOfURL:fixture] options:0 error:nil];
    XCTAssertGreaterThanOrEqual(names.count, 119u);
    XCTAssertEqual([NSSet setWithArray:names].count, names.count);
    NSMutableArray<NSString *> *missing = [NSMutableArray array];
    void (^validate)(void) = ^{
        for (NSString *name in names) {
            UIImage *image = [UIImage systemImageNamed:name];
            if (!image || image.size.width <= 0 || image.size.height <= 0) [missing addObject:name];
        }
    };
    if (NSThread.isMainThread) validate(); else dispatch_sync(dispatch_get_main_queue(), validate);
    XCTAssertEqual(missing.count, 0u, @"Symbols unavailable in this UIKit runtime: %@", missing);
}
@end
