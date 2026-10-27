// File icons and Quick Look thumbnails, as raw pixels for GPUI.
//
// Each function returns a malloc'd BGRA buffer with premultiplied alpha
// (free it with `free`), or NULL. Safe to call from any thread; the
// thumbnail call blocks until Quick Look answers.

#import <AppKit/AppKit.h>
#import <QuickLookThumbnailing/QuickLookThumbnailing.h>
#import <UniformTypeIdentifiers/UniformTypeIdentifiers.h>
#include <sys/xattr.h>

static CGContextRef bgra_context(uint8_t *data, uint32_t w, uint32_t h) {
    CGColorSpaceRef srgb = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
    CGContextRef ctx = CGBitmapContextCreate(data, w, h, 8, (size_t)w * 4, srgb,
                                             kCGImageAlphaPremultipliedFirst | kCGBitmapByteOrder32Little);
    CGColorSpaceRelease(srgb);
    return ctx;
}

static uint8_t *draw_icon(NSImage *icon, uint32_t px) {
    if (!icon || px == 0) return NULL;
    uint8_t *data = calloc((size_t)px * px, 4);
    CGContextRef ctx = bgra_context(data, px, px);
    if (!ctx) {
        free(data);
        return NULL;
    }
    [NSGraphicsContext saveGraphicsState];
    NSGraphicsContext.currentContext = [NSGraphicsContext graphicsContextWithCGContext:ctx flipped:NO];
    // Icons hold several sizes; drawing picks the one made for `px`.
    [icon drawInRect:NSMakeRect(0, 0, px, px) fromRect:NSZeroRect operation:NSCompositingOperationSourceOver fraction:1];
    [NSGraphicsContext restoreGraphicsState];
    CGContextRelease(ctx);
    return data;
}

// The icon Finder shows for this file: a folder's or app's own, or its type's.
uint8_t *nf_file_icon(const char *path, uint32_t px) {
    @autoreleasepool {
        return draw_icon([NSWorkspace.sharedWorkspace iconForFile:@(path)], px);
    }
}

// The icon for files with this extension (or for a plain folder).
uint8_t *nf_type_icon(const char *ext, bool is_dir, uint32_t px) {
    @autoreleasepool {
        UTType *type = is_dir ? UTTypeFolder : [UTType typeWithFilenameExtension:@(ext)];
        return draw_icon([NSWorkspace.sharedWorkspace iconForContentType:type ?: UTTypeData], px);
    }
}

// A Quick Look thumbnail at most `px` on its longer side, in its own aspect
// ratio (`w` by `h`). NULL when the file has none (or it took over 10 s).
uint8_t *nf_thumbnail(const char *path, uint32_t px, uint32_t *w, uint32_t *h) {
    @autoreleasepool {
        QLThumbnailGenerationRequest *request = [[QLThumbnailGenerationRequest alloc]
            initWithFileAtURL:[NSURL fileURLWithPath:@(path)]
                         size:CGSizeMake(px, px)
                        scale:1
          representationTypes:QLThumbnailGenerationRequestRepresentationTypeThumbnail];
        dispatch_semaphore_t done = dispatch_semaphore_create(0);
        __block CGImageRef image = NULL;
        [QLThumbnailGenerator.sharedGenerator
            generateBestRepresentationForRequest:request
                               completionHandler:^(QLThumbnailRepresentation *rep, __unused NSError *error) {
                                 if (rep) image = CGImageRetain(rep.CGImage);
                                 dispatch_semaphore_signal(done);
                               }];
        if (dispatch_semaphore_wait(done, dispatch_time(DISPATCH_TIME_NOW, 10 * NSEC_PER_SEC))) {
            [QLThumbnailGenerator.sharedGenerator cancelRequest:request];
            dispatch_semaphore_wait(done, DISPATCH_TIME_FOREVER);
        }
        if (!image) return NULL;

        *w = (uint32_t)CGImageGetWidth(image);
        *h = (uint32_t)CGImageGetHeight(image);
        uint8_t *data = calloc((size_t)*w * *h, 4);
        CGContextRef ctx = bgra_context(data, *w, *h);
        if (ctx) {
            CGContextDrawImage(ctx, CGRectMake(0, 0, *w, *h), image);
            CGContextRelease(ctx);
        } else {
            free(data);
            data = NULL;
        }
        CGImageRelease(image);
        return data;
    }
}

// Whether folders take their tags' colors (Finder's "Tint folders based on
// tags", on unless it's been turned off).
bool nf_tag_tinting(void) {
    return ![NSUserDefaults.standardUserDefaults boolForKey:@"AppleDisableTagBasedIconTinting"];
}

static void rgb_to_hsv(float r, float g, float b, float *h, float *s, float *v) {
    float max = fmaxf(r, fmaxf(g, b)), min = fminf(r, fminf(g, b)), d = max - min;
    *v = max;
    *s = max > 0 ? d / max : 0;
    if (d == 0) {
        *h = 0;
    } else if (max == r) {
        *h = fmodf((g - b) / d, 6);
    } else if (max == g) {
        *h = (b - r) / d + 2;
    } else {
        *h = (r - g) / d + 4;
    }
    *h = *h < 0 ? *h + 6 : *h;
}

static void hsv_to_rgb(float h, float s, float v, float *r, float *g, float *b) {
    float c = v * s, x = c * (1 - fabsf(fmodf(h, 2) - 1)), m = v - c;
    float rr = 0, gg = 0, bb = 0;
    switch ((int)h % 6) {
        case 0: rr = c, gg = x; break;
        case 1: rr = x, gg = c; break;
        case 2: gg = c, bb = x; break;
        case 3: gg = x, bb = c; break;
        case 4: rr = x, bb = c; break;
        default: rr = c, bb = x; break;
    }
    *r = rr + m, *g = gg + m, *b = bb + m;
}

// A folder's icon in a tag's color (0xRRGGBB), as Finder 26 shows tagged
// folders: the folder's own icon in the tag's hue (its symbol, if it has
// one, following), but an emoji on it as it is (where it differs from a
// plain folder's, it's left be).
uint8_t *nf_folder_icon_tinted(const char *path, uint32_t px, uint32_t rgb) {
    @autoreleasepool {
        uint8_t *own = draw_icon([NSWorkspace.sharedWorkspace iconForFile:@(path)], px);
        if (!own) return NULL;
        char mark[256] = {0};
        ssize_t n = getxattr(path, "com.apple.icon.folder#S", mark, sizeof(mark) - 1, 0, 0);
        bool emoji = n > 0 && strstr(mark, "\"emoji\"") != NULL;
        uint8_t *plain = emoji ? draw_icon([NSWorkspace.sharedWorkspace iconForContentType:UTTypeFolder], px) : NULL;
        float th, ts, tv;
        rgb_to_hsv(((rgb >> 16) & 0xff) / 255.f, ((rgb >> 8) & 0xff) / 255.f, (rgb & 0xff) / 255.f, &th, &ts, &tv);
        // (Gray: the folder's own shading, all but without color.)
        bool gray = ts < 0.1f;
        size_t count = (size_t)px * px;
        for (size_t i = 0; i < count; i++) {
            uint8_t *p = own + i * 4;  // BGRA, premultiplied
            uint8_t a = p[3];
            if (a == 0) continue;
            if (plain) {
                uint8_t *q = plain + i * 4;
                int differs = abs(p[0] - q[0]) + abs(p[1] - q[1]) + abs(p[2] - q[2]) + abs(p[3] - q[3]);
                if (differs > 36) continue;
            }
            float b = p[0] / (float)a, g = p[1] / (float)a, r = p[2] / (float)a;
            float h, s, v;
            rgb_to_hsv(r, g, b, &h, &s, &v);
            if (gray) {
                s *= 0.06f;
                v *= 0.86f;
            } else {
                s = fminf(1.f, s * (0.55f + ts * 0.5f));
            }
            hsv_to_rgb(th, s, v, &r, &g, &b);
            p[0] = (uint8_t)fminf(255.f, b * a + 0.5f);
            p[1] = (uint8_t)fminf(255.f, g * a + 0.5f);
            p[2] = (uint8_t)fminf(255.f, r * a + 0.5f);
        }
        free(plain);
        return own;
    }
}
