// What Finder takes from the system: its colors (following the accent
// color and light or dark), SF Symbols, a type's "Kind", and free space.

#import <AppKit/AppKit.h>
#import <UniformTypeIdentifiers/UniformTypeIdentifiers.h>

static void put(NSColor *color, float *out) {
    NSColor *c = [color colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
    out[0] = c.redComponent;
    out[1] = c.greenComponent;
    out[2] = c.blueComponent;
    out[3] = c.alphaComponent;
}

// Named colors as sRGB RGBA, four floats each, in this order: selected
// content background, unemphasized selected content background, label,
// secondary label, tertiary label, quaternary label, separator, the second
// alternating row background, accent.
void nf_system_colors(bool dark, float *out) {
    @autoreleasepool {
        NSAppearance *appearance = [NSAppearance appearanceNamed:dark ? NSAppearanceNameDarkAqua : NSAppearanceNameAqua];
        [appearance performAsCurrentDrawingAppearance:^{
          NSArray<NSColor *> *colors = @[
              NSColor.selectedContentBackgroundColor, NSColor.unemphasizedSelectedContentBackgroundColor,
              NSColor.labelColor, NSColor.secondaryLabelColor, NSColor.tertiaryLabelColor,
              NSColor.quaternaryLabelColor, NSColor.separatorColor,
              NSColor.alternatingContentBackgroundColors[1], NSColor.controlAccentColor,
          ];
          for (NSUInteger i = 0; i < colors.count; i++) put(colors[i], out + i * 4);
        }];
    }
}

// An SF Symbol `pt` points tall at `scale` pixels per point, in one color:
// premultiplied BGRA, `w` by `h`, malloc'd. NULL if there's no such symbol.
uint8_t *nf_symbol(const char *name, float pt, float weight, float scale, const float *rgba, uint32_t *w, uint32_t *h) {
    @autoreleasepool {
        NSImage *base = [NSImage imageWithSystemSymbolName:@(name) accessibilityDescription:nil];
        NSImageSymbolConfiguration *config = [NSImageSymbolConfiguration configurationWithPointSize:pt
                                                                                             weight:weight
                                                                                              scale:NSImageSymbolScaleMedium];
        NSImage *image = [base imageWithSymbolConfiguration:config];
        if (!image) return NULL;
        *w = (uint32_t)ceil(image.size.width * scale);
        *h = (uint32_t)ceil(image.size.height * scale);
        uint8_t *data = calloc((size_t)*w * *h, 4);
        CGColorSpaceRef srgb = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
        CGContextRef ctx = CGBitmapContextCreate(data, *w, *h, 8, (size_t)*w * 4, srgb,
                                                 kCGImageAlphaPremultipliedFirst | kCGBitmapByteOrder32Little);
        CGColorSpaceRelease(srgb);
        if (!ctx) {
            free(data);
            return NULL;
        }
        [NSGraphicsContext saveGraphicsState];
        NSGraphicsContext.currentContext = [NSGraphicsContext graphicsContextWithCGContext:ctx flipped:NO];
        [image drawInRect:NSMakeRect(0, 0, *w, *h)];
        [NSGraphicsContext restoreGraphicsState];
        // Keep the symbol's shape, take the color.
        CGContextSetBlendMode(ctx, kCGBlendModeSourceIn);
        CGContextSetRGBFillColor(ctx, rgba[0], rgba[1], rgba[2], rgba[3]);
        CGContextFillRect(ctx, CGRectMake(0, 0, *w, *h));
        CGContextRelease(ctx);
        return data;
    }
}

// Finder's "Kind" for files with this extension ("Markdown text file"), or
// for a folder or app: malloc'd UTF-8, or NULL if the system has no name.
char *nf_kind(const char *ext, bool is_dir) {
    @autoreleasepool {
        // A folder's extension only names a type if it's a package (an app).
        UTType *type = is_dir ? [UTType typeWithFilenameExtension:@(ext) conformingToType:UTTypeDirectory]
                              : [UTType typeWithFilenameExtension:@(ext)];
        if (is_dir && ![type conformsToType:UTTypePackage]) type = UTTypeFolder;
        NSString *name = type.localizedDescription;
        if (!name.length) return NULL;
        // As Finder shows it: "Folder", not "folder".
        name = [[name substringToIndex:1].uppercaseString stringByAppendingString:[name substringFromIndex:1]];
        return strdup(name.UTF8String);
    }
}

// This Mac's currency, as its region has it ("KRW"), or NULL (malloc'd).
char *nf_locale_currency(void) {
    @autoreleasepool {
        NSString *code = NSLocale.currentLocale.currencyCode;
        return code.length ? strdup(code.UTF8String) : NULL;
    }
}

// Whether the first language is British English.
bool nf_prefers_british(void) {
    @autoreleasepool {
        return [NSLocale.preferredLanguages.firstObject hasPrefix:@"en-GB"];
    }
}

// Whether folders with this extension are packages (apps, .rtfd,
// .photoslibrary...): opened as one item, not browsed.
bool nf_is_package(const char *ext) {
    @autoreleasepool {
        UTType *type = [UTType typeWithFilenameExtension:@(ext) conformingToType:UTTypeDirectory];
        return [type conformsToType:UTTypePackage];
    }
}

// Space available on the volume holding `path`, as Finder counts it
// (including purgeable space). 0 if unknown.
uint64_t nf_available_bytes(const char *path) {
    @autoreleasepool {
        NSNumber *bytes = nil;
        [[NSURL fileURLWithPath:@(path)] getResourceValue:&bytes forKey:NSURLVolumeAvailableCapacityForImportantUsageKey error:nil];
        return bytes.unsignedLongLongValue;
    }
}

// The Emoji & Symbols viewer, as Edit > Emoji & Symbols opens it: what's
// picked goes into what has the keyboard.
void nf_emoji_palette(void) {
    [NSApp orderFrontCharacterPalette:nil];
}
