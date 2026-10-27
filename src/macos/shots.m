// Screenshots: the permission to take them, the clipboard, the text in
// them (Vision), the color under the pointer, and drawing an edited one
// (its marks, crop and background) into a PNG.

#import <AppKit/AppKit.h>
#import <CoreImage/CoreImage.h>
#import <Vision/Vision.h>

bool nf_capture_allowed(void) {
    return CGPreflightScreenCaptureAccess();
}

// Ask for it (macOS asks once; after that, only System Settings can).
bool nf_capture_request(void) {
    return CGRequestScreenCaptureAccess();
}

// An image's size in pixels, and its pixels per point (2 for a Retina
// screenshot: its resolution says so).
bool nf_image_info(const char *path, uint32_t *w, uint32_t *h, double *scale) {
    @autoreleasepool {
        NSImageRep *rep = [NSImageRep imageRepWithContentsOfFile:@(path)];
        if (!rep || rep.pixelsWide <= 0) {
            return false;
        }
        *w = (uint32_t)rep.pixelsWide;
        *h = (uint32_t)rep.pixelsHigh;
        *scale = rep.size.width > 0 ? rep.pixelsWide / rep.size.width : 1;
        return true;
    }
}

// The image on the pasteboard, as PNG (and TIFF, for older apps).
bool nf_copy_image(const char *path) {
    @autoreleasepool {
        NSData *png = [NSData dataWithContentsOfFile:@(path)];
        NSImage *image = png ? [[NSImage alloc] initWithData:png] : nil;
        if (!image) {
            return false;
        }
        NSPasteboard *pb = NSPasteboard.generalPasteboard;
        [pb clearContents];
        NSPasteboardItem *item = [NSPasteboardItem new];
        [item setData:png forType:NSPasteboardTypePNG];
        NSData *tiff = image.TIFFRepresentation;
        if (tiff) {
            [item setData:tiff forType:NSPasteboardTypeTIFF];
        }
        return [pb writeObjects:@[item]];
    }
}

// The text Vision reads in what `handler` holds, a line each, top to
// bottom (nil if none). (images.m reads PDFs' pages with it too.)
NSString *nf_text_in(VNImageRequestHandler *handler) {
    @autoreleasepool {
        VNRecognizeTextRequest *request = [VNRecognizeTextRequest new];
        request.recognitionLevel = VNRequestTextRecognitionLevelAccurate;
        request.usesLanguageCorrection = YES;
        if (@available(macOS 13, *)) {
            request.automaticallyDetectsLanguage = YES;
        }
        if (![handler performRequests:@[request] error:nil]) {
            return nil;
        }
        // Top to bottom, then left to right (Vision's y goes up).
        NSArray<VNRecognizedTextObservation *> *found = [request.results sortedArrayUsingComparator:^NSComparisonResult(VNRecognizedTextObservation *a, VNRecognizedTextObservation *b) {
            CGFloat dy = CGRectGetMidY(b.boundingBox) - CGRectGetMidY(a.boundingBox);
            if (fabs(dy) > MIN(a.boundingBox.size.height, b.boundingBox.size.height) / 2) {
                return dy > 0 ? NSOrderedDescending : NSOrderedAscending;
            }
            return a.boundingBox.origin.x < b.boundingBox.origin.x ? NSOrderedAscending : NSOrderedDescending;
        }];
        NSMutableArray<NSString *> *lines = [NSMutableArray array];
        for (VNRecognizedTextObservation *o in found) {
            VNRecognizedText *best = [o topCandidates:1].firstObject;
            if (best.string.length) {
                [lines addObject:best.string];
            }
        }
        return lines.count ? [lines componentsJoinedByString:@"\n"] : nil;
    }
}

// The text in an image, a line each (NULL if none), malloc'd.
char *nf_recognize_text(const char *path) {
    @autoreleasepool {
        VNImageRequestHandler *handler = [[VNImageRequestHandler alloc] initWithURL:[NSURL fileURLWithPath:@(path)] options:@{}];
        NSString *text = nf_text_in(handler);
        return text ? strdup(text.UTF8String) : NULL;
    }
}

static NSColorSampler *sampler;

// macOS's magnifier, to pick a color anywhere on screen: `picked` gets it
// as "#RRGGBB" (or NULL if put away), on the main thread.
void nf_pick_color(void (*picked)(const char *hex)) {
    dispatch_async(dispatch_get_main_queue(), ^{
        sampler = [NSColorSampler new];
        [sampler showSamplerWithSelectionHandler:^(NSColor *color) {
            NSColor *rgb = [color colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
            if (!rgb) {
                picked(NULL);
            } else {
                int r = (int)lround(rgb.redComponent * 255), g = (int)lround(rgb.greenComponent * 255), b = (int)lround(rgb.blueComponent * 255);
                picked([NSString stringWithFormat:@"#%02X%02X%02X", r, g, b].UTF8String);
            }
            sampler = nil;
        }];
    });
}

static CGImageRef load_image(NSString *path) {
    NSData *data = [NSData dataWithContentsOfFile:path];
    if (!data) {
        return NULL;
    }
    CGImageSourceRef source = CGImageSourceCreateWithData((__bridge CFDataRef)data, NULL);
    if (!source) {
        return NULL;
    }
    CGImageRef image = CGImageSourceCreateImageAtIndex(source, 0, NULL);
    CFRelease(source);
    return image;
}

// `image` in big squares of `block` pixels (the blur tool's look).
static CGImageRef pixelated(CGImageRef image, double block) {
    CIImage *input = [CIImage imageWithCGImage:image];
    CIFilter *filter = [CIFilter filterWithName:@"CIPixellate"];
    [filter setValue:[input imageByClampingToExtent] forKey:kCIInputImageKey];
    [filter setValue:@(MAX(block, 2)) forKey:kCIInputScaleKey];
    [filter setValue:[CIVector vectorWithX:0 Y:0] forKey:kCIInputCenterKey];
    CIImage *output = [filter.outputImage imageByCroppingToRect:input.extent];
    CIContext *context = [CIContext contextWithOptions:nil];
    return [context createCGImage:output fromRect:input.extent];
}

static bool write_png(CGImageRef image, NSString *path, double scale) {
    NSURL *url = [NSURL fileURLWithPath:path];
    CGImageDestinationRef dest = CGImageDestinationCreateWithURL((__bridge CFURLRef)url, CFSTR("public.png"), 1, NULL);
    if (!dest) {
        return false;
    }
    // (Its resolution: 144 dpi for a Retina one, as screenshots are.)
    NSDictionary *props = @{(id)kCGImagePropertyDPIWidth: @(72 * scale), (id)kCGImagePropertyDPIHeight: @(72 * scale)};
    CGImageDestinationAddImage(dest, image, (__bridge CFDictionaryRef)props);
    bool ok = CGImageDestinationFinalize(dest);
    CFRelease(dest);
    return ok;
}

// The image, pixelated all over, for the editor to show the blur tool's
// marks with.
bool nf_pixelate_file(const char *in, const char *out, double block) {
    @autoreleasepool {
        CGImageRef image = load_image(@(in));
        if (!image) {
            return false;
        }
        CGImageRef squares = pixelated(image, block);
        CGImageRelease(image);
        if (!squares) {
            return false;
        }
        bool ok = write_png(squares, @(out), 1);
        CGImageRelease(squares);
        return ok;
    }
}

static NSColor *color_of(NSString *hex, double alpha) {
    unsigned value = 0;
    [[NSScanner scannerWithString:[hex stringByReplacingOccurrencesOfString:@"#" withString:@""]] scanHexInt:&value];
    return [NSColor colorWithSRGBRed:((value >> 16) & 0xff) / 255.0 green:((value >> 8) & 0xff) / 255.0 blue:(value & 0xff) / 255.0 alpha:alpha];
}

static CGContextRef bitmap(size_t w, size_t h) {
    CGColorSpaceRef space = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
    CGContextRef ctx = CGBitmapContextCreate(NULL, w, h, 8, 0, space, (CGBitmapInfo)kCGImageAlphaPremultipliedLast);
    CGColorSpaceRelease(space);
    // Top-left origin, y down, as the marks are given.
    CGContextTranslateCTM(ctx, 0, h);
    CGContextScaleCTM(ctx, 1, -1);
    return ctx;
}

static void add_points(CGContextRef ctx, NSArray<NSNumber *> *points, bool closed) {
    for (NSUInteger i = 0; i + 1 < points.count; i += 2) {
        CGFloat x = points[i].doubleValue, y = points[i + 1].doubleValue;
        if (i == 0) {
            CGContextMoveToPoint(ctx, x, y);
        } else {
            CGContextAddLineToPoint(ctx, x, y);
        }
    }
    if (closed) {
        CGContextClosePath(ctx);
    }
}

static void draw_text(CGContextRef ctx, NSDictionary *p) {
    NSGraphicsContext *previous = NSGraphicsContext.currentContext;
    NSGraphicsContext.currentContext = [NSGraphicsContext graphicsContextWithCGContext:ctx flipped:YES];
    CGFloat size = [p[@"size"] doubleValue];
    NSFont *font = [NSFont systemFontOfSize:size weight:[p[@"bold"] boolValue] ? NSFontWeightBold : NSFontWeightSemibold];
    NSDictionary *attrs = @{NSFontAttributeName: font, NSForegroundColorAttributeName: color_of(p[@"color"], 1)};
    NSString *text = p[@"text"];
    CGFloat x = [p[@"x"] doubleValue], y = [p[@"y"] doubleValue];
    if ([p[@"center"] boolValue]) {
        NSSize measured = [text sizeWithAttributes:attrs];
        x -= measured.width / 2;
        y -= measured.height / 2;
    }
    // (Each line its own: drawAtPoint keeps the newlines.)
    [text drawAtPoint:NSMakePoint(x, y) withAttributes:attrs];
    NSGraphicsContext.currentContext = previous;
}

// Draw an edited screenshot as `spec` (JSON) says: "image", "out",
// "scale", "pixel" (the blur's squares), "marks" (in the image's pixels,
// y down: "stroke" and "fill" with "points" [x, y, x, y…], "disc", "text",
// "pixelate" with "rect"), "crop" [x, y, w, h] or null, and "background"
// ({"from", "to", "padding", "radius", "shadow"}) or null. NULL if it
// worked, else why not (malloc'd).
char *nf_render_shot(const char *spec_json) {
    @autoreleasepool {
        NSData *json = [NSData dataWithBytes:spec_json length:strlen(spec_json)];
        NSDictionary *spec = [NSJSONSerialization JSONObjectWithData:json options:0 error:nil];
        if (![spec isKindOfClass:NSDictionary.class]) {
            return strdup("bad spec");
        }
        CGImageRef image = load_image(spec[@"image"]);
        if (!image) {
            return strdup("couldn't read the screenshot");
        }
        size_t w = CGImageGetWidth(image), h = CGImageGetHeight(image);
        CGContextRef ctx = bitmap(w, h);
        // (Flipped: draw the image upright.)
        CGContextSaveGState(ctx);
        CGContextTranslateCTM(ctx, 0, h);
        CGContextScaleCTM(ctx, 1, -1);
        CGContextDrawImage(ctx, CGRectMake(0, 0, w, h), image);
        CGContextRestoreGState(ctx);
        CGImageRef squares = NULL;
        for (NSDictionary *p in spec[@"marks"]) {
            NSString *kind = p[@"kind"];
            CGContextSaveGState(ctx);
            if ([kind isEqualToString:@"stroke"]) {
                CGContextSetStrokeColorWithColor(ctx, color_of(p[@"color"], [p[@"alpha"] doubleValue]).CGColor);
                CGContextSetLineWidth(ctx, [p[@"width"] doubleValue]);
                CGContextSetLineCap(ctx, kCGLineCapRound);
                CGContextSetLineJoin(ctx, kCGLineJoinRound);
                add_points(ctx, p[@"points"], [p[@"closed"] boolValue]);
                CGContextStrokePath(ctx);
            } else if ([kind isEqualToString:@"fill"]) {
                CGContextSetFillColorWithColor(ctx, color_of(p[@"color"], [p[@"alpha"] doubleValue]).CGColor);
                add_points(ctx, p[@"points"], true);
                CGContextFillPath(ctx);
            } else if ([kind isEqualToString:@"disc"]) {
                CGFloat r = [p[@"r"] doubleValue];
                CGContextSetFillColorWithColor(ctx, color_of(p[@"color"], 1).CGColor);
                CGContextFillEllipseInRect(ctx, CGRectMake([p[@"x"] doubleValue] - r, [p[@"y"] doubleValue] - r, 2 * r, 2 * r));
            } else if ([kind isEqualToString:@"text"]) {
                draw_text(ctx, p);
            } else if ([kind isEqualToString:@"pixelate"]) {
                if (!squares) {
                    squares = pixelated(image, [spec[@"pixel"] doubleValue]);
                }
                NSArray<NSNumber *> *r = p[@"rect"];
                if (squares && r.count == 4) {
                    CGContextClipToRect(ctx, CGRectMake(r[0].doubleValue, r[1].doubleValue, r[2].doubleValue, r[3].doubleValue));
                    CGContextTranslateCTM(ctx, 0, h);
                    CGContextScaleCTM(ctx, 1, -1);
                    CGContextDrawImage(ctx, CGRectMake(0, 0, w, h), squares);
                }
            }
            CGContextRestoreGState(ctx);
        }
        CGImageRef marked = CGBitmapContextCreateImage(ctx);
        CGContextRelease(ctx);
        if (squares) {
            CGImageRelease(squares);
        }
        CGImageRelease(image);
        NSArray<NSNumber *> *crop = spec[@"crop"];
        if ([crop isKindOfClass:NSArray.class] && crop.count == 4) {
            CGImageRef cropped = CGImageCreateWithImageInRect(marked, CGRectMake(crop[0].doubleValue, crop[1].doubleValue, crop[2].doubleValue, crop[3].doubleValue));
            if (cropped) {
                CGImageRelease(marked);
                marked = cropped;
            }
        }
        NSDictionary *bg = spec[@"background"];
        if ([bg isKindOfClass:NSDictionary.class]) {
            size_t cw = CGImageGetWidth(marked), ch = CGImageGetHeight(marked);
            CGFloat pad = [bg[@"padding"] doubleValue], radius = [bg[@"radius"] doubleValue];
            size_t W = cw + 2 * (size_t)pad, H = ch + 2 * (size_t)pad;
            CGContextRef c = bitmap(W, H);
            // The gradient, from the top left to the bottom right.
            CGColorSpaceRef space = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
            NSArray *colors = @[(id)color_of(bg[@"from"], 1).CGColor, (id)color_of(bg[@"to"], 1).CGColor];
            CGGradientRef gradient = CGGradientCreateWithColors(space, (__bridge CFArrayRef)colors, NULL);
            CGContextDrawLinearGradient(c, gradient, CGPointMake(0, 0), CGPointMake(W, H), 0);
            CGGradientRelease(gradient);
            CGColorSpaceRelease(space);
            CGRect frame = CGRectMake(pad, pad, cw, ch);
            CGPathRef rounded = CGPathCreateWithRoundedRect(frame, MIN(radius, cw / 2.0), MIN(radius, ch / 2.0), NULL);
            if ([bg[@"shadow"] boolValue]) {
                CGContextSaveGState(c);
                // (y down here: a positive offset drops it below.)
                CGContextSetShadowWithColor(c, CGSizeMake(0, -pad * 0.12), pad * 0.45, [NSColor colorWithWhite:0 alpha:0.38].CGColor);
                CGContextAddPath(c, rounded);
                CGContextSetFillColorWithColor(c, NSColor.blackColor.CGColor);
                CGContextFillPath(c);
                CGContextRestoreGState(c);
            }
            CGContextSaveGState(c);
            CGContextAddPath(c, rounded);
            CGContextClip(c);
            CGContextTranslateCTM(c, 0, pad * 2 + ch);
            CGContextScaleCTM(c, 1, -1);
            CGContextDrawImage(c, frame, marked);
            CGContextRestoreGState(c);
            CGPathRelease(rounded);
            CGImageRelease(marked);
            marked = CGBitmapContextCreateImage(c);
            CGContextRelease(c);
        }
        bool ok = write_png(marked, spec[@"out"], [spec[@"scale"] doubleValue] ?: 1);
        CGImageRelease(marked);
        return ok ? NULL : strdup("couldn't write the image");
    }
}

// Which of macOS's own screenshot shortcuts are on: a bit each for 28
// (⇧⌘3), 29 (⌃⇧⌘3), 30 (⇧⌘4), 31 (⌃⇧⌘4), 184 (⇧⌘5), in that order. On
// unless turned off (System Settings > Keyboard > Keyboard Shortcuts).
uint32_t nf_system_screenshot_keys(void) {
    @autoreleasepool {
        // (As they are now: they may have been changed since this app read them.)
        CFPreferencesAppSynchronize(CFSTR("com.apple.symbolichotkeys"));
        NSDictionary *keys = CFBridgingRelease(CFPreferencesCopyAppValue(CFSTR("AppleSymbolicHotKeys"), CFSTR("com.apple.symbolichotkeys")));
        uint32_t on = 0;
        NSArray<NSString *> *ids = @[@"28", @"29", @"30", @"31", @"184"];
        for (NSUInteger i = 0; i < ids.count; i++) {
            NSDictionary *key = [keys isKindOfClass:NSDictionary.class] ? keys[ids[i]] : nil;
            if (![key isKindOfClass:NSDictionary.class] || [key[@"enabled"] boolValue]) {
                on |= 1u << i;
            }
        }
        return on;
    }
}

// Whether macOS's own shortcut `id` is on (64: Spotlight's ⌘Space): on
// unless turned off in System Settings > Keyboard > Keyboard Shortcuts.
bool nf_symbolic_hotkey_on(int32_t id) {
    @autoreleasepool {
        CFPreferencesAppSynchronize(CFSTR("com.apple.symbolichotkeys"));
        NSDictionary *keys = CFBridgingRelease(CFPreferencesCopyAppValue(CFSTR("AppleSymbolicHotKeys"), CFSTR("com.apple.symbolichotkeys")));
        NSDictionary *key = [keys isKindOfClass:NSDictionary.class] ? keys[[NSString stringWithFormat:@"%d", id]] : nil;
        return ![key isKindOfClass:NSDictionary.class] || [key[@"enabled"] boolValue];
    }
}

// A QR code of `text`, as a PNG at `out`: black on white, crisp, `px`
// pixels square with a white border to scan it by. Whether it was made.
bool nf_qr_png(const char *text, const char *out, int px) {
    @autoreleasepool {
        CIFilter *filter = [CIFilter filterWithName:@"CIQRCodeGenerator"];
        [filter setValue:[@(text) dataUsingEncoding:NSUTF8StringEncoding] forKey:@"inputMessage"];
        [filter setValue:@"M" forKey:@"inputCorrectionLevel"];
        CIImage *code = filter.outputImage;
        if (!code || code.extent.size.width <= 0) return false;
        CIContext *context = [CIContext contextWithOptions:nil];
        CGImageRef modules = [context createCGImage:code fromRect:code.extent];
        if (!modules) return false;
        // Each module a whole number of pixels, four modules' border.
        size_t count = (size_t)code.extent.size.width;
        size_t side = (size_t)px / (count + 8);
        if (side < 1) side = 1;
        size_t size = side * (count + 8);
        CGColorSpaceRef gray = CGColorSpaceCreateDeviceGray();
        CGContextRef ctx = CGBitmapContextCreate(NULL, size, size, 8, 0, gray, (CGBitmapInfo)kCGImageAlphaNone);
        CGColorSpaceRelease(gray);
        if (!ctx) {
            CGImageRelease(modules);
            return false;
        }
        CGContextSetGrayFillColor(ctx, 1, 1);
        CGContextFillRect(ctx, CGRectMake(0, 0, size, size));
        CGContextSetInterpolationQuality(ctx, kCGInterpolationNone);
        CGContextDrawImage(ctx, CGRectMake(side * 4, side * 4, side * count, side * count), modules);
        CGImageRelease(modules);
        CGImageRef image = CGBitmapContextCreateImage(ctx);
        CGContextRelease(ctx);
        NSBitmapImageRep *rep = [[NSBitmapImageRep alloc] initWithCGImage:image];
        CGImageRelease(image);
        NSData *png = [rep representationUsingType:NSBitmapImageFileTypePNG properties:@{}];
        return [png writeToFile:@(out) atomically:YES];
    }
}

// What the QR codes (and other barcodes) in an image say, a line each:
// malloc'd, or NULL if there are none.
char *nf_read_qr(const char *path) {
    @autoreleasepool {
        VNDetectBarcodesRequest *request = [VNDetectBarcodesRequest new];
        VNImageRequestHandler *handler = [[VNImageRequestHandler alloc] initWithURL:[NSURL fileURLWithPath:@(path)] options:@{}];
        if (![handler performRequests:@[ request ] error:nil]) return NULL;
        NSMutableOrderedSet<NSString *> *found = [NSMutableOrderedSet orderedSet];
        for (VNBarcodeObservation *code in request.results) {
            if (code.payloadStringValue.length) [found addObject:code.payloadStringValue];
        }
        return found.count ? strdup([found.array componentsJoinedByString:@"\n"].UTF8String) : NULL;
    }
}

// An area to record, dragged out over the screens: each dimmed, the area
// clear, its size beside it; Escape (or a click, or a right-click) puts it
// away. `picked` gets it in points from the main screen's top left (as
// `screencapture -R` takes it), or a width of 0 if put away, on the main
// thread. (macOS's own picker won't record video: "video not valid with
// -i".)
@interface NFAreaView : NSView
@property NSPoint from;
@property NSPoint to;
@property BOOL dragging;
@end

static NSMutableArray<NSWindow *> *area_windows;
static void (*area_picked)(double x, double y, double w, double h);
static id area_keys_global, area_keys_local;

static void area_done(NSRect rect) {
    void (*picked)(double, double, double, double) = area_picked;
    area_picked = NULL;
    for (NSWindow *w in area_windows) [w orderOut:nil];
    area_windows = nil;
    if (area_keys_global) [NSEvent removeMonitor:area_keys_global];
    if (area_keys_local) [NSEvent removeMonitor:area_keys_local];
    area_keys_global = area_keys_local = nil;
    [NSCursor pop];
    if (!picked) return;
    if (rect.size.width < 8 || rect.size.height < 8) return picked(0, 0, 0, 0);
    // (From AppKit's bottom-left to the top left of the main screen.)
    CGFloat main = NSScreen.screens.firstObject.frame.size.height;
    picked(round(rect.origin.x), round(main - rect.origin.y - rect.size.height), round(rect.size.width), round(rect.size.height));
}

@implementation NFAreaView
- (BOOL)acceptsFirstMouse:(NSEvent *)event {
    return YES;
}

- (NSRect)area {
    return NSMakeRect(MIN(self.from.x, self.to.x), MIN(self.from.y, self.to.y), fabs(self.to.x - self.from.x), fabs(self.to.y - self.from.y));
}

- (void)drawRect:(NSRect)dirty {
    [[NSColor colorWithWhite:0 alpha:0.28] setFill];
    NSRectFill(self.bounds);
    if (self.dragging) {
        NSRect area = [self area];
        [NSColor.clearColor setFill];
        NSRectFillUsingOperation(area, NSCompositingOperationCopy);
        [[NSColor colorWithWhite:1 alpha:0.9] setStroke];
        NSBezierPath *edge = [NSBezierPath bezierPathWithRect:NSInsetRect(area, -0.5, -0.5)];
        edge.lineWidth = 1;
        [edge stroke];
        NSString *size = [NSString stringWithFormat:@"%.0f × %.0f", area.size.width, area.size.height];
        NSDictionary *style = @{NSFontAttributeName: [NSFont monospacedDigitSystemFontOfSize:12 weight:NSFontWeightSemibold], NSForegroundColorAttributeName: NSColor.whiteColor};
        NSSize text = [size sizeWithAttributes:style];
        NSRect tag = NSMakeRect(NSMaxX(area) - text.width - 12, NSMinY(area) - text.height - 10, text.width + 12, text.height + 6);
        if (tag.origin.y < 4) tag.origin.y = NSMinY(area) + 6;
        [[NSColor colorWithWhite:0 alpha:0.7] setFill];
        [[NSBezierPath bezierPathWithRoundedRect:tag xRadius:5 yRadius:5] fill];
        [size drawAtPoint:NSMakePoint(tag.origin.x + 6, tag.origin.y + 3) withAttributes:style];
    } else {
        NSString *hint = @"Drag out the area to record · Esc to cancel";
        NSDictionary *style = @{NSFontAttributeName: [NSFont systemFontOfSize:15 weight:NSFontWeightMedium], NSForegroundColorAttributeName: NSColor.whiteColor};
        NSSize text = [hint sizeWithAttributes:style];
        NSRect tag = NSMakeRect(NSMidX(self.bounds) - text.width / 2 - 14, NSMidY(self.bounds) - text.height / 2 - 8, text.width + 28, text.height + 16);
        [[NSColor colorWithWhite:0 alpha:0.6] setFill];
        [[NSBezierPath bezierPathWithRoundedRect:tag xRadius:10 yRadius:10] fill];
        [hint drawAtPoint:NSMakePoint(tag.origin.x + 14, tag.origin.y + 8) withAttributes:style];
    }
}

- (void)mouseDown:(NSEvent *)event {
    self.from = self.to = [self convertPoint:event.locationInWindow fromView:nil];
    self.dragging = YES;
    for (NSWindow *w in area_windows) w.contentView.needsDisplay = YES;
}

- (void)mouseDragged:(NSEvent *)event {
    self.to = [self convertPoint:event.locationInWindow fromView:nil];
    self.needsDisplay = YES;
}

- (void)mouseUp:(NSEvent *)event {
    self.to = [self convertPoint:event.locationInWindow fromView:nil];
    NSRect area = [self.window convertRectToScreen:[self convertRect:[self area] toView:nil]];
    area_done(area);
}

- (void)rightMouseDown:(NSEvent *)event {
    area_done(NSZeroRect);
}
@end

void nf_pick_area(void (*picked)(double x, double y, double w, double h)) {
    dispatch_async(dispatch_get_main_queue(), ^{
      if (area_windows) area_done(NSZeroRect);
      area_picked = picked;
      area_windows = [NSMutableArray array];
      for (NSScreen *screen in NSScreen.screens) {
          NSPanel *w = [[NSPanel alloc] initWithContentRect:screen.frame
                                                  styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel
                                                    backing:NSBackingStoreBuffered
                                                      defer:NO];
          w.level = NSScreenSaverWindowLevel;
          w.opaque = NO;
          w.backgroundColor = NSColor.clearColor;
          w.hasShadow = NO;
          w.ignoresMouseEvents = NO;
          w.collectionBehavior = NSWindowCollectionBehaviorCanJoinAllSpaces | NSWindowCollectionBehaviorFullScreenAuxiliary;
          w.contentView = [[NFAreaView alloc] initWithFrame:NSMakeRect(0, 0, screen.frame.size.width, screen.frame.size.height)];
          [w setFrame:screen.frame display:YES];
          [w orderFrontRegardless];
          [area_windows addObject:w];
      }
      [NSCursor.crosshairCursor push];
      // (Escape, though F2 isn't the app in front: heard anywhere.)
      NSEvent * (^escape)(NSEvent *) = ^NSEvent *(NSEvent *event) {
        if (event.keyCode == 53) {
            area_done(NSZeroRect);
            return nil;
        }
        return event;
      };
      area_keys_global = [NSEvent addGlobalMonitorForEventsMatchingMask:NSEventMaskKeyDown handler:^(NSEvent *event) { escape(event); }];
      area_keys_local = [NSEvent addLocalMonitorForEventsMatchingMask:NSEventMaskKeyDown handler:escape];
    });
}

// An image file's pixels as sRGB RGBA, 8 bits each, rows packed (`*w` × 4
// bytes): malloc'd, NULL if it can't be read.
uint8_t *nf_read_rgba(const char *path, size_t *w, size_t *h) {
    @autoreleasepool {
        CGImageSourceRef src = CGImageSourceCreateWithURL((__bridge CFURLRef)[NSURL fileURLWithPath:@(path)], NULL);
        if (!src) return NULL;
        CGImageRef image = CGImageSourceCreateImageAtIndex(src, 0, NULL);
        CFRelease(src);
        if (!image) return NULL;
        *w = CGImageGetWidth(image);
        *h = CGImageGetHeight(image);
        uint8_t *data = calloc(*w * *h, 4);
        CGColorSpaceRef srgb = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
        CGContextRef ctx = data ? CGBitmapContextCreate(data, *w, *h, 8, *w * 4, srgb, (CGBitmapInfo)kCGImageAlphaPremultipliedLast) : NULL;
        CGColorSpaceRelease(srgb);
        if (!ctx) {
            free(data);
            CGImageRelease(image);
            return NULL;
        }
        CGContextDrawImage(ctx, CGRectMake(0, 0, *w, *h), image);
        CGContextRelease(ctx);
        CGImageRelease(image);
        return data;
    }
}

// sRGB RGBA pixels (8 bits each, rows packed) as a PNG at `path`.
bool nf_write_png(const char *path, const uint8_t *rgba, size_t w, size_t h) {
    @autoreleasepool {
        CGColorSpaceRef srgb = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
        CGDataProviderRef data = CGDataProviderCreateWithData(NULL, rgba, w * h * 4, NULL);
        CGImageRef image = CGImageCreate(w, h, 8, 32, w * 4, srgb, (CGBitmapInfo)kCGImageAlphaPremultipliedLast, data, NULL, false, kCGRenderingIntentDefault);
        CGDataProviderRelease(data);
        CGColorSpaceRelease(srgb);
        if (!image) return false;
        CGImageDestinationRef dest = CGImageDestinationCreateWithURL((__bridge CFURLRef)[NSURL fileURLWithPath:@(path)], CFSTR("public.png"), 1, NULL);
        bool ok = false;
        if (dest) {
            CGImageDestinationAddImage(dest, image, NULL);
            ok = CGImageDestinationFinalize(dest);
            CFRelease(dest);
        }
        CGImageRelease(image);
        return ok;
    }
}

// Where the pointer is (points from the main screen's top left).
void nf_pointer(double *x, double *y) {
    CGEventRef e = CGEventCreate(NULL);
    CGPoint p = CGEventGetLocation(e);
    CFRelease(e);
    *x = p.x;
    *y = p.y;
}

void nf_move_pointer(double x, double y) {
    CGWarpMouseCursorPosition(CGPointMake(x, y));
    CGAssociateMouseAndMouseCursorPosition(true);
}

// Scroll what's under (x, y) by `dy` (negative: down the page), as a
// trackpad would, in a few small steps.
void nf_scroll_at(double x, double y, int dy) {
    CGEventSourceRef source = CGEventSourceCreate(kCGEventSourceStatePrivate);
    const int steps = 8;
    int left = dy;
    for (int i = 0; i < steps; i++) {
        int part = i == steps - 1 ? left : dy / steps;
        left -= part;
        CGEventRef e = CGEventCreateScrollWheelEvent2(source, kCGScrollEventUnitPixel, 1, part, 0, 0);
        CGEventSetLocation(e, CGPointMake(x, y));
        CGEventPost(kCGHIDEventTap, e);
        CFRelease(e);
        usleep(10000);
    }
    if (source) CFRelease(source);
}
