// Photos and PDFs, for Quick Actions: a photo's subject cut out of its
// background (Vision's, as Photos and Preview do it: macOS 14 and later),
// and the text in an image or a PDF (a PDF's own, else read off its pages
// by Vision, as for a scan).

#import <AVFoundation/AVFoundation.h>
#import <AppKit/AppKit.h>
#import <CoreImage/CoreImage.h>
#import <PDFKit/PDFKit.h>
#import <Vision/Vision.h>

// (shots.m's.)
NSString *nf_text_in(VNImageRequestHandler *handler);

bool nf_can_remove_background(void) {
    if (@available(macOS 14.0, *)) return true;
    return false;
}

static char *why(NSError *error, NSString *otherwise) {
    return strdup((error.localizedDescription ?: otherwise).UTF8String);
}

// The image at `path` with its background taken out (what's left of it
// clear), as a PNG at `out`. NULL if it's made, else why not: malloc'd.
char *nf_remove_background(const char *path, const char *out) {
    @autoreleasepool {
        if (@available(macOS 14.0, *)) {
            // (Upright, as it's shown: a phone's photo is kept on its side.)
            CIImage *image = [CIImage imageWithContentsOfURL:[NSURL fileURLWithPath:@(path)] options:@{kCIImageApplyOrientationProperty: @YES}];
            if (!image) return strdup("The image couldn't be read.");
            VNImageRequestHandler *handler = [[VNImageRequestHandler alloc] initWithCIImage:image options:@{}];
            VNGenerateForegroundInstanceMaskRequest *request = [VNGenerateForegroundInstanceMaskRequest new];
            NSError *error = nil;
            if (![handler performRequests:@[request] error:&error]) return why(error, @"Vision couldn't look at it.");
            VNInstanceMaskObservation *found = request.results.firstObject;
            if (!found || found.allInstances.count == 0) return strdup("There's no clear subject in it to keep.");
            CVPixelBufferRef masked = [found generateMaskedImageOfInstances:found.allInstances fromRequestHandler:handler croppedToInstancesExtent:NO error:&error];
            if (!masked) return why(error, @"The background couldn't be taken out.");
            CIImage *cut = [CIImage imageWithCVPixelBuffer:masked];
            CGColorSpaceRef srgb = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
            BOOL ok = [[CIContext context] writePNGRepresentationOfImage:cut toURL:[NSURL fileURLWithPath:@(out)] format:kCIFormatRGBA8 colorSpace:srgb options:@{} error:&error];
            CGColorSpaceRelease(srgb);
            CFRelease(masked);
            return ok ? NULL : why(error, @"The new image couldn't be saved.");
        }
        return strdup("Taking out a background takes macOS 14 or later.");
    }
}

// The text in a PDF: what's typed in it, else (a scan) read off its first
// 30 pages by Vision. NULL if there's none: malloc'd.
char *nf_pdf_text(const char *path) {
    @autoreleasepool {
        PDFDocument *document = [[PDFDocument alloc] initWithURL:[NSURL fileURLWithPath:@(path)]];
        if (!document) return NULL;
        NSCharacterSet *space = NSCharacterSet.whitespaceAndNewlineCharacterSet;
        NSString *own = [document.string stringByTrimmingCharactersInSet:space];
        if (own.length) return strdup(own.UTF8String);
        NSMutableArray<NSString *> *pages = [NSMutableArray array];
        for (NSUInteger i = 0; i < MIN(document.pageCount, (NSUInteger)30); i++) {
            @autoreleasepool {
                PDFPage *page = [document pageAtIndex:i];
                NSRect box = [page boundsForBox:kPDFDisplayBoxMediaBox];
                // (About 2,400 pixels on its long side: enough for print.)
                CGFloat scale = 2400 / MAX(MAX(box.size.width, box.size.height), 1);
                NSImage *drawn = [page thumbnailOfSize:NSMakeSize(box.size.width * scale, box.size.height * scale) forBox:kPDFDisplayBoxMediaBox];
                CGImageRef image = [drawn CGImageForProposedRect:NULL context:nil hints:nil];
                if (!image) continue;
                NSString *text = nf_text_in([[VNImageRequestHandler alloc] initWithCGImage:image options:@{}]);
                if (text.length) [pages addObject:text];
            }
        }
        return pages.count ? strdup([pages componentsJoinedByString:@"\n\n"].UTF8String) : NULL;
    }
}

// A video's frames, in order, each handed to `frame` as RGBA (its rows
// `stride` bytes apart) with how long it shows: `width` pixels wide at most,
// up to 12 a second, and no more than `most` of them (a long video gets
// fewer a second, as few as 4; past that, only its start). One at a time,
// so only one's in memory. NULL if done, else why not (malloc'd). Waits:
// not on the main thread.
char *nf_video_frames(const char *video, int width, int most, void *ctx, void (*frame)(void *ctx, const uint8_t *rgba, size_t w, size_t h, size_t stride, double secs)) {
    @autoreleasepool {
        AVURLAsset *asset = [AVURLAsset URLAssetWithURL:[NSURL fileURLWithPath:@(video)] options:nil];
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
        double secs = CMTimeGetSeconds(asset.duration);
#pragma clang diagnostic pop
        if (!(secs > 0)) return strdup("there's nothing in it to show");
        double fps = MAX(4.0, MIN(12.0, most / secs));
        int count = MAX(1, (int)MIN((double)most, floor(secs * fps)));
        AVAssetImageGenerator *generator = [AVAssetImageGenerator assetImageGeneratorWithAsset:asset];
        generator.appliesPreferredTrackTransform = YES;
        generator.maximumSize = CGSizeMake(width, width);
        CMTime tolerance = CMTimeMakeWithSeconds(0.5 / fps, 600);
        generator.requestedTimeToleranceBefore = tolerance;
        generator.requestedTimeToleranceAfter = tolerance;
        NSMutableArray<NSValue *> *times = [NSMutableArray arrayWithCapacity:count];
        for (int i = 0; i < count; i++) [times addObject:[NSValue valueWithCMTime:CMTimeMakeWithSeconds(i / fps, 600)]];
        CGColorSpaceRef srgb = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
        __block int handed = 0, handled = 0;
        dispatch_semaphore_t finished = dispatch_semaphore_create(0);
        // (Handed back in the order asked for, one at a time.)
        [generator generateCGImagesAsynchronouslyForTimes:times
                                        completionHandler:^(CMTime __unused asked, CGImageRef image, CMTime __unused at, AVAssetImageGeneratorResult result, NSError *__unused error) {
                                          if (result == AVAssetImageGeneratorSucceeded && image) {
                                              size_t w = CGImageGetWidth(image), h = CGImageGetHeight(image);
                                              CGContextRef bitmap = CGBitmapContextCreate(NULL, w, h, 8, 0, srgb, (CGBitmapInfo)kCGImageAlphaPremultipliedLast);
                                              if (bitmap) {
                                                  CGContextDrawImage(bitmap, CGRectMake(0, 0, w, h), image);
                                                  frame(ctx, CGBitmapContextGetData(bitmap), w, h, CGBitmapContextGetBytesPerRow(bitmap), 1.0 / fps);
                                                  CGContextRelease(bitmap);
                                                  handed++;
                                              }
                                          }
                                          if (++handled == count) dispatch_semaphore_signal(finished);
                                        }];
        long waited = dispatch_semaphore_wait(finished, dispatch_time(DISPATCH_TIME_NOW, (int64_t)(10 * 60) * NSEC_PER_SEC));
        CGColorSpaceRelease(srgb);
        if (waited != 0) {
            [generator cancelAllCGImageGeneration];
            return strdup("it took too long");
        }
        return handed ? NULL : strdup("its frames couldn't be read");
    }
}

// A video's first frame (well, half a second in: the first's often black),
// `max` pixels on its long side, as a PNG at `out`. Whether it's made.
bool nf_video_thumbnail(const char *path, const char *out, int max) {
    @autoreleasepool {
        AVURLAsset *asset = [AVURLAsset URLAssetWithURL:[NSURL fileURLWithPath:@(path)] options:nil];
        AVAssetImageGenerator *generator = [AVAssetImageGenerator assetImageGeneratorWithAsset:asset];
        generator.appliesPreferredTrackTransform = YES;
        generator.maximumSize = CGSizeMake(max, max);
        __block CGImageRef frame = NULL;
        dispatch_semaphore_t made = dispatch_semaphore_create(0);
        [generator generateCGImageAsynchronouslyForTime:CMTimeMakeWithSeconds(0.5, 600)
                                      completionHandler:^(CGImageRef image, CMTime __unused at, NSError *__unused error) {
                                        if (image) frame = CGImageRetain(image);
                                        dispatch_semaphore_signal(made);
                                      }];
        dispatch_semaphore_wait(made, dispatch_time(DISPATCH_TIME_NOW, 10 * NSEC_PER_SEC));
        // (A clip shorter than that: its first frame.)
        if (!frame) {
            [generator generateCGImageAsynchronouslyForTime:kCMTimeZero
                                          completionHandler:^(CGImageRef image, CMTime __unused at, NSError *__unused error) {
                                            if (image) frame = CGImageRetain(image);
                                            dispatch_semaphore_signal(made);
                                          }];
            dispatch_semaphore_wait(made, dispatch_time(DISPATCH_TIME_NOW, 10 * NSEC_PER_SEC));
        }
        if (!frame) return false;
        NSBitmapImageRep *rep = [[NSBitmapImageRep alloc] initWithCGImage:frame];
        CGImageRelease(frame);
        NSData *png = [rep representationUsingType:NSBitmapImageFileTypePNG properties:@{}];
        return png && [png writeToFile:@(out) atomically:YES];
    }
}

// `image` as premultiplied BGRA (sRGB), its own size: `*w` by `*h`,
// malloc'd; NULL if it can't be drawn.
static uint8_t *bgra_of(CGImageRef image, uint32_t *w, uint32_t *h) {
    *w = (uint32_t)CGImageGetWidth(image);
    *h = (uint32_t)CGImageGetHeight(image);
    uint8_t *data = calloc((size_t)*w * *h, 4);
    if (!data) return NULL;
    CGColorSpaceRef srgb = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
    CGContextRef ctx = CGBitmapContextCreate(data, *w, *h, 8, (size_t)*w * 4, srgb, (CGBitmapInfo)kCGImageAlphaPremultipliedFirst | kCGBitmapByteOrder32Little);
    CGColorSpaceRelease(srgb);
    if (!ctx) {
        free(data);
        return NULL;
    }
    CGContextDrawImage(ctx, CGRectMake(0, 0, *w, *h), image);
    CGContextRelease(ctx);
    return data;
}

// An image file as ImageIO reads it (HEIC, camera raw, PSD…), turned as
// its EXIF says, `max` pixels on its longer side at most: premultiplied
// BGRA, `*w` by `*h`, malloc'd; NULL if it can't be read. Off the main
// thread is fine.
uint8_t *nf_image_bitmap(const char *path, int max, uint32_t *w, uint32_t *h) {
    @autoreleasepool {
        CGImageSourceRef src = CGImageSourceCreateWithURL((__bridge CFURLRef)[NSURL fileURLWithPath:@(path)], NULL);
        if (!src) return NULL;
        NSDictionary *options = @{
            (__bridge NSString *)kCGImageSourceCreateThumbnailFromImageAlways : @YES,
            (__bridge NSString *)kCGImageSourceCreateThumbnailWithTransform : @YES,
            (__bridge NSString *)kCGImageSourceThumbnailMaxPixelSize : @(max),
            (__bridge NSString *)kCGImageSourceShouldCacheImmediately : @YES,
        };
        CGImageRef image = CGImageSourceCreateThumbnailAtIndex(src, 0, (__bridge CFDictionaryRef)options);
        CFRelease(src);
        if (!image) return NULL;
        uint8_t *out = bgra_of(image, w, h);
        CGImageRelease(image);
        return out;
    }
}

// A folder's background picture (Finder's, behind a disk image's icons):
// how big it's shown, in points (`*pw` by `*ph`: its own size, as Finder
// shows it), and drawn for a screen of `scale` (its 2x representation, if
// it has one) as premultiplied BGRA, `*w` by `*h`, malloc'd; NULL if it
// can't be read. Off the main thread is fine.
uint8_t *nf_picture_bitmap(const char *path, double scale, double *pw, double *ph, uint32_t *w, uint32_t *h) {
    @autoreleasepool {
        NSImage *image = [[NSImage alloc] initWithContentsOfFile:@(path)];
        if (!image || image.size.width < 1 || image.size.height < 1) return NULL;
        *pw = image.size.width;
        *ph = image.size.height;
        NSRect rect = NSMakeRect(0, 0, *pw, *ph);
        NSAffineTransform *scaled = [NSAffineTransform transform];
        [scaled scaleBy:scale];
        CGImageRef best = [image CGImageForProposedRect:&rect context:nil hints:@{NSImageHintCTM : scaled}];
        return best ? bgra_of(best, w, h) : NULL;
    }
}

// A PDF's pages `first` to `first + most - 1`, each `width` pixels wide, on
// white, turned as the page says: each handed to `page` (premultiplied
// BGRA, its own to free). How many pages it has; -1 if it can't be read (or
// is locked). Core Graphics: off the main thread is fine.
long nf_pdf_pages(const char *path, long first, long most, int width, void *ctx, void (*page)(void *ctx, uint8_t *bgra, uint32_t w, uint32_t h)) {
    @autoreleasepool {
        CGPDFDocumentRef doc = CGPDFDocumentCreateWithURL((__bridge CFURLRef)[NSURL fileURLWithPath:@(path)]);
        if (!doc) return -1;
        if (CGPDFDocumentIsEncrypted(doc) && !CGPDFDocumentIsUnlocked(doc) && !CGPDFDocumentUnlockWithPassword(doc, "")) {
            CGPDFDocumentRelease(doc);
            return -1;
        }
        long count = (long)CGPDFDocumentGetNumberOfPages(doc);
        CGColorSpaceRef srgb = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
        for (long i = first; i < count && i < first + most; i++) {
            CGPDFPageRef p = CGPDFDocumentGetPage(doc, (size_t)i + 1);
            if (!p) continue;
            CGRect box = CGPDFPageGetBoxRect(p, kCGPDFCropBox);
            bool turned = CGPDFPageGetRotationAngle(p) % 180 != 0;
            double pw = turned ? box.size.height : box.size.width, ph = turned ? box.size.width : box.size.height;
            if (pw <= 0 || ph <= 0) continue;
            uint32_t w = (uint32_t)width, h = (uint32_t)MAX(1.0, round(ph * width / pw));
            uint8_t *data = calloc((size_t)w * h, 4);
            if (!data) break;
            CGContextRef bitmap = CGBitmapContextCreate(data, w, h, 8, (size_t)w * 4, srgb, (CGBitmapInfo)kCGImageAlphaPremultipliedFirst | kCGBitmapByteOrder32Little);
            if (!bitmap) {
                free(data);
                continue;
            }
            CGContextSetRGBFillColor(bitmap, 1, 1, 1, 1);
            CGContextFillRect(bitmap, CGRectMake(0, 0, w, h));
            // (Drawn at its own size, turned, then scaled to the width: the
            // drawing transform only ever scales down.)
            CGContextScaleCTM(bitmap, w / pw, h / ph);
            CGContextConcatCTM(bitmap, CGPDFPageGetDrawingTransform(p, kCGPDFCropBox, CGRectMake(0, 0, pw, ph), 0, true));
            CGContextDrawPDFPage(bitmap, p);
            CGContextRelease(bitmap);
            page(ctx, data, w, h);
        }
        CGColorSpaceRelease(srgb);
        CGPDFDocumentRelease(doc);
        return count;
    }
}

// An image's size in pixels as it's shown (turned as its EXIF says), and
// whether it can be see-through: false if it can't be read. Only its
// header's read.
bool nf_shown_size(const char *path, uint32_t *w, uint32_t *h, bool *alpha) {
    @autoreleasepool {
        CGImageSourceRef src = CGImageSourceCreateWithURL((__bridge CFURLRef)[NSURL fileURLWithPath:@(path)], NULL);
        if (!src) return false;
        NSDictionary *props = CFBridgingRelease(CGImageSourceCopyPropertiesAtIndex(src, 0, NULL));
        CFRelease(src);
        NSNumber *pw = props[(__bridge NSString *)kCGImagePropertyPixelWidth], *ph = props[(__bridge NSString *)kCGImagePropertyPixelHeight];
        if (!pw || !ph) return false;
        int orientation = [props[(__bridge NSString *)kCGImagePropertyOrientation] intValue];
        bool turned = orientation >= 5 && orientation <= 8;
        *w = (turned ? ph : pw).unsignedIntValue;
        *h = (turned ? pw : ph).unsignedIntValue;
        *alpha = [props[(__bridge NSString *)kCGImagePropertyHasAlpha] boolValue];
        return true;
    }
}

// The image at `path`, `longest` pixels on its longer side, upright, as
// `type` (a UTI: "public.jpeg") at `out`, its metadata kept (but for how
// it was turned). NULL if it's made, else why not: malloc'd. ImageIO's own
// scaling, as Finder's and Preview's: off the main thread is fine.
char *nf_resize_image(const char *path, int longest, const char *type, const char *out) {
    @autoreleasepool {
        CGImageSourceRef src = CGImageSourceCreateWithURL((__bridge CFURLRef)[NSURL fileURLWithPath:@(path)], NULL);
        if (!src) return strdup("It isn't an image that can be read.");
        NSDictionary *options = @{
            (__bridge NSString *)kCGImageSourceCreateThumbnailFromImageAlways : @YES,
            (__bridge NSString *)kCGImageSourceCreateThumbnailWithTransform : @YES,
            (__bridge NSString *)kCGImageSourceThumbnailMaxPixelSize : @(longest),
        };
        CGImageRef image = CGImageSourceCreateThumbnailAtIndex(src, 0, (__bridge CFDictionaryRef)options);
        NSMutableDictionary *props = [CFBridgingRelease(CGImageSourceCopyPropertiesAtIndex(src, 0, NULL)) mutableCopy] ?: [NSMutableDictionary dictionary];
        CFRelease(src);
        if (!image) return strdup("It couldn't be read.");
        // (Upright now: so said, in its TIFF part too.)
        props[(__bridge NSString *)kCGImagePropertyOrientation] = @1;
        NSMutableDictionary *tiff = [props[(__bridge NSString *)kCGImagePropertyTIFFDictionary] mutableCopy];
        if (tiff) {
            tiff[(__bridge NSString *)kCGImagePropertyTIFFOrientation] = @1;
            props[(__bridge NSString *)kCGImagePropertyTIFFDictionary] = tiff;
        }
        props[(__bridge NSString *)kCGImageDestinationLossyCompressionQuality] = @0.85;
        CGImageDestinationRef dest = CGImageDestinationCreateWithURL((__bridge CFURLRef)[NSURL fileURLWithPath:@(out)], (__bridge CFStringRef)@(type), 1, NULL);
        if (!dest) {
            CGImageRelease(image);
            return strdup("It couldn't be written there.");
        }
        CGImageDestinationAddImage(dest, image, (__bridge CFDictionaryRef)props);
        bool ok = CGImageDestinationFinalize(dest);
        CFRelease(dest);
        CGImageRelease(image);
        return ok ? NULL : strdup("It couldn't be written.");
    }
}
