// How many frames each window's Metal layer keeps: GPUI makes it 3, a
// full-size retina surface each (15 MB for a big window); 2 is a third
// less, for frames that may wait a little longer on the one on screen.

#import <AppKit/AppKit.h>
#import <QuartzCore/QuartzCore.h>

static NSUInteger drawables = 3;

static void apply_layer(CALayer *layer) {
    if ([layer isKindOfClass:CAMetalLayer.class]) {
        ((CAMetalLayer *)layer).maximumDrawableCount = drawables;
    }
    for (CALayer *sub in layer.sublayers) {
        apply_layer(sub);
    }
}

static void apply_view(NSView *view) {
    if (view.layer) {
        apply_layer(view.layer);
    }
    for (NSView *sub in view.subviews) {
        apply_view(sub);
    }
}

static void apply_window(NSWindow *window) {
    if (window.contentView) {
        apply_view(window.contentView);
    }
}

// Frames kept per window (2 or 3): for the windows open now, and each one
// as it comes to the front (new ones, Quick Open).
void nf_set_drawables(uint32_t count) {
    dispatch_async(dispatch_get_main_queue(), ^{
        drawables = count < 2 ? 2 : (count > 3 ? 3 : count);
        for (NSWindow *window in NSApp.windows) {
            apply_window(window);
        }
        static dispatch_once_t once;
        dispatch_once(&once, ^{
            [NSNotificationCenter.defaultCenter addObserverForName:NSWindowDidBecomeKeyNotification
                                                            object:nil
                                                             queue:NSOperationQueue.mainQueue
                                                        usingBlock:^(NSNotification *note) {
                                                            apply_window(note.object);
                                                        }];
        });
    });
}
