// Standing in for Finder: noticing the windows Finder opens, to show them
// here instead.

#import <AppKit/AppKit.h>
#import <UniformTypeIdentifiers/UniformTypeIdentifiers.h>

// While Finder is the app in front: the window numbers of its ordinary
// windows on screen (not the desktop), up to `max`, and how many there
// are. -1 when Finder isn't in front.
int32_t nf_front_finder_windows(uint32_t *ids, int32_t max) {
    @autoreleasepool {
        NSRunningApplication *front = NSWorkspace.sharedWorkspace.frontmostApplication;
        if (![front.bundleIdentifier isEqualToString:@"com.apple.finder"]) {
            return -1;
        }
        pid_t pid = front.processIdentifier;
        NSArray *list = CFBridgingRelease(CGWindowListCopyWindowInfo(kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements, kCGNullWindowID));
        int32_t n = 0;
        for (NSDictionary *w in list) {
            if ([w[(id)kCGWindowOwnerPID] intValue] != pid || [w[(id)kCGWindowLayer] intValue] != 0) {
                continue;
            }
            CGRect bounds;
            CGRectMakeWithDictionaryRepresentation((__bridge CFDictionaryRef)w[(id)kCGWindowBounds], &bounds);
            if (bounds.size.width < 200 || bounds.size.height < 150) {
                continue;
            }
            if (n < max) {
                ids[n] = [w[(id)kCGWindowNumber] unsignedIntValue];
            }
            n++;
        }
        return n;
    }
}

// Finder asked here rather than through osascript (a process less, about
// 50 ms), on a queue of its own: an AppleScript runs one at a time.
static NSString *ask_finder(NSString *source) {
    static dispatch_queue_t queue;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        queue = dispatch_queue_create("dev.newfinder.finder-scripts", DISPATCH_QUEUE_SERIAL);
    });
    __block NSString *result = nil;
    dispatch_sync(queue, ^{
        @autoreleasepool {
            NSAppleScript *script = [[NSAppleScript alloc] initWithSource:source];
            NSDictionary *error = nil;
            NSAppleEventDescriptor *out = [script executeAndReturnError:&error];
            if (out && !error) {
                result = out.stringValue ?: @"";
            }
        }
    });
    return result;
}

// Finder's windows, front to back: a line each, "id<tab>folder<tab>name"
// (no folder for one that isn't of one: Recents, the computer). NULL if
// Finder couldn't be asked (malloc'd).
char *nf_finder_windows(void) {
    NSString *out = ask_finder(@"set out to \"\"\n"
                                "tell application \"Finder\"\n"
                                "    with timeout of 3 seconds\n"
                                "        repeat with w in (get every Finder window)\n"
                                "            try\n"
                                "                set p to \"\"\n"
                                "                try\n"
                                "                    set p to POSIX path of ((target of w) as alias)\n"
                                "                end try\n"
                                "                set out to out & (id of w) & tab & p & tab & (name of w) & linefeed\n"
                                "            end try\n"
                                "        end repeat\n"
                                "    end timeout\n"
                                "end tell\n"
                                "return out");
    return out ? strdup(out.UTF8String) : NULL;
}

// Close Finder's windows with these ids.
void nf_close_finder_windows(const int64_t *ids, int32_t n) {
    NSMutableArray *list = [NSMutableArray array];
    for (int32_t i = 0; i < n; i++) {
        [list addObject:[NSString stringWithFormat:@"%lld", ids[i]]];
    }
    if (list.count == 0) {
        return;
    }
    ask_finder([NSString stringWithFormat:@"tell application \"Finder\"\n"
                                           "    with timeout of 3 seconds\n"
                                           "        repeat with i in {%@}\n"
                                           "            try\n"
                                           "                close (Finder window id (contents of i))\n"
                                           "            end try\n"
                                           "        end repeat\n"
                                           "    end timeout\n"
                                           "end tell",
                                           [list componentsJoinedByString:@", "]]);
}

// `came_forward` each time Finder becomes the app in front (from its Dock
// icon, say, or another app opening a folder in it).
void nf_on_finder_forward(void (*came_forward)(void)) {
    dispatch_async(dispatch_get_main_queue(), ^{
        [NSWorkspace.sharedWorkspace.notificationCenter addObserverForName:NSWorkspaceDidActivateApplicationNotification
                                                                    object:nil
                                                                     queue:nil
                                                                usingBlock:^(NSNotification *note) {
                                                                    NSRunningApplication *app = note.userInfo[NSWorkspaceApplicationKey];
                                                                    if ([app.bundleIdentifier isEqualToString:@"com.apple.finder"]) {
                                                                        came_forward();
                                                                    }
                                                                }];
    });
}

// Where read-only disk images are mounted (an app's DMG: its window, laid
// out for dragging the app to Applications, is Finder's to show), one a
// line (malloc'd).
char *nf_read_only_image_volumes(void) {
    @autoreleasepool {
        NSTask *task = [NSTask new];
        task.executableURL = [NSURL fileURLWithPath:@"/usr/bin/hdiutil"];
        task.arguments = @[@"info", @"-plist"];
        NSPipe *pipe = [NSPipe pipe];
        task.standardOutput = pipe;
        task.standardError = NSFileHandle.fileHandleWithNullDevice;
        if (![task launchAndReturnError:nil]) {
            return strdup("");
        }
        NSData *data = [pipe.fileHandleForReading readDataToEndOfFile];
        [task waitUntilExit];
        id info = [NSPropertyListSerialization propertyListWithData:data options:0 format:nil error:nil];
        NSMutableArray *out = [NSMutableArray array];
        if ([info isKindOfClass:NSDictionary.class]) {
            for (NSDictionary *image in info[@"images"]) {
                if ([image[@"writeable"] boolValue]) {
                    continue;
                }
                for (NSDictionary *entity in image[@"system-entities"]) {
                    NSString *mount = entity[@"mount-point"];
                    if ([mount isKindOfClass:NSString.class] && mount.length) {
                        [out addObject:mount];
                    }
                }
            }
        }
        return strdup([out componentsJoinedByString:@"\n"].UTF8String);
    }
}

// Whether disk images open in this app (else in DiskImageMounter, whose
// volumes' windows are Finder's to show).
bool nf_disk_images_ours(void) {
    @autoreleasepool {
        NSString *me = NSBundle.mainBundle.bundleIdentifier;
        NSURL *app = [NSWorkspace.sharedWorkspace URLForApplicationToOpenContentType:[UTType typeWithIdentifier:@"com.apple.disk-image-udif"]];
        return me && app && [[NSBundle bundleWithURL:app].bundleIdentifier isEqualToString:me];
    }
}

// Open .dmg files in this app, or in DiskImageMounter again. macOS asks
// you first, a dialog for each kind changed: so only .dmg's (and .img's,
// the same kind), the one that matters. NULL if done (or you kept it as it
// was), else why not (malloc'd). Waits for your answer: not on the main
// thread.
char *nf_set_disk_images_ours(bool ours) {
    @autoreleasepool {
        if (ours && !NSBundle.mainBundle.bundleIdentifier) return strdup("Only an installed copy can open disk images.");
        NSURL *app = ours ? NSBundle.mainBundle.bundleURL : [NSWorkspace.sharedWorkspace URLForApplicationWithBundleIdentifier:@"com.apple.DiskImageMounter"];
        if (!app) return strdup("DiskImageMounter isn't there.");
        __block NSString *failed = nil;
        dispatch_semaphore_t done = dispatch_semaphore_create(0);
        [NSWorkspace.sharedWorkspace setDefaultApplicationAtURL:app
                                              toOpenContentType:[UTType typeWithIdentifier:@"com.apple.disk-image-udif"]
                                              completionHandler:^(NSError *error) {
                                                  // (Kept as it was, in macOS's dialog: nothing went wrong.)
                                                  bool kept = [error.domain isEqualToString:NSCocoaErrorDomain] && error.code == NSUserCancelledError;
                                                  if (error && !kept) failed = error.localizedDescription;
                                                  dispatch_semaphore_signal(done);
                                              }];
        dispatch_semaphore_wait(done, dispatch_time(DISPATCH_TIME_NOW, 300 * NSEC_PER_SEC));
        return failed ? strdup(failed.UTF8String) : NULL;
    }
}
