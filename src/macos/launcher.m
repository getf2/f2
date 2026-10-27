// What Quick Open's commands ask of the system: the apps running (to quit
// them), and locking the screen.

#import <AppKit/AppKit.h>
#import <Carbon/Carbon.h>
#import <dlfcn.h>

// The languages this Mac reads or types: the ones it's set to, then those
// of the keyboards in its input menu ("en,ko,is,ja"), malloc'd. Looked up
// on the main thread only (Text Input Sources traps off it: F2 crashed),
// and kept: elsewhere, it's what was last looked up.
static NSString *mac_languages = nil;

char *nf_mac_languages(void) {
    @autoreleasepool {
        if (NSThread.isMainThread) {
            NSMutableOrderedSet<NSString *> *languages = [NSMutableOrderedSet orderedSetWithArray:NSLocale.preferredLanguages];
            NSDictionary *enabled = @{(__bridge NSString *)kTISPropertyInputSourceIsEnabled: @YES,
                                      (__bridge NSString *)kTISPropertyInputSourceCategory: (__bridge NSString *)kTISCategoryKeyboardInputSource};
            NSArray *sources = CFBridgingRelease(TISCreateInputSourceList((__bridge CFDictionaryRef)enabled, false));
            for (id source in sources) {
                NSArray *of = (__bridge NSArray *)TISGetInputSourceProperty((__bridge TISInputSourceRef)source, kTISPropertyInputSourceLanguages);
                if (of.count) [languages addObject:of.firstObject];
            }
            NSString *joined = [languages.array componentsJoinedByString:@","];
            @synchronized(NSLocale.class) {
                mac_languages = joined;
            }
        }
        NSString *known;
        @synchronized(NSLocale.class) {
            known = mac_languages ?: [NSLocale.preferredLanguages componentsJoinedByString:@","];
        }
        return strdup(known.UTF8String);
    }
}

static NSString *language_of(NSString *code) {
    return [NSLocale localeWithLocaleIdentifier:code].languageCode ?: code;
}

// A .strings file's (or plist's) dictionary.
static NSDictionary *strings_at(NSString *path) {
    NSDictionary *plist = [NSDictionary dictionaryWithContentsOfFile:path];
    if (plist) return plist;
    NSString *text = [NSString stringWithContentsOfFile:path usedEncoding:nil error:nil];
    // (It throws on a file it can't read: an app's own, mistyped.)
    id parsed = nil;
    @try {
        parsed = text.propertyListFromStringsFileFormat;
    } @catch (NSException *__unused e) {
        return nil;
    }
    return [parsed isKindOfClass:NSDictionary.class] ? parsed : nil;
}

// What Finder calls an app ("メモ" on a Mac in Japanese, "Notes" in
// English; without ".app"), and what it calls itself in `languages`
// (comma-separated), for those it has: a name a line, malloc'd. (Its
// InfoPlist.loctable, as Apple's apps have, or its .lproj folders'
// InfoPlist.strings.)
char *nf_app_names(const char *app, const char *languages) {
    @autoreleasepool {
        NSString *path = @(app);
        NSMutableOrderedSet<NSString *> *names = [NSMutableOrderedSet orderedSet];
        NSString *shown = [NSFileManager.defaultManager displayNameAtPath:path];
        if ([shown.pathExtension isEqualToString:@"app"]) shown = shown.stringByDeletingPathExtension;
        if (shown.length) [names addObject:shown];
        NSString *resources = [path stringByAppendingPathComponent:@"Contents/Resources"];
        NSDictionary *table = [NSDictionary dictionaryWithContentsOfFile:[resources stringByAppendingPathComponent:@"InfoPlist.loctable"]];
        NSArray<NSString *> *have = table.allKeys;
        if (!table) {
            NSMutableArray *folders = [NSMutableArray array];
            for (NSString *name in [NSFileManager.defaultManager contentsOfDirectoryAtPath:resources error:nil]) {
                if ([name.pathExtension isEqualToString:@"lproj"]) [folders addObject:name.stringByDeletingPathExtension];
            }
            have = folders;
        }
        if (!have.count) return strdup([names.array componentsJoinedByString:@"\n"].UTF8String);
        for (NSString *language in [@(languages) componentsSeparatedByString:@","]) {
            NSString *localization = [NSBundle preferredLocalizationsFromArray:have forPreferences:@[language]].firstObject;
            // (Its best for a language it doesn't have is another's.)
            if (!localization || ![language_of(localization) isEqualToString:language_of(language)]) continue;
            NSDictionary *strings = table ? table[localization] : strings_at([resources stringByAppendingFormat:@"/%@.lproj/InfoPlist.strings", localization]);
            if (![strings isKindOfClass:NSDictionary.class]) continue;
            NSString *name = strings[@"CFBundleDisplayName"] ?: strings[@"CFBundleName"];
            if ([name isKindOfClass:NSString.class] && name.length) [names addObject:name];
        }
        return strdup([names.array componentsJoinedByString:@"\n"].UTF8String);
    }
}

// The apps running with a Dock icon (not this one): "pid\tname\tpath" a
// line each, malloc'd.
char *nf_running_apps(void) {
    @autoreleasepool {
        NSMutableString *out = [NSMutableString string];
        pid_t me = NSRunningApplication.currentApplication.processIdentifier;
        for (NSRunningApplication *app in NSWorkspace.sharedWorkspace.runningApplications) {
            if (app.activationPolicy != NSApplicationActivationPolicyRegular || app.processIdentifier == me || !app.bundleURL) {
                continue;
            }
            [out appendFormat:@"%d\t%@\t%@\n", app.processIdentifier, app.localizedName ?: @"", app.bundleURL.path];
        }
        return strdup(out.UTF8String);
    }
}

// Ask an app to quit (it may ask to save first), or with `force` end it.
bool nf_quit_app(int32_t pid, bool force) {
    @autoreleasepool {
        NSRunningApplication *app = [NSRunningApplication runningApplicationWithProcessIdentifier:pid];
        return app && (force ? [app forceTerminate] : [app terminate]);
    }
}

// Hide an app, as ⌘H does.
bool nf_hide_app(int32_t pid) {
    @autoreleasepool {
        NSRunningApplication *app = [NSRunningApplication runningApplicationWithProcessIdentifier:pid];
        return app && [app hide];
    }
}

// Lock the screen as Control Center's Lock Screen does (the login
// framework's own call; there's no public one).
bool nf_lock_screen(void) {
    void *login = dlopen("/System/Library/PrivateFrameworks/login.framework/Versions/Current/login", RTLD_LAZY);
    if (!login) {
        return false;
    }
    void (*lock)(void) = (void (*)(void))dlsym(login, "SACLockScreenImmediate");
    if (!lock) {
        return false;
    }
    lock();
    return true;
}
