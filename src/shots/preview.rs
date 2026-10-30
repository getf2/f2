//! The corner: a screenshot just taken, as macOS and CleanShot show one,
//! at the bottom right of the screen the pointer's on. Drag it into an
//! app (it's a file there); click it to edit it; over it, buttons copy,
//! save, pin or edit it, and × puts it away. It goes by itself after a
//! while, unless the pointer's on it. Newer ones stack above. Notes (text
//! copied, a color picked, a permission missing, a timer done) show here
//! too, as macOS's notifications look: the system's frosted material,
//! rounded, with its own shadow, and wider on a wider screen.

use std::path::PathBuf;
use std::time::Duration;

use gpui::prelude::FluentBuilder as _;
use gpui::*;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

use crate::macos;
use crate::style::{Palette, SEMIBOLD, symbol};

/// The card's width, and its image's height at most.
const WIDTH: f32 = 240.;
const IMAGE: f32 = 160.;
/// A note's height (with a line or two under its title, or none), and its
/// width at least and most (a fifth of the screen, between).
const NOTE: f32 = 78.;
const NOTE_SHORT: f32 = 60.;
const NOTE_WIDTH: (f32, f32) = (360., 440.);
const NOTE_RADIUS: f32 = 18.;
/// Room around a screenshot's card for its shadow, within the window (a
/// note's is the window's own).
const MARGIN: f32 = 14.;
/// From the screen's edges, and between cards.
const EDGE: f32 = 14.;
const SPACING: f32 = 10.;
const RADIUS: f32 = 12.;
/// How long one stays, and after the pointer leaves it.
const STAYS: Duration = Duration::from_secs(8);
const LINGERS: Duration = Duration::from_secs(3);

#[derive(Clone)]
enum Shown {
    Shot(PathBuf),
    /// A title, what it's about, a color's swatch, and a link it opens.
    Note { title: SharedString, detail: SharedString, color: Option<u32>, link: Option<&'static str> },
    /// A screen recording: its file, a frame of it, how long it is.
    Video { path: PathBuf, frame: Option<PathBuf>, secs: u64 },
}

/// The cards up, and their cards' heights.
#[derive(Default)]
struct Corner(Vec<(AnyWindowHandle, f32)>);

impl Global for Corner {}

pub fn show(path: PathBuf, cx: &mut App) {
    let height = match macos::image_info(&path) {
        Some((w, h, _)) if w > 0 => (WIDTH * h as f32 / w as f32).clamp(90., IMAGE),
        _ => IMAGE,
    };
    open(Shown::Shot(path), height, cx);
}

/// A screen recording, just saved: drag it into an app, click it to play
/// it, or show it in its folder.
pub fn video(path: PathBuf, frame: Option<PathBuf>, secs: u64, cx: &mut App) {
    let height = match frame.as_deref().and_then(macos::image_info) {
        Some((w, h, _)) if w > 0 => (WIDTH * h as f32 / w as f32).clamp(90., IMAGE),
        _ => 135.,
    };
    open(Shown::Video { path, frame, secs }, height, cx);
}

pub fn note(title: String, detail: String, color: Option<u32>, link: Option<&'static str>, cx: &mut App) {
    // (From the notch instead, if Settings says so.)
    if crate::notch::note(&title, &detail, color, kind(&title, link.is_some()), link) {
        return;
    }
    let height = if detail.trim().is_empty() { NOTE_SHORT } else { NOTE };
    open(Shown::Note { title: title.into(), detail: detail.into(), color, link }, height, cx);
}

/// A note's symbol and its tile's color: done, a problem, or just so.
fn kind(title: &str, linked: bool) -> (&'static str, u32) {
    let lower = title.to_lowercase();
    if linked || ["couldn't", "off limits", "didn't", "no ", "not "].iter().any(|w| lower.contains(w)) {
        ("exclamationmark.triangle.fill", 0xff9f0a)
    } else if ["copied", "added", "connected", "saved", "allowed", "set", "sound on", "ran ", "done", "stopped", "cancelled"].iter().any(|w| lower.contains(w)) {
        ("checkmark", 0x30d158)
    } else if lower.contains("time's up") || lower.contains("alarm") {
        ("bell.fill", 0xff453a)
    } else {
        ("info", 0x0a84ff)
    }
}

fn open(shown: Shown, card: f32, cx: &mut App) {
    let Some(display) = crate::screens::with_pointer(cx) else { return };
    let visible = display.visible_bounds();
    // Above those up already.
    let mut corner = cx.try_global::<Corner>().map(|c| c.0.clone()).unwrap_or_default();
    corner.retain(|(handle, _)| handle.update(cx, |_, _, _| ()).is_ok());
    let below: f32 = corner.iter().map(|(_, h)| h + SPACING).sum();
    // (A screenshot's window has room round its card for the shadow it
    // draws; a note's is the card, the shadow macOS's.)
    let (width, margin) = match shown {
        Shown::Shot(_) | Shown::Video { .. } => (WIDTH, MARGIN),
        Shown::Note { .. } => ((f32::from(visible.size.width) / 5.).clamp(NOTE_WIDTH.0, NOTE_WIDTH.1), 0.),
    };
    let size = size(px(width + 2. * margin), px(card + 2. * margin));
    let origin = point(
        visible.origin.x + visible.size.width - size.width - px(EDGE - margin),
        visible.origin.y + visible.size.height - size.height - px(EDGE - margin) - px(below),
    );
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds { origin, size })),
        titlebar: None,
        kind: WindowKind::PopUp,
        focus: false,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        display_id: Some(display.id()),
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    };
    match gpui_kit::open_window(options, cx, |window, cx| cx.new(|cx| Preview::new(shown, window, cx))) {
        Ok((handle, _)) => {
            handle
                .update(cx, |_, window, cx| {
                    if let Some(Some(root)) = window.root::<gpui_kit::base::Root>() {
                        root.update(cx, |root, _| root.style().background = Some(transparent_black().into()));
                    }
                })
                .ok();
            corner.push((handle, card));
            cx.set_global(Corner(corner));
        }
        Err(e) => eprintln!("couldn't show the screenshot: {e}"),
    }
}

/// A screenshot being dragged out (to an app, as its file).
#[derive(Clone)]
struct Dragged(PathBuf);

impl Render for Dragged {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        img(self.0.clone()).w(px(120.)).h(px(80.)).object_fit(ObjectFit::Contain).opacity(0.85)
    }
}

struct Preview {
    shown: Shown,
    hovered: bool,
    /// What was done ("Copied", "Saved"), said for a moment.
    done: Option<SharedString>,
    palette: Palette,
    native: bool,
    leave: Task<()>,
}

impl Preview {
    fn new(shown: Shown, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut this = Preview { shown, hovered: false, done: None, palette: Palette::for_window(window), native: false, leave: Task::ready(()) };
        this.leave_after(STAYS, window, cx);
        this
    }

    /// Go after `after`, unless the pointer's on it then.
    fn leave_after(&mut self, after: Duration, window: &mut Window, cx: &mut Context<Self>) {
        self.leave = cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(after).await;
            this.update_in(cx, |this, window, cx| {
                if !this.hovered {
                    this.close(window, cx);
                }
            })
            .ok();
        });
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let me = Window::window_handle(window);
        if let Some(corner) = cx.try_global::<Corner>() {
            let rest = corner.0.iter().filter(|(h, _)| *h != me).cloned().collect();
            cx.set_global(Corner(rest));
        }
        window.remove_window();
    }

    fn path(&self) -> Option<PathBuf> {
        match &self.shown {
            Shown::Shot(path) => Some(path.clone()),
            Shown::Note { .. } | Shown::Video { .. } => None,
        }
    }

    fn edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(path) = self.path() {
            self.close(window, cx);
            super::edit(path, cx);
        }
    }

    fn copy(&mut self, cx: &mut Context<Self>) {
        if let Some(path) = self.path() {
            let copied = macos::copy_image(&path);
            self.done = Some(if copied { "Copied".into() } else { "Couldn't Copy".into() });
            cx.notify();
        }
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        if let Some(path) = self.path() {
            self.done = Some(match super::save(&path, cx) {
                Ok(to) => format!("Saved to {}", to.parent().and_then(|d| d.file_name()).map_or("".into(), |n| n.to_string_lossy())).into(),
                Err(_) => "Couldn't Save".into(),
            });
            cx.notify();
        }
    }

    fn pin(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(path) = self.path() {
            self.close(window, cx);
            super::pin::open(path, cx);
        }
    }

    /// A round button over the image.
    fn button(&self, id: &'static str, name: &'static str, tip: &'static str, window: &Window) -> Stateful<Div> {
        div()
            .id(id)
            .size(px(34.))
            .rounded_full()
            .bg(black().opacity(0.55))
            .hover(|d| d.bg(black().opacity(0.75)))
            .border_1()
            .border_color(white().opacity(0.25))
            .flex()
            .items_center()
            .justify_center()
            .child(symbol(name, 14., SEMIBOLD, white(), window))
            .tooltip(move |window, cx| gpui_kit::component::tooltip::Tooltip::new(tip).build(window, cx))
    }

    fn render_shot(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let hovered = self.hovered;
        let drag = Dragged(path.clone());
        div()
            .id("shot")
            .size_full()
            .relative()
            .bg(black().opacity(0.8))
            // (GPUI clips square: the image rounds its own corners.)
            .child(img(path).size_full().object_fit(ObjectFit::Cover).rounded(px(RADIUS - 1.)))
            .on_drag(drag, |d, _, _, cx| cx.new(|_| d.clone()))
            .external_drag_payload(|d: &Dragged, _, _| Some(ExternalDragPayload::Files(FileDragPaths::new([(d.0.clone(), false)]))))
            .on_click(cx.listener(|this, _, window, cx| this.edit(window, cx)))
            .when(hovered, |d| {
                d.child(
                    div()
                        .absolute()
                        .inset_0()
                        .bg(black().opacity(0.35))
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap_2()
                        .child(self.button("edit", "pencil.tip", "Edit", window).on_click(cx.listener(|this, _, window, cx| {
                            cx.stop_propagation();
                            this.edit(window, cx)
                        })))
                        .child(self.button("copy", "doc.on.doc", "Copy", window).on_click(cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            this.copy(cx)
                        })))
                        .child(self.button("save", "square.and.arrow.down", "Save", window).on_click(cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            this.save(cx)
                        })))
                        .child(self.button("pin", "pin", "Pin over everything", window).on_click(cx.listener(|this, _, window, cx| {
                            cx.stop_propagation();
                            this.pin(window, cx)
                        }))),
                )
            })
            .when_some(self.done.clone(), |d, done| {
                d.child(
                    div().absolute().bottom(px(8.)).left_0().right_0().flex().justify_center().child(
                        div().px_2().py(px(3.)).rounded_full().bg(black().opacity(0.7)).text_color(white()).text_size(px(11.)).font_weight(FontWeight::MEDIUM).child(done),
                    ),
                )
            })
            .into_any_element()
    }

    /// A recording's card: a frame of it, how long it is; clicked, played;
    /// over it, Show in Folder and Copy; dragged, the file.
    fn render_video(&mut self, path: PathBuf, frame: Option<PathBuf>, secs: u64, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let hovered = self.hovered;
        let drag = Dragged(frame.clone().unwrap_or_else(|| path.clone()));
        let file = path.clone();
        div()
            .id("video")
            .size_full()
            .relative()
            .bg(black().opacity(0.85))
            .when_some(frame, |d, frame| d.child(img(frame).size_full().object_fit(ObjectFit::Cover).rounded(px(RADIUS - 1.))))
            .child(div().absolute().inset_0().flex().items_center().justify_center().child(symbol("play.circle.fill", 34., SEMIBOLD, white().opacity(0.9), window)))
            .child(
                div()
                    .absolute()
                    .bottom(px(8.))
                    .right(px(8.))
                    .px(px(6.))
                    .py(px(2.))
                    .rounded(px(5.))
                    .bg(black().opacity(0.65))
                    .text_color(white())
                    .text_size(px(11.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(super::record::clock(secs)),
            )
            .on_drag(drag, |d, _, _, cx| cx.new(|_| d.clone()))
            .external_drag_payload({
                let file = file.clone();
                move |_: &Dragged, _, _| Some(ExternalDragPayload::Files(FileDragPaths::new([(file.clone(), false)])))
            })
            .on_click({
                let file = file.clone();
                cx.listener(move |this, _, window, cx| {
                    cx.open_with_system(&file);
                    this.close(window, cx);
                })
            })
            .when(hovered, |d| {
                let (folder, copied, recorded) = (file.clone(), file.clone(), file.clone());
                d.child(
                    div()
                        .absolute()
                        .top(px(8.))
                        .right(px(8.))
                        .flex()
                        .gap_2()
                        .child(self.button("reveal", "folder", "Show in Folder", window).on_click(cx.listener(move |this, _, window, cx| {
                            cx.stop_propagation();
                            cx.reveal_path(&folder);
                            this.close(window, cx);
                        })))
                        .child(self.button("copy", "doc.on.doc", "Copy", window).on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            macos::copy_files(&[copied.as_path()]);
                            this.done = Some("Copied".into());
                            cx.notify();
                        })))
                        // A GIF of it, beside it, shown in its folder (for where
                        // a video won't play).
                        .child(self.button("gif", "rectangle.stack.badge.play", "Save as GIF", window).on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            if this.done.as_ref().is_some_and(|d| d.starts_with("Making")) {
                                return;
                            }
                            this.done = Some("Making a GIF…".into());
                            cx.notify();
                            let video = recorded.clone();
                            cx.spawn(async move |this, cx| {
                                let made = cx.background_executor().spawn(async move { super::record::to_gif(&video) }).await;
                                // (Shown even if the card's gone meanwhile.)
                                if let Ok(gif) = &made {
                                    let gif = gif.clone();
                                    cx.update(|cx| cx.reveal_path(&gif));
                                }
                                this.update(cx, |this, cx| {
                                    this.done = Some(match made {
                                        Ok(_) => "Saved as GIF".into(),
                                        Err(e) => format!("No GIF: {e}").into(),
                                    });
                                    cx.notify();
                                })
                                .ok();
                            })
                            .detach();
                        }))),
                )
            })
            .when_some(self.done.clone(), |d, done| {
                d.child(
                    div().absolute().bottom(px(8.)).left_0().right_0().flex().justify_center().child(
                        div().px_2().py(px(3.)).rounded_full().bg(black().opacity(0.7)).text_color(white()).text_size(px(11.)).font_weight(FontWeight::MEDIUM).child(done),
                    ),
                )
            })
            .into_any_element()
    }

    fn render_note(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Shown::Note { title, detail, color, link } = self.shown.clone() else { return div().into_any_element() };
        let palette = self.palette;
        let (name, tile) = kind(&title, link.is_some());
        div()
            .id("note")
            .size_full()
            .pl(px(16.))
            .pr(px(18.))
            .flex()
            .items_center()
            .gap(px(14.))
            .child(match color {
                Some(color) => div().size(px(38.)).flex_shrink_0().rounded(px(10.)).bg(rgb(color)).border_1().border_color(palette.glass_border).into_any_element(),
                None => div()
                    .size(px(38.))
                    .flex_shrink_0()
                    .rounded(px(10.))
                    .bg(rgb(tile))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(symbol(name, 17., SEMIBOLD, white(), window))
                    .into_any_element(),
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(div().text_size(px(15.)).font_weight(FontWeight::SEMIBOLD).text_color(palette.label).truncate().child(title))
                    .when(!detail.trim().is_empty(), |d| d.child(div().text_size(px(13.)).line_height(relative(1.3)).text_color(palette.secondary).line_clamp(2).child(detail))),
            )
            .when_some(link, |d, link| d.cursor_pointer().on_click(move |_, _, cx| cx.open_url(link)))
            .on_click(cx.listener(|this, _, window, cx| this.close(window, cx)))
            // (In from the side, as macOS's notifications come.)
            .with_animation("in", Animation::new(Duration::from_millis(320)).with_easing(ease_out_quint()), |el, t| el.opacity(t).ml(px(24. * (1. - t))))
            .into_any_element()
    }
}

impl Render for Preview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !std::mem::replace(&mut self.native, true)
            && let Ok(handle) = window.window_handle()
            && let RawWindowHandle::AppKit(appkit) = handle.as_raw()
        {
            let view = appkit.ns_view.as_ptr();
            let note = matches!(self.shown, Shown::Note { .. });
            cx.spawn(async move |_, _| if note { macos::panel_setup(view, NOTE_RADIUS as f64) } else { macos::panel_bare(view) }).detach();
        }
        let palette = self.palette;
        let note = matches!(self.shown, Shown::Note { .. });
        let body = match self.shown.clone() {
            Shown::Shot(path) => self.render_shot(path, window, cx),
            Shown::Video { path, frame, secs } => self.render_video(path, frame, secs, window, cx),
            Shown::Note { .. } => self.render_note(window, cx),
        };
        if note {
            return div()
                .id("card")
                .size_full()
                .relative()
                .child(body)
                .on_hover(cx.listener(|this, hovered: &bool, window, cx| {
                    this.hovered = *hovered;
                    if !*hovered {
                        this.leave_after(LINGERS, window, cx);
                    }
                    cx.notify();
                }))
                .child(
                    div()
                        .id("close")
                        .absolute()
                        .top(px(5.))
                        .left(px(5.))
                        .size(px(18.))
                        .rounded_full()
                        .bg(palette.label.opacity(0.55))
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(!self.hovered, |d| d.invisible())
                        .child(symbol("xmark", 8., SEMIBOLD, palette.background, window))
                        .on_click(cx.listener(|this, _, window, cx| {
                            cx.stop_propagation();
                            this.close(window, cx)
                        })),
                )
                .into_any_element();
        }
        div().size_full().p(px(MARGIN)).child(
            div()
                .id("card")
                .size_full()
                .relative()
                .rounded(px(RADIUS))
                .overflow_hidden()
                .border_1()
                .border_color(palette.glass_border)
                .shadow(vec![BoxShadow { color: black().opacity(0.3), offset: point(px(0.), px(4.)), blur_radius: px(12.), spread_radius: px(0.), inset: false }])
                .child(body)
                .on_hover(cx.listener(|this, hovered: &bool, window, cx| {
                    this.hovered = *hovered;
                    if !*hovered {
                        this.done = None;
                        this.leave_after(LINGERS, window, cx);
                    }
                    cx.notify();
                }))
                .child(
                    div()
                        .id("close")
                        .absolute()
                        .top(px(6.))
                        .left(px(6.))
                        .size(px(20.))
                        .rounded_full()
                        .bg(black().opacity(0.6))
                        .border_1()
                        .border_color(white().opacity(0.3))
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(!self.hovered, |d| d.invisible())
                        .child(symbol("xmark", 9., SEMIBOLD, white(), window))
                        .on_click(cx.listener(|this, _, window, cx| {
                            cx.stop_propagation();
                            this.close(window, cx)
                        })),
                ),
        )
        .into_any_element()
    }
}
