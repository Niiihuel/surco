#import <Foundation/Foundation.h>

extern int surco_start(void);

int main(void) {
    @autoreleasepool {
        // Winit owns UIApplicationMain and must start on the main thread.
        return surco_start();
    }
}
