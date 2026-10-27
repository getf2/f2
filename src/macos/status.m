// The menu bar item: Finder2 keeps running (⌘Q only closes its windows),
// and this is where it's opened from and quit. Choosing an item calls back
// with its tag. Also Open at Login, through ServiceManagement.

#import <AppKit/AppKit.h>
#import <ServiceManagement/ServiceManagement.h>

typedef struct {
    const char *title;   // NULL: a separator
    const char *symbol;  // an SF Symbol shown before the title, or NULL
    int32_t tag;         // passed back when chosen; -1: Open at Login
} nf_status_entry;

typedef void (*nf_status_chosen)(int32_t tag);

static bool login_items_supported(void) {
    if (@available(macOS 13.0, *)) {
        return true;
    }
    return false;
}

static bool login_item_on(void) {
    if (@available(macOS 13.0, *)) {
        return SMAppService.mainAppService.status == SMAppServiceStatusEnabled;
    }
    return false;
}

@interface NFStatusTarget : NSObject <NSMenuDelegate>
@property nf_status_chosen chosen;
@end

@implementation NFStatusTarget
- (void)choose:(NSMenuItem *)item {
    if (item.tag == -1) {
        if (@available(macOS 13.0, *)) {
            NSError *error = nil;
            SMAppService *app = SMAppService.mainAppService;
            BOOL done = login_item_on() ? [app unregisterAndReturnError:&error] : [app registerAndReturnError:&error];
            if (!done) {
                NSLog(@"Open at Login: %@", error);
            }
        }
        return;
    }
    self.chosen((int32_t)item.tag);
}

// Open at Login's check mark, as it is now (System Settings can change it).
- (void)menuNeedsUpdate:(NSMenu *)menu {
    for (NSMenuItem *item in menu.itemArray) {
        if (item.tag == -1) {
            item.state = login_item_on() ? NSControlStateValueOn : NSControlStateValueOff;
        }
    }
}
@end

static NSStatusItem *status_item;
static NFStatusTarget *status_target;

// A small F2 tile, the app icon's shape: a template image, so the menu bar
// colors it.
static NSImage *status_icon(void) {
    NSImage *image = [NSImage imageWithSize:NSMakeSize(20, 16)
                                    flipped:NO
                             drawingHandler:^BOOL(NSRect rect) {
                               NSRect tile = NSInsetRect(rect, 1.5, 1);
                               [NSColor.blackColor setFill];
                               [[NSBezierPath bezierPathWithRoundedRect:tile xRadius:3.5 yRadius:3.5] fill];
                               NSDictionary *attrs = @{
                                   NSFontAttributeName : [NSFont systemFontOfSize:9.5 weight:NSFontWeightHeavy],
                                   NSForegroundColorAttributeName : NSColor.blackColor,
                                   NSKernAttributeName : @(-0.2),
                               };
                               NSAttributedString *text = [[NSAttributedString alloc] initWithString:@"F2" attributes:attrs];
                               NSSize size = text.size;
                               // Cut out of the tile.
                               NSGraphicsContext.currentContext.compositingOperation = NSCompositingOperationDestinationOut;
                               [text drawAtPoint:NSMakePoint(NSMidX(tile) - size.width / 2, NSMidY(tile) - size.height / 2 + 0.5)];
                               return YES;
                             }];
    image.template = YES;
    return image;
}

void nf_status_item_install(const nf_status_entry *entries, size_t count, nf_status_chosen chosen, const char *tooltip) {
    @autoreleasepool {
        status_target = [NFStatusTarget new];
        status_target.chosen = chosen;
        NSMenu *menu = [NSMenu new];
        menu.autoenablesItems = NO;
        menu.delegate = status_target;
        for (size_t i = 0; i < count; i++) {
            const nf_status_entry *e = &entries[i];
            if (!e->title) {
                [menu addItem:NSMenuItem.separatorItem];
                continue;
            }
            NSMenuItem *item = [[NSMenuItem alloc] initWithTitle:@(e->title) action:@selector(choose:) keyEquivalent:@""];
            item.target = status_target;
            item.tag = e->tag;
            if (e->symbol) {
                item.image = [NSImage imageWithSystemSymbolName:@(e->symbol) accessibilityDescription:nil];
            }
            if (e->tag == -1 && !login_items_supported()) {
                continue;
            }
            [menu addItem:item];
        }
        status_item = [NSStatusBar.systemStatusBar statusItemWithLength:NSVariableStatusItemLength];
        status_item.button.image = status_icon();
        status_item.button.toolTip = @(tooltip);
        status_item.button.accessibilityLabel = @(tooltip);
        status_item.menu = menu;
    }
}

// What's shown beside the icon: a timer's countdown, a cup while the Mac's
// kept awake ("" for nothing). Digits of one width, so it doesn't jiggle.
void nf_status_title(const char *title) {
    @autoreleasepool {
        NSStatusBarButton *button = status_item.button;
        button.imagePosition = NSImageLeft;
        button.font = [NSFont monospacedDigitSystemFontOfSize:NSFont.systemFontSize weight:NSFontWeightRegular];
        button.title = @(title);
    }
}

// Items atop the menu for what's going on (a timer to cancel, keeping awake
// to stop), in place of the last ones: tags of 1000 and up, a separator
// after.
void nf_status_extra(const nf_status_entry *entries, size_t count) {
    @autoreleasepool {
        NSMenu *menu = status_item.menu;
        while (menu.numberOfItems > 0 && [menu itemAtIndex:0].tag >= 1000) [menu removeItemAtIndex:0];
        if (count == 0) return;
        NSMenuItem *separator = NSMenuItem.separatorItem;
        separator.tag = 1000;
        [menu insertItem:separator atIndex:0];
        for (size_t i = count; i-- > 0;) {
            const nf_status_entry *e = &entries[i];
            NSMenuItem *item = [[NSMenuItem alloc] initWithTitle:@(e->title) action:@selector(choose:) keyEquivalent:@""];
            item.target = status_target;
            item.tag = e->tag;
            if (e->symbol) item.image = [NSImage imageWithSystemSymbolName:@(e->symbol) accessibilityDescription:nil];
            [menu insertItem:item atIndex:0];
        }
    }
}

// A system sound ("Glass"), `times` times, a second and a half apart.
void nf_play_sound(const char *name, int times) {
    NSString *sound = @(name);
    for (int i = 0; i < times; i++) {
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(i * 1.5 * NSEC_PER_SEC)), dispatch_get_main_queue(), ^{
          [[NSSound soundNamed:sound] play];
        });
    }
}

// Open at Login, as it is (System Settings can change it).
bool nf_login_item_on(void) {
    return login_item_on();
}

// Open at Login on or off. Whether that worked.
bool nf_login_item_set(bool on) {
    if (@available(macOS 13.0, *)) {
        NSError *error = nil;
        SMAppService *app = SMAppService.mainAppService;
        if (on == login_item_on()) {
            return true;
        }
        BOOL done = on ? [app registerAndReturnError:&error] : [app unregisterAndReturnError:&error];
        if (!done) {
            NSLog(@"Open at Login: %@", error);
        }
        return done;
    }
    return false;
}
