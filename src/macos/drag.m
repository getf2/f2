// Dragging windows with keys held, as Linux's window managers do: with
// them held, a press anywhere in a window and a drag moves it; a press in
// its outer fifth (a side, or a corner) sizes it from there. The window's
// raised, as a click would. An event tap on a thread of its own (presses,
// drags and letting go of the left button: nothing else, and nothing kept
// while the keys aren't held), what's under the pointer from the window
// server (no waiting on the app there), and Accessibility moving it on a
// queue, the latest place only. Points from the main screen's top left,
// y down.

#import <AppKit/AppKit.h>
#import <ApplicationServices/ApplicationServices.h>
#include <os/lock.h>
#include <stdatomic.h>

extern AXError _AXUIElementGetWindow(AXUIElementRef element, CGWindowID *out);

enum { LEFT = 1, RIGHT = 2, TOP = 4, BOTTOM = 8 };
static const CGEventFlags KEYS = kCGEventFlagMaskShift | kCGEventFlagMaskControl | kCGEventFlagMaskAlternate | kCGEventFlagMaskCommand;

static CFMachPortRef dragTap;
static _Atomic CGEventFlags dragKeys;
static dispatch_queue_t dragQueue;

// The tap's thread's: the drag going on.
static bool dragging;
static CGPoint dragFrom;
static CGRect dragStart;
static int dragEdges;

// Where it's wanted next (the tap's thread sets it, the queue takes it).
static os_unfair_lock dragLock = OS_UNFAIR_LOCK_INIT;
static CGRect dragWanted;
static bool dragPending;

// The queue's: the window being dragged.
static AXUIElementRef dragWindow;
static AXUIElementRef dragApp;
static bool dragEnhanced;
static NSInteger dragOwn;
static int dragSides;

// The window under `at` (one in front of the others there, a normal
// one: not the Dock, the menu bar or a panel above it). Overlays high
// above everything (a screen dimmer's, say: the pointer goes through), and
// ones over a whole screen, are looked past.
static bool window_at(CGPoint at, pid_t *pid, CGWindowID *number, CGRect *bounds) {
    CFArrayRef list = CGWindowListCopyWindowInfo(kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements, kCGNullWindowID);
    if (!list) return false;
    CGDirectDisplayID displays[16];
    uint32_t screens = 0;
    CGGetActiveDisplayList(16, displays, &screens);
    bool found = false;
    for (NSDictionary *info in (__bridge NSArray *)list) {
        CGRect r;
        if (!CGRectMakeWithDictionaryRepresentation((__bridge CFDictionaryRef)info[(id)kCGWindowBounds], &r) || !CGRectContainsPoint(r, at)) continue;
        if ([info[(id)kCGWindowAlpha] doubleValue] < 0.05) continue;
        NSInteger layer = [info[(id)kCGWindowLayer] integerValue];
        if (layer >= kCGOverlayWindowLevel) continue;
        // (A whole screen's, above windows: the Dock's, there while it
        // shows. Clicks go through it.)
        bool whole = false;
        for (uint32_t i = 0; i < screens; i++) whole = whole || CGRectEqualToRect(CGDisplayBounds(displays[i]), r);
        if (whole && layer != 0) continue;
        if (layer == 0) {
            *pid = [info[(id)kCGWindowOwnerPID] intValue];
            *number = [info[(id)kCGWindowNumber] unsignedIntValue];
            *bounds = r;
            found = true;
        }
        break;
    }
    CFRelease(list);
    return found;
}

// Which sides a press at `at` sizes from: none (moving) in the middle.
static int edges_at(CGPoint at, CGRect r) {
    double bandX = MIN(r.size.width / 5, 160), bandY = MIN(r.size.height / 5, 160);
    int edges = 0;
    if (at.x < r.origin.x + bandX) edges |= LEFT;
    else if (at.x > CGRectGetMaxX(r) - bandX) edges |= RIGHT;
    if (at.y < r.origin.y + bandY) edges |= TOP;
    else if (at.y > CGRectGetMaxY(r) - bandY) edges |= BOTTOM;
    return edges;
}

// The frame for the pointer at `at`, the drag having started at
// `dragFrom`.
static CGRect dragged_to(CGPoint at) {
    double dx = at.x - dragFrom.x, dy = at.y - dragFrom.y;
    CGRect r = dragStart;
    if (!dragEdges) return CGRectOffset(r, dx, dy);
    const double least = 100;
    if (dragEdges & LEFT) {
        double w = MAX(least, r.size.width - dx);
        r.origin.x = CGRectGetMaxX(dragStart) - w;
        r.size.width = w;
    } else if (dragEdges & RIGHT) {
        r.size.width = MAX(least, r.size.width + dx);
    }
    if (dragEdges & TOP) {
        double h = MAX(least, r.size.height - dy);
        r.origin.y = CGRectGetMaxY(dragStart) - h;
        r.size.height = h;
    } else if (dragEdges & BOTTOM) {
        r.size.height = MAX(least, r.size.height + dy);
    }
    return r;
}

// The app's element for the window `number`: from its windows, else (a
// sheet's, say) the window of what's under the pointer. Retained.
static AXUIElementRef ax_window(AXUIElementRef app, pid_t pid, CGWindowID number, CGPoint at) {
    AXUIElementRef found = NULL;
    CFTypeRef windows = NULL;
    if (AXUIElementCopyAttributeValue(app, kAXWindowsAttribute, &windows) == kAXErrorSuccess && windows) {
        for (id item in (__bridge NSArray *)windows) {
            CGWindowID n = 0;
            if (_AXUIElementGetWindow((__bridge AXUIElementRef)item, &n) == kAXErrorSuccess && n == number) {
                found = (AXUIElementRef)CFRetain((__bridge CFTypeRef)item);
                break;
            }
        }
        CFRelease(windows);
    }
    if (!found) {
        AXUIElementRef wide = AXUIElementCreateSystemWide();
        AXUIElementSetMessagingTimeout(wide, 0.5);
        AXUIElementRef element = NULL;
        pid_t owner = 0;
        if (AXUIElementCopyElementAtPosition(wide, at.x, at.y, &element) == kAXErrorSuccess && element) {
            CFTypeRef window = NULL;
            if (AXUIElementGetPid(element, &owner) == kAXErrorSuccess && owner == pid
                && AXUIElementCopyAttributeValue(element, kAXWindowAttribute, &window) == kAXErrorSuccess && window) {
                found = (AXUIElementRef)window;
            }
            CFRelease(element);
        }
        CFRelease(wide);
    }
    return found;
}

static void drag_end(void) {
    if (dragApp && dragEnhanced) AXUIElementSetAttributeValue(dragApp, CFSTR("AXEnhancedUserInterface"), kCFBooleanTrue);
    if (dragWindow) CFRelease(dragWindow);
    if (dragApp) CFRelease(dragApp);
    dragWindow = NULL;
    dragApp = NULL;
    dragEnhanced = false;
    dragOwn = 0;
}

// On the queue: the window to drag found (one of F2's own: AppKit's, on
// the main thread, as Accessibility here would stop F2), and raised.
static void drag_begin(pid_t pid, CGWindowID number, CGPoint at, int edges) {
    drag_end();
    dragSides = edges;
    if (pid == getpid()) {
        dragOwn = number;
        dispatch_async(dispatch_get_main_queue(), ^{
          NSWindow *w = [NSApp windowWithWindowNumber:number];
          if (!w) return;
          if (@available(macOS 14, *)) {
              [NSApp activate];
          } else {
              [NSApp activateIgnoringOtherApps:YES];
          }
          [w makeKeyAndOrderFront:nil];
        });
        return;
    }
    AXUIElementRef app = AXUIElementCreateApplication(pid);
    AXUIElementSetMessagingTimeout(app, 0.5);
    AXUIElementRef window = ax_window(app, pid, number, at);
    CFTypeRef full = NULL;
    Boolean movable = false;
    if (window) AXUIElementCopyAttributeValue(window, CFSTR("AXFullScreen"), &full);
    if (window) AXUIElementIsAttributeSettable(window, kAXPositionAttribute, &movable);
    bool fullScreen = full && CFGetTypeID(full) == CFBooleanGetTypeID() && CFBooleanGetValue(full);
    if (full) CFRelease(full);
    if (!window || fullScreen || !movable) {
        if (window) CFRelease(window);
        CFRelease(app);
        return;
    }
    AXUIElementPerformAction(window, kAXRaiseAction);
    AXUIElementSetAttributeValue(app, kAXFrontmostAttribute, kCFBooleanTrue);
    // (Its "enhanced user interface", on with VoiceOver and in some apps,
    // animates each move: off meanwhile, as Rectangle does.)
    CFTypeRef enhanced = NULL;
    AXUIElementCopyAttributeValue(app, CFSTR("AXEnhancedUserInterface"), &enhanced);
    dragEnhanced = enhanced && CFGetTypeID(enhanced) == CFBooleanGetTypeID() && CFBooleanGetValue(enhanced);
    if (enhanced) CFRelease(enhanced);
    if (dragEnhanced) AXUIElementSetAttributeValue(app, CFSTR("AXEnhancedUserInterface"), kCFBooleanFalse);
    dragWindow = window;
    dragApp = app;
}

// On the queue: to where it's wanted now. Sized from its left or top, the
// other side stays put, even if the app won't let it be that small.
static void drag_apply(void) {
    os_unfair_lock_lock(&dragLock);
    CGRect to = dragWanted;
    dragPending = false;
    os_unfair_lock_unlock(&dragLock);
    int sides = dragSides;
    if (dragOwn) {
        NSInteger number = dragOwn;
        dispatch_sync(dispatch_get_main_queue(), ^{
          NSWindow *w = [NSApp windowWithWindowNumber:number];
          if (!w) return;
          CGRect r = to;
          if (sides) {
              NSSize least = w.minSize;
              NSSize content = [w frameRectForContentRect:NSMakeRect(0, 0, w.contentMinSize.width, w.contentMinSize.height)].size;
              least = NSMakeSize(MAX(least.width, content.width), MAX(least.height, content.height));
              if (r.size.width < least.width) {
                  if (sides & LEFT) r.origin.x -= least.width - r.size.width;
                  r.size.width = least.width;
              }
              if (r.size.height < least.height) {
                  if (sides & TOP) r.origin.y -= least.height - r.size.height;
                  r.size.height = least.height;
              }
          } else {
              r.size = w.frame.size;
          }
          CGFloat top = NSScreen.screens.firstObject.frame.size.height;
          [w setFrame:NSMakeRect(r.origin.x, top - r.origin.y - r.size.height, r.size.width, r.size.height) display:YES animate:NO];
        });
        return;
    }
    if (!dragWindow) return;
    CGPoint p = to.origin;
    if (sides) {
        CGSize s = to.size;
        AXValueRef size = AXValueCreate(kAXValueCGSizeType, &s);
        AXUIElementSetAttributeValue(dragWindow, kAXSizeAttribute, size);
        CFRelease(size);
        if (!(sides & (LEFT | TOP))) return;
        CFTypeRef now = NULL;
        CGSize got = s;
        if (AXUIElementCopyAttributeValue(dragWindow, kAXSizeAttribute, &now) == kAXErrorSuccess && now) {
            AXValueGetValue(now, kAXValueCGSizeType, &got);
            CFRelease(now);
        }
        if (sides & LEFT) p.x = CGRectGetMaxX(to) - got.width;
        if (sides & TOP) p.y = CGRectGetMaxY(to) - got.height;
    }
    AXValueRef position = AXValueCreate(kAXValueCGPointType, &p);
    AXUIElementSetAttributeValue(dragWindow, kAXPositionAttribute, position);
    CFRelease(position);
}

static void drag_want(CGRect to) {
    os_unfair_lock_lock(&dragLock);
    dragWanted = to;
    bool queued = dragPending;
    dragPending = true;
    os_unfair_lock_unlock(&dragLock);
    if (!queued) dispatch_async(dragQueue, ^{ drag_apply(); });
}

static CGEventRef drag_event(__unused CGEventTapProxy proxy, CGEventType type, CGEventRef event, __unused void *info) {
    if (type == kCGEventTapDisabledByTimeout || type == kCGEventTapDisabledByUserInput) {
        if (atomic_load(&dragKeys)) CGEventTapEnable(dragTap, true);
        return event;
    }
    CGPoint at = CGEventGetLocation(event);
    if (type == kCGEventLeftMouseDown) {
        CGEventFlags keys = atomic_load(&dragKeys);
        if (dragging) {
            dragging = false;
            dispatch_async(dragQueue, ^{ drag_end(); });
        }
        if (!keys || (CGEventGetFlags(event) & KEYS) != keys) return event;
        pid_t pid = 0;
        CGWindowID number = 0;
        CGRect bounds;
        if (!window_at(at, &pid, &number, &bounds)) return event;
        int edges = edges_at(at, bounds);
        dragging = true;
        dragFrom = at;
        dragStart = bounds;
        dragEdges = edges;
        dispatch_async(dragQueue, ^{ drag_begin(pid, number, at, edges); });
        return NULL;
    }
    // (The rest of a drag of ours: the app never saw it start.)
    if (!dragging) return event;
    drag_want(dragged_to(at));
    if (type == kCGEventLeftMouseUp) {
        dragging = false;
        dispatch_async(dragQueue, ^{ drag_end(); });
    }
    return NULL;
}

// Dragging windows with `keys` held (CGEventFlags: ⌃⌘'s, say), or not (0).
// False if it can't be: no Accessibility (it's tried again when asked).
bool nf_window_drag(uint64_t keys) {
    atomic_store(&dragKeys, (CGEventFlags)keys);
    if (!keys) {
        if (dragTap) CGEventTapEnable(dragTap, false);
        return true;
    }
    if (dragTap) {
        CGEventTapEnable(dragTap, true);
        return true;
    }
    if (!AXIsProcessTrusted()) return false;
    CGEventMask mask = CGEventMaskBit(kCGEventLeftMouseDown) | CGEventMaskBit(kCGEventLeftMouseDragged) | CGEventMaskBit(kCGEventLeftMouseUp);
    CFMachPortRef tap = CGEventTapCreate(kCGSessionEventTap, kCGHeadInsertEventTap, kCGEventTapOptionDefault, mask, drag_event, NULL);
    if (!tap) return false;
    dragTap = tap;
    dragQueue = dispatch_queue_create("f2.window-drag", dispatch_queue_attr_make_with_qos_class(DISPATCH_QUEUE_SERIAL, QOS_CLASS_USER_INTERACTIVE, 0));
    NSThread *thread = [[NSThread alloc] initWithBlock:^{
      CFRunLoopSourceRef source = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, tap, 0);
      CFRunLoopAddSource(CFRunLoopGetCurrent(), source, kCFRunLoopCommonModes);
      CFRelease(source);
      CGEventTapEnable(tap, true);
      CFRunLoopRun();
    }];
    thread.name = @"F2 window drag";
    thread.qualityOfService = NSQualityOfServiceUserInteractive;
    [thread start];
    return true;
}
