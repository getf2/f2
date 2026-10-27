//! Extensions' pages in Quick Open (see `crate::extensions`): what a
//! command shows, as its program says it, as it says it: a list (with the
//! row picked's detail beside it, if its rows have them) or a page to
//! read; and a command's rows in Quick Open's own results, by its keyword
//! or (a root command's) by what's typed.

use std::sync::Arc;

use gpui_kit::component::text::TextView;
use serde_json::Value;

use super::*;
use crate::extensions::{self, Action, Detail, Extension, Heard, Loading, Row, Search, Shown, Spec};

/// Where rows are from: an extension's command, with its arguments (what
/// their actions push and run are that extension's).
pub(super) struct Origin {
    pub ext: Arc<Extension>,
    pub command: String,
    pub args: Value,
}

/// An extension's row, as a result.
pub(super) struct ExtRow {
    pub row: Arc<Row>,
    pub from: Arc<Origin>,
}

/// What started a run: what it may do. (It copies or opens something only
/// when you've just asked it for something: not on its own, as you type.)
#[derive(Clone, Copy, PartialEq)]
enum Ran {
    /// Its page's, as you opened it, or asked again (⌘R).
    Chosen,
    /// Its page's, asked again as you typed, or as it said to.
    Again,
    /// An action's, as you chose it: it says what's happened, not what's on
    /// the page.
    Action,
}

/// A page of an extension's, open (the last of `QuickPanel::pages` shown;
/// esc goes back to the one before).
pub(super) struct Page {
    id: u64,
    from: Arc<Origin>,
    spec: Spec,
    /// What it shows (what it showed last time, at first: `stale`, till
    /// its program says what's there now). Its rows are those kept, not
    /// copies.
    shown: Shown,
    stale: bool,
    /// It's working on it: what it says it's doing, and how far along.
    loading: Option<(Option<String>, Option<f32>)>,
    error: Option<String>,
    /// Said something wrong this run: what it shows isn't kept.
    failed: bool,
    /// Its program's stderr, its last run's (⌘K › Show Output).
    output: String,
    /// What was typed on it, for coming back to it.
    typed: String,
    /// A page of F2's own (its output): never run.
    local: bool,
    /// Its run, reading what it says (its program stopped when it's
    /// dropped), or waiting for typing to stop to start one.
    running: Option<Task<()>>,
    /// An action being run.
    doing: Option<Task<()>>,
    /// Its extension's files looked at now and then (while it's kept):
    /// saved, it runs again.
    _watch: Option<Task<()>>,
}

impl Page {
    fn title(&self) -> String {
        self.shown.title.clone().unwrap_or_else(|| self.spec.title.clone())
    }
}

impl QuickPanel {
    pub(super) fn page(&self) -> Option<&Page> {
        self.pages.last().filter(|_| self.mode == Mode::Extension)
    }

    /// `from`'s page opened, over the one shown (`typed` in its field):
    /// what it showed last time at once, and its program asked what's
    /// there now (unless that's still fresh).
    pub(super) fn open_page(&mut self, from: Arc<Origin>, typed: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(spec) = from.ext.spec(&from.command).cloned() else {
            self.error = Some(format!("{} has no command \"{}\".", from.ext.manifest.name, from.command).into());
            return cx.notify();
        };
        // (What it needs set first, in its settings, or a program allowed:
        // Quick Open goes, and they come up, at it.)
        if !from.ext.may_run() || !from.ext.missing().is_empty() {
            self.close_keeping_focus(window, cx);
            let id = from.ext.id.clone();
            cx.defer(move |cx| {
                crate::settings_window::open_extension(&id, cx);
                cx.activate(true);
            });
            return;
        }
        match self.pages.last_mut().filter(|_| self.mode == Mode::Extension) {
            Some(top) => top.typed = self.input.read(cx).value().to_string(),
            None => self.searched_before = self.input.read(cx).value().to_string(),
        }
        let cached = extensions::cached(&from.ext.id, &from.command, &from.args);
        let fresh = cached.as_ref().is_some_and(|c| spec.fresh_for().is_some_and(|f| c.age() < f));
        let shown = cached.map(|c| (*c).clone()).unwrap_or_default();
        self.next_page += 1;
        let page = Page {
            id: self.next_page,
            from,
            spec,
            shown,
            stale: true,
            loading: None,
            error: None,
            failed: false,
            output: String::new(),
            typed: String::new(),
            local: false,
            running: None,
            doing: None,
            _watch: None,
        };
        let script = page.spec.search == Search::Script;
        let watch = self.watch_page(page.id, page.from.ext.clone(), window, cx);
        self.pages.push(Page { _watch: Some(watch), ..page });
        self.enter_page(typed, window, cx);
        if !fresh || script {
            self.run_page(Ran::Chosen, window, cx);
        }
    }

    /// A page of F2's own, over the one shown: `markdown` to read.
    fn open_local(&mut self, title: String, markdown: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(top) = self.pages.last_mut() else { return };
        top.typed = self.input.read(cx).value().to_string();
        let (from, spec) = (top.from.clone(), top.spec.clone());
        self.next_page += 1;
        self.pages.push(Page {
            id: self.next_page,
            from,
            spec,
            shown: Shown { title: Some(title), detail: Some(Detail { markdown, ..Default::default() }), ..Default::default() },
            stale: false,
            loading: None,
            error: None,
            failed: false,
            output: String::new(),
            typed: String::new(),
            local: true,
            running: None,
            doing: None,
            _watch: None,
        });
        self.enter_page(String::new(), window, cx);
    }

    /// Page `id` watched while it's open, twice a second: when its
    /// extension's saved (its script, its manifest), it runs again.
    fn watch_page(&mut self, id: u64, ext: Arc<Extension>, window: &mut Window, cx: &mut Context<Self>) -> Task<()> {
        let mut was = ext.changed();
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(std::time::Duration::from_millis(500)).await;
                let now = ext.changed();
                if now == was {
                    continue;
                }
                was = now;
                if this.update_in(cx, |this, window, cx| this.saved(id, window, cx)).is_err() {
                    return;
                }
            }
        })
    }

    /// Page `id`'s extension was saved: if it's the page shown, run again,
    /// as the extension is now (its manifest read again), or what's wrong
    /// with its manifest said.
    fn saved(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        if self.mode != Mode::Extension || !self.pages.last().is_some_and(|p| p.id == id) {
            return;
        }
        extensions::look_again();
        let (exts, broken) = extensions::every();
        let Some(page) = self.pages.last_mut() else { return };
        if let Some(b) = broken.iter().find(|b| b.dir == page.from.ext.dir) {
            (page.error, page.loading, page.running) = (Some(b.why.clone()), None, None);
            self.find(cx);
            return cx.notify();
        }
        if let Some(ext) = exts.into_iter().find(|e| e.dir == page.from.ext.dir) {
            let Some(spec) = ext.spec(&page.from.command).cloned() else {
                (page.error, page.loading, page.running) = (Some(format!("f2.json has no command \"{}\" now.", page.from.command)), None, None);
                self.find(cx);
                return cx.notify();
            };
            page.from = Arc::new(Origin { ext, command: page.from.command.clone(), args: page.from.args.clone() });
            page.spec = spec;
        }
        self.run_page(Ran::Again, window, cx);
    }

    /// The last page shown, `typed` in its field.
    fn enter_page(&mut self, typed: String, window: &mut Window, cx: &mut Context<Self>) {
        self.mode = Mode::Extension;
        (self.switching, self.steps) = (Switching::No, 0);
        (self.acting, self.confirming, self.error) = (None, None, None);
        self.selected = 0;
        self.input.update(cx, |i, cx| {
            i.set_value(typed, window, cx);
            i.focus(window, cx);
        });
        self.find(cx);
        cx.notify();
    }

    /// Back a page (esc), or, from the first, back to the search.
    pub(super) fn pop_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pages.pop();
        match self.pages.last() {
            Some(page) => {
                let typed = page.typed.clone();
                self.enter_page(typed, window, cx);
            }
            None => {
                self.set_mode(Mode::Search, window, cx);
                let typed = std::mem::take(&mut self.searched_before);
                self.input.update(cx, |i, cx| i.set_value(typed, window, cx));
                self.find(cx);
            }
        }
    }

    /// The page shown's program asked what's there (for what's typed, if
    /// it looks through it itself), what it said before stopped.
    fn run_page(&mut self, ran: Ran, window: &mut Window, cx: &mut Context<Self>) {
        let typed = self.input.read(cx).value().to_string();
        let Some(page) = self.pages.last_mut().filter(|p| !p.local) else { return };
        let query = if page.spec.search == Search::Script { typed } else { String::new() };
        (page.stale, page.failed, page.error) = (true, false, None);
        page.running = None;
        let from = page.from.clone();
        let called = call(&from.ext, &from.command, &from.args, &query, cx);
        let Some(page) = self.pages.last_mut() else { return };
        match called {
            Err(why) => {
                page.loading = None;
                page.error = Some(why);
            }
            Ok((running, mut heard)) => {
                page.loading = Some((None, None));
                let id = page.id;
                page.running = Some(cx.spawn_in(window, async move |this, cx| {
                    let _running = running;
                    while let Some(heard) = heard.next().await {
                        if this.update_in(cx, |this, window, cx| this.heard(id, heard, ran, window, cx)).is_err() {
                            return;
                        }
                    }
                }));
            }
        }
        self.find(cx);
    }

    /// What's typed changed: a page whose program looks through it itself
    /// asked again, once typing stops a moment.
    pub(super) fn typed_on_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(page) = self.pages.last_mut().filter(|p| self.mode == Mode::Extension && p.spec.search == Search::Script && !p.local) else { return };
        let id = page.id;
        page.loading = Some((None, None));
        // (The last run stopped meanwhile.)
        page.running = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(std::time::Duration::from_millis(250)).await;
            this.update_in(cx, |this, window, cx| {
                if this.pages.last().is_some_and(|p| p.id == id) {
                    this.run_page(Ran::Again, window, cx);
                }
            })
            .ok();
        }));
        cx.notify();
    }

    /// What a run said, on page `id`.
    fn heard(&mut self, id: u64, heard: Heard, ran: Ran, window: &mut Window, cx: &mut Context<Self>) {
        let action = ran == Ran::Action;
        let at = self.pages.iter().position(|p| p.id == id);
        let name = at.map(|at| self.pages[at].from.ext.manifest.name.clone()).unwrap_or_default();
        let mut then: Option<fn(&mut QuickPanel, &mut Window, &mut Context<QuickPanel>)> = None;
        match heard {
            Heard::Said(mut message) => {
                if let Some(page) = at.map(|at| &mut self.pages[at]) {
                    let progress = message.progress.or(page.loading.as_ref().and_then(|l| l.1));
                    match &message.loading {
                        Some(Loading::Said(what)) => page.loading = Some((Some(what.clone()), progress)),
                        Some(Loading::On(true)) => page.loading = Some((page.loading.take().and_then(|l| l.0), progress)),
                        Some(Loading::On(false)) => page.loading = None,
                        None if message.progress.is_some() => page.loading = Some((page.loading.take().and_then(|l| l.0), progress)),
                        None => {}
                    }
                    if !action {
                        extensions::fold(&mut page.shown, &mut message, &mut page.stale);
                    }
                    if let Some(why) = &message.error {
                        page.error = Some(why.clone());
                        page.failed = true;
                    }
                }
                if let Some(notice) = message.notice {
                    let (title, text) = match notice {
                        extensions::Notice::Text(text) => (name.clone(), text),
                        extensions::Notice::Titled { title, text } => (title, text),
                    };
                    crate::shots::note(title, text, None, cx);
                }
                if at.is_none()
                    && let Some(why) = message.error
                {
                    crate::shots::note(format!("{name} Didn't Work"), why, None, cx);
                }
                if ran != Ran::Again {
                    if let Some(text) = message.copy {
                        macos::copy_text(&text);
                    }
                    if let Some(target) = message.open {
                        open_target(&target, cx);
                    }
                }
                if message.refresh {
                    then = Some(|this, window, cx| this.run_page(Ran::Again, window, cx));
                }
                if message.pop {
                    then = Some(|this, window, cx| this.pop_page(window, cx));
                }
                if message.close {
                    then = Some(|this, window, cx| this.close(window, cx));
                }
            }
            Heard::Text(text) => match at.map(|at| &mut self.pages[at]) {
                Some(page) if !action => {
                    // (Its lines kept as they are: Markdown would run them together.)
                    let markdown = text.trim_end().lines().collect::<Vec<_>>().join("  \n");
                    page.shown.detail = Some(Detail { markdown, ..Default::default() });
                    page.shown.items.clear();
                    page.stale = false;
                }
                // (An action's: said, the first line.)
                _ => crate::shots::note(name.clone(), text.lines().find(|l| !l.trim().is_empty()).unwrap_or("Done").to_string(), None, cx),
            },
            Heard::Garbled(why) => match at.map(|at| &mut self.pages[at]) {
                Some(page) => {
                    page.error = Some(why);
                    page.failed = true;
                }
                None => crate::shots::note(format!("{name} Didn't Work"), why, None, cx),
            },
            Heard::Ended { ok, output } => {
                if let Some(page) = at.map(|at| &mut self.pages[at]) {
                    if !output.trim().is_empty() {
                        page.output = output.clone();
                    }
                    if action {
                        page.doing = None;
                    } else {
                        page.loading = None;
                        page.running = None;
                        if ok && !page.failed && !page.stale {
                            page.shown.at = extensions::now();
                            extensions::keep(&page.from.ext.id, &page.from.command, &page.from.args, page.shown.clone());
                        }
                    }
                    if !ok && page.error.is_none() {
                        let last = output.lines().rev().find(|l| !l.trim().is_empty()).map(str::trim).unwrap_or("it stopped with an error");
                        page.error = Some(format!("{name} didn't work: {last}"));
                    }
                } else if !ok {
                    let last = output.lines().rev().find(|l| !l.trim().is_empty()).map(str::trim).unwrap_or("it stopped with an error").to_string();
                    crate::shots::note(format!("{name} Didn't Work"), last, None, cx);
                }
            }
        }
        if at.is_some_and(|at| at + 1 == self.pages.len()) && self.mode == Mode::Extension {
            self.find(cx);
        }
        if let Some(then) = then {
            then(self, window, cx);
        }
        cx.notify();
    }

    /// The page's rows matching what's typed (all, with nothing typed, or
    /// when its program looks through them itself), by section, the best
    /// first in each.
    pub(super) fn find_on_page(&mut self, cx: &mut Context<Self>) {
        let text = self.input.read(cx).value().to_string();
        let same = text == self.searched;
        self.searched = text.clone();
        self.hint = None;
        let Some(page) = self.pages.last() else { return };
        let q = text.trim().to_lowercase();
        let filter = page.spec.search == Search::F2 && !q.is_empty();
        let mut groups: Vec<(Option<String>, Vec<(f32, Arc<Row>)>)> = Vec::new();
        for row in &page.shown.items {
            let score = if filter { row_score(row, &q) } else { Some(0.) };
            let Some(score) = score else { continue };
            match groups.iter_mut().find(|(s, _)| *s == row.section) {
                Some((_, rows)) => rows.push((score, row.clone())),
                None => groups.push((row.section.clone(), vec![(score, row.clone())])),
            }
        }
        let (mut items, mut sections) = (Vec::new(), Vec::new());
        for (section, mut rows) in groups {
            if filter {
                rows.sort_by(|a, b| b.0.total_cmp(&a.0));
            }
            if let Some(section) = section {
                sections.push((items.len(), section.into()));
            }
            items.extend(rows.into_iter().map(|(_, row)| Item::Ext(Arc::new(ExtRow { row, from: page.from.clone() }))));
        }
        self.items = items;
        self.sections = sections;
        self.error = page.error.clone().map(Into::into);
        self.lay_out();
        // (More come in as it's read: the row picked stays.)
        if same {
            self.selected = self.selected.min(self.items.len().saturating_sub(1));
        } else {
            (self.selected, self.acting, self.confirming) = (0, None, None);
        }
        self.appear(cx);
        cx.notify();
    }

    /// The selected row's actions (else, on a page to read, its own).
    pub(super) fn ext_actions(&self) -> Vec<Action> {
        match self.items.get(self.selected) {
            Some(Item::Ext(e)) => e.row.actions.clone(),
            _ => self.page().and_then(|p| p.shown.detail.as_ref()).map(|d| d.actions.clone()).unwrap_or_default(),
        }
    }

    /// What ⌘K offers on an extension's row or page: its actions (Return
    /// the first, ⌘Return the second), then F2's: asked again, its output,
    /// its token forgotten.
    pub(super) fn page_actions(&self) -> Vec<(Act, String, &'static str)> {
        let mut acts: Vec<(Act, String, &'static str)> =
            self.ext_actions().into_iter().enumerate().map(|(i, a)| (Act::Ext(i), a.title, match i { 0 => "↩", 1 => "⌘↩", _ => "" })).collect();
        if let Some(page) = self.page() {
            if !page.local {
                acts.push((Act::Again, "Refresh".into(), "⌘R"));
            }
            if !page.output.trim().is_empty() {
                acts.push((Act::ExtOutput, "Show Output".into(), ""));
            }
            if self.page_has_settings() {
                acts.push((Act::ExtSettings, format!("{} Settings…", page.from.ext.manifest.name), "⌘,"));
            }
            if page.from.ext.has_passwords() {
                acts.push((Act::ExtForget, format!("Forget {}'s Saved Passwords", page.from.ext.manifest.name), ""));
            }
        }
        acts
    }

    /// Return on an extension's row (`second`: ⌘Return, its second
    /// action), or on a page to read.
    pub(super) fn choose_ext(&mut self, second: bool, window: &mut Window, cx: &mut Context<Self>) {
        // (Asked, and Return again: the action asked about.)
        let i = if self.confirming.is_some() { self.ext_confirm } else { usize::from(second && self.ext_actions().len() > 1) };
        self.perform_ext(i, window, cx);
    }

    /// Do the selected row's action `i`: asking first if it says to.
    pub(super) fn perform_ext(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(action) = self.ext_actions().into_iter().nth(i) else { return };
        let from = match self.items.get(self.selected) {
            Some(Item::Ext(e)) => e.from.clone(),
            _ => match self.page() {
                Some(page) => page.from.clone(),
                None => return,
            },
        };
        if action.confirm.is_some() && !(self.confirming.is_some() && self.ext_confirm == i) {
            (self.confirming, self.ext_confirm, self.acting) = (Some(self.selected), i, None);
            return cx.notify();
        }
        self.confirming = None;
        if let Some(target) = &action.open {
            open_target(target, cx);
            self.close_keeping_focus(window, cx);
        } else if let Some(text) = &action.copy {
            macos::copy_text(text);
            self.close(window, cx);
        } else if let Some(path) = &action.reveal {
            let path = crate::finder::expand(path);
            self.close_keeping_focus(window, cx);
            cx.defer(move |cx| {
                crate::browser::open(path.parent().map(Path::to_path_buf).unwrap_or_default(), Some(path), cx);
                cx.activate(true);
            });
        } else if let Some(call) = &action.push {
            let origin = Arc::new(Origin { ext: from.ext.clone(), command: call.command.clone(), args: call.args.clone() });
            self.open_page(origin, String::new(), window, cx);
        } else if let Some(call) = &action.run {
            self.run_action(&from.ext, call, window, cx);
        }
        cx.notify();
    }

    /// An action's run: the program asked to do something, the page
    /// staying (showing it's working on it).
    fn run_action(&mut self, ext: &Arc<Extension>, call: &extensions::Call, window: &mut Window, cx: &mut Context<Self>) {
        let query = self.input.read(cx).value().to_string();
        let (running, mut heard) = match self::call(ext, &call.command, &call.args, &query, cx) {
            Ok(run) => run,
            Err(why) => {
                self.error = Some(why.into());
                return cx.notify();
            }
        };
        let id = self.page().map_or(0, |p| p.id);
        let task = cx.spawn_in(window, async move |this, cx| {
            let _running = running;
            while let Some(heard) = heard.next().await {
                if this.update_in(cx, |this, window, cx| this.heard(id, heard, Ran::Action, window, cx)).is_err() {
                    return;
                }
            }
        });
        match self.pages.last_mut().filter(|p| p.id == id && id != 0) {
            Some(page) => page.doing = Some(task),
            // (From Quick Open's own results: it goes on without a page.)
            None => task.detach(),
        }
    }

    /// The page shown has settings: an extension's, or (a page of F2's own)
    /// some in F2's.
    pub(super) fn page_has_settings(&self) -> bool {
        self.page().is_some_and(|p| p.from.ext.native.is_none() || native_settings(&p.from.ext).is_some())
    }

    /// ⌘K's other actions on a page: its output shown, its passwords
    /// forgotten, its settings.
    pub(super) fn perform_page(&mut self, act: Act, window: &mut Window, cx: &mut Context<Self>) {
        let Some(page) = self.page() else { return };
        match act {
            Act::Again => self.run_page(Ran::Chosen, window, cx),
            Act::ExtOutput => {
                let output = format!("```\n{}\n```", page.output.trim_end());
                let title = format!("{}'s Output", page.from.ext.manifest.name);
                self.open_local(title, output, window, cx);
            }
            Act::ExtForget => {
                let ext = page.from.ext.clone();
                ext.forget_passwords();
                crate::shots::note(format!("{}'s Passwords Forgotten", ext.manifest.name), "Set them again in its settings.".into(), None, cx);
            }
            Act::ExtSettings => {
                let (id, section) = (page.from.ext.id.clone(), native_settings(&page.from.ext));
                self.close_keeping_focus(window, cx);
                cx.defer(move |cx| {
                    match section {
                        Some(section) => crate::settings_window::open_at(section, cx),
                        None => crate::settings_window::open_extension(&id, cx),
                    }
                    cx.activate(true);
                });
            }
            _ => {}
        }
    }

    /// Loading, on the page shown: what it says it's doing, and how far.
    pub(super) fn page_loading(&self) -> Option<(Option<String>, Option<f32>)> {
        let page = self.page()?;
        page.loading.clone().or_else(|| page.doing.as_ref().map(|_| (None, None)))
    }

    pub(super) fn page_title(&self) -> Option<String> {
        self.page().map(Page::title)
    }

    pub(super) fn page_placeholder(&self) -> Option<String> {
        let page = self.page()?;
        // (Nothing to look through on a page to read.)
        if self.reading() {
            return Some(String::new());
        }
        Some(page.shown.placeholder.clone().unwrap_or_else(|| format!("Search {}…", page.title())))
    }

    /// What's said in place of the page's list, if it's empty.
    pub(super) fn page_note(&self, typed: bool) -> Option<SharedString> {
        let page = self.page()?;
        if let Some(e) = &self.error {
            return Some(e.clone());
        }
        if !self.items.is_empty() || page.shown.detail.is_some() {
            return None;
        }
        Some(match (&page.loading, typed) {
            (Some((Some(what), _)), _) => what.clone().into(),
            (Some(_), _) => "Loading…".into(),
            (None, true) => "Nothing matches.".into(),
            (None, false) => "Nothing here.".into(),
        })
    }

    /// The page is one to read, not a list.
    pub(super) fn reading(&self) -> bool {
        self.page().is_some_and(|p| p.shown.detail.is_some() && p.shown.items.is_empty())
    }

    /// A row's detail is shown beside the list.
    pub(super) fn page_previews(&self) -> bool {
        self.items.iter().any(|i| matches!(i, Item::Ext(e) if e.row.detail.is_some()))
    }

    /// An extension's icon: its image, an emoji, or a symbol on a tile.
    fn ext_icon(&self, ext: &Extension, name: Option<&str>, tint: Option<&str>, window: &mut Window) -> AnyElement {
        if let Some(name) = name {
            if extensions::is_image(name) {
                let file = if name.starts_with(['/', '~']) { Some(crate::finder::expand(name)).filter(|p| p.is_file()) } else { ext.file(name) };
                if let Some(file) = file {
                    return img(file).size(px(ICON)).rounded(px(4.)).object_fit(ObjectFit::Contain).into_any_element();
                }
            } else if name.chars().next().is_some_and(|c| !c.is_ascii()) {
                return div().text_size(px(16.)).child(name.to_string()).into_any_element();
            }
        }
        match ext.icon(name, tint) {
            Icon::Image(file) => img(file).size(px(ICON)).rounded(px(4.)).object_fit(ObjectFit::Contain).into_any_element(),
            Icon::Tile(symbol, color) => Self::tile(symbol, color, window),
            Icon::Of(_) => div().size(px(ICON)).into_any_element(),
        }
    }

    /// An extension's row: its icon, title and subtitle, its tags, and
    /// what's on its right; asked about, what it asks, in red.
    pub(super) fn render_ext_line(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(Item::Ext(e)) = self.items.get(i).cloned() else { return div().into_any_element() };
        let palette = self.palette;
        let on = i == self.selected;
        let asking = self.confirming == Some(i);
        let red = Self::red(window);
        let narrow = self.mode == Mode::Extension && self.page_previews();
        let icon = self.ext_icon(&e.from.ext, e.row.icon.as_deref(), e.row.color.as_deref(), window);
        let subtitle: SharedString = if asking {
            e.row.actions.get(self.ext_confirm).and_then(|a| a.confirm.clone()).unwrap_or_default().into()
        } else {
            e.row.subtitle.clone().into()
        };
        let tags = e.row.tags.iter().filter(|t| !t.text.is_empty()).map(|t| {
            let color: Hsla = t.color.as_deref().and_then(extensions::color).map_or(palette.secondary, |c| rgb(c).into());
            div().flex_shrink_0().px(px(6.)).py(px(1.)).rounded(px(5.)).bg(color.opacity(0.14)).text_color(color).text_size(px(11.5)).font_weight(FontWeight::MEDIUM).child(t.text.clone())
        });
        let tags: Vec<Div> = if narrow { Vec::new() } else { tags.collect() };
        let accessory = e.row.accessory.clone().unwrap_or_default();
        let row = div()
            .id(("ext", i))
            .h(px(ROW))
            .px(px(10.))
            .rounded(px(8.))
            .flex()
            .items_center()
            .gap(px(10.))
            .when(on, |d| d.bg(if asking { red.opacity(0.14) } else { self.highlight(window) }))
            .child(div().size(px(ICON)).flex_shrink_0().flex().items_center().justify_center().child(icon))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_baseline()
                    .gap_2()
                    .child(if narrow { div().flex_1().min_w_0().truncate().child(e.row.title.clone()) } else { div().flex_shrink_0().max_w(px(380.)).truncate().child(e.row.title.clone()) })
                    .when(!narrow || asking, |d| d.child(div().flex_1().min_w_0().truncate().text_size(px(13.)).text_color(if asking { red } else { palette.secondary }).child(subtitle))),
            )
            .children(tags)
            .when(!accessory.is_empty(), |d| d.child(div().flex_shrink_0().text_size(px(13.)).text_color(palette.secondary).child(accessory)))
            .on_mouse_move(cx.listener(move |this, _, _, cx| {
                if this.selected != i {
                    this.selected = i;
                    this.confirming = None;
                    cx.notify();
                }
            }))
            .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                this.selected = i;
                this.choose(i, e.modifiers().platform, window, cx)
            }));
        div().w_full().px_2().child(row).into_any_element()
    }

    /// The row picked's detail, beside the list.
    pub(super) fn render_ext_preview(&self, _: &mut Window) -> AnyElement {
        let Some(Item::Ext(e)) = self.items.get(self.selected) else { return div().flex_1().into_any_element() };
        match &e.row.detail {
            Some(detail) => self.render_detail(detail, self.selected as u64, LIST + 8.),
            None => div().flex_1().into_any_element(),
        }
    }

    /// A page to read: all of the panel's width.
    pub(super) fn render_reading(&self, _: &mut Window) -> AnyElement {
        match self.page().and_then(|p| p.shown.detail.as_ref()) {
            Some(detail) => div().w_full().flex().child(self.render_detail(detail, 1 << 32 | self.page().map_or(0, |p| p.id), LIST)).into_any_element(),
            None => div().into_any_element(),
        }
    }

    /// A detail: its text (Markdown), then its facts, a label and its text
    /// a line each.
    fn render_detail(&self, detail: &Detail, id: u64, height: f32) -> AnyElement {
        let palette = self.palette;
        let fact = |f: &extensions::Fact| {
            let color: Hsla = f.color.as_deref().and_then(extensions::color).map_or(palette.label, |c| rgb(c).into());
            div().flex().justify_between().gap_3().text_size(px(12.)).child(div().flex_shrink_0().text_color(palette.secondary).child(f.label.clone())).child(div().min_w_0().truncate().text_color(color).child(f.text.clone()))
        };
        div()
            .flex_1()
            .min_w_0()
            .h(px(height))
            .flex()
            .flex_col()
            .child(
                div()
                    .id(ElementId::NamedInteger("ext-detail".into(), id))
                    .flex_1()
                    .min_h_0()
                    .px(px(18.))
                    .py_3()
                    .overflow_y_scroll()
                    .text_size(px(13.))
                    .line_height(px(19.))
                    .text_color(palette.label)
                    .when_some(detail.color.as_deref().and_then(extensions::color), |d, c| {
                        d.child(div().mb_3().h(px(96.)).rounded_lg().bg(rgb(c)).border_1().border_color(palette.separator))
                    })
                    .child(TextView::markdown(ElementId::NamedInteger("ext-detail-text".into(), id), detail.markdown.clone())),
            )
            .when(!detail.metadata.is_empty(), |d| d.child(div().px(px(18.)).py_2().border_t_1().border_color(palette.separator).flex().flex_col().gap_1().children(detail.metadata.iter().map(fact))))
            .into_any_element()
    }
}

/// How well an extension's row matches `q` (lowercase): by its title, its
/// keywords, its subtitle; or, for words, each of them somewhere in it
/// ("has selector").
fn row_score(row: &Row, q: &str) -> Option<f32> {
    let words: Vec<&str> = q.split_whitespace().collect();
    match words.as_slice() {
        [_, _, ..] => row_score_of(row, q).or_else(|| words.iter().map(|w| row_score_of(row, w)).collect::<Option<Vec<f32>>>().map(|s| s.into_iter().fold(f32::MAX, f32::min) - 5.)),
        _ => row_score_of(row, q),
    }
}

fn row_score_of(row: &Row, q: &str) -> Option<f32> {
    let title = crate::apps::name_score(&row.title, q).or_else(|| row.title.to_lowercase().contains(q).then_some(70.));
    let words = row.keywords.iter().filter_map(|w| crate::apps::name_score(w, q)).map(|s| s - 10.).reduce(f32::max);
    let subtitle = row.subtitle.to_lowercase().contains(q).then_some(45.);
    [title, words, subtitle].into_iter().flatten().reduce(f32::max).filter(|s| *s >= 45.)
}

/// How well `q` matches a row by its name (its title or keywords): for
/// Quick Open's own results, among everything else.
fn name_score(row: &Row, q: &str) -> Option<f32> {
    let title = crate::apps::name_score(&row.title, q);
    let words = row.keywords.iter().filter_map(|w| crate::apps::name_score(w, q)).map(|s| s - 10.).reduce(f32::max);
    title.into_iter().chain(words).reduce(f32::max).filter(|s| *s >= 60.)
}

/// A command's run: an extension's script or program, or (a page of F2's
/// own) what its code says.
fn call(ext: &Extension, command: &str, args: &Value, query: &str, cx: &mut App) -> Result<(Option<extensions::Running>, futures::channel::mpsc::UnboundedReceiver<Heard>), String> {
    let Some(native) = ext.native else { return extensions::call(ext, command, args, query).map(|(running, heard)| (Some(running), heard)) };
    let (tell, heard) = futures::channel::mpsc::unbounded();
    native(command, args, query, extensions::Tell(tell), cx);
    Ok((None, heard))
}

/// Where in F2's settings a page of F2's own has its own: Colors', with
/// Pick Color's.
fn native_settings(ext: &Extension) -> Option<&'static str> {
    (ext.id == "f2.colors").then_some("screenshots")
}

/// Quick Open, on a page of F2's own (`ext`'s `command`, on `args`).
pub fn show_native(ext: Arc<Extension>, command: &str, args: Value, cx: &mut App) {
    show_origin(Arc::new(Origin { ext, command: command.to_string(), args }), String::new(), cx);
}

/// A link opened (or a file or folder: a folder in F2, a file in its app).
fn open_target(target: &str, cx: &mut App) {
    if target.contains("://") || target.starts_with("mailto:") {
        return cx.open_url(target);
    }
    let path = crate::finder::expand(target);
    if path.is_dir() && path.extension().is_none_or(|e| e != "app") {
        crate::browser::open(path, None, cx);
        cx.activate(true);
    } else {
        cx.open_with_system(&path);
    }
}

/// Quick Open, on `from`'s page (`typed` in its field).
pub(super) fn show_origin(from: Arc<Origin>, typed: String, cx: &mut App) {
    if cx.try_global::<Up>().and_then(|up| up.0.clone()).is_none() {
        open(None, Mode::Search, cx);
    }
    if let Some((handle, panel)) = cx.try_global::<Up>().and_then(|up| up.0.clone())
        && let Some(panel) = panel.upgrade()
    {
        handle.update(cx, |_, window, cx| panel.update(cx, |this, cx| this.open_page(from, typed, window, cx))).ok();
    }
}

/// Quick Open, on an extension's command's page (its shortcut's, a
/// Recent's).
pub fn show_extension(ext: &str, command: &str, cx: &mut App) {
    let Some(ext) = extensions::by_id(ext) else {
        return crate::shots::note("No Such Extension".into(), format!("There's no \"{ext}\" in {}.", extensions::folder().display()), None, cx);
    };
    show_origin(Arc::new(Origin { ext, command: command.to_string(), args: Value::Null }), String::new(), cx);
}

/// An extension's results in Quick Open's own search, for `q`: its
/// commands, by their titles and keywords; a keyword's command's rows,
/// what's after the keyword looked for in them ("gh f2"); a root
/// command's rows, by what's typed (each with whether it's by its
/// keyword). And those whose manifests are wrong.
pub(super) fn results(q: &str, exts: &[Arc<Extension>], broken: &[extensions::Broken]) -> (Vec<(f32, Command)>, Vec<(String, Vec<Item>, bool)>) {
    let lower = q.trim().to_lowercase();
    let (mut commands, mut sections) = (Vec::new(), Vec::new());
    if lower.is_empty() {
        return (commands, sections);
    }
    for ext in exts {
        // (Its rows under its name; with more than one command's, the
        // command's too.)
        let listed = ext.manifest.commands.iter().filter(|s| s.root || s.keyword.is_some()).count();
        for spec in ext.manifest.commands.iter().filter(|s| !s.hidden) {
            let keyword = spec.keyword.as_deref().map(str::to_lowercase).filter(|k| !k.is_empty());
            let words = format!("{} {} {} extension", ext.manifest.name, ext.manifest.description, keyword.as_deref().unwrap_or("")).to_lowercase();
            let score = if keyword.as_deref() == Some(lower.as_str()) { Some(100.) } else { commands::matches(&spec.title, &words, &lower) };
            let typed_after = keyword.as_deref().and_then(|k| lower.strip_prefix(k)).and_then(|rest| rest.strip_prefix(' ')).map(str::trim);
            if let Some(score) = score.or(typed_after.map(|_| 99.)) {
                commands.push((score, ext.command(spec)));
            }
            // Its rows: after its keyword, those matching what follows (all
            // of them, with nothing after it); a root command's, by what's
            // typed.
            let wanted = match typed_after {
                Some(rest) => Some((rest.to_string(), 8, true)),
                None if spec.root && lower.chars().count() >= 2 => Some((lower.clone(), 5, false)),
                None => None,
            };
            let Some((wanted, most, by_keyword)) = wanted else { continue };
            // (What one that looks things up itself showed was for what was
            // typed then: not to be looked through for this.)
            if spec.search == Search::Script {
                continue;
            }
            let Some(shown) = extensions::cached(&ext.id, &spec.id, &Value::Null) else { continue };
            let from = Arc::new(Origin { ext: ext.clone(), command: spec.id.clone(), args: Value::Null });
            // (By what's typed, among everything else: by their names, not
            // by a word somewhere in their subtitles.)
            let score = |r: &Row| if by_keyword { row_score(r, &wanted) } else { name_score(r, &wanted) };
            let mut rows: Vec<(f32, &Arc<Row>)> = shown.items.iter().filter_map(|r| Some((if wanted.is_empty() { 0. } else { score(r)? }, r))).collect();
            rows.sort_by(|a, b| b.0.total_cmp(&a.0));
            let rows: Vec<Item> = rows.into_iter().take(most).map(|(_, r)| Item::Ext(Arc::new(ExtRow { row: r.clone(), from: from.clone() }))).collect();
            if !rows.is_empty() {
                let heading = if listed > 1 { format!("{}: {}", ext.manifest.name, spec.title) } else { ext.manifest.name.clone() };
                sections.push((heading, rows, by_keyword));
            }
        }
    }
    // Where they go, and what's wrong with those that don't work.
    if let Some(score) = commands::matches("Extensions Folder", "extensions extension folder open plugins", &lower) {
        let command = Command { title: "Extensions Folder".into(), subtitle: crate::APP_NAME.into(), kind: "Command", icon: Icon::Tile("puzzlepiece.extension.fill", commands::FINDER2), run: commands::Run::ExtensionsFolder, confirm: None, key: None };
        commands.push((score, command));
    }
    if let Some(score) = commands::matches("Extension Store", "extensions store browse install get add plugins add-ons addons marketplace", &lower) {
        let command = Command { title: "Extension Store".into(), subtitle: crate::APP_NAME.into(), kind: "Command", icon: Icon::Tile("puzzlepiece.extension.fill", commands::SCREENSHOTS), run: commands::Run::Store, confirm: None, key: None };
        commands.push((score, command));
    }
    if let Some(score) = commands::matches("New Extension", "create make write develop extension plugin", &lower) {
        let command = Command { title: "New Extension".into(), subtitle: crate::APP_NAME.into(), kind: "Command", icon: Icon::Tile("plus", commands::FINDER2), run: commands::Run::NewExtension, confirm: None, key: None };
        commands.push((score, command));
    }
    for b in broken {
        let name = b.dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if commands::matches(&name, "extension extensions", &lower).is_some() {
            let manifest = b.dir.join("f2.json").display().to_string();
            let command = Command { title: format!("{name} Isn't Working"), subtitle: b.why.clone(), kind: "Extension", icon: Icon::Tile("exclamationmark.triangle.fill", 0xff9f0a), run: commands::Run::Quicklink(manifest), confirm: None, key: None };
            commands.push((90., command));
        }
    }
    (commands, sections)
}

#[cfg(test)]
mod tests {
    use super::row_score;
    use crate::extensions::Row;

    #[test]
    fn rows_found() {
        let row = |title: &str, subtitle: &str, keywords: &[&str]| Row { title: title.into(), subtitle: subtitle.into(), keywords: keywords.iter().map(|k| k.to_string()).collect(), ..Default::default() };
        let f2 = row("dagurleo/f2", "F2 for Mac", &["finder"]);
        assert!(row_score(&f2, "f2").is_some());
        assert!(row_score(&f2, "finder").is_some());
        assert!(row_score(&f2, "for mac").is_some());
        assert!(row_score(&f2, "zebra").is_none());
        // Its title over its subtitle.
        let site = row("f2-site", "getf2.com", &[]);
        assert!(row_score(&site, "f2-site").unwrap() > row_score(&f2, "f2 for").unwrap_or(0.));
    }
}
