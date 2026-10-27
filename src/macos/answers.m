// What Quick Open asks the system: the time in a time zone (written as this
// Mac writes times), a word in the Dictionary, Wi-Fi's and Bluetooth's
// power, and a reminder added to Reminders.

#import <AppKit/AppKit.h>
#import <CoreBluetooth/CoreBluetooth.h>
#import <CoreLocation/CoreLocation.h>
#import <CoreServices/CoreServices.h>
#import <CoreWLAN/CoreWLAN.h>
#import <EventKit/EventKit.h>
#import <IOBluetooth/IOBluetooth.h>
#import <IOKit/pwr_mgt/IOPMLib.h>
#import <dlfcn.h>
#import <objc/message.h>

// (IOBluetooth's, what blueutil and System Settings use: there's no
// public way to turn Bluetooth on or off.)
int IOBluetoothPreferenceGetControllerPowerState(void);
void IOBluetoothPreferenceSetControllerPowerState(int state);

// A zone by its name ("Asia/Tokyo"), or by its offset from UTC in seconds
// ("offset:19800"); nil if macOS doesn't know it.
static NSTimeZone *zone_named(const char *zone) {
    NSString *name = @(zone);
    if ([name hasPrefix:@"offset:"]) return [NSTimeZone timeZoneForSecondsFromGMT:[name substringFromIndex:7].integerValue];
    return [NSTimeZone timeZoneWithName:name];
}

// The zone's offset from UTC at `at` (seconds since 1970), in seconds. False
// if macOS doesn't know the zone.
bool nf_zone_offset(const char *zone, double at, int32_t *offset) {
    @autoreleasepool {
        NSTimeZone *tz = zone_named(zone);
        if (!tz) return false;
        *offset = (int32_t)[tz secondsFromGMTForDate:[NSDate dateWithTimeIntervalSince1970:at]];
        return true;
    }
}

// This Mac's time zone's name ("Atlantic/Reykjavik"): malloc'd.
char *nf_local_zone(void) {
    @autoreleasepool {
        return strdup(NSTimeZone.localTimeZone.name.UTF8String);
    }
}

// `at` in `zone`, as this Mac writes it, from a template of what to show
// ("jmm": "17:05", or "5:05 PM" if the Mac uses 12-hour time; "EEEEMMMMd":
// "Wednesday, October 21"): malloc'd.
char *nf_zone_format(const char *zone, double at, const char *template) {
    @autoreleasepool {
        NSDateFormatter *f = [NSDateFormatter new];
        f.locale = NSLocale.currentLocale;
        f.timeZone = zone_named(zone) ?: NSTimeZone.localTimeZone;
        f.dateFormat = [NSDateFormatter dateFormatFromTemplate:@(template) options:0 locale:f.locale];
        return strdup([f stringFromDate:[NSDate dateWithTimeIntervalSince1970:at]].UTF8String);
    }
}

// The languages this Mac's set to, first first ("en-US,is-IS"): malloc'd.
char *nf_preferred_languages(void) {
    @autoreleasepool {
        return strdup([NSLocale.preferredLanguages componentsJoinedByString:@","].UTF8String);
    }
}

// The Dictionary's text for `word` (from the dictionaries Dictionary has
// on), or NULL: malloc'd.
char *nf_define(const char *word) {
    @autoreleasepool {
        NSString *w = @(word);
        CFStringRef text = DCSCopyTextDefinition(NULL, (__bridge CFStringRef)w, CFRangeMake(0, (CFIndex)w.length));
        if (!text) return NULL;
        return strdup([(__bridge_transfer NSString *)text UTF8String]);
    }
}

// `work`, on the main thread (CoreWLAN's and IOBluetooth's objects are
// the main thread's; Quick Open asks from others), waiting for it.
static void on_main(dispatch_block_t work) {
    if (NSThread.isMainThread) work();
    else dispatch_sync(dispatch_get_main_queue(), work);
}

// Wi-Fi's power: 1 on, 0 off, -1 if there's no Wi-Fi. `set` 0 or 1 turns it
// off or on first (-1: as it is).
int nf_wifi(int set) {
    __block int power = -1;
    on_main(^{
      @autoreleasepool {
          CWInterface *wifi = CWWiFiClient.sharedWiFiClient.interface;
          if (!wifi) return;
          if (set >= 0) [wifi setPower:set == 1 error:nil];
          power = wifi.powerOn ? 1 : 0;
      }
    });
    return power;
}

// Whether F2 may use Bluetooth: 1 yes, 0 not asked yet, -1 no. (Asking is
// the system's, the first time Bluetooth's touched.)
int nf_bluetooth_allowed(void) {
    switch (CBManager.authorization) {
        case CBManagerAuthorizationAllowedAlways: return 1;
        case CBManagerAuthorizationNotDetermined: return 0;
        default: return -1;
    }
}

// Bluetooth's power: 1 on, 0 off. `set` 0 or 1 turns it off or on first.
int nf_bluetooth(int set) {
    __block int power = 0;
    on_main(^{
      if (set >= 0) IOBluetoothPreferenceSetControllerPowerState(set);
      power = IOBluetoothPreferenceGetControllerPowerState();
    });
    return power;
}

// The keyboards, mice and trackpads connected over Bluetooth, a name a line
// (what turning it off would cut off): malloc'd.
char *nf_bluetooth_inputs(void) {
    __block char *out = NULL;
    on_main(^{
      @autoreleasepool {
          NSMutableArray<NSString *> *names = [NSMutableArray array];
          for (IOBluetoothDevice *device in IOBluetoothDevice.pairedDevices) {
              if (device.isConnected && device.deviceClassMajor == kBluetoothDeviceClassMajorPeripheral) {
                  [names addObject:device.name.length ? device.name : @"A Bluetooth device"];
              }
          }
          out = strdup([names componentsJoinedByString:@"\n"].UTF8String);
      }
    });
    return out;
}

// Add `title` to Reminders' default list, due at `due` (seconds since 1970;
// below 0, no date), at that time with an alert (`timed`) or on that day.
// macOS asks for access the first time, and this waits for the answer: not
// on the main thread. NULL if it's added, else why not ("denied", or the
// error): malloc'd.
char *nf_add_reminder(const char *title, double due, bool timed) {
    @autoreleasepool {
        EKEventStore *store = [EKEventStore new];
        __block BOOL granted = NO;
        dispatch_semaphore_t answered = dispatch_semaphore_create(0);
        void (^completion)(BOOL, NSError *) = ^(BOOL ok, NSError *__unused error) {
          granted = ok;
          dispatch_semaphore_signal(answered);
        };
        if (@available(macOS 14.0, *)) {
            [store requestFullAccessToRemindersWithCompletion:completion];
        } else {
            [store requestAccessToEntityType:EKEntityTypeReminder completion:completion];
        }
        // (Asking waits for you, for a while.)
        dispatch_semaphore_wait(answered, dispatch_time(DISPATCH_TIME_NOW, 300 * NSEC_PER_SEC));
        if (!granted) return strdup("denied");
        EKCalendar *list = store.defaultCalendarForNewReminders;
        if (!list) return strdup("There's no list in Reminders to add it to.");
        EKReminder *reminder = [EKReminder reminderWithEventStore:store];
        reminder.title = @(title);
        reminder.calendar = list;
        if (due >= 0) {
            NSDate *date = [NSDate dateWithTimeIntervalSince1970:due];
            NSCalendarUnit units = NSCalendarUnitYear | NSCalendarUnitMonth | NSCalendarUnitDay;
            if (timed) units |= NSCalendarUnitHour | NSCalendarUnitMinute;
            reminder.dueDateComponents = [NSCalendar.currentCalendar components:units fromDate:date];
            if (timed) [reminder addAlarm:[EKAlarm alarmWithAbsoluteDate:date]];
        }
        NSError *error = nil;
        if (![store saveReminder:reminder commit:YES error:&error]) {
            return strdup((error.localizedDescription ?: @"Reminders couldn't save it.").UTF8String);
        }
        return NULL;
    }
}

// Reminders not done: those due by the end of today (overdue first), then
// those with no date, at most `limit`. A line each: its identifier, title,
// due (seconds since 1970, or -1), whether at a time (1 or 0) and its list,
// split by tabs. NULL if Reminders is off limits.
char *nf_reminders_due(int limit) {
    @autoreleasepool {
        if ((NSInteger)[EKEventStore authorizationStatusForEntityType:EKEntityTypeReminder] != 3) return NULL;
        EKEventStore *store = [EKEventStore new];
        NSCalendar *calendar = NSCalendar.currentCalendar;
        NSDate *tomorrow = [calendar dateByAddingUnit:NSCalendarUnitDay value:1 toDate:[calendar startOfDayForDate:NSDate.date] options:0];
        NSPredicate *predicate = [store predicateForIncompleteRemindersWithDueDateStarting:nil ending:nil calendars:nil];
        __block NSArray<EKReminder *> *found = @[];
        dispatch_semaphore_t done = dispatch_semaphore_create(0);
        [store fetchRemindersMatchingPredicate:predicate
                                    completion:^(NSArray<EKReminder *> *reminders) {
                                      found = reminders ?: @[];
                                      dispatch_semaphore_signal(done);
                                    }];
        dispatch_semaphore_wait(done, dispatch_time(DISPATCH_TIME_NOW, 8 * NSEC_PER_SEC));
        NSMutableArray<EKReminder *> *dated = [NSMutableArray array];
        NSMutableArray<EKReminder *> *undated = [NSMutableArray array];
        NSMutableDictionary<NSString *, NSDate *> *dues = [NSMutableDictionary dictionary];
        for (EKReminder *reminder in found) {
            NSDate *due = reminder.dueDateComponents ? [calendar dateFromComponents:reminder.dueDateComponents] : nil;
            if (!due) {
                [undated addObject:reminder];
            } else if ([due compare:tomorrow] == NSOrderedAscending) {
                dues[reminder.calendarItemIdentifier] = due;
                [dated addObject:reminder];
            }
        }
        [dated sortUsingComparator:^NSComparisonResult(EKReminder *a, EKReminder *b) {
          return [dues[a.calendarItemIdentifier] compare:dues[b.calendarItemIdentifier]];
        }];
        // (Those with no date: the newest first.)
        [undated sortUsingComparator:^NSComparisonResult(EKReminder *a, EKReminder *b) {
          return [(b.creationDate ?: NSDate.distantPast) compare:(a.creationDate ?: NSDate.distantPast)];
        }];
        NSMutableArray<NSString *> *lines = [NSMutableArray array];
        for (EKReminder *reminder in [dated arrayByAddingObjectsFromArray:undated]) {
            if ((int)lines.count >= limit) break;
            NSDate *due = dues[reminder.calendarItemIdentifier];
            BOOL timed = due && reminder.dueDateComponents.hour != NSDateComponentUndefined;
            NSString *title = [[reminder.title ?: @"" componentsSeparatedByCharactersInSet:NSCharacterSet.newlineCharacterSet] componentsJoinedByString:@" "];
            title = [title stringByReplacingOccurrencesOfString:@"\t" withString:@" "];
            NSString *list = [reminder.calendar.title stringByReplacingOccurrencesOfString:@"\t" withString:@" "] ?: @"";
            [lines addObject:[NSString stringWithFormat:@"%@\t%@\t%.0f\t%d\t%@", reminder.calendarItemIdentifier, title, due ? due.timeIntervalSince1970 : -1., timed ? 1 : 0, list]];
        }
        return strdup([lines componentsJoinedByString:@"\n"].UTF8String);
    }
}

// The reminder (by its identifier) checked off. NULL if it is, else why
// not: malloc'd.
char *nf_reminder_done(const char *identifier) {
    @autoreleasepool {
        EKEventStore *store = [EKEventStore new];
        EKCalendarItem *item = [store calendarItemWithIdentifier:@(identifier)];
        if (![item isKindOfClass:EKReminder.class]) return strdup("It's no longer in Reminders.");
        EKReminder *reminder = (EKReminder *)item;
        reminder.completed = YES;
        NSError *error = nil;
        if (![store saveReminder:reminder commit:YES error:&error]) {
            return strdup((error.localizedDescription ?: @"Reminders couldn't save it.").UTF8String);
        }
        return NULL;
    }
}

// The Mac kept awake (display and all, as when you're reading or showing
// something), or let sleep again. Whether it is now.
static IOPMAssertionID awake_assertion = 0;

bool nf_keep_awake(bool on) {
    if (on && !awake_assertion) {
        IOPMAssertionID id = 0;
        if (IOPMAssertionCreateWithName(kIOPMAssertionTypePreventUserIdleDisplaySleep, kIOPMAssertionLevelOn, CFSTR("F2: Keep Awake"), &id) == kIOReturnSuccess) {
            awake_assertion = id;
        }
    } else if (!on && awake_assertion) {
        IOPMAssertionRelease(awake_assertion);
        awake_assertion = 0;
    }
    return awake_assertion != 0;
}

// Whether F2 may see the calendar (and add to it): 1 yes, 2 only add to it,
// 0 not asked yet, -1 no.
int nf_calendar_access(void) {
    // (Raw values: 3 is full access (Authorized, before macOS 14), 4 is
    // write only.)
    switch ((NSInteger)[EKEventStore authorizationStatusForEntityType:EKEntityTypeEvent]) {
        case 3: return 1;
        case 4: return 2;
        case 0: return 0;
        default: return -1;
    }
}

// Ask for the calendar (macOS asks, the first time), waiting for the answer:
// not on the main thread. Whether it's allowed.
bool nf_calendar_request(void) {
    @autoreleasepool {
        EKEventStore *store = [EKEventStore new];
        __block BOOL granted = NO;
        dispatch_semaphore_t answered = dispatch_semaphore_create(0);
        void (^completion)(BOOL, NSError *) = ^(BOOL ok, NSError *__unused error) {
          granted = ok;
          dispatch_semaphore_signal(answered);
        };
        if (@available(macOS 14.0, *)) {
            [store requestFullAccessToEventsWithCompletion:completion];
        } else {
            [store requestAccessToEntityType:EKEntityTypeEvent completion:completion];
        }
        dispatch_semaphore_wait(answered, dispatch_time(DISPATCH_TIME_NOW, 300 * NSEC_PER_SEC));
        return granted;
    }
}

// Whether F2 may add to Reminders: 1 yes, 0 not asked yet, -1 no.
int nf_reminders_access(void) {
    switch ((NSInteger)[EKEventStore authorizationStatusForEntityType:EKEntityTypeReminder]) {
        case 3: return 1;
        case 0: return 0;
        default: return -1;
    }
}

// Ask for Reminders (macOS asks, the first time), waiting for the answer:
// not on the main thread. Whether it's allowed.
bool nf_reminders_request(void) {
    @autoreleasepool {
        EKEventStore *store = [EKEventStore new];
        __block BOOL granted = NO;
        dispatch_semaphore_t answered = dispatch_semaphore_create(0);
        void (^completion)(BOOL, NSError *) = ^(BOOL ok, NSError *__unused error) {
          granted = ok;
          dispatch_semaphore_signal(answered);
        };
        if (@available(macOS 14.0, *)) {
            [store requestFullAccessToRemindersWithCompletion:completion];
        } else {
            [store requestAccessToEntityType:EKEntityTypeReminder completion:completion];
        }
        dispatch_semaphore_wait(answered, dispatch_time(DISPATCH_TIME_NOW, 300 * NSEC_PER_SEC));
        return granted;
    }
}

// Add an event to the default calendar: `title` from `start` to `end`
// (seconds since 1970), or all day. Asks first, if it's not been. NULL if
// it's added, else why not ("denied", or the error): malloc'd.
char *nf_add_event(const char *title, double start, double end, bool all_day) {
    @autoreleasepool {
        if (nf_calendar_access() <= 0 && !nf_calendar_request()) return strdup("denied");
        EKEventStore *store = [EKEventStore new];
        EKCalendar *calendar = store.defaultCalendarForNewEvents;
        if (!calendar) return strdup("There's no calendar to add it to.");
        EKEvent *event = [EKEvent eventWithEventStore:store];
        event.title = @(title);
        event.calendar = calendar;
        event.allDay = all_day;
        event.startDate = [NSDate dateWithTimeIntervalSince1970:start];
        event.endDate = [NSDate dateWithTimeIntervalSince1970:end];
        NSError *error = nil;
        if (![store saveEvent:event span:EKSpanThisEvent commit:YES error:&error]) {
            return strdup((error.localizedDescription ?: @"Calendar couldn't save it.").UTF8String);
        }
        return NULL;
    }
}

// A video call's link in an event (its URL, place or notes): Zoom's, Meet's,
// Teams', Webex's, FaceTime's.
static NSString *call_link(EKEvent *event) {
    static NSArray<NSString *> *hosts;
    static NSDataDetector *links;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
      hosts = @[ @"zoom.us", @"meet.google.com", @"teams.microsoft.com", @"teams.live.com", @"webex.com", @"facetime.apple.com", @"whereby.com", @"meet.jit.si", @"chime.aws", @"gotomeeting.com" ];
      links = [NSDataDetector dataDetectorWithTypes:NSTextCheckingTypeLink error:nil];
    });
    NSMutableArray<NSString *> *texts = [NSMutableArray array];
    if (event.URL.absoluteString) [texts addObject:event.URL.absoluteString];
    if (event.location) [texts addObject:event.location];
    if (event.notes) [texts addObject:event.notes];
    for (NSString *text in texts) {
        for (NSTextCheckingResult *found in [links matchesInString:text options:0 range:NSMakeRange(0, text.length)]) {
            NSString *host = found.URL.host.lowercaseString ?: @"";
            for (NSString *call in hosts) {
                if ([host isEqualToString:call] || [host hasSuffix:[@"." stringByAppendingString:call]]) return found.URL.absoluteString;
            }
        }
    }
    return nil;
}

// The events from `from` to `to` (seconds since 1970; under way then, too),
// soonest first: "start\tend\tall day (0/1)\ttitle\tcalendar\tcall link"
// a line, malloc'd. NULL without access.
char *nf_events(double from, double to) {
    @autoreleasepool {
        if (nf_calendar_access() != 1) return NULL;
        EKEventStore *store = [EKEventStore new];
        NSPredicate *when = [store predicateForEventsWithStartDate:[NSDate dateWithTimeIntervalSince1970:from - 86400]
                                                           endDate:[NSDate dateWithTimeIntervalSince1970:to]
                                                         calendars:nil];
        NSArray<EKEvent *> *events = [[store eventsMatchingPredicate:when] sortedArrayUsingSelector:@selector(compareStartDateWithEvent:)];
        NSMutableString *out = [NSMutableString string];
        NSCharacterSet *breaks = [NSCharacterSet characterSetWithCharactersInString:@"\t\n\r"];
        for (EKEvent *e in events) {
            if (e.endDate.timeIntervalSince1970 <= from) continue;
            NSString *title = [[(e.title ?: @"") componentsSeparatedByCharactersInSet:breaks] componentsJoinedByString:@" "];
            [out appendFormat:@"%.0f\t%.0f\t%d\t%@\t%@\t%@\n", e.startDate.timeIntervalSince1970, e.endDate.timeIntervalSince1970, e.allDay ? 1 : 0, title, e.calendar.title ?: @"", call_link(e) ?: @""];
        }
        return strdup(out.UTF8String);
    }
}

// Whether temperatures are in Fahrenheit here: as System Settings > General
// > Language & Region has them, else as the region does.
bool nf_fahrenheit(void) {
    @autoreleasepool {
        NSString *unit = [NSUserDefaults.standardUserDefaults stringForKey:@"AppleTemperatureUnit"];
        if (unit.length) return [unit isEqualToString:@"Fahrenheit"];
        return ![[NSLocale.currentLocale objectForKey:NSLocaleUsesMetricSystem] boolValue];
    }
}

// Spelling, in English (British if `british`), as the Mac's spell checker
// has it: "1" if `word` is spelled right, else "0" and its guesses, a line
// each, the likeliest first (malloc'd). (In English: left to guess the
// language, "accomodate" is Italian, and right.)
char *nf_spell(const char *word, bool british) {
    __block char *out = NULL;
    NSString *w = @(word);
    on_main(^{
      @autoreleasepool {
          NSSpellChecker *checker = NSSpellChecker.sharedSpellChecker;
          NSString *english = british && [checker.availableLanguages containsObject:@"en_GB"] ? @"en_GB" : @"en";
          NSRange wrong = [checker checkSpellingOfString:w startingAt:0 language:english wrap:NO inSpellDocumentWithTag:0 wordCount:NULL];
          if (wrong.location == NSNotFound) {
              out = strdup("1");
              return;
          }
          NSMutableArray<NSString *> *lines = [NSMutableArray arrayWithObject:@"0"];
          NSString *best = [checker correctionForWordRange:NSMakeRange(0, w.length) inString:w language:english inSpellDocumentWithTag:0];
          if (best.length) [lines addObject:best];
          for (NSString *guess in [checker guessesForWordRange:NSMakeRange(0, w.length) inString:w language:english inSpellDocumentWithTag:0]) {
              if (![lines containsObject:guess]) [lines addObject:guess];
          }
          out = strdup([lines componentsJoinedByString:@"\n"].UTF8String);
      }
    });
    return out;
}

// Whether the misspelled `word` at `range` of `s` is likely a name or
// code, not a typo: a capital not starting a sentence ("Raycast"), a
// capital after the first letter (NASA, iPhone), a digit.
static bool name_or_code(NSString *s, NSRange range, NSString *word) {
    NSCharacterSet *upper = NSCharacterSet.uppercaseLetterCharacterSet;
    if ([word rangeOfCharacterFromSet:NSCharacterSet.decimalDigitCharacterSet].location != NSNotFound) return true;
    if (word.length > 1 && [[word substringFromIndex:1] rangeOfCharacterFromSet:upper].location != NSNotFound) return true;
    if (![upper characterIsMember:[word characterAtIndex:0]]) return false;
    NSCharacterSet *skipped = [NSCharacterSet characterSetWithCharactersInString:@" \t\"'“‘([«„"];
    NSUInteger i = range.location;
    while (i > 0 && [skipped characterIsMember:[s characterAtIndex:i - 1]]) i--;
    return i > 0 && ![[NSCharacterSet characterSetWithCharactersInString:@".!?…\n\r"] characterIsMember:[s characterAtIndex:i - 1]];
}

// `text` with its misspellings fixed: each word macOS's spell checker is
// sure of a correction for (unsure: left as it is), in the text's language
// as you have it (English as British English, if that's yours); names and
// code left alone. As JSON, {"text": …, "fixes": [[from, to], …]}
// (malloc'd); NULL if there's nothing to fix, or no checking its language.
// `unready`: a word had guesses but no correction (the spell checker, just
// woken, can be slow to have them).
char *nf_fix_spelling(const char *text, bool *unready) {
    __block char *out = NULL;
    __block bool late = false;
    NSString *s = @(text);
    on_main(^{
      @autoreleasepool {
          NSSpellChecker *checker = NSSpellChecker.sharedSpellChecker;
          NSOrthography *orthography = nil;
          [checker checkString:s range:NSMakeRange(0, s.length) types:NSTextCheckingTypeOrthography options:nil inSpellDocumentWithTag:0 orthography:&orthography wordCount:NULL];
          NSString *found = orthography.dominantLanguage;
          if (!found.length || [found isEqual:@"und"]) found = checker.language;
          NSString *base = [found componentsSeparatedByCharactersInSet:[NSCharacterSet characterSetWithCharactersInString:@"-_"]].firstObject;
          BOOL (^same)(NSString *) = ^BOOL(NSString *l) {
            return [l isEqual:base] || [l hasPrefix:[base stringByAppendingString:@"_"]] || [l hasPrefix:[base stringByAppendingString:@"-"]];
          };
          NSString *language = nil;
          for (NSString *l in checker.userPreferredLanguages) {
              if (same(l)) {
                  language = l;
                  break;
              }
          }
          if (!language && [checker.availableLanguages containsObject:found]) language = found;
          for (NSString *l in checker.availableLanguages) {
              if (!language && same(l)) language = l;
          }
          if (!language) return;
          NSMutableString *fixed = [s mutableCopy];
          NSMutableArray *fixes = [NSMutableArray array];
          NSInteger shift = 0;
          NSUInteger start = 0;
          while (start < s.length) {
              NSRange wrong = [checker checkSpellingOfString:s startingAt:start language:language wrap:NO inSpellDocumentWithTag:0 wordCount:NULL];
              if (wrong.location == NSNotFound || wrong.length == 0 || wrong.location < start) break;
              start = NSMaxRange(wrong);
              NSString *word = [s substringWithRange:wrong];
              if (name_or_code(s, wrong, word)) continue;
              NSString *to = [checker correctionForWordRange:wrong inString:s language:language inSpellDocumentWithTag:0];
              if (!to.length && [checker guessesForWordRange:wrong inString:s language:language inSpellDocumentWithTag:0].count) late = true;
              if (!to.length || [to isEqual:word]) continue;
              [fixed replaceCharactersInRange:NSMakeRange((NSUInteger)((NSInteger)wrong.location + shift), wrong.length) withString:to];
              shift += (NSInteger)to.length - (NSInteger)wrong.length;
              [fixes addObject:@[ word, to ]];
          }
          if (!fixes.count) return;
          NSData *json = [NSJSONSerialization dataWithJSONObject:@{@"text" : fixed, @"fixes" : fixes} options:0 error:NULL];
          if (json) out = strndup(json.bytes, json.length);
      }
    });
    *unready = late;
    return out;
}

// A word's entry in macOS's thesaurus (Oxford's: the British one if
// `british`), as text: NULL if it's not in it (malloc'd). (Dictionary
// Services has no public way to pick a dictionary: its own functions,
// looked up.)
char *nf_synonyms(const char *word, bool british) {
    @autoreleasepool {
        static CFArrayRef (*available)(void);
        static CFStringRef (*named)(const void *);
        static dispatch_once_t once;
        dispatch_once(&once, ^{
          void *services = dlopen("/System/Library/Frameworks/CoreServices.framework/Frameworks/DictionaryServices.framework/DictionaryServices", RTLD_NOW);
          if (services) {
              available = dlsym(services, "DCSCopyAvailableDictionaries");
              named = dlsym(services, "DCSDictionaryGetName");
          }
        });
        if (!available || !named) return NULL;
        NSArray *dictionaries = CFBridgingRelease(available());
        NSString *w = @(word);
        // (The one for the Mac's English first, then the other.)
        NSArray<NSString *> *wanted = british ? @[@"Oxford Thesaurus of English", @"American Writer"] : @[@"American Writer", @"Oxford Thesaurus of English"];
        for (NSString *name in wanted) {
            for (id dictionary in dictionaries) {
                NSString *called = (__bridge NSString *)named((__bridge const void *)dictionary);
                if (![called containsString:name]) continue;
                CFStringRef text = DCSCopyTextDefinition((__bridge DCSDictionaryRef)dictionary, (__bridge CFStringRef)w, CFRangeMake(0, (CFIndex)w.length));
                if (text) return strdup([(__bridge_transfer NSString *)text UTF8String]);
            }
        }
        return NULL;
    }
}

// Location, only for the Wi-Fi network's name: macOS gives it to apps
// allowed to know where the Mac is. 1 allowed, 0 not asked yet, -1 no.
static CLLocationManager *locating;

int nf_location_access(void) {
    __block int access = 0;
    on_main(^{
      if (!locating) locating = [CLLocationManager new];
      switch (locating.authorizationStatus) {
          case kCLAuthorizationStatusAuthorizedAlways: access = 1; break;
          case kCLAuthorizationStatusNotDetermined: access = 0; break;
          default: access = -1;
      }
    });
    return access;
}

// Ask for it (macOS asks, the first time).
void nf_location_request(void) {
    dispatch_async(dispatch_get_main_queue(), ^{
      if (!locating) locating = [CLLocationManager new];
      [locating requestWhenInUseAuthorization];
    });
}

// The Wi-Fi network the Mac's on, by name (NULL: none, or not to be told);
// and the Wi-Fi's interface ("en0"), into `device`. Malloc'd.
char *nf_wifi_network(char **device) {
    __block char *out = NULL;
    on_main(^{
      @autoreleasepool {
          CWInterface *wifi = CWWiFiClient.sharedWiFiClient.interface;
          *device = wifi.interfaceName.length ? strdup(wifi.interfaceName.UTF8String) : NULL;
          if (wifi.ssid.length) out = strdup(wifi.ssid.UTF8String);
      }
    });
    return out;
}

// Night Shift, as Control Center turns it (CoreBrightness's, which has no
// public face): 1 on, 0 off, -1 not on this Mac. `set` 0 or 1 turns it off
// or on first (-1: as it is).
typedef struct {
    int hour, minute;
} nf_clock;
typedef struct {
    BOOL active;
    BOOL enabled;
    BOOL sun_schedule_permitted;
    int mode;
    nf_clock from, to;
    unsigned long long disable_flags;
    BOOL available;
} nf_blue_light;

int nf_night_shift(int set) {
    @autoreleasepool {
        static id client;
        static dispatch_once_t once;
        dispatch_once(&once, ^{
          if (dlopen("/System/Library/PrivateFrameworks/CoreBrightness.framework/CoreBrightness", RTLD_NOW)) {
              Class kind = NSClassFromString(@"CBBlueLightClient");
              if (kind) client = [kind new];
          }
        });
        if (!client || ![client respondsToSelector:@selector(getBlueLightStatus:)]) return -1;
        nf_blue_light status = {0};
        BOOL (*get)(id, SEL, nf_blue_light *) = (BOOL (*)(id, SEL, nf_blue_light *))objc_msgSend;
        if (!get(client, @selector(getBlueLightStatus:), &status) || !status.available) return -1;
        if (set >= 0) {
            ((BOOL (*)(id, SEL, BOOL))objc_msgSend)(client, @selector(setEnabled:), set == 1);
            get(client, @selector(getBlueLightStatus:), &status);
        }
        return status.enabled ? 1 : 0;
    }
}

// Whether the desktop shows its icons (Finder's CreateDesktop): 1 yes, 0
// no. `set` 0 or 1 hides or shows them first (Finder restarts to take it).
int nf_desktop_icons(int set) {
    CFStringRef key = CFSTR("CreateDesktop"), finder = CFSTR("com.apple.finder");
    if (set >= 0) {
        CFPreferencesSetAppValue(key, set ? kCFBooleanTrue : kCFBooleanFalse, finder);
        CFPreferencesAppSynchronize(finder);
        for (NSRunningApplication *app in [NSRunningApplication runningApplicationsWithBundleIdentifier:@"com.apple.finder"]) {
            [app terminate];
        }
    }
    CFPreferencesAppSynchronize(finder);
    Boolean valid = false;
    Boolean shown = CFPreferencesGetAppBooleanValue(key, finder, &valid);
    return !valid || shown ? 1 : 0;
}

// Show the desktop, as Mission Control's Show Desktop (F11) does: windows
// aside; again, back. (The Dock's own notification: no public way.)
bool nf_show_desktop(void) {
    void (*send)(CFStringRef, void *) = dlsym(RTLD_DEFAULT, "CoreDockSendNotification");
    if (!send) return false;
    send(CFSTR("com.apple.showdesktop.awake"), NULL);
    return true;
}
