//! ⌘F: text found in the terminal and what's gone by. Each one found is
//! marked, the one shown more so; ↩ goes back to the one before it (older),
//! ⇧↩ on; esc puts it away. As the shell writes on, it's looked for again
//! (a few times a second at most).

use std::time::Duration;

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Direction, Point as GridPoint};
use alacritty_terminal::term::search::{Match, RegexIter, RegexSearch};
use gpui::*;
use gpui_kit::component::input::{Escape, Input, InputEvent, InputState};

use super::TerminalView;
use crate::style::{SEMIBOLD, symbol};

/// At most this many found (the newest kept).
const MOST: usize = 5_000;
/// Looked for again, as the shell writes on, at most this often.
const AGAIN: Duration = Duration::from_millis(250);

pub struct Find {
    pub input: Entity<InputState>,
    /// What's found, oldest first, and which one's shown.
    pub matches: Vec<Match>,
    pub current: Option<usize>,
    /// The shell wrote since it was looked for (its lines moved), and a
    /// look's been asked for.
    stale: bool,
    waiting: bool,
    _watch: Subscription,
}

impl TerminalView {
    pub(super) fn open_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(find) = &self.find {
            find.input.update(cx, |input, cx| {
                input.focus(window, cx);
                input.select_all(window, cx);
            });
            return;
        }
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Find"));
        input.update(cx, |input, cx| input.focus(window, cx));
        let watch = cx.subscribe_in(&input, window, |this, _, e: &InputEvent, _, cx| match e {
            InputEvent::Change => this.search(true, cx),
            InputEvent::PressEnter { shift, .. } => this.step_find(!shift, cx),
            _ => {}
        });
        self.find = Some(Find { input, matches: Vec::new(), current: None, stale: false, waiting: false, _watch: watch });
        cx.notify();
    }

    pub(super) fn close_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.find = None;
        self.focus.focus(window, cx);
        cx.notify();
    }

    /// Looked for (`fresh`: what's looked for changed, so the newest is
    /// shown; else the one shown stays, as far back from the newest).
    fn search(&mut self, fresh: bool, cx: &mut Context<Self>) {
        let Some(find) = &mut self.find else { return };
        let query = find.input.read(cx).value().to_string();
        let back = find.current.map(|c| find.matches.len() - c);
        find.matches.clear();
        find.stale = false;
        if !query.is_empty()
            && let Ok(mut regex) = RegexSearch::new(&escape(&query))
        {
            let term = self.term.lock();
            let start = GridPoint::new(term.topmost_line(), Column(0));
            let end = GridPoint::new(term.bottommost_line(), term.last_column());
            // (From the newest back, so it's the oldest that are left out.)
            let mut found: Vec<Match> = RegexIter::new(end, start, Direction::Left, &term, &mut regex).take(MOST).collect();
            found.reverse();
            find.matches = found;
        }
        let n = find.matches.len();
        find.current = match back {
            Some(back) if !fresh && back <= n && n > 0 => Some(n - back),
            _ => n.checked_sub(1),
        };
        if fresh {
            self.show_found();
        }
        cx.notify();
    }

    /// The one before (`older`), or after, shown.
    pub(super) fn step_find(&mut self, older: bool, cx: &mut Context<Self>) {
        let Some(find) = &mut self.find else { return };
        let n = find.matches.len();
        if n == 0 {
            return;
        }
        find.current = Some(match find.current {
            Some(c) if older => (c + n - 1) % n,
            Some(c) => (c + 1) % n,
            None => n - 1,
        });
        self.show_found();
        cx.notify();
    }

    fn show_found(&mut self) {
        let Some(found) = self.find.as_ref().and_then(|f| Some(f.matches.get(f.current?)?.clone())) else { return };
        self.term.lock().scroll_to_point(*found.start());
    }

    /// The shell wrote: looked for again, soon.
    pub(super) fn find_stale(&mut self, cx: &mut Context<Self>) {
        let Some(find) = &mut self.find else { return };
        find.stale = true;
        if find.waiting {
            return;
        }
        find.waiting = true;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(AGAIN).await;
            this.update(cx, |this, cx| {
                if let Some(find) = &mut this.find {
                    find.waiting = false;
                    if find.stale {
                        this.search(false, cx);
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    /// Whether `point` is in one found: the one shown, or another.
    pub(super) fn found_at(&self, point: GridPoint, visible: &[(usize, Match)]) -> Option<bool> {
        let current = self.find.as_ref()?.current;
        visible.iter().find(|(_, m)| m.contains(&point)).map(|(i, _)| Some(*i) == current)
    }

    /// Those found between `top` and `bottom` (the lines on screen).
    pub(super) fn found_between(&self, top: GridPoint, bottom: GridPoint) -> Vec<(usize, Match)> {
        let Some(find) = &self.find else { return Vec::new() };
        find.matches.iter().enumerate().filter(|(_, m)| *m.end() >= top && *m.start() <= bottom).map(|(i, m)| (i, m.clone())).collect()
    }

    /// The find bar: the field, how many, before and after, done.
    pub(super) fn find_bar(&self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let find = self.find.as_ref()?;
        let t = &self.theme;
        let fg: Hsla = rgb(t.foreground).into();
        let bg: Hsla = rgb(t.background).into();
        // (A little lighter, or darker, than the terminal.)
        let bar = if bg.l < 0.5 { Hsla { l: (bg.l + 0.08).min(1.), ..bg } } else { Hsla { l: (bg.l - 0.06).max(0.), ..bg } };
        let quiet = fg.opacity(0.6);
        let count = match (find.current, find.matches.len()) {
            (_, 0) if find.input.read(cx).value().is_empty() => String::new(),
            (_, 0) => "None".into(),
            (Some(c), n) => format!("{} of {n}", c + 1),
            (None, n) => format!("{n}"),
        };
        let button = |id: &'static str, name: &'static str| div().id(id).size(px(22.)).rounded_md().flex().items_center().justify_center().hover(|d| d.bg(fg.opacity(0.1))).child(symbol(name, 11., SEMIBOLD, quiet, window));
        Some(
            div()
                .id("terminal-find")
                .occlude()
                .absolute()
                .top(px(8.))
                .right(px(14.))
                .h(px(32.))
                .pl_3()
                .pr_1()
                .flex()
                .items_center()
                .gap_1()
                .rounded_lg()
                .bg(bar)
                .border_1()
                .border_color(fg.opacity(0.15))
                .shadow_md()
                .text_color(fg)
                .capture_action(cx.listener(|this, _: &Escape, window, cx| {
                    this.close_find(window, cx);
                    cx.stop_propagation();
                }))
                .child(symbol("magnifyingglass", 11., SEMIBOLD, quiet, window))
                .child(div().w(px(190.)).child(Input::new(&find.input).appearance(false).text_size(px(12.)).text_color(fg)))
                .child(div().min_w(px(56.)).text_size(px(11.)).text_color(quiet).text_right().child(count))
                .child(button("terminal-find-older", "chevron.up").on_click(cx.listener(|this, _, _, cx| this.step_find(true, cx))))
                .child(button("terminal-find-newer", "chevron.down").on_click(cx.listener(|this, _, _, cx| this.step_find(false, cx))))
                .child(button("terminal-find-done", "xmark").on_click(cx.listener(|this, _, window, cx| this.close_find(window, cx))))
                .into_any_element(),
        )
    }
}

/// `text` as a regex that finds it as it is.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if "\\.+*?()|[]{}^$#&-~".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn as_it_is() {
        assert_eq!(super::escape("a.b (c)"), "a\\.b \\(c\\)");
    }
}
