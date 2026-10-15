#import <XCTest/XCTest.h>
#import <UIKit/UIKit.h>

// Test the shipping UI through public XCUIAutomation. The canvas has no UIKit
// accessibility elements yet, so only entry taps use documented phone geometry;
// every sheet assertion/interaction targets an actual native control by ID.
// https://developer.apple.com/documentation/xcuiautomation/xcuiapplication
@interface SurcoNativeUITests : XCTestCase
@property(nonatomic, strong) XCUIApplication *app;
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
- (void)launch {
    [self.app launch];
    XCTAssertTrue([self.app waitForState:XCUIApplicationStateRunningForeground timeout:30]);
    XCTAssertTrue([self.app.windows.firstMatch waitForExistenceWithTimeout:10]);
}
- (CGFloat)canvasTop {
    XCUIElement *status = self.app.statusBars.firstMatch;
    return status.exists ? CGRectGetMaxY(status.frame) : CGRectGetMinY(self.app.frame) + 62;
}
- (void)canvasTap:(NSString *)identifier at:(CGPoint)point {
    XCUIElement *element = [self element:identifier];
    if (element.exists && element.hittable) { [element tap]; return; }
    XCUICoordinate *origin = [self.app coordinateWithNormalizedOffset:CGVectorMake(0, 0)];
    [[origin coordinateWithOffset:CGVectorMake(point.x, point.y)] tap];
}
- (void)openLibrary {
    CGRect frame = self.app.frame;
    // Phone tab bar: 64pt, directly above the 34pt home-indicator safe area.
    [self canvasTap:@"mobile-library" at:CGPointMake(frame.size.width * 5.0 / 6.0, frame.size.height - 66)];
}
- (void)openImport {
    [self openLibrary];
    // Import is below the fixed 76pt header, library controls and artwork card.
    [self canvasTap:@"import-open" at:CGPointMake(60, self.canvasTop + 450)];
    XCTAssertTrue([self.app.textFields[@"import-link"] waitForExistenceWithTimeout:12], @"%@", self.app.debugDescription);
    XCTAssertTrue([self element:@"import-go"].exists);
    XCTAssertTrue([self element:@"sheet-scroll"].exists);
}
- (void)openPlaylistDraft {
    [self openLibrary];
    [self canvasTap:@"playlist-new" at:CGPointMake(self.app.frame.size.width - 38, self.canvasTop + 114)];
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
    [field typeText:@"?prueba=niñez"];
    NSString *edited = [link stringByAppendingString:@"?prueba=niñez"];
    XCTAssertEqualObjects(field.value, edited);
    XCTAssertTrue(self.app.keyboards.firstMatch.exists);
    XCUIDevice.sharedDevice.orientation = UIDeviceOrientationLandscapeLeft;
    [self waitForLandscape:YES];
    XCTAssertEqualObjects(field.value, edited, @"Reflow keeps the live editor and its text");
    XCUIDevice.sharedDevice.orientation = UIDeviceOrientationPortrait;
    [self waitForLandscape:NO];
    [field typeText:@"-surco"];
    XCTAssertEqualObjects(field.value, [edited stringByAppendingString:@"-surco"]);
    [field typeText:@"\n"];
    [self waitFor:self.app.keyboards.firstMatch predicate:@"exists == NO"];
    XCTAssertEqualObjects(field.value, [edited stringByAppendingString:@"-surco"]);
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
    CGFloat top = self.canvasTop;
    [self canvasTap:@"settings" at:CGPointMake(self.app.frame.size.width - 38, top + 38)];
    [self canvasTap:@"setting:connections" at:CGPointMake(self.app.frame.size.width / 2.0, top + 486)];
    XCUIElement *volume = self.app.sliders[@"volume"];
    XCTAssertTrue([volume waitForExistenceWithTimeout:12], @"%@", self.app.debugDescription);
    XCTAssertTrue([self element:@"sheet-route-picker"].exists);
    XCTAssertTrue(volume.frame.size.height >= 44);
    [volume adjustToNormalizedSliderPosition:0.25];
    XCTAssertTrue(volume.value != nil);
    [self attachScreenshot:@"native-connections-route-and-volume"];
    [[self element:@"sheet-close"] tap];
    [self waitFor:volume predicate:@"exists == NO"];
}
@end
