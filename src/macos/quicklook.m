// Quick Look: the system's panel (video, PDF pages, Markup, full screen),
// fed the selected files. The panel asks the responder chain who controls
// it; the key window is taught to answer, and up/down arrows pressed in the
// panel go back to the window, so the selection (and the preview) moves.

#import <AppKit/AppKit.h>
#import <QuickLookUI/QuickLookUI.h>
#import <objc/runtime.h>

@interface NFQuickLook : NSObject <QLPreviewPanelDataSource, QLPreviewPanelDelegate>
@property(strong) NSArray<NSURL *> *items;
@property(weak) NSWindow *window;
@end

@implementation NFQuickLook
- (NSInteger)numberOfPreviewItemsInPreviewPanel:(QLPreviewPanel *)panel {
    return self.items.count;
}

- (id<QLPreviewItem>)previewPanel:(QLPreviewPanel *)panel previewItemAtIndex:(NSInteger)index {
    return self.items[index];
}

- (BOOL)previewPanel:(QLPreviewPanel *)panel handleEvent:(NSEvent *)event {
    // Keys belong to the file list, as in Finder: arrows move through the
    // files, Space and Escape close, typing selects by name.
    if (event.type == NSEventTypeKeyDown && self.window) {
        [self.window sendEvent:event];
        return YES;
    }
    return NO;
}
@end

static NFQuickLook *controller;

static BOOL accepts(__unused id self, __unused SEL _cmd, __unused QLPreviewPanel *panel) {
    return YES;
}

static void begin(__unused id self, __unused SEL _cmd, QLPreviewPanel *panel) {
    panel.dataSource = controller;
    panel.delegate = controller;
}

static void end(__unused id self, __unused SEL _cmd, QLPreviewPanel *panel) {
    panel.dataSource = nil;
    panel.delegate = nil;
}

static void teach(NSWindow *window) {
    Class cls = window.class;
    if (class_respondsToSelector(cls, @selector(acceptsPreviewPanelControl:))) return;
    class_addMethod(cls, @selector(acceptsPreviewPanelControl:), (IMP)accepts, "c@:@");
    class_addMethod(cls, @selector(beginPreviewPanelControl:), (IMP)begin, "v@:@");
    class_addMethod(cls, @selector(endPreviewPanelControl:), (IMP)end, "v@:@");
}

static NSArray<NSURL *> *urls(const char *paths) {
    NSMutableArray<NSURL *> *list = [NSMutableArray array];
    for (NSString *p in [@(paths) componentsSeparatedByString:@"\n"]) {
        if (p.length) [list addObject:[NSURL fileURLWithPath:p]];
    }
    return list;
}

bool nf_quicklook_visible(void) {
    return [QLPreviewPanel sharedPreviewPanelExists] && QLPreviewPanel.sharedPreviewPanel.isVisible;
}

// Show these files (one per line), starting at `index`; or, if the panel
// is up, show them in it.
void nf_quicklook_show(const char *paths, long index) {
    @autoreleasepool {
        if (!controller) controller = [NFQuickLook new];
        controller.items = urls(paths);
        NSWindow *key = NSApp.keyWindow;
        if (key && ![key isKindOfClass:QLPreviewPanel.class]) {
            controller.window = key;
            teach(key);
        }
        QLPreviewPanel *panel = QLPreviewPanel.sharedPreviewPanel;
        if (panel.dataSource != controller) {
            panel.dataSource = controller;
            panel.delegate = controller;
        }
        [panel reloadData];
        panel.currentPreviewItemIndex = MAX(0, MIN(index, (long)controller.items.count - 1));
        if (!panel.isVisible) [panel makeKeyAndOrderFront:nil];
    }
}

void nf_quicklook_close(void) {
    if (nf_quicklook_visible()) [QLPreviewPanel.sharedPreviewPanel orderOut:nil];
}

// Quick Look's panel made (not shown), so the first look of the session
// isn't the slow one. On the main thread.
void nf_quicklook_warm(void) {
    @autoreleasepool {
        (void)QLPreviewPanel.sharedPreviewPanel;
    }
}
