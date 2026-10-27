//! Finder's keyboard: typing a name selects it, Home and End, Page Up and
//! Down, ⌥→ and ⌥← to expand and collapse everything inside, ⌃1–⌃7 to
//! toggle tags.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use gpui::*;

use super::menu::TAGS;
use super::{Finder, View};
use crate::listing;

/// Typing more within this keeps adding to the name looked for.
const TYPE_PAUSE: Duration = Duration::from_millis(900);
/// ⌥→ stops expanding after this many folders.
const EXPAND_LIMIT: usize = 400;

impl Finder {
    /// Select the first item whose name starts with what's been typed (or,
    /// with none, the one that would come next by name).
    pub(super) fn type_select(&mut self, text: &str, cx: &mut Context<Self>) {
        let (typed, at) = &mut self.typed;
        if at.elapsed() > TYPE_PAUSE {
            typed.clear();
        }
        typed.push_str(&text.to_lowercase());
        *at = Instant::now();
        let typed = typed.clone();
        let names: Vec<String> = self.rows.iter().map(|e| e.name.to_lowercase()).collect();
        let found = names.iter().position(|n| n.starts_with(&typed)).or_else(|| {
            names.iter().enumerate().filter(|(_, n)| n.as_str() > typed.as_str()).min_by(|a, b| a.1.cmp(b.1)).map(|(i, _)| i)
        });
        if let Some(ix) = found {
            self.select_only(Some(ix));
            self.scroll_to(ix, ScrollStrategy::Nearest);
            cx.notify();
        }
    }

    /// Home and End (⌥↑ ⌥↓): the first or last item.
    pub(super) fn select_end(&mut self, last: bool, cx: &mut Context<Self>) {
        if self.rows.is_empty() {
            return;
        }
        let ix = if last { self.rows.len() - 1 } else { 0 };
        self.select_only(Some(ix));
        self.scroll_to(ix, ScrollStrategy::Nearest);
        cx.notify();
    }

    /// Page Up and Down: a screenful of rows.
    pub(super) fn page(&mut self, by: isize, window: &mut Window, cx: &mut Context<Self>) {
        let height = f32::from(window.viewport_size().height) - 180.;
        let per_page = match self.view {
            View::Icons => (height / super::items::cell_height(self.icon_size)).max(1.) as isize * self.columns.max(1) as isize,
            _ => (height / super::items::ROW_HEIGHT).max(1.) as isize,
        };
        self.select_by(by * per_page, cx);
    }

    /// ⌥→: expand the selected folders and everything in them; ⌥←:
    /// collapse them all.
    pub(super) fn expand_all(&mut self, open: bool, cx: &mut Context<Self>) {
        if self.flat() || self.view != View::List {
            return;
        }
        let roots: Vec<PathBuf> =
            self.selection().into_iter().filter(|e| e.is_dir && !e.is_bundle()).map(|e| e.path).collect();
        if roots.is_empty() {
            return;
        }
        if !open {
            self.expanded.retain(|p| !roots.iter().any(|r| p.starts_with(r)));
            return self.refresh_in_place(cx);
        }
        cx.spawn(async move |this, cx| {
            let listed = cx
                .background_executor()
                .spawn(async move {
                    let mut listed: HashMap<PathBuf, Vec<listing::Entry>> = HashMap::new();
                    let mut queue: Vec<PathBuf> = roots;
                    while let Some(dir) = queue.pop() {
                        if listed.len() >= EXPAND_LIMIT {
                            break;
                        }
                        let entries = listing::list_dir(&dir).unwrap_or_default();
                        queue.extend(entries.iter().filter(|e| e.is_dir && !e.is_bundle() && !e.hidden).map(|e| e.path.clone()));
                        listed.insert(dir, entries);
                    }
                    listed
                })
                .await;
            this.update(cx, |this, cx| {
                for (dir, entries) in listed {
                    this.expanded.insert(dir.clone());
                    this.children.insert(dir, entries);
                }
                this.refresh_in_place(cx);
            })
            .ok();
        })
        .detach();
    }

    /// ⌃1–⌃7: Finder's tags in its order (Red... Gray), on or off for the
    /// selection.
    pub(super) fn tag_shortcut(&mut self, n: usize, cx: &mut Context<Self>) {
        let Some((name, _)) = TAGS.get(n) else { return };
        let items = self.selection();
        if items.is_empty() {
            return;
        }
        let all = items.iter().all(|e| e.tags.iter().any(|(t, _)| t == name));
        let paths: Vec<PathBuf> = items.into_iter().map(|e| e.path).collect();
        self.set_tag(&paths, name, !all, cx);
    }

    /// ⌃0: no tags.
    pub(super) fn clear_tags(&mut self, cx: &mut Context<Self>) {
        let paths = self.selection_paths();
        for path in &paths {
            if let Err(e) = crate::macos::set_tags(path, &[]) {
                return self.show_notice(e, cx);
            }
        }
        self.pending_select = paths.clone();
        let dirs: Vec<PathBuf> = paths.iter().filter_map(|p| p.parent().map(Into::into)).collect();
        self.reload_folders(&dirs, cx);
    }
}
