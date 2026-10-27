// Sound, music and the screen, for Quick Open: the volume (CoreAudio's
// default output), the media keys (as the keyboard's play and skip keys
// send them, to whatever's playing), and the built-in screen's brightness
// (DisplayServices', as the brightness keys set it: there's no public way).

#import <AppKit/AppKit.h>
#import <CoreAudio/CoreAudio.h>
#import <AudioToolbox/AudioServices.h>
#import <dlfcn.h>

static AudioObjectID output_device(void) {
    AudioObjectID device = kAudioObjectUnknown;
    UInt32 size = sizeof device;
    AudioObjectPropertyAddress address = {kAudioHardwarePropertyDefaultOutputDevice, kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMain};
    if (AudioObjectGetPropertyData(kAudioObjectSystemObject, &address, 0, NULL, &size, &device) != noErr) return kAudioObjectUnknown;
    return device;
}

// The output's volume, 0 to 1 (below 0: none to set). `set` from 0 to 1
// sets it first (below 0: as it is).
float nf_volume(float set) {
    AudioObjectID device = output_device();
    if (device == kAudioObjectUnknown) return -1;
    AudioObjectPropertyAddress address = {kAudioHardwareServiceDeviceProperty_VirtualMainVolume, kAudioDevicePropertyScopeOutput, kAudioObjectPropertyElementMain};
    if (!AudioObjectHasProperty(device, &address)) return -1;
    if (set >= 0) {
        Float32 value = set > 1 ? 1 : set;
        AudioObjectSetPropertyData(device, &address, 0, NULL, sizeof value, &value);
    }
    Float32 value = 0;
    UInt32 size = sizeof value;
    if (AudioObjectGetPropertyData(device, &address, 0, NULL, &size, &value) != noErr) return -1;
    return value;
}

// Whether the output's muted (below 0: it can't be). `set` 0 or 1 mutes or
// unmutes it first (below 0: as it is).
int nf_muted(int set) {
    AudioObjectID device = output_device();
    if (device == kAudioObjectUnknown) return -1;
    AudioObjectPropertyAddress address = {kAudioDevicePropertyMute, kAudioDevicePropertyScopeOutput, kAudioObjectPropertyElementMain};
    if (!AudioObjectHasProperty(device, &address)) return -1;
    if (set >= 0) {
        UInt32 value = set ? 1 : 0;
        AudioObjectSetPropertyData(device, &address, 0, NULL, sizeof value, &value);
    }
    UInt32 value = 0;
    UInt32 size = sizeof value;
    if (AudioObjectGetPropertyData(device, &address, 0, NULL, &size, &value) != noErr) return -1;
    return value ? 1 : 0;
}

// A media key pressed (NX_KEYTYPE_PLAY 16, NEXT 17, PREVIOUS 18), as the
// keyboard sends it: to whatever's playing (Music, Spotify, a browser's).
void nf_media_key(int key) {
    @autoreleasepool {
        for (int down = 1; down >= 0; down--) {
            NSEvent *event = [NSEvent otherEventWithType:NSEventTypeSystemDefined
                                                location:NSZeroPoint
                                           modifierFlags:(down ? 0xa00 : 0xb00)
                                               timestamp:0
                                            windowNumber:0
                                                 context:nil
                                                 subtype:8
                                                   data1:(key << 16) | ((down ? 0xa : 0xb) << 8)
                                                   data2:-1];
            CGEventPost(kCGHIDEventTap, event.CGEvent);
        }
    }
}

// The built-in screen's brightness, 0 to 1 (below 0: there's none here, or
// it can't be set). `set` from 0 to 1 sets it first.
float nf_brightness(float set) {
    static int (*get)(CGDirectDisplayID, float *) = NULL;
    static int (*put)(CGDirectDisplayID, float) = NULL;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
      void *lib = dlopen("/System/Library/PrivateFrameworks/DisplayServices.framework/DisplayServices", RTLD_LAZY);
      if (!lib) return;
      get = (int (*)(CGDirectDisplayID, float *))dlsym(lib, "DisplayServicesGetBrightness");
      put = (int (*)(CGDirectDisplayID, float))dlsym(lib, "DisplayServicesSetBrightness");
    });
    if (!get || !put) return -1;
    CGDirectDisplayID displays[16];
    uint32_t count = 0;
    if (CGGetOnlineDisplayList(16, displays, &count) != kCGErrorSuccess) return -1;
    for (uint32_t i = 0; i < count; i++) {
        if (!CGDisplayIsBuiltin(displays[i])) continue;
        if (set >= 0) put(displays[i], set > 1 ? 1 : set);
        float value = -1;
        if (get(displays[i], &value) != 0) return -1;
        return value;
    }
    return -1;
}
