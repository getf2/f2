//! Window management, as Rectangle's and Raycast's: the window in front to
//! a half, a quarter, a third or two, the whole screen (or almost, or its
//! height), the middle, a size smaller or larger, the next or previous
//! display, back to where it was, or full screen. From Quick Open's
//! commands, or shortcuts in any app (Settings > Windows: Rectangle's or
//! Spectacle's at a click). A half pressed again goes to ⅔, then ⅓, as
//! Rectangle's does. It takes Accessibility, as moving other apps' windows
//! does.

use std::collections::HashMap;
use std::sync::Mutex;

use gpui::App;

use crate::macos;
use crate::settings::Settings;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Place {
    LeftHalf,
    RightHalf,
    TopHalf,
    BottomHalf,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    LeftThird,
    CenterThird,
    RightThird,
    LeftTwoThirds,
    RightTwoThirds,
    Maximize,
    AlmostMaximize,
    MaximizeHeight,
    Center,
    Smaller,
    Larger,
    NextDisplay,
    PreviousDisplay,
    Restore,
    FullScreen,
}

/// A window's frame, or a screen's room: points from the main screen's
/// top left, y down.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Frame {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Frame {
    fn center(&self) -> (f32, f32) {
        (self.x + self.w / 2., self.y + self.h / 2.)
    }

    fn contains(&self, (x, y): (f32, f32)) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }

    fn near(&self, o: &Frame) -> bool {
        (self.x - o.x).abs() < 4. && (self.y - o.y).abs() < 4. && (self.w - o.w).abs() < 4. && (self.h - o.h).abs() < 4.
    }
}

impl Place {
    pub const ALL: [Place; 23] = [
        Place::LeftHalf,
        Place::RightHalf,
        Place::TopHalf,
        Place::BottomHalf,
        Place::TopLeft,
        Place::TopRight,
        Place::BottomLeft,
        Place::BottomRight,
        Place::LeftThird,
        Place::CenterThird,
        Place::RightThird,
        Place::LeftTwoThirds,
        Place::RightTwoThirds,
        Place::Maximize,
        Place::AlmostMaximize,
        Place::MaximizeHeight,
        Place::Center,
        Place::Smaller,
        Place::Larger,
        Place::NextDisplay,
        Place::PreviousDisplay,
        Place::Restore,
        Place::FullScreen,
    ];

    /// Its shortcut's id (see `hotkeys`).
    fn id(self) -> u32 {
        40 + self as u32
    }

    /// Its name in Settings' file.
    pub fn key(self) -> &'static str {
        self.about().0
    }

    pub fn title(self) -> &'static str {
        self.about().1
    }

    pub fn symbol(self) -> &'static str {
        self.about().2
    }

    /// Words it's also found by.
    pub fn words(self) -> &'static str {
        self.about().3
    }

    fn about(self) -> (&'static str, &'static str, &'static str, &'static str) {
        match self {
            Place::LeftHalf => ("left-half", "Left Half", "rectangle.lefthalf.filled", "window left half snap tile"),
            Place::RightHalf => ("right-half", "Right Half", "rectangle.righthalf.filled", "window right half snap tile"),
            Place::TopHalf => ("top-half", "Top Half", "rectangle.tophalf.filled", "window top half upper"),
            Place::BottomHalf => ("bottom-half", "Bottom Half", "rectangle.bottomhalf.filled", "window bottom half lower"),
            Place::TopLeft => ("top-left", "Top Left Quarter", "rectangle.inset.topleft.filled", "window top left quarter corner"),
            Place::TopRight => ("top-right", "Top Right Quarter", "rectangle.inset.topright.filled", "window top right quarter corner"),
            Place::BottomLeft => ("bottom-left", "Bottom Left Quarter", "rectangle.inset.bottomleft.filled", "window bottom left quarter corner"),
            Place::BottomRight => ("bottom-right", "Bottom Right Quarter", "rectangle.inset.bottomright.filled", "window bottom right quarter corner"),
            Place::LeftThird => ("left-third", "Left Third", "rectangle.leftthird.inset.filled", "window first left third"),
            Place::CenterThird => ("center-third", "Center Third", "rectangle.center.inset.filled", "window center middle third"),
            Place::RightThird => ("right-third", "Right Third", "rectangle.rightthird.inset.filled", "window last right third"),
            Place::LeftTwoThirds => ("left-two-thirds", "Left Two Thirds", "rectangle.split.3x1", "window first left two thirds"),
            Place::RightTwoThirds => ("right-two-thirds", "Right Two Thirds", "rectangle.split.3x1.fill", "window last right two thirds"),
            Place::Maximize => ("maximize", "Maximize", "rectangle.inset.filled", "window maximize fill zoom whole screen"),
            Place::AlmostMaximize => ("almost-maximize", "Almost Maximize", "arrow.up.left.and.arrow.down.right", "window almost maximize"),
            Place::MaximizeHeight => ("maximize-height", "Maximize Height", "rectangle.expand.vertical", "window maximize height tall"),
            Place::Center => ("center", "Center", "rectangle.center.inset.filled", "window center middle"),
            Place::Smaller => ("smaller", "Make Smaller", "rectangle.arrowtriangle.2.inward", "window make smaller shrink"),
            Place::Larger => ("larger", "Make Larger", "rectangle.arrowtriangle.2.outward", "window make larger grow bigger"),
            Place::NextDisplay => ("next-display", "Next Display", "arrow.right.to.line", "window next display screen monitor move"),
            Place::PreviousDisplay => ("previous-display", "Previous Display", "arrow.left.to.line", "window previous display screen monitor move"),
            Place::Restore => ("restore", "Restore", "arrow.uturn.backward", "window restore undo back previous size"),
            Place::FullScreen => ("full-screen", "Toggle Full Screen", "arrow.up.backward.and.arrow.down.forward", "window full screen fullscreen"),
        }
    }

    /// Rectangle's shortcut for it, if it has one.
    pub fn rectangle(self) -> Option<&'static str> {
        Some(match self {
            Place::LeftHalf => "ctrl-alt-left",
            Place::RightHalf => "ctrl-alt-right",
            Place::TopHalf => "ctrl-alt-up",
            Place::BottomHalf => "ctrl-alt-down",
            Place::TopLeft => "ctrl-alt-u",
            Place::TopRight => "ctrl-alt-i",
            Place::BottomLeft => "ctrl-alt-j",
            Place::BottomRight => "ctrl-alt-k",
            Place::LeftThird => "ctrl-alt-d",
            Place::CenterThird => "ctrl-alt-f",
            Place::RightThird => "ctrl-alt-g",
            Place::LeftTwoThirds => "ctrl-alt-e",
            Place::RightTwoThirds => "ctrl-alt-t",
            Place::Maximize => "ctrl-alt-enter",
            Place::MaximizeHeight => "ctrl-alt-shift-up",
            Place::Center => "ctrl-alt-c",
            Place::Smaller => "ctrl-alt--",
            Place::Larger => "ctrl-alt-=",
            Place::NextDisplay => "ctrl-alt-cmd-right",
            Place::PreviousDisplay => "ctrl-alt-cmd-left",
            Place::Restore => "ctrl-alt-backspace",
            Place::AlmostMaximize | Place::FullScreen => return None,
        })
    }

    /// Spectacle's, if it has one.
    pub fn spectacle(self) -> Option<&'static str> {
        Some(match self {
            Place::LeftHalf => "alt-cmd-left",
            Place::RightHalf => "alt-cmd-right",
            Place::TopHalf => "alt-cmd-up",
            Place::BottomHalf => "alt-cmd-down",
            Place::TopLeft => "ctrl-cmd-left",
            Place::BottomLeft => "ctrl-shift-cmd-left",
            Place::TopRight => "ctrl-cmd-right",
            Place::BottomRight => "ctrl-shift-cmd-right",
            Place::Maximize => "alt-cmd-f",
            Place::Center => "alt-cmd-c",
            Place::Smaller => "ctrl-alt-shift-left",
            Place::Larger => "ctrl-alt-shift-right",
            Place::NextDisplay => "ctrl-alt-cmd-right",
            Place::PreviousDisplay => "ctrl-alt-cmd-left",
            Place::Restore => "alt-cmd-z",
            _ => return None,
        })
    }
}

/// `room`'s part from `x0` to `x1` and `y0` to `y1` (its fractions), with
/// `gap` round it (half of it between windows).
fn part(room: Frame, (x0, y0, x1, y1): (f32, f32, f32, f32), gap: f32) -> Frame {
    let inset = |at_edge: bool| if at_edge { gap } else { gap / 2. };
    let left = room.x + room.w * x0 + inset(x0 == 0.);
    let right = room.x + room.w * x1 - inset(x1 == 1.);
    let top = room.y + room.h * y0 + inset(y0 == 0.);
    let bottom = room.y + room.h * y1 - inset(y1 == 1.);
    Frame { x: left.round(), y: top.round(), w: (right - left).round(), h: (bottom - top).round() }
}

/// Where `place` puts a window that's at `now`, in a screen's `room`
/// (less `gap` round windows). None for those that aren't a place in a
/// screen (displays, restoring, full screen).
pub fn target(place: Place, now: Frame, room: Frame, gap: f32) -> Option<Frame> {
    // A half again: ⅔, then ⅓, then ½ (Rectangle's way).
    let cycle = |make: &dyn Fn(f32) -> (f32, f32, f32, f32)| {
        let sizes = [0.5, 2. / 3., 1. / 3.];
        let at = sizes.iter().position(|&f| part(room, make(f), gap).near(&now));
        let next = at.map_or(0, |i| (i + 1) % sizes.len());
        part(room, make(sizes[next]), gap)
    };
    let third = 1. / 3.;
    Some(match place {
        Place::LeftHalf => cycle(&|f| (0., 0., f, 1.)),
        Place::RightHalf => cycle(&|f| (1. - f, 0., 1., 1.)),
        Place::TopHalf => cycle(&|f| (0., 0., 1., f)),
        Place::BottomHalf => cycle(&|f| (0., 1. - f, 1., 1.)),
        Place::TopLeft => part(room, (0., 0., 0.5, 0.5), gap),
        Place::TopRight => part(room, (0.5, 0., 1., 0.5), gap),
        Place::BottomLeft => part(room, (0., 0.5, 0.5, 1.), gap),
        Place::BottomRight => part(room, (0.5, 0.5, 1., 1.), gap),
        Place::LeftThird => part(room, (0., 0., third, 1.), gap),
        Place::CenterThird => part(room, (third, 0., 2. * third, 1.), gap),
        Place::RightThird => part(room, (2. * third, 0., 1., 1.), gap),
        Place::LeftTwoThirds => part(room, (0., 0., 2. * third, 1.), gap),
        Place::RightTwoThirds => part(room, (third, 0., 1., 1.), gap),
        Place::Maximize => part(room, (0., 0., 1., 1.), gap),
        Place::AlmostMaximize => part(room, (0.05, 0.05, 0.95, 0.95), gap),
        Place::MaximizeHeight => {
            let full = part(room, (0., 0., 1., 1.), gap);
            Frame { y: full.y, h: full.h, ..now }
        }
        Place::Center => {
            let (w, h) = (now.w.min(room.w), now.h.min(room.h));
            Frame { x: (room.x + (room.w - w) / 2.).round(), y: (room.y + (room.h - h) / 2.).round(), w, h }
        }
        Place::Smaller | Place::Larger => {
            // (A tenth of the screen each way, about its middle, kept in it.)
            let by = if place == Place::Larger { 1. } else { -1. };
            let w = (now.w + by * room.w * 0.1).clamp(240., room.w);
            let h = (now.h + by * room.h * 0.1).clamp(160., room.h);
            let (cx, cy) = now.center();
            let x = (cx - w / 2.).clamp(room.x, room.x + room.w - w);
            let y = (cy - h / 2.).clamp(room.y, room.y + room.h - h);
            Frame { x: x.round(), y: y.round(), w: w.round(), h: h.round() }
        }
        Place::NextDisplay | Place::PreviousDisplay | Place::Restore | Place::FullScreen => return None,
    })
}

/// A window at `now` in `from`'s room, moved to `to`'s: as far across it,
/// as big a part of it (and still in it).
pub fn across(now: Frame, from: Frame, to: Frame) -> Frame {
    let (sx, sy) = (to.w / from.w, to.h / from.h);
    let w = (now.w * sx).min(to.w);
    let h = (now.h * sy).min(to.h);
    let x = (to.x + (now.x - from.x) * sx).clamp(to.x, to.x + to.w - w);
    let y = (to.y + (now.y - from.y) * sy).clamp(to.y, to.y + to.h - h);
    Frame { x: x.round(), y: y.round(), w: w.round(), h: h.round() }
}

/// Where a new window goes: as `last` (the frame last saved), `step` down
/// and right of it, all of it in the room of the screen its middle's on
/// (else the first), smaller if it must be. Steps that won't fit go back
/// the other way from the room's side (left, or up).
pub fn place_new(last: Frame, step: f32, screens: &[(Frame, Frame)]) -> Frame {
    let mine = screens.iter().find(|(_, full)| full.contains(last.center())).or(screens.first());
    let Some(&(room, _)) = mine else { return Frame { x: last.x + step, y: last.y + step, ..last } };
    let (w, h) = (last.w.min(room.w), last.h.min(room.h));
    // (From the room's start, on one side: in it, then stepped, to and fro
    // in the room there is.)
    let along = |at: f32, start: f32, slack: f32| {
        let stepped = if slack < 1. { 0. } else { ((at - start).clamp(0., slack) + step) % (2. * slack) };
        start + if stepped > slack { 2. * slack - stepped } else { stepped }
    };
    Frame { x: along(last.x, room.x, room.w - w).round(), y: along(last.y, room.y, room.h - h).round(), w, h }
}

/// Where each window was before it was first moved here (for Restore).
static BEFORE: Mutex<Option<HashMap<u64, Frame>>> = Mutex::new(None);

/// Move the window in front to `place`.
pub fn apply(place: Place, cx: &mut App) {
    if !macos::accessibility(false) {
        macos::accessibility(true);
        crate::shots::note(
            "Windows Need Accessibility".into(),
            format!("Allow {} in System Settings > Privacy & Security > Accessibility to move windows.", crate::APP_NAME),
            Some("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"),
            cx,
        );
        return;
    }
    if place == Place::FullScreen {
        macos::window_full_screen();
        return;
    }
    let Some((now, key)) = macos::window_get() else { return };
    let screens = macos::screens();
    if screens.is_empty() {
        return;
    }
    // Its screen: the one its middle's on (else the first).
    let at = screens.iter().position(|(_, full)| full.contains(now.center())).unwrap_or(0);
    let room = screens[at].0;
    let gap = Settings::get(cx).window_gap.clamp(0., 64.);
    let mut before = BEFORE.lock().unwrap();
    let before = before.get_or_insert_with(HashMap::new);
    let to = match place {
        Place::Restore => match before.remove(&key) {
            Some(frame) => frame,
            None => return,
        },
        Place::NextDisplay | Place::PreviousDisplay => {
            if screens.len() < 2 {
                return;
            }
            // (Left to right.)
            let mut order: Vec<usize> = (0..screens.len()).collect();
            order.sort_by(|&a, &b| screens[a].1.x.total_cmp(&screens[b].1.x));
            let i = order.iter().position(|&s| s == at).unwrap_or(0);
            let by = if place == Place::NextDisplay { 1 } else { order.len() - 1 };
            let next = order[(i + by) % order.len()];
            across(now, room, screens[next].0)
        }
        _ => match target(place, now, room, gap) {
            Some(to) => to,
            None => return,
        },
    };
    if place != Place::Restore {
        before.entry(key).or_insert(now);
    }
    macos::window_set(to);
}

/// Take the shortcuts Settings set (again when they change).
pub fn init(cx: &mut App) {
    let mut registered: Option<Vec<Option<String>>> = None;
    let mut register = move |cx: &mut App| {
        let shortcuts = &Settings::get(cx).window_shortcuts;
        let wanted: Vec<Option<String>> = Place::ALL.iter().map(|p| shortcuts.get(p.key()).cloned()).collect();
        if registered.as_ref() == Some(&wanted) {
            return;
        }
        for (place, shortcut) in Place::ALL.iter().zip(&wanted) {
            if !crate::hotkeys::set(place.id(), shortcut.as_deref()) {
                eprintln!("{}'s shortcut {shortcut:?} couldn't be had", place.title());
            }
        }
        registered = Some(wanted);
    };
    register(cx);
    cx.observe_global::<Settings>(register).detach();
    drag(cx);
}

/// Dragging windows with keys held (Settings' `window_drag`): from
/// anywhere in one to move it, from near an edge or a corner to size it
/// (see drag.m). Tried again now and then until Accessibility's allowed.
fn drag(cx: &mut App) {
    let mut had: Option<u64> = None;
    let mut want = move |cx: &mut App| {
        let keys = Settings::get(cx).window_drag.flags();
        if had == Some(keys) {
            return;
        }
        if macos::window_drag(keys) {
            had = Some(keys);
        }
    };
    want(cx);
    let want = std::rc::Rc::new(std::cell::RefCell::new(want));
    let again = want.clone();
    cx.observe_global::<Settings>(move |cx| (again.borrow_mut())(cx)).detach();
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(std::time::Duration::from_secs(5)).await;
            cx.update(|cx| (want.borrow_mut())(cx));
        }
    })
    .detach();
}

/// A shortcut was pressed (see `hotkeys`).
pub fn pressed(id: u32, cx: &mut App) -> bool {
    match Place::ALL.into_iter().find(|p| p.id() == id) {
        Some(place) => {
            apply(place, cx);
            true
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{Frame, Place, across, place_new, target};

    const ROOM: Frame = Frame { x: 0., y: 25., w: 1200., h: 800. };

    fn f(x: f32, y: f32, w: f32, h: f32) -> Frame {
        Frame { x, y, w, h }
    }

    #[test]
    fn places() {
        let now = f(100., 100., 500., 400.);
        assert_eq!(target(Place::LeftHalf, now, ROOM, 0.), Some(f(0., 25., 600., 800.)));
        assert_eq!(target(Place::RightHalf, now, ROOM, 0.), Some(f(600., 25., 600., 800.)));
        assert_eq!(target(Place::BottomRight, now, ROOM, 0.), Some(f(600., 425., 600., 400.)));
        assert_eq!(target(Place::CenterThird, now, ROOM, 0.), Some(f(400., 25., 400., 800.)));
        assert_eq!(target(Place::Maximize, now, ROOM, 10.), Some(f(10., 35., 1180., 780.)));
        assert_eq!(target(Place::LeftHalf, now, ROOM, 10.), Some(f(10., 35., 585., 780.)), "half a gap between windows");
        assert_eq!(target(Place::Center, now, ROOM, 0.), Some(f(350., 225., 500., 400.)));
        assert_eq!(target(Place::MaximizeHeight, now, ROOM, 0.), Some(f(100., 25., 500., 800.)));
        assert_eq!(target(Place::Restore, now, ROOM, 0.), None);
    }

    #[test]
    fn halves_cycle() {
        let half = target(Place::LeftHalf, f(0., 0., 10., 10.), ROOM, 0.).unwrap();
        let two_thirds = target(Place::LeftHalf, half, ROOM, 0.).unwrap();
        assert_eq!(two_thirds.w, 800.);
        let third = target(Place::LeftHalf, two_thirds, ROOM, 0.).unwrap();
        assert_eq!(third.w, 400.);
        assert_eq!(target(Place::LeftHalf, third, ROOM, 0.).unwrap().w, 600., "and round again");
    }

    #[test]
    fn sizes_and_displays() {
        let now = f(100., 100., 500., 400.);
        let larger = target(Place::Larger, now, ROOM, 0.).unwrap();
        assert_eq!((larger.w, larger.h), (620., 480.));
        let tiny = target(Place::Smaller, f(0., 25., 250., 170.), ROOM, 0.).unwrap();
        assert_eq!((tiny.w, tiny.h), (240., 160.), "not below a least size");
        let other = f(1200., 0., 2400., 1600.);
        assert_eq!(across(f(0., 25., 600., 800.), ROOM, other), f(1200., 0., 1200., 1600.));
        assert!(Place::ALL.iter().all(|p| Place::ALL.iter().filter(|q| q.key() == p.key()).count() == 1), "keys are unique");
        // (panel.m holds 128 hot keys: ids past that were never registered.)
        assert!(Place::ALL.iter().all(|p| p.id() < 128));
    }

    #[test]
    fn new_windows_on_screen() {
        let screens = [(ROOM, f(0., 0., 1200., 825.)), (f(-1800., 0., 1800., 1100.), f(-1800., 0., 1800., 1100.))];
        // Room for it: as it was, stepped.
        assert_eq!(place_new(f(100., 100., 600., 400.), 0., &screens), f(100., 100., 600., 400.));
        assert_eq!(place_new(f(100., 100., 600., 400.), 48., &screens), f(148., 148., 600., 400.));
        // Off the right side (saved on a wider screen): back in, then stepped
        // left from there, as there's no room right.
        assert_eq!(place_new(f(800., 100., 600., 400.), 0., &screens), f(600., 100., 600., 400.));
        assert_eq!(place_new(f(800., 100., 600., 400.), 24., &screens), f(576., 124., 600., 400.));
        assert_eq!(place_new(f(590., 100., 600., 400.), 24., &screens), f(586., 124., 600., 400.), "to the side and back");
        // Bigger than the room: as big as it.
        assert_eq!(place_new(f(0., 0., 1500., 900.), 24., &screens), f(0., 25., 1200., 800.));
        // On the other screen, kept there; on none (unplugged), the first.
        assert_eq!(place_new(f(-1000., 200., 600., 400.), 24., &screens), f(-976., 224., 600., 400.));
        assert_eq!(place_new(f(5000., 200., 600., 400.), 0., &screens), f(600., 200., 600., 400.));
        assert_eq!(place_new(f(5000., 200., 600., 400.), 0., &[]), f(5000., 200., 600., 400.));
    }
}

/// A picture of the window `id` as it is now (screencapture's, without its
/// shadow): a JPEG in the temporary folder, new each time (`n`: images are
/// kept by their path). None if it can't be taken: Screen Recording not
/// allowed (not asked for here), or the window minimized. Waits: not on the
/// main thread.
pub fn picture(id: u32, n: u64) -> Option<std::path::PathBuf> {
    if !macos::capture_allowed() {
        return None;
    }
    let file = std::env::temp_dir().join(format!("f2-window-{id}-{n}.jpg"));
    let taken = std::process::Command::new("/usr/sbin/screencapture").args(["-x", "-o", "-t", "jpg", &format!("-l{id}")]).arg(&file).status();
    (taken.is_ok_and(|s| s.success()) && file.metadata().is_ok_and(|m| m.len() > 0)).then_some(file)
}
