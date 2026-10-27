#import <UIKit/UIKit.h>

/// The current song's cover once SurcoMedia has decoded it for Now Playing, or nil.
UIKIT_EXTERN UIImage *SurcoArtwork(NSString *url);
/// Posted on the main queue when a cover is decoded; its object is the cover's URL.
UIKIT_EXTERN NSNotificationName const SurcoArtworkDidChange;

/// A real UIPasteControl recipient. `receive` runs on the main queue.
@interface SurcoPasteTarget : UIResponder
@property(nonatomic, copy) void (^receive)(NSString *);
@end

/// UIKit owns presentation/gestures. Rust owns the models and validates every event.
@interface SurcoSurfaces : NSObject
- (instancetype)initWithWake:(void (^)(void))wake;
- (void)updatePayload:(NSDictionary *)payload view:(UIView *)view scale:(CGFloat)scale;
- (NSArray<NSDictionary *> *)takeEvents;
- (void)refresh;
- (void)invalidate;
@end
