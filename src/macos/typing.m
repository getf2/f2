// Typing's part on the Mac (src/typing.rs: Icelandic on ABC's keys): keys
// heard in any app by an event tap of its own (with Accessibility, on a
// thread of its own, as dictation's), each key down given to Rust, which
// says whether to keep it from the app and what to type instead. That's
// typed as Vikey and OpenKey type: Delete so many times, then keys with
// their characters set, posted from the tap so they come before whatever's
// pressed next. And keyboard layouts, as macOS reads keys with them.

#import <AppKit/AppKit.h>
#import <Carbon/Carbon.h>
#import <stdatomic.h>

typedef struct {
    uint32_t back;
    uint16_t guard;
    uint32_t length;
    uint16_t text[256];
    bool key;
} nf_typing_out;

// (On what F2 types itself, so the tap lets it by: "F2Ty".)
static const int64_t NF_TYPED = 0x46325479;

static CFMachPortRef typingTap;
static CGEventSourceRef typingSource;
static bool (*_Atomic typingHeard)(int32_t kind, uint16_t code, uint64_t flags, nf_typing_out *out);
static void (*typingChanged)(int32_t kind, const char *text);
static id typingSourceObserver, typingAppObserver;

// ---- The keys.

// A key pressed and let go: `code`, typing `text` if it's set.
static void nf_typing_press(CGEventTapProxy proxy, CGKeyCode code, const UniChar *text, UniCharCount length) {
    CGEventRef down = CGEventCreateKeyboardEvent(typingSource, code, true);
    CGEventRef up = CGEventCreateKeyboardEvent(typingSource, code, false);
    if (!down || !up) {
        if (down) CFRelease(down);
        if (up) CFRelease(up);
        return;
    }
    CGEventSetIntegerValueField(down, kCGEventSourceUserData, NF_TYPED);
    CGEventSetIntegerValueField(up, kCGEventSourceUserData, NF_TYPED);
    if (length) {
        CGEventKeyboardSetUnicodeString(down, length, text);
        CGEventKeyboardSetUnicodeString(up, length, text);
    }
    CGEventTapPostEvent(proxy, down);
    CGEventTapPostEvent(proxy, up);
    CFRelease(down);
    CFRelease(up);
}

// `text` typed, 16 characters a key at most (as apps take them), a pair of
// surrogates never split.
static void nf_typing_type(CGEventTapProxy proxy, const UniChar *text, uint32_t length) {
    uint32_t at = 0;
    while (at < length) {
        uint32_t n = MIN(16, length - at);
        if (n > 1 && at + n < length && CFStringIsSurrogateHighCharacter(text[at + n - 1])) n--;
        nf_typing_press(proxy, 0, text + at, n);
        at += n;
    }
}

static CGEventRef nf_typing_event(CGEventTapProxy proxy, CGEventType type, CGEventRef event, __unused void *info) {
    if (type == kCGEventTapDisabledByTimeout || type == kCGEventTapDisabledByUserInput) {
        if (typingTap && atomic_load(&typingHeard)) CGEventTapEnable(typingTap, true);
        return event;
    }
    bool (*heard)(int32_t, uint16_t, uint64_t, nf_typing_out *) = atomic_load(&typingHeard);
    if (!heard || CGEventGetIntegerValueField(event, kCGEventSourceUserData) == NF_TYPED) {
        return event;
    }
    // (Sent by F2 itself, a snippet or dictation pasting: the word typed
    // isn't what it was.)
    bool own = CGEventGetIntegerValueField(event, kCGEventSourceUnixProcessID) == getpid();
    int32_t kind = own ? 2 : type == kCGEventKeyDown ? 1 : 0;
    uint16_t code = (uint16_t)CGEventGetIntegerValueField(event, kCGKeyboardEventKeycode);
    nf_typing_out out = {0};
    if (!heard(kind, code, (uint64_t)CGEventGetFlags(event), &out)) {
        return event;
    }
    if (out.back) {
        // (Something typed first, then taken back too: an address bar's
        // suggestion, selected after what's typed, goes, not a letter.)
        if (out.guard) nf_typing_press(proxy, 0, &out.guard, 1);
        uint32_t back = out.back + (out.guard ? 1 : 0);
        for (uint32_t i = 0; i < back; i++) nf_typing_press(proxy, kVK_Delete, NULL, 0);
    }
    nf_typing_type(proxy, out.text, MIN(out.length, 256));
    if (out.key) {
        CGEventRef again = CGEventCreateCopy(event);
        CGEventSetIntegerValueField(again, kCGEventSourceUserData, NF_TYPED);
        CGEventTapPostEvent(proxy, again);
        CFRelease(again);
    }
    return NULL;
}

static void nf_typing_front(NSRunningApplication *app) {
    if (typingChanged) typingChanged(1, (app.bundleIdentifier ?: app.localizedName ?: @"").UTF8String);
}

// Hear keys down and clicks in any app (see typing_listen in macos.rs), and
// the input source changing and apps coming to the front. NULL: stop.
// False if it can't (no Accessibility). On the main thread.
bool nf_typing_listen(bool (*heard)(int32_t kind, uint16_t code, uint64_t flags, nf_typing_out *out), void (*changed)(int32_t kind, const char *text)) {
    atomic_store(&typingHeard, heard);
    typingChanged = heard ? changed : NULL;
    if (!heard) {
        if (typingTap) CGEventTapEnable(typingTap, false);
        if (typingSourceObserver) [NSDistributedNotificationCenter.defaultCenter removeObserver:typingSourceObserver];
        if (typingAppObserver) [NSWorkspace.sharedWorkspace.notificationCenter removeObserver:typingAppObserver];
        typingSourceObserver = typingAppObserver = nil;
        return true;
    }
    if (!typingTap && !AXIsProcessTrusted()) return false;
    if (!typingSourceObserver) {
        typingSourceObserver = [NSDistributedNotificationCenter.defaultCenter addObserverForName:(__bridge NSString *)kTISNotifySelectedKeyboardInputSourceChanged
                                                                                         object:nil
                                                                                          queue:NSOperationQueue.mainQueue
                                                                                     usingBlock:^(__unused NSNotification *n) {
                                                                                       if (typingChanged) typingChanged(0, NULL);
                                                                                     }];
        typingAppObserver = [NSWorkspace.sharedWorkspace.notificationCenter addObserverForName:NSWorkspaceDidActivateApplicationNotification
                                                                                         object:nil
                                                                                          queue:NSOperationQueue.mainQueue
                                                                                     usingBlock:^(NSNotification *n) {
                                                                                       nf_typing_front(n.userInfo[NSWorkspaceApplicationKey]);
                                                                                     }];
    }
    nf_typing_front(NSWorkspace.sharedWorkspace.frontmostApplication);
    if (typingTap) {
        CGEventTapEnable(typingTap, true);
        return true;
    }
    typingSource = CGEventSourceCreate(kCGEventSourceStatePrivate);
    CGEventMask mask = CGEventMaskBit(kCGEventKeyDown) | CGEventMaskBit(kCGEventLeftMouseDown) | CGEventMaskBit(kCGEventRightMouseDown) | CGEventMaskBit(kCGEventOtherMouseDown);
    CFMachPortRef tap = CGEventTapCreate(kCGSessionEventTap, kCGHeadInsertEventTap, kCGEventTapOptionDefault, mask, nf_typing_event, NULL);
    if (!tap) return false;
    typingTap = tap;
    NSThread *thread = [[NSThread alloc] initWithBlock:^{
      CFRunLoopSourceRef source = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, tap, 0);
      CFRunLoopAddSource(CFRunLoopGetCurrent(), source, kCFRunLoopCommonModes);
      CFRelease(source);
      CGEventTapEnable(tap, true);
      CFRunLoopRun();
    }];
    thread.name = @"F2 typing keys";
    thread.qualityOfService = NSQualityOfServiceUserInteractive;
    [thread start];
    return true;
}

// ---- Layouts.

// Whether `code`'s key types (Return, Tab and Space end words instead).
static bool nf_typing_key(UInt16 code) {
    if (code <= 50) return code != kVK_Return && code != kVK_Tab && code != kVK_Space;
    switch (code) {
        case kVK_ANSI_KeypadDecimal: case kVK_ANSI_KeypadMultiply: case kVK_ANSI_KeypadPlus: case kVK_ANSI_KeypadDivide:
        case kVK_ANSI_KeypadMinus: case kVK_ANSI_KeypadEquals: case kVK_ANSI_Keypad0: case kVK_ANSI_Keypad1: case kVK_ANSI_Keypad2:
        case kVK_ANSI_Keypad3: case kVK_ANSI_Keypad4: case kVK_ANSI_Keypad5: case kVK_ANSI_Keypad6: case kVK_ANSI_Keypad7:
        case kVK_ANSI_Keypad8: case kVK_ANSI_Keypad9:
            return true;
    }
    return false;
}

static NSString *nf_typing_text(const UniChar *chars, UniCharCount length) {
    for (UniCharCount i = 0; i < length; i++) {
        if (chars[i] < 0x20 || chars[i] == 0x7f) return nil;
    }
    return [NSString stringWithCharacters:chars length:length];
}

// The keys a keyboard layout types (`id`'s; NULL: the Mac's own now, if
// it's a layout for English), as JSON: {"keys": [[dead key waiting, key
// code, modifiers, text, dead key left], …]}. Modifiers as UCKeyTranslate
// takes them (⇧ 2, ⇪ 4, ⌥ 8), dead keys as its states' low halves (0:
// none), each one's keys read too. Malloc'd; NULL if there's no such
// layout.
char *nf_typing_layout(const char *id) {
    TISInputSourceRef source = NULL;
    if (id) {
        NSDictionary *wanted = @{(__bridge NSString *)kTISPropertyInputSourceID: @(id)};
        CFArrayRef list = TISCreateInputSourceList((__bridge CFDictionaryRef)wanted, true);
        if (list && CFArrayGetCount(list)) source = (TISInputSourceRef)CFRetain(CFArrayGetValueAtIndex(list, 0));
        if (list) CFRelease(list);
    } else {
        source = TISCopyCurrentKeyboardInputSource();
        // (Not the Icelandic layout itself, nor an input method: Korean's.)
        NSString *type = source ? (__bridge NSString *)TISGetInputSourceProperty(source, kTISPropertyInputSourceType) : nil;
        NSArray<NSString *> *languages = source ? (__bridge NSArray *)TISGetInputSourceProperty(source, kTISPropertyInputSourceLanguages) : nil;
        if (![type isEqualToString:(__bridge NSString *)kTISTypeKeyboardLayout] || ![languages.firstObject hasPrefix:@"en"]) {
            if (source) CFRelease(source);
            return NULL;
        }
    }
    if (!source) return NULL;
    CFDataRef data = TISGetInputSourceProperty(source, kTISPropertyUnicodeKeyLayoutData);
    if (!data) {
        CFRelease(source);
        return NULL;
    }
    const UCKeyboardLayout *layout = (const UCKeyboardLayout *)CFDataGetBytePtr(data);
    UInt32 board = LMGetKbdType();
    static const UInt32 mods[] = {0, 2, 4, 6, 8, 10, 12, 14};
    NSMutableArray *keys = [NSMutableArray array];
    NSMutableArray<NSNumber *> *todo = [NSMutableArray arrayWithObject:@0];
    NSMutableSet<NSNumber *> *seen = [NSMutableSet setWithObject:@0];
    while (todo.count) {
        UInt32 state = todo.lastObject.unsignedIntValue;
        [todo removeLastObject];
        for (UInt16 code = 0; code < 128; code++) {
            if (!nf_typing_key(code)) continue;
            for (size_t m = 0; m < sizeof mods / sizeof *mods; m++) {
                UInt32 dead = state;
                UniChar chars[8];
                UniCharCount length = 0;
                if (UCKeyTranslate(layout, code, kUCKeyActionDown, mods[m], board, 0, &dead, 8, &length, chars) != noErr) continue;
                // (Its low half's the dead key: the high one says nothing of one.)
                dead &= 0xffff;
                NSString *text = nf_typing_text(chars, length);
                if (!text || (!length && !dead)) continue;
                [keys addObject:@[@(state), @(code), @(mods[m]), text, @(dead)]];
                // (At most so many: no layout has more.)
                if (dead && ![seen containsObject:@(dead)] && seen.count < 64) {
                    [seen addObject:@(dead)];
                    [todo addObject:@(dead)];
                }
            }
        }
    }
    CFRelease(source);
    NSData *json = [NSJSONSerialization dataWithJSONObject:@{@"keys": keys} options:0 error:nil];
    if (!json) return NULL;
    char *out = malloc(json.length + 1);
    memcpy(out, json.bytes, json.length);
    out[json.length] = 0;
    return out;
}
