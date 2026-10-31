//! The welcome, at F2's first start (and from Quick Open, or Settings >
//! Permissions, after), in the website's look: a painting a step, its
//! headline in the sky, and paper cards over the land. What F2 does; the
//! permissions most of it needs, each turning green as it's allowed; the
//! calendar, Reminders and Do Not Disturb for Quick Open, if they're
//! wanted; the shortcuts (Quick Open's, Spotlight's ⌘Space, macOS's
//! screenshot keys, window shortcuts at a click); the notch, if it's
//! wanted (it's off at first); and some things to try. Its step is kept
//! while it's open: macOS reopens F2 for Full Disk Access and Screen
//! Recording (or the welcome's own Reopen F2 does), and it comes back there.

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use gpui::prelude::FluentBuilder as _;
use gpui::*;

use crate::macos;
use crate::permissions::Permission;
use crate::settings::Settings;
use crate::style::{GEIST, MEDIUM, SEMIBOLD, symbol};

const STEPS: usize = 6;
const YOUR_DAY: usize = 2;
const NOTCH: usize = 4;

/// The website's colors: ink on paper, the sky's blues.
const INK: u32 = 0x0d1726;
const INK2: u32 = 0x4a5468;
const INK3: u32 = 0x7b8396;
const PAPER: u32 = 0xfbf9f4;
const LINE: u32 = 0xe8e3d7;
const SKY: u32 = 0x1d4aa8;
const SKY2: u32 = 0x123a8c;
const NAVY: u32 = 0x102c76;
const GREEN: u32 = 0x1f9d55;
/// The system's font, for ⌘ ⇧ ⌥ ⌃ (which Geist hasn't), and its mono.
const KEYS: &str = ".SystemUIFont";
const MONO: &str = ".AppleSystemUIFontMonospaced";

fn tint(color: u32, alpha: f32) -> Hsla {
    Hsla::from(rgb(color)).opacity(alpha)
}

/// The window, if open.
#[derive(Default)]
struct Open(Option<AnyWindowHandle>);

impl Global for Open {}

/// Over a window opening now: once it's up (else it'd be under it).
pub fn open_soon(cx: &mut App) {
    cx.spawn(async move |cx| {
        cx.background_executor().timer(Duration::from_millis(700)).await;
        cx.update(open);
    })
    .detach();
}

/// A window to start with, once the welcome's gone, if none's open.
fn first_window(cx: &mut App) {
    if crate::browser::front_folder(cx).is_none() {
        crate::browser::open(crate::finder::new_window_folder(cx), None, cx);
    }
}

pub fn open(cx: &mut App) {
    if let Some(handle) = cx.try_global::<Open>().and_then(|o| o.0)
        && crate::raise(handle, cx)
    {
        return;
    }
    // (Shown once: closed early, it's still been seen. Where it was, if F2
    // quit with it open.)
    let step = Settings::get(cx).welcome_step.unwrap_or(0).min(STEPS - 1);
    Settings::update(cx, |s| {
        s.onboarded = true;
        s.welcome_step = Some(step);
    });
    let bounds = Bounds::centered(None, size(px(920.), px(680.)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions { title: Some(format!("Welcome to {}", crate::APP_NAME).into()), appears_transparent: true, traffic_light_position: Some(point(px(18.), px(18.))) }),
        is_resizable: false,
        ..Default::default()
    };
    if let Ok((handle, _)) = gpui_kit::open_window(options, cx, |window, cx| cx.new(|cx| Onboarding::new(step, window, cx))) {
        cx.set_global(Open(Some(handle)));
    }
    cx.activate(true);
}

/// Each step's painting (the website's).
fn painting(step: usize) -> Arc<Image> {
    static PAINTINGS: OnceLock<Vec<Arc<Image>>> = OnceLock::new();
    PAINTINGS
        .get_or_init(|| {
            let jpegs: [&[u8]; STEPS] = [
                include_bytes!("../assets/onboarding/valley.jpg"),
                include_bytes!("../assets/onboarding/meadow.jpg"),
                include_bytes!("../assets/onboarding/lake.jpg"),
                include_bytes!("../assets/onboarding/dusk.jpg"),
                include_bytes!("../assets/onboarding/night.jpg"),
                include_bytes!("../assets/onboarding/golden.jpg"),
            ];
            jpegs.into_iter().map(|jpeg| Arc::new(Image::from_bytes(ImageFormat::Jpeg, jpeg.to_vec()))).collect()
        })[step.min(STEPS - 1)]
        .clone()
}

struct Onboarding {
    step: usize,
    /// The step before, its painting under this one's as it fades in.
    before: usize,
    /// Whether the Do Not Disturb shortcut's been added (None: not known
    /// yet; Shortcuts is asked while its step is up).
    dnd: Option<bool>,
    /// Whether a screen has a notch (else F2 hangs one from the menu bar).
    notched: bool,
    /// Asked for, of those macOS applies only once F2 reopens (so: Reopen
    /// F2, once it's turned on).
    asked: Vec<Permission>,
    _poll: Task<()>,
}

impl Onboarding {
    fn new(step: usize, window: &mut Window, cx: &mut Context<Self>) -> Self {
        // Permissions change in System Settings: looked at again each second
        // (and Shortcuts asked about Do Not Disturb's, on its step).
        let poll = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let Ok(day) = this.update(cx, |this, cx| {
                    cx.notify();
                    this.step == YOUR_DAY
                }) else {
                    break;
                };
                if day {
                    let have = cx.background_executor().spawn(async { crate::toggles::have_shortcut() }).await;
                    let kept = this.update(cx, |this, cx| {
                        if this.dnd != Some(have) {
                            this.dnd = Some(have);
                            cx.notify();
                        }
                    });
                    if kept.is_err() {
                        break;
                    }
                }
            }
        });
        // (Closed early: a window to start with all the same, and not
        // back the next time.)
        window.on_window_should_close(cx, |_, cx| {
            Settings::update(cx, |s| s.welcome_step = None);
            cx.defer(first_window);
            true
        });
        let notched = cx.displays().iter().any(|d| macos::notch_band(u64::from(d.id()) as u32).0 > 0.);
        Onboarding { step, before: step, dnd: None, notched, asked: Vec::new(), _poll: poll }
    }

    fn go(&mut self, step: usize, cx: &mut Context<Self>) {
        let step = step.min(STEPS - 1);
        if step != self.step {
            self.before = self.step;
            self.step = step;
            Settings::update(cx, |s| s.welcome_step = Some(step));
            cx.notify();
        }
    }

    fn finish(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        Settings::update(cx, |s| s.welcome_step = None);
        window.remove_window();
        cx.defer(first_window);
    }

    /// The headline in the sky, and what it's about.
    fn headline(&self, text: &'static str, about: &'static str) -> Div {
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(12.))
            .text_color(white())
            .child(div().max_w(px(760.)).text_center().text_size(px(46.)).line_height(relative(1.08)).font_weight(FontWeight::SEMIBOLD).child(text))
            .child(div().max_w(px(680.)).text_center().text_size(px(16.)).line_height(relative(1.5)).text_color(white().opacity(0.92)).child(about))
    }

    /// Paper, over the land.
    fn card(&self) -> Div {
        div()
            .flex_shrink_0()
            .rounded(px(20.))
            .bg(tint(PAPER, 0.97))
            .border_1()
            .border_color(white().opacity(0.55))
            .text_color(rgb(INK))
            .shadow(vec![
                BoxShadow { color: tint(NAVY, 0.32), offset: point(px(0.), px(24.)), blur_radius: px(56.), spread_radius: px(0.), inset: false },
                BoxShadow { color: tint(INK, 0.16), offset: point(px(0.), px(2.)), blur_radius: px(6.), spread_radius: px(0.), inset: false },
            ])
            .flex()
            .flex_col()
            .overflow_hidden()
    }

    /// A line of a card: its tile, name, what it's for (and anything under
    /// that), then what to do.
    fn row(&self, first: bool, sym: &'static str, color: u32, name: impl Into<SharedString>, about: impl Into<SharedString>, under: Option<AnyElement>, action: AnyElement, window: &Window) -> Div {
        div()
            .px(px(18.))
            .py(px(13.))
            .when(!first, |d| d.border_t_1().border_color(rgb(LINE)))
            .flex()
            .items_center()
            .gap(px(14.))
            .child(div().size(px(36.)).flex_shrink_0().rounded(px(10.)).bg(rgb(color)).flex().items_center().justify_center().child(symbol(sym, 17., SEMIBOLD, white(), window)))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(div().text_size(px(14.5)).font_weight(FontWeight::SEMIBOLD).child(name.into()))
                    .child(div().text_size(px(12.5)).line_height(relative(1.4)).text_color(rgb(INK2)).child(about.into()))
                    .children(under),
            )
            .child(div().flex_shrink_0().child(action))
    }

    /// The sky's button, in a card.
    fn button(&self, id: impl Into<ElementId>, label: impl Into<SharedString>, on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> AnyElement {
        div()
            .id(id)
            .h(px(30.))
            .px(px(14.))
            .rounded_full()
            .bg(rgb(SKY))
            .hover(|s| s.bg(rgb(SKY2)))
            .text_color(white())
            .text_size(px(13.))
            .font_weight(FontWeight::SEMIBOLD)
            .flex()
            .items_center()
            .child(label.into())
            .on_click(on_click)
            .into_any_element()
    }

    /// A quieter one, beside it.
    fn quiet_button(&self, id: impl Into<ElementId>, label: impl Into<SharedString>, on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> AnyElement {
        div()
            .id(id)
            .h(px(30.))
            .px(px(12.))
            .rounded_full()
            .border_1()
            .border_color(rgb(LINE))
            .bg(white())
            .hover(|s| s.bg(tint(SKY, 0.06)))
            .text_color(rgb(INK))
            .text_size(px(13.))
            .font_weight(FontWeight::MEDIUM)
            .flex()
            .items_center()
            .child(label.into())
            .on_click(on_click)
            .into_any_element()
    }

    /// A green tick, and what's so.
    fn done_mark(&self, label: &'static str, window: &Window) -> AnyElement {
        let green: Hsla = rgb(GREEN).into();
        div()
            .flex()
            .items_center()
            .gap(px(5.))
            .text_size(px(13.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(green)
            .child(symbol("checkmark.circle.fill", 15., SEMIBOLD, green, window))
            .child(label)
            .into_any_element()
    }

    /// A switch, on or off.
    fn switch(&self, on: bool) -> Div {
        div()
            .w(px(34.))
            .h(px(20.))
            .flex_shrink_0()
            .rounded_full()
            .bg(if on { rgb(SKY) } else { rgb(0xd9d4c7) })
            .p(px(2.))
            .flex()
            .when(on, |d| d.justify_end())
            .child(div().size(px(16.)).rounded_full().bg(white()).shadow(vec![BoxShadow { color: black().opacity(0.2), offset: point(px(0.), px(1.)), blur_radius: px(2.), spread_radius: px(0.), inset: false }]))
    }

    /// Words as typed into Quick Open.
    fn typed(&self, words: &'static str) -> Div {
        div().h(px(22.)).px(px(8.)).rounded(px(7.)).bg(white()).border_1().border_color(rgb(LINE)).font_family(MONO).text_size(px(11.5)).text_color(rgb(INK2)).flex().items_center().child(words)
    }

    /// Keycaps.
    fn keys(&self, keys: &str) -> Div {
        div().flex().gap(px(3.)).font_family(KEYS).children(keys.split(' ').map(|key| {
            div()
                .h(px(22.))
                .min_w(px(22.))
                .px(px(6.))
                .rounded(px(6.))
                .bg(white())
                .border_1()
                .border_color(rgb(LINE))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(12.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(INK))
                .child(key.to_string())
        }))
    }

    /// A choice of several, in a row: the one chosen in the sky's blue.
    fn pill(&self, id: impl Into<ElementId>, label: &'static str, on: bool, keys: bool, on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Stateful<Div> {
        div()
            .id(id)
            .h(px(26.))
            .px(px(11.))
            .rounded_full()
            .border_1()
            .border_color(if on { rgb(SKY) } else { rgb(LINE) })
            .bg(if on { rgb(SKY) } else { rgb(0xffffff) })
            .when(!on, |d| d.hover(|s| s.bg(tint(SKY, 0.06))))
            .text_color(if on { white() } else { rgb(INK).into() })
            .text_size(px(12.))
            .font_weight(FontWeight::MEDIUM)
            .when(keys, |d| d.font_family(KEYS))
            .flex()
            .items_center()
            .child(label)
            .on_click(on_click)
    }

    /// A line under a card, in white on the painting.
    fn note(&self, text: impl Into<SharedString>) -> Div {
        div().text_size(px(13.)).text_color(white().opacity(0.9)).child(text.into())
    }

    fn welcome(&self, window: &Window) -> Div {
        // (As the website says it: what it's for, in everyday words.)
        let features: [(&'static str, &'static str, &'static str); 6] = [
            ("magnifyingglass", "Find anything", "In plain words, like “photos from last week”."),
            ("chart.pie", "Free up space", "See what's filling your Mac, and clear what's safe to."),
            ("tray.2", "Tidy Downloads", "Loose files sorted into folders, in one click."),
            ("text.bubble", "Ask your Mac", "The time anywhere, a sum, the weather, a reminder."),
            ("camera.viewfinder", "Screenshots", "Capture, draw on it, and blur what's private."),
            ("doc.on.clipboard", "Everything you copied", "Kept, so nothing you copy is lost."),
        ];
        // Three to a line, as captions: a line icon in the sky's blue, the
        // name, what it does.
        let line = |features: &[(&'static str, &'static str, &'static str)]| {
            div().py(px(15.)).flex().gap(px(28.)).children(features.iter().map(|&(sym, name, about)| {
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .gap(px(11.))
                    .child(div().mt(px(1.)).w(px(20.)).flex_shrink_0().flex().justify_center().child(symbol(sym, 16., MEDIUM, rgb(SKY).into(), window)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).child(name))
                            .child(div().text_size(px(12.5)).line_height(relative(1.4)).text_color(rgb(INK2)).child(about)),
                    )
            }))
        };
        div()
            .h_full()
            .flex()
            .flex_col()
            .items_center()
            .child(
                // As the website's: the version, then what it is.
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .pl(px(6.))
                    .pr(px(14.))
                    .py(px(6.))
                    .rounded_full()
                    .bg(white().opacity(0.16))
                    .border_1()
                    .border_color(white().opacity(0.34))
                    .text_color(white())
                    .text_size(px(13.5))
                    .child(div().px(px(8.)).py(px(2.)).rounded_full().bg(white()).text_color(rgb(SKY2)).text_size(px(11.5)).font_weight(FontWeight::SEMIBOLD).child(env!("CARGO_PKG_VERSION")))
                    .child(format!("Welcome to {}", crate::APP_NAME)),
            )
            .child(div().mt(px(22.)).child(self.headline(
                "The Finder your Mac should have come with.",
                "Find any file in a few letters, see what's filling your Mac and clear it, and tidy Downloads in one click. Screenshots, clipboard history and quick answers come with it.",
            )))
            // The painting between, the sheet down on the land.
            .child(div().flex_1())
            .child(self.card().mb(px(6.)).w(px(800.)).px(px(26.)).py(px(4.)).child(line(&features[..3])).child(div().h(px(1.)).bg(rgb(LINE))).child(line(&features[3..])))
    }

    /// A permission's line: Allow…, or Allowed (and what to do in System
    /// Settings, under what it's for). One macOS applies only once F2
    /// reopens, once asked for: Reopen F2 (and System Settings again).
    fn permission(&self, i: usize, p: Permission, examples: Option<AnyElement>, window: &Window, cx: &mut Context<Self>) -> Div {
        let (name, about, sym, color) = p.about();
        let allowed = p.allowed();
        let waiting = !allowed && p.reopens() && self.asked.contains(&p);
        let hint = if waiting {
            Some(div().text_size(px(11.5)).text_color(rgb(INK2)).child("Turned F2 on? Reopen F2 to start using it, and you'll be back here."))
        } else {
            (!allowed).then(|| p.hint()).flatten().map(|hint| div().text_size(px(11.5)).text_color(rgb(INK3)).child(hint))
        };
        let under = div().flex().flex_col().gap(px(2.)).children(hint).children(examples).into_any_element();
        let action = if allowed {
            self.done_mark("Allowed", window)
        } else if waiting {
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(self.quiet_button(("settings", i), "Settings…", move |_, _, cx| p.ask(cx)))
                .child(self.button(("reopen", i), format!("Reopen {}", crate::APP_NAME), |_, _, cx| crate::permissions::reopen(cx)))
                .into_any_element()
        } else {
            self.button(
                ("allow", i),
                p.button(),
                cx.listener(move |this, _, _, cx| {
                    if p.reopens() && !this.asked.contains(&p) {
                        this.asked.push(p);
                    }
                    p.ask(cx);
                    cx.notify();
                }),
            )
        };
        self.row(i == 0, sym, color, name, about, Some(under), action, window)
    }

    fn permissions(&self, window: &Window, cx: &mut Context<Self>) -> Div {
        let allowed = Permission::CORE.iter().filter(|p| p.allowed()).count();
        let all = Permission::CORE.len();
        let rows: Vec<Div> = Permission::CORE.into_iter().enumerate().map(|(i, p)| self.permission(i, p, None, window, cx)).collect();
        div()
            .flex()
            .flex_col()
            .items_center()
            .child(self.headline(
                "A few permissions.",
                "macOS keeps apps out of your folders, your screen and other apps until you let them in. Allow what you'll use: each button opens the right place in System Settings.",
            ))
            .child(self.card().mt(px(30.)).w(px(680.)).children(rows))
            .child(div().mt(px(16.)).child(self.note(if allowed == all {
                "All set: every part of F2 can work.".to_string()
            } else {
                format!("{allowed} of {all} allowed. Full Disk Access and Screen Recording work once F2 reopens: the welcome comes back to this step.")
            })))
    }

    fn your_day(&self, window: &Window, cx: &mut Context<Self>) -> Div {
        let examples = |words: &[&'static str]| div().mt(px(6.)).flex().flex_wrap().gap(px(6.)).children(words.iter().map(|w| self.typed(w))).into_any_element();
        let dnd = match self.dnd {
            Some(true) => self.done_mark("Added", window),
            Some(false) => self.button("dnd", "Add Shortcut…", |_, _, cx| {
                cx.background_executor()
                    .spawn(async {
                        crate::toggles::offer_shortcut();
                    })
                    .detach();
            }),
            None => div().text_size(px(12.5)).text_color(rgb(INK3)).child("Looking…").into_any_element(),
        };
        div()
            .flex()
            .flex_col()
            .items_center()
            .child(self.headline(
                "Your day, in Quick Open.",
                "Say an event or a reminder as you'd say it to a friend, and F2 adds it. Only if you'd like: macOS asks you once for each.",
            ))
            .child(
                self.card()
                    .mt(px(30.))
                    .w(px(680.))
                    .child(self.permission(0, Permission::Calendar, Some(examples(&["lunch with Sara friday at 1", "next meeting"])), window, cx))
                    .child(self.permission(1, Permission::Reminders, Some(examples(&["remind me to call mom at 5pm", "remind me in 20 min"])), window, cx))
                    .child(self.row(
                        false,
                        "moon.fill",
                        0x5e5ce6,
                        "Do Not Disturb",
                        format!("Turned on and off from Quick Open. macOS lets apps change Focus only through Shortcuts, so {} brings one: Shortcuts asks to add it.", crate::APP_NAME),
                        Some(examples(&["do not disturb", "dnd off"])),
                        dnd,
                        window,
                    )),
            )
            .child(div().mt(px(16.)).child(self.note("None of these is needed: Continue leaves them for later.")))
    }

    fn shortcuts(&self, window: &Window, cx: &mut Context<Self>) -> Div {
        use crate::windows::Place;
        let settings = Settings::get(cx);
        let keyboard = "x-apple.systempreferences:com.apple.Keyboard-Settings.extension";
        let spotlight = settings.quick_open_shortcut == "cmd-space" && macos::spotlight_shortcut_on();
        let held = crate::shots::held_by_macos(&settings.shots);
        let preset = |pick: fn(Place) -> Option<&'static str>| Place::ALL.iter().all(|p| settings.window_shortcuts.get(p.key()).map(String::as_str) == pick(*p));
        let windows = if preset(Place::rectangle) {
            Some("Rectangle's")
        } else if preset(Place::spectacle) {
            Some("Spectacle's")
        } else if settings.window_shortcuts.is_empty() {
            None
        } else {
            Some("Your own")
        };
        // What's in the way, under what it's for (wrapping as it must),
        // with the button to fix it where Allow is on the other steps.
        let warning = |why: String| div().mt(px(6.)).text_size(px(12.)).line_height(relative(1.4)).text_color(rgb(0xb4530a)).child(why).into_any_element();
        let fix = |id: &'static str| self.button(id, "Open Shortcuts…", move |_, _, cx| cx.open_url(keyboard));
        let quick = div()
            .mt(px(8.))
            .flex()
            .flex_wrap()
            .gap(px(6.))
            .children(crate::quick_panel::SHORTCUTS.iter().map(|&(name, label)| {
                self.pill(SharedString::from(format!("qo-{name}")), label, settings.quick_open_shortcut == name, true, move |_, _, cx| Settings::update(cx, |s| s.quick_open_shortcut = name.to_string()))
            }))
            .when(spotlight, |d| d.child(div().w_full().child(warning("Spotlight still has ⌘Space: turn Spotlight's off in Keyboard Shortcuts › Spotlight.".into()))));
        // The screenshots' keys, as Settings have them.
        let shots = [(&settings.shots.area, "an area"), (&settings.shots.screen, "the screen"), (&settings.shots.text, "the text in an area")]
            .into_iter()
            .filter_map(|(key, what)| key.as_deref().map(|key| format!("{} {what}", crate::hotkeys::label(key))))
            .collect::<Vec<_>>();
        let pick = |pick: Option<fn(Place) -> Option<&'static str>>| {
            move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                Settings::update(cx, |s| {
                    s.window_shortcuts.clear();
                    if let Some(pick) = pick {
                        for place in Place::ALL {
                            if let Some(key) = pick(place) {
                                s.window_shortcuts.insert(place.key().into(), key.into());
                            }
                        }
                    }
                })
            }
        };
        let window_sets = div()
            .mt(px(8.))
            .flex()
            .gap(px(6.))
            .child(self.pill("w-rectangle", "Rectangle's", windows == Some("Rectangle's"), false, pick(Some(Place::rectangle))))
            .child(self.pill("w-spectacle", "Spectacle's", windows == Some("Spectacle's"), false, pick(Some(Place::spectacle))))
            .child(self.pill("w-none", "None", windows.is_none(), false, pick(None)));
        let ok = |good: bool| if good { self.done_mark("Ready", window) } else { div().into_any_element() };
        let quick_action = if spotlight { fix("spotlight") } else { ok(crate::quick_panel::shortcut(settings).is_some()) };
        let shots_action = if held.is_empty() { ok(!shots.is_empty()) } else { fix("held") };
        div()
            .flex()
            .flex_col()
            .items_center()
            .child(self.headline("Keys that work anywhere.", "Quick Open, screenshots and windows each have keys that work in every app. A few of macOS's own are in the way: here's what to turn off."))
            .child(
                self.card()
                    .mt(px(30.))
                    .w(px(680.))
                    .child(self.row(
                        true,
                        "magnifyingglass",
                        0x0a84ff,
                        "Quick Open",
                        match crate::quick_panel::shortcut(settings) {
                            Some(shortcut) => format!("{} opens it, in any app. Or record your own in Settings › Quick Open.", crate::hotkeys::label(shortcut)),
                            None => "No shortcut: pick one, or open it from the menu bar item.".into(),
                        },
                        Some(quick.into_any_element()),
                        quick_action,
                        window,
                    ))
                    .child(self.row(
                        false,
                        "camera.viewfinder",
                        crate::commands::SCREENSHOTS,
                        "Screenshots",
                        if shots.is_empty() { "No shortcuts yet: Settings › Screenshots records them.".to_string() } else { format!("{}, in any app.", shots.join(", ")) },
                        (!held.is_empty()).then(|| warning(format!("macOS still has {} for its own screenshots: turn those off in Keyboard Shortcuts › Screenshots.", held.join(" and ")))),
                        shots_action,
                        window,
                    ))
                    .child(self.row(
                        false,
                        "macwindow.on.rectangle",
                        crate::commands::WINDOWS,
                        "Windows",
                        match windows {
                            Some(which) => format!("{which} shortcuts, in any app: change them in Settings › Windows."),
                            None => "Halves, thirds and the next display, from the keyboard: pick a set you know.".into(),
                        },
                        Some(window_sets.into_any_element()),
                        ok(windows.is_some()),
                        window,
                    )),
            )
    }

    fn notch(&self, window: &Window, cx: &mut Context<Self>) -> Div {
        let on = Settings::get(cx).notch.on;
        let (headline, about, where_) = if self.notched {
            ("Your notch, put to work.", "What's going on shows beside it: a timer, a recording, what's playing. Notes come down from it, and pointing at it shows everything.", "beside the notch")
        } else {
            ("A notch of your own.", "What's going on shows at the top of the screen, from a notch that's there only while it's needed. Notes come down from it, and pointing at it shows everything.", "at the top of the screen")
        };
        let switch = div()
            .id("notch-on")
            .child(self.switch(on))
            .on_click(cx.listener(|_, _, _, cx| {
                Settings::update(cx, |s| s.notch.on = !s.notch.on);
                cx.notify();
            }))
            .into_any_element();
        let try_it = if on {
            self.button("notch-note", "Show a Note", |_, _, cx| {
                crate::shots::note("A Note".into(), "Notes come down from here. Point at the notch to see the last few again.".into(), None, cx)
            })
        } else {
            div().into_any_element()
        };
        div()
            .flex()
            .flex_col()
            .items_center()
            .child(self.headline(headline, about))
            .child(
                self.card()
                    .mt(px(30.))
                    .w(px(680.))
                    .child(self.row(
                        true,
                        "macbook",
                        0x1d1d1f,
                        "What's going on, in the notch",
                        format!("A timer counting down, the stopwatch, a screen recording, dictation listening, or what Music or Spotify is playing, {where_}."),
                        None,
                        switch,
                        window,
                    ))
                    .child(
                        self.row(
                            false,
                            "bell.fill",
                            0xff9f0a,
                            "Notes from the notch",
                            "A timer done, text copied, a permission missing: they come down from the notch, not into the corner.",
                            None,
                            try_it,
                            window,
                        )
                        .when(!on, |d| d.opacity(0.45)),
                    ),
            )
            .child(div().mt(px(16.)).child(self.note(if on { "On. Settings › Notch has the rest." } else { "Only if you'd like: Settings › Notch has it any time." })))
    }

    fn done(&self, cx: &mut Context<Self>) -> Div {
        // Keys (keycaps), or what's typed (in Quick Open, or anywhere).
        let tips: [(&str, bool, &str); 6] = [
            ("⌘ Space", true, "an app, a file, a command"),
            ("100 usd in eur", false, "money, units, sums"),
            ("time in tokyo", false, "the time anywhere"),
            ("photos from last week", false, "in plain words"),
            ("⇧ ⌘ 4", true, "capture an area"),
            ("clip", false, "what you've copied lately"),
        ];
        let login = macos::login_item_on();
        div()
            .flex()
            .flex_col()
            .items_center()
            .child(self.headline("Give Finder the afternoon off.", "Press ⌘Space and type the name of anything on your Mac. A few more to try:"))
            .child(
                self.card()
                    .mt(px(30.))
                    .w(px(680.))
                    .child(div().px(px(22.)).py(px(18.)).flex().flex_wrap().gap_y(px(12.)).children(tips.into_iter().map(|(keys, caps, what)| {
                        let shown = if caps { self.keys(keys) } else { self.typed(keys) };
                        div().w(px(312.)).flex().items_center().gap(px(10.)).child(shown).child(div().text_size(px(13.)).text_color(rgb(INK2)).child(what))
                    })))
                    .child(
                        div()
                            .id("login")
                            .px(px(22.))
                            .py(px(14.))
                            .border_t_1()
                            .border_color(rgb(LINE))
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .child(self.switch(login))
                            .child(div().text_size(px(13.5)).font_weight(FontWeight::MEDIUM).child(format!("Open {} at login", crate::APP_NAME)))
                            .child(div().text_size(px(12.5)).text_color(rgb(INK3)).child("so Quick Open and the shortcuts are always there"))
                            .on_click(cx.listener(move |_, _, _, cx| {
                                macos::set_login_item(!login);
                                cx.notify();
                            })),
                    ),
            )
    }
}

impl Render for Onboarding {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = match self.step {
            0 => self.welcome(window),
            1 => self.permissions(window, cx),
            YOUR_DAY => self.your_day(window, cx),
            3 => self.shortcuts(window, cx),
            NOTCH => self.notch(window, cx),
            _ => self.done(cx),
        };
        let last = self.step == STEPS - 1;
        let scene = |step: usize| img(painting(step)).absolute().inset_0().size_full().object_fit(ObjectFit::Cover);
        div()
            .size_full()
            .relative()
            .overflow_hidden()
            .bg(rgb(0x1a47a6))
            .font_family(GEIST)
            .text_size(px(13.))
            // The painting (the one before under it, as it fades in).
            .when(self.before != self.step, |d| d.child(scene(self.before)))
            .child(scene(self.step).with_animation(("painting", self.step), Animation::new(Duration::from_millis(650)).with_easing(ease_out_quint()), |el, t| el.opacity(t)))
            // The sky darker behind the headline, and the ground behind the
            // buttons, as the website's hero is.
            .child(div().absolute().inset_0().bg(linear_gradient(180., linear_color_stop(tint(0x0e286e, 0.46), 0.), linear_color_stop(tint(0x0e286e, 0.), 0.55))))
            .child(div().absolute().inset_0().bg(linear_gradient(180., linear_color_stop(tint(0x08142e, 0.), 0.7), linear_color_stop(tint(0x08142e, 0.42), 1.))))
            // (Its top drags it, as a title bar does.)
            .child(div().id("drag").absolute().top_0().left(px(80.)).right_0().h(px(44.)).on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move()))
            .child(
                div()
                    .absolute()
                    .top(px(56.))
                    .left_0()
                    .right_0()
                    .bottom(px(84.))
                    .flex()
                    .justify_center()
                    .child(body.with_animation(("step", self.step), Animation::new(Duration::from_millis(500)).with_easing(ease_out_quint()), |el, t| el.opacity(t).mt(px(16. * (1. - t))))),
            )
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .right_0()
                    .h(px(84.))
                    .px(px(34.))
                    .flex()
                    .items_center()
                    .child(div().flex().gap(px(7.)).children((0..STEPS).map(|i| {
                        div()
                            .id(("dot", i))
                            .h(px(8.))
                            .w(px(if i == self.step { 24. } else { 8. }))
                            .rounded_full()
                            .bg(if i == self.step { white() } else { white().opacity(0.45) })
                            .on_click(cx.listener(move |this, _, _, cx| this.go(i, cx)))
                    })))
                    .child(div().flex_1())
                    .when(self.step > 0, |d| {
                        d.child(
                            div()
                                .id("back")
                                .h(px(42.))
                                .px(px(20.))
                                .rounded(px(13.))
                                .bg(tint(NAVY, 0.34))
                                .hover(|s| s.bg(tint(NAVY, 0.48)))
                                .border_1()
                                .border_color(white().opacity(0.3))
                                .text_color(white())
                                .text_size(px(15.))
                                .flex()
                                .items_center()
                                .child("Back")
                                .on_click(cx.listener(|this, _, _, cx| this.go(this.step.saturating_sub(1), cx))),
                        )
                    })
                    .child(
                        div()
                            .id("next")
                            .ml(px(10.))
                            .h(px(42.))
                            .px(px(24.))
                            .rounded(px(13.))
                            .bg(white())
                            .hover(|s| s.bg(rgb(0xeaf0fc)))
                            .text_color(rgb(INK))
                            .text_size(px(15.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .flex()
                            .items_center()
                            .shadow(vec![BoxShadow { color: tint(0x0a1e5a, 0.28), offset: point(px(0.), px(10.)), blur_radius: px(30.), spread_radius: px(0.), inset: false }])
                            .child(match self.step {
                                0 => "Get Started",
                                s if s == STEPS - 1 => "Start Using F2",
                                _ => "Continue",
                            })
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if last {
                                    this.finish(window, cx);
                                } else {
                                    this.go(this.step + 1, cx);
                                }
                            })),
                    ),
            )
    }
}
