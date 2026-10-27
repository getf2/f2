// Updates (Sparkle, loaded from the app's Frameworks when it's set up:
// a build without it, as cargo's, has none), the About panel, and what
// the app's Info.plist and macOS say about themselves (for the log).

#import <AppKit/AppKit.h>
#import <objc/message.h>

static id updater;

// Start Sparkle, if this build has it and Info.plist says where updates
// are and what they're signed with. Whether it's on.
bool nf_updates_start(void) {
    @autoreleasepool {
        NSDictionary *info = NSBundle.mainBundle.infoDictionary;
        if ([info[@"SUFeedURL"] length] == 0 || [info[@"SUPublicEDKey"] length] == 0) {
            return false;
        }
        NSString *path = [NSBundle.mainBundle.privateFrameworksPath stringByAppendingPathComponent:@"Sparkle.framework"];
        NSBundle *sparkle = [NSBundle bundleWithPath:path];
        NSError *error = nil;
        if (!sparkle || ![sparkle loadAndReturnError:&error]) {
            NSLog(@"Sparkle didn't load: %@", error);
            return false;
        }
        Class controller = NSClassFromString(@"SPUStandardUpdaterController");
        if (!controller) {
            return false;
        }
        // (Its own window and checks, once a day, as Info.plist says.)
        SEL make = NSSelectorFromString(@"initWithStartingUpdater:updaterDelegate:userDriverDelegate:");
        updater = ((id(*)(id, SEL, BOOL, id, id))objc_msgSend)([controller alloc], make, YES, nil, nil);
        return updater != nil;
    }
}

// Look for an update now, with Sparkle's window. False if updates are off.
bool nf_updates_check(void) {
    if (!updater) {
        return false;
    }
    ((void (*)(id, SEL, id))objc_msgSend)(updater, NSSelectorFromString(@"checkForUpdates:"), nil);
    return true;
}

// macOS's About panel: the icon, name, version and copyright from
// Info.plist, and `credits` under them.
void nf_about(const char *credits) {
    @autoreleasepool {
        NSMutableParagraphStyle *centered = [NSMutableParagraphStyle new];
        centered.alignment = NSTextAlignmentCenter;
        NSAttributedString *text = [[NSAttributedString alloc]
            initWithString:@(credits)
                attributes:@{NSFontAttributeName: [NSFont systemFontOfSize:11], NSForegroundColorAttributeName: NSColor.secondaryLabelColor, NSParagraphStyleAttributeName: centered}];
        [NSApp activateIgnoringOtherApps:YES];
        [NSApp orderFrontStandardAboutPanelWithOptions:@{NSAboutPanelOptionCredits: text}];
    }
}

// One of Info.plist's values, as text (malloc'd), or NULL.
char *nf_bundle_value(const char *key) {
    @autoreleasepool {
        id value = NSBundle.mainBundle.infoDictionary[@(key)];
        return [value isKindOfClass:NSString.class] ? strdup([value UTF8String]) : NULL;
    }
}

// The app's own resource `name` (Contents/Resources), as a path
// (malloc'd), or NULL.
char *nf_resource(const char *name) {
    @autoreleasepool {
        NSString *path = [NSBundle.mainBundle.resourcePath stringByAppendingPathComponent:@(name)];
        return [NSFileManager.defaultManager fileExistsAtPath:path] ? strdup(path.UTF8String) : NULL;
    }
}

// "Version 26.5 (Build 25F71)" (malloc'd).
char *nf_os_version(void) {
    return strdup(NSProcessInfo.processInfo.operatingSystemVersionString.UTF8String);
}
