#import <Foundation/Foundation.h>
#import <CoreServices/CoreServices.h>

// Whether F2 may send Apple Events to the app `bundle`: 0 yes, -1743 no,
// -1744 macOS would ask, -600 it isn't open. With `ask`, macOS asks (once;
// it waits for the answer: not on the main thread).
int nf_automation(const char *bundle, bool ask) {
    @autoreleasepool {
        NSAppleEventDescriptor *target = [NSAppleEventDescriptor descriptorWithBundleIdentifier:[NSString stringWithUTF8String:bundle]];
        return (int)AEDeterminePermissionToAutomateTarget(target.aeDesc, typeWildCard, typeWildCard, ask);
    }
}
