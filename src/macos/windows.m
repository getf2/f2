// Window management, as Rectangle's: the window in front (its app's
// focused one, not a floating panel), where it is, moving and sizing it
// (Accessibility), full screen, and the screens' room (less the menu bar
// and the Dock). Points from the main screen's top left, y down.

#import <AppKit/AppKit.h>
#include <dlfcn.h>
#import <ApplicationServices/ApplicationServices.h>

// The front app's window to move, retained (NULL if none): its focused
// one, or its main one if that's a panel (Quick Open's, say).
static AXUIElementRef front_window(pid_t *pid_out) {
    NSRunningApplication *front = NSWorkspace.sharedWorkspace.frontmostApplication;
    if (!front) {
        return NULL;
    }
    pid_t pid = front.processIdentifier;
    if (pid_out) {
        *pid_out = pid;
    }
    AXUIElementRef app = AXUIElementCreateApplication(pid);
    AXUIElementRef window = NULL;
    for (NSString *name in @[(__bridge NSString *)kAXFocusedWindowAttribute, (__bridge NSString *)kAXMainWindowAttribute]) {
        CFStringRef attribute = (__bridge CFStringRef)name;
        CFTypeRef value = NULL;
        if (AXUIElementCopyAttributeValue(app, attribute, &value) != kAXErrorSuccess || !value) {
            continue;
        }
        CFTypeRef subrole = NULL;
        AXUIElementCopyAttributeValue(value, kAXSubroleAttribute, &subrole);
        bool panel = subrole && (CFEqual(subrole, kAXFloatingWindowSubrole) || CFEqual(subrole, kAXSystemFloatingWindowSubrole));
        if (subrole) {
            CFRelease(subrole);
        }
        if (!panel) {
            window = (AXUIElementRef)value;
            break;
        }
        CFRelease(value);
    }
    CFRelease(app);
    return window;
}

// This app's window in front (not a panel: Quick Open's), for moving its
// own windows without Accessibility (see `nf_window_set`).
static NSWindow *own_window(void) {
    NSWindow *window = NSApp.keyWindow;
    if (!window || [window isKindOfClass:NSPanel.class]) {
        window = NSApp.mainWindow;
    }
    if (!window || [window isKindOfClass:NSPanel.class]) {
        window = nil;
        for (NSWindow *w in NSApp.orderedWindows) {
            if (w.isVisible && ![w isKindOfClass:NSPanel.class] && (w.styleMask & NSWindowStyleMaskTitled)) {
                window = w;
                break;
            }
        }
    }
    return window;
}

static bool frame_of(AXUIElementRef window, double *x, double *y, double *w, double *h) {
    CFTypeRef position = NULL, size = NULL;
    CGPoint p;
    CGSize s;
    bool ok = AXUIElementCopyAttributeValue(window, kAXPositionAttribute, &position) == kAXErrorSuccess
        && AXUIElementCopyAttributeValue(window, kAXSizeAttribute, &size) == kAXErrorSuccess
        && AXValueGetValue(position, kAXValueCGPointType, &p) && AXValueGetValue(size, kAXValueCGSizeType, &s);
    if (position) {
        CFRelease(position);
    }
    if (size) {
        CFRelease(size);
    }
    if (ok) {
        *x = p.x, *y = p.y, *w = s.width, *h = s.height;
    }
    return ok;
}

// The window in front: where it is, its app, and a key for it (the same
// while it's open). False if there's none (or no Accessibility).
bool nf_window_get(double *x, double *y, double *w, double *h, int32_t *pid, uint64_t *key) {
    @autoreleasepool {
        pid_t owner = 0;
        AXUIElementRef window = front_window(&owner);
        if (!window) {
            return false;
        }
        bool ok = frame_of(window, x, y, w, h);
        *pid = owner;
        *key = (uint64_t)CFHash(window) ^ ((uint64_t)owner << 32);
        CFRelease(window);
        return ok;
    }
}

static void set_frame(AXUIElementRef window, CGRect frame) {
    CGPoint p = frame.origin;
    CGSize s = frame.size;
    AXValueRef position = AXValueCreate(kAXValueCGPointType, &p);
    AXValueRef size = AXValueCreate(kAXValueCGSizeType, &s);
    // (Size, place, size: a window too big for where it's going is sized
    // first, and one stopped at a screen's edge is sized again.)
    AXUIElementSetAttributeValue(window, kAXSizeAttribute, size);
    AXUIElementSetAttributeValue(window, kAXPositionAttribute, position);
    AXUIElementSetAttributeValue(window, kAXSizeAttribute, size);
    CFRelease(position);
    CFRelease(size);
}

// Move and size the window in front. Its app's "enhanced user interface"
// (on with VoiceOver, and in some apps) animates and fights it: off
// meanwhile, as Rectangle does.
bool nf_window_set(double x, double y, double w, double h) {
    @autoreleasepool {
        // One of this app's own: AppKit moves it, once the shortcut's been
        // handled. (Through Accessibility it'd be resized there and then, in
        // the middle of GPUI's handling it, which misses the resize and
        // draws at the old size.)
        if (NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier == getpid()) {
            dispatch_async(dispatch_get_main_queue(), ^{
                NSWindow *window = own_window();
                CGFloat top = NSScreen.screens.firstObject.frame.size.height;
                [window setFrame:NSMakeRect(x, top - y - h, w, h) display:YES animate:NO];
            });
            return true;
        }
        pid_t pid = 0;
        AXUIElementRef window = front_window(&pid);
        if (!window) {
            return false;
        }
        AXUIElementRef app = AXUIElementCreateApplication(pid);
        CFTypeRef enhanced = NULL;
        AXUIElementCopyAttributeValue(app, CFSTR("AXEnhancedUserInterface"), &enhanced);
        bool was = enhanced && CFBooleanGetValue(enhanced);
        if (enhanced) {
            CFRelease(enhanced);
        }
        if (was) {
            AXUIElementSetAttributeValue(app, CFSTR("AXEnhancedUserInterface"), kCFBooleanFalse);
        }
        set_frame(window, CGRectMake(x, y, w, h));
        if (was) {
            AXUIElementSetAttributeValue(app, CFSTR("AXEnhancedUserInterface"), kCFBooleanTrue);
        }
        CFRelease(app);
        CFRelease(window);
        return true;
    }
}

// The window in front, into full screen or out of it.
bool nf_window_full_screen(void) {
    @autoreleasepool {
        if (NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier == getpid()) {
            dispatch_async(dispatch_get_main_queue(), ^{
                [own_window() toggleFullScreen:nil];
            });
            return true;
        }
        AXUIElementRef window = front_window(NULL);
        if (!window) {
            return false;
        }
        CFTypeRef full = NULL;
        AXUIElementCopyAttributeValue(window, CFSTR("AXFullScreen"), &full);
        bool now = full && CFBooleanGetValue(full);
        if (full) {
            CFRelease(full);
        }
        AXError error = AXUIElementSetAttributeValue(window, CFSTR("AXFullScreen"), now ? kCFBooleanFalse : kCFBooleanTrue);
        CFRelease(window);
        return error == kAXErrorSuccess;
    }
}

// The screens: for each, the room windows have (its visible frame) and
// all of it, 8 numbers (x, y, w, h twice) into `out`. How many.
int32_t nf_screens(double *out, int32_t max) {
    @autoreleasepool {
        NSArray<NSScreen *> *screens = NSScreen.screens;
        CGFloat top = screens.firstObject.frame.size.height;
        int32_t n = 0;
        for (NSScreen *screen in screens) {
            if (n >= max) {
                break;
            }
            NSRect v = screen.visibleFrame, f = screen.frame;
            double *o = out + n * 8;
            o[0] = v.origin.x, o[1] = top - NSMaxY(v), o[2] = v.size.width, o[3] = v.size.height;
            o[4] = f.origin.x, o[5] = top - NSMaxY(f), o[6] = f.size.width, o[7] = f.size.height;
            n++;
        }
        return n;
    }
}

// The text selected in an app (its focused element's, through
// Accessibility), if it says: NULL if none, or it doesn't (some apps don't:
// then copying is the way). Malloc'd.
char *nf_selected_text(int32_t pid) {
    @autoreleasepool {
        if (pid <= 0 || !AXIsProcessTrusted()) return NULL;
        AXUIElementRef app = AXUIElementCreateApplication(pid);
        AXUIElementSetMessagingTimeout(app, 0.3);
        CFTypeRef focused = NULL;
        char *out = NULL;
        if (AXUIElementCopyAttributeValue(app, kAXFocusedUIElementAttribute, &focused) == kAXErrorSuccess && focused) {
            AXUIElementSetMessagingTimeout(focused, 0.3);
            CFTypeRef text = NULL;
            if (AXUIElementCopyAttributeValue(focused, kAXSelectedTextAttribute, &text) == kAXErrorSuccess && text) {
                if (CFGetTypeID(text) == CFStringGetTypeID() && CFStringGetLength(text) > 0) {
                    out = strdup([(__bridge NSString *)text UTF8String]);
                }
                CFRelease(text);
            }
            CFRelease(focused);
        }
        CFRelease(app);
        return out;
    }
}

// The app in front (its pid), as it is now.
int32_t nf_front_pid(void) {
    return NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier;
}

// A window's number, as the window server knows it (private, but what
// every window switcher uses to match Accessibility's windows to it).
extern AXError _AXUIElementGetWindow(AXUIElementRef element, CGWindowID *out);

// Each app's windows (Accessibility's: not panels or sheets), front to
// back as they're stacked; then those minimized, or their app hidden, or
// on another desktop. JSON: [{"pid", "id", "title", "app", "path",
// "minimized", "hidden"}] (malloc'd). NULL without Accessibility. Not on
// the main thread: it asks each app, and waits a little for each.
char *nf_windows(void) {
    @autoreleasepool {
        if (!AXIsProcessTrusted()) return NULL;
        // (How far back each on-screen window is.)
        NSMutableDictionary<NSNumber *, NSNumber *> *depth = [NSMutableDictionary dictionary];
        CFArrayRef stacked = CGWindowListCopyWindowInfo(kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements, kCGNullWindowID);
        if (stacked) {
            NSArray *list = (__bridge NSArray *)stacked;
            for (NSUInteger i = 0; i < list.count; i++) {
                NSDictionary *w = list[i];
                if ([w[(__bridge NSString *)kCGWindowLayer] intValue] == 0) depth[w[(__bridge NSString *)kCGWindowNumber]] = @(i);
            }
            CFRelease(stacked);
        }
        NSMutableArray<NSDictionary *> *found = [NSMutableArray array];
        // F2's own, from AppKit on the main thread: asked through
        // Accessibility, F2 would answer itself here, off the main thread
        // (and turn its own accessibility on).
        pid_t me = getpid();
        void (^own)(void) = ^{
          for (NSWindow *w in NSApp.orderedWindows) {
              if ([w isKindOfClass:NSPanel.class] || !(w.styleMask & NSWindowStyleMaskTitled) || !(w.isVisible || w.isMiniaturized)) continue;
              [found addObject:@{
                  @"pid" : @(me),
                  @"id" : @(w.windowNumber),
                  @"title" : w.title ?: @"",
                  @"app" : NSRunningApplication.currentApplication.localizedName ?: @"",
                  @"path" : NSBundle.mainBundle.bundlePath ?: @"",
                  @"minimized" : w.isMiniaturized ? @YES : @NO,
                  @"hidden" : NSApp.isHidden ? @YES : @NO,
                  @"depth" : depth[@(w.windowNumber)] ?: @(NSIntegerMax),
              }];
          }
        };
        // (Only in the app: a tool or a test has no NSApp, nor a main
        // thread that'd ever run this.)
        if (NSApp && NSThread.isMainThread) {
            own();
        } else if (NSApp) {
            dispatch_sync(dispatch_get_main_queue(), own);
        }
        for (NSRunningApplication *app in NSWorkspace.sharedWorkspace.runningApplications) {
            if (app.activationPolicy != NSApplicationActivationPolicyRegular || app.terminated || app.processIdentifier == me) continue;
            AXUIElementRef element = AXUIElementCreateApplication(app.processIdentifier);
            AXUIElementSetMessagingTimeout(element, 0.25);
            CFTypeRef windows = NULL;
            if (AXUIElementCopyAttributeValue(element, kAXWindowsAttribute, &windows) == kAXErrorSuccess && windows) {
                for (id item in (__bridge NSArray *)windows) {
                    AXUIElementRef window = (__bridge AXUIElementRef)item;
                    CFTypeRef subrole = NULL, title = NULL, minimized = NULL;
                    AXUIElementCopyAttributeValue(window, kAXSubroleAttribute, &subrole);
                    bool standard = subrole && CFEqual(subrole, kAXStandardWindowSubrole);
                    if (subrole) CFRelease(subrole);
                    CGWindowID number = 0;
                    if (!standard || _AXUIElementGetWindow(window, &number) != kAXErrorSuccess || !number) continue;
                    AXUIElementCopyAttributeValue(window, kAXTitleAttribute, &title);
                    AXUIElementCopyAttributeValue(window, kAXMinimizedAttribute, &minimized);
                    NSString *named = title && CFGetTypeID(title) == CFStringGetTypeID() ? (__bridge NSString *)title : @"";
                    NSNumber *at = depth[@(number)];
                    [found addObject:@{
                        @"pid" : @(app.processIdentifier),
                        @"id" : @(number),
                        @"title" : [named copy],
                        @"app" : app.localizedName ?: @"",
                        @"path" : app.bundleURL.path ?: @"",
                        // (As true or false: `&&`'s an int, boxed as 0 or 1.)
                        @"minimized" : (minimized && CFEqual(minimized, kCFBooleanTrue)) ? @YES : @NO,
                        @"hidden" : app.hidden ? @YES : @NO,
                        @"depth" : at ?: @(NSIntegerMax),
                    }];
                    if (title) CFRelease(title);
                    if (minimized) CFRelease(minimized);
                }
                CFRelease(windows);
            }
            CFRelease(element);
        }
        // Windows on other desktops: Accessibility only says what's on this
        // one; the window server knows them all (their titles with Screen
        // Recording allowed). A real window has a title and a size (not an
        // app's 1×1 helpers, its bars, its blank squares).
        NSMutableSet<NSNumber *> *listed = [NSMutableSet set];
        for (NSDictionary *w in found) [listed addObject:w[@"id"]];
        CFArrayRef everywhere = CGWindowListCopyWindowInfo(kCGWindowListOptionAll | kCGWindowListExcludeDesktopElements, kCGNullWindowID);
        if (everywhere) {
            for (NSDictionary *w in (__bridge NSArray *)everywhere) {
                NSNumber *number = w[(__bridge NSString *)kCGWindowNumber];
                pid_t pid = [w[(__bridge NSString *)kCGWindowOwnerPID] intValue];
                NSString *title = w[(__bridge NSString *)kCGWindowName];
                CGRect bounds = CGRectZero;
                CGRectMakeWithDictionaryRepresentation((__bridge CFDictionaryRef)w[(__bridge NSString *)kCGWindowBounds], &bounds);
                if ([w[(__bridge NSString *)kCGWindowLayer] intValue] != 0 || pid == me || [listed containsObject:number] || depth[number] || !title.length
                    || bounds.size.width < 200 || bounds.size.height < 120) {
                    continue;
                }
                NSRunningApplication *app = [NSRunningApplication runningApplicationWithProcessIdentifier:pid];
                if (!app || app.activationPolicy != NSApplicationActivationPolicyRegular) continue;
                [listed addObject:number];
                [found addObject:@{
                    @"pid" : @(pid),
                    @"id" : number,
                    @"title" : title,
                    @"app" : app.localizedName ?: @"",
                    @"path" : app.bundleURL.path ?: @"",
                    @"minimized" : @NO,
                    @"hidden" : app.hidden ? @YES : @NO,
                    @"elsewhere" : @YES,
                    @"depth" : @(NSIntegerMax),
                }];
            }
            CFRelease(everywhere);
        }
        // (Stable: those not on screen keep their apps' order.)
        NSArray *sorted = [found sortedArrayWithOptions:NSSortStable
                                        usingComparator:^NSComparisonResult(NSDictionary *a, NSDictionary *b) {
                                          return [a[@"depth"] compare:b[@"depth"]];
                                        }];
        NSMutableArray *out = [NSMutableArray array];
        for (NSDictionary *w in sorted) {
            NSMutableDictionary *m = [w mutableCopy];
            [m removeObjectForKey:@"depth"];
            [out addObject:m];
        }
        NSData *json = [NSJSONSerialization dataWithJSONObject:out options:0 error:NULL];
        return json ? strndup(json.bytes, json.length) : NULL;
    }
}

// The Accessibility element of window `number` of app `pid` wherever it
// is, other desktops too, which the app's window list leaves out: made from
// a remote token (the app's pid, "coco", an element id), the ids tried in
// turn, as AltTab finds them. NULL if none is that window (+1).
static AXUIElementRef element_anywhere(pid_t pid, uint32_t number) {
    static AXUIElementRef (*from_token)(CFDataRef);
    static dispatch_once_t once;
    dispatch_once(&once, ^{
      from_token = dlsym(RTLD_DEFAULT, "_AXUIElementCreateWithRemoteToken");
    });
    if (!from_token) return NULL;
    uint8_t token[20] = {0};
    memcpy(token, &pid, sizeof pid);
    int32_t coco = 0x636f636f;
    memcpy(token + 8, &coco, sizeof coco);
    for (uint64_t id = 0; id < 1000; id++) {
        memcpy(token + 12, &id, sizeof id);
        CFDataRef data = CFDataCreate(NULL, token, sizeof token);
        AXUIElementRef element = from_token(data);
        CFRelease(data);
        if (!element) continue;
        AXUIElementSetMessagingTimeout(element, 0.05);
        CGWindowID n = 0;
        if (_AXUIElementGetWindow(element, &n) == kAXErrorSuccess && n == number) return element;
        CFRelease(element);
    }
    return NULL;
}

// The window server's own "this window, in front" (private, as window
// managers use it: yabai's way): it goes to the window's desktop. Looked up,
// not linked: false if this macOS hasn't it.
static bool window_server_focus(pid_t pid, uint32_t number) {
    static OSStatus (*front)(ProcessSerialNumber *, uint32_t, uint32_t);
    static OSStatus (*post)(ProcessSerialNumber *, uint8_t *);
    static dispatch_once_t once;
    dispatch_once(&once, ^{
      void *skylight = dlopen("/System/Library/PrivateFrameworks/SkyLight.framework/SkyLight", RTLD_LAZY);
      if (skylight) {
          front = dlsym(skylight, "_SLPSSetFrontProcessWithOptions");
          post = dlsym(skylight, "SLPSPostEventRecordTo");
      }
    });
    if (!front || !post) {
        NSLog(@"switch window: no window server focus here");
        return false;
    }
    ProcessSerialNumber psn = {0, 0};
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
    OSStatus found = GetProcessForPID(pid, &psn);
#pragma clang diagnostic pop
    if (found != noErr) {
        NSLog(@"switch window: no process for %d (%d)", pid, (int)found);
        return false;
    }
    // (0x200: as the user asked for it.)
    OSStatus fronted = front(&psn, number, 0x200);
    if (fronted != noErr) {
        NSLog(@"switch window: window server focus failed (%d)", (int)fronted);
        return false;
    }
    // (Then the window made key, as a click on it would: two event records.)
    uint8_t record[0xf8] = {0};
    record[0x04] = 0xf8;
    record[0x3a] = 0x10;
    memcpy(&record[0x3c], &number, sizeof number);
    memset(&record[0x20], 0xff, 0x10);
    record[0x08] = 0x01;
    post(&psn, record);
    record[0x08] = 0x02;
    post(&psn, record);
    return true;
}

// Bring the window `number` of the app `pid` forward: shown again if it
// was minimized or its app hidden, raised, and its app put in front (on
// another desktop: that desktop gone to). False if it's gone. Not on the
// main thread.
bool nf_window_focus(int32_t pid, uint32_t number) {
    @autoreleasepool {
        // One of F2's own: AppKit's, on the main thread (through
        // Accessibility it'd be done here, off it, and AppKit stops F2).
        if (pid == getpid()) {
            __block bool done = false;
            void (^focus)(void) = ^{
              NSWindow *w = [NSApp windowWithWindowNumber:number];
              if (!w) return;
              if (NSApp.isHidden) [NSApp unhide:nil];
              if (w.isMiniaturized) [w deminiaturize:nil];
              if (@available(macOS 14, *)) {
                  [NSApp activate];
              } else {
                  [NSApp activateIgnoringOtherApps:YES];
              }
              [w makeKeyAndOrderFront:nil];
              done = true;
            };
            if (NSThread.isMainThread) {
                focus();
            } else {
                dispatch_sync(dispatch_get_main_queue(), focus);
            }
            return done;
        }
        NSRunningApplication *app = [NSRunningApplication runningApplicationWithProcessIdentifier:pid];
        if (!app || !AXIsProcessTrusted()) return false;
        AXUIElementRef element = AXUIElementCreateApplication(pid);
        AXUIElementSetMessagingTimeout(element, 0.5);
        CFTypeRef windows = NULL;
        bool done = false;
        if (AXUIElementCopyAttributeValue(element, kAXWindowsAttribute, &windows) == kAXErrorSuccess && windows) {
            for (id item in (__bridge NSArray *)windows) {
                AXUIElementRef window = (__bridge AXUIElementRef)item;
                CGWindowID n = 0;
                if (_AXUIElementGetWindow(window, &n) != kAXErrorSuccess || n != number) continue;
                if (app.hidden) [app unhide];
                AXUIElementSetAttributeValue(window, kAXMinimizedAttribute, kCFBooleanFalse);
                AXUIElementSetAttributeValue(window, kAXMainAttribute, kCFBooleanTrue);
                AXUIElementPerformAction(window, kAXRaiseAction);
                AXUIElementSetAttributeValue(element, kAXFrontmostAttribute, kCFBooleanTrue);
                [app activateWithOptions:0];
                done = true;
                break;
            }
            CFRelease(windows);
        }
        // (Not on this desktop: the app's window list leaves it out. Its
        // element found anyway; the window server brings it, and its desktop;
        // and it's raised, as AltTab does.)
        if (!done) {
            if (app.hidden) [app unhide];
            AXUIElementRef there = element_anywhere(pid, number);
            bool fronted = window_server_focus(pid, number);
            if (there) {
                AXUIElementSetAttributeValue(there, kAXMinimizedAttribute, kCFBooleanFalse);
                AXUIElementSetAttributeValue(there, kAXMainAttribute, kCFBooleanTrue);
                AXError raised = AXUIElementPerformAction(there, kAXRaiseAction);
                NSLog(@"switch window: %u on another desktop: found, window server %@, raised (%d)", number, fronted ? @"ok" : @"failed", (int)raised);
                CFRelease(there);
            } else {
                NSLog(@"switch window: %u on another desktop: not found, window server %@", number, fronted ? @"ok" : @"failed");
            }
            if (!fronted) [app activateWithOptions:0];
            done = true;
        }
        CFRelease(element);
        return done;
    }
}

// How tall a window's title bar is (with no toolbar, as a disk image's
// window has): what's under it is that much less than the window.
double nf_titlebar_height(void) {
    NSRect content = NSMakeRect(0, 0, 100, 100);
    NSWindowStyleMask style = NSWindowStyleMaskTitled | NSWindowStyleMaskClosable | NSWindowStyleMaskMiniaturizable;
    return [NSWindow frameRectForContentRect:content styleMask:style].size.height - content.size.height;
}

// Let the process `pid` (F2's own dialog, run as a process of its own)
// come forward: since macOS 14 the app in front has to let it.
void nf_yield_to(int32_t pid) {
    dispatch_async(dispatch_get_main_queue(), ^{
        if (@available(macOS 14, *)) {
            NSRunningApplication *app = [NSRunningApplication runningApplicationWithProcessIdentifier:pid];
            if (app) [NSApp yieldActivationToApplication:app];
        }
    });
}
