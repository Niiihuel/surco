#import "SurcoSurfaces.h"
#import "SurcoSheet.h"
#import <LinkPresentation/LinkPresentation.h>
#include <math.h>

static UIColor *SurcoSurfaceColor(void) { return [UIColor colorWithWhite:28.0 / 255.0 alpha:1]; }
static NSString *SurcoString(id value) { return [value isKindOfClass:NSString.class] ? value : @""; }
static NSArray *SurcoArray(id value) { return [value isKindOfClass:NSArray.class] ? value : @[]; }
static NSDictionary *SurcoDictionary(id value) { return [value isKindOfClass:NSDictionary.class] ? value : @{}; }
// The soft edge below the header, where the blur fades into the page.
static const CGFloat SurcoEdgeFade = 40;
// view.rs MOBILE_HEADER_HEIGHT: the header's row keeps it while Search collapses the slot.
static const CGFloat SurcoHeaderHeight = 56;
// mobile_motion.rs RETURN: the header collapse the search field restyles in step with.
static const NSTimeInterval SurcoSearchMotion = 0.22;
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

/// The link the system share sheet sends, previewed as the track instead of a bare address.
@interface SurcoShareItem : NSObject <UIActivityItemSource>
@property(nonatomic) NSURL *url;
@property(nonatomic) LPLinkMetadata *metadata;
@end
@implementation SurcoShareItem
- (id)activityViewControllerPlaceholderItem:(UIActivityViewController *)controller { return _url; }
- (id)activityViewController:(UIActivityViewController *)controller itemForActivityType:(UIActivityType)type { return _url; }
- (LPLinkMetadata *)activityViewControllerLinkMetadata:(UIActivityViewController *)controller { return _metadata; }
@end
static UIActivityViewController *SurcoShareController(NSDictionary *model, NSDictionary *share) {
    NSURL *url = [NSURL URLWithString:SurcoString(share[@"url"])];
    if (!url) return nil;
    SurcoShareItem *item = [SurcoShareItem new];
    item.url = url;
    item.metadata = [LPLinkMetadata new];
    item.metadata.originalURL = url;
    item.metadata.URL = url;
    NSString *title = SurcoString(model[@"title"]), *artist = SurcoString(model[@"description"]);
    item.metadata.title = artist.length ? [NSString stringWithFormat:@"%@ · %@", title, artist] : title;
    NSURL *artwork = [NSURL URLWithString:SurcoString(share[@"artwork"])];
    if ([artwork.scheme isEqualToString:@"https"]) {
        // The cover loads while the sheet opens; until then the system shows its placeholder.
        NSItemProvider *icon = [NSItemProvider new];
        [icon registerObjectOfClass:UIImage.class visibility:NSItemProviderRepresentationVisibilityAll loadHandler:^NSProgress *(void (^completion)(id<NSItemProviderWriting>, NSError *)) {
            NSURLSessionDataTask *task = [NSURLSession.sharedSession dataTaskWithURL:artwork completionHandler:^(NSData *data, NSURLResponse *response, NSError *error) {
                UIImage *image = data ? [UIImage imageWithData:data] : nil;
                completion(image, image ? nil : (error ?: [NSError errorWithDomain:NSURLErrorDomain code:NSURLErrorCannotDecodeContentData userInfo:nil]));
            }];
            [task resume];
            return task.progress;
        }];
        item.metadata.iconProvider = icon;
    }
    return [[UIActivityViewController alloc] initWithActivityItems:@[item] applicationActivities:nil];
}

// No pasteboard reads occur while checking availability.
// https://developer.apple.com/documentation/uikit/uipastecontrol

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

// Empty chrome passes touches to the canvas below; only its buttons take them.
@interface SurcoChromeView : UIView
@end
@implementation SurcoChromeView
- (UIView *)hitTest:(CGPoint)point withEvent:(UIEvent *)event {
    UIView *hit = [super hitTest:point withEvent:event];
    return hit == self ? nil : hit;
}
@end

@interface SurcoGradientView : UIView
@property(nonatomic, readonly) CAGradientLayer *gradient;
@end
@implementation SurcoGradientView
+ (Class)layerClass { return CAGradientLayer.class; }
- (CAGradientLayer *)gradient { return (CAGradientLayer *)self.layer; }
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
    BOOL _searchStyled;
    BOOL _searchCompact;
    NSDictionary *_desiredChrome;
    // What the bars were last laid out from; refresh runs every frame and mostly finds it unchanged.
    NSDictionary *_appliedChrome;
    CGRect _appliedBounds;
    BOOL _appliedLive;
    UIView *_backdrop;
    NSArray<SurcoGradientView *> *_backdropMasks;
    SurcoGradientView *_shade;
    SurcoChromeView *_header;
    UIStackView *_headerRow;
    UILabel *_title;
    NSArray *_headerButtons;
    SurcoChromeView *_tabs;
    UIStackView *_tabRow;
    NSArray *_tabItems;
    NSMutableArray<NSDictionary *> *_events;
    void (^_wake)(void);
}
- (void)emit:(NSDictionary *)event;
- (void)reconcileSheet;
- (void)updateSearch:(NSDictionary *)model;
- (void)updateChrome:(NSDictionary *)model;
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
    for (UIView *chrome in @[_backdrop ?: NSNull.null, _shade ?: NSNull.null, _header ?: NSNull.null, _tabs ?: NSNull.null]) {
        if ([chrome isKindOfClass:UIView.class]) [chrome removeFromSuperview];
    }
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
        _appliedBounds = CGRectNull;
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
    _desiredChrome = [payload[@"chrome"] isKindOfClass:NSDictionary.class] ? payload[@"chrome"] : nil;
    [self updateChrome:_desiredChrome];
    [self updateSearch:_desiredSearch];
    [self updatePaste:_desiredPaste];
    [self reconcileSheet];
}
- (void)refresh {
    [self updateChrome:_desiredChrome];
    [self updateSearch:_desiredSearch];
    [self updatePaste:_desiredPaste];
    [self reconcileSheet];
}
- (BOOL)chromeAnimates {
    return !_reduceMotion && !UIAccessibilityIsReduceMotionEnabled();
}
// Chrome fades instead of popping while the canvas slides the player over it.
- (void)setChrome:(UIView *)view shown:(BOOL)shown {
    BOOL interactive = shown && view != _backdrop && view != _shade;
    if (!view || (view.alpha == (shown ? 1 : 0) && view.userInteractionEnabled == interactive)) return;
    view.userInteractionEnabled = interactive;
    if (shown) view.hidden = NO;
    void (^change)(void) = ^{ view.alpha = shown ? 1 : 0; };
    if (![self chromeAnimates] || !view.window) { change(); return; }
    [UIView animateWithDuration:0.22 delay:0 options:UIViewAnimationOptionBeginFromCurrentState |
        UIViewAnimationOptionAllowUserInteraction animations:change completion:nil];
}
- (void)updateChrome:(NSDictionary *)model {
    BOOL live = model && _view.window;
    if (live == _appliedLive && CGRectEqualToRect(_view.bounds, _appliedBounds)
        && (_appliedChrome == model || [_appliedChrome isEqual:model])) return;
    _appliedChrome = model;
    _appliedBounds = _view.bounds;
    _appliedLive = live;
    CGRect header = live ? [self rect:model[@"header"]] : CGRectNull;
    CGRect tabs = live ? [self rect:model[@"tabs"]] : CGRectNull;
    double edge = live && [model[@"edge"] isKindOfClass:NSNumber.class] ? [model[@"edge"] doubleValue] : 0;
    edge = isfinite(edge) && edge > 0 ? edge * _scale : 0;
    [self updateBackdrop:edge tint:SurcoArray(model[@"tint"])];
    [self updateHeader:model rect:header];
    [self updateTabs:SurcoArray(model[@"items"]) rect:tabs];
}
- (void)updateBackdrop:(CGFloat)edge tint:(NSArray *)tint {
    if (edge <= 0) { [self setChrome:_backdrop shown:NO]; [self setChrome:_shade shown:NO]; return; }
    if (!_backdrop) {
        // iOS 26's soft scroll edge, from public parts: blurs stacked under masks that end
        // at different depths, so rows blur more the nearer they are to the top of the
        // screen, and a wash of the page's color that deepens the same way. (The system's
        // own UIScrollEdgeEffect only blurs a UIScrollView's content, never this canvas.)
        // https://developer.apple.com/documentation/uikit/uiscrolledgeeffect
        _backdrop = [UIView new];
        NSMutableArray *masks = [NSMutableArray array];
        for (NSUInteger index = 0; index < 3; index++) {
            UIVisualEffectView *blur = [[UIVisualEffectView alloc] initWithEffect:[UIBlurEffect effectWithStyle:UIBlurEffectStyleSystemUltraThinMaterialDark]];
            SurcoGradientView *mask = [SurcoGradientView new];
            mask.gradient.colors = @[(id)UIColor.blackColor.CGColor, (id)UIColor.blackColor.CGColor, (id)UIColor.clearColor.CGColor];
            blur.maskView = mask;
            blur.autoresizingMask = UIViewAutoresizingFlexibleWidth | UIViewAutoresizingFlexibleHeight;
            [_backdrop addSubview:blur];
            [masks addObject:mask];
        }
        _backdropMasks = masks;
        _shade = [SurcoGradientView new];
        for (UIView *view in @[_backdrop, _shade]) { view.userInteractionEnabled = NO; view.alpha = 0; }
    }
    if (_backdrop.superview != _view) { [_view addSubview:_backdrop]; [_view addSubview:_shade]; }
    CGRect bounds = _view.bounds;
    CGRect frame = CGRectMake(bounds.origin.x, bounds.origin.y, bounds.size.width, edge + SurcoEdgeFade);
    _backdrop.frame = frame;
    // The first blur reaches below the header, the last only covers the status bar.
    for (NSUInteger index = 0; index < _backdropMasks.count; index++) {
        SurcoGradientView *mask = _backdropMasks[index];
        UIView *blur = _backdrop.subviews[index];
        blur.frame = _backdrop.bounds;
        mask.frame = blur.bounds;
        double end = 1.0 - 0.2 * index;
        mask.gradient.locations = @[@0, @(MAX(0, end - 0.45)), @(end)];
    }
    CGFloat channel[3] = {18, 18, 18};
    for (NSUInteger index = 0; index < 3 && index < tint.count; index++) {
        if ([tint[index] isKindOfClass:NSNumber.class]) channel[index] = MAX(0, MIN(255, [tint[index] doubleValue]));
    }
    UIColor *color = [UIColor colorWithRed:channel[0] / 255 green:channel[1] / 255 blue:channel[2] / 255 alpha:1];
    _shade.frame = frame;
    // Darkest over the status bar, still holding the header, gone just below it.
    _shade.gradient.colors = @[(id)[color colorWithAlphaComponent:0.86].CGColor, (id)[color colorWithAlphaComponent:0.62].CGColor,
                               (id)[color colorWithAlphaComponent:0].CGColor];
    _shade.gradient.locations = @[@0, @(MAX(0, MIN(1, edge / frame.size.height))), @1];
    [self setChrome:_backdrop shown:YES];
    [self setChrome:_shade shown:YES];
}
- (UIButton *)chromeButton:(id)value {
    NSDictionary *item = SurcoDictionary(value);
    NSString *identifier = SurcoString(item[@"id"]);
    UIButtonConfiguration *configuration = nil;
    // Liquid Glass circles on iOS 26, a quiet translucent circle before it.
    if (@available(iOS 26.0, *)) configuration = [UIButtonConfiguration glassButtonConfiguration];
    if (!configuration) {
        configuration = [UIButtonConfiguration plainButtonConfiguration];
        configuration.background.backgroundColor = [UIColor colorWithWhite:1 alpha:0.1];
    }
    configuration.cornerStyle = UIButtonConfigurationCornerStyleCapsule;
    configuration.baseForegroundColor = UIColor.whiteColor;
    configuration.image = [UIImage systemImageNamed:SurcoString(item[@"symbol"])];
    configuration.preferredSymbolConfigurationForImage = [UIImageSymbolConfiguration configurationWithPointSize:17 weight:UIImageSymbolWeightSemibold];
    __weak SurcoSurfaces *weakSelf = self;
    UIButton *button = [UIButton buttonWithConfiguration:configuration primaryAction:[UIAction actionWithHandler:^(__kindof UIAction *action) {
        [weakSelf emit:@{@"kind":@"chrome", @"id":identifier}];
    }]];
    button.accessibilityIdentifier = identifier;
    button.accessibilityLabel = SurcoString(item[@"title"]);
    [NSLayoutConstraint activateConstraints:@[
        [button.widthAnchor constraintEqualToConstant:44],
        [button.heightAnchor constraintEqualToConstant:44],
    ]];
    return button;
}
- (void)updateHeader:(NSDictionary *)model rect:(CGRect)rect {
    if (CGRectIsNull(rect) || CGRectIsEmpty(rect)) { [self setChrome:_header shown:NO]; return; }
    if (!_header) {
        _header = [SurcoChromeView new];
        _header.alpha = 0;
        _title = [UILabel new];
        _title.textColor = UIColor.whiteColor;
        _title.adjustsFontForContentSizeCategory = YES;
        _title.accessibilityTraits = UIAccessibilityTraitHeader;
        _title.accessibilityIdentifier = @"surco.native.title";
        [_title setContentHuggingPriority:UILayoutPriorityDefaultLow forAxis:UILayoutConstraintAxisHorizontal];
        [_title setContentCompressionResistancePriority:UILayoutPriorityDefaultLow forAxis:UILayoutConstraintAxisHorizontal];
        _headerRow = [[UIStackView alloc] initWithArrangedSubviews:@[_title]];
        _headerRow.axis = UILayoutConstraintAxisHorizontal;
        _headerRow.alignment = UIStackViewAlignmentCenter;
        _headerRow.spacing = 8;
        _headerRow.translatesAutoresizingMaskIntoConstraints = NO;
        [_header addSubview:_headerRow];
        [NSLayoutConstraint activateConstraints:@[
            [_headerRow.leadingAnchor constraintEqualToAnchor:_header.leadingAnchor constant:16],
            [_headerRow.trailingAnchor constraintEqualToAnchor:_header.trailingAnchor constant:-12],
            // Pinned to the bottom at full height: as Search collapses the slot the
            // title slides up with the field and is cropped, never squeezed or
            // left behind the rising field.
            [_headerRow.heightAnchor constraintEqualToConstant:SurcoHeaderHeight],
            [_headerRow.bottomAnchor constraintEqualToAnchor:_header.bottomAnchor],
        ]];
    }
    if (_header.superview != _view) [_view addSubview:_header];
    _header.frame = rect;
    // Only a collapsing header crops; at rest the glass buttons keep their glow.
    _header.clipsToBounds = CGRectGetHeight(rect) < SurcoHeaderHeight;
    BOOL compact = [model[@"compact"] boolValue];
    UIFont *font = [UIFont systemFontOfSize:compact ? 17 : 24 weight:compact ? UIFontWeightSemibold : UIFontWeightBold];
    _title.font = [[UIFontMetrics metricsForTextStyle:UIFontTextStyleTitle2] scaledFontForFont:font maximumPointSize:compact ? 22 : 30];
    NSString *title = SurcoString(model[@"title"]);
    if (![_title.text isEqualToString:title]) {
        if (_title.text.length && [self chromeAnimates]) {
            CATransition *fade = [CATransition animation];
            fade.type = kCATransitionFade;
            fade.duration = 0.18;
            [_title.layer addAnimation:fade forKey:@"title"];
        }
        _title.text = title;
    }
    NSArray *buttons = @[SurcoArray(model[@"leading"]), SurcoArray(model[@"trailing"])];
    if (![_headerButtons isEqual:buttons]) {
        _headerButtons = buttons;
        for (UIView *view in [_headerRow.arrangedSubviews copy]) {
            if (view == _title) continue;
            [_headerRow removeArrangedSubview:view];
            [view removeFromSuperview];
        }
        NSUInteger index = 0;
        for (id item in buttons[0]) [_headerRow insertArrangedSubview:[self chromeButton:item] atIndex:index++];
        for (id item in buttons[1]) [_headerRow addArrangedSubview:[self chromeButton:item]];
    }
    [self setChrome:_header shown:YES];
}
- (UIButtonConfiguration *)tabConfiguration:(NSDictionary *)item {
    BOOL selected = [item[@"selected"] isKindOfClass:NSNumber.class] && [item[@"selected"] boolValue];
    UIButtonConfiguration *configuration = [UIButtonConfiguration plainButtonConfiguration];
    configuration.image = [UIImage systemImageNamed:SurcoString(item[@"symbol"])];
    configuration.preferredSymbolConfigurationForImage = [UIImageSymbolConfiguration configurationWithPointSize:22
        weight:selected ? UIImageSymbolWeightSemibold : UIImageSymbolWeightRegular];
    configuration.imagePlacement = NSDirectionalRectEdgeTop;
    configuration.imagePadding = 4;
    configuration.contentInsets = NSDirectionalEdgeInsetsMake(4, 2, 2, 2);
    // The iPhone kit's tab bar: white when selected, a quiet gray otherwise.
    configuration.baseForegroundColor = selected ? UIColor.whiteColor : [UIColor colorWithWhite:0.70 alpha:1];
    UIFont *font = [[UIFontMetrics metricsForTextStyle:UIFontTextStyleCaption2]
        scaledFontForFont:[UIFont systemFontOfSize:10 weight:UIFontWeightMedium] maximumPointSize:13];
    configuration.attributedTitle = [[NSAttributedString alloc] initWithString:SurcoString(item[@"title"])
        attributes:@{NSFontAttributeName: font}];
    configuration.titleLineBreakMode = NSLineBreakByTruncatingTail;
    return configuration;
}
- (void)updateTabs:(NSArray *)items rect:(CGRect)rect {
    if (CGRectIsNull(rect) || CGRectIsEmpty(rect) || !items.count) { [self setChrome:_tabs shown:NO]; return; }
    if (!_tabs) {
        _tabs = [SurcoChromeView new];
        _tabs.alpha = 0;
        _tabs.accessibilityTraits = UIAccessibilityTraitTabBar;
        _tabRow = [UIStackView new];
        _tabRow.axis = UILayoutConstraintAxisHorizontal;
        _tabRow.distribution = UIStackViewDistributionFillEqually;
        _tabRow.translatesAutoresizingMaskIntoConstraints = NO;
        [_tabs addSubview:_tabRow];
        [NSLayoutConstraint activateConstraints:@[
            [_tabRow.leadingAnchor constraintEqualToAnchor:_tabs.leadingAnchor],
            [_tabRow.trailingAnchor constraintEqualToAnchor:_tabs.trailingAnchor],
            [_tabRow.topAnchor constraintEqualToAnchor:_tabs.topAnchor],
            [_tabRow.bottomAnchor constraintEqualToAnchor:_tabs.bottomAnchor],
        ]];
    }
    if (_tabs.superview != _view) [_view addSubview:_tabs];
    _tabs.frame = rect;
    [self setChrome:_tabs shown:YES];
    if ([_tabItems isEqual:items]) return;
    NSString *(^selection)(NSArray *) = ^NSString *(NSArray *list) {
        for (id value in list) if ([SurcoDictionary(value)[@"selected"] boolValue]) return SurcoString(SurcoDictionary(value)[@"id"]);
        return nil;
    };
    NSString *before = selection(_tabItems);
    NSString *after = selection(items);
    _tabItems = [items copy];
    NSArray<UIView *> *buttons = _tabRow.arrangedSubviews;
    BOOL same = buttons.count == items.count;
    for (NSUInteger index = 0; same && index < items.count; index++) {
        same = [buttons[index].accessibilityIdentifier isEqualToString:SurcoString(SurcoDictionary(items[index])[@"id"])];
    }
    if (!same) {
        for (UIView *view in buttons) { [_tabRow removeArrangedSubview:view]; [view removeFromSuperview]; }
        __weak SurcoSurfaces *weakSelf = self;
        for (id value in items) {
            NSString *identifier = SurcoString(SurcoDictionary(value)[@"id"]);
            UIButton *button = [UIButton buttonWithConfiguration:[UIButtonConfiguration plainButtonConfiguration]
                primaryAction:[UIAction actionWithHandler:^(__kindof UIAction *action) {
                    [weakSelf emit:@{@"kind":@"chrome", @"id":identifier}];
                }]];
            button.accessibilityIdentifier = identifier;
            [button.heightAnchor constraintGreaterThanOrEqualToConstant:44].active = YES;
            [_tabRow addArrangedSubview:button];
        }
    }
    for (NSUInteger index = 0; index < items.count; index++) {
        NSDictionary *item = SurcoDictionary(items[index]);
        UIButton *button = (UIButton *)_tabRow.arrangedSubviews[index];
        BOOL selected = [item[@"selected"] isKindOfClass:NSNumber.class] && [item[@"selected"] boolValue];
        button.configuration = [self tabConfiguration:item];
        button.accessibilityLabel = SurcoString(item[@"title"]);
        button.accessibilityTraits = UIAccessibilityTraitButton | (selected ? UIAccessibilityTraitSelected : 0);
        if (selected && before && ![before isEqualToString:after ?: @""] && [self chromeAnimates]) {
            // A small spring on the newly selected glyph, like the system tab bar.
            UIImageView *glyph = button.imageView;
            glyph.transform = CGAffineTransformMakeScale(0.82, 0.82);
            [UIView animateWithDuration:0.42 delay:0 usingSpringWithDamping:0.45 initialSpringVelocity:0.6
                options:UIViewAnimationOptionAllowUserInteraction animations:^{ glyph.transform = CGAffineTransformIdentity; } completion:nil];
        }
    }
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
        _searchStyled = NO; // Reappear in the current style without a fade.
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
        _searchBar.accessibilityIdentifier = @"surco.native.search";
        _searchBar.searchTextField.accessibilityIdentifier = @"search";
        _searchBar.searchTextField.font = [UIFont preferredFontForTextStyle:UIFontTextStyleBody];
        _searchBar.searchTextField.adjustsFontForContentSizeCategory = YES;
        _searchBar.tintColor = UIColor.whiteColor;
    }
    NSString *placeholder = SurcoString(model[@"placeholder"]);
    if (![_searchBar.placeholder isEqualToString:placeholder]) {
        // A new language: setting it drops the styled placeholder, so restyle once.
        _searchBar.placeholder = placeholder;
        _searchStyled = NO;
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
    // Restyle only on a change: resetting the placeholder every frame of the
    // header's collapse re-laid the field under its Cancel animation.
    if (!_searchStyled || _searchCompact != compact) [self styleSearch:compact];
    if (_searchBar.showsCancelButton != editing) [_searchBar setShowsCancelButton:editing animated:[self searchAnimates]];
    if (_searchWantsEditing != editing) {
        _searchWantsEditing = editing;
        if (editing && !_searchBar.searchTextField.isFirstResponder) [_searchBar.searchTextField becomeFirstResponder];
        else if (!editing && _searchBar.searchTextField.isFirstResponder) [_searchBar.searchTextField resignFirstResponder];
    }
}
- (BOOL)searchAnimates {
    return [self chromeAnimates] && _searchBar.window;
}
// The Figma kit's white idle field becomes the dark editing field. The colors
// cross-fade over the header's collapse instead of flipping in one frame.
- (void)styleSearch:(BOOL)compact {
    UISearchTextField *field = _searchBar.searchTextField;
    BOOL animated = _searchStyled && [self searchAnimates];
    _searchStyled = YES;
    _searchCompact = compact;
    UIColor *ink = compact ? UIColor.whiteColor : UIColor.blackColor;
    NSString *placeholder = _searchBar.placeholder ?: @"";
    void (^change)(void) = ^{
        field.backgroundColor = compact ? [UIColor colorWithWhite:49.0 / 255.0 alpha:1] : UIColor.whiteColor;
        field.textColor = ink;
        field.tintColor = ink;
        field.leftView.tintColor = ink;
        field.attributedPlaceholder = [[NSAttributedString alloc] initWithString:placeholder
            attributes:@{NSForegroundColorAttributeName: compact ? [UIColor colorWithWhite:0.70 alpha:1] : UIColor.darkGrayColor}];
    };
    if (!animated) { change(); return; }
    [UIView transitionWithView:field duration:SurcoSearchMotion options:UIViewAnimationOptionTransitionCrossDissolve |
        UIViewAnimationOptionAllowUserInteraction | UIViewAnimationOptionBeginFromCurrentState animations:change completion:nil];
}
- (void)searchBarTextDidBeginEditing:(UISearchBar *)searchBar {
    if (![self searchIsLive]) return;
    // Start the editing look with the keyboard rather than a render later;
    // Rust's next payload confirms both (editing is always compact).
    if (!_searchCompact) [self styleSearch:YES];
    if (!searchBar.showsCancelButton) [searchBar setShowsCancelButton:YES animated:[self searchAnimates]];
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
- (void)endSearchEditing {
    [_searchBar.searchTextField resignFirstResponder];
    // The editor lives inside the UIKeyInput canvas. Simulator traces show that
    // resigning only the child can leave that ancestor keyboard-focused.
    // Return/Cancel end editing for this hierarchy, once per explicit gesture.
    // https://developer.apple.com/documentation/uikit/uiview/endediting(_:)
    [_view endEditing:YES];
}
- (void)searchBarSearchButtonClicked:(UISearchBar *)searchBar {
    if (![self searchIsLive]) return;
    [searchBar.searchTextField unmarkText];
    [self sendSearchText];
    [self emit:@{@"kind":@"search_submit", @"session":_searchSession}];
    [self endSearchEditing];
}
- (void)searchBarCancelButtonClicked:(UISearchBar *)searchBar {
    if (![self searchIsLive]) return;
    NSString *session = _searchSession;
    [self endSearchEditing];
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
            [button addTarget:self action:@selector(legacyPaste) forControlEvents:UIControlEventTouchUpInside];
            _paste = button;
        }
    }
    if ([_paste isKindOfClass:UIButton.class]) [(UIButton *)_paste setTitle:SurcoString(slot[@"title"]) forState:UIControlStateNormal];
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
        for (NSDictionary *item in model[@"actions"]) {
            // The dismiss action is the alert's cancel, under the title Rust gave it.
            if ([item[@"id"] isEqual:model[@"dismiss"]]) {
                [alert addAction:[UIAlertAction actionWithTitle:SurcoString(item[@"title"]) style:UIAlertActionStyleCancel handler:^(UIAlertAction *action) {
                    emit(@{@"kind": @"sheet_close", @"sheet": desired});
                }]];
                continue;
            }
            [alert addAction:[UIAlertAction actionWithTitle:item[@"title"] style:[item[@"destructive"] boolValue] ? UIAlertActionStyleDestructive : UIAlertActionStyleDefault handler:^(UIAlertAction *action) {
                emit(@{@"kind": @"sheet_action", @"sheet": desired, @"action": item[@"id"]});
            }]];
        }
        _sheet = alert;
    } else if ([model[@"share"] isKindOfClass:NSDictionary.class]) {
        UIActivityViewController *activity = SurcoShareController(model, model[@"share"]);
        if (!activity) { emit(@{@"kind": @"sheet_close", @"sheet": desired}); _dismissedSheet = desired; return; }
        activity.popoverPresentationController.sourceView = _view;
        activity.popoverPresentationController.sourceRect = CGRectMake(CGRectGetMidX(_view.bounds), CGRectGetMidY(_view.bounds), 1, 1);
        __weak UIActivityViewController *weakActivity = activity;
        activity.completionWithItemsHandler = ^(UIActivityType type, BOOL completed, NSArray *items, NSError *error) {
            // A cancelled extension returns to the share sheet; anything else has dismissed it.
            if (type && !completed) return;
            SurcoSurfaces *owner = weakSelf;
            // Rust closing the sheet dismisses it through reconcileSheet, which owns that path.
            if (!owner || !owner->_valid || owner->_sheet != weakActivity || ![owner->_desiredSheet[@"id"] isEqual:desired]) return;
            owner->_dismissedSheet = desired;
            owner->_sheet = nil;
            owner->_sheetId = nil;
            owner->_transitioning = NO;
            [owner emit:@{@"kind": @"sheet_close", @"sheet": desired}];
        };
        _sheet = activity;
    } else {
        SurcoSheet *sheet = [[SurcoSheet alloc] initWithModel:model emit:emit];
        sheet.modalPresentationStyle = UIModalPresentationPageSheet;
        UISheetPresentationController *presentation = sheet.sheetPresentationController;
        presentation.detents = @[[UISheetPresentationControllerDetent mediumDetent], [UISheetPresentationControllerDetent largeDetent]];
        presentation.selectedDetentIdentifier = [model[@"large"] boolValue] ? UISheetPresentationControllerDetentIdentifierLarge : UISheetPresentationControllerDetentIdentifierMedium;
        if (@available(iOS 16.0, *)) {
            // Sized to its rows, so a short menu leaves no empty sheet below them; a long one still reaches large.
            __weak SurcoSheet *weakSheet = sheet;
            UISheetPresentationControllerDetent *fit = [UISheetPresentationControllerDetent customDetentWithIdentifier:@"surco.fit"
                resolver:^CGFloat(id<UISheetPresentationControllerDetentResolutionContext> context) {
                    CGFloat height = weakSheet.fittingHeight;
                    return height > 0 ? MIN(height, context.maximumDetentValue) : context.maximumDetentValue / 2;
                }];
            presentation.detents = @[fit, [UISheetPresentationControllerDetent largeDetent]];
            presentation.selectedDetentIdentifier = fit.identifier;
        }
        presentation.prefersGrabberVisible = YES;
        presentation.prefersScrollingExpandsWhenScrolledToEdge = YES;
        presentation.prefersEdgeAttachedInCompactHeight = YES;
        presentation.widthFollowsPreferredContentSizeWhenEdgeAttached = YES;
        presentation.delegate = self;
        _sheet = sheet;
    }
    _sheet.overrideUserInterfaceStyle = UIUserInterfaceStyleDark;
    // SurcoSheet paints its own lifted surface.
    if ([_sheet isKindOfClass:UIAlertController.class]) _sheet.view.backgroundColor = SurcoSurfaceColor();
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
