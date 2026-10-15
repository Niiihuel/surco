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
- (void)waitForVisibleEditor:(XCUIElement *)field {
    XCUIElement *scroll = [self element:@"sheet-scroll"];
    NSPredicate *predicate = [NSPredicate predicateWithBlock:^BOOL(id object, NSDictionary *bindings) {
        XCUIElement *editor = object;
        CGRect visible = CGRectIntersection(editor.frame, scroll.frame);
        return self.app.keyboards.firstMatch.exists && editor.hittable &&
            !CGRectIsNull(visible) && visible.size.height >= 44 && visible.size.width >= 44;
    }];
    XCTNSPredicateExpectation *expectation = [[XCTNSPredicateExpectation alloc] initWithPredicate:predicate object:field];
    XCTAssertEqual([XCTWaiter waitForExpectations:@[expectation] timeout:12], XCTWaiterResultCompleted,
        @"The editor needs a full touch target above the keyboard. Field: %@, scroll: %@", field, scroll);
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
    XCTAssertEqual(rect.count, 4u, @"Missing verified Frame target %@:%@", page, identifier);
    CGFloat top = [self.entryGeometry[@"safe_insets"][0] doubleValue];
    return CGPointMake([rect[0] doubleValue] + [rect[2] doubleValue] / 2.0,
        top + [rect[1] doubleValue] + [rect[3] doubleValue] / 2.0);
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
- (void)openLibrary {
    [self canvasTap:@"mobile-library" page:@"home"];
}
- (void)openImport {
    [self openLibrary];
    [self canvasTap:@"import-open" page:@"library"];
    XCTAssertTrue([self.app.textFields[@"import-link"] waitForExistenceWithTimeout:12], @"%@", self.app.debugDescription);
    XCTAssertTrue([self element:@"import-go"].exists);
    XCTAssertTrue([self element:@"sheet-scroll"].exists);
}
- (void)openPlaylistDraft {
    [self openLibrary];
    [self canvasTap:@"playlist-new" page:@"library"];
    XCTAssertTrue([self.app.textFields[@"playlist-name"] waitForExistenceWithTimeout:12], @"%@", self.app.debugDescription);
    XCTAssertTrue([self element:@"playlist-create"].exists);
}
- (void)attachScreenshot:(NSString *)name {
    XCTAttachment *attachment = [XCTAttachment attachmentWithScreenshot:self.app.screenshot];
    attachment.name = name;
    attachment.lifetime = XCTAttachmentLifetimeKeepAlways;
    [self addAttachment:attachment];
}

- (void)testPlaylistCardUsesNativePasteWithoutStartingAnImport {
    NSString *link = @"https://open.spotify.com/playlist/surco-ui-card";
    UIPasteboard.generalPasteboard.string = link;
    [self launch];
    [self openLibrary];
    XCUIElement *paste = [self element:@"surco.native.paste.import-card"];
    XCTAssertTrue([paste waitForExistenceWithTimeout:12], @"%@", self.app.debugDescription);
    [self waitFor:paste predicate:@"enabled == YES AND hittable == YES"];
    [paste tap];
    XCUIElement *field = self.app.textFields[@"import-link"];
    XCTAssertTrue([field waitForExistenceWithTimeout:12]);
    [self waitFor:field predicate:[NSString stringWithFormat:@"value == '%@'", link]];
    XCTAssertTrue([self element:@"import-go"].exists, @"Paste prepares the form and leaves import explicit");
    XCTAssertEqual(self.app.alerts.count, 0u, @"UIPasteControl must not prompt for pasteboard permission");
    [self attachScreenshot:@"native-playlist-card-paste"];
    [[self element:@"sheet-close"] tap];
    [self waitFor:field predicate:@"exists == NO"];
}

- (void)testNativeImportPasteAndTypingSurviveKeyboardAndReflow {
    NSString *link = @"https://open.spotify.com/playlist/surco-ui-field";
    UIPasteboard.generalPasteboard.string = link;
    [self launch];
    [self openImport];
    XCUIElement *paste = [self element:@"sheet-paste:import-link"];
    XCTAssertTrue([paste waitForExistenceWithTimeout:10]);
    [self waitFor:paste predicate:@"enabled == YES AND hittable == YES"];
    [paste tap];
    XCUIElement *field = self.app.textFields[@"import-link"];
    [self waitFor:field predicate:[NSString stringWithFormat:@"value == '%@'", link]];
    [field tap];
    NSString *marker = @"?prueba=niñez";
    [field typeText:marker];
    // A tap chooses the native caret position, which need not be the URL's end.
    NSString *edited = [field.value copy];
    NSArray<NSString *> *parts = [edited componentsSeparatedByString:marker];
    XCTAssertEqual(parts.count, 2u, @"Typing inserts the marker exactly once");
    XCTAssertEqualObjects([parts componentsJoinedByString:@""], link, @"Typing preserves the complete pasted URL");
    XCTAssertTrue(self.app.keyboards.firstMatch.exists);
    XCUIDevice.sharedDevice.orientation = UIDeviceOrientationLandscapeLeft;
    [self waitForLandscape:YES];
    XCTAssertEqualObjects(field.value, edited, @"Reflow keeps the live editor and its text");
    [self waitForVisibleEditor:field];
    [field typeText:@"-apaisado"];
    NSString *landscapeEdited = [edited stringByReplacingOccurrencesOfString:marker withString:[marker stringByAppendingString:@"-apaisado"]];
    XCTAssertEqualObjects(field.value, landscapeEdited, @"The same editor remains usable above the landscape keyboard");
    [self attachScreenshot:@"native-import-keyboard-landscape"];
    XCUIDevice.sharedDevice.orientation = UIDeviceOrientationPortrait;
    [self waitForLandscape:NO];
    XCTAssertEqualObjects(field.value, landscapeEdited, @"Portrait keeps the landscape edit");
    [field typeText:@"-surco"];
    NSString *portraitEdited = [edited stringByReplacingOccurrencesOfString:marker withString:[marker stringByAppendingString:@"-apaisado-surco"]];
    XCTAssertEqualObjects(field.value, portraitEdited, @"Rotation keeps the insertion point as well as the text");
    [field typeText:@"\n"];
    [self waitFor:self.app.keyboards.firstMatch predicate:@"exists == NO"];
    XCTAssertEqualObjects(field.value, portraitEdited);
    XCTAssertTrue([self element:@"import-go"].exists, @"Return dismisses the keyboard and does not fetch the playlist");
    XCTAssertEqual(self.app.alerts.count, 0u);
    [self attachScreenshot:@"native-import-edited-after-reflow"];
    [[self element:@"import-cancel"] tap];
    [self waitFor:field predicate:@"exists == NO"];
}

- (void)testPlaylistDraftClosesByPullingTheHeaderAndCancelsItsContents {
    [self launch];
    [self openPlaylistDraft];
    XCUIElement *field = self.app.textFields[@"playlist-name"];
    [field tap];
    [field typeText:@"Borrador para cancelar\n"];
    [self waitFor:self.app.keyboards.firstMatch predicate:@"exists == NO"];
    XCTAssertEqualObjects(field.value, @"Borrador para cancelar");
    XCUIElement *close = [self element:@"sheet-close"];
    XCTAssertTrue(close.exists);
    CGFloat headerY = CGRectGetMidY(close.frame);
    XCUICoordinate *origin = [self.app coordinateWithNormalizedOffset:CGVectorMake(0, 0)];
    XCUICoordinate *start = [origin coordinateWithOffset:CGVectorMake(self.app.frame.size.width / 2.0, headerY)];
    XCUICoordinate *end = [origin coordinateWithOffset:CGVectorMake(self.app.frame.size.width / 2.0, self.app.frame.size.height - 30)];
    [start pressForDuration:0.1 thenDragToCoordinate:end];
    [self waitFor:field predicate:@"exists == NO"];
    XCTAssertFalse([self element:@"sheet-close"].exists, @"UIKit's pull dismisses the complete sheet");
    [self openPlaylistDraft];
    XCUIElement *reopened = self.app.textFields[@"playlist-name"];
    XCTAssertTrue([reopened.value isEqual:@""] || [reopened.value isEqual:reopened.placeholderValue],
        @"A dismissed draft must be canceled, not kept as an invisible open form: %@", reopened.value);
    [self attachScreenshot:@"native-playlist-empty-after-drag-dismiss"];
    [[self element:@"playlist-cancel"] tap];
    [self waitFor:reopened predicate:@"exists == NO"];
}

- (void)testConnectionsUseNativeRouteAndVolumeControlsWithoutPlayback {
    [self launch];
    [self canvasTap:@"settings" page:@"home"];
    [self canvasTap:@"setting:connections" page:@"settings"];
    XCUIElement *volume = self.app.sliders[@"volume"];
    XCTAssertTrue([volume waitForExistenceWithTimeout:12], @"%@", self.app.debugDescription);
    XCTAssertTrue([self element:@"sheet-route-picker"].exists);
    [self waitFor:volume predicate:@"enabled == YES AND hittable == YES"];
    // XCUI exposes Accessibility geometry, not the Auto Layout touch bounds.
    // UISlider keeps its required 44pt height; verify its native interaction and state.
    // https://developer.apple.com/documentation/xcuiautomation/xcuielementattributes
    NSLog(@"SURCO_NATIVE_SLIDER_AX_FRAME=%@", NSStringFromCGRect(volume.frame));
    NSString *before = [volume.value copy];
    XCTAssertNotNil(before);
    [volume adjustToNormalizedSliderPosition:0.5];
    NSString *adjusted = [volume.value copy];
    XCTAssertNotNil(adjusted);
    XCTAssertNotEqualObjects(adjusted, before, @"The native slider changes the volume");
    [self attachScreenshot:@"native-connections-route-and-volume"];
    [[self element:@"sheet-close"] tap];
    [self waitFor:volume predicate:@"exists == NO"];
    [self canvasTap:@"setting:connections" page:@"settings"];
    XCTAssertTrue([volume waitForExistenceWithTimeout:12]);
    XCTAssertEqualObjects(volume.value, adjusted, @"Reopening restores the volume received by Rust");
    [volume adjustToNormalizedSliderPosition:0];
    XCTAssertEqualObjects(volume.value, before, @"Restore the fixture's silent volume");
    [[self element:@"sheet-close"] tap];
    [self waitFor:volume predicate:@"exists == NO"];
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
