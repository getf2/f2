// A native context menu, run to completion: the call returns when it
// closes, with the tag of the chosen item or -1. And Continuity Camera's
// Import from iPhone or iPad in it (Take Photo, Scan Documents, Add
// Sketch), which saves what comes into a folder.

#import <AppKit/AppKit.h>
#import <objc/message.h>

typedef struct {
    const char *title;   // NULL: a separator
    const char *icon;    // a file whose icon to show (an app), or NULL
    const char *share;   // files to share, one per line: the system's Share… item
    const char *key;     // key equivalent shown, "" for none
    uint64_t modifiers;  // NSEventModifierFlags for `key`
    int32_t tag;         // what the call returns; -1 opens a submenu
    uint8_t depth;       // 0, or 1: in the submenu of the last tag -1 item
    bool enabled;
    bool checked;
    bool alternate;      // shown instead of the item before it while ⌥ is down
    uint8_t swatch;      // a tag color dot (Finder's color numbers), or 0
    const char *import_to; // a folder: the item's Import from iPhone or iPad, into it
} nf_menu_item;

// What Continuity Camera sends (a photo, a scan, a sketch), saved into the
// folder its menu was for: this, put after the key window's first
// responder (GPUI's view), where AppKit looks for what can take an image.
// It stays there (the photo comes after the menu's gone, once it's
// taken), till another menu's for another folder.
@interface NFImporter : NSResponder
@property(copy) NSString *folder;
@end

static void (*imported)(const char *path);

void nf_import_listen(void (*listener)(const char *path)) {
    imported = listener;
}

static NSArray<NSPasteboardType> *import_types(void) {
    return @[NSPasteboardTypePDF, @"public.jpeg", @"public.heic", NSPasteboardTypePNG, NSPasteboardTypeTIFF];
}

// AppKit fills in Continuity Camera's devices (and Take Photo, Scan
// Documents, Add Sketch) only for its item in the menu bar: one's kept
// there, hidden (once GPUI's menu bar is up), and moved into a context
// menu while that's open, as Finder's is.
static NSMenuItem *continuity_item(void) {
    static NSMenuItem *item;
    NSMenu *bar = NSApp.mainMenu;
    if (!bar) return nil;
    if (item && item.menu) return item;
    // (Not there: GPUI's menu bar was set again, or it's the first time.)
    NSMenu *first = bar.itemArray.firstObject.submenu;
    if (!first) return nil;
    item = [[NSMenuItem alloc] initWithTitle:@"Import from iPhone or iPad" action:NSSelectorFromString(@"importFromDevice:") keyEquivalent:@""];
    item.identifier = NSMenuItemImportFromDeviceIdentifier;
    item.hidden = YES;
    [first addItem:item];
    SEL customize = NSSelectorFromString(@"_customizeImportFromDeviceMenuItem");
    if ([NSApp respondsToSelector:customize]) ((void (*)(id, SEL))objc_msgSend)(NSApp, customize);
    return item;
}

void nf_continuity_setup(void) {
    [NSApp registerServicesMenuSendTypes:@[] returnTypes:import_types()];
    continuity_item();
}

@implementation NFImporter
- (id)validRequestorForSendType:(NSPasteboardType)sendType returnType:(NSPasteboardType)returnType {
    if (returnType && ([import_types() containsObject:returnType] || [NSImage.imageTypes containsObject:returnType])) return self;
    return [super validRequestorForSendType:sendType returnType:returnType];
}

- (BOOL)readSelectionFromPasteboard:(NSPasteboard *)pasteboard {
    NSPasteboardType type = [pasteboard availableTypeFromArray:import_types()];
    NSData *data = type ? [pasteboard dataForType:type] : nil;
    if (!data || !self.folder) return NO;
    // (A scan's a PDF; a photo's as it came; a picture without a file
    // type, PNG.)
    NSString *what = [type isEqual:NSPasteboardTypePDF] ? @"Scan" : @"Photo";
    NSString *ext = @"png";
    if ([type isEqual:NSPasteboardTypePDF]) ext = @"pdf";
    else if ([type isEqual:@"public.jpeg"]) ext = @"jpg";
    else if ([type isEqual:@"public.heic"]) ext = @"heic";
    else if ([type isEqual:NSPasteboardTypeTIFF]) {
        NSBitmapImageRep *rep = [NSBitmapImageRep imageRepWithData:data];
        data = [rep representationUsingType:NSBitmapImageFileTypePNG properties:@{}];
        if (!data) return NO;
    }
    NSDateFormatter *when = [NSDateFormatter new];
    when.dateFormat = @"yyyy-MM-dd 'at' HH.mm.ss";
    NSString *name = [NSString stringWithFormat:@"%@ %@", what, [when stringFromDate:NSDate.date]];
    NSString *path = [self.folder stringByAppendingPathComponent:[name stringByAppendingPathExtension:ext]];
    for (int n = 2; [NSFileManager.defaultManager fileExistsAtPath:path]; n++) {
        path = [self.folder stringByAppendingPathComponent:[[NSString stringWithFormat:@"%@ %d", name, n] stringByAppendingPathExtension:ext]];
    }
    if (![data writeToFile:path atomically:YES]) return NO;
    if (imported) imported(path.UTF8String);
    return YES;
}
@end

static NFImporter *importer;

// Put the importer after the key window's first responder, for `folder`.
static void import_into(NSString *folder) {
    NSResponder *first = NSApp.keyWindow.firstResponder;
    if (!first) return;
    if (!importer) importer = [NFImporter new];
    // (Out of wherever it was first.)
    for (NSResponder *r = first; r; r = r.nextResponder) {
        if (r.nextResponder == importer) {
            r.nextResponder = importer.nextResponder;
            break;
        }
    }
    importer.folder = folder;
    importer.nextResponder = first.nextResponder;
    first.nextResponder = importer;
}

// Finder's tag colors by number: gray, green, purple, blue, yellow, red, orange.
static NSColor *tag_color(uint8_t n) {
    switch (n) {
        case 1: return NSColor.systemGrayColor;
        case 2: return NSColor.systemGreenColor;
        case 3: return NSColor.systemPurpleColor;
        case 4: return NSColor.systemBlueColor;
        case 5: return NSColor.systemYellowColor;
        case 6: return NSColor.systemRedColor;
        case 7: return NSColor.systemOrangeColor;
        default: return nil;
    }
}

static NSImage *dot(NSColor *color) {
    return [NSImage imageWithSize:NSMakeSize(12, 12)
                          flipped:NO
                   drawingHandler:^BOOL(NSRect rect) {
                     [color setFill];
                     [[NSBezierPath bezierPathWithOvalInRect:NSInsetRect(rect, 1, 1)] fill];
                     return YES;
                   }];
}

// The picker behind a Share… item must outlive the menu: it shows the
// share sheet after the menu closes.
static NSSharingServicePicker *share_picker;

@interface NFMenuTarget : NSObject <NSMenuItemValidation>
@property NSInteger chosen;
/// The items' tags that are disabled (menus validate their items: it's
/// then that Continuity Camera's fill in).
@property NSMutableSet<NSNumber *> *disabled;
@end

@implementation NFMenuTarget
- (void)choose:(NSMenuItem *)item {
    self.chosen = item.tag;
}

- (BOOL)validateMenuItem:(NSMenuItem *)item {
    return ![self.disabled containsObject:@(item.tag)];
}
@end

int32_t nf_popup_menu(const nf_menu_item *items, size_t count) {
    @autoreleasepool {
        NFMenuTarget *target = [NFMenuTarget new];
        target.chosen = -1;
        target.disabled = [NSMutableSet set];
        NSMenuItem *moved = nil;
        NSMenu *moved_from = nil;
        NSInteger moved_at = 0;
        NSMenu *root = [[NSMenu alloc] initWithTitle:@""];
        NSMenu *submenu = root;
        for (size_t i = 0; i < count; i++) {
            const nf_menu_item *it = &items[i];
            NSMenu *menu = it->depth ? submenu : root;
            if (!it->title) {
                [menu addItem:NSMenuItem.separatorItem];
                continue;
            }
            if (it->import_to) {
                // (The menu bar's, moved in while this is open: see
                // `continuity_item`.)
                NSMenuItem *item = continuity_item();
                if (!item) continue;
                import_into(@(it->import_to));
                moved = item;
                moved_from = item.menu;
                moved_at = [moved_from indexOfItem:item];
                [moved_from removeItem:item];
                item.hidden = NO;
                [menu addItem:item];
                continue;
            }
            if (it->share) {
                NSMutableArray<NSURL *> *urls = [NSMutableArray array];
                for (NSString *p in [@(it->share) componentsSeparatedByString:@"\n"]) [urls addObject:[NSURL fileURLWithPath:p]];
                share_picker = [[NSSharingServicePicker alloc] initWithItems:urls];
                [menu addItem:share_picker.standardShareMenuItem];
                continue;
            }
            NSMenuItem *item = [[NSMenuItem alloc] initWithTitle:@(it->title) action:nil keyEquivalent:@(it->key)];
            item.keyEquivalentModifierMask = it->modifiers;
            item.enabled = it->enabled;
            item.state = it->checked ? NSControlStateValueOn : NSControlStateValueOff;
            item.alternate = it->alternate;
            NSColor *swatch = tag_color(it->swatch);
            if (swatch) item.image = dot(swatch);
            if (it->icon) {
                NSImage *icon = [[NSWorkspace.sharedWorkspace iconForFile:@(it->icon)] copy];
                icon.size = NSMakeSize(16, 16);
                item.image = icon;
            }
            if (it->tag < 0) {
                submenu = [[NSMenu alloc] initWithTitle:@(it->title)];
                item.submenu = submenu;
            } else {
                item.tag = it->tag;
                item.target = target;
                item.action = @selector(choose:);
                if (!it->enabled) [target.disabled addObject:@(it->tag)];
            }
            [menu addItem:item];
        }
        // (Over the key window's view: Continuity Camera's items look for
        // what can take an image from it, along its responder chain.)
        NSView *view = [NSApp.keyWindow.firstResponder isKindOfClass:NSView.class] ? (NSView *)NSApp.keyWindow.firstResponder : NSApp.keyWindow.contentView;
        if (view) {
            NSPoint at = [view convertPoint:[view.window convertPointFromScreen:NSEvent.mouseLocation] fromView:nil];
            [root popUpMenuPositioningItem:nil atLocation:at inView:view];
        } else {
            [root popUpMenuPositioningItem:nil atLocation:NSEvent.mouseLocation inView:nil];
        }
        // (Back where it was, hidden.)
        if (moved) {
            [moved.menu removeItem:moved];
            moved.hidden = YES;
            [moved_from insertItem:moved atIndex:MIN(moved_at, moved_from.numberOfItems)];
        }
        return (int32_t)target.chosen;
    }
}
