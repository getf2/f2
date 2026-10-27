// Finder's sounds (the Trash's), played as macOS plays its own interface
// sounds: at the alert volume, through the alert device, and only while
// "Play user interface sound effects" is on (System Settings › Sound).

#import <AudioToolbox/AudioToolbox.h>
#import <Foundation/Foundation.h>

void nf_play_ui_sound(const char *path) {
    static NSMutableDictionary<NSString *, NSNumber *> *loaded;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        loaded = [NSMutableDictionary dictionary];
    });
    NSString *file = [NSString stringWithUTF8String:path];
    if (!file) return;
    SystemSoundID sound = 0;
    @synchronized(loaded) {
        NSNumber *known = loaded[file];
        if (known) {
            sound = known.unsignedIntValue;
        } else {
            NSURL *url = [NSURL fileURLWithPath:file];
            if (AudioServicesCreateSystemSoundID((__bridge CFURLRef)url, &sound) != kAudioServicesNoError) return;
            // (1 is the default, said anyway: it's what keeps the setting
            // above in charge.)
            UInt32 ui = 1;
            AudioServicesSetProperty(kAudioServicesPropertyIsUISound, sizeof(sound), &sound, sizeof(ui), &ui);
            loaded[file] = @(sound);
        }
    }
    AudioServicesPlaySystemSound(sound);
}
