// Snippets: their keywords heard as they're typed in any app (macOS's key
// monitors, which take Accessibility, and hear nothing in password
// fields), the keyword taken back and the snippet pasted in its place,
// with the clipboard put back after. And the document in front (an
// editor's), for the repo you're in.

#import <AppKit/AppKit.h>
#import <ApplicationServices/ApplicationServices.h>

static NSArray<NSString *> *keywords;
static NSMutableString *typed;
static id key_monitor, own_key_monitor, mouse_monitor, own_mouse_monitor;
static void (*matched)(int32_t index);

static void forget(void) {
    [typed setString:@""];
}

static void heard(NSEvent *event) {
    // (What this app sends itself, taking a keyword back: not typing.)
    if (CGEventGetIntegerValueField(event.CGEvent, kCGEventSourceUnixProcessID) == getpid()) {
        return;
    }
    if (event.modifierFlags & (NSEventModifierFlagCommand | NSEventModifierFlagControl)) {
        return forget();
    }
    switch (event.keyCode) {
        case 51:  // delete
            if (typed.length) {
                [typed deleteCharactersInRange:NSMakeRange(typed.length - 1, 1)];
            }
            return;
        case 36: case 48: case 53: case 76: case 115: case 116: case 117: case 119: case 121: case 123: case 124: case 125: case 126:
            return forget();
    }
    NSString *chars = event.characters;
    if (!chars.length) {
        return;
    }
    [typed appendString:chars];
    if (typed.length > 64) {
        [typed deleteCharactersInRange:NSMakeRange(0, typed.length - 64)];
    }
    for (NSUInteger i = 0; i < keywords.count; i++) {
        NSString *keyword = keywords[i];
        if (!keyword.length || ![typed hasSuffix:keyword]) {
            continue;
        }
        // A keyword that starts with a letter or digit, only as a word of
        // its own ("mail" not inside "email").
        unichar first = [keyword characterAtIndex:0];
        NSUInteger at = typed.length - keyword.length;
        if ([NSCharacterSet.alphanumericCharacterSet characterIsMember:first] && at > 0
            && [NSCharacterSet.alphanumericCharacterSet characterIsMember:[typed characterAtIndex:at - 1]]) {
            continue;
        }
        forget();
        if (matched) {
            matched((int32_t)i);
        }
        return;
    }
}

// Listen for these keywords (one a line; none: stop listening), calling
// `on_match` with the index of the one typed, on the main thread.
void nf_snippets_watch(const char *list, void (*on_match)(int32_t index)) {
    // (Copied now: `list` is gone by the time the block runs.)
    NSString *text = list ? @(list) : @"";
    dispatch_async(dispatch_get_main_queue(), ^{
        NSMutableArray<NSString *> *words = [NSMutableArray array];
        for (NSString *line in [text componentsSeparatedByString:@"\n"]) {
            [words addObject:line];
        }
        bool any = false;
        for (NSString *w in words) {
            any = any || w.length > 0;
        }
        keywords = words;
        matched = on_match;
        if (!typed) {
            typed = [NSMutableString string];
        }
        if (any && !key_monitor) {
            key_monitor = [NSEvent addGlobalMonitorForEventsMatchingMask:NSEventMaskKeyDown handler:^(NSEvent *event) { heard(event); }];
            own_key_monitor = [NSEvent addLocalMonitorForEventsMatchingMask:NSEventMaskKeyDown handler:^NSEvent *(NSEvent *event) {
                heard(event);
                return event;
            }];
            NSEventMask clicks = NSEventMaskLeftMouseDown | NSEventMaskRightMouseDown;
            mouse_monitor = [NSEvent addGlobalMonitorForEventsMatchingMask:clicks handler:^(__unused NSEvent *event) { forget(); }];
            own_mouse_monitor = [NSEvent addLocalMonitorForEventsMatchingMask:clicks handler:^NSEvent *(NSEvent *event) {
                forget();
                return event;
            }];
        } else if (!any && key_monitor) {
            for (id monitor in @[key_monitor, own_key_monitor, mouse_monitor, own_mouse_monitor]) {
                [NSEvent removeMonitor:monitor];
            }
            key_monitor = own_key_monitor = mouse_monitor = own_mouse_monitor = nil;
        }
    });
}

static void post_key(CGEventSourceRef source, CGKeyCode key) {
    for (int down = 1; down >= 0; down--) {
        CGEventRef e = CGEventCreateKeyboardEvent(source, key, down);
        CGEventSetFlags(e, 0);
        CGEventPost(kCGSessionEventTap, e);
        CFRelease(e);
    }
    usleep(4000);
}

// ⌫, `n` times, to the app in front (a keyword taken back).
void nf_type_backspaces(int32_t n) {
    CGEventSourceRef source = CGEventSourceCreate(kCGEventSourceStateCombinedSessionState);
    for (int32_t i = 0; i < n; i++) {
        post_key(source, 51);
    }
    if (source) {
        CFRelease(source);
    }
}

// ←, `n` times (to where a snippet's "{cursor}" was).
void nf_post_left(int32_t n) {
    CGEventSourceRef source = CGEventSourceCreate(kCGEventSourceStateCombinedSessionState);
    for (int32_t i = 0; i < n; i++) {
        post_key(source, 123);
    }
    if (source) {
        CFRelease(source);
    }
}

static NSArray<NSDictionary<NSString *, NSData *> *> *saved;

// Keep what's on the clipboard (every item, every type) to put back.
void nf_clipboard_save(void) {
    @autoreleasepool {
        NSMutableArray *items = [NSMutableArray array];
        for (NSPasteboardItem *item in NSPasteboard.generalPasteboard.pasteboardItems) {
            NSMutableDictionary *kept = [NSMutableDictionary dictionary];
            for (NSString *type in item.types) {
                NSData *data = [item dataForType:type];
                if (data) {
                    kept[type] = data;
                }
            }
            [items addObject:kept];
        }
        saved = items;
    }
}

void nf_clipboard_restore(void) {
    @autoreleasepool {
        if (!saved) {
            return;
        }
        NSPasteboard *pb = NSPasteboard.generalPasteboard;
        [pb clearContents];
        NSMutableArray *items = [NSMutableArray array];
        for (NSDictionary<NSString *, NSData *> *kept in saved) {
            NSPasteboardItem *item = [NSPasteboardItem new];
            for (NSString *type in kept) {
                [item setData:kept[type] forType:type];
            }
            [items addObject:item];
        }
        if (items.count) {
            [pb writeObjects:items];
        }
        saved = nil;
    }
}

// The clipboard's text (malloc'd), or NULL.
char *nf_clipboard_text(void) {
    @autoreleasepool {
        NSString *text = [NSPasteboard.generalPasteboard stringForType:NSPasteboardTypeString];
        return text ? strdup(text.UTF8String) : NULL;
    }
}

// The document in front: the focused window's (an editor's file), as a
// path (malloc'd), or NULL.
char *nf_front_document(void) {
    @autoreleasepool {
        NSRunningApplication *front = NSWorkspace.sharedWorkspace.frontmostApplication;
        if (!front || front.processIdentifier == getpid()) {
            return NULL;
        }
        AXUIElementRef app = AXUIElementCreateApplication(front.processIdentifier);
        CFTypeRef window = NULL, document = NULL;
        AXUIElementCopyAttributeValue(app, kAXFocusedWindowAttribute, &window);
        if (window) {
            AXUIElementCopyAttributeValue(window, kAXDocumentAttribute, &document);
            CFRelease(window);
        }
        CFRelease(app);
        if (!document) {
            return NULL;
        }
        id value = CFBridgingRelease(document);
        if (![value isKindOfClass:NSString.class]) {
            return NULL;
        }
        NSString *url = value;
        NSString *path = [url hasPrefix:@"file://"] ? [NSURL URLWithString:url].path : url;
        return path.length ? strdup(path.UTF8String) : NULL;
    }
}
