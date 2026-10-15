#import <UIKit/UIKit.h>

@interface SurcoSheet : UIViewController
@property(nonatomic, readonly) NSString *surfaceId;
- (instancetype)initWithModel:(NSDictionary *)model emit:(void (^)(NSDictionary *))emit;
- (void)updateModel:(NSDictionary *)model;
@end
