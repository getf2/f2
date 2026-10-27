//! The screenshot editor, as BetterShot's and CleanShot's: arrows, lines,
//! rectangles, ellipses, a pen, a highlighter, text, numbered steps, blur,
//! and crop, in a color and a width; marks picked, moved, deleted; undone
//! and redone. A background (one of ten gradients, with padding, round
//! corners and a shadow) sets it off. Copy (⌘C), Save (⌘S), or Done (⌘↩:
//! copied, and closed). Keys pick tools: V A L R O D H T 1 B C.
//!
//! The marks are in the image's own pixels (see `marks`); the view scales
//! them, and the export draws the same `Prim`s at full size.

use std::path::PathBuf;

use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_kit::component::input::{Input, InputState};
use lyon_tessellation::{LineCap, LineJoin, StrokeOptions};

use super::marks::{self, BACKGROUNDS, Background, Kind, Mark, P, Prim, R, Tool, p, rect};
use crate::macos;
use crate::style::{MEDIUM, Palette, symbol};

const TOOLBAR: f32 = 48.;
/// The background's options, under the toolbar while it's on.
const OPTIONS: f32 = 44.;
/// Around the image, in the window.
const MARGIN: f32 = 32.;
const COLORS: [u32; 8] = [0xff3b30, 0xff9500, 0xffcc00, 0x34c759, 0x007aff, 0xaf52de, 0x000000, 0xffffff];
/// Line widths, in points: small, medium, large.
const WIDTHS: [f32; 3] = [3., 5., 8.];
const PADDINGS: [f32; 3] = [32., 64., 96.];
const RADII: [f32; 3] = [0., 12., 24.];

pub fn open(path: PathBuf, cx: &mut App) {
    let Some((w, h, scale)) = macos::image_info(&path) else {
        return eprintln!("couldn't open {} to edit", path.display());
    };
    let display = crate::screens::with_pointer(cx);
    let visible = display.as_ref().map_or(Bounds { origin: point(px(0.), px(0.)), size: size(px(1440.), px(900.)) }, |d| d.visible_bounds());
    // Its own size, with room for the toolbar: at most most of the screen,
    // and not huge (a whole wide screen's shot is shown smaller).
    let (pw, ph) = (w as f32 / scale + 2. * MARGIN, h as f32 / scale + 2. * MARGIN + TOOLBAR + 28.);
    let width = pw.min(f32::from(visible.size.width) * 0.9).min(1800.).max(720.);
    let height = ph.min(f32::from(visible.size.height) * 0.9).min(1200.).max(480.);
    let bounds = Bounds { origin: visible.center() - point(px(width / 2.), px(height / 2.)), size: size(px(width), px(height)) };
    let title = path.file_stem().map_or("Screenshot".into(), |s| s.to_string_lossy().into_owned());
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions { title: Some(title.into()), ..Default::default() }),
        window_min_size: Some(size(px(640.), px(400.))),
        display_id: display.map(|d| d.id()),
        ..Default::default()
    };
    if let Err(e) = gpui_kit::open_window(options, cx, |window, cx| cx.new(|cx| Editor::new(path, (w as f32, h as f32), scale, window, cx))) {
        eprintln!("couldn't open the editor: {e}");
    }
    cx.activate(true);
}

/// What undo goes back to.
#[derive(Clone, PartialEq)]
struct State {
    marks: Vec<Mark>,
    crop: Option<R>,
    background: Option<Background>,
}

enum Gesture {
    /// A new mark, following the pointer.
    Drawing(Mark),
    /// The selected mark, moved from where it was when picked up.
    Moving { ix: usize, from: P, original: Mark, moved: bool },
    Cropping { from: P },
}

/// Where things are in the window: image pixels to points.
#[derive(Clone, Copy)]
struct Layout {
    /// The part of the image shown (all of it while cropping).
    shown: R,
    /// Points per image pixel.
    k: f32,
    /// Where the shown part's top left is, in the window.
    origin: Point<Pixels>,
    /// The background around it, in the window, if any.
    backdrop: Option<Bounds<Pixels>>,
}

impl Layout {
    fn to_view(&self, q: P) -> Point<Pixels> {
        point(self.origin.x + px((q.x - self.shown.x) * self.k), self.origin.y + px((q.y - self.shown.y) * self.k))
    }

    fn to_image(&self, v: Point<Pixels>) -> P {
        p(self.shown.x + f32::from(v.x - self.origin.x) / self.k, self.shown.y + f32::from(v.y - self.origin.y) / self.k)
    }

    fn frame(&self) -> Bounds<Pixels> {
        Bounds { origin: self.origin, size: size(px(self.shown.w * self.k), px(self.shown.h * self.k)) }
    }
}

pub struct Editor {
    path: PathBuf,
    /// The image's size, in pixels, and its pixels per point.
    pixels: (f32, f32),
    scale: f32,
    /// The image pixelated all over, once made: the blur's look.
    squares: Option<PathBuf>,
    state: State,
    undo: Vec<State>,
    redo: Vec<State>,
    tool: Tool,
    color: u32,
    /// The line width, in points.
    width: f32,
    gesture: Option<Gesture>,
    selected: Option<usize>,
    /// A text being typed, and where.
    writing: Option<(Entity<InputState>, P)>,
    /// The background as it was set, to bring back when it's turned on.
    last_background: Background,
    notice: Option<SharedString>,
    notice_task: Task<()>,
    focus: FocusHandle,
    palette: Palette,
    _subscriptions: Vec<Subscription>,
}

impl Editor {
    fn new(path: PathBuf, pixels: (f32, f32), scale: f32, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        // The blur's squares, made once, off the main thread.
        let source = path.clone();
        cx.spawn(async move |this, cx| {
            let out = std::env::temp_dir().join(format!("finder2-squares-{}.png", std::process::id() as u64 * 1000 + rand_ish()));
            let made = cx.background_executor().spawn({
                let out = out.clone();
                async move { macos::pixelate_file(&source, &out, marks::pixel(scale)).then_some(out) }
            });
            if let Some(out) = made.await {
                this.update(cx, |this, cx| {
                    this.squares = Some(out);
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
        let subscriptions = vec![cx.observe_window_appearance(window, |this, window, cx| {
            this.palette = Palette::for_window(window);
            cx.notify();
        })];
        cx.on_release(|this, _| {
            if let Some(squares) = &this.squares {
                let _ = std::fs::remove_file(squares);
            }
        })
        .detach();
        Editor {
            path,
            pixels,
            scale: scale.max(1.),
            squares: None,
            state: State { marks: Vec::new(), crop: None, background: None },
            undo: Vec::new(),
            redo: Vec::new(),
            tool: Tool::Arrow,
            color: COLORS[0],
            width: WIDTHS[1],
            gesture: None,
            selected: None,
            writing: None,
            last_background: Background::default(),
            notice: None,
            notice_task: Task::ready(()),
            focus,
            palette: Palette::for_window(window),
            _subscriptions: subscriptions,
        }
    }

    fn layout(&self, window: &Window) -> Layout {
        let view = window.viewport_size();
        let top = TOOLBAR + if self.state.background.is_some() { OPTIONS } else { 0. };
        let full = R { x: 0., y: 0., w: self.pixels.0, h: self.pixels.1 };
        let shown = match (self.tool, self.state.crop) {
            (Tool::Crop, _) | (_, None) => full,
            (_, Some(crop)) => crop,
        };
        let pad = self.state.background.map_or(0., |b| b.padding * self.scale);
        let (cw, ch) = (shown.w + 2. * pad, shown.h + 2. * pad);
        let (aw, ah) = (f32::from(view.width) - 2. * MARGIN, f32::from(view.height) - top - 2. * MARGIN);
        // (No bigger than its own size.)
        let k = (aw / cw).min(ah / ch).min(1. / self.scale).max(0.01);
        let corner = point(px((f32::from(view.width) - cw * k) / 2.), px(top + MARGIN + (ah - ch * k) / 2.));
        Layout {
            shown,
            k,
            origin: corner + point(px(pad * k), px(pad * k)),
            backdrop: self.state.background.map(|_| Bounds { origin: corner, size: size(px(cw * k), px(ch * k)) }),
        }
    }

    fn checkpoint(&mut self) {
        self.undo.push(self.state.clone());
        self.redo.clear();
    }

    fn undo(&mut self, cx: &mut Context<Self>) {
        if let Some(state) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.state, state));
            self.selected = None;
            cx.notify();
        }
    }

    fn redo(&mut self, cx: &mut Context<Self>) {
        if let Some(state) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.state, state));
            self.selected = None;
            cx.notify();
        }
    }

    fn set_tool(&mut self, tool: Tool, window: &mut Window, cx: &mut Context<Self>) {
        self.commit_text(window, cx);
        self.tool = tool;
        if tool != Tool::Select {
            self.selected = None;
        }
        cx.notify();
    }

    /// The color, for what's drawn next (and the mark picked).
    fn set_color(&mut self, color: u32, cx: &mut Context<Self>) {
        self.color = color;
        if let Some(ix) = self.selected {
            self.checkpoint();
            self.state.marks[ix].color = color;
        }
        cx.notify();
    }

    fn set_width(&mut self, width: f32, cx: &mut Context<Self>) {
        self.width = width;
        if let Some(ix) = self.selected {
            self.checkpoint();
            self.state.marks[ix].width = width * self.scale;
        }
        cx.notify();
    }

    fn set_background(&mut self, background: Option<Background>, cx: &mut Context<Self>) {
        self.checkpoint();
        if let Some(b) = background {
            self.last_background = b;
        }
        self.state.background = background;
        cx.notify();
    }

    fn delete_selected(&mut self, cx: &mut Context<Self>) {
        if let Some(ix) = self.selected.take() {
            self.checkpoint();
            self.state.marks.remove(ix);
            cx.notify();
        }
    }

    /// The next step's number.
    fn next_step(&self) -> u32 {
        self.state.marks.iter().filter_map(|m| if let Kind::Counter(_, n) = m.kind { Some(n) } else { None }).max().unwrap_or(0) + 1
    }

    fn mark(&self, kind: Kind) -> Mark {
        Mark { kind, color: self.color, width: self.width * self.scale }
    }

    /// Keep the text typed (if any), and stop typing.
    fn commit_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((input, at)) = self.writing.take() {
            let text = input.read(cx).value().to_string();
            if !text.trim().is_empty() {
                self.checkpoint();
                let mark = self.mark(Kind::Text(at, text));
                self.state.marks.push(mark);
            }
            window.focus(&self.focus, cx);
            cx.notify();
        }
    }

    fn start_text(&mut self, at: P, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Type here"));
        // (Once the click's done: the window takes the focus on a click,
        // after this.)
        let focus = input.clone();
        cx.defer_in(window, move |_, window, cx| focus.update(cx, |i, cx| i.focus(window, cx)));
        self.writing = Some((input, at));
        cx.notify();
    }

    fn mouse_down(&mut self, e: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let layout = self.layout(window);
        let q = layout.to_image(e.position);
        if self.writing.is_some() {
            return self.commit_text(window, cx);
        }
        window.focus(&self.focus, cx);
        let full = R { x: 0., y: 0., w: self.pixels.0, h: self.pixels.1 };
        let inside = |q: P| p(q.x.clamp(0., full.w), q.y.clamp(0., full.h));
        let q = inside(q);
        match self.tool {
            Tool::Select => {
                let slack = 6. / layout.k;
                self.selected = self.state.marks.iter().rposition(|m| m.hit(q, slack));
                if let Some(ix) = self.selected {
                    let original = self.state.marks[ix].clone();
                    self.color = original.color;
                    self.gesture = Some(Gesture::Moving { ix, from: q, original, moved: false });
                }
            }
            Tool::Arrow => self.gesture = Some(Gesture::Drawing(self.mark(Kind::Arrow(q, q)))),
            Tool::Line => self.gesture = Some(Gesture::Drawing(self.mark(Kind::Line(q, q)))),
            Tool::Rect => self.gesture = Some(Gesture::Drawing(self.mark(Kind::Rect(q, q)))),
            Tool::Ellipse => self.gesture = Some(Gesture::Drawing(self.mark(Kind::Ellipse(q, q)))),
            Tool::Highlight => self.gesture = Some(Gesture::Drawing(self.mark(Kind::Highlight(q, q)))),
            Tool::Blur => self.gesture = Some(Gesture::Drawing(self.mark(Kind::Blur(q, q)))),
            Tool::Pen => self.gesture = Some(Gesture::Drawing(self.mark(Kind::Pen(vec![q])))),
            Tool::Text => self.start_text(q, window, cx),
            Tool::Counter => {
                self.checkpoint();
                let mark = self.mark(Kind::Counter(q, self.next_step()));
                self.state.marks.push(mark);
            }
            Tool::Crop => {
                self.checkpoint();
                self.gesture = Some(Gesture::Cropping { from: q });
            }
        }
        cx.notify();
    }

    fn mouse_move(&mut self, e: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        if e.pressed_button != Some(MouseButton::Left) || self.gesture.is_none() {
            return;
        }
        let layout = self.layout(window);
        let q = layout.to_image(e.position);
        let q = p(q.x.clamp(0., self.pixels.0), q.y.clamp(0., self.pixels.1));
        let shift = e.modifiers.shift;
        match &mut self.gesture {
            Some(Gesture::Drawing(mark)) => match &mut mark.kind {
                Kind::Arrow(a, b) | Kind::Line(a, b) => *b = if shift { snap(*a, q) } else { q },
                Kind::Rect(a, b) | Kind::Ellipse(a, b) | Kind::Highlight(a, b) | Kind::Blur(a, b) => *b = if shift { square(*a, q) } else { q },
                Kind::Pen(points) => {
                    if points.last().is_none_or(|l| (l.x - q.x).hypot(l.y - q.y) * layout.k > 1.5) {
                        points.push(q);
                    }
                }
                _ => {}
            },
            Some(Gesture::Moving { ix, from, original, moved }) => {
                if !*moved {
                    *moved = true;
                    self.undo.push(self.state.clone());
                    self.redo.clear();
                }
                let by = p(q.x - from.x, q.y - from.y);
                self.state.marks[*ix] = original.moved(by);
            }
            Some(Gesture::Cropping { from }) => self.state.crop = Some(rect(*from, q)),
            None => {}
        }
        cx.notify();
    }

    fn mouse_up(&mut self, cx: &mut Context<Self>) {
        match self.gesture.take() {
            Some(Gesture::Drawing(mark)) if !mark.is_empty() => {
                self.checkpoint();
                self.state.marks.push(mark);
            }
            Some(Gesture::Cropping { .. }) => {
                // (A click, not a drag: no crop.)
                if self.state.crop.is_some_and(|c| c.w < 8. || c.h < 8.) {
                    self.state.crop = None;
                }
                if self.undo.last() == Some(&self.state) {
                    self.undo.pop();
                }
            }
            _ => {}
        }
        cx.notify();
    }

    fn key(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let k = &e.keystroke;
        let m = k.modifiers;
        // (Typing: ↩ keeps the text, escape drops it, and nothing else is
        // a shortcut, ↩ least of all.)
        if self.writing.is_some() {
            match k.key.as_str() {
                "enter" => self.commit_text(window, cx),
                "escape" => {
                    self.writing = None;
                    window.focus(&self.focus, cx);
                    cx.notify();
                }
                _ => return,
            }
            return cx.stop_propagation();
        }
        match (k.key.as_str(), m.platform, m.shift) {
            ("z", true, false) => self.undo(cx),
            ("z", true, true) => self.redo(cx),
            ("c", true, _) => self.export(Export::Copy, window, cx),
            ("s", true, _) => self.export(Export::Save, window, cx),
            ("enter", true, _) => self.export(Export::Done, window, cx),
            ("backspace" | "delete", false, _) => self.delete_selected(cx),
            ("escape", false, _) if self.selected.is_some() => {
                self.selected = None;
                cx.notify();
            }
            (key, false, false) if !m.control && !m.alt => {
                if let Some(tool) = Tool::for_key(key) {
                    self.set_tool(tool, window, cx);
                }
            }
            _ => return,
        }
        cx.stop_propagation();
    }

    fn say(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.notice = Some(text.into());
        self.notice_task = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(std::time::Duration::from_secs(2)).await;
            this.update(cx, |this, cx| {
                this.notice = None;
                cx.notify();
            })
            .ok();
        });
        cx.notify();
    }

    /// Draw it as edited, then copy it, save it, or both (copied) and close.
    fn export(&mut self, how: Export, window: &mut Window, cx: &mut Context<Self>) {
        self.commit_text(window, cx);
        let stem = self.path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
        let out = match how {
            Export::Save => {
                let dir = super::folder(cx);
                let _ = std::fs::create_dir_all(&dir);
                (1..).map(|n| dir.join(if n == 1 { format!("{stem} (edited).png") } else { format!("{stem} (edited {n}).png") })).find(|p| !p.exists()).unwrap_or_default()
            }
            _ => self.path.with_file_name(format!("{stem} (edited).png")),
        };
        let edited = !self.state.marks.is_empty() || self.state.crop.is_some() || self.state.background.is_some();
        let spec = marks::spec(&self.path.to_string_lossy(), &out.to_string_lossy(), self.scale, &self.state.marks, self.state.crop, self.state.background).to_string();
        let source = self.path.clone();
        let handle = window.window_handle();
        cx.spawn(async move |this, cx| {
            let made = cx
                .background_executor()
                .spawn({
                    let out = out.clone();
                    async move {
                        // (Nothing changed: the screenshot as it is.)
                        if edited { macos::render_shot(&spec) } else { std::fs::copy(&source, &out).map(|_| ()).map_err(|e| e.to_string()) }
                    }
                })
                .await;
            cx.update(|cx| {
                let copied = made.is_ok() && how != Export::Save && macos::copy_image(&out);
                let said = match (&made, how) {
                    (Err(e), _) => format!("Couldn't make it: {e}"),
                    (Ok(()), Export::Save) => format!("Saved to {}", out.parent().and_then(|d| d.file_name()).map_or("".into(), |n| n.to_string_lossy())),
                    (Ok(()), _) if copied => "Copied".into(),
                    (Ok(()), _) => "Couldn't copy it".into(),
                };
                if how == Export::Done && copied {
                    handle.update(cx, |_, window, _| window.remove_window()).ok();
                } else {
                    this.update(cx, |this, cx| this.say(said, cx)).ok();
                }
            });
        })
        .detach();
    }

    fn icon_button(&self, id: impl Into<ElementId>, name: &'static str, on: bool, tip: String, window: &Window) -> Stateful<Div> {
        let palette = self.palette;
        div()
            .id(id.into())
            .size(px(30.))
            .rounded(px(7.))
            .flex()
            .items_center()
            .justify_center()
            .when(on, |d| d.bg(palette.accent))
            .when(!on, |d| d.hover(|d| d.bg(palette.glass_selected)))
            .child(symbol(name, 14., MEDIUM, if on { palette.selected_text } else { palette.label }, window))
            .tooltip(move |window, cx| gpui_kit::component::tooltip::Tooltip::new(tip.clone()).build(window, cx))
    }

    fn render_toolbar(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette;
        let divider = || div().w(px(1.)).h(px(20.)).mx_1().bg(palette.separator);
        let tools = Tool::ALL.into_iter().map(|tool| {
            let (name, sym, key) = tool.about();
            self.icon_button(("tool", tool as usize), sym, self.tool == tool, format!("{name} ({})", key.to_uppercase()), window)
                .on_click(cx.listener(move |this, _, window, cx| this.set_tool(tool, window, cx)))
        });
        let colors = COLORS.into_iter().map(|color| {
            let on = self.color == color;
            div()
                .id(("color", color as usize))
                .size(px(22.))
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .when(on, |d| d.border_2().border_color(palette.accent))
                .child(div().size(px(16.)).rounded_full().bg(rgb(color)).border_1().border_color(black().opacity(0.2)))
                .on_click(cx.listener(move |this, _, _, cx| this.set_color(color, cx)))
        });
        let widths = WIDTHS.into_iter().enumerate().map(|(i, width)| {
            let on = (self.width - width).abs() < 0.1;
            div()
                .id(("width", i))
                .size(px(26.))
                .rounded(px(6.))
                .flex()
                .items_center()
                .justify_center()
                .when(on, |d| d.bg(palette.glass_selected))
                .child(div().size(px(4. + width)).rounded_full().bg(palette.label))
                .tooltip(move |window, cx| gpui_kit::component::tooltip::Tooltip::new(["Thin", "Medium", "Thick"][i]).build(window, cx))
                .on_click(cx.listener(move |this, _, _, cx| this.set_width(width, cx)))
        });
        let background_on = self.state.background.is_some();
        div()
            .h(px(TOOLBAR))
            .flex_shrink_0()
            .px_3()
            .flex()
            .items_center()
            .gap(px(2.))
            .border_b_1()
            .border_color(palette.separator)
            .bg(palette.background)
            .children(tools)
            .child(divider())
            .child(div().flex().gap(px(2.)).children(colors))
            .child(divider())
            .child(div().flex().gap(px(2.)).children(widths))
            .child(divider())
            .child(self.icon_button("undo", "arrow.uturn.backward", false, "Undo (⌘Z)".into(), window).when(self.undo.is_empty(), |d| d.opacity(0.4)).on_click(cx.listener(|this, _, _, cx| this.undo(cx))))
            .child(self.icon_button("redo", "arrow.uturn.forward", false, "Redo (⇧⌘Z)".into(), window).when(self.redo.is_empty(), |d| d.opacity(0.4)).on_click(cx.listener(|this, _, _, cx| this.redo(cx))))
            .child(div().flex_1())
            .child(self.icon_button("background", "photo.on.rectangle.angled", background_on, "Background".into(), window).on_click(cx.listener(|this, _, _, cx| {
                let next = if this.state.background.is_some() { None } else { Some(this.last_background) };
                this.set_background(next, cx)
            })))
            .child(self.icon_button("copy", "doc.on.doc", false, "Copy (⌘C)".into(), window).on_click(cx.listener(|this, _, window, cx| this.export(Export::Copy, window, cx))))
            .child(self.icon_button("save", "square.and.arrow.down", false, "Save (⌘S)".into(), window).on_click(cx.listener(|this, _, window, cx| this.export(Export::Save, window, cx))))
            .child(
                div()
                    .id("done")
                    .ml_1()
                    .px_3()
                    .h(px(28.))
                    .rounded(px(7.))
                    .bg(palette.accent)
                    .text_color(palette.selected_text)
                    .text_size(px(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .flex()
                    .items_center()
                    .child("Done")
                    .tooltip(|window, cx| gpui_kit::component::tooltip::Tooltip::new("Copy and close (⌘↩)").build(window, cx))
                    .on_click(cx.listener(|this, _, window, cx| this.export(Export::Done, window, cx))),
            )
    }

    fn render_options(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette;
        let b = self.state.background.unwrap_or_default();
        let label = |text: &'static str| div().ml_3().mr_1().text_size(px(12.)).text_color(palette.secondary).child(text);
        let pill = |id: (&'static str, usize), text: String, on: bool| {
            div().id(id).px_2().py(px(3.)).rounded(px(6.)).text_size(px(12.)).when(on, |d| d.bg(palette.glass_selected)).child(text)
        };
        let gradients = BACKGROUNDS.iter().enumerate().map(|(i, &(from, to))| {
            let on = b.gradient == i;
            div()
                .id(("gradient", i))
                .size(px(24.))
                .rounded(px(6.))
                .when(on, |d| d.border_2().border_color(palette.accent))
                .bg(linear_gradient(135., linear_color_stop(rgb(from), 0.), linear_color_stop(rgb(to), 1.)))
                .on_click(cx.listener(move |this, _, _, cx| this.set_background(Some(Background { gradient: i, ..b }), cx)))
        });
        div()
            .h(px(OPTIONS))
            .flex_shrink_0()
            .px_3()
            .flex()
            .items_center()
            .gap_1()
            .border_b_1()
            .border_color(palette.separator)
            .bg(palette.background)
            .children(gradients)
            .child(label("Padding"))
            .children(PADDINGS.iter().enumerate().map(|(i, &pad)| {
                pill(("padding", i), ["S", "M", "L"][i].into(), (b.padding - pad).abs() < 0.5)
                    .on_click(cx.listener(move |this, _, _, cx| this.set_background(Some(Background { padding: pad, ..b }), cx)))
            }))
            .child(label("Corners"))
            .children(RADII.iter().enumerate().map(|(i, &radius)| {
                pill(("radius", i), ["None", "Round", "Rounder"][i].into(), (b.radius - radius).abs() < 0.5)
                    .on_click(cx.listener(move |this, _, _, cx| this.set_background(Some(Background { radius, ..b }), cx)))
            }))
            .child(label("Shadow"))
            .child(pill(("shadow", 0), if b.shadow { "On".into() } else { "Off".into() }, b.shadow).on_click(cx.listener(move |this, _, _, cx| {
                this.set_background(Some(Background { shadow: !b.shadow, ..b }), cx)
            })))
    }

    /// The image, its marks, and what's being drawn: inside the shown
    /// part's frame (which clips them).
    fn render_canvas(&mut self, layout: Layout, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let frame = layout.frame();
        let k = layout.k;
        let (iw, ih) = (self.pixels.0 * k, self.pixels.1 * k);
        // Within the frame: image pixels to points from its top left.
        let local = move |q: P| point(px((q.x - layout.shown.x) * k), px((q.y - layout.shown.y) * k));
        let mut marks: Vec<Mark> = self.state.marks.clone();
        if let Some(Gesture::Drawing(mark)) = &self.gesture {
            marks.push(mark.clone());
        }
        let prims: Vec<Prim> = marks.iter().flat_map(Mark::prims).collect();
        let radius = self.state.background.map_or(0., |b| b.radius * self.scale * k);
        let shadow = self.state.background.is_some_and(|b| b.shadow);
        let mut frame_div = div()
            .absolute()
            .left(frame.origin.x)
            .top(frame.origin.y - px(self.top()))
            .w(frame.size.width)
            .h(frame.size.height)
            .overflow_hidden()
            .rounded(px(radius))
            .when(shadow, |d| d.shadow(vec![BoxShadow { color: black().opacity(0.38), offset: point(px(0.), px(8.)), blur_radius: px(24.), spread_radius: px(0.), inset: false }]))
            .child(
                img(self.path.clone())
                    .absolute()
                    .left(px(-layout.shown.x * k))
                    .top(px(-layout.shown.y * k))
                    .w(px(iw))
                    .h(px(ih))
                    // (GPUI clips square: the image rounds its own corners.)
                    .when(layout.shown.w == self.pixels.0 && layout.shown.h == self.pixels.1, |i| i.rounded(px(radius))),
            );
        // The blur's marks: the pixelated copy, through each one's area.
        for prim in &prims {
            if let (Prim::Pixelate { rect }, Some(squares)) = (prim, &self.squares) {
                let at = local(p(rect.x, rect.y));
                frame_div = frame_div.child(
                    div().absolute().left(at.x).top(at.y).w(px(rect.w * k)).h(px(rect.h * k)).overflow_hidden().child(
                        img(squares.clone()).absolute().left(px(-(rect.x) * k)).top(px(-(rect.y) * k)).w(px(iw)).h(px(ih)),
                    ),
                );
            }
        }
        let painted = prims.clone();
        frame_div = frame_div.child(
            canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    let at = |q: P| bounds.origin + local(q);
                    for prim in &painted {
                        match prim {
                            Prim::Stroke { points, width, color, alpha, closed } => {
                                let width = (width * k).max(1.);
                                let options = StrokeOptions::default().with_line_width(width).with_line_cap(LineCap::Round).with_line_join(LineJoin::Round);
                                let mut path = PathBuilder::stroke(px(width)).with_style(PathStyle::Stroke(options));
                                let mut points = points.iter();
                                if let Some(first) = points.next() {
                                    path.move_to(at(*first));
                                    let mut any = false;
                                    for q in points {
                                        path.line_to(at(*q));
                                        any = true;
                                    }
                                    // (A dot: a pen's tap.)
                                    if !any {
                                        path.line_to(at(*first) + point(px(0.01), px(0.)));
                                    }
                                    if *closed {
                                        path.close();
                                    }
                                }
                                if let Ok(path) = path.build() {
                                    window.paint_path(path, Hsla::from(rgb(*color)).opacity(*alpha));
                                }
                            }
                            Prim::Fill { points, color, alpha } => {
                                let mut path = PathBuilder::fill();
                                let points: Vec<Point<Pixels>> = points.iter().map(|q| at(*q)).collect();
                                path.add_polygon(&points, true);
                                if let Ok(path) = path.build() {
                                    window.paint_path(path, Hsla::from(rgb(*color)).opacity(*alpha));
                                }
                            }
                            Prim::Disc { center, r, color } => {
                                let r = px(r * k);
                                let c = at(*center);
                                window.paint_quad(fill(Bounds { origin: c - point(r, r), size: size(r * 2., r * 2.) }, rgb(*color)).corner_radii(r));
                            }
                            Prim::Text { .. } | Prim::Pixelate { .. } => {}
                        }
                    }
                },
            )
            .absolute()
            .inset_0(),
        );
        // Texts (and steps' numbers), as text.
        for prim in &prims {
            if let Prim::Text { at, size, color, text, center } = prim {
                let size = size * k;
                let q = local(*at);
                frame_div = frame_div.child(if *center {
                    div()
                        .absolute()
                        .left(q.x - px(size))
                        .top(q.y - px(size))
                        .size(px(size * 2.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(size))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(*color))
                        .child(text.clone())
                } else {
                    div()
                        .absolute()
                        .left(q.x)
                        .top(q.y)
                        .whitespace_nowrap()
                        .text_size(px(size))
                        .line_height(relative(1.2))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(*color))
                        .child(text.clone())
                });
            }
        }
        // The mark picked: a dashed box round it.
        if let Some(mark) = self.selected.and_then(|ix| self.state.marks.get(ix)) {
            let b = mark.bounds();
            let pad = mark.width / 2. + 4. / k;
            let at = local(p(b.x - pad, b.y - pad));
            frame_div = frame_div.child(
                div().absolute().left(at.x).top(at.y).w(px((b.w + 2. * pad) * k)).h(px((b.h + 2. * pad) * k)).border_1().border_dashed().border_color(self.palette.accent),
            );
        }
        // A text being typed.
        if let Some((input, at)) = &self.writing {
            let size = marks::text_size(self.width * self.scale) * k;
            let q = local(*at);
            frame_div = frame_div.child(
                div().absolute().left(q.x - px(2.)).top(q.y).min_w(px(160.)).child(
                    Input::new(input)
                        .appearance(false)
                        .text_size(px(size))
                        .line_height(px((size * 1.2).round()))
                        .h(px((size * 1.2).round() + 4.))
                        .px_0()
                        .py_0()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(self.color)),
                ),
            );
        }
        let _ = (window, cx);
        frame_div.into_any_element()
    }

    /// The toolbar's (and options') height.
    fn top(&self) -> f32 {
        TOOLBAR + if self.state.background.is_some() { OPTIONS } else { 0. }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Export {
    Copy,
    Save,
    /// Copied, and closed.
    Done,
}

/// `q` from `a`, snapped to the nearest 45°.
fn snap(a: P, q: P) -> P {
    let (dx, dy) = (q.x - a.x, q.y - a.y);
    let len = dx.hypot(dy);
    let angle = (dy.atan2(dx) / std::f32::consts::FRAC_PI_4).round() * std::f32::consts::FRAC_PI_4;
    p(a.x + len * angle.cos(), a.y + len * angle.sin())
}

/// `q` from `a`, as a square's (or circle's) corner.
fn square(a: P, q: P) -> P {
    let side = (q.x - a.x).abs().max((q.y - a.y).abs());
    p(a.x + side * (q.x - a.x).signum(), a.y + side * (q.y - a.y).signum())
}

/// Something different each time (a temporary file's name).
fn rand_ish() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos() as u64)
}

impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette;
        let layout = self.layout(window);
        let top = self.top();
        let dark = matches!(window.appearance(), WindowAppearance::Dark | WindowAppearance::VibrantDark);
        let backdrop = layout.backdrop.zip(self.state.background).map(|(b, bg)| {
            let (from, to) = BACKGROUNDS[bg.gradient.min(BACKGROUNDS.len() - 1)];
            div()
                .absolute()
                .left(b.origin.x)
                .top(b.origin.y - px(top))
                .w(b.size.width)
                .h(b.size.height)
                .bg(linear_gradient(135., linear_color_stop(rgb(from), 0.), linear_color_stop(rgb(to), 1.)))
        });
        // While cropping: the rest dimmed, the crop outlined.
        let crop_marks = (self.tool == Tool::Crop).then(|| self.state.crop).flatten().map(|c| {
            let frame = layout.frame();
            let a = layout.to_view(p(c.x, c.y));
            let b = layout.to_view(p(c.x + c.w, c.y + c.h));
            let dim = black().opacity(0.5);
            let y = |v: Pixels| v - px(top);
            vec![
                div().absolute().left(frame.origin.x).top(y(frame.origin.y)).w(frame.size.width).h(a.y - frame.origin.y).bg(dim),
                div().absolute().left(frame.origin.x).top(y(b.y)).w(frame.size.width).h(frame.origin.y + frame.size.height - b.y).bg(dim),
                div().absolute().left(frame.origin.x).top(y(a.y)).w(a.x - frame.origin.x).h(b.y - a.y).bg(dim),
                div().absolute().left(b.x).top(y(a.y)).w(frame.origin.x + frame.size.width - b.x).h(b.y - a.y).bg(dim),
                div().absolute().left(a.x).top(y(a.y)).w(b.x - a.x).h(b.y - a.y).border_2().border_color(white()),
            ]
        });
        let cursor = match self.tool {
            Tool::Select => CursorStyle::Arrow,
            Tool::Text => CursorStyle::IBeam,
            _ => CursorStyle::Crosshair,
        };
        let hint = match self.tool {
            Tool::Crop if self.state.crop.is_none() => Some("Drag out the part to keep."),
            Tool::Text if self.writing.is_none() => Some("Click where the text goes."),
            Tool::Select if self.state.marks.is_empty() => Some("Pick a tool above: arrows, shapes, text, steps, blur…"),
            _ => None,
        };
        let canvas = self.render_canvas(layout, window, cx);
        div()
            .key_context("Utility")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(if dark { rgb(0x1c1c1e) } else { rgb(0xe8e8ea) })
            .text_color(palette.label)
            .text_size(px(13.))
            // (Before the text being typed sees ↩.)
            .capture_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| this.key(e, window, cx)))
            .child(self.render_toolbar(window, cx))
            .when(self.state.background.is_some(), |d| d.child(self.render_options(cx)))
            .child(
                div()
                    .id("canvas")
                    .flex_1()
                    .relative()
                    .overflow_hidden()
                    .cursor(cursor)
                    .children(backdrop)
                    .child(canvas)
                    .children(crop_marks.into_iter().flatten())
                    .when_some(hint, |d, hint| {
                        d.child(div().absolute().bottom(px(10.)).left_0().right_0().flex().justify_center().child(
                            div().px_3().py_1().rounded_full().bg(black().opacity(0.6)).text_color(white()).text_size(px(12.)).child(hint),
                        ))
                    })
                    .when_some(self.notice.clone(), |d, notice| {
                        d.child(div().absolute().top(px(12.)).left_0().right_0().flex().justify_center().child(
                            div()
                                .px_3()
                                .py_1()
                                .rounded_full()
                                .bg(black().opacity(0.75))
                                .text_color(white())
                                .text_size(px(12.))
                                .font_weight(FontWeight::MEDIUM)
                                .child(notice),
                        ))
                    })
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, e: &MouseDownEvent, window, cx| this.mouse_down(e, window, cx)))
                    .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, window, cx| this.mouse_move(e, window, cx)))
                    .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, cx| this.mouse_up(cx)))
                    .on_mouse_up_out(MouseButton::Left, cx.listener(|this, _, _, cx| this.mouse_up(cx))),
            )
    }
}

