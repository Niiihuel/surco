#import <Foundation/Foundation.h>
#import <UIKit/UIKit.h>
#import <objc/runtime.h>
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
