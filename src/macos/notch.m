// The notch: an island that's the notch itself, on a screen with one, and
// grows out of it, as the iPhone's Dynamic Island does. Beside it, what's
// going on (a timer counting down, dictation listening, what's playing);
// notes come down from it; under the pointer, it opens to show all of that
// and the last notes. A screen without a notch gets one, hanging from the
// menu bar, only while there's something to show (if Settings says so).
//
// Core Animation draws it, in a window over the menu bar that's clear round
// the island: clicks go through there. While it's still nothing runs (but
// what's playing's bars, which Core Animation moves by itself), and its
// window is no bigger than the notch.

#import <AppKit/AppKit.h>
#import <QuartzCore/QuartzCore.h>

uint32_t nf_screen_with_pointer(void);
float nf_mic_level(void);

// (dictation.m's: its pill's bars, drawn here too.)
@interface NFDictationBars : NSView
@property(nonatomic) int mode;
@property(nonatomic) float *levels;
@property(nonatomic) double phase;
@end

enum { HudHidden = 0, HudListening = 1, HudHandsFree = 2, HudWorking = 3 };
enum { DictationBars = 9 };

typedef enum { IslandHidden, IslandClosed, IslandCompact, IslandBanner, IslandOpen } IslandState;

// What's chosen in it (see notch.rs): a thing going on stopped (its id),
// play or pause, the next song or the one before, the player shown.
enum { ChoseStop = 1, ChosePlayPause = 2, ChoseNext = 3, ChosePrevious = 4, ChosePlayer = 5 };

// Its size, and its corners: rounded below, flaring out where it meets the
// screen's top edge, as the notch does.
typedef struct {
    CGFloat w, h, radius, shoulder;
    float shadow;
} IslandShape;

// Room round it, in its window, for its shadow (clicks go through it).
static const CGFloat kShadowRoom = 36;
static const CGFloat kBannerWidth = 380, kOpenWidth = 400, kTallest = 470;
static const CGFloat kWideRadius = 22;
// How long a note stays, or when another's waiting.
static const NSTimeInterval kStays = 4.5, kQueued = 1.8;
static const NSUInteger kRecent = 3, kGoing = 6;

static void (*notchChose)(int32_t what, int64_t id);
static bool notchOn, notchBare;
// From F2: what's going on ("live": the one beside the notch; "going": all
// of them, to stop; "music": what's playing).
static NSDictionary *notchModel;
// The notes shown, newest first, with when.
static NSMutableArray<NSDictionary *> *notchRecent;
static int notchDictation;
static uint32_t notchDictationScreen;
static NSMutableDictionary<NSNumber *, id> *notchIslands;
static NSTimer *notchBarsTimer;
static float notchLevels[DictationBars];
static __weak NFDictationBars *notchBarsView;

static uint32_t nf_island_screen_id(NSScreen *screen) {
    return [screen.deviceDescription[@"NSScreenNumber"] unsignedIntValue];
}

static NSScreen *nf_island_screen(uint32_t id) {
    for (NSScreen *screen in NSScreen.screens) {
        if (nf_island_screen_id(screen) == id) return screen;
    }
    return nil;
}

// The notch on `screen`, in screen coordinates, if it has one.
static bool nf_island_notch(NSScreen *screen, NSRect *out) {
    if (@available(macOS 12.0, *)) {
        NSRect left = screen.auxiliaryTopLeftArea, right = screen.auxiliaryTopRightArea;
        CGFloat top = screen.safeAreaInsets.top;
        CGFloat width = NSWidth(screen.frame) - NSWidth(left) - NSWidth(right);
        if (top > 0 && !NSIsEmptyRect(left) && !NSIsEmptyRect(right) && width > 0) {
            *out = NSMakeRect(NSMinX(screen.frame) + NSWidth(left), NSMaxY(screen.frame) - top, width, top);
            return true;
        }
    }
    return false;
}

// Where it lives: the screen with the notch, else (if screens without one
// get one) the main screen. 0: none.
static uint32_t nf_island_home(void) {
    NSRect notch;
    for (NSScreen *screen in NSScreen.screens) {
        if (nf_island_notch(screen, &notch)) return nf_island_screen_id(screen);
    }
    return notchBare && NSScreen.screens.count ? nf_island_screen_id(NSScreen.screens.firstObject) : 0;
}

static bool nf_island_can_show(NSScreen *screen) {
    NSRect notch;
    return screen && (notchBare || nf_island_notch(screen, &notch));
}

static NSColor *nf_island_rgb(uint32_t c) {
    return [NSColor colorWithSRGBRed:((c >> 16) & 255) / 255.0 green:((c >> 8) & 255) / 255.0 blue:(c & 255) / 255.0 alpha:1];
}

static NSString *nf_island_string(id value) {
    return [value isKindOfClass:NSString.class] ? value : @"";
}

static NSTextField *nf_island_label(NSString *text, CGFloat size, NSFontWeight weight, CGFloat alpha, bool digits) {
    NSTextField *label = [NSTextField labelWithString:text ?: @""];
    label.font = digits ? [NSFont monospacedDigitSystemFontOfSize:size weight:weight] : [NSFont systemFontOfSize:size weight:weight];
    label.textColor = [NSColor colorWithWhite:1 alpha:alpha];
    label.lineBreakMode = NSLineBreakByTruncatingTail;
    label.maximumNumberOfLines = 1;
    [label sizeToFit];
    return label;
}

// A label up to `lines` lines in `width`, its height fitted.
static NSTextField *nf_island_wrapped(NSString *text, CGFloat size, NSFontWeight weight, CGFloat alpha, CGFloat width, NSInteger lines) {
    NSTextField *label = [NSTextField wrappingLabelWithString:text ?: @""];
    label.font = [NSFont systemFontOfSize:size weight:weight];
    label.textColor = [NSColor colorWithWhite:1 alpha:alpha];
    label.maximumNumberOfLines = lines;
    // (Truncating the tail would make it one line: wrapped, its last line
    // cut short.)
    label.lineBreakMode = NSLineBreakByWordWrapping;
    label.cell.truncatesLastVisibleLine = YES;
    label.preferredMaxLayoutWidth = width;
    label.selectable = NO;
    NSFont *font = label.font;
    CGFloat line = ceil(font.ascender - font.descender + font.leading);
    CGFloat height = [label.cell cellSizeForBounds:NSMakeRect(0, 0, width, CGFLOAT_MAX)].height;
    label.frame = NSMakeRect(0, 0, width, ceil(MIN(height, line * lines + 2)));
    return label;
}

static NSImageView *nf_island_symbol(NSString *name, CGFloat size, NSColor *tint) {
    NSImage *image = [NSImage imageWithSystemSymbolName:name accessibilityDescription:nil] ?: [NSImage imageWithSystemSymbolName:@"circle.fill" accessibilityDescription:nil];
    image = [image imageWithSymbolConfiguration:[NSImageSymbolConfiguration configurationWithPointSize:size weight:NSFontWeightSemibold]];
    NSImageView *view = [NSImageView imageViewWithImage:image];
    view.contentTintColor = tint;
    return view;
}

// A picture, `size` square, rounded.
static NSImageView *nf_island_picture(NSImage *image, CGFloat size, CGFloat radius) {
    NSImageView *view = [[NSImageView alloc] initWithFrame:NSMakeRect(0, 0, size, size)];
    view.image = image;
    view.imageScaling = NSImageScaleProportionallyUpOrDown;
    view.wantsLayer = YES;
    view.layer.cornerRadius = radius;
    view.layer.masksToBounds = YES;
    return view;
}

// The artwork's color, bright enough for black.
static NSColor *nf_island_tint(NSImage *image) {
    if (!image) return [NSColor colorWithWhite:1 alpha:0.9];
    enum { Side = 8 };
    NSBitmapImageRep *rep = [[NSBitmapImageRep alloc] initWithBitmapDataPlanes:NULL pixelsWide:Side pixelsHigh:Side bitsPerSample:8 samplesPerPixel:4 hasAlpha:YES isPlanar:NO colorSpaceName:NSDeviceRGBColorSpace bytesPerRow:Side * 4 bitsPerPixel:32];
    [NSGraphicsContext saveGraphicsState];
    NSGraphicsContext.currentContext = [NSGraphicsContext graphicsContextWithBitmapImageRep:rep];
    [image drawInRect:NSMakeRect(0, 0, Side, Side)];
    [NSGraphicsContext restoreGraphicsState];
    double r = 0, g = 0, b = 0;
    unsigned char *px = rep.bitmapData;
    for (int i = 0; i < Side * Side; i++) {
        r += px[i * 4];
        g += px[i * 4 + 1];
        b += px[i * 4 + 2];
    }
    NSColor *mean = [NSColor colorWithSRGBRed:r / (Side * Side * 255.0) green:g / (Side * Side * 255.0) blue:b / (Side * Side * 255.0) alpha:1];
    CGFloat h, s, v, a;
    [mean getHue:&h saturation:&s brightness:&v alpha:&a];
    return [NSColor colorWithHue:h saturation:MIN(s, 0.75) brightness:MAX(v, 0.85) alpha:1];
}

// "Now", "4 min", "2 h".
static NSString *nf_island_ago(NSDate *at) {
    NSTimeInterval secs = -at.timeIntervalSinceNow;
    if (secs < 60) return @"Now";
    if (secs < 3600) return [NSString stringWithFormat:@"%d min", (int)(secs / 60)];
    return [NSString stringWithFormat:@"%d h", (int)(secs / 3600)];
}

// ---- Its parts.

// Its black: the view's layer is the shape.
@interface NFIslandShape : NSView
@end

@implementation NFIslandShape
- (CALayer *)makeBackingLayer {
    return [CAShapeLayer layer];
}
- (NSView *)hitTest:(__unused NSPoint)point {
    return nil;
}
@end

// What's in it, cut to its shape: only its buttons take clicks (the rest
// are the island's own).
@interface NFIslandClip : NSView
@end

@implementation NFIslandClip
- (NSView *)hitTest:(NSPoint)point {
    NSView *hit = [super hitTest:point];
    return hit == self ? nil : hit;
}
@end

@interface NFIslandButton : NSView
@property(nonatomic, copy) void (^action)(void);
// Pointed at, or not: as it changes.
@property(nonatomic, copy) void (^hover)(bool in);
@property(nonatomic) bool pointed;
@property(nonatomic, strong) NSTrackingArea *tracking;
// Its background when it isn't pointed at.
@property(nonatomic, strong) NSColor *resting;
+ (instancetype)symbol:(NSString *)name size:(CGFloat)size box:(CGFloat)box alpha:(CGFloat)alpha action:(void (^)(void))action;
@end

@implementation NFIslandButton
+ (instancetype)symbol:(NSString *)name size:(CGFloat)size box:(CGFloat)box alpha:(CGFloat)alpha action:(void (^)(void))action {
    NFIslandButton *button = [[NFIslandButton alloc] initWithFrame:NSMakeRect(0, 0, box, box)];
    button.wantsLayer = YES;
    button.layer.cornerRadius = box / 2;
    NSImageView *icon = nf_island_symbol(name, size, [NSColor colorWithWhite:1 alpha:alpha]);
    [icon setFrameOrigin:NSMakePoint(round((box - NSWidth(icon.frame)) / 2), round((box - NSHeight(icon.frame)) / 2))];
    [button addSubview:icon];
    button.action = action;
    button.accessibilityRole = NSAccessibilityButtonRole;
    return button;
}
// (Laid out from the top, as the content it's in is.)
- (BOOL)isFlipped {
    return YES;
}
- (BOOL)acceptsFirstMouse:(__unused NSEvent *)event {
    return YES;
}
- (NSView *)hitTest:(NSPoint)point {
    if (!NSPointInRect(point, self.frame)) return nil;
    // (A button in it, as a note's ×, takes its own clicks.)
    NSPoint inside = [self convertPoint:point fromView:self.superview];
    for (NSView *view in self.subviews.reverseObjectEnumerator) {
        NSView *hit = [view isKindOfClass:NFIslandButton.class] && !view.hidden ? [view hitTest:inside] : nil;
        if (hit) return hit;
    }
    return self;
}
- (void)updateTrackingAreas {
    [super updateTrackingAreas];
    if (self.tracking) [self removeTrackingArea:self.tracking];
    self.tracking = [[NSTrackingArea alloc] initWithRect:self.bounds options:NSTrackingMouseEnteredAndExited | NSTrackingActiveAlways owner:self userInfo:nil];
    [self addTrackingArea:self.tracking];
    // (Come in under the pointer, as the next note does when one's
    // dismissed: no entered for that.)
    if (self.window) [self point:NSPointInRect([self convertPoint:self.window.mouseLocationOutsideOfEventStream fromView:nil], self.bounds)];
}
- (void)point:(bool)pointed {
    if (pointed == self.pointed) return;
    self.pointed = pointed;
    self.layer.backgroundColor = pointed && self.action ? [NSColor colorWithWhite:1 alpha:0.14].CGColor : self.resting.CGColor;
    if (self.hover) self.hover(pointed);
}
- (void)mouseEntered:(__unused NSEvent *)event {
    [self point:YES];
}
- (void)mouseExited:(__unused NSEvent *)event {
    [self point:NO];
}
- (void)mouseDown:(__unused NSEvent *)event {
    if (self.action) self.alphaValue = 0.55;
}
- (void)mouseUp:(NSEvent *)event {
    self.alphaValue = 1;
    if (NSPointInRect([self convertPoint:event.locationInWindow fromView:nil], self.bounds) && self.action) self.action();
}
@end

// Laid out top down.
@interface NFIslandContent : NSView
@end

@implementation NFIslandContent
- (BOOL)isFlipped {
    return YES;
}
@end

// What's playing's bars, moved by Core Animation (nothing here runs).
@interface NFMusicBars : NSView
- (instancetype)initWithFrame:(NSRect)frame color:(NSColor *)color playing:(bool)playing;
@end

@implementation NFMusicBars
- (instancetype)initWithFrame:(NSRect)frame color:(NSColor *)color playing:(bool)playing {
    self = [super initWithFrame:frame];
    self.layer = [CALayer layer];
    self.wantsLayer = YES;
    enum { Count = 4 };
    CGFloat width = 3, gap = 2.5;
    CGFloat x = (NSWidth(frame) - (Count * width + (Count - 1) * gap)) / 2;
    for (int i = 0; i < Count; i++) {
        CALayer *bar = [CALayer layer];
        bar.backgroundColor = color.CGColor;
        bar.cornerRadius = width / 2;
        bar.frame = CGRectMake(x + i * (width + gap), 0, width, NSHeight(frame));
        [self.layer addSublayer:bar];
        if (playing) {
            CABasicAnimation *bounce = [CABasicAnimation animationWithKeyPath:@"transform.scale.y"];
            bounce.fromValue = @0.22;
            bounce.toValue = @1.0;
            bounce.duration = 0.3 + 0.09 * ((i * 5) % 4);
            bounce.autoreverses = YES;
            bounce.repeatCount = HUGE_VALF;
            bounce.timeOffset = 0.17 * i;
            bounce.timingFunction = [CAMediaTimingFunction functionWithName:kCAMediaTimingFunctionEaseInEaseOut];
            [bar addAnimation:bounce forKey:@"bounce"];
        } else {
            bar.transform = CATransform3DMakeScale(1, 0.22, 1);
        }
    }
    return self;
}
@end

@class NFIsland;

@interface NFIslandView : NSView
@property(nonatomic, weak) NFIsland *island;
@property(nonatomic, strong) NSTrackingArea *tracking;
@end

// One screen's.
@interface NFIsland : NSObject
@property(nonatomic) uint32_t screenId;
@property(nonatomic, strong) NSPanel *panel;
@property(nonatomic, strong) NFIslandView *view;
@property(nonatomic, strong) CAShapeLayer *shape;
@property(nonatomic, strong) CAShapeLayer *mask;
@property(nonatomic, strong) NFIslandClip *clip;
@property(nonatomic, strong) NSView *content;
// The notch (screen coordinates), and the middle of the screen's top the
// island hangs from.
@property(nonatomic) bool hasNotch;
@property(nonatomic) NSRect notch;
@property(nonatomic) CGFloat anchor, top, bar;
@property(nonatomic) bool big;
@property(nonatomic) IslandState state;
@property(nonatomic) IslandShape now;
// What it shows: its kind (another fades in), and all of it (the same:
// left as it is).
@property(nonatomic, copy) NSString *kind, *showing;
@property(nonatomic, strong) NSMutableArray<NSDictionary *> *notes;
@property(nonatomic, strong) NSDictionary *note;
@property(nonatomic) NSTimeInterval noteAt, outsideSince;
@property(nonatomic, strong) NSTimer *noteTimer, *hoverTimer, *pollTimer;
@property(nonatomic) bool open;
@property(nonatomic) NSUInteger generation;
@end


@interface NFIsland ()
- (void)entered;
- (void)exited;
- (void)clicked;
- (void)track;
@end

@implementation NFIslandView
- (BOOL)acceptsFirstMouse:(__unused NSEvent *)event {
    return YES;
}
- (void)mouseDown:(__unused NSEvent *)event {
}
- (void)mouseUp:(__unused NSEvent *)event {
    [self.island clicked];
}
- (void)mouseEntered:(__unused NSEvent *)event {
    [self.island entered];
}
- (void)mouseExited:(__unused NSEvent *)event {
    [self.island exited];
}
- (void)updateTrackingAreas {
    [super updateTrackingAreas];
    [self.island track];
}
@end

static CGPathRef nf_island_path(IslandShape s, CGFloat cx, CGFloat top) CF_RETURNS_RETAINED {
    // (Always the same parts, so one animates into another.)
    CGMutablePathRef p = CGPathCreateMutable();
    const CGFloat k = 0.5523;
    CGFloat l = cx - s.w / 2, r = cx + s.w / 2, b = top - s.h;
    CGFloat sh = MAX(0, MIN(s.shoulder, s.h / 2));
    CGFloat rad = MAX(0, MIN(s.radius, MIN(s.w / 2, s.h - sh)));
    CGPathMoveToPoint(p, NULL, l - sh, top);
    CGPathAddCurveToPoint(p, NULL, l - sh + sh * k, top, l, top - sh + sh * k, l, top - sh);
    CGPathAddLineToPoint(p, NULL, l, b + rad);
    CGPathAddCurveToPoint(p, NULL, l, b + rad - rad * k, l + rad - rad * k, b, l + rad, b);
    CGPathAddLineToPoint(p, NULL, r - rad, b);
    CGPathAddCurveToPoint(p, NULL, r - rad + rad * k, b, r, b + rad - rad * k, r, b + rad);
    CGPathAddLineToPoint(p, NULL, r, top - sh);
    CGPathAddCurveToPoint(p, NULL, r, top - sh + sh * k, r + sh - sh * k, top, r + sh, top);
    CGPathCloseSubpath(p);
    return p;
}

static bool nf_island_same(IslandShape a, IslandShape b) {
    return a.w == b.w && a.h == b.h && a.radius == b.radius && a.shoulder == b.shoulder && a.shadow == b.shadow;
}

static void nf_island_all(void);
static void nf_island_forget(NSDictionary *note);

@implementation NFIsland

- (instancetype)initWithScreen:(uint32_t)screen {
    self = [super init];
    _screenId = screen;
    _notes = [NSMutableArray array];
    _state = IslandHidden;
    _panel = [[NSPanel alloc] initWithContentRect:NSMakeRect(0, 0, 10, 10) styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel backing:NSBackingStoreBuffered defer:NO];
    _panel.level = NSMainMenuWindowLevel + 3;
    _panel.opaque = NO;
    _panel.backgroundColor = NSColor.clearColor;
    _panel.hasShadow = NO;
    _panel.hidesOnDeactivate = NO;
    _panel.releasedWhenClosed = NO;
    _panel.movable = NO;
    _panel.animationBehavior = NSWindowAnimationBehaviorNone;
    _panel.collectionBehavior = NSWindowCollectionBehaviorCanJoinAllSpaces | NSWindowCollectionBehaviorStationary | NSWindowCollectionBehaviorFullScreenAuxiliary | NSWindowCollectionBehaviorIgnoresCycle;
    _panel.appearance = [NSAppearance appearanceNamed:NSAppearanceNameDarkAqua];
    _panel.accessibilityLabel = @"Notch";
    _view = [[NFIslandView alloc] initWithFrame:NSMakeRect(0, 0, 10, 10)];
    _view.wantsLayer = YES;
    _view.island = self;
    NFIslandShape *shapeView = [[NFIslandShape alloc] initWithFrame:_view.bounds];
    shapeView.wantsLayer = YES;
    shapeView.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
    [_view addSubview:shapeView];
    _shape = (CAShapeLayer *)shapeView.layer;
    _shape.fillColor = NSColor.blackColor.CGColor;
    _shape.shadowColor = NSColor.blackColor.CGColor;
    _shape.shadowOpacity = 0;
    _shape.shadowRadius = 16;
    _shape.shadowOffset = CGSizeMake(0, -6);
    _clip = [[NFIslandClip alloc] initWithFrame:_view.bounds];
    _clip.wantsLayer = YES;
    _clip.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
    _mask = [CAShapeLayer layer];
    _mask.fillColor = NSColor.blackColor.CGColor;
    _clip.layer.mask = _mask;
    [_view addSubview:_clip];
    _panel.contentView = _view;
    [self place];
    return self;
}

- (void)close {
    [self.noteTimer invalidate];
    [self.hoverTimer invalidate];
    [self.pollTimer invalidate];
    self.noteTimer = self.hoverTimer = self.pollTimer = nil;
    self.view.island = nil;
    [self.panel orderOut:nil];
    [self.panel close];
}

// Its screen's notch, and where the island hangs from: after screens change.
- (void)place {
    NSScreen *screen = nf_island_screen(self.screenId);
    if (!screen) return;
    NSRect notch = NSZeroRect;
    self.hasNotch = nf_island_notch(screen, &notch);
    self.notch = notch;
    self.anchor = self.hasNotch ? NSMidX(notch) : NSMidX(screen.frame);
    self.top = NSMaxY(screen.frame);
    CGFloat bar = NSMaxY(screen.frame) - NSMaxY(screen.visibleFrame);
    self.bar = bar > 0 && bar < 60 ? bar : 24;
    [self setBig:self.big force:YES];
}

// Inside the notch (the screen has no pixels there): where it's pointed at.
- (IslandShape)closedShape {
    return (IslandShape){NSWidth(self.notch) - 6, NSHeight(self.notch) - 3, 9, 0, 0};
}

// Tucked into the top edge, before it comes out (or after it's gone).
- (IslandShape)tuckedShape {
    return (IslandShape){MAX(self.now.w * 0.6, 60), 0, 0, 0, 0};
}

// Its window: big enough for it open (and its shadow), or, while it's
// closed, the notch's size.
- (NSRect)frame:(bool)big {
    if (!big) {
        IslandShape closed = [self closedShape];
        CGFloat w = ceil(closed.w);
        return NSMakeRect(round(self.anchor - w / 2), self.top - closed.h, w, closed.h);
    }
    NSScreen *screen = nf_island_screen(self.screenId);
    CGFloat w = MIN(NSWidth(screen.frame), MAX(kOpenWidth, kBannerWidth) + 2 * kShadowRoom + 40);
    return NSMakeRect(round(self.anchor - w / 2), self.top - kTallest - kShadowRoom, w, kTallest + kShadowRoom);
}

- (CGFloat)middle {
    return self.anchor - NSMinX(self.panel.frame);
}

- (NSRect)shapeRect {
    IslandShape s = self.now;
    CGFloat sh = MIN(s.shoulder, s.h / 2);
    return NSMakeRect(self.middle - s.w / 2 - sh, NSHeight(self.view.bounds) - s.h, s.w + 2 * sh, s.h);
}

- (void)setBig:(bool)big force:(bool)force {
    if (big == self.big && !force) return;
    self.big = big;
    [self.panel setFrame:[self frame:big] display:NO];
    // (The same shape, where it now is in the window: not animated.)
    [CATransaction begin];
    [CATransaction setDisableActions:YES];
    self.mask.frame = self.clip.bounds;
    CGPathRef path = nf_island_path(self.now, self.middle, NSHeight(self.view.bounds));
    for (CAShapeLayer *layer in @[self.shape, self.mask]) {
        [layer removeAnimationForKey:@"path"];
        layer.path = path;
    }
    [self.shape removeAnimationForKey:@"shadowPath"];
    self.shape.shadowPath = path;
    CGPathRelease(path);
    [CATransaction commit];
    self.content.frame = [self contentRect];
    [self track];
}

- (NSRect)contentRect {
    return NSMakeRect(round(self.middle - self.now.w / 2), NSHeight(self.view.bounds) - self.now.h, self.now.w, self.now.h);
}

// Pointed at: on it (or the notch), give or take `slack`.
- (bool)pointerInside:(CGFloat)slack {
    NSPoint at = NSEvent.mouseLocation;
    NSRect shape = NSOffsetRect([self shapeRect], NSMinX(self.panel.frame), NSMinY(self.panel.frame));
    return NSPointInRect(at, NSInsetRect(shape, -slack, -slack)) || (self.hasNotch && NSPointInRect(at, NSInsetRect(self.notch, -slack, -slack)));
}

- (void)track {
    if (!self.view) return;
    if (self.view.tracking) [self.view removeTrackingArea:self.view.tracking];
    NSRect rect = [self shapeRect];
    if (self.hasNotch) rect = NSUnionRect(rect, NSIntersectionRect(NSOffsetRect(self.notch, -NSMinX(self.panel.frame), -NSMinY(self.panel.frame)), self.view.bounds));
    self.view.tracking = [[NSTrackingArea alloc] initWithRect:rect options:NSTrackingMouseEnteredAndExited | NSTrackingActiveAlways owner:self.view userInfo:nil];
    [self.view addTrackingArea:self.view.tracking];
}

// ---- What it shows.

- (IslandState)resting {
    if (self.open) return IslandOpen;
    if (self.note) return IslandBanner;
    if ([self live]) return IslandCompact;
    return self.hasNotch ? IslandClosed : IslandHidden;
}

// What's beside the notch: dictation (on this screen), else (on the home
// screen) a timer, the stopwatch, recording, or what's playing.
- (NSDictionary *)live {
    if (notchDictation != HudHidden && notchDictationScreen == self.screenId) return @{@"dictation": @(notchDictation)};
    if (self.screenId != nf_island_home()) return nil;
    NSDictionary *live = notchModel[@"live"];
    if ([live isKindOfClass:NSDictionary.class]) return live;
    NSDictionary *music = notchModel[@"music"];
    if (self.hasNotch && [music isKindOfClass:NSDictionary.class] && [music[@"playing"] boolValue]) return @{@"music": music};
    return nil;
}

- (void)go {
    IslandState state = [self resting];
    if (state == IslandHidden) {
        [self hide];
        return;
    }
    IslandShape shape = {0};
    NSString *kind = nil, *showing = nil;
    NSView *content = nil;
    switch (state) {
    case IslandClosed:
        shape = [self closedShape];
        kind = showing = @"closed";
        break;
    case IslandCompact:
        content = [self compact:[self live] shape:&shape kind:&kind showing:&showing];
        break;
    case IslandBanner:
        content = [self banner:self.note shape:&shape];
        kind = showing = [NSString stringWithFormat:@"banner %p", (void *)self.note];
        break;
    case IslandOpen:
        content = [self opened:&shape showing:&showing];
        kind = @"open";
        break;
    case IslandHidden:
        return;
    }
    self.state = state;
    if (self.panel.isVisible && [showing isEqualToString:self.showing] && nf_island_same(shape, self.now)) return;
    if (!self.panel.isVisible) {
        self.now = [self tuckedShape];
        [self setBig:YES force:YES];
        [self.panel orderFrontRegardless];
    } else {
        [self setBig:YES force:NO];
    }
    bool fade = ![kind isEqualToString:self.kind];
    self.kind = kind;
    self.showing = showing;
    [self morph:shape];
    [self show:content fade:fade];
    [self track];
}

- (void)hide {
    self.state = IslandHidden;
    self.kind = self.showing = nil;
    if (!self.panel.isVisible) return;
    [self show:nil fade:YES];
    [self morph:[self tuckedShape]];
}

- (void)morph:(IslandShape)to {
    IslandShape from = self.now;
    self.now = to;
    bool moving = [self.shape animationForKey:@"path"] != nil;
    if (nf_island_same(from, to) && !moving) return;
    bool growing = to.w * to.h > from.w * from.h;
    CGPathRef path = nf_island_path(to, self.middle, NSHeight(self.view.bounds));
    NSUInteger generation = ++self.generation;
    __weak NFIsland *weak = self;
    [CATransaction begin];
    [CATransaction setCompletionBlock:^{
        [weak settled:generation];
    }];
    for (NSString *key in @[@"path", @"shadowPath"]) {
        for (CAShapeLayer *layer in [key isEqual:@"path"] ? @[self.shape, self.mask] : @[self.shape]) {
            CAShapeLayer *shown = (CAShapeLayer *)layer.presentationLayer;
            CGPathRef before = [key isEqual:@"path"] ? (moving && shown.path ? shown.path : layer.path) : (moving && shown.shadowPath ? shown.shadowPath : layer.shadowPath);
            CASpringAnimation *spring = [CASpringAnimation animationWithKeyPath:key];
            spring.fromValue = (__bridge id)before;
            spring.toValue = (__bridge id)path;
            spring.mass = 1;
            spring.stiffness = growing ? 240 : 320;
            spring.damping = growing ? 20 : 34;
            spring.duration = spring.settlingDuration;
            [layer addAnimation:spring forKey:key];
        }
    }
    CABasicAnimation *shadow = [CABasicAnimation animationWithKeyPath:@"shadowOpacity"];
    shadow.fromValue = @(((CAShapeLayer *)self.shape.presentationLayer ?: self.shape).shadowOpacity);
    shadow.toValue = @(to.shadow);
    shadow.duration = 0.3;
    [self.shape addAnimation:shadow forKey:@"shadowOpacity"];
    [CATransaction begin];
    [CATransaction setDisableActions:YES];
    self.shape.path = path;
    self.mask.path = path;
    self.shape.shadowPath = path;
    self.shape.shadowOpacity = to.shadow;
    [CATransaction commit];
    [CATransaction commit];
    CGPathRelease(path);
}

// Done moving: tucked away, it goes; closed, its window's the notch's size.
- (void)settled:(NSUInteger)generation {
    if (generation != self.generation) return;
    if (self.state == IslandHidden) {
        [self.panel orderOut:nil];
    } else if (self.state == IslandClosed) {
        [self setBig:NO force:NO];
    }
}

- (void)show:(NSView *)content fade:(bool)fade {
    NSView *old = self.content;
    self.content = content;
    if (content) {
        content.frame = [self contentRect];
        [self.clip addSubview:content];
    }
    if (!fade) {
        [old removeFromSuperview];
        return;
    }
    if (content) {
        content.alphaValue = 0;
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(0.08 * NSEC_PER_SEC)), dispatch_get_main_queue(), ^{
            [NSAnimationContext runAnimationGroup:^(NSAnimationContext *context) {
                context.duration = 0.22;
                content.animator.alphaValue = 1;
            }];
        });
    }
    if (old) {
        [NSAnimationContext
            runAnimationGroup:^(NSAnimationContext *context) {
                context.duration = 0.1;
                old.animator.alphaValue = 0;
            }
            completionHandler:^{
                [old removeFromSuperview];
            }];
    }
}

- (CGFloat)inset {
    return self.hasNotch ? NSHeight(self.notch) : 4;
}

- (CGFloat)shoulder {
    return self.hasNotch ? 7 : 6;
}

- (CGFloat)widest:(CGFloat)wanted {
    NSScreen *screen = nf_island_screen(self.screenId);
    return MIN(wanted, NSWidth(screen.frame) - 40);
}

// Beside the notch: something on the left, what it says on the right.
- (NSView *)compact:(NSDictionary *)live shape:(IslandShape *)shape kind:(NSString **)kind showing:(NSString **)showing {
    CGFloat h = self.hasNotch ? NSHeight(self.notch) : self.bar + 6;
    CGFloat gap = self.hasNotch ? NSWidth(self.notch) : 10;
    NSColor *white = [NSColor colorWithWhite:1 alpha:0.92];
    NSView *left, *right;
    NSDictionary *music = live[@"music"];
    if (live[@"dictation"]) {
        int mode = [live[@"dictation"] intValue];
        NSString *name = mode == HudHandsFree ? @"lock.fill" : mode == HudWorking ? @"waveform" : @"mic.fill";
        left = nf_island_symbol(name, 13, mode == HudListening ? nf_island_rgb(0xff375f) : white);
        NFDictationBars *bars = [[NFDictationBars alloc] initWithFrame:NSMakeRect(0, 0, 48, MIN(h - 12, 22))];
        bars.levels = notchLevels;
        bars.mode = mode;
        notchBarsView = bars;
        right = bars;
        *kind = @"compact dictation";
        *showing = [NSString stringWithFormat:@"compact dictation %d", mode];
    } else if ([music isKindOfClass:NSDictionary.class]) {
        NSString *art = nf_island_string(music[@"art"]);
        NSImage *image = art.length ? [[NSImage alloc] initWithContentsOfFile:art] : nil;
        left = image ? nf_island_picture(image, MIN(h - 14, 24), 5) : nf_island_symbol(@"music.note", 13, white);
        bool playing = [music[@"playing"] boolValue];
        right = [[NFMusicBars alloc] initWithFrame:NSMakeRect(0, 0, 22, MIN(h - 20, 14)) color:nf_island_tint(image) playing:playing];
        *kind = [NSString stringWithFormat:@"compact music %@ %@ %d", nf_island_string(music[@"title"]), art, playing];
        *showing = *kind;
    } else {
        NSString *name = nf_island_string(live[@"symbol"]);
        uint32_t tint = [live[@"tint"] unsignedIntValue];
        NSColor *color = tint ? nf_island_rgb(tint) : white;
        left = nf_island_symbol(name, 13, color);
        NSString *text = nf_island_string(live[@"text"]);
        NSTextField *label = nf_island_label(text, 13, NSFontWeightSemibold, 1, true);
        label.textColor = color;
        right = label;
        *kind = [@"compact " stringByAppendingString:name];
        *showing = [NSString stringWithFormat:@"%@ %@ %u", *kind, text, tint];
    }
    // (In steps, so a countdown's ears don't twitch as its digits change.)
    CGFloat ear = ceil(MAX(MAX(NSWidth(left.frame), NSWidth(right.frame)) + 22, 38) / 8) * 8;
    CGFloat w = gap + 2 * ear;
    *shape = (IslandShape){w, h, self.hasNotch ? 10 : MIN(12, h / 2), self.shoulder, 0};
    NFIslandContent *content = [[NFIslandContent alloc] initWithFrame:NSMakeRect(0, 0, w, h)];
    [left setFrameOrigin:NSMakePoint(round((ear - NSWidth(left.frame)) / 2) + 3, round((h - NSHeight(left.frame)) / 2))];
    [right setFrameOrigin:NSMakePoint(round(w - ear + (ear - NSWidth(right.frame)) / 2) - 3, round((h - NSHeight(right.frame)) / 2))];
    [content addSubview:left];
    [content addSubview:right];
    return content;
}

// A note's swatch (a color picked), else F2's icon.
- (NSView *)iconFor:(NSDictionary *)note size:(CGFloat)size {
    // (As the corner's: its color, else its symbol on a tile.)
    NSNumber *color = note[@"color"];
    bool swatch = [color isKindOfClass:NSNumber.class];
    NSView *tile = [[NSView alloc] initWithFrame:NSMakeRect(0, 0, size, size)];
    tile.wantsLayer = YES;
    tile.layer.cornerRadius = round(size * 10 / 38);
    tile.layer.backgroundColor = nf_island_rgb(swatch ? color.unsignedIntValue : [note[@"tile"] unsignedIntValue]).CGColor;
    if (swatch) {
        tile.layer.borderWidth = 1;
        tile.layer.borderColor = [NSColor colorWithWhite:1 alpha:0.25].CGColor;
        return tile;
    }
    NSImageView *symbol = nf_island_symbol(nf_island_string(note[@"symbol"]), round(size * 17 / 38), NSColor.whiteColor);
    [symbol setFrameOrigin:NSMakePoint(round((size - NSWidth(symbol.frame)) / 2), round((size - NSHeight(symbol.frame)) / 2))];
    [tile addSubview:symbol];
    return tile;
}

// A note, under the notch: its title and up to two lines more.
- (NSView *)banner:(NSDictionary *)note shape:(IslandShape *)shape {
    CGFloat inset = self.inset, w = [self widest:kBannerWidth], icon = 30, x = 58;
    bool linked = nf_island_string(note[@"link"]).length > 0;
    CGFloat textW = w - x - (linked ? 34 : 20);
    NSTextField *title = nf_island_label(nf_island_string(note[@"title"]), 13, NSFontWeightSemibold, 1, false);
    NSString *more = nf_island_string(note[@"detail"]);
    NSTextField *detail = more.length ? nf_island_wrapped(more, 12, NSFontWeightRegular, 0.68, textW, 2) : nil;
    CGFloat textH = NSHeight(title.frame) + (detail ? 2 + NSHeight(detail.frame) : 0);
    CGFloat body = MAX(icon, textH), h = inset + 12 + body + 14, y = inset + 12 + (body - textH) / 2;
    NFIslandContent *content = [[NFIslandContent alloc] initWithFrame:NSMakeRect(0, 0, w, h)];
    NSView *picture = [self iconFor:note size:icon];
    [picture setFrameOrigin:NSMakePoint(16, inset + 12 + (body - icon) / 2)];
    [content addSubview:picture];
    title.frame = NSMakeRect(x, y, textW, NSHeight(title.frame));
    [content addSubview:title];
    if (detail) {
        [detail setFrameOrigin:NSMakePoint(x, y + NSHeight(title.frame) + 2)];
        [content addSubview:detail];
    }
    if (linked) {
        NSImageView *go = nf_island_symbol(@"chevron.right", 11, [NSColor colorWithWhite:1 alpha:0.35]);
        [go setFrameOrigin:NSMakePoint(w - 18 - NSWidth(go.frame), inset + 12 + (body - NSHeight(go.frame)) / 2)];
        [content addSubview:go];
    }
    *shape = (IslandShape){w, h, kWideRadius, self.shoulder, 0.45};
    return content;
}

- (CGFloat)line:(NSView *)content y:(CGFloat)y width:(CGFloat)w {
    NSView *line = [[NSView alloc] initWithFrame:NSMakeRect(16, y + 5, w - 32, 1)];
    line.wantsLayer = YES;
    line.layer.backgroundColor = [NSColor colorWithWhite:1 alpha:0.1].CGColor;
    [content addSubview:line];
    return y + 11;
}

// What's playing: its artwork (click it for the player), title, artist,
// and the buttons.
- (CGFloat)music:(NSDictionary *)music in:(NSView *)content y:(CGFloat)y width:(CGFloat)w {
    NSString *art = nf_island_string(music[@"art"]);
    NSImage *image = art.length ? [[NSImage alloc] initWithContentsOfFile:art] : nil;
    CGFloat side = 46;
    NFIslandButton *cover = [[NFIslandButton alloc] initWithFrame:NSMakeRect(16, y + 6, side, side)];
    cover.wantsLayer = YES;
    cover.layer.cornerRadius = 8;
    cover.resting = [NSColor colorWithWhite:1 alpha:0.12];
    cover.layer.backgroundColor = cover.resting.CGColor;
    if (image) {
        [cover addSubview:nf_island_picture(image, side, 8)];
    } else {
        NSImageView *note = nf_island_symbol(@"music.note", 18, [NSColor colorWithWhite:1 alpha:0.6]);
        [note setFrameOrigin:NSMakePoint(round((side - NSWidth(note.frame)) / 2), round((side - NSHeight(note.frame)) / 2))];
        [cover addSubview:note];
    }
    cover.action = ^{
        if (notchChose) notchChose(ChosePlayer, 0);
    };
    [content addSubview:cover];
    bool playing = [music[@"playing"] boolValue];
    CGFloat box = 30, right = w - 14;
    NSArray *buttons = @[
        @[@"forward.fill", @14, @(ChoseNext)],
        @[playing ? @"pause.fill" : @"play.fill", @18, @(ChosePlayPause)],
        @[@"backward.fill", @14, @(ChosePrevious)],
    ];
    for (NSArray *b in buttons) {
        int32_t what = [b[2] intValue];
        NFIslandButton *button = [NFIslandButton symbol:b[0] size:[b[1] doubleValue] box:box alpha:0.92 action:^{
            if (notchChose) notchChose(what, 0);
        }];
        right -= box;
        [button setFrameOrigin:NSMakePoint(right, y + 6 + (side - box) / 2)];
        [content addSubview:button];
        right -= 2;
    }
    CGFloat x = 16 + side + 12, textW = right - x - 8;
    NSTextField *title = nf_island_label(nf_island_string(music[@"title"]), 13, NSFontWeightSemibold, 1, false);
    NSTextField *artist = nf_island_label(nf_island_string(music[@"artist"]), 12, NSFontWeightRegular, 0.6, false);
    CGFloat textH = NSHeight(title.frame) + 2 + NSHeight(artist.frame), top = y + 6 + (side - textH) / 2;
    title.frame = NSMakeRect(x, top, textW, NSHeight(title.frame));
    artist.frame = NSMakeRect(x, top + NSHeight(title.frame) + 2, textW, NSHeight(artist.frame));
    [content addSubview:title];
    [content addSubview:artist];
    return y + side + 12;
}

// Something going on: what, how long, and a button to stop it.
- (CGFloat)going:(NSDictionary *)item in:(NSView *)content y:(CGFloat)y width:(CGFloat)w {
    CGFloat row = 30;
    uint32_t tint = [item[@"tint"] unsignedIntValue];
    NSColor *color = tint ? nf_island_rgb(tint) : [NSColor colorWithWhite:1 alpha:0.9];
    NSImageView *icon = nf_island_symbol(nf_island_string(item[@"symbol"]), 13, color);
    [icon setFrameOrigin:NSMakePoint(round(28 - NSWidth(icon.frame) / 2), round(y + (row - NSHeight(icon.frame)) / 2))];
    [content addSubview:icon];
    int64_t id = [item[@"id"] longLongValue];
    NFIslandButton *stop = [NFIslandButton symbol:@"xmark.circle.fill" size:15 box:24 alpha:0.4 action:^{
        if (notchChose) notchChose(ChoseStop, id);
    }];
    stop.accessibilityLabel = @"Stop";
    [stop setFrameOrigin:NSMakePoint(w - 14 - 24, y + (row - 24) / 2)];
    [content addSubview:stop];
    NSTextField *time = nf_island_label(nf_island_string(item[@"text"]), 13, NSFontWeightSemibold, 1, true);
    time.textColor = color;
    [time setFrameOrigin:NSMakePoint(NSMinX(stop.frame) - 6 - NSWidth(time.frame), round(y + (row - NSHeight(time.frame)) / 2))];
    [content addSubview:time];
    NSTextField *title = nf_island_label(nf_island_string(item[@"title"]), 13, NSFontWeightMedium, 0.92, false);
    title.frame = NSMakeRect(46, round(y + (row - NSHeight(title.frame)) / 2), MAX(0, NSMinX(time.frame) - 10 - 46), NSHeight(title.frame));
    [content addSubview:title];
    return y + row;
}

// A note shown lately (click it to open its link, if it has one).
- (CGFloat)recent:(NSDictionary *)note in:(NSView *)content y:(CGFloat)y width:(CGFloat)w {
    CGFloat row = 40;
    NFIslandButton *line = [[NFIslandButton alloc] initWithFrame:NSMakeRect(6, y, w - 12, row)];
    line.wantsLayer = YES;
    line.layer.cornerRadius = 10;
    NSString *link = nf_island_string(note[@"link"]);
    if (link.length) {
        line.action = ^{
            NSURL *url = [NSURL URLWithString:link];
            if (url) [NSWorkspace.sharedWorkspace openURL:url];
        };
    }
    NSView *icon = [self iconFor:note size:20];
    [icon setFrameOrigin:NSMakePoint(12, (row - 20) / 2)];
    [line addSubview:icon];
    NSTextField *ago = nf_island_label(nf_island_ago(note[@"at"]), 11, NSFontWeightRegular, 0.4, false);
    [ago setFrameOrigin:NSMakePoint(NSWidth(line.frame) - 10 - NSWidth(ago.frame), 5)];
    [line addSubview:ago];
    // (Pointed at, it can go: its × where its time was.)
    NFIslandButton *dismiss = [NFIslandButton symbol:@"xmark" size:9 box:20 alpha:0.75 action:^{
        nf_island_forget(note);
    }];
    dismiss.resting = [NSColor colorWithWhite:1 alpha:0.12];
    dismiss.layer.backgroundColor = dismiss.resting.CGColor;
    dismiss.accessibilityLabel = @"Dismiss";
    [dismiss setFrameOrigin:NSMakePoint(NSWidth(line.frame) - 8 - 20, (row - 20) / 2)];
    dismiss.hidden = YES;
    [line addSubview:dismiss];
    line.hover = ^(bool in) {
        ago.hidden = in;
        dismiss.hidden = !in;
    };
    CGFloat x = 42, textW = NSMinX(ago.frame) - 8 - x;
    NSString *more = nf_island_string(note[@"detail"]);
    NSTextField *title = nf_island_label(nf_island_string(note[@"title"]), 12, NSFontWeightSemibold, 0.92, false);
    NSTextField *detail = nf_island_label(more, 11, NSFontWeightRegular, 0.55, false);
    CGFloat textH = NSHeight(title.frame) + (more.length ? 1 + NSHeight(detail.frame) : 0), top = round((row - textH) / 2);
    title.frame = NSMakeRect(x, top, textW, NSHeight(title.frame));
    [line addSubview:title];
    if (more.length) {
        detail.frame = NSMakeRect(x, top + NSHeight(title.frame) + 1, NSMinX(dismiss.frame) - 8 - x, NSHeight(detail.frame));
        [line addSubview:detail];
    }
    [content addSubview:line];
    return y + row;
}

// Open: what's playing, what's going on, the last notes.
- (NSView *)opened:(IslandShape *)shape showing:(NSString **)showing {
    CGFloat w = [self widest:kOpenWidth], y = self.inset + 6;
    NFIslandContent *content = [[NFIslandContent alloc] initWithFrame:NSZeroRect];
    NSMutableString *all = [NSMutableString stringWithString:@"open"];
    bool any = false;
    NSDictionary *music = notchModel[@"music"];
    if ([music isKindOfClass:NSDictionary.class]) {
        y = [self music:music in:content y:y width:w];
        [all appendFormat:@"|%@ %@ %@ %@", music[@"title"], music[@"artist"], music[@"playing"], music[@"art"]];
        any = true;
    }
    NSArray *going = notchModel[@"going"];
    if ([going isKindOfClass:NSArray.class] && going.count) {
        if (any) y = [self line:content y:y width:w];
        for (NSDictionary *item in [going subarrayWithRange:NSMakeRange(0, MIN(going.count, kGoing))]) {
            if (![item isKindOfClass:NSDictionary.class]) continue;
            y = [self going:item in:content y:y width:w];
            [all appendFormat:@"|%@ %@ %@", item[@"id"], item[@"title"], item[@"text"]];
        }
        any = true;
    }
    if (notchRecent.count) {
        if (any) y = [self line:content y:y width:w];
        for (NSDictionary *note in [notchRecent subarrayWithRange:NSMakeRange(0, MIN(notchRecent.count, kRecent))]) {
            y = [self recent:note in:content y:y width:w];
            [all appendFormat:@"|%p %@", (void *)note, nf_island_ago(note[@"at"])];
        }
        any = true;
    }
    if (!any) {
        NSTextField *none = nf_island_label(@"Nothing going on", 12, NSFontWeightMedium, 0.5, false);
        [none setFrameOrigin:NSMakePoint(round((w - NSWidth(none.frame)) / 2), y + 6)];
        [content addSubview:none];
        y += NSHeight(none.frame) + 12;
    }
    CGFloat h = MIN(y + 8, kTallest);
    content.frame = NSMakeRect(0, 0, w, h);
    *shape = (IslandShape){w, h, kWideRadius, self.shoulder, 0.45};
    *showing = all;
    return content;
}

// ---- The pointer, and notes.

- (void)entered {
    if (self.state == IslandBanner) {
        [self.noteTimer invalidate];
        self.noteTimer = nil;
        return;
    }
    if (self.open) {
        self.outsideSince = 0;
        return;
    }
    [self.hoverTimer invalidate];
    __weak NFIsland *weak = self;
    self.hoverTimer = [NSTimer scheduledTimerWithTimeInterval:0.15 repeats:NO block:^(__unused NSTimer *timer) {
        [weak hovered];
    }];
}

- (void)hovered {
    self.hoverTimer = nil;
    if (!self.open && self.state != IslandBanner && [self pointerInside:2]) [self openUp];
}

- (void)exited {
    [self.hoverTimer invalidate];
    self.hoverTimer = nil;
    if (self.state == IslandBanner && !self.noteTimer) [self noteEndsIn:2];
}

- (void)clicked {
    if (self.state == IslandBanner && self.note) {
        NSString *link = nf_island_string(self.note[@"link"]);
        NSURL *url = link.length ? [NSURL URLWithString:link] : nil;
        if (url) [NSWorkspace.sharedWorkspace openURL:url];
        [self.noteTimer invalidate];
        self.noteTimer = nil;
        [self nextNote];
    } else if (!self.open) {
        [self openUp];
    }
}

// Open while it's pointed at (looked for ten times a second, only while
// it's open: the pointer may leave over another window).
- (void)openUp {
    self.open = YES;
    self.outsideSince = 0;
    [self go];
    [self.pollTimer invalidate];
    __weak NFIsland *weak = self;
    self.pollTimer = [NSTimer scheduledTimerWithTimeInterval:0.1 repeats:YES block:^(__unused NSTimer *timer) {
        [weak poll];
    }];
}

- (void)poll {
    if (!self.open) {
        [self.pollTimer invalidate];
        self.pollTimer = nil;
        return;
    }
    if ([self pointerInside:12]) {
        self.outsideSince = 0;
        return;
    }
    NSTimeInterval now = NSDate.timeIntervalSinceReferenceDate;
    if (self.outsideSince == 0) {
        self.outsideSince = now;
    } else if (now - self.outsideSince > 0.3) {
        self.open = NO;
        [self.pollTimer invalidate];
        self.pollTimer = nil;
        [self go];
    }
}

- (void)addNote:(NSDictionary *)note {
    // (Open, it's among the last notes there.)
    if (self.open) {
        [self go];
        return;
    }
    [self.notes addObject:note];
    if (!self.note) {
        [self nextNote];
    } else if (self.noteTimer) {
        [self noteEndsIn:MAX(0.2, kQueued - (NSDate.timeIntervalSinceReferenceDate - self.noteAt))];
    }
}

- (void)nextNote {
    if (!self.notes.count) {
        self.note = nil;
        [self go];
        return;
    }
    self.note = self.notes.firstObject;
    [self.notes removeObjectAtIndex:0];
    self.noteAt = NSDate.timeIntervalSinceReferenceDate;
    [self go];
    [self noteEndsIn:self.notes.count ? kQueued : kStays];
}

- (void)noteEndsIn:(NSTimeInterval)after {
    [self.noteTimer invalidate];
    __weak NFIsland *weak = self;
    self.noteTimer = [NSTimer scheduledTimerWithTimeInterval:after repeats:NO block:^(__unused NSTimer *timer) {
        [weak noteEnded];
    }];
}

- (void)noteEnded {
    self.noteTimer = nil;
    // (Still pointed at: it stays.)
    if (self.state == IslandBanner && [self pointerInside:0]) return;
    [self nextNote];
}

@end

// ---- F2's side.

static NFIsland *nf_island_for(uint32_t screen, bool make) {
    if (!notchIslands) notchIslands = [NSMutableDictionary dictionary];
    NFIsland *island = notchIslands[@(screen)];
    if (!island && make && nf_island_screen(screen)) {
        island = [[NFIsland alloc] initWithScreen:screen];
        notchIslands[@(screen)] = island;
    }
    return island;
}

// Each shown as things now are: the home screen's always there (in the
// notch, closed), the others while they're needed.
static void nf_island_all(void) {
    if (!notchOn) return;
    uint32_t home = nf_island_home();
    if (home) nf_island_for(home, true);
    for (NFIsland *island in notchIslands.allValues) [island go];
}

// A note dismissed from the open notch: not among the last any more.
static void nf_island_forget(NSDictionary *note) {
    [notchRecent removeObjectIdenticalTo:note];
    nf_island_all();
}

static void nf_island_screens_changed(void) {
    for (NSNumber *key in notchIslands.allKeys) {
        NFIsland *island = notchIslands[key];
        NSScreen *screen = nf_island_screen(key.unsignedIntValue);
        if (!nf_island_can_show(screen)) {
            [island close];
            [notchIslands removeObjectForKey:key];
        } else {
            [island place];
        }
    }
    nf_island_all();
}

static id notchScreens, notchWake;

// On or off; on screens without a notch too; what's chosen in it. On the
// main thread, as all of these.
void nf_notch_setup(bool on, bool bare, void (*chose)(int32_t what, int64_t id)) {
    notchChose = chose;
    notchBare = bare;
    notchOn = on;
    if (!notchRecent) notchRecent = [NSMutableArray array];
    if (!on) {
        for (NFIsland *island in notchIslands.allValues) [island close];
        [notchIslands removeAllObjects];
        [notchBarsTimer invalidate];
        notchBarsTimer = nil;
        notchDictation = HudHidden;
        if (notchScreens) [NSNotificationCenter.defaultCenter removeObserver:notchScreens];
        if (notchWake) [NSWorkspace.sharedWorkspace.notificationCenter removeObserver:notchWake];
        notchScreens = notchWake = nil;
        return;
    }
    if (!notchScreens) {
        notchScreens = [NSNotificationCenter.defaultCenter addObserverForName:NSApplicationDidChangeScreenParametersNotification object:nil queue:NSOperationQueue.mainQueue usingBlock:^(__unused NSNotification *n) {
            nf_island_screens_changed();
        }];
        notchWake = [NSWorkspace.sharedWorkspace.notificationCenter addObserverForName:NSWorkspaceDidWakeNotification object:nil queue:NSOperationQueue.mainQueue usingBlock:^(__unused NSNotification *n) {
            nf_island_screens_changed();
        }];
    }
    nf_island_screens_changed();
}

// What's going on, as JSON: { "live": { "symbol", "tint", "text" } | null,
// "going": [{ "id", "symbol", "tint", "title", "text" }], "music": {
// "title", "artist", "playing", "art" } | null }.
void nf_notch_live(const char *json) {
    if (!notchOn || !json) return;
    id model = [NSJSONSerialization JSONObjectWithData:[NSData dataWithBytes:json length:strlen(json)] options:0 error:nil];
    notchModel = [model isKindOfClass:NSDictionary.class] ? model : nil;
    nf_island_all();
}

// A note, down from the notch on the screen with the pointer (a color's
// swatch with it, if `color` isn't -1; a link to open if it's clicked).
// False if that screen can't show it (no notch there, and screens without
// one don't get one): the corner shows it then.
bool nf_notch_note(const char *title, const char *detail, int64_t color, const char *symbol, uint32_t tile, const char *link) {
    if (!notchOn) return false;
    NSScreen *screen = nf_island_screen(nf_screen_with_pointer()) ?: NSScreen.mainScreen;
    if (!nf_island_can_show(screen)) return false;
    NSString *said = [NSString stringWithUTF8String:title ?: ""] ?: @"";
    NSMutableDictionary *note = [@{@"title": said, @"detail": [NSString stringWithUTF8String:detail ?: ""] ?: @"", @"at": NSDate.date, @"symbol": [NSString stringWithUTF8String:symbol ?: ""] ?: @"", @"tile": @(tile)} mutableCopy];
    if (color >= 0) note[@"color"] = @(color);
    NSString *url = link ? [NSString stringWithUTF8String:link] : nil;
    if (url.length) note[@"link"] = url;
    // (The same again: only the newest of them kept.)
    [notchRecent filterUsingPredicate:[NSPredicate predicateWithBlock:^BOOL(NSDictionary *was, __unused NSDictionary *bindings) {
        return !([was[@"title"] isEqual:note[@"title"]] && [was[@"detail"] isEqual:note[@"detail"]]);
    }]];
    [notchRecent insertObject:note atIndex:0];
    while (notchRecent.count > 8) [notchRecent removeLastObject];
    NFIsland *island = nf_island_for(nf_island_screen_id(screen), true);
    [island addNote:note];
    for (NFIsland *other in notchIslands.allValues) {
        if (other != island && other.open) [other go];
    }
    // (VoiceOver says it, as it says a notification.)
    NSAccessibilityPostNotificationWithUserInfo(NSApp, NSAccessibilityAnnouncementRequestedNotification, @{NSAccessibilityAnnouncementKey: said, NSAccessibilityPriorityKey: @(NSAccessibilityPriorityMedium)});
    return true;
}

static void nf_island_bars_tick(void) {
    float now = nf_mic_level();
    // (The newest in the middle, older out to the sides: as the pill's.)
    for (int i = 0; i < DictationBars / 2; i++) {
        notchLevels[i] = notchLevels[i + 1];
        notchLevels[DictationBars - 1 - i] = notchLevels[DictationBars - 2 - i];
    }
    float before = notchLevels[DictationBars / 2];
    notchLevels[DictationBars / 2] = now > before ? now : before * 0.6f + now * 0.4f;
    NFDictationBars *bars = notchBarsView;
    bars.phase += 0.25;
    bars.needsDisplay = YES;
}

// Dictation, as dictation.m's pill shows it (`mode`), beside the notch on
// `screen` instead: true if it is (then the pill isn't).
bool nf_notch_dictation(int32_t mode, uint32_t screen) {
    if (!notchOn) return false;
    if (mode != HudHidden && !nf_island_can_show(nf_island_screen(screen))) mode = HudHidden;
    if (mode == HudHidden) {
        bool was = notchDictation != HudHidden;
        notchDictation = HudHidden;
        [notchBarsTimer invalidate];
        notchBarsTimer = nil;
        if (was) nf_island_all();
        return was || nf_island_can_show(nf_island_screen(screen));
    }
    if (notchDictation == HudHidden) memset(notchLevels, 0, sizeof notchLevels);
    notchDictation = mode;
    notchDictationScreen = screen;
    nf_island_for(screen, true);
    if (!notchBarsTimer) {
        notchBarsTimer = [NSTimer timerWithTimeInterval:1.0 / 30 repeats:YES block:^(__unused NSTimer *timer) {
            nf_island_bars_tick();
        }];
        [NSRunLoop.mainRunLoop addTimer:notchBarsTimer forMode:NSRunLoopCommonModes];
    }
    nf_island_all();
    return true;
}

// ---- What's playing, as Music and Spotify say it (to any app that
// listens: no asking). Delivered at once, F2 in front or not.

@interface NFMusicListener : NSObject
@property(nonatomic) void (*heard)(const char *json);
@end

@implementation NFMusicListener
- (void)changed:(NSNotification *)notification {
    NSDictionary *info = notification.userInfo ?: @{};
    NSString *player = [notification.name hasPrefix:@"com.spotify"] ? @"Spotify" : @"Music";
    NSDictionary *said = @{
        @"player": player,
        @"state": nf_island_string(info[@"Player State"]),
        @"title": nf_island_string(info[@"Name"]),
        @"artist": nf_island_string(info[@"Artist"]),
        @"album": nf_island_string(info[@"Album"]),
    };
    NSData *json = [NSJSONSerialization dataWithJSONObject:said options:0 error:nil];
    NSString *text = json ? [[NSString alloc] initWithData:json encoding:NSUTF8StringEncoding] : nil;
    if (text && self.heard) self.heard(text.UTF8String);
}
@end

static NFMusicListener *notchMusic;

// `heard` gets what's playing as it changes (JSON: player, state, title,
// artist, album); NULL: no more.
void nf_notch_music(void (*heard)(const char *json)) {
    NSDistributedNotificationCenter *center = NSDistributedNotificationCenter.defaultCenter;
    if (notchMusic) {
        [center removeObserver:notchMusic];
        notchMusic = nil;
    }
    if (!heard) return;
    notchMusic = [NFMusicListener new];
    notchMusic.heard = heard;
    for (NSString *name in @[@"com.apple.Music.playerInfo", @"com.spotify.client.PlaybackStateChanged"]) {
        [center addObserver:notchMusic selector:@selector(changed:) name:name object:nil suspensionBehavior:NSNotificationSuspensionBehaviorDeliverImmediately];
    }
}
