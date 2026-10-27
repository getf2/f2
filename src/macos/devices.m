// What's plugged in and paired, for Quick Open: the Mac's battery
// (IOKit's power sources), where the sound goes (CoreAudio's outputs, and
// the one to use), and Bluetooth devices to connect and disconnect
// (IOBluetooth's: on the main thread, as it wants).

#import <AppKit/AppKit.h>
#import <CoreAudio/CoreAudio.h>
#import <IOBluetooth/IOBluetooth.h>
#import <IOKit/ps/IOPSKeys.h>
#import <IOKit/ps/IOPowerSources.h>

static void on_main(dispatch_block_t work) {
    if (NSThread.isMainThread) work();
    else dispatch_sync(dispatch_get_main_queue(), work);
}

// The Mac's battery: its charge (0 to 100), whether it's charging and on
// power, and the minutes till it's full (charging) or empty (-1: not known
// yet). False: there's no battery.
bool nf_battery(int *percent, bool *charging, bool *plugged, int *minutes) {
    CFTypeRef info = IOPSCopyPowerSourcesInfo();
    if (!info) return false;
    CFArrayRef sources = IOPSCopyPowerSourcesList(info);
    bool found = false;
    for (CFIndex i = 0; sources && i < CFArrayGetCount(sources); i++) {
        NSDictionary *source = (__bridge NSDictionary *)IOPSGetPowerSourceDescription(info, CFArrayGetValueAtIndex(sources, i));
        if (![source[@kIOPSTypeKey] isEqual:@kIOPSInternalBatteryType]) continue;
        int now = [source[@kIOPSCurrentCapacityKey] intValue], max = [source[@kIOPSMaxCapacityKey] intValue];
        *percent = max > 0 ? (int)lround(100.0 * now / max) : now;
        *charging = [source[@kIOPSIsChargingKey] boolValue];
        *plugged = [source[@kIOPSPowerSourceStateKey] isEqual:@kIOPSACPowerValue];
        NSNumber *left = *charging ? source[@kIOPSTimeToFullChargeKey] : source[@kIOPSTimeToEmptyKey];
        *minutes = left ? left.intValue : -1;
        found = true;
        break;
    }
    if (sources) CFRelease(sources);
    CFRelease(info);
    return found;
}

static AudioObjectID default_output(void) {
    AudioObjectID device = kAudioObjectUnknown;
    UInt32 size = sizeof device;
    AudioObjectPropertyAddress address = {kAudioHardwarePropertyDefaultOutputDevice, kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMain};
    AudioObjectGetPropertyData(kAudioObjectSystemObject, &address, 0, NULL, &size, &device);
    return device;
}

// The sound outputs, a line each: "id \t name \t kind \t 1 if it's in use",
// kind "builtin", "bluetooth", "airplay", "display", "usb" or "other"
// (malloc'd).
char *nf_audio_outputs(void) {
    @autoreleasepool {
        AudioObjectPropertyAddress all = {kAudioHardwarePropertyDevices, kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMain};
        UInt32 size = 0;
        if (AudioObjectGetPropertyDataSize(kAudioObjectSystemObject, &all, 0, NULL, &size) != noErr) return strdup("");
        UInt32 count = size / sizeof(AudioObjectID);
        AudioObjectID *devices = calloc(count, sizeof(AudioObjectID));
        AudioObjectGetPropertyData(kAudioObjectSystemObject, &all, 0, NULL, &size, devices);
        AudioObjectID used = default_output();
        NSMutableArray<NSString *> *lines = [NSMutableArray array];
        for (UInt32 i = 0; i < count; i++) {
            AudioObjectID device = devices[i];
            // (Outputs only: those with a stream to play into.)
            AudioObjectPropertyAddress streams = {kAudioDevicePropertyStreams, kAudioDevicePropertyScopeOutput, kAudioObjectPropertyElementMain};
            UInt32 streams_size = 0;
            if (AudioObjectGetPropertyDataSize(device, &streams, 0, NULL, &streams_size) != noErr || streams_size == 0) continue;
            UInt32 hidden = 0, hidden_size = sizeof hidden;
            AudioObjectPropertyAddress is_hidden = {kAudioDevicePropertyIsHidden, kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMain};
            if (AudioObjectGetPropertyData(device, &is_hidden, 0, NULL, &hidden_size, &hidden) == noErr && hidden) continue;
            CFStringRef name = NULL;
            UInt32 name_size = sizeof name;
            AudioObjectPropertyAddress named = {kAudioObjectPropertyName, kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMain};
            if (AudioObjectGetPropertyData(device, &named, 0, NULL, &name_size, &name) != noErr || !name) continue;
            UInt32 transport = 0, transport_size = sizeof transport;
            AudioObjectPropertyAddress how = {kAudioDevicePropertyTransportType, kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMain};
            AudioObjectGetPropertyData(device, &how, 0, NULL, &transport_size, &transport);
            NSString *kind = @"other";
            switch (transport) {
                case kAudioDeviceTransportTypeBuiltIn: kind = @"builtin"; break;
                case kAudioDeviceTransportTypeBluetooth:
                case kAudioDeviceTransportTypeBluetoothLE: kind = @"bluetooth"; break;
                case kAudioDeviceTransportTypeAirPlay: kind = @"airplay"; break;
                case kAudioDeviceTransportTypeHDMI:
                case kAudioDeviceTransportTypeDisplayPort: kind = @"display"; break;
                case kAudioDeviceTransportTypeUSB: kind = @"usb"; break;
            }
            NSString *clean = [(__bridge NSString *)name stringByReplacingOccurrencesOfString:@"\t" withString:@" "];
            [lines addObject:[NSString stringWithFormat:@"%u\t%@\t%@\t%d", device, clean, kind, device == used]];
            CFRelease(name);
        }
        free(devices);
        return strdup([lines componentsJoinedByString:@"\n"].UTF8String);
    }
}

// Play sound (and the system's alerts) through `device`. Whether it took.
bool nf_set_audio_output(uint32_t device) {
    AudioObjectID id = device;
    AudioObjectPropertyAddress output = {kAudioHardwarePropertyDefaultOutputDevice, kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMain};
    bool ok = AudioObjectSetPropertyData(kAudioObjectSystemObject, &output, 0, NULL, sizeof id, &id) == noErr;
    AudioObjectPropertyAddress alerts = {kAudioHardwarePropertyDefaultSystemOutputDevice, kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMain};
    AudioObjectSetPropertyData(kAudioObjectSystemObject, &alerts, 0, NULL, sizeof id, &id);
    return ok;
}

// The Bluetooth devices paired with the Mac, a line each: "address \t name
// \t 1 if connected" (malloc'd). (Touching Bluetooth: macOS asks the first
// time, so only once it's allowed.)
char *nf_bluetooth_devices(void) {
    __block char *out = NULL;
    on_main(^{
      @autoreleasepool {
          NSMutableArray<NSString *> *lines = [NSMutableArray array];
          for (IOBluetoothDevice *device in IOBluetoothDevice.pairedDevices) {
              if (!device.addressString.length || !device.name.length) continue;
              NSString *name = [device.name stringByReplacingOccurrencesOfString:@"\t" withString:@" "];
              [lines addObject:[NSString stringWithFormat:@"%@\t%@\t%d", device.addressString, name, device.isConnected]];
          }
          out = strdup([lines componentsJoinedByString:@"\n"].UTF8String);
      }
    });
    return out;
}

// (Told when a connection's made: with a target, IOBluetooth connects in
// the background, not holding up the main thread till it's done.)
@interface NFConnecting : NSObject
@end

@implementation NFConnecting
- (void)connectionComplete:(IOBluetoothDevice *__unused)device status:(IOReturn __unused)status {
}
@end

static NFConnecting *connecting;

// Connect to a paired device (as clicking it in Control Center does), or
// disconnect it. Connecting goes on after this returns: see
// `nf_bluetooth_connected`.
void nf_bluetooth_connect(const char *address, bool connect) {
    NSString *wanted = @(address);
    dispatch_async(dispatch_get_main_queue(), ^{
      IOBluetoothDevice *device = [IOBluetoothDevice deviceWithAddressString:wanted];
      if (!device) return;
      if (connect) {
          if (!connecting) connecting = [NFConnecting new];
          [device openConnection:connecting];
      } else {
          [device closeConnection];
      }
    });
}

// Whether a paired device is connected now.
bool nf_bluetooth_connected(const char *address) {
    __block bool connected = false;
    NSString *wanted = @(address);
    on_main(^{
      connected = [IOBluetoothDevice deviceWithAddressString:wanted].isConnected;
    });
    return connected;
}
