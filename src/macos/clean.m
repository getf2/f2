#import <AppKit/AppKit.h>

// Cleaning mode: the keyboard and clicks off for a while (an event tap
// that lets none through, with Accessibility), so it can be wiped; a panel
// in the middle of the screen counts down, and its Done (the one place a
// click goes through) ends it early. It ends by itself whatever happens:
// the countdown, and a second timer a little after, in case.

static CFMachPortRef cleanTap;
static CFRunLoopSourceRef cleanSource;
static NSPanel *cleanPanel;
static NSTextField *cleanLeft;
static NSTimer *cleanTimer;
static int cleanSeconds;
static CGRect cleanDone;
static NSUInteger cleanRound;

static void nf_clean_stop(void);

@interface NFCleanDone : NSObject
@end

@implementation NFCleanDone
- (void)done:(id)sender {
    nf_clean_stop();
}
@end

static NFCleanDone *cleanTarget;

static CGEventRef nf_clean_swallow(__unused CGEventTapProxy proxy, CGEventType type, CGEventRef event, __unused void *info) {
    if (type == kCGEventTapDisabledByTimeout || type == kCGEventTapDisabledByUserInput) {
        if (cleanTap) CGEventTapEnable(cleanTap, true);
        return event;
    }
    // (A click on Done goes through.)
    if ((type == kCGEventLeftMouseDown || type == kCGEventLeftMouseUp) && CGRectContainsPoint(cleanDone, CGEventGetLocation(event))) {
        return event;
    }
    return NULL;
}

static void nf_clean_said(void) {
    cleanLeft.stringValue = [NSString stringWithFormat:@"Keys and clicks are off · %d s", cleanSeconds];
}

static void nf_clean_stop(void) {
    [cleanTimer invalidate];
    cleanTimer = nil;
    if (cleanTap) {
        CGEventTapEnable(cleanTap, false);
        if (cleanSource) CFRunLoopRemoveSource(CFRunLoopGetMain(), cleanSource, kCFRunLoopCommonModes);
        CFMachPortInvalidate(cleanTap);
        CFRelease(cleanTap);
        cleanTap = NULL;
    }
    if (cleanSource) {
        CFRelease(cleanSource);
        cleanSource = NULL;
    }
    [cleanPanel orderOut:nil];
    cleanPanel = nil;
    cleanLeft = nil;
}

// The panel: the keyboard, what's going on, the seconds left, and Done.
static NSPanel *nf_clean_panel(int seconds) {
    NSScreen *screen = NSScreen.mainScreen ?: NSScreen.screens.firstObject;
    NSRect frame = NSMakeRect(0, 0, 340, 220);
    frame.origin.x = NSMidX(screen.frame) - frame.size.width / 2;
    frame.origin.y = NSMidY(screen.frame) - frame.size.height / 2;
    NSPanel *panel = [[NSPanel alloc] initWithContentRect:frame styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel backing:NSBackingStoreBuffered defer:NO];
    panel.level = NSScreenSaverWindowLevel;
    panel.opaque = NO;
    panel.backgroundColor = NSColor.clearColor;
    panel.hasShadow = YES;
    panel.releasedWhenClosed = NO;
    panel.collectionBehavior = NSWindowCollectionBehaviorCanJoinAllSpaces | NSWindowCollectionBehaviorFullScreenAuxiliary | NSWindowCollectionBehaviorStationary;

    NSVisualEffectView *back = [[NSVisualEffectView alloc] initWithFrame:NSMakeRect(0, 0, frame.size.width, frame.size.height)];
    back.material = NSVisualEffectMaterialHUDWindow;
    back.blendingMode = NSVisualEffectBlendingModeBehindWindow;
    back.state = NSVisualEffectStateActive;
    back.wantsLayer = YES;
    back.layer.cornerRadius = 22;
    back.layer.masksToBounds = YES;
    panel.contentView = back;

    NSImageSymbolConfiguration *big = [NSImageSymbolConfiguration configurationWithPointSize:44 weight:NSFontWeightRegular];
    NSImageView *keyboard = [NSImageView imageViewWithImage:[[NSImage imageWithSystemSymbolName:@"keyboard" accessibilityDescription:nil] imageWithSymbolConfiguration:big]];
    keyboard.contentTintColor = NSColor.labelColor;
    keyboard.frame = NSMakeRect(0, 132, frame.size.width, 56);

    NSTextField *title = [NSTextField labelWithString:@"Cleaning Mode"];
    title.font = [NSFont systemFontOfSize:20 weight:NSFontWeightSemibold];
    title.alignment = NSTextAlignmentCenter;
    title.frame = NSMakeRect(0, 98, frame.size.width, 26);

    cleanLeft = [NSTextField labelWithString:@""];
    cleanLeft.font = [NSFont monospacedDigitSystemFontOfSize:13 weight:NSFontWeightRegular];
    cleanLeft.textColor = NSColor.secondaryLabelColor;
    cleanLeft.alignment = NSTextAlignmentCenter;
    cleanLeft.frame = NSMakeRect(0, 74, frame.size.width, 18);
    cleanSeconds = seconds;
    nf_clean_said();

    cleanTarget = cleanTarget ?: [NFCleanDone new];
    NSButton *done = [NSButton buttonWithTitle:@"Done" target:cleanTarget action:@selector(done:)];
    done.bezelStyle = NSBezelStylePush;
    done.controlSize = NSControlSizeLarge;
    done.frame = NSMakeRect((frame.size.width - 96) / 2, 22, 96, 32);

    for (NSView *view in @[ keyboard, title, cleanLeft, done ]) [back addSubview:view];
    [panel orderFrontRegardless];

    // Done's place on screen, as events have it (from the top of the
    // first screen, not the bottom).
    NSRect onScreen = [panel convertRectToScreen:[done convertRect:done.bounds toView:nil]];
    CGFloat top = NSMaxY(NSScreen.screens.firstObject.frame);
    cleanDone = CGRectMake(onScreen.origin.x, top - NSMaxY(onScreen), onScreen.size.width, onScreen.size.height);
    return panel;
}

// Cleaning mode for `seconds`. False if it can't be (no Accessibility, or
// it's on already). On the main thread.
bool nf_clean_keyboard(int seconds) {
    if (cleanTap || seconds <= 0) return false;
    CGEventMask mask = CGEventMaskBit(kCGEventKeyDown) | CGEventMaskBit(kCGEventKeyUp) | CGEventMaskBit(kCGEventFlagsChanged)
        | CGEventMaskBit(kCGEventLeftMouseDown) | CGEventMaskBit(kCGEventLeftMouseUp) | CGEventMaskBit(kCGEventLeftMouseDragged)
        | CGEventMaskBit(kCGEventRightMouseDown) | CGEventMaskBit(kCGEventRightMouseUp) | CGEventMaskBit(kCGEventRightMouseDragged)
        | CGEventMaskBit(kCGEventOtherMouseDown) | CGEventMaskBit(kCGEventOtherMouseUp) | CGEventMaskBit(kCGEventOtherMouseDragged)
        | CGEventMaskBit(kCGEventScrollWheel)
        // (The media and brightness keys: NSEventTypeSystemDefined. And the
        // trackpad's gestures: rotate, begin and end, gesture, magnify,
        // swipe, smart magnify, pressure.)
        | CGEventMaskBit(14) | CGEventMaskBit(18) | CGEventMaskBit(19) | CGEventMaskBit(20) | CGEventMaskBit(29) | CGEventMaskBit(30) | CGEventMaskBit(31) | CGEventMaskBit(32) | CGEventMaskBit(34);
    CFMachPortRef tap = CGEventTapCreate(kCGSessionEventTap, kCGHeadInsertEventTap, kCGEventTapOptionDefault, mask, nf_clean_swallow, NULL);
    if (!tap) return false;
    cleanTap = tap;
    cleanSource = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, tap, 0);
    CFRunLoopAddSource(CFRunLoopGetMain(), cleanSource, kCFRunLoopCommonModes);
    CGEventTapEnable(tap, true);
    cleanPanel = nf_clean_panel(seconds);
    NSUInteger round = ++cleanRound;
    cleanTimer = [NSTimer scheduledTimerWithTimeInterval:1 repeats:YES block:^(__unused NSTimer *timer) {
        cleanSeconds -= 1;
        if (cleanSeconds <= 0) {
            nf_clean_stop();
        } else {
            nf_clean_said();
        }
    }];
    [NSRunLoop.mainRunLoop addTimer:cleanTimer forMode:NSRunLoopCommonModes];
    // (In case: off a little after, whatever the countdown did.)
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(seconds + 3) * NSEC_PER_SEC), dispatch_get_main_queue(), ^{
        if (cleanRound == round) nf_clean_stop();
    });
    return true;
}
