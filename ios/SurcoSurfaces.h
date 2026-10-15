#import <UIKit/UIKit.h>

/// UIKit owns presentation/gestures. Rust owns the models and validates every event.
@interface SurcoSurfaces : NSObject
- (instancetype)initWithWake:(void (^)(void))wake;
- (void)updatePayload:(NSDictionary *)payload view:(UIView *)view scale:(CGFloat)scale;
- (NSArray<NSDictionary *> *)takeEvents;
- (void)refresh;
- (void)invalidate;
@end
