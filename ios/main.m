#import <Foundation/Foundation.h>
#import <UIKit/UIKit.h>
#import <AVFoundation/AVFoundation.h>
#import <PhotosUI/PhotosUI.h>
#import <objc/runtime.h>
#include <stdlib.h>
#include <string.h>

extern int surco_start(void);

// The link Surco was opened with, as a phone's camera hands over the code of an account on
// another device, until the app takes it.
static NSString *SurcoOpenedLink = nil;

static void SurcoKeepLink(NSURL *url) {
    @synchronized ([UIApplication class]) { SurcoOpenedLink = url.absoluteString; }
}

static BOOL SurcoOpenURL(id delegate, SEL selector, UIApplication *application, NSURL *url, NSDictionary *options) {
    SurcoKeepLink(url);
    return YES;
}

// Winit's delegate answers UIKit; once it has, it also takes the links Surco is opened with.
// One that launched Surco comes with the launch.
// https://developer.apple.com/documentation/uikit/uiapplicationdelegate/application(_:open:options:)
static void SurcoWatchLinks(void) {
    [NSNotificationCenter.defaultCenter addObserverForName:UIApplicationDidFinishLaunchingNotification object:nil queue:nil
        usingBlock:^(NSNotification *note) {
            NSURL *url = note.userInfo[UIApplicationLaunchOptionsURLKey];
            if (url) SurcoKeepLink(url);
            Class delegate = [UIApplication.sharedApplication.delegate class];
            if (delegate) class_addMethod(delegate, @selector(application:openURL:options:), (IMP)SurcoOpenURL, "B@:@@@");
        }];
}

// The link Surco was last opened with, once; NULL when there is none. Freed with free().
char *surco_take_opened_link(void) {
    @synchronized ([UIApplication class]) {
        if (!SurcoOpenedLink) return NULL;
        char *link = strdup(SurcoOpenedLink.UTF8String);
        SurcoOpenedLink = nil;
        return link;
    }
}

// What presents over Surco: the key window's root, or what it presents already.
static UIViewController *SurcoTopController(void) {
    UIWindow *window = nil;
    for (UIScene *scene in UIApplication.sharedApplication.connectedScenes) {
        if ([scene isKindOfClass:UIWindowScene.class]) window = ((UIWindowScene *)scene).keyWindow ?: ((UIWindowScene *)scene).windows.firstObject;
        if (window) break;
    }
    UIViewController *top = window.rootViewController;
    while (top.presentedViewController) top = top.presentedViewController;
    return top;
}

static NSString *SurcoText(const char *text) {
    return text ? [NSString stringWithUTF8String:text] : @"";
}

// The camera reading the code another device shows, inside Surco: what it reads goes the way
// of a link Surco is opened with.
@interface SurcoScanner : UIViewController <AVCaptureMetadataOutputObjectsDelegate>
@property (nonatomic, copy) NSString *hint;
@property (nonatomic, copy) NSString *closeLabel;
@end

@implementation SurcoScanner {
    AVCaptureSession *_session;
    AVCaptureVideoPreviewLayer *_preview;
    UIView *_frame;
    BOOL _read;
}

- (void)viewDidLoad {
    [super viewDidLoad];
    self.view.backgroundColor = UIColor.blackColor;
    _session = [AVCaptureSession new];
    AVCaptureDevice *camera = [AVCaptureDevice defaultDeviceWithMediaType:AVMediaTypeVideo];
    AVCaptureDeviceInput *input = camera ? [AVCaptureDeviceInput deviceInputWithDevice:camera error:nil] : nil;
    if (input && [_session canAddInput:input]) [_session addInput:input];
    AVCaptureMetadataOutput *output = [AVCaptureMetadataOutput new];
    if ([_session canAddOutput:output]) {
        [_session addOutput:output];
        [output setMetadataObjectsDelegate:self queue:dispatch_get_main_queue()];
        if ([output.availableMetadataObjectTypes containsObject:AVMetadataObjectTypeQRCode]) output.metadataObjectTypes = @[AVMetadataObjectTypeQRCode];
    }
    _preview = [AVCaptureVideoPreviewLayer layerWithSession:_session];
    _preview.videoGravity = AVLayerVideoGravityResizeAspectFill;
    [self.view.layer addSublayer:_preview];
    // A square to aim at, and what to aim it at.
    _frame = [UIView new];
    _frame.layer.borderColor = UIColor.whiteColor.CGColor;
    _frame.layer.borderWidth = 2;
    _frame.layer.cornerRadius = 16;
    [self.view addSubview:_frame];
    UILabel *hint = [UILabel new];
    hint.text = self.hint;
    hint.textColor = UIColor.whiteColor;
    hint.font = [UIFont systemFontOfSize:17 weight:UIFontWeightSemibold];
    hint.textAlignment = NSTextAlignmentCenter;
    hint.numberOfLines = 0;
    hint.tag = 1;
    [self.view addSubview:hint];
    UIButton *close = [UIButton buttonWithType:UIButtonTypeSystem];
    [close setImage:[UIImage systemImageNamed:@"xmark"] forState:UIControlStateNormal];
    close.tintColor = UIColor.whiteColor;
    close.backgroundColor = [UIColor colorWithWhite:0 alpha:0.5];
    close.layer.cornerRadius = 22;
    close.accessibilityLabel = self.closeLabel;
    close.tag = 2;
    [close addTarget:self action:@selector(shut) forControlEvents:UIControlEventTouchUpInside];
    [self.view addSubview:close];
}

- (void)viewDidLayoutSubviews {
    [super viewDidLayoutSubviews];
    CGRect bounds = self.view.bounds;
    UIEdgeInsets safe = self.view.safeAreaInsets;
    _preview.frame = bounds;
    CGFloat side = MIN(CGRectGetWidth(bounds), CGRectGetHeight(bounds)) * 0.66;
    _frame.frame = CGRectMake((CGRectGetWidth(bounds) - side) / 2, (CGRectGetHeight(bounds) - side) / 2, side, side);
    [self.view viewWithTag:1].frame = CGRectMake(24, CGRectGetMaxY(_frame.frame) + 24, CGRectGetWidth(bounds) - 48, 60);
    [self.view viewWithTag:2].frame = CGRectMake(CGRectGetWidth(bounds) - 16 - 44, safe.top + 8, 44, 44);
}

- (void)viewWillAppear:(BOOL)animated {
    [super viewWillAppear:animated];
    AVCaptureSession *session = _session;
    // Starting takes a moment; off the main thread, as Apple asks.
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{ [session startRunning]; });
}

- (void)viewWillDisappear:(BOOL)animated {
    [super viewWillDisappear:animated];
    AVCaptureSession *session = _session;
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{ [session stopRunning]; });
}

- (void)captureOutput:(AVCaptureOutput *)output didOutputMetadataObjects:(NSArray<__kindof AVMetadataObject *> *)objects fromConnection:(AVCaptureConnection *)connection {
    if (_read) return;
    for (AVMetadataObject *object in objects) {
        if (![object isKindOfClass:AVMetadataMachineReadableCodeObject.class]) continue;
        NSString *text = ((AVMetadataMachineReadableCodeObject *)object).stringValue;
        NSURL *url = text ? [NSURL URLWithString:text] : nil;
        if (!url) continue;
        _read = YES;
        UIImpactFeedbackGenerator *tap = [[UIImpactFeedbackGenerator alloc] initWithStyle:UIImpactFeedbackStyleMedium];
        [tap impactOccurred];
        SurcoKeepLink(url);
        [self shut];
        return;
    }
}

- (void)shut {
    [self dismissViewControllerAnimated:YES completion:nil];
}
@end

// Opens the camera on the code of an account on another device, asking for it first; refused,
// it says so and offers the way to Settings. Strings come from the app's own catalog.
void surco_scan_code(const char *hint, const char *close, const char *denied_title, const char *denied_body, const char *settings, const char *cancel) {
    NSString *hintText = SurcoText(hint), *closeText = SurcoText(close), *title = SurcoText(denied_title), *body = SurcoText(denied_body),
        *settingsText = SurcoText(settings), *cancelText = SurcoText(cancel);
    dispatch_async(dispatch_get_main_queue(), ^{
        void (^present)(void) = ^{
            SurcoScanner *scanner = [SurcoScanner new];
            scanner.hint = hintText;
            scanner.closeLabel = closeText;
            scanner.modalPresentationStyle = UIModalPresentationFullScreen;
            scanner.overrideUserInterfaceStyle = UIUserInterfaceStyleDark;
            [SurcoTopController() presentViewController:scanner animated:YES completion:nil];
        };
        void (^refused)(void) = ^{
            UIAlertController *alert = [UIAlertController alertControllerWithTitle:title message:body preferredStyle:UIAlertControllerStyleAlert];
            [alert addAction:[UIAlertAction actionWithTitle:cancelText style:UIAlertActionStyleCancel handler:nil]];
            [alert addAction:[UIAlertAction actionWithTitle:settingsText style:UIAlertActionStyleDefault handler:^(UIAlertAction *action) {
                [UIApplication.sharedApplication openURL:[NSURL URLWithString:UIApplicationOpenSettingsURLString] options:@{} completionHandler:nil];
            }]];
            [SurcoTopController() presentViewController:alert animated:YES completion:nil];
        };
        switch ([AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeVideo]) {
            case AVAuthorizationStatusAuthorized: present(); break;
            case AVAuthorizationStatusNotDetermined:
                [AVCaptureDevice requestAccessForMediaType:AVMediaTypeVideo completionHandler:^(BOOL granted) {
                    dispatch_async(dispatch_get_main_queue(), granted ? present : refused);
                }];
                break;
            default: refused(); break;
        }
    });
}

// A picture from the phone's photos, through the system's own picker, which asks for no
// permission: one at a time, until Rust takes it.
@interface SurcoPhotoPicker : NSObject <PHPickerViewControllerDelegate>
@end

static SurcoPhotoPicker *SurcoPicking = nil;
// 0 while picking or idle, 1 once a picture is in, 2 once the picker was left without one.
static int SurcoPickedState = 0;
static NSData *SurcoPickedPhoto = nil;

static void SurcoPicked(NSData *photo) {
    @synchronized ([SurcoPhotoPicker class]) {
        SurcoPickedPhoto = photo;
        SurcoPickedState = photo ? 1 : 2;
    }
}

@implementation SurcoPhotoPicker
- (void)picker:(PHPickerViewController *)picker didFinishPicking:(NSArray<PHPickerResult *> *)results {
    [picker dismissViewControllerAnimated:YES completion:nil];
    SurcoPicking = nil;
    NSItemProvider *provider = results.firstObject.itemProvider;
    if (!provider || ![provider canLoadObjectOfClass:UIImage.class]) { SurcoPicked(nil); return; }
    [provider loadObjectOfClass:UIImage.class completionHandler:^(__kindof id<NSItemProviderReading> object, NSError *error) {
        UIImage *image = [object isKindOfClass:UIImage.class] ? object : nil;
        SurcoPicked(image ? UIImageJPEGRepresentation(image, 0.9) : nil);
    }];
}
@end

void surco_pick_photo(void) {
    @synchronized ([SurcoPhotoPicker class]) { SurcoPickedState = 0; SurcoPickedPhoto = nil; }
    dispatch_async(dispatch_get_main_queue(), ^{
        PHPickerConfiguration *configuration = [PHPickerConfiguration new];
        configuration.filter = PHPickerFilter.imagesFilter;
        configuration.selectionLimit = 1;
        PHPickerViewController *picker = [[PHPickerViewController alloc] initWithConfiguration:configuration];
        SurcoPicking = [SurcoPhotoPicker new];
        picker.delegate = SurcoPicking;
        [SurcoTopController() presentViewController:picker animated:YES completion:nil];
    });
}

// 0 while the picker is open, 1 with the picture's bytes (freed with free()), 2 when it was
// left without one.
int surco_take_picked_photo(uint8_t **bytes, size_t *length) {
    @synchronized ([SurcoPhotoPicker class]) {
        int state = SurcoPickedState;
        *bytes = NULL;
        *length = 0;
        if (state == 1 && SurcoPickedPhoto.length > 0) {
            *bytes = malloc(SurcoPickedPhoto.length);
            memcpy(*bytes, SurcoPickedPhoto.bytes, SurcoPickedPhoto.length);
            *length = SurcoPickedPhoto.length;
        }
        if (state != 0) { SurcoPickedState = 0; SurcoPickedPhoto = nil; }
        return state;
    }
}

// Foundation resolves the current sandbox container; its UUID may change after an update.
// https://developer.apple.com/documentation/foundation/url/applicationsupportdirectory
char *surco_application_support_dir(void) {
    @autoreleasepool {
        NSError *error = nil;
        NSURL *directory = [NSFileManager.defaultManager URLForDirectory:NSApplicationSupportDirectory
                                                              inDomain:NSUserDomainMask
                                                     appropriateForURL:nil
                                                                create:YES
                                                                 error:&error];
        if (!directory) {
            NSLog(@"Surco could not resolve its data directory: %@", error);
            return NULL;
        }
        return strdup(directory.fileSystemRepresentation);
    }
}

int surco_exclude_from_backup(const char *path) {
    @autoreleasepool {
        NSURL *directory = [NSURL fileURLWithFileSystemRepresentation:path isDirectory:YES relativeToURL:nil];
        NSError *error = nil;
        if (![directory setResourceValue:@YES forKey:NSURLIsExcludedFromBackupKey error:&error]) {
            NSLog(@"Surco could not exclude downloaded audio from backups: %@", error);
            return 0;
        }
        return 1;
    }
}

int main(void) {
    @autoreleasepool {
        SurcoWatchLinks();
        // Winit owns UIApplicationMain and must start on the main thread.
        return surco_start();
    }
}
