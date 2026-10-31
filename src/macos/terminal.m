// The notch's terminal's look, imported from Terminal's or iTerm2's
// default profile (src/terminal/import.rs): their colors are in their
// preferences, archived (Terminal) or as components (iTerm2), so they're
// read here. And the monospaced fonts there are, for Settings › Terminal,
// and whether one's there (CoreText's, as asked: quick, and on any thread;
// NSFontManager's take a sixth of a second the first time).

#import <AppKit/AppKit.h>
#import <CoreText/CoreText.h>

static NSString *hex(NSColor *color) {
    NSColor *c = [color colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
    if (!c) return nil;
    int r = (int)lround(fmin(fmax(c.redComponent, 0), 1) * 255), g = (int)lround(fmin(fmax(c.greenComponent, 0), 1) * 255),
        b = (int)lround(fmin(fmax(c.blueComponent, 0), 1) * 255);
    return [NSString stringWithFormat:@"#%02x%02x%02x", r, g, b];
}

static id unarchived(id data) {
    if (![data isKindOfClass:NSData.class]) return nil;
    NSKeyedUnarchiver *u = [[NSKeyedUnarchiver alloc] initForReadingFromData:data error:nil];
    if (!u) return nil;
    u.requiresSecureCoding = NO;
    id object = [u decodeObjectForKey:NSKeyedArchiveRootObjectKey];
    [u finishDecoding];
    return object;
}

static char *json(NSDictionary *profile) {
    NSData *data = [NSJSONSerialization dataWithJSONObject:profile options:0 error:nil];
    if (!data) return NULL;
    return strdup([[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding].UTF8String);
}

static id preference(NSString *key, NSString *app) {
    return CFBridgingRelease(CFPreferencesCopyAppValue((__bridge CFStringRef)key, (__bridge CFStringRef)app));
}

// Terminal's default profile ("Default Window Settings"), its colors where
// it has them, else Terminal's own (as Basic has them).
static char *terminal_app(void) {
    NSString *name = preference(@"Default Window Settings", @"com.apple.Terminal") ?: @"Basic";
    NSDictionary *profiles = preference(@"Window Settings", @"com.apple.Terminal");
    NSDictionary *p = [profiles isKindOfClass:NSDictionary.class] ? profiles[name] : nil;
    if (![p isKindOfClass:NSDictionary.class]) p = @{};
    NSArray *ansi = @[ @"Black", @"Red", @"Green", @"Yellow", @"Blue", @"Magenta", @"Cyan", @"White" ];
    NSArray *own = @[
        @"#000000", @"#c23621", @"#25bc24", @"#adad27", @"#492ee1", @"#d338d3", @"#33bbc8", @"#cbcccd",
        @"#818383", @"#fc391f", @"#31e722", @"#eaec23", @"#5833ff", @"#f935f8", @"#14f0f0", @"#e9ebeb",
    ];
    NSMutableDictionary *colors = [NSMutableDictionary dictionary];
    NSColor *background = unarchived(p[@"BackgroundColor"]);
    colors[@"background"] = hex(background) ?: @"#ffffff";
    colors[@"foreground"] = hex(unarchived(p[@"TextColor"])) ?: @"#000000";
    NSString *cursor = hex(unarchived(p[@"CursorColor"]));
    if (cursor) colors[@"cursor"] = cursor;
    NSString *selection = hex(unarchived(p[@"SelectionColor"]));
    if (selection) colors[@"selection"] = selection;
    for (NSUInteger i = 0; i < 16; i++) {
        NSString *key = [NSString stringWithFormat:@"ANSI%@%@Color", i < 8 ? @"" : @"Bright", ansi[i % 8]];
        colors[[NSString stringWithFormat:@"color%lu", (unsigned long)i]] = hex(unarchived(p[key])) ?: own[i];
    }
    NSMutableDictionary *out = [@{@"name" : name, @"colors" : colors} mutableCopy];
    // (SF Mono, Terminal's own, comes back as the system's font, hidden
    // ("."): there's no family to name for it, only its size.)
    NSFont *font = unarchived(p[@"Font"]);
    BOOL font_ok = [font isKindOfClass:NSFont.class];
    if (font_ok && ![font.familyName hasPrefix:@"."]) out[@"font_family"] = font.familyName;
    out[@"font_size"] = font_ok ? @(font.pointSize) : @11;
    if (background) out[@"opacity"] = @([background colorUsingColorSpace:NSColorSpace.sRGBColorSpace].alphaComponent);
    NSNumber *type = p[@"CursorType"];
    out[@"cursor"] = @[ @"block", @"underline", @"bar" ][MIN(type.unsignedIntegerValue, 2u)];
    if ([p[@"CursorBlink"] isKindOfClass:NSNumber.class]) out[@"blink"] = p[@"CursorBlink"];
    return json(out);
}

// One of iTerm2's colors: its components, in the color space it names (its
// "(Dark)" one, if the profile has its own for dark mode).
static NSString *iterm_color(NSDictionary *p, NSString *key, BOOL dark) {
    NSDictionary *c = dark && p[[key stringByAppendingString:@" (Dark)"]] ? p[[key stringByAppendingString:@" (Dark)"]] : p[key];
    if (![c isKindOfClass:NSDictionary.class]) return nil;
    CGFloat r = [c[@"Red Component"] doubleValue], g = [c[@"Green Component"] doubleValue], b = [c[@"Blue Component"] doubleValue];
    NSString *space = c[@"Color Space"];
    NSColor *color = [space isEqual:@"sRGB"] ? [NSColor colorWithSRGBRed:r green:g blue:b alpha:1]
                     : [space isEqual:@"P3"] ? [NSColor colorWithDisplayP3Red:r green:g blue:b alpha:1]
                                             : [NSColor colorWithCalibratedRed:r green:g blue:b alpha:1];
    return hex(color);
}

// iTerm2's default profile ("Default Bookmark Guid"), else its first.
static char *iterm(void) {
    NSArray *profiles = preference(@"New Bookmarks", @"com.googlecode.iterm2");
    if (![profiles isKindOfClass:NSArray.class] || profiles.count == 0) return NULL;
    NSString *guid = preference(@"Default Bookmark Guid", @"com.googlecode.iterm2");
    NSDictionary *p = profiles[0];
    for (NSDictionary *each in profiles) {
        if ([each isKindOfClass:NSDictionary.class] && [each[@"Guid"] isEqual:guid]) p = each;
    }
    if (![p isKindOfClass:NSDictionary.class]) return NULL;
    BOOL dark = [p[@"Use Separate Colors for Light and Dark Mode"] boolValue];
    NSMutableDictionary *colors = [NSMutableDictionary dictionary];
    NSDictionary *keys = @{
        @"background" : @"Background Color",
        @"foreground" : @"Foreground Color",
        @"cursor" : @"Cursor Color",
        @"cursor_text" : @"Cursor Text Color",
        @"selection" : @"Selection Color",
        @"selected_text" : @"Selected Text Color",
    };
    for (NSString *key in keys) {
        NSString *c = iterm_color(p, keys[key], dark);
        if (c) colors[key] = c;
    }
    for (int i = 0; i < 16; i++) {
        NSString *c = iterm_color(p, [NSString stringWithFormat:@"Ansi %d Color", i], dark);
        if (c) colors[[NSString stringWithFormat:@"color%d", i]] = c;
    }
    NSMutableDictionary *out = [@{@"name" : p[@"Name"] ?: @"Default", @"colors" : colors} mutableCopy];
    // ("JetBrainsMono-Regular 13": a font's PostScript name, and its size.)
    NSString *normal = p[@"Normal Font"];
    NSRange space = [normal isKindOfClass:NSString.class] ? [normal rangeOfString:@" " options:NSBackwardsSearch] : NSMakeRange(NSNotFound, 0);
    if (space.location != NSNotFound) {
        CGFloat size = [[normal substringFromIndex:space.location + 1] doubleValue];
        NSFont *font = [NSFont fontWithName:[normal substringToIndex:space.location] size:size > 0 ? size : 12];
        if (font && ![font.familyName hasPrefix:@"."]) out[@"font_family"] = font.familyName;
        if (size > 0) out[@"font_size"] = @(size);
    }
    if ([p[@"Transparency"] isKindOfClass:NSNumber.class]) out[@"opacity"] = @(1 - [p[@"Transparency"] doubleValue]);
    if ([p[@"Cursor Type"] isKindOfClass:NSNumber.class]) out[@"cursor"] = @[ @"underline", @"bar", @"block" ][MIN([p[@"Cursor Type"] unsignedIntegerValue], 2u)];
    if ([p[@"Blinking Cursor"] isKindOfClass:NSNumber.class]) out[@"blink"] = p[@"Blinking Cursor"];
    return json(out);
}

// Terminal's (or iTerm2's) default profile's look, as JSON: its name, its
// colors ("background", "color0"…, as "#rrggbb"), font_family, font_size,
// opacity, cursor ("block", "bar", "underline") and blink. NULL if there's
// none.
char *nf_terminal_look(bool iterm2) {
    @autoreleasepool {
        return iterm2 ? iterm() : terminal_app();
    }
}

// The monospaced fonts' families, a line each.
char *nf_monospace_fonts(void) {
    @autoreleasepool {
        NSDictionary *mono = @{(id)kCTFontTraitsAttribute : @{(id)kCTFontSymbolicTrait : @(kCTFontTraitMonoSpace)}};
        CTFontDescriptorRef wanted = CTFontDescriptorCreateWithAttributes((__bridge CFDictionaryRef)mono);
        CTFontCollectionRef collection = CTFontCollectionCreateWithFontDescriptors((__bridge CFArrayRef) @[ (__bridge id)wanted ], NULL);
        CFRelease(wanted);
        NSArray *found = CFBridgingRelease(CTFontCollectionCreateMatchingFontDescriptors(collection));
        CFRelease(collection);
        NSMutableSet *families = [NSMutableSet set];
        for (id descriptor in found) {
            NSString *family = CFBridgingRelease(CTFontDescriptorCopyAttribute((__bridge CTFontDescriptorRef)descriptor, kCTFontFamilyNameAttribute));
            if (family.length && ![family hasPrefix:@"."]) [families addObject:family];
        }
        NSArray *sorted = [families.allObjects sortedArrayUsingSelector:@selector(localizedCaseInsensitiveCompare:)];
        return strdup([sorted componentsJoinedByString:@"\n"].UTF8String);
    }
}

// Whether a font family of that name is there.
bool nf_has_font_family(const char *name) {
    @autoreleasepool {
        NSString *family = @(name);
        if (!family) return false;
        CTFontDescriptorRef wanted = CTFontDescriptorCreateWithAttributes((__bridge CFDictionaryRef) @{(id)kCTFontFamilyNameAttribute : family});
        CTFontDescriptorRef found = CTFontDescriptorCreateMatchingFontDescriptor(wanted, (__bridge CFSetRef)[NSSet setWithObject:(id)kCTFontFamilyNameAttribute]);
        CFRelease(wanted);
        if (found) CFRelease(found);
        return found != NULL;
    }
}

// Whether a mouse button's held (a drag, maybe, toward the terminal).
bool nf_mouse_pressed(void) {
    return NSEvent.pressedMouseButtons != 0;
}
