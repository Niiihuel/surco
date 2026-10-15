#import "SurcoSurfaces.h"
#import "SurcoSheet.h"
#include <math.h>

static UIColor *SurcoSurfaceColor(void) { return [UIColor colorWithWhite:28.0 / 255.0 alpha:1]; }
static NSString *SurcoString(id value) { return [value isKindOfClass:NSString.class] ? value : @""; }
static BOOL SurcoDeliverPaste(NSString *text, void (^receive)(NSString *)) {
    if (!text.length || text.length > 65536) return NO;
    dispatch_async(dispatch_get_main_queue(), ^{ if (receive) receive(text); });
    return YES;
}
static void SurcoLoadPasteString(NSItemProvider *provider, void (^receive)(NSString *)) {
    if (!provider) return;
    [provider loadObjectOfClass:NSString.class completionHandler:^(id<NSItemProviderReading> item, NSError *error) {
        if (!error && [item isKindOfClass:NSString.class]) SurcoDeliverPaste((NSString *)item, receive);
    }];
}

// A real UIPasteControl recipient. No pasteboard reads occur while checking availability.
// https://developer.apple.com/documentation/uikit/uipastecontrol
@interface SurcoPasteTarget : UIResponder
@property(nonatomic, copy) void (^receive)(NSString *);
@end

@implementation SurcoPasteTarget
- (instancetype)init {
    if ((self = [super init])) {
        // Accept every text/URL representation supported by the classes we load,
        // including strings copied as UTF-16 by another native app.
        // https://developer.apple.com/documentation/uikit/uipasteconfiguration
        UIPasteConfiguration *configuration = [[UIPasteConfiguration alloc] initWithTypeIdentifiersForAcceptingClass:NSURL.class];
        [configuration addTypeIdentifiersForAcceptingClass:NSString.class];
        self.pasteConfiguration = configuration;
    }
    return self;
}
- (void)pasteItemProviders:(NSArray<NSItemProvider *> *)providers {
    NSItemProvider *urlProvider = nil;
    NSItemProvider *textProvider = nil;
    for (NSItemProvider *provider in providers) {
        if (!urlProvider && [provider canLoadObjectOfClass:NSURL.class]) urlProvider = provider;
        if (!textProvider && [provider canLoadObjectOfClass:NSString.class]) textProvider = provider;
    }
    void (^receive)(NSString *) = self.receive;
    if (!urlProvider) { SurcoLoadPasteString(textProvider, receive); return; }
    [urlProvider loadObjectOfClass:NSURL.class completionHandler:^(id<NSItemProviderReading> item, NSError *error) {
        if (!error && [item isKindOfClass:NSURL.class] && SurcoDeliverPaste([(NSURL *)item absoluteString], receive)) return;
        // A promised URL representation can fail while a usable text version
        // remains available. Load just one fallback, so a tap emits at most once.
        SurcoLoadPasteString(textProvider, receive);
    }];
}
@end

@interface SurcoSearchBar : UISearchBar
@end
@implementation SurcoSearchBar
- (void)layoutSubviews {
    [super layoutSubviews];
    self.searchTextField.layer.cornerRadius = CGRectGetHeight(self.searchTextField.bounds) / 2.0;
    self.searchTextField.clipsToBounds = YES;
}
@end

@interface SurcoSurfaces () <UIContextMenuInteractionDelegate, UISheetPresentationControllerDelegate, UISearchBarDelegate> {
    __weak UIView *_view;
    UIContextMenuInteraction *_interaction;
    NSArray<NSDictionary *> *_menus;
    NSDictionary *_activeMenu;
    NSDictionary *_desiredSheet;
    NSDictionary *_desiredPaste;
    UIViewController *_sheet;
    NSString *_sheetId;
    NSString *_dismissedSheet;
    BOOL _transitioning;
    BOOL _menuVisible;
    BOOL _valid;
    BOOL _reduceMotion;
    CGFloat _scale;
    UIControl *_paste;
    SurcoPasteTarget *_pasteTarget;
    NSString *_pasteDestination;
    NSUInteger _pasteGeneration;
    NSUInteger _menuGeneration;
    UISearchBar *_searchBar;
    NSDictionary *_desiredSearch;
    NSString *_searchSession;
    NSString *_searchSentText;
    NSUInteger _searchRevision;
    BOOL _searchWantsEditing;
    BOOL _updatingSearch;
    NSMutableArray<NSDictionary *> *_events;
    void (^_wake)(void);
}
- (void)emit:(NSDictionary *)event;
- (void)reconcileSheet;
- (void)updateSearch:(NSDictionary *)model;
@end

@implementation SurcoSurfaces
- (instancetype)initWithWake:(void (^)(void))wake {
    if ((self = [super init])) {
        _wake = [wake copy];
        _events = [NSMutableArray array];
        _menus = @[];
        _valid = YES;
    }
    return self;
}
- (void)emit:(NSDictionary *)event {
    if (!_valid) return;
    [_events addObject:event];
    if (_wake) _wake();
}
- (NSArray<NSDictionary *> *)takeEvents {
    NSArray *events = [_events copy];
    [_events removeAllObjects];
    return events;
}
- (void)invalidate {
    _valid = NO;
    _wake = nil;
    _pasteGeneration++;
    _menuGeneration++;
    _pasteTarget.receive = nil;
    [_paste removeFromSuperview];
    _paste = nil;
    _searchBar.delegate = nil;
    [_searchBar.searchTextField resignFirstResponder];
    [_searchBar removeFromSuperview];
    _searchBar = nil;
    _searchSession = nil;
    [_interaction dismissMenu];
    [_interaction.view removeInteraction:_interaction];
    _interaction = nil;
    [_sheet dismissViewControllerAnimated:NO completion:nil];
    _sheet = nil;
    _view = nil;
    [_events removeAllObjects];
}
- (CGRect)rect:(NSArray<NSNumber *> *)values {
    if (![values isKindOfClass:NSArray.class] || values.count != 4) return CGRectNull;
    for (id value in values) if (![value isKindOfClass:NSNumber.class] || !isfinite([value doubleValue])) return CGRectNull;
    CGRect rect = CGRectMake(values[0].doubleValue * _scale, values[1].doubleValue * _scale,
                             values[2].doubleValue * _scale, values[3].doubleValue * _scale);
    rect.origin.x += _view.bounds.origin.x;
    rect.origin.y += _view.bounds.origin.y;
    return rect;
}
- (void)updatePayload:(NSDictionary *)payload view:(UIView *)view scale:(CGFloat)scale {
    if (!_valid) return;
    if (_view != view) {
        _updatingSearch = YES;
        [self updateSearch:nil];
        [_searchBar removeFromSuperview];
        _updatingSearch = NO;
        _menuGeneration++;
        [_interaction dismissMenu];
        [_interaction.view removeInteraction:_interaction];
        [_paste removeFromSuperview];
        UIViewController *oldSheet = _sheet;
        _sheet = nil;
        _sheetId = nil;
        _activeMenu = nil;
        _menuVisible = NO;
        _transitioning = NO;
        _pasteGeneration++;
        [oldSheet dismissViewControllerAnimated:NO completion:nil];
        _view = view;
        _interaction = [[UIContextMenuInteraction alloc] initWithDelegate:self];
        [view addInteraction:_interaction];
    }
    _scale = scale;
    _reduceMotion = [payload[@"reduce_motion"] boolValue];
    _menus = payload[@"menus"] ?: @[];
    _desiredSheet = [payload[@"sheet"] isKindOfClass:NSDictionary.class] ? payload[@"sheet"] : nil;
    if (!_desiredSheet || ![_desiredSheet[@"id"] isEqual:_dismissedSheet]) _dismissedSheet = nil;
    _desiredPaste = [payload[@"paste"] isKindOfClass:NSDictionary.class] ? payload[@"paste"] : nil;
    _desiredSearch = [payload[@"search"] isKindOfClass:NSDictionary.class] ? payload[@"search"] : nil;
    [self updateSearch:_desiredSearch];
    [self updatePaste:_desiredPaste];
    [self reconcileSheet];
}
- (void)refresh {
    [self updateSearch:_desiredSearch];
    [self updatePaste:_desiredPaste];
    [self reconcileSheet];
}
- (BOOL)searchIsLive {
    return _valid && !_updatingSearch && _searchSession.length && !_searchBar.hidden && _view.window;
}
- (void)sendSearchText {
    // UITextInput's marked range belongs to the IME. Never search or overwrite
    // a half-composed word; UIKit commits it before Return/end editing.
    // https://developer.apple.com/documentation/uikit/uitextinput/markedtextrange
    if (![self searchIsLive] || _searchBar.searchTextField.markedTextRange) return;
    NSString *text = _searchBar.text ?: @"";
    if ([_searchSentText isEqualToString:text]) return;
    _searchRevision++;
    _searchSentText = [text copy];
    [self emit:@{@"kind":@"search_edit", @"session":_searchSession,
        @"revision":@(_searchRevision), @"text":text}];
}
- (void)updateSearch:(NSDictionary *)model {
    CGRect rect = model ? [self rect:model[@"rect"]] : CGRectNull;
    BOOL visible = model && _view.window && !_desiredSheet && !_menuVisible;
    if (!visible) {
        [_searchBar.searchTextField resignFirstResponder];
        _searchBar.hidden = YES;
        _searchBar.userInteractionEnabled = NO;
        // A context menu temporarily covers Search. Preserve its revision so a
        // queued pre-menu model cannot reopen the keyboard at menu completion.
        if (!model || !_menuVisible) _searchSession = nil;
        _searchWantsEditing = NO;
        return;
    }
    // refresh can run between UIView rotation and the newly measured Rust Frame.
    // An old landscape slot is not a navigation/focus intention. Keep the live
    // editor and selection until updatePayload supplies its new geometry.
    if (CGRectIsNull(rect) || CGRectIsEmpty(rect) || !CGRectContainsRect(_view.bounds, rect)) return;
    if (!_searchBar) {
        // UISearchBar owns its text field, clear control, Cancel and edit menu.
        // https://developer.apple.com/documentation/uikit/uisearchbar
        _searchBar = [SurcoSearchBar new];
        _searchBar.delegate = self;
        _searchBar.searchBarStyle = UISearchBarStyleMinimal;
        _searchBar.backgroundColor = UIColor.clearColor;
        _searchBar.overrideUserInterfaceStyle = UIUserInterfaceStyleDark;
        _searchBar.returnKeyType = UIReturnKeySearch;
        _searchBar.autocapitalizationType = UITextAutocapitalizationTypeNone;
        _searchBar.autocorrectionType = UITextAutocorrectionTypeNo;
        _searchBar.placeholder = @"Canciones, artistas o álbumes";
        _searchBar.accessibilityIdentifier = @"surco.native.search";
        _searchBar.searchTextField.accessibilityIdentifier = @"search";
        _searchBar.searchTextField.font = [UIFont preferredFontForTextStyle:UIFontTextStyleBody];
        _searchBar.searchTextField.adjustsFontForContentSizeCategory = YES;
    }
    NSString *session = SurcoString(model[@"session"]);
    NSUInteger revision = [model[@"revision"] unsignedIntegerValue];
    NSString *text = SurcoString(model[@"text"]);
    if (![_searchSession isEqual:session]) {
        // A new tab session or explicit recent/browse selection supersedes the
        // old editor's callbacks, while keeping the UIKit view itself alive.
        _updatingSearch = YES;
        [_searchBar.searchTextField resignFirstResponder];
        _searchSession = [session copy];
        _searchRevision = revision;
        _searchBar.text = text;
        _searchSentText = [text copy];
        _searchWantsEditing = NO;
        _updatingSearch = NO;
    }
    if (_searchBar.superview != _view) [_view addSubview:_searchBar];
    _searchBar.frame = rect;
    _searchBar.hidden = NO;
    _searchBar.userInteractionEnabled = YES;
    [_view bringSubviewToFront:_searchBar];
    // A queued render may predate a keystroke OR a focus gesture. Both share
    // one revision stream, so neither stale text nor stale focus can bounce it.
    if (revision < _searchRevision) return;
    BOOL editing = [model[@"editing"] boolValue];
    BOOL compact = [model[@"compact"] boolValue];
    if (!_searchBar.searchTextField.markedTextRange && ![_searchBar.text isEqualToString:text]) {
        _updatingSearch = YES;
        _searchBar.text = text;
        _searchSentText = [text copy];
        _updatingSearch = NO;
    }
    UIColor *ink = compact ? UIColor.whiteColor : UIColor.blackColor;
    _searchBar.tintColor = UIColor.whiteColor;
    _searchBar.searchTextField.backgroundColor = compact ? [UIColor colorWithWhite:0.2 alpha:1] : UIColor.whiteColor;
    _searchBar.searchTextField.textColor = ink;
    _searchBar.searchTextField.tintColor = ink;
    _searchBar.searchTextField.leftView.tintColor = ink;
    _searchBar.searchTextField.attributedPlaceholder = [[NSAttributedString alloc] initWithString:_searchBar.placeholder
        attributes:@{NSForegroundColorAttributeName: compact ? UIColor.lightGrayColor : UIColor.darkGrayColor}];
    if (_searchBar.showsCancelButton != editing) [_searchBar setShowsCancelButton:editing
        animated:!_reduceMotion && !UIAccessibilityIsReduceMotionEnabled()];
    if (_searchWantsEditing != editing) {
        _searchWantsEditing = editing;
        if (editing && !_searchBar.searchTextField.isFirstResponder) [_searchBar.searchTextField becomeFirstResponder];
        else if (!editing && _searchBar.searchTextField.isFirstResponder) [_searchBar.searchTextField resignFirstResponder];
    }
}
- (void)searchBarTextDidBeginEditing:(UISearchBar *)searchBar {
    if (![self searchIsLive]) return;
    _searchWantsEditing = YES;
    _searchRevision++;
    [self emit:@{@"kind":@"search_focus", @"session":_searchSession, @"revision":@(_searchRevision), @"editing":@YES}];
}
- (void)searchBar:(UISearchBar *)searchBar textDidChange:(NSString *)searchText {
    if (![self searchIsLive]) return;
    if (searchBar.searchTextField.markedTextRange) {
        _searchRevision++; // Protect the active composition from an older render.
        return;
    }
    [self sendSearchText];
}
- (void)searchBarTextDidEndEditing:(UISearchBar *)searchBar {
    if (![self searchIsLive]) return;
    [self sendSearchText];
    _searchWantsEditing = NO;
    _searchRevision++;
    [self emit:@{@"kind":@"search_focus", @"session":_searchSession, @"revision":@(_searchRevision), @"editing":@NO}];
}
- (void)searchBarSearchButtonClicked:(UISearchBar *)searchBar {
    if (![self searchIsLive]) return;
    [searchBar.searchTextField unmarkText];
    [self sendSearchText];
    [self emit:@{@"kind":@"search_submit", @"session":_searchSession}];
    [searchBar.searchTextField resignFirstResponder];
}
- (void)searchBarCancelButtonClicked:(UISearchBar *)searchBar {
    if (![self searchIsLive]) return;
    NSString *session = _searchSession;
    [searchBar.searchTextField resignFirstResponder];
    [self emit:@{@"kind":@"search_cancel", @"session":session}];
}
- (void)updatePaste:(NSDictionary *)slot {
    CGRect rect = slot ? [self rect:slot[@"rect"]] : CGRectNull;
    BOOL visible = slot && _view.window && !_desiredSheet && !_menuVisible &&
                   !CGRectIsEmpty(rect) && !CGRectIsNull(rect) && CGRectContainsRect(_view.bounds, rect);
    NSString *destination = visible ? slot[@"target"] : nil;
    if (!(_pasteDestination == destination || [_pasteDestination isEqual:destination])) {
        _pasteGeneration++;
        _pasteDestination = [destination copy];
        _pasteTarget.receive = nil;
    }
    if (!visible) { _paste.hidden = YES; _paste.userInteractionEnabled = NO; return; }
    if (!_paste) {
        _pasteTarget = [SurcoPasteTarget new];
        if (@available(iOS 16.0, *)) {
            UIPasteControlConfiguration *configuration = [UIPasteControlConfiguration new];
            configuration.displayMode = UIPasteControlDisplayModeIconAndLabel;
            configuration.cornerStyle = UIButtonConfigurationCornerStyleCapsule;
            configuration.baseBackgroundColor = UIColor.whiteColor;
            configuration.baseForegroundColor = UIColor.blackColor;
            UIPasteControl *control = [[UIPasteControl alloc] initWithConfiguration:configuration];
            control.target = _pasteTarget;
            _paste = control;
        } else {
            UIButton *button = [UIButton buttonWithType:UIButtonTypeSystem];
            [button setTitle:@"Pegar" forState:UIControlStateNormal];
            [button addTarget:self action:@selector(legacyPaste) forControlEvents:UIControlEventTouchUpInside];
            _paste = button;
        }
    }
    NSUInteger generation = _pasteGeneration;
    __weak SurcoSurfaces *weakSelf = self;
    _pasteTarget.receive = ^(NSString *text) {
        SurcoSurfaces *owner = weakSelf;
        if (!owner || owner->_pasteGeneration != generation || owner->_paste.hidden || !owner->_view.window) return;
        [owner emit:@{@"kind": @"paste", @"target": destination, @"text": text}];
    };
    if (_paste.superview != _view) [_view addSubview:_paste];
    _paste.frame = rect;
    _paste.accessibilityIdentifier = [@"surco.native.paste." stringByAppendingString:destination];
    _paste.hidden = NO;
    _paste.userInteractionEnabled = YES;
    [_view bringSubviewToFront:_paste];
}
- (void)legacyPaste {
    // iOS 15 has no UIPasteControl. This path still requires an explicit tap.
    UIPasteboard *pasteboard = UIPasteboard.generalPasteboard;
    NSString *text = pasteboard.string ?: pasteboard.URL.absoluteString;
    if (text.length && _pasteTarget.receive) _pasteTarget.receive(text);
}
- (UIMenu *)menu:(NSDictionary *)model {
    NSMutableArray<UIMenuElement *> *items = [NSMutableArray array];
    __weak SurcoSurfaces *weakSelf = self;
    for (NSDictionary *item in model[@"items"]) {
        UIMenuElementAttributes attributes = [item[@"destructive"] boolValue] ? UIMenuElementAttributesDestructive : 0;
        UIAction *action = [UIAction actionWithTitle:SurcoString(item[@"title"]) image:[UIImage systemImageNamed:SurcoString(item[@"symbol"])]
            identifier:nil handler:^(UIAction *selected) {
                [weakSelf emit:@{@"kind": @"menu_action", @"id": model[@"id"], @"action": item[@"id"]}];
            }];
        action.attributes = attributes;
        action.state = [item[@"selected"] boolValue] ? UIMenuElementStateOn : UIMenuElementStateOff;
        action.subtitle = SurcoString(item[@"subtitle"]);
        [items addObject:action];
    }
    return [UIMenu menuWithTitle:@"" children:items];
}
- (UIContextMenuConfiguration *)contextMenuInteraction:(UIContextMenuInteraction *)interaction configurationForMenuAtLocation:(CGPoint)location {
    if (_desiredSheet || _sheet || _transitioning) return nil;
    NSDictionary *target = nil;
    for (NSDictionary *menu in [_menus reverseObjectEnumerator]) {
        if (CGRectContainsPoint([self rect:menu[@"rect"]], location)) { target = menu; break; }
    }
    if (!target || ![target[@"items"] count]) return nil;
    _menuGeneration++;
    _activeMenu = [target copy];
    _menuVisible = YES;
    [self updateSearch:_desiredSearch];
    [self updatePaste:nil];
    [self emit:@{@"kind": @"menu_open", @"id": target[@"id"]}];
    __weak SurcoSurfaces *weakSelf = self;
    NSDictionary *identity = @{@"id": target[@"id"], @"generation": @(_menuGeneration)};
    return [UIContextMenuConfiguration configurationWithIdentifier:identity previewProvider:nil
        actionProvider:^UIMenu *(NSArray<UIMenuElement *> *suggested) { return [weakSelf menu:target]; }];
}
- (UITargetedPreview *)preview {
    if (!_activeMenu || !_view.window) return nil;
    CGRect rect = [self rect:_activeMenu[@"rect"]];
    UIView *snapshot = [_view resizableSnapshotViewFromRect:rect afterScreenUpdates:NO withCapInsets:UIEdgeInsetsZero];
    if (!snapshot) return nil;
    UIPreviewParameters *parameters = [UIPreviewParameters new];
    parameters.backgroundColor = UIColor.clearColor;
    parameters.visiblePath = [UIBezierPath bezierPathWithRoundedRect:snapshot.bounds cornerRadius:8];
    UIPreviewTarget *target = [[UIPreviewTarget alloc] initWithContainer:_view center:CGPointMake(CGRectGetMidX(rect), CGRectGetMidY(rect))];
    return [[UITargetedPreview alloc] initWithView:snapshot parameters:parameters target:target];
}
- (BOOL)isCurrentMenu:(UIContextMenuConfiguration *)configuration {
    NSDictionary *identity = (NSDictionary *)configuration.identifier;
    return _valid && [identity[@"generation"] unsignedIntegerValue] == _menuGeneration;
}
- (UITargetedPreview *)contextMenuInteraction:(UIContextMenuInteraction *)interaction previewForHighlightingMenuWithConfiguration:(UIContextMenuConfiguration *)configuration { return [self isCurrentMenu:configuration] ? [self preview] : nil; }
- (UITargetedPreview *)contextMenuInteraction:(UIContextMenuInteraction *)interaction previewForDismissingMenuWithConfiguration:(UIContextMenuConfiguration *)configuration { return [self isCurrentMenu:configuration] ? [self preview] : nil; }
- (void)contextMenuInteraction:(UIContextMenuInteraction *)interaction willEndForConfiguration:(UIContextMenuConfiguration *)configuration animator:(id<UIContextMenuInteractionAnimating>)animator {
    NSDictionary *identity = (NSDictionary *)configuration.identifier;
    NSString *identifier = identity[@"id"];
    NSDictionary *closingMenu = _activeMenu;
    NSUInteger generation = [identity[@"generation"] unsignedIntegerValue];
    __weak SurcoSurfaces *weakSelf = self;
    void (^finished)(void) = ^{
        SurcoSurfaces *owner = weakSelf;
        if (!owner || !owner->_valid || owner->_interaction != interaction || owner->_menuGeneration != generation || owner->_activeMenu != closingMenu) return;
        // Selection is queued first; closing only after UIKit's animation keeps its snapshot alive.
        [owner emit:@{@"kind": @"menu_close", @"id": identifier}];
        owner->_menuVisible = NO;
        owner->_activeMenu = nil;
        [owner updateSearch:owner->_desiredSearch];
        [owner updatePaste:owner->_desiredPaste];
        [owner reconcileSheet];
    };
    if (animator) [animator addCompletion:finished]; else finished();
}
- (UIViewController *)presenter {
    for (UIResponder *responder = _view; responder; responder = responder.nextResponder) {
        if ([responder isKindOfClass:UIViewController.class]) return (UIViewController *)responder;
    }
    return nil;
}
- (void)reconcileSheet {
    if (!_valid || !_view.window || _transitioning || _menuVisible) return;
    NSString *desired = _desiredSheet[@"id"];
    if (_sheet && [_sheetId isEqual:desired]) {
        if ([_sheet isKindOfClass:SurcoSheet.class]) [(SurcoSheet *)_sheet updateModel:_desiredSheet];
        return;
    }
    __weak SurcoSurfaces *weakSelf = self;
    if (_sheet) {
        _transitioning = YES;
        UIViewController *closingSheet = _sheet;
        [closingSheet dismissViewControllerAnimated:!(_reduceMotion || UIAccessibilityIsReduceMotionEnabled()) completion:^{
            SurcoSurfaces *owner = weakSelf;
            if (!owner || !owner->_valid || owner->_sheet != closingSheet) return;
            owner->_sheet = nil;
            owner->_sheetId = nil;
            owner->_transitioning = NO;
            [owner reconcileSheet];
        }];
        return;
    }
    UIViewController *presenter = [self presenter];
    if (!desired || [desired isEqual:_dismissedSheet] || !presenter || presenter.presentedViewController) return;
    NSDictionary *model = _desiredSheet;
    void (^emit)(NSDictionary *) = ^(NSDictionary *event) { [weakSelf emit:event]; };
    if ([model[@"confirmation"] boolValue]) {
        UIAlertController *alert = [UIAlertController alertControllerWithTitle:model[@"title"] message:model[@"description"] preferredStyle:UIAlertControllerStyleAlert];
        [alert addAction:[UIAlertAction actionWithTitle:@"Cancelar" style:UIAlertActionStyleCancel handler:^(UIAlertAction *action) {
            emit(@{@"kind": @"sheet_close", @"sheet": desired});
        }]];
        for (NSDictionary *item in model[@"actions"]) {
            if ([item[@"id"] isEqual:model[@"dismiss"]]) continue;
            [alert addAction:[UIAlertAction actionWithTitle:item[@"title"] style:[item[@"destructive"] boolValue] ? UIAlertActionStyleDestructive : UIAlertActionStyleDefault handler:^(UIAlertAction *action) {
                emit(@{@"kind": @"sheet_action", @"sheet": desired, @"action": item[@"id"]});
            }]];
        }
        _sheet = alert;
    } else {
        SurcoSheet *sheet = [[SurcoSheet alloc] initWithModel:model emit:emit];
        sheet.modalPresentationStyle = UIModalPresentationPageSheet;
        UISheetPresentationController *presentation = sheet.sheetPresentationController;
        presentation.detents = @[[UISheetPresentationControllerDetent mediumDetent], [UISheetPresentationControllerDetent largeDetent]];
        presentation.selectedDetentIdentifier = [model[@"large"] boolValue] ? UISheetPresentationControllerDetentIdentifierLarge : UISheetPresentationControllerDetentIdentifierMedium;
        presentation.prefersGrabberVisible = YES;
        presentation.prefersScrollingExpandsWhenScrolledToEdge = YES;
        presentation.prefersEdgeAttachedInCompactHeight = YES;
        presentation.widthFollowsPreferredContentSizeWhenEdgeAttached = YES;
        presentation.delegate = self;
        _sheet = sheet;
    }
    _sheet.overrideUserInterfaceStyle = UIUserInterfaceStyleDark;
    _sheet.view.backgroundColor = SurcoSurfaceColor();
    _sheet.view.tintColor = UIColor.whiteColor;
    _sheetId = desired;
    _transitioning = YES;
    UIViewController *openingSheet = _sheet;
    [presenter presentViewController:openingSheet animated:!(_reduceMotion || UIAccessibilityIsReduceMotionEnabled()) completion:^{
        SurcoSurfaces *owner = weakSelf;
        if (!owner || !owner->_valid || owner->_sheet != openingSheet) return;
        owner->_transitioning = NO;
        [owner reconcileSheet];
    }];
}
- (void)presentationControllerDidDismiss:(UIPresentationController *)presentationController {
    if (presentationController.presentedViewController != _sheet) return;
    NSString *identifier = _sheetId;
    _dismissedSheet = identifier;
    _sheet = nil;
    _sheetId = nil;
    [self emit:@{@"kind": @"sheet_close", @"sheet": identifier}];
}
@end
