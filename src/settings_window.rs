//! Settings (⌘,), as Finder's: General, Sidebar, Advanced. Changes apply
//! at once to every window, and are saved.

use std::path::PathBuf;

use futures::StreamExt;

use gpui::prelude::FluentBuilder as _;
use gpui::*;

use gpui_kit::component::input::{Input, InputEvent, InputState, Textarea, TextareaState};

use crate::settings::{CrashReports, QuickScreen, Quicklink, Settings, Snippet};
use crate::style::{Palette, SEMIBOLD, glass, symbol};

mod dictation;
mod extensions;
mod folder_rules;
mod notch;
mod terminal;
mod typing;
pub use extensions::{new_extension, open_extension};
pub use folder_rules::open_for as open_rules_for;

/// The Settings window, if open.
#[derive(Default)]
struct Open(Option<(AnyWindowHandle, WeakEntity<SettingsWindow>)>);

impl Global for Open {}

pub fn open(cx: &mut App) {
    open_on(None, cx);
}

/// Settings, on Quick Open's tab (its shortcut, font and size).
pub fn open_quick_open(cx: &mut App) {
    open_on(Some(Tab::QuickOpen), cx);
}

/// Settings, at a section: "snippets", "quicklinks", "windows",
/// "search", "rules", "screenshots", "dictation", "typing", "notch"
/// (else General).
pub fn open_at(section: &str, cx: &mut App) {
    let tab = match section {
        "snippets" => Tab::Snippets,
        "quicklinks" => Tab::Quicklinks,
        "permissions" => Tab::Permissions,
        "keyboard" => Tab::Keyboard,
        "windows" => Tab::Windows,
        "search" => Tab::Search,
        "rules" => Tab::Rules,
        "screenshots" => Tab::Screenshots,
        "dictation" => Tab::Dictation,
        "typing" => Tab::Typing,
        "terminal" => Tab::Terminal,
        "notch" => Tab::Notch,
        _ => Tab::General,
    };
    open_on(Some(tab), cx);
}

/// Settings' Keyboard, recording a shortcut for `what` (as `keyboard`
/// keeps them: a command's key, "open:<path>"), found in its list.
pub fn open_shortcut(what: String, cx: &mut App) {
    open_on(Some(Tab::Keyboard), cx);
    let Some((handle, view)) = cx.try_global::<Open>().and_then(|o| o.0.clone()) else { return };
    handle
        .update(cx, |_, window, cx| {
            view.update(cx, |this, cx| {
                let path = what.strip_prefix("open:").map(PathBuf::from);
                if let Some(path) = path.filter(|p| !this.adding.contains(p)) {
                    this.adding.push(path);
                }
                let settings = Settings::get(cx);
                let title = crate::keyboard::lines(settings, &this.adding).into_iter().flat_map(|(_, lines)| lines).find(|l| l.what == what).map(|l| l.title);
                this.in_app = false;
                this.search.update(cx, |i, cx| i.set_value(title.unwrap_or_default(), window, cx));
                this.set_recording(Some(Recording::of(&what)), window, cx);
            })
        })
        .ok();
}

fn open_on(tab: Option<Tab>, cx: &mut App) {
    if let Some((handle, view)) = cx.try_global::<Open>().and_then(|o| o.0.clone())
        && crate::raise(handle, cx)
    {
        if let Some(tab) = tab {
            view.update(cx, |this, cx| {
                this.tab = tab;
                cx.notify();
            })
            .ok();
        }
        return;
    }
    let bounds = Bounds::centered(None, size(px(780.), px(620.)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions { title: Some("Settings".into()), ..Default::default() }),
        is_resizable: false,
        ..Default::default()
    };
    let opened = gpui_kit::open_window(options, cx, |window, cx| {
        cx.new(|cx| {
            let mut this = SettingsWindow::new(window, cx);
            this.tab = tab.unwrap_or(Tab::General);
            this
        })
    });
    if let Ok((handle, view)) = opened {
        cx.set_global(Open(Some((handle, view.downgrade()))));
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    General,
    QuickOpen,
    Keyboard,
    Quicklinks,
    Snippets,
    Clipboard,
    Dictation,
    Typing,
    Terminal,
    Notch,
    Windows,
    Screenshots,
    Rules,
    Extensions,
    Sidebar,
    Search,
    Advanced,
    Permissions,
}

/// What a shortcut's recorded for. From any app: Quick Open, its
/// clipboard history, emoji or selection, a screenshot, a window's place,
/// or anything else Quick Open does (Settings' `shortcuts`, by its key: see
/// `keyboard`). In F2's windows: one of its actions (see `keymap`).
#[derive(Clone, PartialEq)]
enum Recording {
    Shot(crate::shots::Action),
    QuickOpen,
    Clipboard,
    Emoji,
    Selection,
    Typing,
    Terminal,
    Window(crate::windows::Place),
    Key(SharedString),
    InApp(&'static str),
}

impl Recording {
    /// What Settings' Keyboard lists `what` by ("clipboard", "shot:Area",
    /// "Lock Screen"…) records into.
    fn of(what: &str) -> Recording {
        use crate::shots::Action;
        use crate::windows::Place;
        match what {
            "quick-open" => Recording::QuickOpen,
            "clipboard" => Recording::Clipboard,
            "emoji" => Recording::Emoji,
            "selection" => Recording::Selection,
            _ => {
                let shot = what.strip_prefix("shot:").and_then(|t| Action::ALL.into_iter().find(|a| a.title() == t));
                let place = what.strip_prefix("window:").and_then(|k| Place::ALL.into_iter().find(|p| p.key() == k));
                shot.map(Recording::Shot).or(place.map(Recording::Window)).unwrap_or_else(|| Recording::Key(what.to_string().into()))
            }
        }
    }

    /// Everything a shortcut can be for, as Settings have them.
    fn all(s: &Settings) -> Vec<Recording> {
        [Recording::QuickOpen, Recording::Clipboard, Recording::Emoji, Recording::Selection, Recording::Typing, Recording::Terminal]
            .into_iter()
            .chain(crate::shots::Action::ALL.into_iter().map(Recording::Shot))
            .chain(crate::windows::Place::ALL.into_iter().map(Recording::Window))
            .chain(s.shortcuts.keys().map(|k| Recording::Key(k.clone().into())))
            .chain(crate::keymap::all().iter().map(|k| Recording::InApp(k.name())))
            .collect()
    }

    fn get(&self, s: &Settings) -> Option<String> {
        match self {
            Recording::Shot(action) => action.shortcut(&s.shots).map(str::to_string),
            Recording::QuickOpen => crate::quick_panel::shortcut(s).map(str::to_string),
            Recording::Clipboard => s.clipboard_shortcut.clone(),
            Recording::Emoji => s.emoji_shortcut.clone(),
            Recording::Selection => s.selection_shortcut.clone(),
            Recording::Typing => s.typing.shortcut.clone(),
            Recording::Terminal => s.quick_terminal.shortcut.clone(),
            Recording::Window(place) => s.window_shortcuts.get(place.key()).cloned(),
            Recording::Key(what) => s.shortcuts.get(what.as_ref()).cloned(),
            Recording::InApp(name) => crate::keymap::keys(name, s).first().cloned(),
        }
    }

    /// All its keys (one of F2's actions may have two: ⌘O and ⌘↓).
    fn keys(&self, s: &Settings) -> Vec<String> {
        match self {
            Recording::InApp(name) => crate::keymap::keys(name, s),
            _ => self.get(s).into_iter().collect(),
        }
    }

    fn set(&self, s: &mut Settings, shortcut: Option<String>) {
        match self {
            Recording::Shot(action) => action.set_shortcut(&mut s.shots, shortcut),
            Recording::QuickOpen => s.quick_open_shortcut = shortcut.unwrap_or_else(|| "off".into()),
            Recording::Clipboard => s.clipboard_shortcut = shortcut,
            Recording::Emoji => s.emoji_shortcut = shortcut,
            Recording::Selection => s.selection_shortcut = shortcut,
            Recording::Typing => s.typing.shortcut = shortcut,
            Recording::Terminal => s.quick_terminal.shortcut = shortcut,
            Recording::Window(place) => match shortcut {
                Some(shortcut) => _ = s.window_shortcuts.insert(place.key().into(), shortcut),
                None => _ = s.window_shortcuts.remove(place.key()),
            },
            Recording::Key(what) => match shortcut {
                Some(shortcut) => _ = s.shortcuts.insert(what.to_string(), shortcut),
                None => _ = s.shortcuts.remove(what.as_ref()),
            },
            Recording::InApp(name) => Self::set_keys(s, name, shortcut.into_iter().collect()),
        }
    }

    /// One of F2's actions' keys (Finder's again: no change kept).
    fn set_keys(s: &mut Settings, name: &'static str, keys: Vec<String>) {
        if keys == crate::keymap::default_keys(name) {
            s.window_keys.remove(name);
        } else {
            s.window_keys.insert(name.to_string(), keys);
        }
    }

    /// Give it `shortcut`, taking that from anything else that had it (in
    /// any app, or in F2's windows: one held in any app would never reach
    /// them).
    fn take(&self, s: &mut Settings, shortcut: Option<String>) {
        if let Some(shortcut) = &shortcut {
            for other in Recording::all(s) {
                let had = other.keys(s);
                if other == *self || !had.iter().any(|k| crate::hotkeys::same(k, shortcut)) {
                    continue;
                }
                match other {
                    Recording::InApp(name) => Self::set_keys(s, name, had.into_iter().filter(|k| !crate::hotkeys::same(k, shortcut)).collect()),
                    _ => other.set(s, None),
                }
            }
        }
        self.set(s, shortcut);
    }
}

struct SettingsWindow {
    focus: FocusHandle,
    tab: Tab,
    palette: Palette,
    /// Whether other apps' "Show in Finder" comes here.
    instead_of_finder: bool,
    /// Disk images open in F2 (see `disk_images`), not DiskImageMounter.
    disk_images_here: bool,
    /// Why changing that didn't work.
    takeover_error: Option<String>,
    /// The shortcut being recorded (the next keys pressed).
    recording: Option<Recording>,
    /// A quicklink or snippet being added (None) or changed (Some: which).
    editing: Option<Editing>,
    /// A folder rule's form, open.
    rule_form: Option<folder_rules::RuleForm>,
    /// Extensions': the one picked, its settings' fields (by extension and
    /// setting), and what's watched in them; New Extension…'s name field
    /// (open), why it couldn't be made, and the one just made.
    ext_selected: Option<String>,
    ext_fields: std::collections::HashMap<String, Entity<InputState>>,
    ext_watch: Vec<Subscription>,
    ext_new: Option<Entity<InputState>>,
    ext_new_error: Option<String>,
    ext_made: Option<String>,
    /// Keyboard's: what its list is narrowed to, whether it lists F2's own
    /// keys (else those in any app), apps and folders being given a
    /// shortcut, and their icons.
    search: Entity<InputState>,
    in_app: bool,
    adding: Vec<PathBuf>,
    icons: crate::icons::Icons,
    /// Terminal's Look section's.
    look: terminal::Look,
    _subscriptions: Vec<Subscription>,
}

/// A quicklink's or a snippet's form, open: its name, keyword, and link
/// or text.
struct Editing {
    snippet: bool,
    index: Option<usize>,
    name: Entity<InputState>,
    keyword: Entity<InputState>,
    link: Entity<InputState>,
    text: Entity<TextareaState>,
    error: Option<&'static str>,
}

impl Tab {
    /// Its name, symbol, and tile's color (as System Settings' sidebar).
    fn about(self) -> (&'static str, &'static str, u32) {
        match self {
            Tab::General => ("General", "gearshape.fill", 0x8e8e93),
            Tab::QuickOpen => ("Quick Open", "magnifyingglass", 0x0a84ff),
            Tab::Keyboard => ("Keyboard", "keyboard.fill", 0x8e8e93),
            Tab::Quicklinks => ("Quicklinks", "link", crate::commands::QUICKLINKS),
            Tab::Snippets => ("Snippets", "text.quote", crate::commands::SNIPPETS),
            Tab::Clipboard => ("Clipboard", "doc.on.clipboard", crate::commands::CLIPBOARD),
            Tab::Dictation => ("Dictation", "mic.fill", crate::dictation::COLOR),
            Tab::Typing => ("Typing", "keyboard", crate::typing::COLOR),
            Tab::Terminal => ("Terminal", "terminal.fill", 0x1d1d1f),
            Tab::Notch => ("Notch", "macbook", 0x1d1d1f),
            Tab::Windows => ("Windows", "macwindow.on.rectangle", crate::commands::WINDOWS),
            Tab::Screenshots => ("Screenshots", "camera.viewfinder", crate::commands::SCREENSHOTS),
            Tab::Rules => ("Folder Rules", "wand.and.stars", folder_rules::COLOR),
            Tab::Extensions => ("Extensions", "puzzlepiece.extension.fill", 0x5e5ce6),
            Tab::Sidebar => ("Sidebar", "sidebar.left", 0x0a84ff),
            Tab::Search => ("Search", "doc.text.magnifyingglass", 0x8e8e93),
            Tab::Advanced => ("Advanced", "gearshape.2.fill", 0x636366),
            Tab::Permissions => ("Permissions", "hand.raised.fill", 0x0a84ff),
        }
    }

    /// Not finished yet: there with Settings' beta features on.
    fn beta(self) -> bool {
        matches!(self, Tab::Typing | Tab::Terminal)
    }

    const ALL: [Tab; 18] = [
        Tab::General,
        Tab::QuickOpen,
        Tab::Keyboard,
        Tab::Quicklinks,
        Tab::Snippets,
        Tab::Clipboard,
        Tab::Dictation,
        Tab::Typing,
        Tab::Terminal,
        Tab::Notch,
        Tab::Windows,
        Tab::Screenshots,
        Tab::Rules,
        Tab::Extensions,
        Tab::Sidebar,
        Tab::Search,
        Tab::Advanced,
        Tab::Permissions,
    ];
}

/// "Beta", beside what isn't finished yet.
fn beta_tag(palette: &Palette) -> Div {
    div().px(px(5.)).rounded(px(4.)).border_1().border_color(palette.glass_border).text_size(px(10.)).text_color(palette.secondary).child("Beta")
}

/// A white symbol on a tile (System Settings' look).
fn tile(name: &'static str, color: u32, window: &Window) -> Div {
    div().size(px(22.)).flex_shrink_0().rounded(px(6.)).bg(rgb(color)).flex().items_center().justify_center().child(symbol(name, 12., SEMIBOLD, white(), window))
}

impl SettingsWindow {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search shortcuts"));
        let focus = cx.focus_handle();
        let subscriptions = vec![
            cx.observe_window_appearance(window, |this, window, cx| {
                this.palette = Palette::for_window(window);
                cx.notify();
            }),
            cx.observe_global::<Settings>(|_, cx| cx.notify()),
            cx.observe(&search, |_, _, cx| cx.notify()),
            cx.subscribe_in(&search, window, |this, _, e: &InputEvent, window, cx| {
                if matches!(e, InputEvent::Focus) {
                    this.set_recording(None, window, cx);
                }
            }),
            // (Clicked elsewhere, or another app in front: no longer
            // recording, and the shortcuts work again.)
            cx.on_focus_out(&focus, window, |this, _, window, cx| this.set_recording(None, window, cx)),
            cx.observe_window_activation(window, |this, window, cx| {
                if !window.is_window_active() {
                    this.set_recording(None, window, cx);
                }
            }),
        ];
        let (icons, mut finished) = crate::icons::Icons::new();
        cx.spawn(async move |this, cx| {
            while let Some(done) = finished.next().await {
                let alive = this.update(cx, |this, cx| {
                    for evicted in this.icons.finish(done) {
                        cx.drop_image(evicted, None);
                    }
                    cx.notify();
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
        // (Search's: how the index is doing, as it changes.)
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(std::time::Duration::from_secs(1)).await;
                let alive = this.update(cx, |this, cx| {
                    if this.tab == Tab::Search {
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
        window.focus(&focus, cx);
        SettingsWindow {
            focus,
            tab: Tab::General,
            palette: Palette::for_window(window),
            instead_of_finder: crate::takeover::is_default(),
            disk_images_here: crate::macos::disk_images_ours(),
            takeover_error: None,
            recording: None,
            editing: None,
            rule_form: None,
            ext_selected: None,
            ext_fields: Default::default(),
            ext_watch: Vec::new(),
            ext_new: None,
            ext_new_error: None,
            ext_made: None,
            search,
            in_app: false,
            adding: Vec::new(),
            icons,
            look: Default::default(),
            _subscriptions: subscriptions,
        }
    }

    /// Open disk images here (or in DiskImageMounter again), from other
    /// apps too.
    fn open_disk_images_here(&mut self, ours: bool, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async move { crate::macos::set_disk_images_ours(ours) }).await;
            let now = cx.background_executor().spawn(async { crate::macos::disk_images_ours() }).await;
            this.update(cx, |this, cx| {
                this.disk_images_here = now;
                this.takeover_error = result.err();
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Make this app (or Finder again) what other apps' "Show in Finder"
    /// opens.
    fn use_instead_of_finder(&mut self, ours: bool, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async move { crate::takeover::set_default(ours) }).await;
            let now = cx.background_executor().spawn(async { crate::takeover::is_default() }).await;
            this.update(cx, |this, cx| {
                this.instead_of_finder = now;
                this.takeover_error = result.err();
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

impl SettingsWindow {
    /// Open the form for a new quicklink or snippet (`index` None), or one
    /// to change.
    fn start_edit(&mut self, snippet: bool, index: Option<usize>, window: &mut Window, cx: &mut Context<Self>) {
        let settings = Settings::get(cx);
        let (name, keyword, body) = match (snippet, index) {
            (false, Some(i)) => settings.quicklinks.get(i).map(|l| (l.name.clone(), l.keyword.clone(), l.link.clone())).unwrap_or_default(),
            (true, Some(i)) => settings.snippets.get(i).map(|s| (s.name.clone(), s.keyword.clone(), s.text.clone())).unwrap_or_default(),
            _ => Default::default(),
        };
        let field = |value: String, placeholder: &'static str, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| InputState::new(window, cx).placeholder(placeholder).default_value(value))
        };
        let name = field(name, if snippet { "Email Signature" } else { "GitHub Search" }, window, cx);
        let keyword = field(keyword, if snippet { ";sig" } else { "gh" }, window, cx);
        let link = field(if snippet { String::new() } else { body.clone() }, "https://github.com/search?q={query}", window, cx);
        let text = cx.new(|cx| TextareaState::new(window, cx).auto_grow(4, 10).placeholder("Best,\n{cursor}").default_value(if snippet { body } else { String::new() }));
        name.update(cx, |i, cx| i.focus(window, cx));
        self.editing = Some(Editing { snippet, index, name, keyword, link, text, error: None });
        cx.notify();
    }

    /// Keep the form's quicklink or snippet (if it's complete), and close it.
    fn save_edit(&mut self, cx: &mut Context<Self>) {
        let Some(e) = &mut self.editing else { return };
        let name = e.name.read(cx).value().trim().to_string();
        let keyword = e.keyword.read(cx).value().trim().to_string();
        let (snippet, index) = (e.snippet, e.index);
        if name.is_empty() {
            e.error = Some("Give it a name.");
            return cx.notify();
        }
        if snippet {
            let text = e.text.read(cx).value().to_string();
            if text.is_empty() {
                e.error = Some("Give it some text.");
                return cx.notify();
            }
            let item = Snippet { name, keyword, text };
            Settings::update(cx, |s| match index.filter(|&i| i < s.snippets.len()) {
                Some(i) => {
                    // (Its shortcut, by its name, follows it.)
                    let was = std::mem::replace(&mut s.snippets[i], item);
                    let now = s.snippets[i].name.clone();
                    crate::keyboard::renamed(s, "snippet", &was.name, Some(&now));
                }
                None => s.snippets.push(item),
            });
        } else {
            let link = e.link.read(cx).value().trim().to_string();
            if link.is_empty() {
                e.error = Some("Give it a link: a web address, or a folder's path.");
                return cx.notify();
            }
            let item = Quicklink { name, keyword, link };
            Settings::update(cx, |s| match index.filter(|&i| i < s.quicklinks.len()) {
                Some(i) => {
                    let was = std::mem::replace(&mut s.quicklinks[i], item);
                    let now = s.quicklinks[i].name.clone();
                    crate::keyboard::renamed(s, "quicklink", &was.name, Some(&now));
                }
                None => s.quicklinks.push(item),
            });
        }
        self.editing = None;
        cx.notify();
    }

    /// The open form, if it's this kind's.
    fn render_form(&self, snippet: bool, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let e = self.editing.as_ref().filter(|e| e.snippet == snippet)?;
        let palette = self.palette;
        let label = |text: &'static str| div().w(px(80.)).flex_shrink_0().text_size(px(12.)).text_color(palette.secondary).child(text);
        let row = |l: &'static str, field: AnyElement| div().flex().items_center().gap_2().child(label(l)).child(div().flex_1().child(field));
        let body: AnyElement = if snippet {
            Textarea::new(&e.text).into_any_element()
        } else {
            Input::new(&e.link).into_any_element()
        };
        Some(
            div()
                .mt_2()
                .p_3()
                .rounded_lg()
                .border_1()
                .border_color(palette.glass_border)
                .bg(palette.sidebar)
                .flex()
                .flex_col()
                .gap_2()
                .child(row("Name", Input::new(&e.name).into_any_element()))
                .child(row("Keyword", Input::new(&e.keyword).into_any_element()))
                .child(div().flex().items_start().gap_2().child(label(if snippet { "Text" } else { "Link" }).mt_1()).child(div().flex_1().child(body)))
                .child(div().pl(px(88.)).text_size(px(11.)).text_color(palette.secondary).child(if snippet {
                    "Typed anywhere, the keyword becomes the text. {clipboard} {date} {time} {datetime} {day} {uuid} are filled in; the cursor goes to {cursor}."
                } else {
                    "A web address or a folder's path (~/Downloads). {query} in it: what's typed after the keyword."
                }))
                .when_some(e.error, |d, error| d.child(div().pl(px(88.)).text_size(px(12.)).text_color(rgb(0xff453a)).child(error)))
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap_2()
                        .child(glass(26., &palette).id("form-cancel").px_3().child("Cancel").on_click(cx.listener(|this, _, _, cx| {
                            this.editing = None;
                            cx.notify();
                        })))
                        .child(
                            div()
                                .id("form-save")
                                .h(px(26.))
                                .px_3()
                                .rounded_md()
                                .bg(palette.accent)
                                .text_color(palette.selected_text)
                                .flex()
                                .items_center()
                                .child("Save")
                                .on_click(cx.listener(|this, _, _, cx| this.save_edit(cx))),
                        ),
                )
                .into_any_element(),
        )
        .map(|form| {
            let _ = window;
            form
        })
    }

    fn render_quicklinks(&self, settings: &Settings, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette;
        let rows: Vec<Stateful<Div>> = settings
            .quicklinks
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let icon = if l.link.contains("://") { tile("link", crate::commands::QUICKLINKS, window) } else { tile("folder.fill", 0x0a84ff, window) };
                self.list_row(("quicklink", i), icon, l.name.clone(), l.keyword.clone(), l.link.clone(), false, i, window, cx)
            })
            .collect();
        div()
            .child(div().mt_1().text_size(px(12.)).text_color(palette.secondary).child(
                "Open a site or a folder from Quick Open, by its name or keyword. With {query} in its link, what's typed after the keyword goes there: “gh rust” searches GitHub for rust.",
            ))
            .child(div().mt_3().flex().flex_col().gap_1().children(rows))
            .children(self.render_form(false, window, cx))
            .when(self.editing.as_ref().is_none_or(|e| e.snippet), |d| {
                d.child(div().mt_3().flex().child(glass(26., &palette).id("add-quicklink").px_3().child("Add Quicklink").on_click(cx.listener(|this, _, window, cx| {
                    this.start_edit(false, None, window, cx)
                }))))
            })
            .into_any_element()
    }

    fn render_snippets(&self, settings: &Settings, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette;
        let trusted = crate::macos::accessibility(false);
        let on = settings.snippet_expansion;
        let rows: Vec<Stateful<Div>> = settings
            .snippets
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let first: String = s.text.lines().next().unwrap_or("").chars().take(60).collect();
                self.list_row(("snippet", i), tile("text.quote", crate::commands::SNIPPETS, window), s.name.clone(), s.keyword.clone(), first, true, i, window, cx)
            })
            .collect();
        div()
            .child(div().mt_1().text_size(px(12.)).text_color(palette.secondary).child(
                "Text to paste: find it by name in Quick Open, or type its keyword in any app and it becomes the text.",
            ))
            .child(
                div()
                    .id("expand")
                    .mt_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .size(px(14.))
                            .rounded_sm()
                            .border_1()
                            .border_color(palette.glass_border)
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(on, |d| d.bg(palette.accent).child(symbol("checkmark", 8., SEMIBOLD, palette.selected_text, window))),
                    )
                    .child("Expand keywords as they're typed, in any app")
                    .on_click(|_, _, cx| Settings::update(cx, |s| s.snippet_expansion = !s.snippet_expansion)),
            )
            .child(div().pl(px(22.)).text_size(px(11.)).text_color(palette.secondary).child(
                "Only the last few characters typed are kept, to match against keywords; password fields aren't heard at all.",
            ))
            .when(on && !trusted, |d| {
                d.child(
                    div()
                        .mt_2()
                        .p_2()
                        .rounded_md()
                        .bg(rgb(0xff9f0a).opacity(0.12))
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(div().flex_1().min_w_0().text_size(px(11.)).child(format!(
                            "Hearing keywords, and pasting, needs Accessibility: allow {} in System Settings > Privacy & Security > Accessibility.",
                            crate::APP_NAME
                        )))
                        .child(glass(24., &palette).id("allow-snippets").px_3().text_size(px(12.)).child("Allow…").on_click(|_, _, cx| {
                            crate::macos::accessibility(true);
                            cx.open_url("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
                        })),
                )
            })
            .child(div().mt_3().flex().flex_col().gap_1().children(rows))
            .children(self.render_form(true, window, cx))
            .when(self.editing.as_ref().is_none_or(|e| !e.snippet), |d| {
                d.child(div().mt_3().flex().child(glass(26., &palette).id("add-snippet").px_3().child("Add Snippet").on_click(cx.listener(|this, _, window, cx| {
                    this.start_edit(true, None, window, cx)
                }))))
            })
            .into_any_element()
    }

    /// A quicklink's or snippet's line: its icon, name, keyword, what it
    /// is; edit and delete.
    #[allow(clippy::too_many_arguments)]
    fn list_row(
        &self,
        id: (&'static str, usize),
        icon: Div,
        name: String,
        keyword: String,
        about: String,
        snippet: bool,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let palette = self.palette;
        let editing = self.editing.as_ref().is_some_and(|e| e.snippet == snippet && e.index == Some(index));
        div()
            .id(id)
            .px_2()
            .py(px(6.))
            .rounded_md()
            .flex()
            .items_center()
            .gap_3()
            .when(editing, |d| d.bg(palette.glass_selected))
            .child(icon)
            .child(div().w(px(150.)).flex_shrink_0().truncate().font_weight(FontWeight::MEDIUM).child(name))
            .child(
                div()
                    .w(px(70.))
                    .flex_shrink_0()
                    .child(div().when(keyword.is_empty(), |d| d.invisible()).px(px(6.)).py(px(1.)).rounded(px(4.)).bg(palette.glass_selected).text_size(px(11.)).font_family(".AppleSystemUIFontMonospaced").truncate().child(keyword)),
            )
            .child(div().flex_1().min_w_0().truncate().text_size(px(12.)).text_color(palette.secondary).child(about))
            .child(
                div()
                    .id((id.0, id.1 + 10_000))
                    .size(px(22.))
                    .rounded_md()
                    .flex()
                    .items_center()
                    .justify_center()
                    .hover(|d| d.bg(palette.glass_selected))
                    .child(symbol("pencil", 11., 0., palette.secondary, window))
                    .on_click(cx.listener(move |this, _, window, cx| this.start_edit(snippet, Some(index), window, cx))),
            )
            .child(
                div()
                    .id((id.0, id.1 + 20_000))
                    .size(px(22.))
                    .rounded_md()
                    .flex()
                    .items_center()
                    .justify_center()
                    .hover(|d| d.bg(palette.glass_selected))
                    .child(symbol("trash", 11., 0., palette.secondary, window))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.editing = None;
                        Settings::update(cx, |s| {
                            if snippet && index < s.snippets.len() {
                                let gone = s.snippets.remove(index);
                                crate::keyboard::renamed(s, "snippet", &gone.name, None);
                            } else if !snippet && index < s.quicklinks.len() {
                                let gone = s.quicklinks.remove(index);
                                crate::keyboard::renamed(s, "quicklink", &gone.name, None);
                            }
                        });
                    })),
            )
    }

    /// Start recording `what`'s shortcut, or stop (None). The shortcuts,
    /// and F2's own keys, are let go meanwhile: pressed, one would be done
    /// (⌘W closing this), not recorded.
    fn set_recording(&mut self, what: Option<Recording>, window: &mut Window, cx: &mut Context<Self>) {
        // (Dictation's shortcut is recorded on its own: one or the other.)
        crate::dictation::record(false, cx);
        let on = what.is_some();
        self.recording = what;
        crate::hotkeys::suspend(on);
        crate::typing::suspend(on);
        crate::keymap::suspend(on, cx);
        if on {
            window.focus(&self.focus, cx);
        }
        cx.notify();
    }

    /// Keys pressed while recording: a shortcut (with ⌘, ⌃ or ⌥) is kept,
    /// ⌫ clears it, escape stops. (F2's own keys may be ones that type
    /// nothing, alone: Return, ⌫, F5. × clears those.)
    fn record(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(what) = self.recording.clone() else { return };
        cx.stop_propagation();
        let in_app = matches!(what, Recording::InApp(_));
        // (⌃⌥⌫ is a shortcut: only ⌫ alone clears, escape alone stops.)
        let m = e.keystroke.modifiers;
        let bare = !(m.platform || m.control || m.alt || m.shift);
        let pressed = if in_app { crate::keymap::from_keystroke(&e.keystroke) } else { crate::hotkeys::from_keystroke(&e.keystroke) };
        let set = match e.keystroke.key.as_str() {
            "escape" if bare => None,
            "backspace" | "delete" if bare && !in_app => Some(None),
            _ => match pressed {
                Some(shortcut) => Some(Some(shortcut)),
                // (Not one: still waiting.)
                None => return,
            },
        };
        if let Some(shortcut) = set {
            // (One shortcut, one thing: taken from any other.)
            Settings::update(cx, |s| what.take(s, shortcut));
        }
        self.set_recording(None, window, cx);
    }

    /// A shortcut's field: click it, then press the keys; × clears it. (One
    /// of F2's actions changed from Finder's: ↺ puts Finder's back.)
    fn recorder(&self, id: usize, what: Recording, settings: &Settings, window: &Window, cx: &mut Context<Self>) -> Div {
        let palette = self.palette;
        let recording = self.recording.as_ref() == Some(&what);
        let keys = what.keys(settings);
        let shortcut = (!keys.is_empty()).then(|| keys.iter().map(|k| crate::hotkeys::label(k)).collect::<Vec<_>>().join("  "));
        let changed = match &what {
            Recording::InApp(name) => settings.window_keys.contains_key(*name),
            _ => false,
        };
        let (on_click, on_clear, on_restore) = (what.clone(), what.clone(), what.clone());
        div()
            .flex()
            .items_center()
            .gap_1()
            .child(
                div()
                    .id(("record", id))
                    .w(px(130.))
                    .h(px(24.))
                    .rounded_md()
                    .border_1()
                    .border_color(if recording { palette.accent } else { palette.glass_border })
                    .bg(palette.glass)
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(12.))
                    .text_color(if shortcut.is_some() || recording { palette.label } else { palette.tertiary })
                    .child(if recording { "Type a shortcut…".to_string() } else { shortcut.clone().unwrap_or_else(|| "Record Shortcut".into()) })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        let next = if this.recording.as_ref() == Some(&on_click) { None } else { Some(on_click.clone()) };
                        this.set_recording(next, window, cx)
                    })),
            )
            .child(
                div()
                    .id(("clear", id))
                    .size(px(20.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(shortcut.is_none(), |d| d.invisible())
                    .child(symbol("xmark.circle.fill", 12., 0., palette.tertiary, window))
                    .on_click(move |_, _, cx| Settings::update(cx, |s| on_clear.set(s, None))),
            )
            .when(matches!(what, Recording::InApp(_)), |d| {
                d.child(
                    div()
                        .id(("restore", id))
                        .size(px(20.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(!changed, |d| d.invisible())
                        .child(symbol("arrow.uturn.backward.circle.fill", 12., 0., palette.tertiary, window))
                        .on_click(move |_, _, cx| {
                            if let Recording::InApp(name) = &on_restore {
                                Settings::update(cx, |s| _ = s.window_keys.remove(*name));
                            }
                        }),
                )
            })
    }

    /// Keyboard: every shortcut, in one list. From any app: Quick Open's,
    /// apps and folders, quicklinks, snippets, every command. Or F2's own
    /// keys, in its windows.
    fn render_keyboard(&mut self, settings: &Settings, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette;
        let query = self.search.read(cx).value().trim().to_lowercase();
        let found = |texts: &[&str]| query.is_empty() || texts.iter().any(|t| t.to_lowercase().contains(&query));
        let heading = |text: String| div().mt_3().mb_1().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(palette.secondary).child(text);
        let row = |title: String, about: Option<String>| {
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(div().truncate().child(title))
                .children(about.filter(|a| !a.is_empty()).map(|a| div().truncate().text_size(px(11.)).text_color(palette.secondary).child(a)))
        };
        let in_app = self.in_app;
        let switch = |id: &'static str, label: &'static str, on: bool| {
            div()
                .id(id)
                .px_3()
                .py(px(3.))
                .rounded(px(5.))
                .text_size(px(12.))
                .when(on, |d| d.bg(palette.glass_selected))
                .child(label)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.in_app = id == "keys-in-app";
                    this.set_recording(None, window, cx);
                }))
        };
        let top = div()
            .mt_1()
            .flex()
            .items_center()
            .gap_3()
            .child(
                div()
                    .flex()
                    .gap(px(2.))
                    .p(px(2.))
                    .rounded_md()
                    .bg(palette.glass)
                    .child(switch("keys-any-app", "In Any App", !in_app))
                    .child(switch("keys-in-app", "In F2's Windows", in_app)),
            )
            .child(
                div()
                    .flex_1()
                    .h(px(26.))
                    .px_2()
                    .rounded_md()
                    .border_1()
                    .border_color(palette.glass_border)
                    .bg(palette.glass)
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(symbol("magnifyingglass", 11., 0., palette.secondary, window))
                    .child(div().flex_1().child(Input::new(&self.search).appearance(false).text_size(px(12.)))),
            );
        let mut list = div().flex().flex_col();
        if in_app {
            let mut section = "";
            for (i, key) in crate::keymap::all().into_iter().enumerate() {
                let what = Recording::InApp(key.name());
                let labels: Vec<String> = what.keys(settings).iter().map(|k| crate::hotkeys::label(k)).collect();
                if !found(&[&key.title, key.section, &labels.join(" ")]) {
                    continue;
                }
                if key.section != section {
                    section = key.section;
                    list = list.child(heading(section.to_string()));
                }
                list = list.child(div().flex().items_center().gap_2().py(px(2.)).child(row(key.title.clone(), None)).child(self.recorder(20_000 + i, what, settings, window, cx)));
            }
            return div()
                .child(top)
                .child(div().mt_2().text_size(px(11.)).text_color(palette.secondary).child(
                    "F2's windows and menus, with Finder's keys. Click one and press new keys; ↺ puts Finder's back. Return, ⌫ and F-keys work alone here.",
                ))
                .child(list)
                .when(!settings.window_keys.is_empty(), |d| {
                    d.child(div().mt_3().flex().child(glass(26., &palette).id("keys-restore").px_3().child("Use Finder's Keys Again").on_click(|_, _, cx| {
                        Settings::update(cx, |s| s.window_keys.clear())
                    })))
                })
                .into_any_element();
        }
        let taken = cx.try_global::<crate::keyboard::Taken>().map(|t| t.0.clone()).unwrap_or_default();
        let icon_px = (22. * window.scale_factor()).round() as u32;
        let mut n = 0;
        for (section, lines) in crate::keyboard::lines(settings, &self.adding) {
            let apps = section == "Apps & Folders";
            let lines: Vec<_> = lines
                .into_iter()
                .filter(|l| {
                    let shortcut = Recording::of(&l.what).get(settings).map(|k| crate::hotkeys::label(&k)).unwrap_or_default();
                    found(&[&l.title, &l.about, section, &shortcut])
                })
                .collect();
            if lines.is_empty() && !(apps && query.is_empty()) {
                continue;
            }
            list = list.child(
                div().flex().items_end().child(div().flex_1().child(heading(section.to_string()))).when(apps, |d| {
                    d.child(glass(22., &palette).id("keys-add-app").mb_1().px_2().text_size(px(11.)).child("Add App or Folder…").on_click(cx.listener(|_, _, window, cx| {
                        let chosen = cx.prompt_for_paths(PathPromptOptions { files: true, directories: true, multiple: true, prompt: Some("Choose".into()) });
                        cx.spawn_in(window, async move |this, cx| {
                            if let Ok(Ok(Some(paths))) = chosen.await {
                                this.update_in(cx, |this, window, cx| {
                                    let first = paths.first().map(|p| format!("open:{}", p.display()));
                                    for path in paths {
                                        if !this.adding.contains(&path) {
                                            this.adding.push(path);
                                        }
                                    }
                                    // (The first, ready for its keys.)
                                    if let Some(first) = first {
                                        this.search.update(cx, |i, cx| i.set_value("", window, cx));
                                        this.set_recording(Some(Recording::Key(first.into())), window, cx);
                                    }
                                })
                                .ok();
                            }
                        })
                        .detach();
                    })))
                }),
            );
            if apps && lines.is_empty() {
                list = list.child(div().text_size(px(11.)).text_color(palette.secondary).child("An app or a folder, opened from anywhere. Pressed again, the app hides."));
            }
            for l in lines {
                let icon = match &l.icon {
                    crate::commands::Icon::Tile(name, color) => tile(name, *color, window).into_any_element(),
                    crate::commands::Icon::Of(path) => match crate::listing::Entry::from_path(path.clone()) {
                        Some(e) => crate::finder::file_image(self.icons.get(&e, icon_px), &e, 22., palette.accent),
                        None => tile("questionmark", 0x8e8e93, window).into_any_element(),
                    },
                    crate::commands::Icon::Image(file) => img(file.clone()).size(px(22.)).rounded(px(5.)).into_any_element(),
                };
                let what = Recording::of(&l.what);
                let held = taken.contains(&l.what) && what.get(settings).is_some();
                list = list.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .py(px(3.))
                        .child(div().size(px(22.)).flex_shrink_0().flex().items_center().justify_center().child(icon))
                        .child(row(l.title, Some(l.about)))
                        .when(held, |d| d.child(div().text_size(px(11.)).text_color(rgb(0xff453a)).child("Another app has it")))
                        .child(self.recorder(10_000 + n, what, settings, window, cx)),
                );
                n += 1;
            }
        }
        div()
            .child(top)
            .child(div().mt_2().text_size(px(11.)).text_color(palette.secondary).child(
                "Shortcuts that work in any app. Click one and press the keys: with ⌘, ⌃ or ⌥ (⌃⌥ is rarely taken), or an F-key. One shortcut does one thing: given to another, it moves.",
            ))
            .child(list)
            .into_any_element()
    }

    fn render_windows(&self, settings: &Settings, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        use crate::windows::Place;
        let palette = self.palette;
        let heading = |text: &'static str| div().mt_3().mb_1().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(palette.secondary).child(text);
        let preset = |id: &'static str, label: &'static str, pick: fn(Place) -> Option<&'static str>| {
            glass(24., &palette).id(id).px_3().text_size(px(12.)).child(label).on_click(move |_, _, cx| {
                Settings::update(cx, |s| {
                    for place in Place::ALL {
                        Recording::Window(place).take(s, pick(place).map(str::to_string));
                    }
                })
            })
        };
        let trusted = crate::macos::accessibility(false);
        let gaps = [0f32, 4., 8., 12.];
        div()
            .when(!trusted, |d| {
                d.child(
                    div()
                        .mt_2()
                        .p_2()
                        .rounded_md()
                        .bg(rgb(0xff9f0a).opacity(0.12))
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(div().flex_1().min_w_0().text_size(px(11.)).child(format!(
                            "Moving other apps' windows needs Accessibility: allow {} in System Settings > Privacy & Security > Accessibility.",
                            crate::APP_NAME
                        )))
                        .child(glass(24., &palette).id("allow-windows").px_3().text_size(px(12.)).child("Allow…").on_click(|_, _, cx| {
                            crate::macos::accessibility(true);
                            cx.open_url("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
                        })),
                )
            })
            .child(heading("Drag a window with keys held:"))
            .child(div().flex().items_center().gap_1().children(crate::settings::DragKeys::ALL.into_iter().enumerate().map(|(i, keys)| {
                div()
                    .id(("drag-keys", i))
                    .px_3()
                    .py(px(3.))
                    .rounded_md()
                    .text_size(px(12.))
                    .when(settings.window_drag == keys, |d| d.bg(palette.glass_selected))
                    .child(keys.label())
                    .on_click(move |_, _, cx| Settings::update(cx, |s| s.window_drag = keys))
            })))
            .child(div().mt_1().text_size(px(11.)).text_color(palette.secondary).child(match settings.window_drag {
                crate::settings::DragKeys::Off => "Hold keys and drag anywhere in a window to move it, or near an edge or a corner to size it.".to_string(),
                keys => format!("Hold {} and drag anywhere in a window to move it, or near an edge or a corner to size it.", keys.label()),
            }))
            .child(heading("Shortcuts, from any app:"))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(preset("preset-rectangle", "Use Rectangle's", Place::rectangle))
                    .child(preset("preset-spectacle", "Use Spectacle's", Place::spectacle))
                    .child(preset("preset-none", "Clear All", |_| None))
                    .child(div().flex_1())
                    .child(div().text_size(px(12.)).text_color(palette.secondary).child("Gap"))
                    .children(gaps.into_iter().enumerate().map(|(i, gap)| {
                        div()
                            .id(("gap", i))
                            .px_2()
                            .py(px(3.))
                            .rounded_md()
                            .text_size(px(12.))
                            .when((settings.window_gap - gap).abs() < 0.5, |d| d.bg(palette.glass_selected))
                            .child(format!("{gap:.0}"))
                            .on_click(move |_, _, cx| Settings::update(cx, |s| s.window_gap = gap))
                    })),
            )
            .child(div().mt_1().mb_1().text_size(px(11.)).text_color(palette.secondary).child(
                "A half pressed again goes to two thirds, then a third. Quick Open finds them all too (\"left half\", \"next display\").",
            ))
            .children(Place::ALL.into_iter().enumerate().map(|(i, place)| {
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .py(px(2.))
                    .child(div().w(px(20.)).flex().justify_center().child(symbol(place.symbol(), 13., 0., palette.label, window)))
                    .child(div().flex_1().child(place.title()))
                    .child(self.recorder(200 + i, Recording::Window(place), settings, window, cx))
            }))
            .into_any_element()
    }

    /// How search's index is doing: what it holds, what it leaves out and
    /// why, and Rebuild.
    fn render_search(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        use crate::listing::{human_size, thousands};
        let palette = self.palette;
        let heading = |text: &'static str| div().mt_4().mb_1().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(palette.secondary).child(text);
        let note = |text: String| div().text_size(px(11.)).text_color(palette.secondary).child(text);
        let search = cx.try_global::<crate::browser::Shared>().and_then(|s| s.search.clone());
        let Some(search) = search else {
            return div()
                .mt_2()
                .child("Search is off: its index couldn't be opened.")
                .child(note("Help > Show Log File says why.".into()))
                .into_any_element();
        };
        let h = search.health();
        let (color, state, detail) = if !h.ready {
            (0xff9f0a, "Getting search ready…".to_string(), format!("{} files and folders listed so far. Searches in a folder work meanwhile.", thousands(h.listed)))
        } else if h.rebuilding {
            (0xff9f0a, "Rebuilding…".to_string(), format!("{} files and folders listed so far. Searches use the index as it was until it's done.", thousands(h.listed)))
        } else {
            let saved = h.saved.and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| format!(", saved {}", crate::clips::ago(d.as_secs()).to_lowercase()));
            (
                0x30d158,
                "Search is ready".to_string(),
                format!("{} files and folders on this Mac, kept up to date as they change ({} index{}).", thousands(h.entries), human_size(h.bytes), saved.unwrap_or_default()),
            )
        };
        let can_rebuild = h.ready && !h.rebuilding && h.owner;
        let rebuild = glass(26., &palette)
            .id("rebuild-index")
            .px_3()
            .flex_shrink_0()
            .child("Rebuild Index")
            .when(!can_rebuild, |d| d.opacity(0.4))
            .when(can_rebuild, |d| {
                d.on_click(cx.listener(|_, _, _, cx| {
                    if let Some(search) = cx.try_global::<crate::browser::Shared>().and_then(|s| s.search.clone()) {
                        search.rebuild();
                    }
                    cx.notify();
                }))
            });
        // (By what macOS calls them, home's as ~.)
        let home = home();
        let named = |p: &PathBuf| -> String {
            let rest = p.strip_prefix(&home).ok().map(|r| r.to_string_lossy().into_owned());
            match rest.as_deref() {
                Some(r @ ("Desktop" | "Documents" | "Downloads")) => r.into(),
                Some(".Trash") => crate::macos::bin().into(),
                Some("Library/Mobile Documents") => "iCloud Drive".into(),
                Some("Library/Containers") => "Apps' own data (Library/Containers)".into(),
                Some("Library/Group Containers") => "Apps' shared data (Library/Group Containers)".into(),
                Some("Library/CloudStorage") => "Dropbox, Google Drive and OneDrive (Library/CloudStorage)".into(),
                Some("Pictures/Photos Library.photoslibrary") => "Your Photos library".into(),
                Some("Music/Music") => "Your Music library".into(),
                Some("Movies/TV") => "Your TV library".into(),
                Some(r) => format!("~/{r}"),
                None => p.to_string_lossy().into_owned(),
            }
        };
        let row = |text: String| div().flex().items_center().gap_2().py(px(1.)).child(symbol("folder.fill", 11., 0., palette.secondary, window)).child(text);
        let left_out: Vec<String> = h.left_out.iter().filter(|p| !p.starts_with("/Volumes")).map(named).collect();
        const SHOWN: usize = 8;
        // (Yours by name; apps' data in your Library, and macOS's own,
        // counted. Not while they're collected again.)
        let collected = h.ready && !h.rebuilding;
        let (yours, system): (Vec<&PathBuf>, Vec<&PathBuf>) = h.refused.iter().filter(|_| collected).partition(|p| p.starts_with(&home));
        let (apps, yours): (Vec<&PathBuf>, Vec<&PathBuf>) = yours.into_iter().partition(|p| p.starts_with(home.join("Library")));
        let refused = yours.len();
        div()
            .child(
                div()
                    .mt_2()
                    .p_3()
                    .rounded_lg()
                    .border_1()
                    .border_color(palette.glass_border)
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(div().size(px(9.)).flex_shrink_0().rounded_full().bg(rgb(color)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(div().font_weight(FontWeight::SEMIBOLD).child(state))
                            .child(note(detail))
                            .when(!h.owner, |d| d.child(note(format!(
                                "Another app keeps this index up to date (fsearch, or another copy of {0}), and {0} reads it: rebuild it there.",
                                crate::APP_NAME
                            )))),
                    )
                    .child(rebuild),
            )
            .child(div().mt_1().child(note(
                "If a search misses something that's there, Rebuild Index lists the whole disk again, in the background (a minute or two).".into(),
            )))
            .child(heading("Left out:"))
            .when(!h.full_disk_access, |d| {
                d.child(note(format!(
                    "Without Full Disk Access, macOS asks before an app opens each of these, so {} doesn't keep them in the index (a search in one you've opened still looks through it):",
                    crate::APP_NAME
                )))
                .child(div().mt_1().children(left_out.iter().cloned().map(row)))
                .child(
                    div()
                        .mt_2()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(glass(24., &palette).id("search-fda").px_3().text_size(px(12.)).child("Turn On Full Disk Access…").on_click(|_, _, cx| {
                            crate::permissions::Permission::FullDisk.ask(cx)
                        }))
                        .child(note(format!("It takes effect when {} next opens.", crate::APP_NAME))),
                )
            })
            .when(refused > 0, |d| {
                d.child(div().mt_2().child(note("macOS kept these closed even so:".into())))
                    .child(div().mt_1().children(yours.iter().take(SHOWN).map(|p| row(named(p)))))
                    .when(refused > SHOWN, |d| d.child(note(format!("and {} more", refused - SHOWN))))
            })
            .when(!apps.is_empty(), |d| {
                d.child(div().mt_2().child(row(format!("{} folders of apps' data in your Library", thousands(apps.len())))))
                    .child(div().pl(px(19.)).child(note("Mail's, Messages' and Safari's, say: Full Disk Access opens most of them.".into())))
            })
            .when(!system.is_empty(), |d| {
                d.child(div().mt_2().child(row(format!("{} of macOS's own folders", thousands(system.len())))))
                    .child(div().pl(px(19.)).child(note("System caches and databases only macOS may open.".into())))
            })
            .child(div().mt_2().child(row("Other disks".into())))
            .child(div().pl(px(19.)).child(note(
                "USB drives, disk images and network shares aren't in the index: a search in one looks through it there and then (slower on a big disk).".into(),
            )))
            .when(collected && h.full_disk_access && refused == 0 && apps.is_empty(), |d| d.child(div().mt_2().child(note("Everything else on this Mac's disk is in.".into()))))
            .child(heading("Text inside files:"))
            .child(note(format!(
                "{} text and code files in your home folder, for grep: searches: up to 1 MB each (10 MB in a git repository), not Library, caches, apps or folders like node_modules and .git.{}",
                thousands(h.text_files),
                if h.text_pending > 0 { " Some are still being read." } else { "" }
            )))
            .child(div().mt_1().child(note("Searching a window's contents (\"Contents\" in its search bar) asks Spotlight.".into())))
            .into_any_element()
    }

    fn render_clipboard(&self, settings: &Settings, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette;
        let heading = |text: &'static str| div().mt_3().mb_1().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(palette.secondary).child(text);
        let check = |id: &'static str, label: SharedString, on: bool, f: fn(&mut Settings), window: &Window| {
            div()
                .id(id)
                .flex()
                .items_center()
                .gap_2()
                .py(px(3.))
                .child(
                    div()
                        .size(px(14.))
                        .rounded_sm()
                        .border_1()
                        .border_color(palette.glass_border)
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(on, |d| d.bg(palette.accent).child(symbol("checkmark", 8., SEMIBOLD, palette.selected_text, window))),
                )
                .child(label)
                .on_click(move |_, _, cx| Settings::update(cx, f))
        };
        let pill = |id: (&'static str, usize), label: String, on: bool| {
            div().id(id).px_3().py(px(3.)).rounded_md().text_size(px(12.)).when(on, |d| d.bg(palette.glass_selected)).child(label)
        };
        let blocked = crate::clips::blocked();
        let trusted = crate::macos::accessibility(false);
        let row = |label: &'static str, about: &'static str, field: Div| {
            div()
                .flex()
                .items_center()
                .gap_2()
                .py(px(3.))
                .child(div().flex_1().flex().flex_col().child(label).child(div().text_size(px(11.)).text_color(palette.secondary).child(about)))
                .child(field)
        };
        div()
            .child(heading("Clipboard History:"))
            .child(check(
                "clip-history",
                "Keep a history of what's copied (not passwords: their apps mark them)".into(),
                settings.clipboard_history,
                |s| s.clipboard_history = !s.clipboard_history,
                window,
            ))
            .child(
                div()
                    .pl(px(22.))
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(div().mr_1().text_size(px(12.)).text_color(palette.secondary).child("Keep the last"))
                    .children([50usize, 200, 1000].into_iter().enumerate().map(|(i, n)| {
                        pill(("keep", i), n.to_string(), settings.clipboard_keep == n).on_click(move |_, _, cx| Settings::update(cx, |s| s.clipboard_keep = n))
                    }))
                    .child(div().flex_1())
                    .child(glass(24., &palette).id("clear-clips").px_3().text_size(px(12.)).child("Clear History").on_click(|_, _, _| crate::clips::clear())),
            )
            .when_some(blocked, |d, why| {
                d.child(
                    div()
                        .mt_2()
                        .p_2()
                        .rounded_md()
                        .bg(rgb(0xff9f0a).opacity(0.12))
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(div().flex_1().min_w_0().text_size(px(11.)).child(format!("{why} Allow it in System Settings > Privacy & Security > Paste from Other Apps.")))
                        .child(glass(24., &palette).id("paste-privacy").px_3().text_size(px(12.)).child("Open").on_click(|_, _, cx| {
                            cx.open_url("x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension")
                        })),
                )
            })
            .child(heading("Return on a clip or an emoji:"))
            .child(
                div()
                    .flex()
                    .gap_1()
                    .child(pill(("paste", 0), "Pastes it into the app in front".into(), settings.paste_on_return).on_click(|_, _, cx| Settings::update(cx, |s| s.paste_on_return = true)))
                    .child(pill(("paste", 1), "Only copies it".into(), !settings.paste_on_return).on_click(|_, _, cx| Settings::update(cx, |s| s.paste_on_return = false))),
            )
            .when(settings.paste_on_return && !trusted, |d| {
                d.child(
                    div()
                        .mt_2()
                        .p_2()
                        .rounded_md()
                        .bg(rgb(0xff9f0a).opacity(0.12))
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(div().flex_1().min_w_0().text_size(px(11.)).child(format!(
                            "Pasting needs Accessibility: allow {} in System Settings > Privacy & Security > Accessibility. Until then, Return copies.",
                            crate::APP_NAME
                        )))
                        .child(glass(24., &palette).id("allow-paste").px_3().text_size(px(12.)).child("Allow…").on_click(|_, _, cx| {
                            crate::macos::accessibility(true);
                            cx.open_url("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
                        })),
                )
            })
            .child(div().mt_1().text_size(px(11.)).text_color(palette.secondary).child("⌘Return copies, whichever this is."))
            .child(heading("Shortcuts, from any app:"))
            .child(row("Clipboard History", "Quick Open's list of what's been copied", self.recorder(100, Recording::Clipboard, settings, window, cx)))
            .child(row("Emoji & Symbols", "Quick Open's emoji, or type : and a name there", self.recorder(101, Recording::Emoji, settings, window, cx)))
            .into_any_element()
    }

    fn render_screenshots(&self, settings: &Settings, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        use crate::shots::Action;
        let palette = self.palette;
        let shots = settings.shots.clone();
        let heading = |text: &'static str| div().mt_3().mb_1().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(palette.secondary).child(text);
        let taken = cx.try_global::<crate::shots::Taken>().map(|t| t.0.clone()).unwrap_or_default();
        let held = crate::shots::held_by_macos(&shots);
        let rows: Vec<Div> = Action::ALL
            .into_iter()
            .enumerate()
            .map(|(i, action)| {
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .py(px(3.))
                    .child(div().w(px(20.)).flex().justify_center().child(symbol(action.symbol(), 13., 0., palette.label, window)))
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .child(action.title())
                            .child(div().text_size(px(11.)).text_color(palette.secondary).child(action.about()))
                            .when(taken.contains(&action), |d| d.child(div().text_size(px(11.)).text_color(rgb(0xff453a)).child("Another app uses this shortcut."))),
                    )
                    .child(self.recorder(i, Recording::Shot(action), settings, window, cx))
            })
            .collect();
        let check = |id: &'static str, label: SharedString, on: bool, f: fn(&mut crate::settings::Shots), window: &Window| {
            div()
                .id(id)
                .flex()
                .items_center()
                .gap_2()
                .py(px(3.))
                .child(
                    div()
                        .size(px(14.))
                        .rounded_sm()
                        .border_1()
                        .border_color(palette.glass_border)
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(on, |d| d.bg(palette.accent).child(symbol("checkmark", 8., SEMIBOLD, palette.selected_text, window))),
                )
                .child(label)
                .on_click(move |_, _, cx| Settings::update(cx, |s| f(&mut s.shots)))
        };
        let home = home();
        let folder = shots.folder.clone().unwrap_or_else(|| home.join("Desktop"));
        let folders = [("Desktop", home.join("Desktop")), ("Pictures", home.join("Pictures")), ("Downloads", home.join("Downloads"))];
        div()
            .child(heading("Shortcuts, from any app:"))
            .children(rows)
            .when(!held.is_empty(), |d| {
                d.child(
                    div()
                        .mt_2()
                        .p_2()
                        .rounded_md()
                        .bg(rgb(0xff9f0a).opacity(0.12))
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(div().flex_1().min_w_0().text_size(px(11.)).child(format!(
                            "macOS's own screenshots still take {}. Turn them off in System Settings > Keyboard > Keyboard Shortcuts > Screenshots.",
                            held.join(" and ")
                        )))
                        .child(glass(24., &palette).id("keyboard-shortcuts").px_3().text_size(px(12.)).child("Open").on_click(|_, _, cx| {
                            cx.open_url("x-apple.systempreferences:com.apple.Keyboard-Settings.extension")
                        })),
                )
            })
            .child(heading("After capturing:"))
            .child(check("shot-copy", "Copy it to the clipboard".into(), shots.copy, |s| s.copy = !s.copy, window))
            .child(check("shot-preview", "Show it in the corner (drag it into an app, or edit it)".into(), shots.preview, |s| s.preview = !s.preview, window))
            .child(check("shot-save", "Save it to a folder".into(), shots.save, |s| s.save = !s.save, window))
            .child(
                div().pl(px(22.)).flex().items_center().gap_1().children(folders.into_iter().map(|(name, path)| {
                    let on = folder == path;
                    div()
                        .id(name)
                        .px_3()
                        .py(px(3.))
                        .rounded_md()
                        .text_size(px(12.))
                        .when(on, |d| d.bg(palette.glass_selected))
                        .child(name)
                        .on_click(move |_, _, cx| {
                            let path = path.clone();
                            Settings::update(cx, |s| s.shots.folder = Some(path))
                        })
                })),
            )
            .child(div().pl(px(22.)).text_size(px(11.)).text_color(palette.secondary).child("Save (in the corner or the editor) saves there too."))
            .child(check("shot-sound", "Play the camera sound".into(), shots.sound, |s| s.sound = !s.sound, window))
            .child(heading("Screen recordings:"))
            .child(check("record-audio", "Record the microphone too".into(), shots.record_audio, |s| s.record_audio = !s.record_audio, window))
            .child(check("record-clicks", "Show clicks".into(), shots.record_clicks, |s| s.record_clicks = !s.record_clicks, window))
            .child(div().pl(px(22.)).text_size(px(11.)).text_color(palette.secondary).child(
                "Saved as MP4 in the folder above (the Desktop, else). Stop one from the menu bar (●), Quick Open, or its shortcut again.",
            ))
            .child(check(
                "tidy-shots",
                "Keep the Desktop tidy: move screenshots on it into a Screenshots folder".into(),
                shots.tidy_desktop,
                |s| s.tidy_desktop = !s.tidy_desktop,
                window,
            ))
            .child(check("shot-shadow", "Keep a window's shadow".into(), shots.shadow, |s| s.shadow = !s.shadow, window))
            .child(heading("Pick Color copies a color as:"))
            .child(div().flex().flex_wrap().items_center().gap_1().children(crate::colors::FORMATS.into_iter().map(|(id, name)| {
                div()
                    .id(id)
                    .px_3()
                    .py(px(3.))
                    .rounded_md()
                    .text_size(px(12.))
                    .when(shots.color_format == id, |d| d.bg(palette.glass_selected))
                    .child(name)
                    .on_click(move |_, _, cx| Settings::update(cx, |s| s.shots.color_format = id.to_string()))
            })))
            .child(div().text_size(px(11.)).text_color(palette.secondary).child(
                "Colors, in Quick Open, keeps those picked, to copy any way they're written, with their contrast on white and black.",
            ))
            .into_any_element()
    }
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map_or_else(|| PathBuf::from("/"), PathBuf::from)
}

impl Render for SettingsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette;
        let settings = Settings::get(cx).clone();
        let check = |id: &'static str, label: SharedString, on: bool, f: fn(&mut Settings), window: &Window| {
            div()
                .id(id)
                .flex()
                .items_center()
                .gap_2()
                .py(px(3.))
                .child(
                    div()
                        .size(px(14.))
                        .rounded_sm()
                        .border_1()
                        .border_color(palette.glass_border)
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(on, |d| d.bg(palette.accent).child(symbol("checkmark", 8., SEMIBOLD, palette.selected_text, window))),
                )
                .child(label)
                .on_click(move |_, _, cx| Settings::update(cx, f))
        };
        let choice = |id: &'static str, label: &'static str, on: bool, f: Box<dyn Fn(&mut Settings)>| {
            div()
                .id(id)
                .px_3()
                .py(px(4.))
                .rounded_md()
                .when(on, |d| d.bg(palette.glass_selected))
                .child(label)
                .on_click(move |_, _, cx| Settings::update(cx, |s| f(s)))
        };
        let heading = |text: &'static str| div().mt_3().mb_1().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(palette.secondary).child(text);
        let new_window = settings.new_window_folder.clone();
        let body = match self.tab {
            Tab::General => {
                let home = home();
                let is = |p: Option<PathBuf>| new_window == p;
                div()
                    .child(heading("New windows show:"))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_1()
                            .child(choice("nw-home", "Home", is(None) || is(Some(home.clone())), Box::new(|s| s.new_window_folder = None)))
                            .child(choice("nw-desktop", "Desktop", is(Some(home.join("Desktop"))), Box::new(|s| {
                                s.new_window_folder = Some(home_dir("Desktop"))
                            })))
                            .child(choice("nw-documents", "Documents", is(Some(home.join("Documents"))), Box::new(|s| {
                                s.new_window_folder = Some(home_dir("Documents"))
                            })))
                            .child(choice("nw-downloads", "Downloads", is(Some(home.join("Downloads"))), Box::new(|s| {
                                s.new_window_folder = Some(home_dir("Downloads"))
                            })))
                            .child(choice("nw-mac", "Macintosh HD", is(Some("/".into())), Box::new(|s| s.new_window_folder = Some("/".into()))))
                            // (Another folder, chosen: its name; click to choose again.)
                            .child({
                                let custom = new_window.clone().filter(|p| ![home.clone(), home.join("Desktop"), home.join("Documents"), home.join("Downloads"), PathBuf::from("/")].contains(p));
                                let label = custom.as_ref().and_then(|f| f.file_name()).map_or_else(
                                    || "Other…".to_string(),
                                    |n| {
                                        let name = n.to_string_lossy();
                                        let short: String = name.chars().take(24).collect();
                                        format!("{short}{}…", if short.len() < name.len() { "…" } else { "" }).replace("……", "…")
                                    },
                                );
                                div()
                                    .id("nw-other")
                                    .px_3()
                                    .py(px(4.))
                                    .rounded_md()
                                    .when(custom.is_some(), |d| d.bg(palette.glass_selected))
                                    .child(label)
                                    .on_click(|_, _, cx| {
                                        let chosen = cx.prompt_for_paths(PathPromptOptions { files: false, directories: true, multiple: false, prompt: Some("Open New Windows Here".into()) });
                                        cx.spawn(async move |cx| {
                                            if let Ok(Ok(Some(paths))) = chosen.await
                                                && let Some(folder) = paths.into_iter().next()
                                            {
                                                cx.update(|cx| Settings::update(cx, |s| s.new_window_folder = Some(folder)));
                                            }
                                        })
                                        .detach();
                                    })
                            }),
                    )
                    .when_some(new_window.clone().filter(|p| ![home.clone(), home.join("Desktop"), home.join("Documents"), home.join("Downloads"), PathBuf::from("/")].contains(p)), |d, folder| {
                        d.child(div().mt_1().text_size(px(11.)).text_color(palette.secondary).child(folder.display().to_string()))
                    })
                    .child(div().mt_2().child(check(
                        "window-at-launch",
                        format!("Open a window when {} starts", crate::APP_NAME).into(),
                        settings.open_window_at_launch,
                        |s| s.open_window_at_launch = !s.open_window_at_launch,
                        window,
                    )))
                    .child(div().pl(px(22.)).text_size(px(11.)).text_color(palette.secondary).child(format!(
                        "Off, {0} starts in the menu bar: its Dock icon, or New {0} Window in its menu, opens one.",
                        crate::APP_NAME
                    )))
                    .child(heading("Typing in a window:"))
                    .child(
                        div()
                            .flex()
                            .gap_1()
                            .child(choice("type-select", "Selects items by name", !settings.typing_searches, Box::new(|s| s.typing_searches = false)))
                            .child(choice("type-search", "Starts a search", settings.typing_searches, Box::new(|s| s.typing_searches = true))),
                    )
                    .child(heading("Open in:"))
                    .children([(crate::tools::Kind::Terminal, "Terminal", "⌥⌘T"), (crate::tools::Kind::Editor, "Editor", "⌥⌘E")].map(|(kind, label, key)| {
                        let using = crate::tools::chosen(kind, &settings).map(|t| t.id);
                        let installed = crate::tools::installed(kind);
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(div().w(px(70.)).text_size(px(12.)).text_color(palette.secondary).child(label))
                            .when(installed.is_empty(), |d| d.child(div().text_size(px(12.)).text_color(palette.secondary).child("None installed")))
                            .children(installed.iter().map(|tool| {
                                let id = tool.id;
                                choice(id, tool.name.as_str(), using == Some(id), Box::new(move |s| match kind {
                                    crate::tools::Kind::Terminal => s.terminal = Some(id.to_string()),
                                    crate::tools::Kind::Editor => s.editor = Some(id.to_string()),
                                }))
                            }))
                            .child(div().flex_1())
                            .child(div().text_size(px(11.)).text_color(palette.tertiary).child(key))
                    }))
                    .child(heading("Instead of Finder:"))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(div().flex_1().text_size(px(12.)).text_color(palette.secondary).child(format!(
                                "Other apps' “Show in Finder” opens {}.",
                                if self.instead_of_finder { crate::APP_NAME } else { "Finder" }
                            )))
                            .child(
                                glass(26., &palette)
                                    .id("viewer")
                                    .px_3()
                                    .child(if self.instead_of_finder { "Use Finder Again".to_string() } else { format!("Use {}", crate::APP_NAME) })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let ours = !this.instead_of_finder;
                                        this.use_instead_of_finder(ours, cx)
                                    })),
                            ),
                    )
                    // (macOS's own leaves an image's window to Finder: with
                    // Finder quit, nothing shows.)
                    .child(
                        div()
                            .mt_2()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(div().flex_1().min_w_0().text_size(px(12.)).text_color(palette.secondary).child(if self.disk_images_here {
                                format!("Disk images (.dmg) open in {}, laid out as their makers set them, even with Finder quit.", crate::APP_NAME)
                            } else {
                                "Disk images (.dmg) from other apps open in macOS's DiskImageMounter, which needs Finder to show them.".to_string()
                            }))
                            .child(
                                glass(26., &palette)
                                    .id("disk-images")
                                    .px_3()
                                    .child(if self.disk_images_here { "Use macOS's Again".to_string() } else { format!("Use {}", crate::APP_NAME) })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let ours = !this.disk_images_here;
                                        this.open_disk_images_here(ours, cx)
                                    })),
                            ),
                    )
                    .when_some(self.takeover_error.clone(), |d, e| {
                        d.child(div().mt_1().text_size(px(12.)).text_color(rgb(0xff453a)).child(format!("That didn't work: {e}")))
                    })
                    .child(div().mt_2().child(check(
                        "take-finder",
                        format!("Also show windows Finder opens in {} (experimental)", crate::APP_NAME).into(),
                        settings.take_finder_windows,
                        |s| s.take_finder_windows = !s.take_finder_windows,
                        window,
                    )))
                    .child(div().pl(px(22.)).text_size(px(11.)).text_color(palette.secondary).child(format!(
                        "macOS doesn't let Finder or its Dock icon be removed, and opens folders other apps \
                         ask for in Finder. With this on, a window Finder opens (from its Dock icon, or a \
                         folder another app opens) closes and opens here instead: Finder's window shows for a \
                         moment first, and macOS asks once to let {0} control Finder. The desktop and \
                         Finder's AirDrop window stay Finder's.",
                        crate::APP_NAME
                    )))
                    .into_any_element()
            }
            Tab::QuickOpen => {
                let (family, text_size) = crate::quick_panel::font(&settings);
                div()
                    .child(heading("Shortcut, from any app:"))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .children(crate::quick_panel::SHORTCUTS.iter().map(|&(name, label)| {
                                choice(name, label, settings.quick_open_shortcut == name, Box::new(move |s| s.quick_open_shortcut = name.to_string()))
                            })),
                    )
                    // (Or any, recorded: taken from whatever else of F2's had it.)
                    .child(
                        div()
                            .mt_2()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().text_size(px(12.)).text_color(palette.secondary).child("Or your own:"))
                            .child(self.recorder(900, Recording::QuickOpen, &settings, window, cx)),
                    )
                    // (Quick Open with what's selected in the app in front.)
                    .child(
                        div()
                            .mt_2()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().text_size(px(12.)).text_color(palette.secondary).child("With selected text:"))
                            .child(self.recorder(901, Recording::Selection, &settings, window, cx))
                            .child(div().text_size(px(11.)).text_color(palette.secondary).child("translate, define, search or count what's selected, in any app")),
                    )
                    .when(cx.try_global::<crate::quick_panel::ShortcutTaken>().is_some_and(|t| t.0), |d| {
                        d.child(div().mt_1().text_size(px(12.)).text_color(rgb(0xff453a)).child("Another app uses that shortcut: pick another."))
                    })
                    .child(div().mt_1().text_size(px(11.)).text_color(palette.secondary).child(
                        "⌘Space is Spotlight's until it's turned off in System Settings > Keyboard > Keyboard Shortcuts > Spotlight.",
                    ))
                    .child(heading("Opens on:"))
                    .child(div().flex().flex_wrap().gap_1().children(
                        [
                            ("screen-pointer", "The screen with the pointer", QuickScreen::Pointer),
                            ("screen-window", "The active window's screen", QuickScreen::ActiveWindow),
                            ("screen-main", "The main screen", QuickScreen::Main),
                        ]
                        .map(|(id, label, screen)| choice(id, label, settings.quick_open_screen == screen, Box::new(move |s| s.quick_open_screen = screen))),
                    ))
                    .child(
                        div()
                            .mt_1()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(div().flex_1().min_w_0().text_size(px(11.)).text_color(palette.secondary).child(
                                "Drag it by its bottom bar, or by its search field while it's empty, to put it anywhere: it snaps to the middle.",
                            ))
                            .when(settings.quick_open_position.is_some(), |d| {
                                d.child(
                                    glass(24., &palette)
                                        .id("qo-center")
                                        .px_3()
                                        .text_size(px(12.))
                                        .child("Center It Again")
                                        .on_click(|_, _, cx| Settings::update(cx, |s| s.quick_open_position = None)),
                                )
                            }),
                    )
                    .child(heading("Results:"))
                    .child(check(
                        "qo-simple",
                        "Simple mode: only the search field until you type".into(),
                        settings.quick_open_simple,
                        |s| s.quick_open_simple = !s.quick_open_simple,
                        window,
                    ))
                    .child(div().pl(px(22.)).text_size(px(11.)).text_color(palette.secondary).child(
                        "Recent, Favorites, the commands and the apps aren't listed with nothing typed.",
                    ))
                    .child(div().mt_2().child(check(
                        "qo-files",
                        "Find files and folders as you type".into(),
                        settings.quick_open_files,
                        |s| s.quick_open_files = !s.quick_open_files,
                        window,
                    )))
                    .child(div().pl(px(22.)).text_size(px(11.)).text_color(palette.secondary).child(
                        "When it's off, type f and a space first (“f budget”), or choose Search Files under the results. \
                         “f budget” finds only files and folders either way.",
                    ))
                    .child(heading("Font:"))
                    .child(
                        // (Each in itself.)
                        div().flex().flex_wrap().gap_1().children(crate::quick_panel::fonts(cx).iter().map(|&(name, label, family)| {
                            choice(name, label, settings.quick_open_font == name, Box::new(move |s| s.quick_open_font = name.to_string()))
                                .font_family(family)
                                .text_size(px(14.))
                        })),
                    )
                    .child(heading("Search text size:"))
                    .child(div().flex().gap_1().children(crate::quick_panel::TEXT_SIZES.iter().map(|&size| {
                        let on = (settings.quick_open_text_size - size).abs() < 0.5;
                        div()
                            .id(("text-size", size as usize))
                            .px_3()
                            .py(px(4.))
                            .rounded_md()
                            .when(on, |d| d.bg(palette.glass_selected))
                            .child(format!("{size:.0} pt"))
                            .on_click(move |_, _, cx| Settings::update(cx, |s| s.quick_open_text_size = size))
                    })))
                    .child(heading("Looks like:"))
                    .child(
                        div()
                            .mt_1()
                            .rounded(px(12.))
                            .border_1()
                            .border_color(palette.glass_border)
                            .bg(palette.sidebar)
                            .overflow_hidden()
                            .font_family(family)
                            .child(
                                div()
                                    .h(px(text_size + 34.))
                                    .px(px(18.))
                                    .flex()
                                    .items_center()
                                    .border_b_1()
                                    .border_color(palette.separator)
                                    .text_size(px(text_size))
                                    .line_height(px((text_size * 1.3).round()))
                                    .text_color(crate::quick_panel::placeholder_color(&palette))
                                    .child(crate::quick_panel::PLACEHOLDER),
                            )
                            .child(
                                div()
                                    .h(px(40.))
                                    .mx_2()
                                    .my_1()
                                    .px(px(10.))
                                    .rounded(px(8.))
                                    .bg(palette.label.opacity(0.07))
                                    .flex()
                                    .items_center()
                                    .gap(px(10.))
                                    .text_size(px(14.))
                                    .child(div().size(px(20.)).rounded(px(5.)).bg(rgb(crate::commands::FINDER2)).flex().items_center().justify_center().child(symbol(
                                        "chart.pie.fill",
                                        11.,
                                        SEMIBOLD,
                                        white(),
                                        window,
                                    )))
                                    .child("Disk Usage")
                                    .child(div().text_size(px(13.)).text_color(palette.secondary).child(crate::APP_NAME))
                                    .child(div().flex_1())
                                    .child(div().text_size(px(13.)).text_color(palette.secondary).child("Command")),
                            ),
                    )
                    .into_any_element()
            }
            Tab::Screenshots => self.render_screenshots(&settings, window, cx),
            Tab::Rules => self.render_rules(&settings, window, cx),
            Tab::Extensions => self.render_extensions(&settings, window, cx),
            Tab::Keyboard => self.render_keyboard(&settings, window, cx),
            Tab::Clipboard => self.render_clipboard(&settings, window, cx),
            Tab::Dictation => self.render_dictation(&settings, window, cx),
            Tab::Typing => self.render_typing(&settings, window, cx),
            Tab::Terminal => self.render_terminal(&settings, window, cx),
            Tab::Notch => self.render_notch(&settings, window),
            Tab::Quicklinks => self.render_quicklinks(&settings, window, cx),
            Tab::Permissions => div()
                .child(div().mt_1().mb_3().text_size(px(12.)).text_color(palette.secondary).child(
                    "What F2 asks macOS for, and why. This updates as they're allowed.",
                ))
                .child(div().flex().flex_col().gap_2().children(crate::permissions::rows(&palette, window)))
                .child(div().mt_4().flex().child(glass(26., &palette).id("welcome").px_3().child("Show the Welcome Again").on_click(|_, _, cx| crate::onboarding::open(cx))))
                .into_any_element(),
            Tab::Snippets => self.render_snippets(&settings, window, cx),
            Tab::Windows => self.render_windows(&settings, window, cx),
            Tab::Search => self.render_search(window, cx),
            Tab::Sidebar => {
                let sb = settings.sidebar.clone();
                div()
                    .child(heading("Show these items in the sidebar:"))
                    .child(check("sb-recents", "Recents".into(), sb.recents, |s| s.sidebar.recents = !s.sidebar.recents, window))
                    .child(check("sb-airdrop", "AirDrop".into(), sb.airdrop, |s| s.sidebar.airdrop = !s.sidebar.airdrop, window))
                    .child(check("sb-icloud", "iCloud Drive".into(), sb.icloud, |s| s.sidebar.icloud = !s.sidebar.icloud, window))
                    .child(check("sb-disks", "External disks".into(), sb.external_disks, |s| s.sidebar.external_disks = !s.sidebar.external_disks, window))
                    .child(check("sb-cloud", "Cloud storage".into(), sb.cloud_storage, |s| s.sidebar.cloud_storage = !s.sidebar.cloud_storage, window))
                    .child(check("sb-network", "Network".into(), sb.network, |s| s.sidebar.network = !s.sidebar.network, window))
                    .child(check("sb-servers", "Servers".into(), sb.servers, |s| s.sidebar.servers = !s.sidebar.servers, window))
                    .child(check("sb-tags", "Tags".into(), sb.tags, |s| s.sidebar.tags = !s.sidebar.tags, window))
                    .child(check("sb-trash", crate::macos::bin().into(), sb.trash, |s| s.sidebar.trash = !s.sidebar.trash, window))
                    .child(div().mt_3().child(glass(26., &palette).id("fav-defaults").px_3().child("Restore Default Favorites").on_click(|_, _, cx| {
                        Settings::update(cx, |s| s.favorites = None)
                    })))
                    .into_any_element()
            }
            Tab::Advanced => div()
                .child(heading("Show:"))
                .child(check("exts", "Show all filename extensions".into(), settings.show_all_extensions, |s| s.show_all_extensions = !s.show_all_extensions, window))
                .child(check("warn-ext", "Show warning before changing an extension".into(), settings.warn_extension_change, |s| {
                    s.warn_extension_change = !s.warn_extension_change
                }, window))
                .child(check("warn-trash", format!("Show warning before emptying the {}", crate::macos::bin()).into(), settings.warn_empty_trash, |s| {
                    s.warn_empty_trash = !s.warn_empty_trash
                }, window))
                .child(heading("Sounds:"))
                .child(check("drop-sound", "Play a sound when files are dropped".into(), settings.drop_sound, |s| s.drop_sound = !s.drop_sound, window))
                .child(
                    div()
                        .pl(px(24.))
                        .text_size(px(11.))
                        .text_color(palette.secondary)
                        .child(format!("The {} makes Finder's sounds. All of them follow “Play user interface sound effects” in System Settings › Sound.", crate::macos::bin())),
                )
                .child(heading("When F2 quits unexpectedly:"))
                .child(
                    div()
                        .flex()
                        .gap_1()
                        .child(choice("crash-ask", "Ask to Send a Report", settings.crash_reports == CrashReports::Ask, Box::new(|s| s.crash_reports = CrashReports::Ask)))
                        .child(choice("crash-send", "Send a Report", settings.crash_reports == CrashReports::Send, Box::new(|s| s.crash_reports = CrashReports::Send)))
                        .child(choice("crash-never", "Don't Send", settings.crash_reports == CrashReports::Never, Box::new(|s| s.crash_reports = CrashReports::Never))),
                )
                .child(
                    div()
                        .pl(px(2.))
                        .text_size(px(11.))
                        .text_color(palette.secondary)
                        .child("A report says where in F2 it stopped, F2's and macOS's versions and your Mac's model, and goes to getf2.com only (getf2.com/privacy says more)."),
                )
                .child(heading("Keep folders on top:"))
                .child(check("folders-top", "In windows when sorting by name".into(), settings.folders_on_top, |s| s.folders_on_top = !s.folders_on_top, window))
                .child(check("sizes", "Calculate all sizes".into(), settings.calculate_sizes, |s| s.calculate_sizes = !s.calculate_sizes, window))
                .child(heading("When performing a search:"))
                .child(
                    div()
                        .flex()
                        .gap_1()
                        .child(choice("scope-mac", "Search This Mac", settings.search_this_mac, Box::new(|s| s.search_this_mac = true)))
                        .child(choice("scope-folder", "Search the Current Folder", !settings.search_this_mac, Box::new(|s| s.search_this_mac = false))),
                )
                .child(heading("Drawing:"))
                .child(check("lean-drawing", "Use less memory (two frames per window, not three)".into(), settings.lean_drawing, |s| {
                    s.lean_drawing = !s.lean_drawing
                }, window))
                .child(
                    div()
                        .pl(px(24.))
                        .text_size(px(11.))
                        .text_color(palette.secondary)
                        .child("About 15 MB less for each big window. If scrolling stutters, turn it off."),
                )
                .child(heading("Beta:"))
                .child(check("beta", "Turn on beta features".into(), settings.beta, |s| s.beta = !s.beta, window))
                .child(
                    div()
                        .pl(px(24.))
                        .text_size(px(11.))
                        .text_color(palette.secondary)
                        .child("What isn't finished yet, to try: Typing (Icelandic, Korean and English on ABC, without switching), and a terminal that comes down from the notch. Each has its own place in Settings, marked Beta. They may change, or go."),
                )
                .into_any_element(),
        };
        div()
            .key_context("Utility")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| this.record(e, window, cx)))
            .size_full()
            .bg(palette.background)
            .text_color(palette.label)
            .text_size(px(13.))
            .flex()
            // Sections down the side, as System Settings has them.
            .child(
                div()
                    .w(px(200.))
                    .h_full()
                    .flex_shrink_0()
                    .pt_3()
                    .px_2()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .bg(palette.sidebar)
                    .border_r_1()
                    .border_color(palette.sidebar_border)
                    // (Extensions only once they're on.)
                    // (Dictation only where it works: macOS 26. What's in
                    // beta only with beta features on.)
                    .children(Tab::ALL.into_iter().filter(|t| (*t != Tab::Extensions || settings.extensions_on()) && (*t != Tab::Dictation || crate::dictation::available()) && (!t.beta() || settings.beta)).map(|t| {
                        let (label, name, color) = t.about();
                        div()
                            .id(("section", t as usize))
                            .h(px(30.))
                            .px_2()
                            .rounded_md()
                            .flex()
                            .items_center()
                            .gap_2()
                            .when(self.tab == t, |d| d.bg(palette.glass_selected))
                            .child(tile(name, color, window))
                            .child(label)
                            .when(t.beta(), |d| d.child(beta_tag(&palette)))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.tab = t;
                                this.editing = None;
                                this.rule_form = None;
                                cx.notify();
                            }))
                    })),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .px_6()
                            .pt_4()
                            .pb_1()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().text_size(px(20.)).font_weight(FontWeight::SEMIBOLD).child(self.tab.about().0))
                            .when(self.tab.beta(), |d| d.child(beta_tag(&palette))),
                    )
                    // (A long section scrolls.)
                    .child(div().id("body").flex_1().min_h_0().overflow_y_scroll().px_6().pb_4().child(body)),
            )
    }
}

fn home_dir(name: &str) -> PathBuf {
    home().join(name)
}
