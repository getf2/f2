// Quick Open's floating panel, as Spotlight's: no window buttons, its top
// edge staying put as it grows and shrinks with its results; which screen
// it opens on (the pointer's, the front window's); and the shortcut that
// brings it up from any app (a Carbon hot key: no permission needed).

#import <AppKit/AppKit.h>
#import <Carbon/Carbon.h>
#import <QuartzCore/QuartzCore.h>

// A rounded rectangle to clip the blur behind the panel to its card.
static NSImage *rounded(CGFloat radius) {
    CGFloat side = radius * 2 + 1;
    NSImage *mask = [NSImage imageWithSize:NSMakeSize(side, side)
                                   flipped:NO
                            drawingHandler:^BOOL(NSRect rect) {
                              [NSColor.blackColor setFill];
                              [[NSBezierPath bezierPathWithRoundedRect:rect xRadius:radius yRadius:radius] fill];
                              return YES;
                            }];
    mask.capInsets = NSEdgeInsetsMake(radius, radius, radius, radius);
    mask.resizingMode = NSImageResizingModeStretch;
    return mask;
}

// No title bar at all (GPUI's title-less window still has a hidden one,
// which its content is laid out below), so no buttons either; and behind
// it, rounded as the card is, the system's own popover material, as
// Spotlight's and Raycast's: blurred, and tinted so text stays legible on
// any background (GPUI's blur strips the tint: over white, gray).
// `nf_panel_setup`'s, `height` points tall (0: as it is), drawn now if
// `display`.
static void setup(void *view, double radius, double height, BOOL display) {
    @autoreleasepool {
        NSWindow *window = [(__bridge NSView *)view window];
        NSRect frame = window.frame;
        if (height > 0) {
            CGFloat top = NSMaxY(frame);
            frame.size.height = height;
            frame.origin.y = top - height;
        }
        window.styleMask = NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel | NSWindowStyleMaskFullSizeContentView;
        [window setFrame:frame display:display];
        // GPUI's view, all of it (it was made the size asked for, short of
        // the title bar's height; resizing keeps whatever gap there is).
        ((__bridge NSView *)view).frame = window.contentView.bounds;
        // (A new style mask hands the keyboard to the window itself.)
        [window makeFirstResponder:(__bridge NSView *)view];
        NSVisualEffectView *material = [[NSVisualEffectView alloc] initWithFrame:window.contentView.bounds];
        material.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
        material.material = NSVisualEffectMaterialPopover;
        material.blendingMode = NSVisualEffectBlendingModeBehindWindow;
        material.state = NSVisualEffectStateActive;
        material.maskImage = rounded(radius);
        [window.contentView addSubview:material positioned:NSWindowBelow relativeTo:(__bridge NSView *)view];
        [window invalidateShadow];
        window.movableByWindowBackground = NO;
        window.hasShadow = YES;
    }
}

void nf_panel_setup(void *view, double radius) {
    setup(view, radius, 0, YES);
}

// The same for a panel made hidden (Quick Open's), `height` points tall,
// not drawn yet: `nf_panel_present` draws it once, and shows it.
void nf_panel_prepare(void *view, double radius, double height) {
    setup(view, radius, height, NO);
}

// No title bar at all, as `nf_panel_setup`, but nothing behind GPUI's view
// either, and no window shadow: it draws its own card, and its shadow,
// within its clear window (the screenshots' corner, a pinned one).
void nf_panel_bare(void *view) {
    @autoreleasepool {
        NSWindow *window = [(__bridge NSView *)view window];
        NSRect frame = window.frame;
        window.styleMask = NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel | NSWindowStyleMaskFullSizeContentView;
        [window setFrame:frame display:YES];
        ((__bridge NSView *)view).frame = window.contentView.bounds;
        window.hasShadow = NO;
        window.movableByWindowBackground = NO;
    }
}

// The app that was in front when the panel took it, to give it back to.
static pid_t panel_took_from = 0;

// Make F2 the active app while the panel's up, the app in front noted: an
// input method (Korean's, Japanese', Chinese') composes only in the active
// app's key window; in a panel of an app that isn't, "안녕" came out as
// "ㅇㅏㄴㄴㅕㅇ", or as the keys' letters. (As Spotlight and Raycast take
// it.)
void nf_panel_take_focus(void *view) {
    @autoreleasepool {
        NSRunningApplication *front = NSWorkspace.sharedWorkspace.frontmostApplication;
        panel_took_from = (front && front.processIdentifier != getpid()) ? front.processIdentifier : 0;
        if (!panel_took_from) return;
        NSWindow *panel = [(__bridge NSView *)view window];
        // (Asked of the system, not AppKit: opened from the menu bar item,
        // AppKit takes F2 for active already, and does nothing.)
        [NSRunningApplication.currentApplication activateWithOptions:NSApplicationActivateIgnoringOtherApps];
        [panel makeKeyAndOrderFront:nil];
    }
}

// Give the app the panel took focus from its place back, if F2 still has
// it (clicked into another app, that one keeps it). Whether it did.
bool nf_panel_give_back(void) {
    @autoreleasepool {
        pid_t pid = panel_took_from;
        panel_took_from = 0;
        if (!pid || !NSApp.isActive) return false;
        return [[NSRunningApplication runningApplicationWithProcessIdentifier:pid] activateWithOptions:0];
    }
}

// Whether F2 is the app in front.
bool nf_in_front(void) {
    @autoreleasepool {
        return NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier == getpid();
    }
}

// Make the panel `height` points tall, its top where it was.
void nf_panel_fit(void *view, double height) {
    @autoreleasepool {
        NSWindow *window = [(__bridge NSView *)view window];
        NSRect frame = window.frame;
        CGFloat top = NSMaxY(frame);
        frame.size.height = height;
        frame.origin.y = top - height;
        [window setFrame:frame display:YES];
        [window invalidateShadow];
    }
}

// A panel made hidden (Quick Open's): GPUI's view drawn now (it draws in
// displayLayer:, its frame then part of this transaction), sent to the
// screen, and only then the window put in front, key: it comes up whole.
void nf_panel_present(void *view) {
    @autoreleasepool {
        NSView *v = (__bridge NSView *)view;
        [v.layer setNeedsDisplay];
        [v.layer displayIfNeeded];
        [CATransaction flush];
        [v.window makeKeyAndOrderFront:nil];
        [v.window invalidateShadow];
    }
}

void nf_panel_shadow(void *view) {
    @autoreleasepool {
        [[(__bridge NSView *)view window] invalidateShadow];
    }
}

static uint32_t screen_number(NSScreen *screen) {
    return [screen.deviceDescription[@"NSScreenNumber"] unsignedIntValue];
}

// The screen the pointer's on: its display id (0, none). (GPUI's displays'
// bounds all start at 0, 0: they can't tell.)
uint32_t nf_screen_with_pointer(void) {
    @autoreleasepool {
        NSPoint p = NSEvent.mouseLocation;
        for (NSScreen *screen in NSScreen.screens) {
            if (NSMouseInRect(p, screen.frame, NO)) {
                return screen_number(screen);
            }
        }
        return 0;
    }
}

// The screen most of the front app's front window is on (0: it has none
// up, as with the desktop). From the window server's list, which needs no
// permission for where windows are.
uint32_t nf_screen_with_front_window(void) {
    @autoreleasepool {
        pid_t front = NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier;
        CFArrayRef list = CGWindowListCopyWindowInfo(kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements, kCGNullWindowID);
        if (!list) {
            return 0;
        }
        uint32_t found = 0;
        // (Front to back: the first of its own is its front one.)
        for (NSDictionary *w in (__bridge NSArray *)list) {
            CGRect frame;
            if ([w[(id)kCGWindowOwnerPID] intValue] != front || [w[(id)kCGWindowLayer] intValue] != 0 || [w[(id)kCGWindowAlpha] doubleValue] == 0
                || !CGRectMakeWithDictionaryRepresentation((__bridge CFDictionaryRef)w[(id)kCGWindowBounds], &frame) || frame.size.width < 40
                || frame.size.height < 40) {
                continue;
            }
            CGFloat most = 0;
            for (NSScreen *screen in NSScreen.screens) {
                CGRect overlap = CGRectIntersection(frame, CGDisplayBounds(screen_number(screen)));
                CGFloat area = CGRectIsNull(overlap) ? 0 : overlap.size.width * overlap.size.height;
                if (area > most) {
                    most = area;
                    found = screen_number(screen);
                }
            }
            break;
        }
        CFRelease(list);
        return found;
    }
}

// A display's place and size, in points from the top left of the main one.
void nf_display_frame(uint32_t display, double *x, double *y, double *w, double *h) {
    CGRect b = CGDisplayBounds(display);
    *x = b.origin.x;
    *y = b.origin.y;
    *w = b.size.width;
    *h = b.size.height;
}

// The app the panel took the keyboard from (0: none).
int32_t nf_panel_took_from(void) {
    return panel_took_from;
}

// The shortcuts, by id (Quick Open's, the screenshots'): Carbon hot keys
// call `hot_key_pressed` with the id of the one pressed.
#define HOT_KEYS 512
static void (*hot_key_pressed)(uint32_t id);
static EventHotKeyRef hot_keys[HOT_KEYS];

static OSStatus on_hot_key(__unused EventHandlerCallRef next, EventRef event, __unused void *data) {
    EventHotKeyID id;
    if (GetEventParameter(event, kEventParamDirectObject, typeEventHotKeyID, NULL, sizeof id, NULL, &id) == noErr && hot_key_pressed) {
        hot_key_pressed(id.id);
    }
    return noErr;
}

void nf_hot_keys_listen(void (*pressed)(uint32_t id)) {
    hot_key_pressed = pressed;
    EventTypeSpec spec = {kEventClassKeyboard, kEventHotKeyPressed};
    InstallApplicationEventHandler(&on_hot_key, 1, &spec, NULL, NULL);
}

// Register shortcut `id` (a virtual key code and Carbon modifiers: cmdKey,
// shiftKey, optionKey, controlKey), replacing what it was; key code
// UINT32_MAX: none. False if it can't be had (another app has it).
bool nf_hot_key_set(uint32_t id, uint32_t key_code, uint32_t modifiers) {
    if (id >= HOT_KEYS) {
        return false;
    }
    if (hot_keys[id]) {
        UnregisterEventHotKey(hot_keys[id]);
        hot_keys[id] = NULL;
    }
    if (key_code == UINT32_MAX) {
        return true;
    }
    EventHotKeyID hot = {'NFhk', id};
    return RegisterEventHotKey(key_code, modifiers, hot, GetApplicationEventTarget(), 0, &hot_keys[id]) == noErr;
}
