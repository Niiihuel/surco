#import "SurcoSheet.h"
#import <AVKit/AVKit.h>
#import <math.h>

// The Rust model owns content and actions. UIKit owns scrolling, text editing,
// keyboard dismissal and the sheet's interactive presentation.
// https://developer.apple.com/documentation/uikit/uiscrollview
// https://developer.apple.com/documentation/uikit/uiview/keyboardlayoutguide

static NSString *SheetString(id value) {
    return [value isKindOfClass:NSString.class] ? value : @"";
}
static NSArray *SheetArray(id value) {
    return [value isKindOfClass:NSArray.class] ? value : @[];
}
static BOOL SheetBool(id value) {
    return [value isKindOfClass:NSNumber.class] && [value boolValue];
}
static double SheetNumber(id value, double fallback) {
    double number = [value isKindOfClass:NSNumber.class] ? [value doubleValue] : fallback;
    return isfinite(number) ? number : fallback;
}
static NSString *SheetSliderValue(UISlider *slider) {
    BOOL volume = [slider.accessibilityIdentifier isEqualToString:@"volume"];
    NSString *value = [NSNumberFormatter localizedStringFromNumber:@(slider.value)
        numberStyle:volume ? NSNumberFormatterPercentStyle : NSNumberFormatterDecimalStyle];
    return [slider.accessibilityIdentifier hasPrefix:@"equalizer:"] ? [value stringByAppendingString:@" dB"] : value;
}
static UIColor *SheetSurface(void) { return [UIColor colorWithWhite:28.0 / 255.0 alpha:1]; }
static UIColor *SheetMuted(void) { return [UIColor colorWithWhite:0.72 alpha:1]; }
static UILabel *SheetLabel(UIFontTextStyle style, BOOL muted) {
    UILabel *label = [UILabel new];
    label.font = [UIFont preferredFontForTextStyle:style];
    label.adjustsFontForContentSizeCategory = YES;
    label.numberOfLines = 0;
    label.textColor = muted ? SheetMuted() : UIColor.whiteColor;
    return label;
}
static UIStackView *SheetStack(NSArray<UIView *> *views, UILayoutConstraintAxis axis, CGFloat spacing) {
    UIStackView *stack = [[UIStackView alloc] initWithArrangedSubviews:views];
    stack.axis = axis;
    stack.spacing = spacing;
    return stack;
}

@interface SurcoSheetRow : NSObject
@property(nonatomic, strong) UIView *view;
@property(nonatomic, strong) UILabel *title;
@property(nonatomic, strong) UILabel *secondary;
@property(nonatomic, strong) UIView *control;
@property(nonatomic, strong) NSLayoutConstraint *height;
@property(nonatomic, copy) NSDictionary *model;
@end
@implementation SurcoSheetRow
@end

@interface SurcoSheet () <UITextFieldDelegate> {
    NSDictionary *_model;
    void (^_emit)(NSDictionary *);
    UILabel *_heading;
    UILabel *_descriptionLabel;
    UIScrollView *_scroll;
    UIStackView *_body;
    UIStackView *_footer;
    NSArray<SurcoSheetRow *> *_rows;
    NSArray *_structure;
    NSArray<UIButton *> *_footerButtons;
    NSArray<NSString *> *_footerIds;
    __weak UITextField *_editingField;
    CGSize _viewportSize;
    BOOL _revealEditingField;
}
@property(nonatomic, copy, readwrite) NSString *surfaceId;
@end

@implementation SurcoSheet
- (instancetype)initWithModel:(NSDictionary *)model emit:(void (^)(NSDictionary *))emit {
    if ((self = [super initWithNibName:nil bundle:nil])) {
        self.surfaceId = SheetString(model[@"id"]);
        _model = [model copy];
        _emit = [emit copy];
        _rows = @[];
        _footerButtons = @[];
        self.overrideUserInterfaceStyle = UIUserInterfaceStyleDark;
    }
    return self;
}

- (void)viewDidLoad {
    [super viewDidLoad];
    self.view.backgroundColor = SheetSurface();
    self.view.tintColor = UIColor.whiteColor;
    self.view.accessibilityIdentifier = self.surfaceId;

    _heading = SheetLabel(UIFontTextStyleTitle2, NO);
    _heading.accessibilityTraits |= UIAccessibilityTraitHeader;
    _descriptionLabel = SheetLabel(UIFontTextStyleSubheadline, YES);
    UIStackView *copy = SheetStack(@[_heading, _descriptionLabel], UILayoutConstraintAxisVertical, 6);
    UIButton *close = [UIButton buttonWithType:UIButtonTypeSystem];
    [close setImage:[UIImage systemImageNamed:@"xmark"] forState:UIControlStateNormal];
    close.backgroundColor = [UIColor colorWithWhite:1 alpha:0.08];
    close.tintColor = UIColor.whiteColor;
    close.layer.cornerRadius = 22;
    close.accessibilityLabel = @"Cerrar";
    close.accessibilityIdentifier = @"sheet-close";
    [close addTarget:self action:@selector(closeSheet) forControlEvents:UIControlEventTouchUpInside];
    [NSLayoutConstraint activateConstraints:@[
        [close.widthAnchor constraintEqualToConstant:44],
        [close.heightAnchor constraintEqualToConstant:44],
    ]];
    UIStackView *header = SheetStack(@[copy, close], UILayoutConstraintAxisHorizontal, 12);
    header.alignment = UIStackViewAlignmentTop;
    [copy setContentHuggingPriority:UILayoutPriorityDefaultLow forAxis:UILayoutConstraintAxisHorizontal];

    _scroll = [UIScrollView new];
    _scroll.alwaysBounceVertical = YES;
    _scroll.showsVerticalScrollIndicator = YES;
    _scroll.showsHorizontalScrollIndicator = NO;
    _scroll.keyboardDismissMode = UIScrollViewKeyboardDismissModeInteractive;
    _scroll.contentInsetAdjustmentBehavior = UIScrollViewContentInsetAdjustmentNever;
    _scroll.accessibilityIdentifier = @"sheet-scroll";
    _body = SheetStack(@[], UILayoutConstraintAxisVertical, 16);
    _footer = SheetStack(@[], UILayoutConstraintAxisVertical, 8);
    // The whole form can scroll when the keyboard leaves little vertical room.
    UIStackView *content = SheetStack(@[header, _body, _footer], UILayoutConstraintAxisVertical, 20);
    for (UIView *view in @[_scroll, content]) view.translatesAutoresizingMaskIntoConstraints = NO;
    [_scroll addSubview:content];
    [self.view addSubview:_scroll];
    UILayoutGuide *safe = self.view.safeAreaLayoutGuide;
    [NSLayoutConstraint activateConstraints:@[
        [_scroll.topAnchor constraintEqualToAnchor:safe.topAnchor],
        [_scroll.leadingAnchor constraintEqualToAnchor:safe.leadingAnchor],
        [_scroll.trailingAnchor constraintEqualToAnchor:safe.trailingAnchor],
        [_scroll.bottomAnchor constraintEqualToAnchor:self.view.keyboardLayoutGuide.topAnchor],
        [content.topAnchor constraintEqualToAnchor:_scroll.contentLayoutGuide.topAnchor constant:20],
        [content.bottomAnchor constraintEqualToAnchor:_scroll.contentLayoutGuide.bottomAnchor constant:-12],
        [content.leadingAnchor constraintEqualToAnchor:_scroll.contentLayoutGuide.leadingAnchor constant:20],
        [content.trailingAnchor constraintEqualToAnchor:_scroll.contentLayoutGuide.trailingAnchor constant:-20],
        [content.widthAnchor constraintEqualToAnchor:_scroll.frameLayoutGuide.widthAnchor constant:-40],
    ]];
    [self updateModel:_model];
}

- (void)revealEditor {
    UITextField *field = _editingField;
    if (!field.isFirstResponder || _scroll.isDragging || _scroll.bounds.size.height <= 0) return;
    CGRect rect = [field convertRect:field.bounds toView:_scroll];
    [_scroll scrollRectToVisible:CGRectInset(rect, 0, -8) animated:NO];
}

- (void)viewDidLayoutSubviews {
    [super viewDidLayoutSubviews];
    CGSize viewport = _scroll.bounds.size;
    if (_revealEditingField || !CGSizeEqualToSize(viewport, _viewportSize)) {
        _viewportSize = viewport;
        _revealEditingField = NO;
        [self revealEditor];
    }
}

- (UIButton *)actionButton {
    UIButton *button = [UIButton buttonWithType:UIButtonTypeSystem];
    button.contentHorizontalAlignment = UIControlContentHorizontalAlignmentLeading;
    [button.heightAnchor constraintGreaterThanOrEqualToConstant:44].active = YES;
    [button addTarget:self action:@selector(actionTapped:) forControlEvents:UIControlEventTouchUpInside];
    return button;
}

- (void)updateButton:(UIButton *)button action:(NSDictionary *)action footer:(BOOL)footer primary:(BOOL)primary {
    BOOL destructive = SheetBool(action[@"destructive"]);
    BOOL selected = SheetBool(action[@"selected"]);
    UIButtonConfiguration *configuration = primary ? [UIButtonConfiguration filledButtonConfiguration] : [UIButtonConfiguration plainButtonConfiguration];
    configuration.baseBackgroundColor = destructive ? UIColor.systemRedColor : UIColor.whiteColor;
    configuration.baseForegroundColor = primary ? UIColor.blackColor : destructive ? UIColor.systemRedColor : UIColor.whiteColor;
    configuration.contentInsets = NSDirectionalEdgeInsetsMake(12, 12, 12, 12);
    configuration.titleAlignment = footer ? UIButtonConfigurationTitleAlignmentCenter : UIButtonConfigurationTitleAlignmentLeading;
    configuration.titleLineBreakMode = NSLineBreakByWordWrapping;
    configuration.attributedTitle = [[NSAttributedString alloc] initWithString:SheetString(action[@"title"])
        attributes:@{NSFontAttributeName: [UIFont preferredFontForTextStyle:UIFontTextStyleBody]}];
    NSString *subtitle = SheetString(action[@"subtitle"]);
    if (subtitle.length) configuration.attributedSubtitle = [[NSAttributedString alloc] initWithString:subtitle
        attributes:@{NSFontAttributeName: [UIFont preferredFontForTextStyle:UIFontTextStyleFootnote]}];
    NSString *symbol = SheetString(action[@"symbol"]);
    configuration.image = symbol.length ? [UIImage systemImageNamed:symbol] : selected ? [UIImage systemImageNamed:@"checkmark"] : nil;
    configuration.imagePadding = 12;
    button.configuration = configuration;
    button.accessibilityIdentifier = SheetString(action[@"id"]);
    button.contentHorizontalAlignment = footer ? UIControlContentHorizontalAlignmentCenter : UIControlContentHorizontalAlignmentLeading;
    button.selected = selected;
    button.accessibilityTraits = UIAccessibilityTraitButton | (selected ? UIAccessibilityTraitSelected : 0);
}

- (SurcoSheetRow *)makeRow:(NSDictionary *)model {
    SurcoSheetRow *row = [SurcoSheetRow new];
    row.model = model;
    NSString *kind = SheetString(model[@"kind"]);
    row.title = SheetLabel(UIFontTextStyleBody, NO);
    row.secondary = SheetLabel(UIFontTextStyleFootnote, YES);
    if ([kind isEqualToString:@"action"]) {
        row.control = [self actionButton];
        row.view = row.control;
    } else if ([kind isEqualToString:@"field"]) {
        UITextField *field = [UITextField new];
        field.borderStyle = UITextBorderStyleRoundedRect;
        field.backgroundColor = [UIColor colorWithWhite:0.16 alpha:1];
        field.textColor = UIColor.whiteColor;
        field.font = [UIFont preferredFontForTextStyle:UIFontTextStyleBody];
        field.adjustsFontForContentSizeCategory = YES;
        field.clearButtonMode = UITextFieldViewModeWhileEditing;
        field.returnKeyType = UIReturnKeyDone;
        field.delegate = self;
        [field.heightAnchor constraintGreaterThanOrEqualToConstant:48].active = YES;
        [field addTarget:self action:@selector(fieldChanged:) forControlEvents:UIControlEventEditingChanged];
        row.control = field;
        // UITextField already owns the native selection, Paste edit menu and
        // keyboard shortcuts. Keep its full width and never read the pasteboard
        // to enable another control beside it.
        // https://developer.apple.com/documentation/uikit/uieditmenuinteraction
        row.view = field;
    } else if ([kind isEqualToString:@"slider"]) {
        UISlider *slider = [UISlider new];
        slider.continuous = YES;
        slider.minimumTrackTintColor = UIColor.whiteColor;
        [slider.heightAnchor constraintGreaterThanOrEqualToConstant:44].active = YES;
        [slider addTarget:self action:@selector(sliderChanged:) forControlEvents:UIControlEventValueChanged];
        row.control = slider;
        [row.secondary setContentHuggingPriority:UILayoutPriorityRequired forAxis:UILayoutConstraintAxisHorizontal];
        UIStackView *labels = SheetStack(@[row.title, row.secondary], UILayoutConstraintAxisHorizontal, 12);
        labels.alignment = UIStackViewAlignmentFirstBaseline;
        row.view = SheetStack(@[labels, slider], UILayoutConstraintAxisVertical, 4);
    } else if ([kind isEqualToString:@"segments"]) {
        UISegmentedControl *segments = [[UISegmentedControl alloc] initWithItems:@[]];
        segments.apportionsSegmentWidthsByContent = YES;
        [segments setContentCompressionResistancePriority:UILayoutPriorityRequired forAxis:UILayoutConstraintAxisHorizontal];
        [segments addTarget:self action:@selector(segmentChanged:) forControlEvents:UIControlEventValueChanged];
        UIScrollView *strip = [UIScrollView new];
        strip.showsHorizontalScrollIndicator = YES;
        strip.showsVerticalScrollIndicator = NO;
        [strip addSubview:segments];
        segments.translatesAutoresizingMaskIntoConstraints = NO;
        [NSLayoutConstraint activateConstraints:@[
            [segments.leadingAnchor constraintEqualToAnchor:strip.contentLayoutGuide.leadingAnchor],
            [segments.trailingAnchor constraintEqualToAnchor:strip.contentLayoutGuide.trailingAnchor],
            [segments.topAnchor constraintEqualToAnchor:strip.contentLayoutGuide.topAnchor],
            [segments.bottomAnchor constraintEqualToAnchor:strip.contentLayoutGuide.bottomAnchor],
            [segments.heightAnchor constraintEqualToAnchor:strip.frameLayoutGuide.heightAnchor],
            [segments.widthAnchor constraintGreaterThanOrEqualToAnchor:strip.frameLayoutGuide.widthAnchor],
        ]];
        row.height = [strip.heightAnchor constraintEqualToConstant:44];
        row.height.active = YES;
        row.control = segments;
        row.view = strip;
    } else if ([kind isEqualToString:@"route"]) {
        AVRoutePickerView *picker = [AVRoutePickerView new];
        picker.prioritizesVideoDevices = NO;
        picker.tintColor = UIColor.whiteColor;
        picker.activeTintColor = UIColor.whiteColor;
        picker.accessibilityLabel = @"Elegir dispositivo de audio";
        [NSLayoutConstraint activateConstraints:@[
            [picker.widthAnchor constraintEqualToConstant:44],
            [picker.heightAnchor constraintEqualToConstant:44],
        ]];
        UIStackView *copy = SheetStack(@[row.title, row.secondary], UILayoutConstraintAxisVertical, 6);
        UIStackView *stack = SheetStack(@[copy, picker], UILayoutConstraintAxisHorizontal, 12);
        stack.alignment = UIStackViewAlignmentCenter;
        row.control = picker;
        row.view = stack;
    } else {
        row.view = SheetStack(@[row.title, row.secondary], UILayoutConstraintAxisVertical, 6);
    }
    return row;
}

- (void)updateRow:(SurcoSheetRow *)row model:(NSDictionary *)model {
    row.model = model;
    NSString *kind = SheetString(model[@"kind"]);
    NSString *identifier = SheetString(model[@"id"]);
    row.title.text = SheetString(model[@"title"]);
    row.title.textColor = [kind isEqualToString:@"text"] && SheetBool(model[@"secondary"]) ? SheetMuted() : UIColor.whiteColor;
    row.secondary.text = SheetString(model[@"secondary"]);
    if ([kind isEqualToString:@"route"]) row.secondary.text = SheetString(model[@"subtitle"]);
    row.title.hidden = !row.title.text.length;
    row.secondary.hidden = !row.secondary.text.length;
    row.control.accessibilityIdentifier = identifier.length ? identifier : [kind isEqualToString:@"route"] ? @"sheet-route-picker" : @"";
    if ([kind isEqualToString:@"action"]) {
        [self updateButton:(UIButton *)row.control action:model footer:NO primary:NO];
    } else if ([kind isEqualToString:@"field"]) {
        UITextField *field = (UITextField *)row.control;
        NSString *value = SheetString(model[@"value"]);
        if (!field.isFirstResponder && ![field.text isEqualToString:value]) field.text = value;
        field.placeholder = SheetString(model[@"placeholder"]);
        field.accessibilityLabel = field.placeholder;
        BOOL url = [SheetString(model[@"keyboard"]) isEqualToString:@"url"];
        field.keyboardType = url ? UIKeyboardTypeURL : UIKeyboardTypeDefault;
        field.autocorrectionType = url ? UITextAutocorrectionTypeNo : UITextAutocorrectionTypeDefault;
        field.autocapitalizationType = url ? UITextAutocapitalizationTypeNone : UITextAutocapitalizationTypeSentences;
        field.spellCheckingType = url ? UITextSpellCheckingTypeNo : UITextSpellCheckingTypeDefault;
    } else if ([kind isEqualToString:@"slider"]) {
        UISlider *slider = (UISlider *)row.control;
        if (!slider.isTracking) {
            slider.minimumValue = (float)SheetNumber(model[@"min"], 0);
            slider.maximumValue = (float)MAX(slider.minimumValue, SheetNumber(model[@"max"], 1));
            slider.value = (float)SheetNumber(model[@"value"], slider.minimumValue);
        }
        row.secondary.text = SheetSliderValue(slider);
        row.secondary.hidden = NO;
        slider.accessibilityLabel = row.title.text;
        slider.accessibilityValue = row.secondary.text;
    } else if ([kind isEqualToString:@"segments"]) {
        UISegmentedControl *segments = (UISegmentedControl *)row.control;
        NSArray *items = SheetArray(model[@"items"]);
        while (segments.numberOfSegments > items.count) [segments removeSegmentAtIndex:segments.numberOfSegments - 1 animated:NO];
        for (NSUInteger index = 0; index < items.count; index++) {
            NSDictionary *item = [items[index] isKindOfClass:NSDictionary.class] ? items[index] : @{};
            NSString *title = SheetString(item[@"title"]);
            if (index >= segments.numberOfSegments) [segments insertSegmentWithTitle:title atIndex:index animated:NO];
            else if (![[segments titleForSegmentAtIndex:index] isEqualToString:title]) [segments setTitle:title forSegmentAtIndex:index];
        }
        double selected = SheetNumber(model[@"selected"], -1);
        segments.selectedSegmentIndex = selected >= 0 && selected < items.count ? (NSInteger)selected : UISegmentedControlNoSegment;
        UIFont *font = [UIFont preferredFontForTextStyle:UIFontTextStyleSubheadline];
        [segments setTitleTextAttributes:@{NSFontAttributeName:font} forState:UIControlStateNormal];
        row.height.constant = MAX(44, ceil(font.lineHeight + 16));
    }
}

- (NSArray *)structureForRows:(NSArray *)rows {
    NSMutableArray *structure = [NSMutableArray array];
    for (id value in rows) {
        NSDictionary *row = [value isKindOfClass:NSDictionary.class] ? value : @{};
        [structure addObject:@[SheetString(row[@"kind"]), SheetString(row[@"id"])]];
    }
    return structure;
}

- (void)updateModel:(NSDictionary *)model {
    if (![SheetString(model[@"id"]) isEqualToString:self.surfaceId]) return;
    if (_structure && [_model isEqual:model]) return;
    _model = [model copy];
    if (!self.isViewLoaded) return;
    _heading.text = SheetString(model[@"title"]);
    _descriptionLabel.text = SheetString(model[@"description"]);
    _descriptionLabel.hidden = !_descriptionLabel.text.length;
    NSArray *rows = SheetArray(model[@"rows"]);
    NSArray *structure = [self structureForRows:rows];
    if (![_structure isEqual:structure]) {
        // Stable structures update in place, preserving scroll offset, the live
        // editor and selection, route picker, and any slider being manipulated.
        for (UIView *view in _body.arrangedSubviews) { [_body removeArrangedSubview:view]; [view removeFromSuperview]; }
        NSMutableArray *views = [NSMutableArray array];
        for (id value in rows) {
            NSDictionary *item = [value isKindOfClass:NSDictionary.class] ? value : @{};
            SurcoSheetRow *row = [self makeRow:item];
            [views addObject:row];
            [_body addArrangedSubview:row.view];
        }
        _rows = views;
        _structure = structure;
    }
    for (NSUInteger index = 0; index < rows.count; index++) {
        NSDictionary *row = [rows[index] isKindOfClass:NSDictionary.class] ? rows[index] : @{};
        [self updateRow:_rows[index] model:row];
    }
    NSArray *actions = SheetArray(model[@"actions"]);
    NSMutableArray *ids = [NSMutableArray array];
    for (id value in actions) [ids addObject:[value isKindOfClass:NSDictionary.class] ? SheetString(value[@"id"]) : @""];
    if (![_footerIds isEqual:ids]) {
        for (UIView *view in _footer.arrangedSubviews) { [_footer removeArrangedSubview:view]; [view removeFromSuperview]; }
        NSMutableArray *buttons = [NSMutableArray array];
        for (NSUInteger index = 0; index < actions.count; index++) {
            UIButton *button = [self actionButton];
            [_footer addArrangedSubview:button];
            [buttons addObject:button];
        }
        _footerButtons = buttons;
        _footerIds = ids;
    }
    for (NSUInteger index = 0; index < actions.count; index++) {
        NSDictionary *action = [actions[index] isKindOfClass:NSDictionary.class] ? actions[index] : @{};
        [self updateButton:_footerButtons[index] action:action footer:YES primary:index + 1 == actions.count];
    }
}

- (void)traitCollectionDidChange:(UITraitCollection *)previousTraitCollection {
    [super traitCollectionDidChange:previousTraitCollection];
    if (![previousTraitCollection.preferredContentSizeCategory isEqualToString:self.traitCollection.preferredContentSizeCategory]) {
        NSDictionary *model = _model;
        _model = nil;
        [self updateModel:model];
        _revealEditingField = YES;
        [self.view setNeedsLayout];
    }
}

- (void)actionTapped:(UIButton *)button {
    if (_emit && button.accessibilityIdentifier.length) _emit(@{@"kind":@"sheet_action", @"sheet":self.surfaceId, @"action":button.accessibilityIdentifier});
}
- (void)closeSheet {
    NSString *action = SheetString(_model[@"dismiss"]);
    if (_emit) _emit(action.length ? @{@"kind":@"sheet_action", @"sheet":self.surfaceId, @"action":action}
                                  : @{@"kind":@"sheet_close", @"sheet":self.surfaceId});
}
- (void)fieldChanged:(UITextField *)field {
    if (_emit) _emit(@{@"kind":@"sheet_input", @"sheet":self.surfaceId, @"id":field.accessibilityIdentifier ?: @"", @"text":field.text ?: @""});
}
- (void)sliderChanged:(UISlider *)slider {
    for (SurcoSheetRow *row in _rows) {
        if (row.control != slider) continue;
        row.secondary.text = SheetSliderValue(slider);
        slider.accessibilityValue = row.secondary.text;
        break;
    }
    if (_emit) _emit(@{@"kind":@"sheet_slider", @"sheet":self.surfaceId, @"id":slider.accessibilityIdentifier ?: @"", @"value":@(slider.value)});
}
- (void)segmentChanged:(UISegmentedControl *)segments {
    for (SurcoSheetRow *row in _rows) {
        if (row.control != segments) continue;
        NSArray *items = SheetArray(row.model[@"items"]);
        NSInteger index = segments.selectedSegmentIndex;
        if (index < 0 || index >= (NSInteger)items.count) return;
        NSDictionary *item = [items[index] isKindOfClass:NSDictionary.class] ? items[index] : @{};
        NSString *action = SheetString(item[@"id"]);
        if (_emit && action.length) _emit(@{@"kind":@"sheet_action", @"sheet":self.surfaceId, @"action":action});
        break;
    }
}
- (BOOL)textFieldShouldReturn:(UITextField *)textField {
    [textField resignFirstResponder];
    return YES;
}
- (void)textFieldDidBeginEditing:(UITextField *)textField {
    _editingField = textField;
    _revealEditingField = YES;
    [self.view setNeedsLayout];
}
- (void)textFieldDidEndEditing:(UITextField *)textField {
    if (_editingField == textField) _editingField = nil;
}
@end
