//! The terminal that comes down from the notch, as Guake's and Yakuake's
//! come down from the top of the screen (and Ghostty's quick terminal):
//! its shortcut (⌃`) brings it down, the notch's black growing out into
//! it, and takes it back up. Its shells go on while it's away: the views
//! are kept, only the window comes and goes. A click elsewhere puts it
//! away, if Settings say so (once the mouse is let go: it might be files
//! being dragged onto it). A new shell starts in your home folder, F2's
//! front window's folder, or one you chose (Settings › Terminal); "Open
//! Here in the Notch's Terminal" (File) takes it to a window's folder.
//! Tabs (⌘T, ⌘W, ⌘1…, ⌘⇧[ and ⌘⇧]) are in the notch's black, from its
//! left end, clear of the notch (where the notch's own grows out: `CLEAR`).
//! In beta.

use std::path::PathBuf;
use std::time::Duration;

use gpui::prelude::FluentBuilder as _;
use gpui::*;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

use super::{Asked, Ended, Session, TerminalView};
use crate::settings::{Settings, TerminalStart};

/// How long it takes to come down, and to go back up (quicker: coming
/// down looks done well before its end, and going up has to keep up).
const DOWN: Duration = Duration::from_millis(220);
const UP: Duration = Duration::from_millis(160);
/// Its lower corners.
const RADIUS: f32 = 18.;
/// Left clear beside the notch, for the notch's own (src/macos/notch.m,
/// a window above this one): its ears, a timer's or what's playing, are
/// up to about 90 points, then its shoulder's curve.
const CLEAR: f32 = 112.;
/// A tab at its smallest, and … for those left out.
const TAB: f32 = 44.;
const MORE: f32 = 14.;

/// The tabs' terminals (kept while it's away), the one shown, and the
/// window while it's down.
#[derive(Default)]
struct Notch {
    tabs: Vec<Entity<TerminalView>>,
    current: usize,
    window: Option<(AnyWindowHandle, WeakEntity<NotchPanel>)>,
}

impl Global for Notch {}

impl Notch {
    fn shown(&self) -> Option<Entity<TerminalView>> {
        self.tabs.get(self.current).cloned()
    }
}

/// Whether it's there to have: beta features on, and it on.
pub fn available(cx: &App) -> bool {
    let s = Settings::get(cx);
    s.beta && s.quick_terminal.on
}

/// Its shortcut taken as Settings have it (again when they change), and
/// File's "Open Here" with it.
pub fn init(cx: &mut App) {
    cx.set_global(Notch::default());
    let mut registered: Option<Option<String>> = None;
    let mut menu: Option<bool> = None;
    let mut apply = move |cx: &mut App| {
        let on = available(cx);
        let wanted = on.then(|| Settings::get(cx).quick_terminal.shortcut.clone()).flatten();
        if registered.as_ref() != Some(&wanted) {
            if !crate::hotkeys::set(crate::hotkeys::TERMINAL, wanted.as_deref()) {
                eprintln!("the notch's terminal's shortcut {wanted:?} is taken by another app");
            }
            registered = Some(wanted);
        }
        if menu.is_some_and(|was| was != on) {
            crate::menus::menu_bar(cx);
        }
        menu = Some(on);
        // (Turned off: put away, its shells ended.)
        if !on {
            close(cx);
        }
    };
    apply(cx);
    cx.observe_global::<Settings>(move |cx| apply(cx)).detach();
    // (F2_TERMINAL=1: down at launch, for trying it in a test copy.)
    if std::env::var_os("F2_TERMINAL").is_some() {
        cx.defer(|cx| show(None, cx));
    }
}

/// The shortcut: down, or back up.
pub fn toggle(cx: &mut App) {
    if down(cx).is_some() {
        hide(cx);
    } else {
        show(None, cx);
    }
}

/// Down, in `dir` (a window's folder): `cd` there, if the shell's at its
/// prompt; a new tab there, if it's running something; a new shell there,
/// if there's none.
pub fn open_here(dir: PathBuf, cx: &mut App) {
    if !available(cx) {
        return;
    }
    match cx.global::<Notch>().shown() {
        Some(terminal) if !terminal.read(cx).busy() => {
            terminal.update(cx, |terminal, cx| terminal.cd(&dir, cx));
            show(None, cx);
        }
        Some(_) => {
            show(None, cx);
            new_tab(Some(dir), cx);
        }
        None => show(Some(dir), cx),
    }
}

/// The panel, if it's down (not on its way up).
fn down(cx: &App) -> Option<Entity<NotchPanel>> {
    let (_, panel) = cx.global::<Notch>().window.clone()?;
    panel.upgrade().filter(|p| !p.read(cx).closing)
}

/// Where a new shell starts, as Settings have it.
fn start_folder(cx: &App) -> Option<PathBuf> {
    let s = &Settings::get(cx).quick_terminal;
    match s.start {
        TerminalStart::Home => None,
        TerminalStart::Window => crate::browser::front_folder(cx),
        TerminalStart::Folder => s.folder.clone().filter(|f| f.is_dir()),
    }
}

fn show(dir: Option<PathBuf>, cx: &mut App) {
    if !available(cx) {
        return;
    }
    // (On its way up: down again.)
    if let Some((handle, panel)) = cx.global::<Notch>().window.clone()
        && let Some(panel) = panel.upgrade()
    {
        panel.update(cx, |panel, cx| {
            if panel.closing {
                panel.closing = false;
                panel.generation += 1;
                cx.notify();
            }
        });
        present(handle, None, cx);
        return;
    }
    let s = Settings::get(cx).quick_terminal.clone();
    let Some(display) = crate::screens::with_pointer(cx) else { return };
    let id = u64::from(display.id()) as u32;
    let (notch_width, band) = crate::macos::notch_band(id);
    let screen = display.bounds().size;
    let (sw, sh) = (f32::from(screen.width), f32::from(screen.height));
    let look = &s.look;
    let full = size(px((sw * look.width.clamp(0.3, 1.)).max(480.).min(sw)), px((sh * look.height.clamp(0.2, 0.95)).max(240.).min(sh)));
    let notch = size(px(if notch_width > 0. { notch_width } else { 200. }), px(band));
    let bounds = Bounds { origin: point(px((sw - f32::from(full.width)) / 2.), px(0.)), size: full };
    // A shell, if there isn't one going.
    let mut session = None;
    if cx.global::<Notch>().tabs.is_empty() {
        match Session::start(dir.or_else(|| start_folder(cx))) {
            Ok(started) => session = Some(started),
            Err(e) => return eprintln!("the notch's terminal: {e}"),
        }
    }
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: None,
        kind: WindowKind::PopUp,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        display_id: Some(display.id()),
        window_background: WindowBackgroundAppearance::Transparent,
        // (Made hidden: shown once it's set up over the menu bar.)
        show: false,
        focus: false,
        ..Default::default()
    };
    let mut built = None;
    let opened = cx.open_window(options, |window, cx| {
        let made = session.take().map(|session| cx.new(|cx| TerminalView::new(session, window, cx)));
        let panel = cx.new(|cx| NotchPanel::new(notch, full, px(band), made.clone(), window, cx));
        built = Some((panel.clone(), made));
        cx.new(|cx| {
            let mut root = gpui_kit::base::Root::new(panel, window, cx);
            root.style().background = Some(transparent_black().into());
            root
        })
    });
    let (Ok(handle), Some((panel, made))) = (opened, built) else { return eprintln!("the notch's terminal didn't open") };
    let handle = AnyWindowHandle::from(handle);
    cx.global_mut::<Notch>().window = Some((handle, panel.downgrade()));
    if let Some(terminal) = made {
        adopt(&terminal, cx);
    }
    present(handle, Some((id, f32::from(full.width), f32::from(full.height))), cx);
}

/// A terminal made a tab, and shown; what it asks heard.
fn adopt(terminal: &Entity<TerminalView>, cx: &mut App) {
    let notch = cx.global_mut::<Notch>();
    notch.tabs.push(terminal.clone());
    notch.current = notch.tabs.len() - 1;
    cx.subscribe(terminal, |terminal, _: &Ended, cx| close_tab(&terminal, cx)).detach();
    cx.subscribe(terminal, |terminal, asked: &Asked, cx| {
        let notch = cx.global::<Notch>();
        let (n, current) = (notch.tabs.len(), notch.current);
        match *asked {
            Asked::Focus => {
                if let Some((handle, _)) = notch.window.clone() {
                    present(handle, None, cx);
                }
            }
            Asked::NewTab => new_tab(terminal.read(cx).folder(), cx),
            Asked::CloseTab => close_tab(&terminal, cx),
            Asked::NextTab(on) if n > 0 => select(if on { (current + 1) % n } else { (current + n - 1) % n }, cx),
            // (⌘9: the last, as in other apps.)
            Asked::Tab(i) if n > 0 => select(if i == 8 { n - 1 } else { i }, cx),
            _ => {}
        }
    })
    .detach();
    select(cx.global::<Notch>().current, cx);
}

/// A new tab, its shell in `dir` (else where Settings say), while it's down.
fn new_tab(dir: Option<PathBuf>, cx: &mut App) {
    let Some((handle, _)) = cx.global::<Notch>().window.clone() else { return };
    let session = match Session::start(dir.or_else(|| start_folder(cx))) {
        Ok(session) => session,
        Err(e) => return eprintln!("the notch's terminal: {e}"),
    };
    let made = handle.update(cx, |_, window, cx| cx.new(|cx| TerminalView::new(session, window, cx)));
    if let Ok(terminal) = made {
        adopt(&terminal, cx);
    }
}

/// The tab numbered `i` shown, and typed into.
fn select(i: usize, cx: &mut App) {
    let notch = cx.global_mut::<Notch>();
    if i >= notch.tabs.len() {
        return;
    }
    notch.current = i;
    let Some(terminal) = notch.shown() else { return };
    let Some((handle, panel)) = notch.window.clone() else { return };
    let focus = terminal.read(cx).focus.clone();
    let _ = handle.update(cx, |_, window, cx| focus.focus(window, cx));
    if let Some(panel) = panel.upgrade() {
        panel.update(cx, |_, cx| cx.notify());
    }
}

/// A tab closed (⌘W), or its shell ended (exit): its shell gone with it.
/// The last one: it goes back up, and next time a new shell starts.
fn close_tab(terminal: &Entity<TerminalView>, cx: &mut App) {
    let notch = cx.global_mut::<Notch>();
    let Some(i) = notch.tabs.iter().position(|t| t == terminal) else { return };
    notch.tabs.remove(i);
    if notch.tabs.is_empty() {
        notch.current = 0;
        hide(cx);
        return;
    }
    if notch.current > i || notch.current >= notch.tabs.len() {
        notch.current = notch.current.saturating_sub(1);
    }
    let current = notch.current;
    select(current, cx);
}

/// Put on screen, the app in front noted and F2 made it (an input method
/// composes only in the active app's key window, as Quick Open has it),
/// set up over the menu bar first (`place`: on which screen, how big),
/// once it's drawn.
fn present(handle: AnyWindowHandle, place: Option<(u32, f32, f32)>, cx: &mut App) {
    crate::typing::terminal_up(true);
    cx.spawn(async move |cx| {
        let view = handle
            .update(cx, |_, window, _| match HasWindowHandle::window_handle(window).map(|h| h.as_raw()) {
                Ok(RawWindowHandle::AppKit(appkit)) => appkit.ns_view.as_ptr(),
                _ => std::ptr::null_mut(),
            })
            .unwrap_or(std::ptr::null_mut());
        if view.is_null() {
            return;
        }
        if let Some((display, width, height)) = place {
            crate::macos::panel_bare(view);
            crate::macos::terminal_window(view, display, width, height);
            crate::macos::panel_present(view);
        }
        crate::macos::panel_take_focus(view);
    })
    .detach();
}

/// Back up into the notch, then gone (its shells going on).
pub fn hide(cx: &mut App) {
    let Some(panel) = down(cx) else { return };
    let Some((handle, _)) = cx.global::<Notch>().window.clone() else { return };
    let generation = panel.update(cx, |panel, cx| {
        panel.closing = true;
        panel.generation += 1;
        cx.notify();
        panel.generation
    });
    crate::typing::terminal_up(false);
    crate::macos::panel_give_back();
    cx.spawn(async move |cx| {
        // (Once its last frame's drawn, in the notch: two frames on.)
        cx.background_executor().timer(UP + Duration::from_millis(34)).await;
        cx.update(|cx| {
            // (Not if it was brought down again meanwhile.)
            let p = panel.read(cx);
            if p.closing && p.generation == generation {
                let _ = handle.update(cx, |_, window, _| window.remove_window());
                cx.global_mut::<Notch>().window = None;
            }
        });
    })
    .detach();
}

/// Gone at once, its shells too (the terminal's turned off): the next
/// time, a new one.
fn close(cx: &mut App) {
    let notch = cx.global_mut::<Notch>();
    let window = notch.window.take();
    notch.tabs.clear();
    notch.current = 0;
    if let Some((handle, _)) = window {
        let _ = handle.update(cx, |_, window, _| window.remove_window());
        crate::typing::terminal_up(false);
        crate::macos::panel_give_back();
    }
}

/// The panel: the notch's black, grown out (the menu bar's band of it
/// still black, the tabs in it beside the notch), the terminal shown in
/// it, laid out at its full size as it grows, so the shell isn't resized
/// all the way down.
pub struct NotchPanel {
    notch: Size<Pixels>,
    full: Size<Pixels>,
    band: Pixels,
    closing: bool,
    /// Each time it comes down or goes up (its animation's).
    generation: usize,
    _active: Subscription,
}

impl NotchPanel {
    fn new(notch: Size<Pixels>, full: Size<Pixels>, band: Pixels, made: Option<Entity<TerminalView>>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        if let Some(terminal) = made.or_else(|| cx.global::<Notch>().shown()) {
            let focus = terminal.read(cx).focus.clone();
            focus.focus(window, cx);
        }
        // (Clicked elsewhere: put away, if Settings say; once the mouse is
        // let go, and if it wasn't brought back meanwhile, as a press
        // elsewhere might start a drag of files to drop on it.)
        let active = cx.observe_window_activation(window, |this, window, cx| {
            if window.is_window_active() || this.closing || !Settings::get(cx).quick_terminal.hide_on_click_away {
                return;
            }
            let handle = Window::window_handle(window);
            cx.spawn(async move |_, cx| {
                while crate::macos::mouse_pressed() {
                    cx.background_executor().timer(Duration::from_millis(50)).await;
                }
                cx.background_executor().timer(Duration::from_millis(150)).await;
                let active = handle.update(cx, |_, window, _| window.is_window_active()).unwrap_or(true);
                if !active {
                    cx.update(hide);
                }
            })
            .detach();
        });
        NotchPanel { notch, full, band, closing: false, generation: 0, _active: active }
    }

    /// The tabs, left of the notch, from the band's left end (smaller as
    /// there are more; those that don't fit cut off before `CLEAR`), and +
    /// for a new one.
    fn tabs(&self, cx: &mut Context<Self>) -> Div {
        let notch = cx.global::<Notch>();
        let names: Vec<(String, bool)> = notch.tabs.iter().enumerate().map(|(i, t)| (t.read(cx).name(), i == notch.current)).collect();
        let n = names.len();
        let height = (self.band - px(8.)).clamp(px(16.), px(22.));
        let white: Hsla = gpui::white();
        let many = names.len() > 1;
        // (More than fit, at their smallest: those round the one shown,
        // and … where there are more.)
        let room = (self.full.width - self.notch.width) / 2. - px(RADIUS + CLEAR) - height - px(4. + 2. * (MORE + 4.));
        let fit = ((f32::from(room) + 4.) / (TAB + 4.)).floor().max(1.) as usize;
        let current = names.iter().position(|(_, c)| *c).unwrap_or(0);
        let start = if names.len() > fit { (current + 1).saturating_sub(fit).min(names.len() - fit) } else { 0 };
        let end = (start + fit).min(names.len());
        let more = |id: &'static str| div().id(id).flex_none().w(px(MORE)).text_size(px(11.)).text_center().text_color(white.opacity(0.45)).child("…");
        // (The tabs as wide as they need, smaller as there are more, cut
        // off past the room; + after them, never cut off.)
        let strip = div()
            .flex_initial()
            .min_w_0()
            .h_full()
            .overflow_hidden()
            .flex()
            .items_center()
            .gap_1()
            .when(many && start > 0, |d| d.child(more("notch-tabs-before")))
            .when(many, |d| {
                d.children(names.into_iter().enumerate().skip(start).take(end - start).map(|(i, (name, current))| {
                    div()
                        .id(("notch-tab", i))
                        .h(height)
                        .min_w(px(TAB))
                        .max_w(px(150.))
                        .overflow_hidden()
                        .px_2()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_md()
                        .text_size(px(11.))
                        .text_color(if current { white } else { white.opacity(0.55) })
                        .when(current, |d| d.bg(white.opacity(0.14)))
                        .hover(|d| d.text_color(white))
                        .child(div().truncate().child(name))
                        .on_click(move |_, _, cx| select(i, cx))
                }))
            })
            .when(many && end < n, |d| d.child(more("notch-tabs-after")));
        let plus = div()
            .id("notch-new-tab")
            .flex_none()
            .size(height)
            .flex()
            .items_center()
            .justify_center()
            .rounded_md()
            .text_size(px(14.))
            .text_color(white.opacity(0.45))
            .hover(|d| d.text_color(white).bg(white.opacity(0.1)))
            .child("+")
            .on_click(|_, _, cx| {
                let dir = cx.global::<Notch>().shown().and_then(|t| t.read(cx).folder());
                new_tab(dir, cx);
            });
        div().flex_1().min_w_0().h_full().flex().items_center().gap_1().pl(px(RADIUS)).pr(px(CLEAR)).child(strip).child(plus)
    }
}

/// Going back up: well on its way from the first frame, a little faster
/// as it goes, into the notch. Coming down's curve, run backwards, lost a
/// quarter of it in the first frame, then crept the rest of the way; one
/// that started slow felt late.
fn rising(t: f32) -> f32 {
    0.6 * t + 0.4 * t * t
}

impl Render for NotchPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (from, to, animation) = if self.closing {
            (self.full, self.notch, Animation::new(UP).with_easing(rising))
        } else {
            (self.notch, self.full, Animation::new(DOWN).with_easing(ease_out_quint()))
        };
        let lerp = |a: Pixels, b: Pixels, t: f32| a + (b - a) * t;
        let shown = cx.global::<Notch>().shown();
        let background = shown.as_ref().map_or(black(), |t| t.read(cx).background());
        let tabs = self.tabs(cx);
        let closing = self.closing;
        // (Its two sides padded alike, so they're as wide as each other and
        // the gap between them is where the notch is.)
        let right = div().flex_1().min_w_0().pl(px(CLEAR)).pr(px(RADIUS));
        let band = div().flex_none().w_full().h(self.band).bg(black()).flex().child(tabs).child(div().flex_none().w(self.notch.width)).child(right);
        div().size_full().flex().justify_center().items_start().child(
            div()
                .flex_none()
                .w(to.width)
                .h(to.height)
                .overflow_hidden()
                .rounded_b(px(RADIUS))
                .bg(background)
                .flex()
                .flex_col()
                .child(band)
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .w_full()
                        .overflow_hidden()
                        .flex()
                        .justify_center()
                        .child(div().flex_none().w(self.full.width).h(self.full.height - self.band).children(shown)),
                )
                .with_animation(("notch-terminal", self.generation), animation, move |el, t| {
                    // (Up, gone with its last frame: the window goes a little
                    // after, and on a screen without a notch the notch-sized
                    // black it ends as would wait for it.)
                    let el = el.w(lerp(from.width, to.width, t)).h(lerp(from.height, to.height, t));
                    if closing && t >= 1. { el.opacity(0.) } else { el }
                }),
        )
    }
}
