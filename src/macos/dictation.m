// Dictation's part on the Mac: its keys heard in any app (an event tap,
// with Accessibility: Rust says which to let through), the microphone
// (which one: see nf_mic_pick) heard as 16 kHz mono (what speech.swift and
// other speech models take), and the pill at the bottom of the screen
// while it listens.

#import <AppKit/AppKit.h>
#import <AVFoundation/AVFoundation.h>
#import <AudioToolbox/AudioToolbox.h>
#import <CoreAudio/CoreAudio.h>
#import <IOKit/IOKitLib.h>
#import <os/lock.h>
#import <stdatomic.h>

uint32_t nf_screen_with_front_window(void);
uint32_t nf_screen_with_pointer(void);

// ---- The keys.

static CFMachPortRef dictationTap;
static bool (*_Atomic dictationHeard)(int32_t kind, uint16_t code, uint64_t flags, bool repeat);

static CGEventRef nf_dictation_event(__unused CGEventTapProxy proxy, CGEventType type, CGEventRef event, __unused void *info) {
    if (type == kCGEventTapDisabledByTimeout || type == kCGEventTapDisabledByUserInput) {
        if (dictationTap && atomic_load(&dictationHeard)) CGEventTapEnable(dictationTap, true);
        return event;
    }
    bool (*heard)(int32_t, uint16_t, uint64_t, bool) = atomic_load(&dictationHeard);
    // (What F2 sends itself, pasting: not you.)
    if (!heard || CGEventGetIntegerValueField(event, kCGEventSourceUnixProcessID) == getpid()) {
        return event;
    }
    int32_t kind = type == kCGEventFlagsChanged ? 0 : type == kCGEventKeyDown ? 1 : 2;
    uint16_t code = (uint16_t)CGEventGetIntegerValueField(event, kCGKeyboardEventKeycode);
    bool repeat = type == kCGEventKeyDown && CGEventGetIntegerValueField(event, kCGKeyboardEventAutorepeat) != 0;
    return heard(kind, code, (uint64_t)CGEventGetFlags(event), repeat) ? NULL : event;
}

// Hear every key, calling `heard` with what it is (0: a modifier changed,
// 1: down, 2: up), its key code, the modifiers (their left and right bits
// too) and whether it's held down repeating; true keeps it from the app.
// NULL: stop. False if it can't (no Accessibility). `heard` is called on
// a thread of its own, so every app's keys never wait on F2's main one.
bool nf_dictation_listen(bool (*heard)(int32_t kind, uint16_t code, uint64_t flags, bool repeat)) {
    atomic_store(&dictationHeard, heard);
    if (!heard) {
        if (dictationTap) CGEventTapEnable(dictationTap, false);
        return true;
    }
    if (dictationTap) {
        CGEventTapEnable(dictationTap, true);
        return true;
    }
    if (!AXIsProcessTrusted()) return false;
    CGEventMask mask = CGEventMaskBit(kCGEventKeyDown) | CGEventMaskBit(kCGEventKeyUp) | CGEventMaskBit(kCGEventFlagsChanged);
    CFMachPortRef tap = CGEventTapCreate(kCGSessionEventTap, kCGHeadInsertEventTap, kCGEventTapOptionDefault, mask, nf_dictation_event, NULL);
    if (!tap) return false;
    dictationTap = tap;
    NSThread *thread = [[NSThread alloc] initWithBlock:^{
      CFRunLoopSourceRef source = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, tap, 0);
      CFRunLoopAddSource(CFRunLoopGetCurrent(), source, kCFRunLoopCommonModes);
      CFRelease(source);
      CGEventTapEnable(tap, true);
      CFRunLoopRun();
    }];
    thread.name = @"F2 dictation keys";
    thread.qualityOfService = NSQualityOfServiceUserInteractive;
    [thread start];
    return true;
}

// Whether `code`'s key is down now (one whose letting go wasn't heard,
// in a password field, isn't).
bool nf_key_is_down(uint16_t code) {
    return CGEventSourceKeyState(kCGEventSourceStateHIDSystemState, code);
}

// ---- The microphone.

static dispatch_queue_t micQueue;
static AVAudioEngine *micEngine;
static NSMutableData *micHeard;
static os_unfair_lock micLock = OS_UNFAIR_LOCK_INIT;
static _Atomic float micLevel;
static NSString *micError;
static id micObserver;
// (Ten minutes, at most.)
static const NSUInteger micMost = 16000 * 60 * 10;

static dispatch_queue_t nf_mic_queue(void) {
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        micQueue = dispatch_queue_create("dev.f2.microphone", DISPATCH_QUEUE_SERIAL);
    });
    return micQueue;
}

static void nf_mic_failed(NSString *why) {
    os_unfair_lock_lock(&micLock);
    micError = why;
    os_unfair_lock_unlock(&micLock);
}

// The microphone's allowed: 1, not asked yet: -1, else 0.
int32_t nf_mic_access(void) {
    switch ([AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeAudio]) {
        case AVAuthorizationStatusAuthorized: return 1;
        case AVAuthorizationStatusNotDetermined: return -1;
        default: return 0;
    }
}

// Ask for it (macOS asks, once). Returns at once.
void nf_mic_request(void) {
    // (Without its reason in Info.plist, asking would crash: a bare build.)
    if (![NSBundle.mainBundle objectForInfoDictionaryKey:@"NSMicrophoneUsageDescription"]) return;
    [AVCaptureDevice requestAccessForMediaType:AVMediaTypeAudio completionHandler:^(__unused BOOL granted) {}];
}

// ---- Which microphone: one picked (by its UID), the one in Sound
// settings ("system"), or by itself (""): Sound settings' unless that's
// Bluetooth headphones (AirPods), whose microphone would switch them to
// call quality, then the Mac's own (unless its lid's closed, which turns
// it off), or another (not an iPhone's: that would wake it).

static UInt32 nf_audio_u32(AudioObjectID object, AudioObjectPropertySelector selector) {
    UInt32 value = 0, size = sizeof value;
    AudioObjectPropertyAddress address = {selector, kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMain};
    AudioObjectGetPropertyData(object, &address, 0, NULL, &size, &value);
    return value;
}

static NSString *nf_audio_string(AudioObjectID object, AudioObjectPropertySelector selector) {
    CFStringRef value = NULL;
    UInt32 size = sizeof value;
    AudioObjectPropertyAddress address = {selector, kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMain};
    if (AudioObjectGetPropertyData(object, &address, 0, NULL, &size, &value) != noErr || !value) return nil;
    return (__bridge_transfer NSString *)value;
}

static bool nf_audio_bluetooth(AudioObjectID device) {
    UInt32 transport = nf_audio_u32(device, kAudioDevicePropertyTransportType);
    return transport == kAudioDeviceTransportTypeBluetooth || transport == kAudioDeviceTransportTypeBluetoothLE;
}

// An iPhone's microphone, through Continuity ('ccwd', 'ccwl'; 'ccap' on
// macOS 13).
static bool nf_audio_iphone(AudioObjectID device) {
    UInt32 transport = nf_audio_u32(device, kAudioDevicePropertyTransportType);
    return transport == 'ccwd' || transport == 'ccwl' || transport == 'ccap';
}

static AudioObjectID nf_default_input(void) {
    return nf_audio_u32(kAudioObjectSystemObject, kAudioHardwarePropertyDefaultInputDevice);
}

// Whether the lid's closed (a MacBook's; its microphone's off then,
// though still listed).
static bool nf_lid_closed(void) {
    io_service_t root = IOServiceGetMatchingService(kIOMainPortDefault, IOServiceMatching("IOPMrootDomain"));
    if (!root) return false;
    CFTypeRef state = IORegistryEntryCreateCFProperty(root, CFSTR("AppleClamshellState"), kCFAllocatorDefault, 0);
    IOObjectRelease(root);
    bool closed = state && CFGetTypeID(state) == CFBooleanGetTypeID() && CFBooleanGetValue(state);
    if (state) CFRelease(state);
    return closed;
}

// The inputs there are, in CoreAudio's order (not hidden ones).
static NSArray<NSNumber *> *nf_inputs(void) {
    AudioObjectPropertyAddress all = {kAudioHardwarePropertyDevices, kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMain};
    UInt32 size = 0;
    if (AudioObjectGetPropertyDataSize(kAudioObjectSystemObject, &all, 0, NULL, &size) != noErr) return @[];
    UInt32 count = size / sizeof(AudioObjectID);
    AudioObjectID *devices = calloc(count, sizeof(AudioObjectID));
    AudioObjectGetPropertyData(kAudioObjectSystemObject, &all, 0, NULL, &size, devices);
    NSMutableArray<NSNumber *> *out = [NSMutableArray array];
    for (UInt32 i = 0; i < count; i++) {
        AudioObjectPropertyAddress streams = {kAudioDevicePropertyStreams, kAudioDevicePropertyScopeInput, kAudioObjectPropertyElementMain};
        UInt32 streams_size = 0;
        if (AudioObjectGetPropertyDataSize(devices[i], &streams, 0, NULL, &streams_size) != noErr || streams_size == 0) continue;
        if (nf_audio_u32(devices[i], kAudioDevicePropertyIsHidden)) continue;
        [out addObject:@(devices[i])];
    }
    free(devices);
    return out;
}

static AudioObjectID nf_mic_pick(NSString *wanted) {
    NSArray<NSNumber *> *inputs = nf_inputs();
    AudioObjectID fallback = nf_default_input();
    if (wanted.length && ![wanted isEqualToString:@"system"]) {
        for (NSNumber *device in inputs) {
            if ([nf_audio_string(device.unsignedIntValue, kAudioDevicePropertyDeviceUID) isEqualToString:wanted]) return device.unsignedIntValue;
        }
        // (Not here now: by itself.)
    } else if (wanted.length) {
        return fallback;
    }
    if (!fallback || !nf_audio_bluetooth(fallback)) return fallback;
    bool closed = nf_lid_closed();
    AudioObjectID other = 0;
    for (NSNumber *device in inputs) {
        AudioObjectID d = device.unsignedIntValue;
        if (nf_audio_bluetooth(d) || nf_audio_iphone(d)) continue;
        if (nf_audio_u32(d, kAudioDevicePropertyTransportType) == kAudioDeviceTransportTypeBuiltIn) {
            if (!closed) return d;
        } else if (!other) {
            other = d;
        }
    }
    return other ?: fallback;
}

// The microphones, a line each: "uid \t name \t kind \t 1 if it's Sound
// settings' input", kind "builtin", "bluetooth", "iphone", "usb" or
// "other" (malloc'd).
char *nf_mic_inputs(void) {
    @autoreleasepool {
        AudioObjectID used = nf_default_input();
        NSMutableArray<NSString *> *lines = [NSMutableArray array];
        for (NSNumber *device in nf_inputs()) {
            AudioObjectID d = device.unsignedIntValue;
            NSString *uid = nf_audio_string(d, kAudioDevicePropertyDeviceUID);
            NSString *name = nf_audio_string(d, kAudioObjectPropertyName);
            if (!uid || !name) continue;
            UInt32 transport = nf_audio_u32(d, kAudioDevicePropertyTransportType);
            NSString *kind = transport == kAudioDeviceTransportTypeBuiltIn ? @"builtin"
                : nf_audio_bluetooth(d)                                ? @"bluetooth"
                : nf_audio_iphone(d)                                   ? @"iphone"
                : transport == kAudioDeviceTransportTypeUSB            ? @"usb"
                                                                       : @"other";
            NSString *clean = [name stringByReplacingOccurrencesOfString:@"\t" withString:@" "];
            [lines addObject:[NSString stringWithFormat:@"%@\t%@\t%@\t%d", uid, clean, kind, d == used]];
        }
        return strdup([lines componentsJoinedByString:@"\n"].UTF8String);
    }
}

// The UID of the microphone `wanted` (as nf_mic_start takes it) comes to
// now (malloc'd), or NULL if there's none.
char *nf_mic_chosen(const char *wanted) {
    @autoreleasepool {
        AudioObjectID device = nf_mic_pick(wanted ? @(wanted) : @"");
        NSString *uid = device ? nf_audio_string(device, kAudioDevicePropertyDeviceUID) : nil;
        return uid ? strdup(uid.UTF8String) : NULL;
    }
}

static AudioObjectID micDevice;

// Start hearing the microphone `wanted` comes to (see nf_mic_pick),
// afresh. Returns at once; nf_mic_error says if it didn't start.
void nf_mic_start(const char *wanted) {
    NSString *want = wanted ? @(wanted) : @"";
    dispatch_async(nf_mic_queue(), ^{
        atomic_store(&micLevel, 0);
        os_unfair_lock_lock(&micLock);
        micError = nil;
        micHeard = [NSMutableData data];
        os_unfair_lock_unlock(&micLock);
        AudioObjectID device = nf_mic_pick(want);
        // (Another microphone: an engine for it.)
        if (micEngine && micDevice != device) micEngine = nil;
        @try {
            if (!micEngine) {
                micEngine = [AVAudioEngine new];
                micDevice = device;
                if (device) {
                    AudioUnit unit = micEngine.inputNode.audioUnit;
                    if (!unit || AudioUnitSetProperty(unit, kAudioOutputUnitProperty_CurrentDevice, kAudioUnitScope_Global, 0, &device, sizeof device) != noErr) {
                        nf_mic_failed(@"That microphone didn't start.");
                        micEngine = nil;
                        return;
                    }
                }
                // (A headset plugged in, its format changed: a new engine
                // next time.)
                if (micObserver) [NSNotificationCenter.defaultCenter removeObserver:micObserver];
                __weak AVAudioEngine *engine = micEngine;
                micObserver = [NSNotificationCenter.defaultCenter addObserverForName:AVAudioEngineConfigurationChangeNotification object:micEngine queue:nil usingBlock:^(__unused NSNotification *note) {
                    dispatch_async(nf_mic_queue(), ^{
                        if (engine && micEngine == engine && !engine.isRunning) micEngine = nil;
                    });
                }];
            }
            AVAudioInputNode *input = micEngine.inputNode;
            AVAudioFormat *from = [input outputFormatForBus:0];
            if (from.sampleRate <= 0 || from.channelCount == 0) {
                nf_mic_failed(@"There's no microphone.");
                micEngine = nil;
                return;
            }
            AVAudioFormat *to = [[AVAudioFormat alloc] initWithCommonFormat:AVAudioPCMFormatFloat32 sampleRate:16000 channels:1 interleaved:NO];
            // (Converted from what each buffer comes in: a tap given a format
            // that isn't the microphone's throws.)
            __block AVAudioConverter *converter = nil;
            [input removeTapOnBus:0];
            [input installTapOnBus:0 bufferSize:1024 format:nil block:^(AVAudioPCMBuffer *buffer, __unused AVAudioTime *when) {
                if (!converter || ![converter.inputFormat isEqual:buffer.format]) {
                    converter = [[AVAudioConverter alloc] initFromFormat:buffer.format toFormat:to];
                    converter.downmix = YES;
                }
                if (!converter || buffer.format.sampleRate <= 0) return;
                AVAudioFrameCount room = (AVAudioFrameCount)ceil(buffer.frameLength * 16000.0 / buffer.format.sampleRate) + 64;
                AVAudioPCMBuffer *out = [[AVAudioPCMBuffer alloc] initWithPCMFormat:to frameCapacity:room];
                __block bool given = false;
                [converter convertToBuffer:out error:nil withInputFromBlock:^AVAudioBuffer *(__unused AVAudioPacketCount count, AVAudioConverterInputStatus *status) {
                    if (given) {
                        *status = AVAudioConverterInputStatus_NoDataNow;
                        return nil;
                    }
                    given = true;
                    *status = AVAudioConverterInputStatus_HaveData;
                    return buffer;
                }];
                const float *samples = out.floatChannelData[0];
                AVAudioFrameCount n = out.frameLength;
                if (!n) return;
                float sum = 0;
                for (AVAudioFrameCount i = 0; i < n; i++) sum += samples[i] * samples[i];
                // (Loudness, 0 to 1: -50 dB and under is nothing.)
                float db = 10 * log10f(sum / n + 1e-12f);
                atomic_store(&micLevel, fminf(fmaxf((db + 50) / 40, 0), 1));
                os_unfair_lock_lock(&micLock);
                if (micHeard && micHeard.length / sizeof(float) < micMost) [micHeard appendBytes:samples length:n * sizeof(float)];
                os_unfair_lock_unlock(&micLock);
            }];
            [micEngine prepare];
            NSError *error = nil;
            if (![micEngine startAndReturnError:&error]) {
                nf_mic_failed(error.localizedDescription ?: @"The microphone didn't start.");
                [input removeTapOnBus:0];
                micEngine = nil;
            }
        } @catch (NSException *e) {
            nf_mic_failed(e.reason ?: @"The microphone didn't start.");
            micEngine = nil;
        }
    });
}

// Stop hearing it, after what was asked before: `heard` gets what it
// heard (16 kHz mono, `count` samples, `count` 0 if none), on a queue of
// its own; NULL: it's thrown away. Returns at once.
void nf_mic_stop(void (*heard)(const float *samples, size_t count)) {
    dispatch_async(nf_mic_queue(), ^{
        if (micEngine) {
            [micEngine.inputNode removeTapOnBus:0];
            [micEngine stop];
        }
        os_unfair_lock_lock(&micLock);
        NSData *got = micHeard;
        micHeard = nil;
        os_unfair_lock_unlock(&micLock);
        atomic_store(&micLevel, 0);
        if (heard) heard(got.bytes, got.length / sizeof(float));
    });
}

// Why the microphone didn't start, if it didn't (malloc'd), else NULL.
// (Once it's been tried: as it starts, NULL.)
char *nf_mic_error(void) {
    os_unfair_lock_lock(&micLock);
    char *out = micError ? strdup(micError.UTF8String) : NULL;
    os_unfair_lock_unlock(&micLock);
    return out;
}

// ---- The pill: bars that go with your voice while it listens (a lock
// beside them while it listens hands-free), a slow wave while it works.

enum { HudHidden = 0, HudListening = 1, HudHandsFree = 2, HudWorking = 3 };

enum { Bars = 9 };

@interface NFDictationBars : NSView
@property(nonatomic) int mode;
@property(nonatomic) float *levels;
@property(nonatomic) double phase;
@end

@implementation NFDictationBars
- (void)drawRect:(__unused NSRect)dirty {
    NSRect b = self.bounds;
    CGFloat width = 3, gap = 2.5, most = b.size.height - 4, least = 3;
    CGFloat x = NSMidX(b) - (Bars * width + (Bars - 1) * gap) / 2;
    [[NSColor colorWithWhite:1 alpha:self.mode == HudWorking ? 0.55 : 0.92] setFill];
    for (int i = 0; i < Bars; i++) {
        CGFloat level;
        if (self.mode == HudWorking) {
            level = 0.18 + 0.22 * (0.5 + 0.5 * sin(self.phase - i * 0.7));
        } else {
            // (Loudest in the middle, as a voice looks.)
            CGFloat shape = 1 - fabs(i - (Bars - 1) / 2.0) / Bars;
            level = self.levels[i] * shape;
        }
        CGFloat h = least + (most - least) * fmin(fmax(level, 0), 1);
        NSRect bar = NSMakeRect(x + i * (width + gap), NSMidY(b) - h / 2, width, h);
        [[NSBezierPath bezierPathWithRoundedRect:bar xRadius:width / 2 yRadius:width / 2] fill];
    }
}
@end

static NSPanel *hudPanel;
static NFDictationBars *hudBars;
static NSImageView *hudLock;
static NSTimer *hudTimer;
static float hudLevels[Bars];
static int hudMode;

static NSScreen *nf_dictation_screen(void) {
    uint32_t want = nf_screen_with_front_window();
    if (!want) want = nf_screen_with_pointer();
    for (NSScreen *screen in NSScreen.screens) {
        if ([screen.deviceDescription[@"NSScreenNumber"] unsignedIntValue] == want) return screen;
    }
    return NSScreen.mainScreen ?: NSScreen.screens.firstObject;
}

static void nf_dictation_place(void) {
    NSScreen *screen = nf_dictation_screen();
    CGFloat w = hudMode == HudHandsFree ? 92 : 72, h = 28;
    NSRect visible = screen.visibleFrame;
    [hudPanel setFrame:NSMakeRect(round(NSMidX(visible) - w / 2), NSMinY(visible) + 26, w, h) display:YES];
    hudBars.frame = NSMakeRect(hudMode == HudHandsFree ? 16 : 0, 0, 72, h);
    hudLock.hidden = hudMode != HudHandsFree;
}

static void nf_dictation_tick(void) {
    float now = atomic_load(&micLevel);
    // (The newest in the middle, older out to the sides.)
    for (int i = 0; i < Bars / 2; i++) {
        hudLevels[i] = hudLevels[i + 1];
        hudLevels[Bars - 1 - i] = hudLevels[Bars - 2 - i];
    }
    float before = hudLevels[Bars / 2];
    hudLevels[Bars / 2] = now > before ? now : before * 0.6f + now * 0.4f;
    hudBars.phase += 0.25;
    hudBars.needsDisplay = YES;
}

// Show the pill as `mode` is (0: hidden). On the main thread.
void nf_dictation_hud(int32_t mode) {
    if (mode == HudHidden) {
        if (!hudPanel || hudMode == HudHidden) return;
        hudMode = HudHidden;
        [hudTimer invalidate];
        hudTimer = nil;
        NSPanel *panel = hudPanel;
        [NSAnimationContext runAnimationGroup:^(NSAnimationContext *context) {
            context.duration = 0.15;
            panel.animator.alphaValue = 0;
        } completionHandler:^{
            if (hudMode == HudHidden) [panel orderOut:nil];
        }];
        return;
    }
    if (!hudPanel) {
        hudPanel = [[NSPanel alloc] initWithContentRect:NSMakeRect(0, 0, 72, 28) styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel backing:NSBackingStoreBuffered defer:NO];
        hudPanel.level = NSStatusWindowLevel;
        hudPanel.opaque = NO;
        hudPanel.backgroundColor = NSColor.clearColor;
        hudPanel.hasShadow = YES;
        hudPanel.ignoresMouseEvents = YES;
        hudPanel.releasedWhenClosed = NO;
        hudPanel.collectionBehavior = NSWindowCollectionBehaviorCanJoinAllSpaces | NSWindowCollectionBehaviorFullScreenAuxiliary | NSWindowCollectionBehaviorStationary | NSWindowCollectionBehaviorIgnoresCycle;
        hudPanel.appearance = [NSAppearance appearanceNamed:NSAppearanceNameDarkAqua];
        NSVisualEffectView *back = [[NSVisualEffectView alloc] initWithFrame:NSMakeRect(0, 0, 72, 28)];
        back.material = NSVisualEffectMaterialHUDWindow;
        back.blendingMode = NSVisualEffectBlendingModeBehindWindow;
        back.state = NSVisualEffectStateActive;
        back.wantsLayer = YES;
        back.layer.cornerRadius = 14;
        back.layer.masksToBounds = YES;
        back.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
        hudPanel.contentView = back;
        hudBars = [[NFDictationBars alloc] initWithFrame:back.bounds];
        hudBars.levels = hudLevels;
        [back addSubview:hudBars];
        NSImageSymbolConfiguration *small = [NSImageSymbolConfiguration configurationWithPointSize:10 weight:NSFontWeightSemibold];
        hudLock = [NSImageView imageViewWithImage:[[NSImage imageWithSystemSymbolName:@"lock.fill" accessibilityDescription:@"Hands-free"] imageWithSymbolConfiguration:small]];
        hudLock.contentTintColor = [NSColor colorWithWhite:1 alpha:0.75];
        hudLock.frame = NSMakeRect(12, 7, 14, 14);
        [back addSubview:hudLock];
        hudPanel.accessibilityLabel = @"Dictation";
    }
    bool showing = hudMode != HudHidden;
    hudMode = mode;
    hudBars.mode = mode;
    if (!showing) {
        memset(hudLevels, 0, sizeof hudLevels);
        hudPanel.alphaValue = 0;
        nf_dictation_place();
        [hudPanel orderFrontRegardless];
        [NSAnimationContext runAnimationGroup:^(NSAnimationContext *context) {
            context.duration = 0.12;
            hudPanel.animator.alphaValue = 1;
        }];
    } else {
        nf_dictation_place();
        hudPanel.alphaValue = 1;
        [hudPanel orderFrontRegardless];
    }
    if (!hudTimer) {
        hudTimer = [NSTimer timerWithTimeInterval:1.0 / 30 repeats:YES block:^(__unused NSTimer *timer) { nf_dictation_tick(); }];
        [NSRunLoop.mainRunLoop addTimer:hudTimer forMode:NSRunLoopCommonModes];
    }
    hudBars.needsDisplay = YES;
}
