#import "SurcoSurfaces.h"
#import "SurcoSheet.h"
#include <math.h>

static UIColor *SurcoSurfaceColor(void) { return [UIColor colorWithWhite:28.0 / 255.0 alpha:1]; }
static NSString *SurcoString(id value) { return [value isKindOfClass:NSString.class] ? value : @""; }

// A real UIPasteControl recipient. No pasteboard reads occur while checking availability.
// https://developer.apple.com/documentation/uikit/uipastecontrol
@interface SurcoPasteTarget : UIResponder
@property(nonatomic, copy) void (^receive)(NSString *);
@end

@implementation SurcoPasteTarget
- (instancetype)init {
    if ((self = [super init])) {
        self.pasteConfiguration = [[UIPasteConfiguration alloc] initWithAcceptableTypeIdentifiers:@[@"public.url", @"public.utf8-plain-text", @"public.text"]];
    }
    return self;
}
- (void)pasteItemProviders:(NSArray<NSItemProvider *> *)providers {
    NSItemProvider *chosen = nil;
    Class type = NSString.class;
    for (NSItemProvider *provider in providers) {
        if ([provider canLoadObjectOfClass:NSURL.class]) { chosen = provider; type = NSURL.class; break; }
        if (!chosen && [provider canLoadObjectOfClass:NSString.class]) chosen = provider;
    }
    if (!chosen) return;
    void (^receive)(NSString *) = self.receive;
    [chosen loadObjectOfClass:type completionHandler:^(id<NSItemProviderReading> item, NSError *error) {
        NSString *text = [item isKindOfClass:NSURL.class] ? [(NSURL *)item absoluteString] :
                         [item isKindOfClass:NSString.class] ? (NSString *)item : nil;
        if (!error && text.length && text.length <= 65536) {
            dispatch_async(dispatch_get_main_queue(), ^{ if (receive) receive(text); });
        }
    }];
}
@end

@interface SurcoSurfaces () <UIContextMenuInteractionDelegate, UISheetPresentationControllerDelegate> {
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
    NSMutableArray<NSDictionary *> *_events;
    void (^_wake)(void);
}
- (void)emit:(NSDictionary *)event;
- (void)reconcileSheet;
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
    [self updatePaste:_desiredPaste];
    [self reconcileSheet];
}
- (void)refresh {
    [self updatePaste:_desiredPaste];
    [self reconcileSheet];
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
    NSString *text = UIPasteboard.generalPasteboard.string;
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
    [self updatePaste:nil];
    [self emit:@{@"kind": @"menu_open", @"id": target[@"id"]}];
    __weak SurcoSurfaces *weakSelf = self;
    return [UIContextMenuConfiguration configurationWithIdentifier:target[@"id"] previewProvider:nil
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
- (UITargetedPreview *)contextMenuInteraction:(UIContextMenuInteraction *)interaction previewForHighlightingMenuWithConfiguration:(UIContextMenuConfiguration *)configuration { return [self preview]; }
- (UITargetedPreview *)contextMenuInteraction:(UIContextMenuInteraction *)interaction previewForDismissingMenuWithConfiguration:(UIContextMenuConfiguration *)configuration { return [self preview]; }
- (void)contextMenuInteraction:(UIContextMenuInteraction *)interaction willEndForConfiguration:(UIContextMenuConfiguration *)configuration animator:(id<UIContextMenuInteractionAnimating>)animator {
    NSString *identifier = [(NSString *)configuration.identifier copy];
    NSDictionary *closingMenu = _activeMenu;
    NSUInteger generation = _menuGeneration;
    __weak SurcoSurfaces *weakSelf = self;
    void (^finished)(void) = ^{
        SurcoSurfaces *owner = weakSelf;
        if (!owner || !owner->_valid || owner->_interaction != interaction || owner->_menuGeneration != generation || owner->_activeMenu != closingMenu) return;
        // Selection is queued first; closing only after UIKit's animation keeps its snapshot alive.
        [owner emit:@{@"kind": @"menu_close", @"id": identifier}];
        owner->_menuVisible = NO;
        owner->_activeMenu = nil;
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
