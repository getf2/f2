// Files dragged in from apps that only promise them (Mail attachments,
// Photos, Safari images): GPUI's windows take dropped file paths only, so
// their drag methods are extended to accept promises too. Promised files
// are written into a staging folder, then handed to `callback` with the
// window's token, to be moved where they were dropped.

#import <AppKit/AppKit.h>
#import <objc/runtime.h>

typedef void (*nf_promises_cb)(uint64_t token, const char *paths, double x, double y);

static nf_promises_cb callback;
static NSMapTable<NSWindow *, NSNumber *> *tokens;
static NSDragOperation (*original_entered)(id, SEL, id);
static NSDragOperation (*original_updated)(id, SEL, id);
static BOOL (*original_perform)(id, SEL, id);

// Promises only: real files take GPUI's own path.
static BOOL promises_only(id<NSDraggingInfo> info) {
    NSPasteboard *pb = info.draggingPasteboard;
    if ([pb canReadObjectForClasses:@[ NSURL.class ] options:@{NSPasteboardURLReadingFileURLsOnlyKey : @YES}]) return NO;
    return [pb canReadObjectForClasses:@[ NSFilePromiseReceiver.class ] options:nil];
}

static NSDragOperation entered(id self, SEL cmd, id<NSDraggingInfo> info) {
    NSDragOperation op = original_entered(self, cmd, info);
    return op == NSDragOperationNone && promises_only(info) ? NSDragOperationCopy : op;
}

static NSDragOperation updated(id self, SEL cmd, id<NSDraggingInfo> info) {
    NSDragOperation op = original_updated(self, cmd, info);
    return op == NSDragOperationNone && promises_only(info) ? NSDragOperationCopy : op;
}

static BOOL perform(id self, SEL cmd, id<NSDraggingInfo> info) {
    if (!promises_only(info)) return original_perform(self, cmd, info);
    NSWindow *window = self;
    uint64_t token = [tokens objectForKey:window].unsignedLongLongValue;
    // Where it was dropped, from the window's top left, in points.
    NSPoint at = info.draggingLocation;
    double x = at.x, y = window.contentView.frame.size.height - at.y;

    NSString *stage = [NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString];
    [NSFileManager.defaultManager createDirectoryAtPath:stage withIntermediateDirectories:YES attributes:nil error:nil];
    NSArray<NSFilePromiseReceiver *> *receivers = [info.draggingPasteboard readObjectsForClasses:@[ NSFilePromiseReceiver.class ]
                                                                                         options:nil];
    NSOperationQueue *queue = [NSOperationQueue new];
    dispatch_group_t group = dispatch_group_create();
    NSMutableArray<NSString *> *received = [NSMutableArray array];
    for (NSFilePromiseReceiver *receiver in receivers) {
        NSUInteger files = MAX(receiver.fileNames.count, 1u);
        for (NSUInteger i = 0; i < files; i++) dispatch_group_enter(group);
        [receiver receivePromisedFilesAtDestination:[NSURL fileURLWithPath:stage]
                                            options:@{}
                                     operationQueue:queue
                                             reader:^(NSURL *file, __unused NSError *error) {
                                               if (file) {
                                                   @synchronized(received) {
                                                       [received addObject:file.path];
                                                   }
                                               }
                                               dispatch_group_leave(group);
                                             }];
    }
    dispatch_group_notify(group, dispatch_get_main_queue(), ^{
      if (callback && received.count) callback(token, [received componentsJoinedByString:@"\n"].UTF8String, x, y);
    });
    return YES;
}

// Accept promised files dropped on the window holding `view`.
void nf_accept_promises(void *view, uint64_t token, nf_promises_cb cb) {
    NSWindow *window = ((__bridge NSView *)view).window;
    if (!window) return;
    callback = cb;
    if (!tokens) tokens = [NSMapTable weakToStrongObjectsMapTable];
    [tokens setObject:@(token) forKey:window];
    NSArray *types = [@[ NSPasteboardTypeFileURL, @"NSFilenamesPboardType" ]
        arrayByAddingObjectsFromArray:NSFilePromiseReceiver.readableDraggedTypes];
    [window registerForDraggedTypes:types];

    Class cls = window.class;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
      Method m;
      m = class_getInstanceMethod(cls, @selector(draggingEntered:));
      original_entered = (void *)method_setImplementation(m, (IMP)entered);
      m = class_getInstanceMethod(cls, @selector(draggingUpdated:));
      original_updated = (void *)method_setImplementation(m, (IMP)updated);
      m = class_getInstanceMethod(cls, @selector(performDragOperation:));
      original_perform = (void *)method_setImplementation(m, (IMP)perform);
    });
}
