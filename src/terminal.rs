//! The terminal that comes down from the notch (`panel`): alacritty_terminal
//! is the terminal (its screen and scrollback, from what the shell under
//! it, on a pty, writes), and GPUI draws it, in the theme, font and cursor
//! Settings › Terminal has (`theme`; another terminal's, imported:
//! `import`), its box drawing as shapes (`boxes`). Keys go to the
//! shell as terminals send them (`keys`); text, and an input method's
//! (Korean), through GPUI's input. Dragging selects, ⌘C copies, ⌘V pastes;
//! the mouse goes to programs that ask for it, ⌘-click opens links, files
//! dropped are typed in, and there's a menu (`pointer`); ⌘F finds (`find`).
//! What the shell runs, and where, macOS says (`shell`).
//! The renderer only reads cells, so another terminal underneath
//! (libghostty-vt, once it's built ready) could take alacritty_terminal's
//! place.

mod boxes;
mod find;
pub mod import;
mod keys;
mod links;
mod mouse;
mod panel;
mod pointer;
mod shell;
pub mod theme;

pub use panel::{available, init, open_here, toggle};

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use alacritty_terminal::event::{Event, EventListener, Notify, OnResize, WindowSize};
use alacritty_terminal::event_loop::{EventLoop, Msg, Notifier};
use alacritty_terminal::grid::Scroll;
use alacritty_terminal::index::{Column, Line, Point as GridPoint, Side};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::tty;
use alacritty_terminal::vte::ansi::{Color, CursorShape, CursorStyle, NamedColor};
use futures::StreamExt;
use futures::channel::mpsc::{UnboundedSender, unbounded};
use gpui::prelude::FluentBuilder as _;
use gpui::*;

use crate::settings::{Settings, TerminalCursor};
use theme::Theme;

/// Room round the cells.
const PADDING: f32 = 8.;
/// A blinking cursor's on, then off, for this long each.
const BLINK: Duration = Duration::from_millis(530);

/// The terminal's events, from its thread to GPUI's.
#[derive(Clone)]
struct Listener(UnboundedSender<Event>);

impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        let _ = self.0.unbounded_send(event);
    }
}

/// What's drawn, measured as the frame's laid out.
struct Frame {
    backgrounds: Vec<(Bounds<Pixels>, Hsla)>,
    /// Box drawing and blocks, drawn as shapes.
    shapes: Vec<boxes::Shape>,
    lines: Vec<(Point<Pixels>, ShapedLine)>,
    cursor: Option<(Bounds<Pixels>, Hsla, bool)>,
    /// The character under a block cursor, in the cursor's text color.
    under: Option<(Point<Pixels>, ShapedLine)>,
    marked: Option<(Point<Pixels>, ShapedLine, Bounds<Pixels>)>,
    /// Under the link ⌘'s over.
    underlines: Vec<(Bounds<Pixels>, Hsla)>,
    line_height: Pixels,
}

/// A shell on a pty, and the terminal its output makes, on a thread of
/// their own.
pub struct Session {
    term: Arc<FairMutex<Term<Listener>>>,
    notifier: Notifier,
    events: futures::channel::mpsc::UnboundedReceiver<Event>,
    shell: Option<shell::Shell>,
}

impl Session {
    /// A shell (yours, logged in, as Terminal starts it) in `dir`, else
    /// home.
    pub fn start(dir: Option<PathBuf>) -> Result<Session, String> {
        let (tx, events) = unbounded();
        let listener = Listener(tx);
        let (cols, rows) = (80, 24);
        let config = Config { scrolling_history: 10_000, ..Config::default() };
        let term = Arc::new(FairMutex::new(Term::new(config, &TermSize::new(cols, rows), listener.clone())));
        let mut env = std::collections::HashMap::new();
        env.insert("TERM".to_string(), "xterm-256color".to_string());
        env.insert("COLORTERM".to_string(), "truecolor".to_string());
        env.insert("TERM_PROGRAM".to_string(), crate::APP_NAME.to_string());
        // (Started from the Dock, F2 has no locale: Hangul needs one.)
        if std::env::var_os("LANG").is_none() {
            env.insert("LANG".to_string(), "en_US.UTF-8".to_string());
        }
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let options = tty::Options { shell: None, working_directory: dir.or(home), drain_on_exit: false, env };
        let size = WindowSize { num_lines: rows as u16, num_cols: cols as u16, cell_width: 8, cell_height: 16 };
        let pty = tty::new(&options, size, 0).map_err(|e| format!("The shell didn't start: {e}"))?;
        let shell = shell::Shell::new(pty.child().id(), pty.file());
        let event_loop = EventLoop::new(term.clone(), listener, pty, false, false).map_err(|e| format!("The terminal didn't start: {e}"))?;
        let notifier = Notifier(event_loop.channel());
        event_loop.spawn();
        Ok(Session { term, notifier, events, shell })
    }
}

pub struct TerminalView {
    term: Arc<FairMutex<Term<Listener>>>,
    notifier: Notifier,
    focus: FocusHandle,
    theme: Theme,
    font: Font,
    /// Columns and rows, as the terminal has them now.
    size: (usize, usize),
    /// A cell's size, and where the cursor is (for an input method's
    /// window).
    cell: Size<Pixels>,
    cursor: Bounds<Pixels>,
    /// What an input method's making, not typed yet.
    marked: Option<String>,
    /// Scrolling not yet a whole line.
    scrolled: f32,
    /// Where the cells start, as last drawn (for where a click is).
    origin: Point<Pixels>,
    /// The mouse is down, selecting.
    selecting: bool,
    /// A blinking cursor's off now; and each time it's kept on (typed at),
    /// its blinking from then.
    blinked: bool,
    blinks: usize,
    shell: Option<shell::Shell>,
    /// A button down that the program was told of (vim's mouse), and the
    /// cell it was last told the mouse is in.
    reported: Option<mouse::Button>,
    told_at: Option<(usize, usize)>,
    /// Where the mouse is (for ⌘ pressed over a link, without moving), and
    /// the link under it while ⌘ is held.
    pointer: Option<Point<Pixels>>,
    link: Option<pointer::Link>,
    find: Option<find::Find>,
    /// ⌘+ and ⌘−: points more (or less) than Settings' size.
    zoom: f32,
    /// What a program named it (OSC 0 and 2: Claude Code's ✳), for its tab.
    title: Option<String>,
}

/// What the terminal asks of the panel round it.
pub enum Asked {
    /// Its window in front (files were dropped on it).
    Focus,
    NewTab,
    CloseTab,
    /// The next tab (or, false, the one before).
    NextTab(bool),
    /// The tab numbered so (⌘1 the first; ⌘9 the last).
    Tab(usize),
}

impl EventEmitter<Asked> for TerminalView {}

impl TerminalView {
    /// The view of `session`'s terminal.
    pub fn new(session: Session, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let Session { term, notifier, events: mut rx, shell } = session;
        cx.spawn(async move |this, cx| {
            while let Some(event) = rx.next().await {
                // (Many at once: one redraw.)
                let mut events = vec![event];
                while let Ok(event) = rx.try_recv() {
                    events.push(event);
                }
                let alive = this.update(cx, |this, cx| {
                    for event in events {
                        this.event(event, cx);
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
        let theme = Theme::of(&Settings::get(cx).quick_terminal);
        let font = terminal_font(theme.font_family.as_deref());
        cursor_style(&mut term.lock(), &theme);
        // (Settings changed: restyled as they are.)
        cx.observe_global::<Settings>(|this, cx| {
            let theme = Theme::of(&Settings::get(cx).quick_terminal);
            if theme.font_family != this.theme.font_family {
                this.font = terminal_font(theme.font_family.as_deref());
            }
            cursor_style(&mut this.term.lock(), &theme);
            this.theme = theme;
            this.blink(cx);
            cx.notify();
        })
        .detach();
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let mut view = TerminalView {
            term,
            notifier,
            focus,
            font,
            theme,
            size: (80, 24),
            cell: size(px(8.), px(16.)),
            cursor: Bounds::default(),
            marked: None,
            scrolled: 0.,
            origin: Point::default(),
            selecting: false,
            blinked: false,
            blinks: 0,
            shell,
            reported: None,
            told_at: None,
            pointer: None,
            link: None,
            find: None,
            zoom: 0.,
            title: None,
        };
        view.blink(cx);
        view
    }

    /// The cursor blinking (if Settings, or the program, say), from on;
    /// again when either says.
    fn blink(&mut self, cx: &mut Context<Self>) {
        self.blinked = false;
        self.blinks += 1;
        let blinks = self.blinks;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(BLINK).await;
                let going = this.update(cx, |this, cx| {
                    if this.blinks != blinks {
                        return false;
                    }
                    // (Not blinking now: on, till it is again.)
                    let blinking = this.term.lock().cursor_style().blinking;
                    if blinking || this.blinked {
                        this.blinked = blinking && !this.blinked;
                        cx.notify();
                    }
                    blinking
                });
                if !matches!(going, Ok(true)) {
                    break;
                }
            }
        })
        .detach();
    }

    fn event(&mut self, event: Event, cx: &mut Context<Self>) {
        match event {
            Event::PtyWrite(text) => self.notifier.notify(text.into_bytes()),
            Event::ClipboardStore(_, text) => cx.write_to_clipboard(ClipboardItem::new_string(text)),
            Event::ClipboardLoad(_, reply) => {
                let text = cx.read_from_clipboard().and_then(|c| c.text()).unwrap_or_default();
                self.notifier.notify(reply(&text).into_bytes());
            }
            Event::ColorRequest(index, reply) => {
                let color = self.named(index, &self.term.lock().colors().clone());
                let rgb = alacritty_terminal::vte::ansi::Rgb { r: (color >> 16) as u8, g: (color >> 8) as u8, b: color as u8 };
                self.notifier.notify(reply(rgb).into_bytes());
            }
            Event::TextAreaSizeRequest(reply) => {
                let (cols, rows) = self.size;
                let size = WindowSize { num_lines: rows as u16, num_cols: cols as u16, cell_width: f32::from(self.cell.width) as u16, cell_height: f32::from(self.cell.height) as u16 };
                self.notifier.notify(reply(size).into_bytes());
            }
            Event::ChildExit(_) | Event::Exit => cx.emit(Ended),
            Event::CursorBlinkingChange => self.blink(cx),
            Event::Title(title) => self.title = Some(title),
            Event::ResetTitle => self.title = None,
            Event::Wakeup => self.find_stale(cx),
            _ => {}
        }
        cx.notify();
    }

    /// Its tab's name: what it's running (as the program names itself, else
    /// its process's name), else its folder's name.
    pub fn name(&self) -> String {
        let Some(shell) = &self.shell else { return "Shell".into() };
        if shell.busy() {
            let title = self.title.clone().filter(|t| !t.trim().is_empty());
            if let Some(name) = title.or_else(|| shell.running()) {
                return name;
            }
        }
        let home = std::env::var_os("HOME").map(PathBuf::from);
        match shell.shell_folder() {
            Some(folder) if Some(&folder) == home.as_ref() => "~".into(),
            Some(folder) => folder.file_name().map_or_else(|| "/".into(), |n| n.to_string_lossy().into_owned()),
            None => "Shell".into(),
        }
    }

    /// The shell's folder (a new tab starts there).
    pub fn folder(&self) -> Option<PathBuf> {
        self.shell.as_ref()?.shell_folder()
    }

    /// Whether it's running something (not at its prompt).
    pub fn busy(&self) -> bool {
        self.shell.as_ref().is_some_and(|s| s.busy())
    }

    /// Bytes to the shell, scrolled back down to where it's typing, what
    /// was selected let go (the cursor on while you type).
    fn send(&mut self, bytes: Vec<u8>, cx: &mut Context<Self>) {
        let mut term = self.term.lock();
        term.scroll_display(Scroll::Bottom);
        term.selection = None;
        drop(term);
        self.notifier.notify(bytes);
        if self.term.lock().cursor_style().blinking {
            self.blink(cx);
        }
    }

    /// `cd` to `dir`, typed (if no full-screen program has the terminal:
    /// vim, less).
    pub fn cd(&mut self, dir: &std::path::Path, cx: &mut Context<Self>) {
        if self.term.lock().mode().contains(TermMode::ALT_SCREEN) {
            return;
        }
        let quoted = dir.to_string_lossy().replace('\'', "'\\''");
        self.send(format!("cd -- '{quoted}'\r").into_bytes(), cx);
    }

    /// The cell at `position` (in the window), and which half of it.
    fn cell_at(&self, position: Point<Pixels>) -> (GridPoint, Side) {
        let (cols, rows) = self.size;
        let x = f32::from(position.x - self.origin.x).max(0.);
        let y = f32::from(position.y - self.origin.y).max(0.);
        let width = f32::from(self.cell.width).max(1.);
        let col = ((x / width) as usize).min(cols.saturating_sub(1));
        let row = ((y / f32::from(self.cell.height).max(1.)) as usize).min(rows.saturating_sub(1));
        let side = if x % width > width / 2. { Side::Right } else { Side::Left };
        let offset = self.term.lock().grid().display_offset() as i32;
        (GridPoint::new(Line(row as i32 - offset), Column(col)), side)
    }

    /// What's selected, copied.
    fn copy(&mut self, cx: &mut Context<Self>) {
        if let Some(text) = self.term.lock().selection_to_string().filter(|t| !t.is_empty()) {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    fn paste(&mut self, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) else { return };
        self.paste_text(text, cx);
    }

    fn paste_text(&mut self, text: String, cx: &mut Context<Self>) {
        let text = text.replace("\r\n", "\r").replace('\n', "\r");
        // (Bracketed, if the program asked: a shell won't run it line by
        // line.)
        let bytes = if self.term.lock().mode().contains(TermMode::BRACKETED_PASTE) { format!("\x1b[200~{}\x1b[201~", text.replace('\x1b', "")) } else { text };
        self.send(bytes.into_bytes(), cx);
    }

    fn key_down(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let k = &e.keystroke;
        let m = &k.modifiers;
        // ⌘'s: the terminal's own (and its tabs', asked of the panel).
        if m.platform && !m.control && !m.alt {
            let key = k.key.as_str();
            match (key, m.shift) {
                ("v", false) => self.paste(cx),
                ("c", false) => self.copy(cx),
                ("a", false) => self.select_all(cx),
                ("f", false) => self.open_find(window, cx),
                ("g", shift) if self.find.is_some() => self.step_find(!shift, cx),
                ("k", false) => self.clear(cx),
                ("o", false) => self.open_folder(cx),
                ("=" | "+", _) => self.zoom(Some(1.), cx),
                ("-", false) => self.zoom(Some(-1.), cx),
                ("0", false) => self.zoom(None, cx),
                ("t", false) => cx.emit(Asked::NewTab),
                ("w", false) => cx.emit(Asked::CloseTab),
                ("]" | "}", true) => cx.emit(Asked::NextTab(true)),
                ("[" | "{", true) => cx.emit(Asked::NextTab(false)),
                (digit, false) if digit.len() == 1 && ("1"..="9").contains(&digit) => cx.emit(Asked::Tab(digit.parse::<usize>().unwrap_or(1) - 1)),
                _ => return self.type_key(e, cx),
            }
            cx.stop_propagation();
            return;
        }
        self.type_key(e, cx);
    }

    /// A key typed to the shell, as terminals send it.
    fn type_key(&mut self, e: &KeyDownEvent, cx: &mut Context<Self>) {
        let k = &e.keystroke;
        let m = &k.modifiers;
        // (⌃⇥ and ⌃⇧⇥: tabs, as in other apps.)
        if k.key == "tab" && m.control && !m.platform && !m.alt {
            cx.emit(Asked::NextTab(!m.shift));
            cx.stop_propagation();
            return;
        }
        // (⇧↩ a newline, not a return: a new line in Claude Code's prompt,
        // as Ghostty's keybind for it has it; a shell takes it as ↩.)
        if k.key == "enter" && m.shift && !m.control && !m.alt && !m.platform {
            self.send(b"\n".to_vec(), cx);
            cx.stop_propagation();
            return;
        }
        let app = self.term.lock().mode().contains(TermMode::APP_CURSOR);
        if let Some(bytes) = keys::bytes(&k.key, m, app) {
            self.send(bytes, cx);
            cx.stop_propagation();
        }
    }

    /// A color (of the 256, or the theme's named ones), as the program set
    /// it, else as the theme has it.
    fn named(&self, index: usize, colors: &alacritty_terminal::term::color::Colors) -> u32 {
        if let Some(rgb) = colors[index] {
            return (rgb.r as u32) << 16 | (rgb.g as u32) << 8 | rgb.b as u32;
        }
        let t = &self.theme;
        match index {
            0..=255 => t.palette[index],
            i if i == NamedColor::Foreground as usize || i == NamedColor::BrightForeground as usize => t.foreground,
            i if i == NamedColor::Background as usize => t.background,
            i if i == NamedColor::Cursor as usize => t.cursor.unwrap_or(t.foreground),
            i if i == NamedColor::DimForeground as usize => dim(t.foreground),
            // (Dim black to dim white.)
            i => dim(t.palette[(i - NamedColor::DimBlack as usize).min(7)]),
        }
    }

    fn color(&self, color: Color, colors: &alacritty_terminal::term::color::Colors) -> u32 {
        match color {
            Color::Spec(rgb) => (rgb.r as u32) << 16 | (rgb.g as u32) << 8 | rgb.b as u32,
            Color::Named(named) => self.named(named as usize, colors),
            Color::Indexed(i) => self.named(i as usize, colors),
        }
    }

    /// The terminal sized to `bounds`, and what's on it laid out.
    fn frame(&mut self, bounds: Bounds<Pixels>, focused: bool, window: &mut Window) -> Frame {
        let text_system = window.text_system().clone();
        let font_size = px((self.theme.font_size + self.zoom).clamp(6., 48.));
        let id = text_system.resolve_font(&self.font);
        let cell_width = text_system.em_advance(id, font_size).unwrap_or(px(8.));
        let line_height = (font_size * 1.25).round();
        self.cell = size(cell_width, line_height);
        self.origin = bounds.origin;
        let cols = ((f32::from(bounds.size.width) / f32::from(cell_width)) as usize).max(2);
        let rows = ((f32::from(bounds.size.height) / f32::from(line_height)) as usize).max(1);
        if (cols, rows) != self.size {
            self.size = (cols, rows);
            self.term.lock().resize(TermSize::new(cols, rows));
            self.notifier.on_resize(WindowSize { num_lines: rows as u16, num_cols: cols as u16, cell_width: f32::from(cell_width) as u16, cell_height: f32::from(line_height) as u16 });
        }
        let term = self.term.lock();
        let content = term.renderable_content();
        let colors = *content.colors;
        let background = self.theme.background;
        let selection = content.selection;
        let (selected_fg, selected_bg) = (self.theme.selection_foreground, self.theme.selection_background.unwrap_or(dim(self.theme.foreground)));
        let at = |col: usize, row: usize| point(bounds.origin.x + cell_width * col as f32, bounds.origin.y + line_height * row as f32);
        let mut frame = Frame { backgrounds: Vec::new(), shapes: Vec::new(), lines: Vec::new(), cursor: None, under: None, marked: None, underlines: Vec::new(), line_height };
        // (What ⌘F found on screen: the one shown orange, the rest yellow.)
        let screen_top = GridPoint::new(Line(-(content.display_offset as i32)), Column(0));
        let screen_bottom = GridPoint::new(Line(rows as i32 - 1 - content.display_offset as i32), Column(cols.saturating_sub(1)));
        let found = self.found_between(screen_top, screen_bottom);
        // (A run of cells drawn as one: its text, its styles, where it
        // starts, how wide its cells are.)
        let mut run = String::new();
        let mut runs: Vec<TextRun> = Vec::new();
        let mut start = (0usize, 0usize);
        let mut wide = false;
        let flush = |run: &mut String, runs: &mut Vec<TextRun>, start: (usize, usize), wide: bool, frame: &mut Frame| {
            if !run.is_empty() {
                let text = SharedString::from(std::mem::take(run));
                if wide {
                    // (Its two cells filled, as Ghostty sizes a fallback
                    // font's Hangul or kanji: a little bigger, at most a
                    // third; in the middle of them.)
                    let mut line = text_system.shape_line(text.clone(), font_size, runs, None);
                    let fill = f32::from(cell_width * 2.) / f32::from(line.width).max(1.);
                    if fill > 1.05 {
                        line = text_system.shape_line(text, font_size * fill.min(1.3), runs, None);
                    }
                    let origin = at(start.0, start.1);
                    let room = ((cell_width * 2. - line.width) / 2.).max(px(0.));
                    frame.lines.push((point(origin.x + room, origin.y), line));
                } else {
                    let line = text_system.shape_line(text, font_size, runs, Some(cell_width));
                    frame.lines.push((at(start.0, start.1), line));
                }
            }
            runs.clear();
        };
        let mut next = (usize::MAX, usize::MAX);
        let offset = content.display_offset as i32;
        let mut under = None;
        let cursor_point = content.cursor.point;
        for indexed in content.display_iter {
            let row = (indexed.point.line.0 + offset) as usize;
            let col = indexed.point.column.0;
            let cell = &indexed.cell;
            if cell.flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER) {
                continue;
            }
            let (mut fg, mut bg) = (self.color(cell.fg, &colors), self.color(cell.bg, &colors));
            if cell.flags.contains(Flags::INVERSE) {
                std::mem::swap(&mut fg, &mut bg);
            }
            if cell.flags.contains(Flags::DIM) {
                fg = dim(fg);
            }
            if cell.flags.contains(Flags::HIDDEN) {
                fg = bg;
            }
            if selection.is_some_and(|s| s.contains(indexed.point)) {
                bg = selected_bg;
                fg = selected_fg.unwrap_or(fg);
            }
            if let Some(shown) = self.found_at(indexed.point, &found) {
                (bg, fg) = if shown { (0xff9f0a, 0x000000) } else { (0xffd60a, 0x000000) };
            }
            if self.link.as_ref().is_some_and(|l| l.contains(indexed.point)) {
                let width = if cell.flags.contains(Flags::WIDE_CHAR) { cell_width * 2. } else { cell_width };
                let bottom = at(col, row) + point(px(0.), line_height - px(2.));
                frame.underlines.push((Bounds::new(bottom, size(width, px(1.))), rgb(fg).into()));
            }
            let is_wide = cell.flags.contains(Flags::WIDE_CHAR);
            if bg != background {
                let width = if is_wide { cell_width * 2. } else { cell_width };
                frame.backgrounds.push((Bounds::new(at(col, row), size(width, line_height)), rgb(bg).into()));
            }
            let mut font = self.font.clone();
            if cell.flags.contains(Flags::BOLD) {
                font.weight = FontWeight::BOLD;
            }
            if cell.flags.contains(Flags::ITALIC) {
                font.style = FontStyle::Italic;
            }
            let color: Hsla = rgb(fg).into();
            let style = TextRun {
                len: 0,
                font,
                color,
                background_color: None,
                underline: cell.flags.intersects(Flags::ALL_UNDERLINES).then(|| UnderlineStyle { thickness: px(1.), color: Some(color), wavy: cell.flags.contains(Flags::UNDERCURL) }),
                strikethrough: cell.flags.contains(Flags::STRIKEOUT).then(|| StrikethroughStyle { thickness: px(1.), color: Some(color) }),
            };
            // (Box drawing and blocks: shapes, not text.)
            if let Some(shapes) = boxes::draw(cell.c, Bounds::new(at(col, row), size(cell_width, line_height)), color) {
                frame.shapes.extend(shapes);
                continue;
            }
            let mut ch = String::from(cell.c);
            ch.extend(cell.zerowidth().into_iter().flatten());
            if indexed.point == cursor_point {
                under = Some((col, row, ch.clone(), style.clone()));
            }
            // (Each wide character on its own, two cells wide; the rest in
            // runs, as long as they're next to each other.)
            let joins = !is_wide && !wide && (col, row) == next;
            if !joins {
                flush(&mut run, &mut runs, start, wide, &mut frame);
                start = (col, row);
                wide = is_wide;
            }
            next = (col + 1, row);
            match runs.last_mut() {
                Some(last) if last.font == style.font && last.color == style.color && last.underline == style.underline && last.strikethrough == style.strikethrough => last.len += ch.len(),
                _ => runs.push(TextRun { len: ch.len(), ..style }),
            }
            run.push_str(&ch);
        }
        flush(&mut run, &mut runs, start, wide, &mut frame);
        // The cursor (and what an input method's making, at it).
        let row = (cursor_point.line.0 + offset) as usize;
        let origin = at(cursor_point.column.0, row);
        self.cursor = Bounds::new(origin, size(cell_width, line_height));
        let cursor_color: Hsla = rgb(self.theme.cursor.unwrap_or(self.theme.foreground)).into();
        if let Some(marked) = self.marked.clone().filter(|m| !m.is_empty()) {
            let run = TextRun {
                len: marked.len(),
                font: self.font.clone(),
                color: rgb(self.theme.foreground).into(),
                background_color: Some(rgb(background).into()),
                underline: Some(UnderlineStyle { thickness: px(1.), color: None, wavy: false }),
                strikethrough: None,
            };
            let line = text_system.shape_line(marked.into(), font_size, &[run], None);
            let width = line.width;
            frame.marked = Some((origin, line, Bounds::new(origin, size(width, line_height))));
        } else if content.cursor.shape != CursorShape::Hidden && content.display_offset == 0 && !(self.blinked && focused) {
            let block = focused && content.cursor.shape == CursorShape::Block;
            let shape = match content.cursor.shape {
                CursorShape::Beam => size(px(2.), line_height),
                CursorShape::Underline => size(cell_width, px(2.)),
                _ => size(cell_width, line_height),
            };
            let at_bottom = content.cursor.shape == CursorShape::Underline;
            let place = if at_bottom { point(origin.x, origin.y + line_height - px(2.)) } else { origin };
            frame.cursor = Some((Bounds::new(place, shape), cursor_color, block || content.cursor.shape != CursorShape::Block));
            if block && let Some((_, _, ch, style)) = under.filter(|u| u.0 == cursor_point.column.0 && u.1 == row) {
                let color = rgb(self.theme.cursor_text.unwrap_or(background)).into();
                let line = text_system.shape_line(ch.clone().into(), font_size, &[TextRun { len: ch.len(), color, ..style }], Some(cell_width));
                frame.under = Some((origin, line));
            }
        }
        frame
    }
}

use theme::dim;

/// The shell ended (`exit`, or it quit).
pub struct Ended;

impl EventEmitter<Ended> for TerminalView {}

impl TerminalView {
    /// The theme's background, as the panel round it paints it (as see-through
    /// as Settings have it).
    pub fn background(&self) -> Hsla {
        Hsla::from(rgb(self.theme.background)).opacity(self.theme.opacity)
    }
}

/// The font to draw in: Settings' (`font_family`), then a Nerd Font's
/// symbols for what it hasn't got (the icons in starship's and
/// powerlevel10k's prompts), from one that's installed, as Ghostty has
/// them built in.
fn terminal_font(wanted: Option<&str>) -> Font {
    static SYMBOLS: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    let symbols = SYMBOLS.get_or_init(|| {
        ["Symbols Nerd Font Mono", "Symbols Nerd Font", "JetBrainsMono Nerd Font Mono", "JetBrainsMono Nerd Font", "FiraCode Nerd Font Mono", "Hack Nerd Font Mono", "MesloLGS NF", "MesloLGS Nerd Font Mono"]
            .into_iter()
            .find(|f| crate::macos::has_font_family(f))
            .map(str::to_string)
    });
    let mut font = font(font_family(wanted));
    font.fallbacks = symbols.clone().map(|f| FontFallbacks::from_fonts(vec![f]));
    font
}

/// The font Settings name (`wanted`), if it's here; else Ghostty's own
/// (JetBrains Mono), else macOS's.
pub fn font_family(wanted: Option<&str>) -> String {
    wanted.into_iter().chain(["JetBrains Mono"]).find(|f| crate::macos::has_font_family(f)).unwrap_or("Menlo").to_string()
}

/// The cursor Settings have (a program can set its own).
fn cursor_style(term: &mut Term<Listener>, theme: &Theme) {
    let shape = match theme.cursor_shape {
        TerminalCursor::Block => CursorShape::Block,
        TerminalCursor::Bar => CursorShape::Beam,
        TerminalCursor::Underline => CursorShape::Underline,
    };
    let config = Config { scrolling_history: 10_000, default_cursor_style: CursorStyle { shape, blinking: theme.blink }, ..Config::default() };
    term.set_options(config);
}

impl Drop for TerminalView {
    fn drop(&mut self) {
        // (The shell goes with it.)
        let _ = self.notifier.0.send(Msg::Shutdown);
    }
}

impl Focusable for TerminalView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for TerminalView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        let focus = self.focus.clone();
        let focused = focus.is_focused(window);
        let background: Hsla = rgb(self.theme.background).into();
        let find_bar = self.find_bar(window, cx);
        let dropping: Hsla = rgb(self.theme.foreground).into();
        // (Its background the panel's, round it: rounded as it is. The find
        // bar beside the cells, not in them: its keys aren't the shell's.)
        let cells = div()
            .size_full()
            .track_focus(&self.focus)
            .key_context("Terminal")
            .on_key_down(cx.listener(Self::key_down))
            .on_scroll_wheel(cx.listener(Self::scroll))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::mouse_down))
            .on_mouse_down(MouseButton::Middle, cx.listener(Self::mouse_down))
            .on_mouse_move(cx.listener(Self::mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::mouse_up))
            .on_mouse_up(MouseButton::Right, cx.listener(Self::mouse_up))
            .on_mouse_up(MouseButton::Middle, cx.listener(Self::mouse_up))
            .on_modifiers_changed(cx.listener(Self::modifiers_changed))
            .drag_over::<ExternalPaths>(move |style, _, _, _| style.bg(dropping.opacity(0.06)))
            .on_drop(cx.listener(|this, dropped: &ExternalPaths, window, cx| this.drop_paths(dropped.paths(), window, cx)))
            .when(self.link.is_some(), |d| d.cursor_pointer())
            .p(px(PADDING))
            .child(
                canvas(
                    {
                        let view = view.clone();
                        move |bounds, window, cx| view.update(cx, |this, _| this.frame(bounds, focused, window))
                    },
                    move |bounds, frame, window, cx| {
                        for (bounds, color) in frame.backgrounds {
                            window.paint_quad(fill(bounds, color));
                        }
                        for shape in frame.shapes {
                            match shape {
                                boxes::Shape::Rect(bounds, color) => window.paint_quad(fill(bounds, color)),
                                boxes::Shape::Path(path, color) => window.paint_path(path, color),
                            }
                        }
                        if let Some((bounds, color, solid)) = frame.cursor {
                            if solid {
                                window.paint_quad(fill(bounds, color));
                            } else {
                                window.paint_quad(outline(bounds, color, BorderStyle::Solid));
                            }
                        }
                        for (origin, line) in frame.lines {
                            let _ = line.paint(origin, frame.line_height, TextAlign::Left, None, window, cx);
                        }
                        if let Some((origin, line)) = frame.under {
                            let _ = line.paint(origin, frame.line_height, TextAlign::Left, None, window, cx);
                        }
                        for (bounds, color) in frame.underlines {
                            window.paint_quad(fill(bounds, color));
                        }
                        if let Some((origin, line, bounds)) = frame.marked {
                            window.paint_quad(fill(bounds, background));
                            let _ = line.paint(origin, frame.line_height, TextAlign::Left, None, window, cx);
                        }
                        window.handle_input(&focus, ElementInputHandler::new(bounds, view), cx);
                    },
                )
                .size_full(),
            );
        div().size_full().relative().child(cells).children(find_bar)
    }
}

impl EntityInputHandler for TerminalView {
    fn text_for_range(&mut self, _: std::ops::Range<usize>, _: &mut Option<std::ops::Range<usize>>, _: &mut Window, _: &mut Context<Self>) -> Option<String> {
        None
    }

    fn selected_text_range(&mut self, _: bool, _: &mut Window, _: &mut Context<Self>) -> Option<UTF16Selection> {
        Some(UTF16Selection { range: 0..0, reversed: false })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<std::ops::Range<usize>> {
        self.marked.as_ref().map(|m| 0..m.encode_utf16().count())
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.marked = None;
        cx.notify();
    }

    fn replace_text_in_range(&mut self, _: Option<std::ops::Range<usize>>, text: &str, _: &mut Window, cx: &mut Context<Self>) {
        self.marked = None;
        if !text.is_empty() {
            self.send(text.as_bytes().to_vec(), cx);
        }
        cx.notify();
    }

    fn replace_and_mark_text_in_range(&mut self, _: Option<std::ops::Range<usize>>, text: &str, _: Option<std::ops::Range<usize>>, _: &mut Window, cx: &mut Context<Self>) {
        self.marked = (!text.is_empty()).then(|| text.to_string());
        cx.notify();
    }

    fn bounds_for_range(&mut self, _: std::ops::Range<usize>, _: Bounds<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<Bounds<Pixels>> {
        Some(self.cursor)
    }

    fn character_index_for_point(&mut self, _: Point<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        None
    }
}
