//! Selecting one item or several, as in Finder: click for one, ⌘-click to
//! add or remove one, ⇧-click or ⇧-arrows for a range, ⌘A for all, a click
//! on empty space for none. Kept by path, so it survives sorting and
//! reloads; `selected` is the one the keyboard is on.

use std::path::PathBuf;

use gpui::*;

use super::Finder;
use crate::listing::Entry;

impl Finder {
    /// Select just the item at `ix`, or nothing.
    pub(super) fn select_only(&mut self, ix: Option<usize>) {
        self.selected = ix.filter(|&i| i < self.rows.len());
        self.chosen.clear();
        self.anchor = self.selected.map(|i| self.rows[i].path.clone());
        self.chosen.extend(self.anchor.clone());
    }

    pub(super) fn is_chosen(&self, ix: usize) -> bool {
        self.rows.get(ix).is_some_and(|e| self.chosen.contains(&e.path))
    }

    /// ⌘-click: add the item to the selection, or take it out.
    pub(super) fn toggle_chosen(&mut self, ix: usize) {
        let Some(path) = self.rows.get(ix).map(|e| e.path.clone()) else { return };
        if self.chosen.remove(&path) {
            // The keyboard moves to another selected item, if any.
            self.selected = self.rows.iter().position(|e| self.chosen.contains(&e.path));
        } else {
            self.chosen.insert(path.clone());
            self.selected = Some(ix);
        }
        self.anchor = Some(path);
    }

    /// ⇧-click or ⇧-arrow: everything from where the range started to `ix`.
    pub(super) fn extend_to(&mut self, ix: usize) {
        if ix >= self.rows.len() {
            return;
        }
        let from = self.anchor.as_ref().and_then(|a| self.rows.iter().position(|e| e.path == *a)).or(self.selected).unwrap_or(ix);
        let range = from.min(ix)..=from.max(ix);
        self.chosen = self.rows[range].iter().map(|e| e.path.clone()).collect();
        self.selected = Some(ix);
    }

    /// ⇧↑ / ⇧↓ (a row of icons at a time in icon view).
    pub(super) fn extend_by(&mut self, rows: isize, cx: &mut Context<Self>) {
        let step = if self.view == super::View::Icons { self.columns.max(1) as isize } else { 1 };
        let Some(ix) = self.selected else { return self.select_by(rows, cx) };
        let to = (ix as isize + rows * step).clamp(0, self.rows.len() as isize - 1) as usize;
        if self.anchor.is_none() {
            self.anchor = Some(self.rows[ix].path.clone());
        }
        self.extend_to(to);
        self.scroll_to(to, ScrollStrategy::Nearest);
        cx.notify();
    }

    pub(super) fn select_all(&mut self, cx: &mut Context<Self>) {
        self.chosen = self.rows.iter().map(|e| e.path.clone()).collect();
        if self.selected.is_none() && !self.rows.is_empty() {
            self.selected = Some(0);
        }
        cx.notify();
    }

    /// The selected items, in the order shown.
    pub(super) fn selection(&self) -> Vec<Entry> {
        self.rows.iter().filter(|e| self.chosen.contains(&e.path)).cloned().collect()
    }

    pub(super) fn selection_paths(&self) -> Vec<PathBuf> {
        self.rows.iter().filter(|e| self.chosen.contains(&e.path)).map(|e| e.path.clone()).collect()
    }
}
