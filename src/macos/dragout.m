// Files dragged out of F2's windows into other apps, as out of Finder's.
// GPUI hands a drag of files over to AppKit once the pointer has left the
// window it started in; here, for the windows given to `nf_drag_out`:
// - a press on an item of a window in the background leaves the window
//   there (it comes forward on letting go, unless what was pressed was
//   dragged off), so what it's dragged to stays in sight; a press anywhere
//   else brings it forward as ever (macOS makes F2 the active app at that
//   press all the same, and brings it forward once the drag's over: the
//   app that was in front is given the front back then);
// - the drag is handed over as soon as the pointer is over another window
//   (in front of this one), not only once it's out of this one's frame;
// - other apps are offered what Finder offers: to move, copy or link them,
//   theirs to choose (Finder moves them within a volume), and the Dock's
//   Trash to delete them, which is left to the app they came from: they're
//   handed to `trash` with the window's token;
// - and the drag shows the items' own icons (one's name beside).

#import <AppKit/AppKit.h>
#import <objc/runtime.h>

typedef void (*nf_drag_out_cb)(uint64_t token, const char *paths);

static nf_drag_out_cb trash;
static NSMapTable<NSWindow *, NSNumber *> *tokens;
// The press going on: its window left where it was, on an item
// (`nf_item_pressed`, while GPUI handles it), so a drag of files; and its
// window to come forward on letting go (not dragged out).
static bool delayed, on_item, armed, forward;
// The app in front at that press (in the background), given back the
// front once what was pressed has been dragged out.
static NSRunningApplication *previous;
// Whether the drag out going on is over one of F2's windows.
static bool over_ours;
// What the drag out going on holds.
static NSArray<NSURL *> *dragged;

static BOOL (*original_delay)(id, SEL, NSEvent *);
static void (*original_down)(id, SEL, NSEvent *);
static void (*original_up)(id, SEL, NSEvent *);
static void (*original_dragged)(id, SEL, NSEvent *);
static NSDraggingSession *(*original_begin)(id, SEL, NSArray *, NSEvent *, id);
static NSDragOperation (*original_mask)(id, SEL, NSDraggingSession *, NSDraggingContext);
static void (*original_moved_to)(id, SEL, NSDraggingSession *, NSPoint);
static void (*original_ended)(id, SEL, NSDraggingSession *, NSPoint, NSDragOperation);

static bool ours(NSWindow *window) {
    return window && [tokens objectForKey:window];
}

static BOOL delay(id self, SEL cmd, NSEvent *event) {
    NSWindow *window = ((NSView *)self).window;
    delayed = ours(window) && event.type == NSEventTypeLeftMouseDown && !(event.modifierFlags & NSEventModifierFlagControl)
              && !(NSApp.isActive && window.isKeyWindow);
    NSRunningApplication *front = NSWorkspace.sharedWorkspace.frontmostApplication;
    previous = delayed && ![front isEqual:NSRunningApplication.currentApplication] ? front : nil;
    return delayed || original_delay(self, cmd, event);
}

static void bring_forward(NSWindow *window) {
    if (@available(macOS 14, *)) {
        [NSApp activate];
    } else {
        [NSApp activateIgnoringOtherApps:YES];
    }
    [window makeKeyAndOrderFront:nil];
}

static void down(id self, SEL cmd, NSEvent *event) {
    bool waited = delayed;
    delayed = on_item = armed = forward = false;
    original_down(self, cmd, event);
    NSWindow *window = ((NSView *)self).window;
    armed = on_item && ours(window);
    if (!waited || !on_item) previous = nil;
    if (!waited) return;
    // On an item, it comes forward on letting go, but not after a drag out
    // (here, not by AppKit, which would then too); else now, as it would
    // have come.
    if (on_item) {
        [NSApp preventWindowOrdering];
        forward = true;
    } else {
        bring_forward(window);
    }
}

static void up(id self, SEL cmd, NSEvent *event) {
    bool bring = forward;
    armed = forward = false;
    previous = nil;
    original_up(self, cmd, event);
    if (bring) bring_forward(((NSView *)self).window);
}

static void moved(id self, SEL cmd, NSEvent *event) {
    NSWindow *window = ((NSView *)self).window;
    if (armed && window) {
        NSInteger under = [NSWindow windowNumberAtPoint:NSEvent.mouseLocation belowWindowWithWindowNumber:0];
        // Over another window: GPUI hands a drag over once the pointer's
        // out of the window, so it's told it is.
        if (under && under != window.windowNumber) {
            NSEvent *out = [NSEvent mouseEventWithType:event.type
                                              location:NSMakePoint(-10000, -10000)
                                         modifierFlags:event.modifierFlags
                                             timestamp:event.timestamp
                                          windowNumber:event.windowNumber
                                               context:nil
                                           eventNumber:event.eventNumber
                                            clickCount:event.clickCount
                                              pressure:event.pressure];
            if (out) event = out;
        }
    }
    original_dragged(self, cmd, event);
}

// A name as Finder shows it being dragged: on the selection's color.
static NSImage *label(NSString *name) {
    NSMutableParagraphStyle *style = [NSMutableParagraphStyle new];
    style.lineBreakMode = NSLineBreakByTruncatingMiddle;
    NSDictionary *attrs = @{
        NSFontAttributeName : [NSFont systemFontOfSize:12 weight:NSFontWeightMedium],
        NSForegroundColorAttributeName : NSColor.whiteColor,
        NSParagraphStyleAttributeName : style,
    };
    NSSize text = [name sizeWithAttributes:attrs];
    NSSize size = NSMakeSize(MIN(ceil(text.width) + 12, 260), ceil(text.height) + 4);
    return [NSImage imageWithSize:size
                          flipped:NO
                   drawingHandler:^BOOL(NSRect r) {
                     [NSColor.selectedContentBackgroundColor setFill];
                     [[NSBezierPath bezierPathWithRoundedRect:r xRadius:5 yRadius:5] fill];
                     [name drawInRect:NSInsetRect(r, 6, 2) withAttributes:attrs];
                     return YES;
                   }];
}

static NSDraggingSession *begin(id self, SEL cmd, NSArray<NSDraggingItem *> *items, NSEvent *event, id source) {
    armed = forward = false;
    if (!ours(((NSView *)self).window)) return original_begin(self, cmd, items, event, source);
    NSMutableArray<NSURL *> *urls = [NSMutableArray array];
    for (NSDraggingItem *item in items) {
        if (![item.item isKindOfClass:NSURL.class]) continue;
        NSURL *url = item.item;
        NSUInteger i = urls.count;
        [urls addObject:url];
        // (A few in a pile, the rest under them as they were: each icon
        // asked of the system as it's drawn.)
        if (i >= 8) continue;
        NSRect frame = item.draggingFrame;
        frame.origin.x += 4 * i;
        frame.origin.y -= 4 * i;
        // (One's name; several are counted by AppKit.)
        NSImage *name = items.count == 1 ? label([NSFileManager.defaultManager displayNameAtPath:url.path]) : nil;
        if (name) frame.size.width += 4 + name.size.width;
        item.draggingFrame = frame;
        NSString *path = url.path;
        item.imageComponentsProvider = ^NSArray<NSDraggingImageComponent *> * {
          NSDraggingImageComponent *icon = [NSDraggingImageComponent draggingImageComponentWithKey:NSDraggingImageComponentIconKey];
          icon.contents = [NSWorkspace.sharedWorkspace iconForFile:path];
          icon.frame = NSMakeRect(0, 0, 32, 32);
          if (!name) return @[ icon ];
          NSDraggingImageComponent *text = [NSDraggingImageComponent draggingImageComponentWithKey:NSDraggingImageComponentLabelKey];
          text.contents = name;
          text.frame = NSMakeRect(36, (32 - name.size.height) / 2, name.size.width, name.size.height);
          return @[ icon, text ];
        };
    }
    dragged = urls;
    over_ours = false;
    return original_begin(self, cmd, items, event, source);
}

static NSDragOperation mask(id self, SEL cmd, NSDraggingSession *session, NSDraggingContext context) {
    if (context == NSDraggingContextOutsideApplication && ours(self)) {
        return NSDragOperationCopy | NSDragOperationLink | NSDragOperationGeneric | NSDragOperationMove | NSDragOperationDelete;
    }
    return original_mask(self, cmd, session, context);
}

static void moved_to(id self, SEL cmd, NSDraggingSession *session, NSPoint at) {
    if (original_moved_to) original_moved_to(self, cmd, session, at);
    over_ours = [NSApp windowWithWindowNumber:[NSWindow windowNumberAtPoint:at belowWindowWithWindowNumber:0]] != nil;
}

static void ended(id self, SEL cmd, NSDraggingSession *session, NSPoint at, NSDragOperation operation) {
    original_ended(self, cmd, session, at, operation);
    // (Not dropped back on one of F2's windows, which takes the front. By
    // now it's in front at the point: where the drag was last seen.)
    NSRunningApplication *front = previous;
    previous = nil;
    if (front && !over_ours) {
        if (@available(macOS 14, *)) {
            [NSApp yieldActivationToApplication:front];
            [front activateFromApplication:NSRunningApplication.currentApplication options:0];
        } else {
            [front activateWithOptions:0];
        }
    }
    NSArray<NSURL *> *urls = dragged;
    dragged = nil;
    NSNumber *token = [tokens objectForKey:self];
    if (operation != NSDragOperationDelete || !token || !trash || !urls.count) return;
    NSMutableArray<NSString *> *paths = [NSMutableArray array];
    for (NSURL *url in urls) [paths addObject:url.path];
    trash(token.unsignedLongLongValue, [paths componentsJoinedByString:@"\n"].UTF8String);
}

// `cls`'s `sel` made `imp`; what it was (its own, or what it inherited;
// none, and it's given `types`).
static IMP swap(Class cls, SEL sel, IMP imp, const char *types) {
    Method m = class_getInstanceMethod(cls, sel);
    IMP before = m ? method_getImplementation(m) : NULL;
    class_replaceMethod(cls, sel, imp, m ? method_getTypeEncoding(m) : types);
    return before;
}

// Pressed on an item: said while GPUI handles the press.
void nf_item_pressed(void) {
    on_item = true;
}

// Drags of files out of the window holding `view` go to other apps as out
// of Finder's; those dropped on the Trash go to `cb`.
void nf_drag_out(void *view, uint64_t token, nf_drag_out_cb cb) {
    NSView *v = (__bridge NSView *)view;
    NSWindow *window = v.window;
    if (!window) return;
    trash = cb;
    if (!tokens) tokens = [NSMapTable weakToStrongObjectsMapTable];
    [tokens setObject:@(token) forKey:window];

    Class viewClass = v.class, windowClass = window.class;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
      original_delay = (void *)swap(viewClass, @selector(shouldDelayWindowOrderingForEvent:), (IMP)delay, NULL);
      original_down = (void *)swap(viewClass, @selector(mouseDown:), (IMP)down, NULL);
      original_up = (void *)swap(viewClass, @selector(mouseUp:), (IMP)up, NULL);
      original_dragged = (void *)swap(viewClass, @selector(mouseDragged:), (IMP)moved, NULL);
      original_begin = (void *)swap(viewClass, @selector(beginDraggingSessionWithItems:event:source:), (IMP)begin, NULL);
      original_mask = (void *)swap(windowClass, @selector(draggingSession:sourceOperationMaskForDraggingContext:), (IMP)mask, NULL);
      original_moved_to = (void *)swap(windowClass, @selector(draggingSession:movedToPoint:), (IMP)moved_to, "v@:@{CGPoint=dd}");
      original_ended = (void *)swap(windowClass, @selector(draggingSession:endedAtPoint:operation:), (IMP)ended, NULL);
    });
}
