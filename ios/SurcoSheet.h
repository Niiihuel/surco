#import <UIKit/UIKit.h>

@interface SurcoSheet : UIViewController
@property(nonatomic, readonly) NSString *surfaceId;
- (instancetype)initWithModel:(NSDictionary *)model emit:(void (^)(NSDictionary *))emit;
- (void)updateModel:(NSDictionary *)model;
/// The height its rows ask for, or 0 before it is laid out.
@property(nonatomic, readonly) CGFloat fittingHeight;
@end
