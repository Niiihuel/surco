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
- (void)pasteThroughEditingMenu:(XCUIElement *)field {
    [field pressForDuration:1.1];
    // UIKit owns the edit menu. Match its localized action, not an app button.
    NSPredicate *label = [NSPredicate predicateWithFormat:@"(label == 'Paste' OR label == 'Pegar') AND NOT (identifier BEGINSWITH 'sheet-paste:')"];
    XCUIElement *paste = [[self.app descendantsMatchingType:XCUIElementTypeAny] matchingPredicate:label].firstMatch;
    XCTAssertTrue([paste waitForExistenceWithTimeout:12], @"The native editor must offer Paste: %@", self.app.debugDescription);
    [self waitFor:paste predicate:@"enabled == YES AND hittable == YES"];
    [paste tap];
}

- (void)testPlaylistCardUsesNativePasteWithoutStartingAnImport {
    NSString *link = @"https://open.spotify.com/playlist/surco-ui-card";
    UIPasteboard.generalPasteboard.URL = [NSURL URLWithString:link];
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

- (void)testPlaylistCardAcceptsCopiedUTF16TextWithoutImporting {
    NSString *text = @"Lista para el viaje — niñez 東京";
    UIPasteboard.generalPasteboard.items = @[@{@"public.utf16-external-plain-text": [text dataUsingEncoding:NSUTF16StringEncoding]}];
    [self launch];
    [self openLibrary];
    XCUIElement *paste = [self element:@"surco.native.paste.import-card"];
    XCTAssertTrue([paste waitForExistenceWithTimeout:12]);
    [self waitFor:paste predicate:@"enabled == YES AND hittable == YES"];
    [paste tap];
    XCUIElement *field = self.app.textFields[@"import-link"];
    XCTAssertTrue([field waitForExistenceWithTimeout:12]);
    [self waitFor:field predicate:[NSString stringWithFormat:@"value == '%@'", text]];
    XCTAssertTrue([self element:@"import-go"].exists, @"Copied text is an editable draft, not an automatic import");
    XCTAssertEqual(self.app.alerts.count, 0u);
    [self attachScreenshot:@"native-playlist-card-utf16-text"];
    [[self element:@"import-go"] tap];
    XCUIElement *error = self.app.staticTexts[@"No reconocimos ese enlace."];
    XCTAssertTrue([error waitForExistenceWithTimeout:12]);
    [field tap];
    [self waitFor:self.app.keyboards.firstMatch predicate:@"exists == YES"];
    NSString *correction = @"-corregido";
    [field typeText:correction];
    NSString *edited = [field.value copy];
    NSArray<NSString *> *parts = [edited componentsSeparatedByString:correction];
    XCTAssertEqual(parts.count, 2u, @"Validation must not replace the live field after the first correction character");
    XCTAssertEqualObjects([parts componentsJoinedByString:@""], text);
    [self waitFor:error predicate:@"exists == NO"];
    XCTAssertTrue(self.app.keyboards.firstMatch.exists, @"Correcting an invalid link preserves keyboard and caret");
    [self attachScreenshot:@"native-import-invalid-text-correction"];
    [[self element:@"import-cancel"] tap];
    [self waitFor:field predicate:@"exists == NO"];
}

- (void)testPlaylistCardFallsBackToTextWhenTheCopiedURLRepresentationFails {
    NSString *text = @"https://open.spotify.com/playlist/surco-ui-url-fallback";
    const unsigned char malformedURL[] = {0xFF, 0xFE, 0xFF};
    UIPasteboard.generalPasteboard.items = @[@{
        @"public.url": [NSData dataWithBytes:malformedURL length:sizeof(malformedURL)],
        @"public.utf8-plain-text": [text dataUsingEncoding:NSUTF8StringEncoding],
    }];
    [self launch];
    [self openLibrary];
    XCUIElement *paste = [self element:@"surco.native.paste.import-card"];
    XCTAssertTrue([paste waitForExistenceWithTimeout:12]);
    [self waitFor:paste predicate:@"enabled == YES AND hittable == YES"];
    [paste tap];
    XCUIElement *field = self.app.textFields[@"import-link"];
    XCTAssertTrue([field waitForExistenceWithTimeout:12]);
    [self waitFor:field predicate:[NSString stringWithFormat:@"value == '%@'", text]];
    XCTAssertTrue([self element:@"import-go"].exists);
    XCTAssertEqual(self.app.alerts.count, 0u);
    [[self element:@"import-cancel"] tap];
    [self waitFor:field predicate:@"exists == NO"];
}

- (void)testImportPasteControlFillsTheLinkWithoutImporting {
    NSString *link = @"https://open.spotify.com/playlist/surco-ui-paste-control";
    UIPasteboard.generalPasteboard.URL = [NSURL URLWithString:link];
    [self launch];
    [self openImport];
    XCUIElement *field = self.app.textFields[@"import-link"];
    XCUIElement *paste = [self element:@"sheet-paste:import-link"];
    XCTAssertTrue([paste waitForExistenceWithTimeout:12], @"%@", self.app.debugDescription);
    [self waitFor:paste predicate:@"enabled == YES AND hittable == YES"];
    [paste tap];
    [self waitFor:field predicate:[NSString stringWithFormat:@"value == '%@'", link]];
    XCTAssertTrue([self element:@"import-go"].exists, @"Paste fills the link and leaves import explicit");
    XCTAssertEqual(self.app.alerts.count, 0u, @"UIPasteControl must not prompt for pasteboard permission");
    [self attachScreenshot:@"native-import-paste-control"];
    [[self element:@"import-cancel"] tap];
    [self waitFor:field predicate:@"exists == NO"];
}

- (void)testImportUsesTheNativeEditingMenuToPasteAndReplaceSelectedText {
    NSString *text = @"niñez";
    UIPasteboard.generalPasteboard.string = text;
    [self launch];
    [self openImport];
    XCUIElement *field = self.app.textFields[@"import-link"];
    [self pasteThroughEditingMenu:field];
    [self waitFor:field predicate:[NSString stringWithFormat:@"value == '%@'", text]];
    // Double-tapping this one-word value selects it through UITextField itself.
    NSString *replacement = @"https://open.spotify.com/playlist/surco-ui-selection";
    UIPasteboard.generalPasteboard.string = replacement;
    [field doubleTap];
    NSPredicate *label = [NSPredicate predicateWithFormat:@"(label == 'Paste' OR label == 'Pegar') AND NOT (identifier BEGINSWITH 'sheet-paste:')"];
    XCUIElement *paste = [[self.app descendantsMatchingType:XCUIElementTypeAny] matchingPredicate:label].firstMatch;
    XCTAssertTrue([paste waitForExistenceWithTimeout:12], @"%@", self.app.debugDescription);
    [paste tap];
    [self waitFor:field predicate:[NSString stringWithFormat:@"value == '%@'", replacement]];
    XCTAssertTrue([self element:@"import-go"].exists, @"Native paste replaces the selection once and leaves import explicit");
    [field typeText:@"\n"];
    [self waitFor:self.app.keyboards.firstMatch predicate:@"exists == NO"];
    [self attachScreenshot:@"native-import-edit-menu-selection-replacement"];
    [[self element:@"import-cancel"] tap];
    [self waitFor:field predicate:@"exists == NO"];
}

- (void)testNativeImportPasteAndTypingSurviveKeyboardAndReflow {
    NSString *link = @"https://open.spotify.com/playlist/surco-ui-field";
    UIPasteboard.generalPasteboard.string = link;
    [self launch];
    [self openImport];
    XCUIElement *field = self.app.textFields[@"import-link"];
    [self pasteThroughEditingMenu:field];
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
    [self canvasTap:@"settings-part:devices" page:@"settings"];
    [self canvasTap:@"setting:connections" page:@"settings-devices"];
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
    XCTAssertEqual(volume.normalizedSliderPosition, 0.0, @"The fixture begins at the native slider's exact minimum");
    [volume adjustToNormalizedSliderPosition:0.5];
    NSString *adjusted = [volume.value copy];
    XCTAssertNotNil(adjusted);
    XCTAssertNotEqualObjects(adjusted, before, @"The native slider changes the volume");
    [self attachScreenshot:@"native-connections-route-and-volume"];
    [[self element:@"sheet-close"] tap];
    [self waitFor:volume predicate:@"exists == NO"];
    [self canvasTap:@"setting:connections" page:@"settings-devices"];
    XCTAssertTrue([volume waitForExistenceWithTimeout:12]);
    [self waitFor:volume predicate:@"enabled == YES AND hittable == YES"];
    XCTAssertEqualObjects(volume.value, adjusted, @"Reopening restores the volume received by Rust");
    // adjustToNormalizedSliderPosition is best effort. To require the exact
    // endpoint, drag the current thumb to the slider's left edge, inside the app.
    // https://developer.apple.com/documentation/xcuiautomation/xcuielement/adjust(tonormalizedsliderposition:)
    CGFloat position = volume.normalizedSliderPosition;
    XCTAssertTrue(position > 0.0 && position <= 1.0, @"The changed volume has a valid nonzero thumb position");
    XCUICoordinate *thumb = [volume coordinateWithNormalizedOffset:CGVectorMake(position, 0.5)];
    XCUICoordinate *minimum = [volume coordinateWithNormalizedOffset:CGVectorMake(0, 0.5)];
    [thumb pressForDuration:0.2 thenDragToCoordinate:minimum];
    [self waitFor:volume predicate:[NSString stringWithFormat:@"value == '%@'", before]];
    XCTAssertEqualObjects(volume.value, before, @"Restore the fixture's silent volume");
    XCTAssertEqual(volume.normalizedSliderPosition, 0.0);
    [self attachScreenshot:@"native-connections-restored-to-zero"];
    [[self element:@"sheet-close"] tap];
    [self waitFor:volume predicate:@"exists == NO"];
    [self canvasTap:@"setting:connections" page:@"settings-devices"];
    XCTAssertTrue([volume waitForExistenceWithTimeout:12]);
    [self waitFor:volume predicate:@"enabled == YES AND hittable == YES"];
    XCTAssertEqualObjects(volume.value, before, @"The exact minimum persists after closing and reopening");
    XCTAssertEqual(volume.normalizedSliderPosition, 0.0);
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

- (void)testNativeSearchEditingMenuClearAndReturnKeepTheEditor {
    NSString *original = @"niñez";
    UIPasteboard.generalPasteboard.string = original;
    [self launch];
    [self canvasTap:@"mobile-search" page:@"home"];
    XCUIElement *field = [self element:@"search"];
    XCTAssertTrue([field waitForExistenceWithTimeout:12], @"%@", self.app.debugDescription);
    XCTAssertTrue([self element:@"surco.native.search"].exists);
    XCTAssertFalse([self element:@"ios-paste:search"].exists, @"Search uses the native editor's editing menu");
    [field tap];
    [self waitFor:self.app.keyboards.firstMatch predicate:@"exists == YES"];
    [self pasteThroughEditingMenu:field];
    [self waitFor:field predicate:@"value == 'niñez'"];
    NSString *marker = @"-surco";
    [field typeText:marker];
    NSString *edited = [field.value copy];
    NSArray<NSString *> *parts = [edited componentsSeparatedByString:marker];
    XCTAssertEqual(parts.count, 2u, @"The native editor inserts the Unicode edit exactly once");
    XCTAssertEqualObjects([parts componentsJoinedByString:@""], original);
    XCUIDevice.sharedDevice.orientation = UIDeviceOrientationLandscapeLeft;
    [self waitForLandscape:YES];
    [self waitFor:field predicate:@"hittable == YES"];
    XCTAssertTrue(self.app.keyboards.firstMatch.exists, @"Rotation must not resign Search");
    XCTAssertEqualObjects(field.value, edited);
    [field typeText:@"-horizontal"];
    NSString *landscapeEdited = [edited stringByReplacingOccurrencesOfString:marker withString:[marker stringByAppendingString:@"-horizontal"]];
    XCTAssertEqualObjects(field.value, landscapeEdited, @"The retained editor keeps its native caret");
    [self attachScreenshot:@"native-search-keyboard-landscape"];
    XCUIDevice.sharedDevice.orientation = UIDeviceOrientationPortrait;
    [self waitForLandscape:NO];
    [self waitFor:field predicate:@"hittable == YES"];
    XCTAssertTrue(self.app.keyboards.firstMatch.exists);
    [field typeText:@"-vertical"];
    NSString *finalText = [landscapeEdited stringByReplacingOccurrencesOfString:@"-horizontal" withString:@"-horizontal-vertical"];
    XCTAssertEqualObjects(field.value, finalText, @"Rotation back must keep composition and selection ownership");
    [field typeText:@"\n"];
    [self waitFor:self.app.keyboards.firstMatch predicate:@"exists == NO"];
    XCTAssertEqualObjects(field.value, finalText, @"Return preserves the submitted query without reopening the keyboard");
    [field tap];
    [self waitFor:self.app.keyboards.firstMatch predicate:@"exists == YES"];
    NSPredicate *clearLabel = [NSPredicate predicateWithFormat:@"label == 'Clear text' OR label == 'Borrar texto' OR label == 'Clear' OR label == 'Borrar'"];
    XCUIElement *clear = [field.buttons matchingPredicate:clearLabel].firstMatch;
    XCTAssertTrue([clear waitForExistenceWithTimeout:12], @"UIKit must own Search's clear control: %@", self.app.debugDescription);
    [clear tap];
    [self waitFor:field predicate:@"value == '' OR value == 'Canciones, artistas o álbumes'"];
    XCTAssertTrue(self.app.keyboards.firstMatch.exists, @"Clear keeps the active editor and keyboard");
    [self attachScreenshot:@"native-search-editor-cleared"];
}

- (void)testNativeSearchCancelAndTabNavigationKeepTheQuery {
    [self launch];
    [self canvasTap:@"mobile-search" page:@"home"];
    XCUIElement *field = [self element:@"search"];
    XCTAssertTrue([field waitForExistenceWithTimeout:12]);
    [field tap];
    [field typeText:@"consulta retenida"];
    [self waitFor:field predicate:@"value == 'consulta retenida'"];
    // iOS 26 names UISearchBar's trailing cancel action Close/Cerrar. Scope
    // the localized control to the real bar rather than any screen button.
    NSPredicate *cancelLabel = [NSPredicate predicateWithFormat:@"label == 'Cancel' OR label == 'Cancelar' OR label == 'Close' OR label == 'Cerrar'"];
    XCUIElement *cancel = [[self element:@"surco.native.search"].buttons matchingPredicate:cancelLabel].firstMatch;
    XCTAssertTrue([cancel waitForExistenceWithTimeout:12]);
    [self waitFor:cancel predicate:@"hittable == YES"];
    XCTAssertGreaterThanOrEqual(cancel.frame.size.width, 44.0);
    XCTAssertGreaterThanOrEqual(cancel.frame.size.height, 44.0);
    [cancel tap];
    [self waitFor:self.app.keyboards.firstMatch predicate:@"exists == NO"];
    XCTAssertEqualObjects(field.value, @"consulta retenida", @"Cancel leaves the user's query intact");
    // iOS 26's Search folds the tab bar into a glass circle home; the library is a tab away from there.
    [self canvasTap:@"home" page:@"home"];
    [self waitFor:[self element:@"mobile-library"] predicate:@"hittable == YES"];
    [self canvasTap:@"mobile-library" page:@"home"];
    [self waitFor:field predicate:@"hittable == NO"];
    [self canvasTap:@"mobile-search" page:@"home"];
    [self waitFor:field predicate:@"exists == YES AND hittable == YES"];
    XCTAssertEqualObjects(field.value, @"consulta retenida");
    XCTAssertFalse(self.app.keyboards.firstMatch.exists, @"Returning to Search does not request keyboard focus");
    [self attachScreenshot:@"native-search-query-after-cancel-and-navigation"];
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
    [[self entryCoordinate:@"settings-part:devices" page:@"settings"] pressForDuration:0.05
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
