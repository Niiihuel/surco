// Public UIKit bridge. All entry points and notification delivery run on the main thread.
#import <UIKit/UIKit.h>
#import <AVKit/AVKit.h>
#import <AVFAudio/AVFAudio.h>
#import "SurcoSurfaces.h"
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <stdatomic.h>

// Whether Surco is on screen. UIKit's notifications write it on the main thread; the
// backend reads it from its own, to stop waking a window nobody can see.
static atomic_int SurcoOnScreen = 1;
// Set when iOS runs short of memory; the next frame lets the decoded covers go.
static atomic_int SurcoMemoryWarned = 0;
static void SurcoWatchScreen(void) {
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        NSNotificationCenter *center = NSNotificationCenter.defaultCenter;
        [center addObserverForName:UIApplicationDidEnterBackgroundNotification object:nil queue:nil
            usingBlock:^(NSNotification *note) { atomic_store(&SurcoOnScreen, 0); }];
        [center addObserverForName:UIApplicationWillEnterForegroundNotification object:nil queue:nil
            usingBlock:^(NSNotification *note) { atomic_store(&SurcoOnScreen, 1); }];
        [center addObserverForName:UIApplicationDidReceiveMemoryWarningNotification object:nil queue:nil
            usingBlock:^(NSNotification *note) { atomic_store(&SurcoMemoryWarned, 1); }];
    });
}
int surco_ui_on_screen(void) { return atomic_load(&SurcoOnScreen); }
int surco_ui_take_memory_warning(void) { return atomic_exchange(&SurcoMemoryWarned, 0); }

typedef void (*SurcoUiWake)(void *context);

typedef struct {
    double width, height, scale;
    double safe_top, safe_right, safe_bottom, safe_left;
    double keyboard_x, keyboard_y, keyboard_width, keyboard_height;
    uint64_t route_revision;
} SurcoUiGeometry;

@interface SurcoNativeView : NSObject {
    __weak UIView *_view;
    __weak UIScreen *_keyboardScreen;
    AVRoutePickerView *_picker;
    SurcoSurfaces *_surfaces;
    NSMutableArray<id> *_observers;
    CGRect _keyboardFrame;
    uint64_t _routeRevision;
    SurcoUiWake _wake;
    void *_context;
}
- (instancetype)initWithWake:(SurcoUiWake)wake context:(void *)context;
- (void)requestRepaint;
- (void)ensurePicker;
- (void)invalidate;
- (void)updateView:(UIView *)view geometry:(SurcoUiGeometry *)geometry;
- (char *)copyRoute;
- (void)placePicker:(CGRect)rect visible:(BOOL)visible;
- (void)setTint:(UIColor *)tint active:(UIColor *)active;
- (void)updateSurfaces:(NSDictionary *)payload scale:(CGFloat)scale;
- (NSArray<NSDictionary *> *)takeSurfaceEvents;
@end

@implementation SurcoNativeView
- (instancetype)initWithWake:(SurcoUiWake)wake context:(void *)context {
    self = [super init];
    if (!self) return nil;
    _wake = wake;
    _context = context;
    _keyboardFrame = CGRectNull;
    _routeRevision = 1;
    _observers = [NSMutableArray array];
    __weak SurcoNativeView *weakSelf = self;
    _surfaces = [[SurcoSurfaces alloc] initWithWake:^{ [weakSelf requestRepaint]; }];
    NSNotificationCenter *center = NSNotificationCenter.defaultCenter;
    id frame = [center addObserverForName:UIKeyboardWillChangeFrameNotification object:nil
                                   queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *note) {
        SurcoNativeView *owner = weakSelf;
        if (!owner) return;
        NSNumber *local = note.userInfo[UIKeyboardIsLocalUserInfoKey];
        NSValue *value = note.userInfo[UIKeyboardFrameEndUserInfoKey];
        owner->_keyboardFrame = value && (!local || local.boolValue) ? value.CGRectValue : CGRectNull;
        // Since iOS 16.1 the notification identifies the keyboard's screen explicitly.
        owner->_keyboardScreen = [note.object isKindOfClass:UIScreen.class] ? note.object : nil;
        [owner requestRepaint];
    }];
    id hide = [center addObserverForName:UIKeyboardWillHideNotification object:nil
                                  queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *note) {
        SurcoNativeView *owner = weakSelf;
        if (!owner) return;
        owner->_keyboardFrame = CGRectNull;
        owner->_keyboardScreen = nil;
        [owner requestRepaint];
    }];
    id motion = [center addObserverForName:UIAccessibilityReduceMotionStatusDidChangeNotification object:nil
                                    queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *note) {
        [weakSelf requestRepaint];
    }];
    // Off screen the backend stops waking the window; coming back draws it at once.
    id foreground = [center addObserverForName:UIApplicationWillEnterForegroundNotification object:nil
                                        queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *note) {
        [weakSelf requestRepaint];
    }];
    [_observers addObjectsFromArray:@[frame, hide, motion, foreground]];
    for (NSNotificationName name in @[AVAudioSessionRouteChangeNotification,
                                     AVAudioSessionMediaServicesWereResetNotification]) {
        id token = [center addObserverForName:name object:nil queue:NSOperationQueue.mainQueue
                                  usingBlock:^(NSNotification *note) {
            SurcoNativeView *owner = weakSelf;
            if (!owner) return;
            owner->_routeRevision += 1;
            [owner requestRepaint];
        }];
        [_observers addObject:token];
    }
    return self;
}

- (void)requestRepaint {
    if (_wake) _wake(_context);
}

- (void)invalidate {
    // A queued observer can still upgrade weakSelf while this method runs. Clearing the
    // callback first makes it harmless before Rust releases its Context allocation.
    _wake = NULL;
    _context = NULL;
    [_surfaces invalidate];
    _surfaces = nil;
    for (id token in _observers) [NSNotificationCenter.defaultCenter removeObserver:token];
    [_observers removeAllObjects];
    _picker.hidden = YES;
    _picker.userInteractionEnabled = NO;
    [_picker removeFromSuperview];
    _picker = nil;
    _view = nil;
}

- (void)updateView:(UIView *)view geometry:(SurcoUiGeometry *)geometry {
    BOOL viewChanged = _view != view;
    if (viewChanged) {
        [_picker removeFromSuperview];
        _picker.hidden = YES;
        _picker.userInteractionEnabled = NO;
        _view = view;
    }
    *geometry = (SurcoUiGeometry){0};
    geometry->route_revision = _routeRevision;
    if (!view) return;
    // A replaced view is attached by updatePayload later in this frame.
    if (!viewChanged) [_surfaces refresh];
    CGRect bounds = view.bounds;
    geometry->width = bounds.size.width;
    geometry->height = bounds.size.height;
    geometry->scale = view.contentScaleFactor;
    UIEdgeInsets safe = view.safeAreaInsets;
    geometry->safe_top = safe.top;
    geometry->safe_right = safe.right;
    geometry->safe_bottom = safe.bottom;
    geometry->safe_left = safe.left;
    UIScreen *screen = _keyboardScreen ?: view.window.screen;
    if (CGRectIsNull(_keyboardFrame) || !screen || screen != view.window.screen) return;
    // Keyboard notification frames belong to the screen, which may differ from this
    // window's frame in Split View / Stage Manager. Do not subtract safeAreaInsets here.
    CGRect keyboard = [screen.coordinateSpace convertRect:_keyboardFrame toCoordinateSpace:view];
    geometry->keyboard_x = keyboard.origin.x - bounds.origin.x;
    geometry->keyboard_y = keyboard.origin.y - bounds.origin.y;
    geometry->keyboard_width = keyboard.size.width;
    geometry->keyboard_height = keyboard.size.height;
}

- (char *)copyRoute {
    NSArray<AVAudioSessionPortDescription *> *outputs = AVAudioSession.sharedInstance.currentRoute.outputs;
    if (outputs.count == 0) return NULL;
    NSMutableArray<NSString *> *names = [NSMutableArray array];
    for (AVAudioSessionPortDescription *output in outputs) {
        if (output.portName.length > 0) [names addObject:output.portName];
    }
    NSDictionary *route = @{@"name": [names componentsJoinedByString:@", "],
                            @"port_type": outputs.firstObject.portType};
    NSError *error = nil;
    NSData *data = [NSJSONSerialization dataWithJSONObject:route options:0 error:&error];
    if (!data) {
        NSLog(@"Surco could not serialize the audio route: %@", error.localizedDescription);
        return NULL;
    }
    NSString *json = [[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding];
    return json ? strdup(json.UTF8String) : NULL;
}

- (void)ensurePicker {
    if (_picker) return;
    _picker = [[AVRoutePickerView alloc] initWithFrame:CGRectZero];
    _picker.prioritizesVideoDevices = NO;
    _picker.tintColor = UIColor.whiteColor;
    _picker.activeTintColor = UIColor.whiteColor;
    _picker.hidden = YES;
    _picker.userInteractionEnabled = NO;
}

- (void)placePicker:(CGRect)rect visible:(BOOL)visible {
    UIView *view = _view;
    CGRect bounds = view.bounds;
    rect.origin.x += bounds.origin.x;
    rect.origin.y += bounds.origin.y;
    visible = visible && view.window && !CGRectIsEmpty(rect) && CGRectContainsRect(bounds, rect);
    if (!visible) {
        _picker.hidden = YES;
        _picker.userInteractionEnabled = NO;
        return;
    }
    [self ensurePicker];
    if (_picker.superview != view) [view addSubview:_picker];
    _picker.frame = rect;
    _picker.hidden = NO;
    _picker.userInteractionEnabled = YES;
    [view bringSubviewToFront:_picker];
}

- (void)setTint:(UIColor *)tint active:(UIColor *)active {
    [self ensurePicker];
    _picker.tintColor = tint;
    _picker.activeTintColor = active;
}
- (void)updateSurfaces:(NSDictionary *)payload scale:(CGFloat)scale {
    [self ensurePicker];
    id label = payload[@"route_label"];
    _picker.accessibilityLabel = [label isKindOfClass:NSString.class] ? label : @"";
    [_surfaces updatePayload:payload view:_view scale:scale];
}
- (NSArray<NSDictionary *> *)takeSurfaceEvents { return [_surfaces takeEvents]; }
@end

void *surco_ui_create(SurcoUiWake wake, void *context) {
    if (!NSThread.isMainThread || !wake || !context) return NULL;
    SurcoWatchScreen();
    return (__bridge_retained void *)[[SurcoNativeView alloc] initWithWake:wake context:context];
}

void surco_ui_destroy(void *handle) {
    // Rust's !Send/!Sync owner is created and dropped on the app main thread.
    NSCAssert(NSThread.isMainThread, @"Surco native UI must be destroyed on the main thread");
    SurcoNativeView *owner = (__bridge_transfer SurcoNativeView *)handle;
    [owner invalidate];
}

void surco_ui_update(void *handle, void *view, SurcoUiGeometry *geometry) {
    NSCAssert(NSThread.isMainThread, @"Surco native UI must run on the main thread");
    [(__bridge SurcoNativeView *)handle updateView:(__bridge UIView *)view geometry:geometry];
}

char *surco_ui_copy_route(void *handle) {
    NSCAssert(NSThread.isMainThread, @"Surco native UI must run on the main thread");
    return [(__bridge SurcoNativeView *)handle copyRoute];
}

void surco_ui_free_string(char *value) { free(value); }

int surco_reduce_motion_enabled(void) {
    NSCAssert(NSThread.isMainThread, @"Surco accessibility preferences must run on the main thread");
    return UIAccessibilityIsReduceMotionEnabled() ? 1 : 0;
}

int surco_ui_copy_text(const uint8_t *bytes, size_t length) {
    NSCAssert(NSThread.isMainThread, @"Surco clipboard must run on the main thread");
    NSString *text = [[NSString alloc] initWithBytes:bytes length:length encoding:NSUTF8StringEncoding];
    if (!text) return 0;
    UIPasteboard.generalPasteboard.string = text;
    return 1;
}

uint8_t *surco_ui_paste_text(size_t *length) {
    NSCAssert(NSThread.isMainThread, @"Surco clipboard must run on the main thread");
    *length = 0;
    // Called only after a user paste request; UIKit owns any privacy permission prompt.
    NSString *text = UIPasteboard.generalPasteboard.string;
    if (!text) return NULL;
    NSData *data = [text dataUsingEncoding:NSUTF8StringEncoding];
    if (!data) return NULL;
    uint8_t *copy = malloc(MAX(data.length, 1));
    if (!copy) return NULL;
    if (data.length) memcpy(copy, data.bytes, data.length);
    *length = data.length;
    return copy;
}

void surco_ui_picker(void *handle, double x, double y, double w, double h, int visible) {
    NSCAssert(NSThread.isMainThread, @"Surco native UI must run on the main thread");
    [(__bridge SurcoNativeView *)handle placePicker:CGRectMake(x, y, w, h) visible:visible != 0];
}

static UIColor *SurcoUiColor(uint32_t rgba) {
    return [UIColor colorWithRed:((rgba >> 24) & 255) / 255.0
                          green:((rgba >> 16) & 255) / 255.0
                           blue:((rgba >> 8) & 255) / 255.0
                          alpha:(rgba & 255) / 255.0];
}

void surco_ui_picker_colors(void *handle, uint32_t tint, uint32_t active) {
    NSCAssert(NSThread.isMainThread, @"Surco native UI must run on the main thread");
    [(__bridge SurcoNativeView *)handle setTint:SurcoUiColor(tint) active:SurcoUiColor(active)];
}

int surco_ui_surfaces(void *handle, const uint8_t *bytes, size_t length, double scale) {
    NSCAssert(NSThread.isMainThread, @"Surco native surfaces must run on the main thread");
    if (!isfinite(scale) || scale <= 0) return 0;
    NSData *data = [NSData dataWithBytes:bytes length:length];
    id payload = [NSJSONSerialization JSONObjectWithData:data options:0 error:NULL];
    if (![payload isKindOfClass:NSDictionary.class]) return 0;
    [(__bridge SurcoNativeView *)handle updateSurfaces:payload scale:scale];
    return 1;
}

char *surco_ui_take_surface_events(void *handle) {
    NSCAssert(NSThread.isMainThread, @"Surco native surfaces must run on the main thread");
    NSArray *events = [(__bridge SurcoNativeView *)handle takeSurfaceEvents];
    NSData *data = [NSJSONSerialization dataWithJSONObject:events ?: @[] options:0 error:NULL];
    if (!data) return NULL;
    NSString *json = [[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding];
    return json ? strdup(json.UTF8String) : NULL;
}

// White premultiplied RGBA coverage for limn's tintable, size-cached icon textures.
// Symbols are loaded from this device's UIKit; no Apple artwork is bundled or exported.
uint8_t *surco_ui_symbol(const uint8_t *name, size_t length, uint16_t pixels) {
    NSCAssert(NSThread.isMainThread, @"SF Symbols must be rendered on the main thread");
    if (!name || !length || length > 128 || !pixels || pixels > 512) return NULL;
    @autoreleasepool {
        NSString *symbolName = [[NSString alloc] initWithBytes:name length:length encoding:NSUTF8StringEncoding];
        if (!symbolName.length) return NULL;
        UIImageSymbolConfiguration *configuration = [UIImageSymbolConfiguration configurationWithPointSize:pixels weight:UIImageSymbolWeightRegular];
        UIImage *image = [UIImage systemImageNamed:symbolName withConfiguration:configuration];
        if (!image || image.size.width <= 0 || image.size.height <= 0) return NULL;
        image = [image imageWithTintColor:UIColor.whiteColor renderingMode:UIImageRenderingModeAlwaysOriginal];
        size_t rowBytes = (size_t)pixels * 4;
        uint8_t *bytes = calloc(pixels, rowBytes);
        if (!bytes) return NULL;
        CGColorSpaceRef colorSpace = CGColorSpaceCreateDeviceRGB();
        CGContextRef context = CGBitmapContextCreate(bytes, pixels, pixels, 8, rowBytes, colorSpace,
                                                    kCGImageAlphaPremultipliedLast | kCGBitmapByteOrder32Big);
        CGColorSpaceRelease(colorSpace);
        if (!context) { free(bytes); return NULL; }
        CGFloat factor = pixels / MAX(image.size.width, image.size.height);
        CGSize size = CGSizeMake(image.size.width * factor, image.size.height * factor);
        CGRect rect = CGRectMake((pixels - size.width) / 2, (pixels - size.height) / 2, size.width, size.height);
        CGContextTranslateCTM(context, 0, pixels);
        CGContextScaleCTM(context, 1, -1);
        UIGraphicsPushContext(context);
        [image drawInRect:rect];
        UIGraphicsPopContext();
        CGContextRelease(context);
        return bytes;
    }
}

void surco_ui_free_symbol(uint8_t *bytes) { free(bytes); }
