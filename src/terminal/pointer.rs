//! The mouse in the terminal: told to a program that asked for it (`mouse`;
//! ⇧ selects anyway, as in other terminals), else dragging selects. ⌘
//! over a link underlines it and ⌘-click opens it (`links`); a right-click
//! has a menu. Files dropped on it are typed in, as their paths. And what
//! the menu does: ⌘K's clearing, ⌘+ and ⌘−'s size, "Open in F2".

use std::path::{Path, PathBuf};

use alacritty_terminal::event::Notify;
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Line, Point as GridPoint, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::TermMode;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::vte::ansi::{ClearMode, Handler};
use gpui::*;

use super::links::{self, Target};
use super::mouse::{self, Button};
use super::{Asked, TerminalView, keys};
use crate::macos;

/// A link under the mouse, while ⌘ is held: its first and last cells.
#[derive(Clone)]
pub struct Link {
    pub start: GridPoint,
    pub end: GridPoint,
    pub target: Target,
}

impl Link {
    pub fn contains(&self, point: GridPoint) -> bool {
        self.start <= point && point <= self.end
    }
}

impl TerminalView {
    /// The cell at `position` on the screen (columns and rows from 0, as a
    /// program counts them).
    fn screen_cell(&self, position: Point<Pixels>) -> (usize, usize) {
        let (cols, rows) = self.size;
        let x = f32::from(position.x - self.origin.x).max(0.);
        let y = f32::from(position.y - self.origin.y).max(0.);
        let col = ((x / f32::from(self.cell.width).max(1.)) as usize).min(cols.saturating_sub(1));
        let row = ((y / f32::from(self.cell.height).max(1.)) as usize).min(rows.saturating_sub(1));
        (col, row)
    }

    /// Whether the program has the mouse (it asked, and ⇧ isn't held).
    fn program_has_mouse(&self, m: &Modifiers) -> Option<TermMode> {
        let mode = *self.term.lock().mode();
        (mouse::wanted(mode) && !m.shift).then_some(mode)
    }

    fn tell(&mut self, button: Button, pressed: bool, moved: bool, m: &Modifiers, position: Point<Pixels>, mode: TermMode) {
        let (col, row) = self.screen_cell(position);
        if moved && self.told_at == Some((col, row)) {
            return;
        }
        self.told_at = Some((col, row));
        if let Some(bytes) = mouse::report(button, pressed, moved, m, col, row, mode) {
            self.notifier.notify(bytes);
        }
    }

    /// A press: a link opened (⌘), told to a program, a selection begun
    /// (a word, pressed twice; a line, three times), or the menu.
    pub(super) fn mouse_down(&mut self, e: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.focus.focus(window, cx);
        let (point, side) = self.cell_at(e.position);
        if e.modifiers.platform
            && e.button == MouseButton::Left
            && let Some(link) = self.link.clone().or_else(|| self.link_at(point))
        {
            self.open_link(link.target, cx);
            return;
        }
        if let Some(mode) = self.program_has_mouse(&e.modifiers)
            && let Some(button) = Button::of(e.button)
        {
            self.told_at = None;
            self.tell(button, true, false, &e.modifiers, e.position, mode);
            self.reported = Some(button);
            return;
        }
        match e.button {
            MouseButton::Left => {
                let kind = match e.click_count {
                    2 => SelectionType::Semantic,
                    3 => SelectionType::Lines,
                    _ => SelectionType::Simple,
                };
                self.term.lock().selection = Some(Selection::new(kind, point, side));
                self.selecting = true;
                cx.notify();
            }
            MouseButton::Right => self.menu(window, cx),
            _ => {}
        }
    }

    pub(super) fn mouse_move(&mut self, e: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.pointer = Some(e.position);
        self.hover_link(e.modifiers.platform, cx);
        if let Some(mode) = self.program_has_mouse(&e.modifiers) {
            let button = self.reported.filter(|_| e.pressed_button.is_some());
            if button.is_some() && mode.intersects(TermMode::MOUSE_DRAG | TermMode::MOUSE_MOTION) || mode.contains(TermMode::MOUSE_MOTION) {
                self.tell(button.unwrap_or(Button::None), true, true, &e.modifiers, e.position, mode);
            }
            return;
        }
        if !self.selecting || e.pressed_button != Some(MouseButton::Left) {
            return;
        }
        let (point, side) = self.cell_at(e.position);
        if let Some(selection) = self.term.lock().selection.as_mut() {
            selection.update(point, side);
        }
        cx.notify();
    }

    pub(super) fn mouse_up(&mut self, e: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        if let Some(button) = self.reported.take() {
            let mode = *self.term.lock().mode();
            if mouse::wanted(mode) && Button::of(e.button) == Some(button) {
                self.tell(button, false, false, &e.modifiers, e.position, mode);
            }
            return;
        }
        self.selecting = false;
    }

    /// ⌘ held or let go: the link under the mouse underlined, or not.
    pub(super) fn modifiers_changed(&mut self, e: &ModifiersChangedEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.hover_link(e.modifiers.platform, cx);
    }

    fn hover_link(&mut self, command: bool, cx: &mut Context<Self>) {
        let link = command.then(|| self.pointer.map(|p| self.cell_at(p).0)).flatten().and_then(|point| {
            // (Still on the one underlined: not looked for again.)
            match &self.link {
                Some(link) if link.contains(point) => Some(link.clone()),
                _ => self.link_at(point),
            }
        });
        let changed = link.as_ref().map(|l| (l.start, l.end)) != self.link.as_ref().map(|l| (l.start, l.end));
        self.link = link;
        if changed {
            cx.notify();
        }
    }

    /// The link at `point`: one a program made (OSC 8), else a URL or a
    /// path that's there, in its line's text (wrapped lines and all).
    pub(super) fn link_at(&self, point: GridPoint) -> Option<Link> {
        let term = self.term.lock();
        let grid = term.grid();
        if point.line < grid.topmost_line() || point.line > grid.bottommost_line() {
            return None;
        }
        let mut point = point;
        if grid[point].flags.contains(Flags::WIDE_CHAR_SPACER) && point.column.0 > 0 {
            point.column -= 1;
        }
        let last = grid.last_column();
        if let Some(link) = grid[point].hyperlink() {
            let same = |p: GridPoint| grid[p].hyperlink().is_some_and(|h| h == link);
            let (mut start, mut end) = (point, point);
            while let Some(p) = step(start, -1, last, grid.topmost_line(), grid.bottommost_line()).filter(|&p| same(p)) {
                start = p;
            }
            while let Some(p) = step(end, 1, last, grid.topmost_line(), grid.bottommost_line()).filter(|&p| same(p)) {
                end = p;
            }
            return Some(Link { start, end, target: Target::Url(link.uri().to_string()) });
        }
        // The line's text, from the row it wrapped from to the row it
        // wraps into.
        let wraps = |line: Line| grid[line][last].flags.contains(Flags::WRAPLINE);
        let mut first = point.line;
        while first > grid.topmost_line() && wraps(first - 1) {
            first -= 1;
        }
        let mut final_line = point.line;
        while final_line < grid.bottommost_line() && wraps(final_line) {
            final_line += 1;
        }
        let mut text = Vec::new();
        let mut points = Vec::new();
        for line in first.0..=final_line.0 {
            for col in 0..grid.columns() {
                let at = GridPoint::new(Line(line), Column(col));
                let cell = &grid[at];
                if cell.flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER) {
                    continue;
                }
                text.push(cell.c);
                points.push(at);
            }
        }
        drop(term);
        let at = points.iter().position(|p| *p == point)?;
        let (range, target) = links::at(&text, at)?;
        if let Target::Path(path) = &target {
            links::resolve(path, self.shell.as_ref().and_then(|s| s.folder()).as_deref())?;
        }
        Some(Link { start: points[range.start], end: points[range.end - 1], target })
    }

    /// A URL in your browser; a folder in an F2 window; a file in its app
    /// (one that runs, or an app, shown in F2 instead).
    fn open_link(&mut self, target: Target, cx: &mut Context<Self>) {
        self.link = None;
        match target {
            Target::Url(url) => cx.open_url(&url),
            Target::Path(text) => {
                let Some(path) = links::resolve(&text, self.shell.as_ref().and_then(|s| s.folder()).as_deref()) else { return };
                if path.is_dir() && path.extension().is_none_or(|e| e != "app") {
                    open_in_f2(path, cx);
                } else if runs(&path) {
                    crate::browser::reveal_all(vec![path], cx);
                } else {
                    cx.open_with_system(&path);
                }
            }
        }
        cx.notify();
    }

    pub(super) fn scroll(&mut self, e: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        let delta = f32::from(e.delta.pixel_delta(self.cell.height).y);
        self.scrolled += delta / f32::from(self.cell.height);
        let lines = self.scrolled.trunc() as i32;
        if lines == 0 {
            return;
        }
        self.scrolled -= lines as f32;
        if let Some(mode) = self.program_has_mouse(&e.modifiers) {
            let button = if lines > 0 { Button::WheelUp } else { Button::WheelDown };
            for _ in 0..lines.unsigned_abs().min(10) {
                self.told_at = None;
                self.tell(button, true, false, &e.modifiers, e.position, mode);
            }
            return;
        }
        let mut term = self.term.lock();
        // (A full-screen program, less or vim: its own scrolling, as arrows.)
        if term.mode().contains(TermMode::ALT_SCREEN | TermMode::ALTERNATE_SCROLL) {
            let app = term.mode().contains(TermMode::APP_CURSOR);
            drop(term);
            let arrow = keys::bytes(if lines > 0 { "up" } else { "down" }, &Modifiers::default(), app).unwrap_or_default();
            self.notifier.notify(arrow.repeat(lines.unsigned_abs() as usize));
        } else {
            term.scroll_display(Scroll::Delta(lines));
        }
        cx.notify();
    }

    /// Files dropped on it: their paths typed, quoted, as if pasted; the
    /// terminal in front, to go on typing.
    pub(super) fn drop_paths(&mut self, paths: &[PathBuf], window: &mut Window, cx: &mut Context<Self>) {
        let quoted: Vec<String> = paths.iter().map(|p| format!("'{}'", p.to_string_lossy().replace('\'', "'\\''"))).collect();
        if quoted.is_empty() {
            return;
        }
        self.paste_text(format!("{} ", quoted.join(" ")), cx);
        self.focus.focus(window, cx);
        cx.emit(Asked::Focus);
    }

    /// ⌘K: what's gone by, and the screen, cleared (at the shell's prompt,
    /// the prompt drawn again at the top: ⌃L).
    pub(super) fn clear(&mut self, cx: &mut Context<Self>) {
        let mut term = self.term.lock();
        if term.mode().contains(TermMode::ALT_SCREEN) {
            return;
        }
        term.scroll_display(Scroll::Bottom);
        term.selection = None;
        term.clear_screen(ClearMode::All);
        term.clear_screen(ClearMode::Saved);
        drop(term);
        if self.shell.as_ref().is_some_and(|s| !s.busy()) {
            self.notifier.notify(b"\x0c".to_vec());
        }
        cx.notify();
    }

    /// Everything selected, what's gone by too.
    pub(super) fn select_all(&mut self, cx: &mut Context<Self>) {
        let mut term = self.term.lock();
        let (top, bottom, last) = (term.grid().topmost_line(), term.grid().bottommost_line(), term.grid().last_column());
        let mut selection = Selection::new(SelectionType::Simple, GridPoint::new(top, Column(0)), Side::Left);
        selection.update(GridPoint::new(bottom, last), Side::Right);
        term.selection = Some(selection);
        drop(term);
        cx.notify();
    }

    /// ⌘+, ⌘− (a point at a time) and ⌘0: bigger or smaller than Settings
    /// have it, for this terminal, till it's back.
    pub(super) fn zoom(&mut self, by: Option<f32>, cx: &mut Context<Self>) {
        self.zoom = by.map_or(0., |by| (self.zoom + by).clamp(6. - self.theme.font_size, 48. - self.theme.font_size));
        cx.notify();
    }

    /// The shell's folder in an F2 window.
    pub(super) fn open_folder(&mut self, cx: &mut Context<Self>) {
        if let Some(folder) = self.shell.as_ref().and_then(|s| s.shell_folder()) {
            open_in_f2(folder, cx);
        }
    }

    /// The right-click menu.
    fn menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        const COPY: i32 = 1;
        const PASTE: i32 = 2;
        const SELECT_ALL: i32 = 3;
        const FIND: i32 = 4;
        const CLEAR: i32 = 5;
        const OPEN: i32 = 6;
        const NEW_TAB: i32 = 7;
        const SETTINGS: i32 = 8;
        let selected = self.term.lock().selection.as_ref().is_some_and(|s| !s.is_empty());
        let folder = self.shell.as_ref().and_then(|s| s.shell_folder());
        let item = |title: String, tag: i32, key: &'static str, modifiers: u64, enabled: bool| macos::MenuItem { title: Some(title), tag, key, modifiers, enabled, ..Default::default() };
        let separator = || macos::MenuItem { title: None, ..Default::default() };
        let items = vec![
            item("Copy".into(), COPY, "c", macos::COMMAND, selected),
            item("Paste".into(), PASTE, "v", macos::COMMAND, true),
            item("Select All".into(), SELECT_ALL, "a", macos::COMMAND, true),
            separator(),
            item("Find…".into(), FIND, "f", macos::COMMAND, true),
            item("Clear".into(), CLEAR, "k", macos::COMMAND, true),
            separator(),
            item(
                match &folder {
                    Some(f) => format!("Open “{}” in {}", f.file_name().map_or_else(|| "/".into(), |n| n.to_string_lossy()), crate::APP_NAME),
                    None => format!("Open in {}", crate::APP_NAME),
                },
                OPEN,
                "o",
                macos::COMMAND,
                folder.is_some(),
            ),
            item("New Tab".into(), NEW_TAB, "t", macos::COMMAND, true),
            separator(),
            item("Terminal Settings…".into(), SETTINGS, "", 0, true),
        ];
        // (Not inside this event's handling: the menu runs its own loop.)
        cx.spawn_in(window, async move |this, cx| {
            let Some(tag) = macos::popup_menu(&items) else { return };
            this.update_in(cx, |this, window, cx| match tag as i32 {
                COPY => this.copy(cx),
                PASTE => this.paste(cx),
                SELECT_ALL => this.select_all(cx),
                FIND => this.open_find(window, cx),
                CLEAR => this.clear(cx),
                OPEN => this.open_folder(cx),
                NEW_TAB => cx.emit(Asked::NewTab),
                SETTINGS => crate::settings_window::open_at("terminal", cx),
                _ => {}
            })
            .ok();
        })
        .detach();
    }
}

/// The cell `by` cells on from `point` (−1 back), wrapping from line to
/// line; None past the first or last.
fn step(point: GridPoint, by: i32, last: Column, top: Line, bottom: Line) -> Option<GridPoint> {
    match by {
        -1 if point.column.0 > 0 => Some(GridPoint::new(point.line, point.column - 1)),
        -1 if point.line > top => Some(GridPoint::new(point.line - 1, last)),
        1 if point.column < last => Some(GridPoint::new(point.line, point.column + 1)),
        1 if point.line < bottom => Some(GridPoint::new(point.line + 1, Column(0))),
        _ => None,
    }
}

/// A new F2 window at `folder`, F2 in front.
fn open_in_f2(folder: PathBuf, cx: &mut App) {
    crate::browser::open(folder, None, cx);
    cx.activate(true);
}

/// Whether opening it would run something (a script, a program, an
/// installer, an app): those are shown in F2 instead.
fn runs(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    let kind = path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
    let script = kind.as_deref().is_some_and(|k| ["app", "command", "tool", "sh", "zsh", "bash", "fish", "py", "rb", "pl", "js", "scpt", "applescript", "workflow", "pkg", "mpkg"].contains(&k));
    let executable = path.is_file() && std::fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o111 != 0);
    script || executable
}
