#import <Foundation/Foundation.h>
#include <string.h>

extern int surco_start(void);

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
        // Winit owns UIApplicationMain and must start on the main thread.
        return surco_start();
    }
}
