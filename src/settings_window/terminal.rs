//! Settings › Terminal (src/terminal.rs): the terminal that comes down from
//! the notch on or off, its shortcut, whether a click elsewhere puts it
//! away, where a new shell starts, and how it looks: a theme it comes with
//! or one of yours (another terminal's, imported, or one made from a theme
//! and changed here), its font and size, its cursor, how see-through it is
//! and how big it comes down.

use std::sync::Arc;

use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_kit::component::color_picker::{ColorPicker, ColorPickerEvent, ColorPickerState};
use gpui_kit::component::input::{Input, InputEvent, InputState};

use super::{Recording, SettingsWindow};
use crate::macos;
use crate::settings::{QuickTerminal, Settings, TerminalCursor, TerminalLook, TerminalStart, TerminalTheme};
use crate::style::{Palette, SEMIBOLD, glass, symbol};
use crate::terminal::import::{self, Source};
use crate::terminal::theme::{self, Colors};

/// The Look section's own: its themes' search, the theme of yours being
/// changed, what the last import said, and (looked for once) the
/// monospaced fonts there are and the terminals to import from.
#[derive(Default)]
pub(super) struct Look {
    search: Option<Entity<InputState>>,
    editor: Option<Editor>,
    said: Option<String>,
    fonts: Option<Vec<String>>,
    sources: Option<Vec<Source>>,
    scroll: UniformListScrollHandle,
    /// The theme the list was last scrolled to (chosen then).
    shown: Option<String>,
    _watch: Vec<Subscription>,
}

/// A theme of yours, being changed: its name (as saved), its name's
/// field, and a picker for each of its colors.
struct Editor {
    theme: String,
    name: Entity<InputState>,
    pickers: Vec<(Slot, Entity<ColorPickerState>)>,
    _watch: Vec<Subscription>,
}

/// One of a theme's colors.
#[derive(Clone, Copy, PartialEq)]
enum Slot {
    Background,
    Foreground,
    Cursor,
    CursorText,
    Selection,
    SelectedText,
    Color(usize),
}

const COLOR_NAMES: [&str; 8] = ["Black", "Red", "Green", "Yellow", "Blue", "Magenta", "Cyan", "White"];

impl Slot {
    fn all() -> impl Iterator<Item = Slot> {
        [Slot::Background, Slot::Foreground, Slot::Cursor, Slot::CursorText, Slot::Selection, Slot::SelectedText].into_iter().chain((0..16).map(Slot::Color))
    }

    fn label(self) -> &'static str {
        match self {
            Slot::Background => "Background",
            Slot::Foreground => "Text",
            Slot::Cursor => "Cursor",
            Slot::CursorText => "Under the Cursor",
            Slot::Selection => "Selection",
            Slot::SelectedText => "Selected Text",
            Slot::Color(i) => COLOR_NAMES[i % 8],
        }
    }

    /// Its color as it's drawn (where the theme has none, what's drawn
    /// instead).
    fn of(self, c: &Colors) -> u32 {
        match self {
            Slot::Background => c.background,
            Slot::Foreground => c.foreground,
            Slot::Cursor => c.cursor.unwrap_or(c.foreground),
            Slot::CursorText => c.cursor_text.unwrap_or(c.background),
            Slot::Selection => c.selection.unwrap_or(theme::dim(c.foreground)),
            Slot::SelectedText => c.selected_text.unwrap_or(c.foreground),
            Slot::Color(i) => c.palette[i],
        }
    }

    fn set(self, t: &mut TerminalTheme, hex: String) {
        match self {
            Slot::Background => t.background = hex,
            Slot::Foreground => t.foreground = hex,
            Slot::Cursor => t.cursor = Some(hex),
            Slot::CursorText => t.cursor_text = Some(hex),
            Slot::Selection => t.selection = Some(hex),
            Slot::SelectedText => t.selected_text = Some(hex),
            Slot::Color(i) => {
                if let Some(c) = t.palette.get_mut(i) {
                    *c = hex;
                }
            }
        }
    }
}

fn hex_of(color: Hsla) -> String {
    let c = Rgba::from(color);
    let byte = |v: f32| (v.clamp(0., 1.) * 255.).round() as u32;
    theme::to_hex(byte(c.r) << 16 | byte(c.g) << 8 | byte(c.b))
}

/// `name`, if no theme has it (yours, or one F2 comes with); else "name 2",
/// "name 3"…
fn free_name(name: &str, own: &[TerminalTheme]) -> String {
    let taken = |n: &str| own.iter().any(|t| t.name == n) || theme::bundled().iter().any(|(b, _)| b == n);
    if !taken(name) {
        return name.to_string();
    }
    (2..).map(|i| format!("{name} {i}")).find(|n| !taken(n)).unwrap_or_default()
}

impl SettingsWindow {
    /// Another terminal's look taken (see `import`), and what was taken said.
    fn import_look(&mut self, source: Source, cx: &mut Context<Self>) {
        let said = match source.import() {
            Ok(imported) => {
                let mut said = String::new();
                Settings::update(cx, |s| said = import::apply(source, imported, &mut s.quick_terminal));
                said
            }
            Err(why) => why,
        };
        self.look.editor = None;
        self.look.said = Some(said);
        cx.notify();
    }

    /// A theme it comes with, made yours ("My Carbonfox"), chosen, to change.
    fn make_own(&mut self, from: &str, window: &mut Window, cx: &mut Context<Self>) {
        let s = &Settings::get(cx).quick_terminal;
        let Some(colors) = theme::colors(from, &s.themes) else { return };
        let name = free_name(&format!("My {from}"), &s.themes);
        let theme = colors.named(&name);
        Settings::update(cx, |s| {
            s.quick_terminal.themes.push(theme);
            s.quick_terminal.look.theme = name.clone();
        });
        self.edit_theme(name, window, cx);
    }

    /// One of your themes opened to change: each color as you change it.
    fn edit_theme(&mut self, name: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(colors) = Settings::get(cx).quick_terminal.themes.iter().find(|t| t.name == name).and_then(Colors::of) else { return };
        let field = cx.new(|cx| InputState::new(window, cx).default_value(name.clone()));
        let mut watch = Vec::new();
        let pickers = Slot::all()
            .map(|slot| {
                let picker = cx.new(|cx| ColorPickerState::new(window, cx).default_value(rgb(slot.of(&colors))));
                watch.push(cx.subscribe(&picker, move |this, _, e: &ColorPickerEvent, cx| {
                    let ColorPickerEvent::Change(Some(color)) = e else { return };
                    let Some(editing) = this.look.editor.as_ref().map(|e| e.theme.clone()) else { return };
                    let hex = hex_of(*color);
                    Settings::update(cx, |s| {
                        if let Some(t) = s.quick_terminal.themes.iter_mut().find(|t| t.name == editing) {
                            slot.set(t, hex);
                        }
                    });
                }));
                (slot, picker)
            })
            .collect();
        self.look.editor = Some(Editor { theme: name, name: field, pickers, _watch: watch });
        self.look.said = None;
        cx.notify();
    }

    /// Done changing: renamed, if its name was changed.
    fn done_editing(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.look.editor.take() else { return };
        let wanted = editor.name.read(cx).value().trim().to_string();
        if !wanted.is_empty() && wanted != editor.theme {
            let others: Vec<TerminalTheme> = Settings::get(cx).quick_terminal.themes.iter().filter(|t| t.name != editor.theme).cloned().collect();
            let name = free_name(&wanted, &others);
            Settings::update(cx, |s| {
                let q = &mut s.quick_terminal;
                if let Some(t) = q.themes.iter_mut().find(|t| t.name == editor.theme) {
                    t.name = name.clone();
                }
                if q.look.theme == editor.theme {
                    q.look.theme = name;
                }
            });
        }
        cx.notify();
    }

    fn delete_theme(&mut self, name: String, cx: &mut Context<Self>) {
        if self.look.editor.as_ref().is_some_and(|e| e.theme == name) {
            self.look.editor = None;
        }
        Settings::update(cx, |s| {
            let q = &mut s.quick_terminal;
            q.themes.retain(|t| t.name != name);
            if q.look.theme == name {
                q.look.theme = TerminalLook::default().theme;
            }
        });
    }

    pub(super) fn render_terminal(&mut self, settings: &Settings, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette;
        let t = settings.quick_terminal.clone();
        let heading = |text: &'static str| div().mt_3().mb_1().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(palette.secondary).child(text);
        let note = |text: String| div().text_size(px(11.)).text_color(palette.secondary).child(text);
        let check = |id: &'static str, label: &'static str, on: bool, enabled: bool, f: fn(&mut Settings)| {
            div()
                .id(id)
                .flex()
                .items_center()
                .gap_2()
                .py(px(3.))
                .when(!enabled, |d| d.opacity(0.45))
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
                .when(enabled, |d| d.on_click(move |_, _, cx| Settings::update(cx, f)))
        };
        let choice = |id: ElementId, label: String, on: bool, f: Box<dyn Fn(&mut Settings)>| {
            div()
                .id(id)
                .px_3()
                .py(px(3.))
                .rounded_md()
                .text_size(px(12.))
                .when(on, |d| d.bg(palette.glass_selected))
                .child(label)
                .on_click(move |_, _, cx| Settings::update(cx, |s| f(s)))
        };
        let folder = t.folder.as_ref().map(|f| crate::finder::items::location(f).to_string());
        div()
            .child(heading("Terminal:"))
            .child(check("terminal-on", "Bring a terminal down from the notch", t.on, true, |s| s.quick_terminal.on = !s.quick_terminal.on))
            .child(div().pl(px(22.)).child(note(
                "Your shell. It goes on while the terminal's away: bring it back down and it's as you left it.".into(),
            )))
            .child(heading("Down and back up:"))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .py(px(3.))
                    .when(!t.on, |d| d.opacity(0.45))
                    .child(div().flex_1().flex().flex_col().child("Shortcut").child(note("From any app. The notch's black grows out into the terminal.".into())))
                    .child(self.recorder(31_000, Recording::Terminal, settings, window, cx)),
            )
            .child(check("terminal-away", "Put it away when you click elsewhere", t.hide_on_click_away, t.on, |s| {
                s.quick_terminal.hide_on_click_away = !s.quick_terminal.hide_on_click_away
            }))
            .child(heading("A new shell starts in:"))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .when(!t.on, |d| d.opacity(0.45))
                    .child(choice("terminal-home".into(), "Your Home Folder".into(), t.start == TerminalStart::Home, Box::new(|s| s.quick_terminal.start = TerminalStart::Home)))
                    .child(choice(
                        "terminal-window".into(),
                        format!("{}'s Front Window's Folder", crate::APP_NAME),
                        t.start == TerminalStart::Window,
                        Box::new(|s| s.quick_terminal.start = TerminalStart::Window),
                    ))
                    .child(
                        div()
                            .id("terminal-folder")
                            .px_3()
                            .py(px(3.))
                            .rounded_md()
                            .text_size(px(12.))
                            .when(t.start == TerminalStart::Folder, |d| d.bg(palette.glass_selected))
                            .child(folder.clone().map_or("A Folder…".to_string(), |f| format!("{f}…")))
                            .on_click(cx.listener(|_, _, window, cx| {
                                let chosen = cx.prompt_for_paths(PathPromptOptions { files: false, directories: true, multiple: false, prompt: Some("Choose".into()) });
                                cx.spawn_in(window, async move |_, cx| {
                                    if let Ok(Ok(Some(paths))) = chosen.await
                                        && let Some(path) = paths.into_iter().next()
                                    {
                                        cx.update(|_, cx| {
                                            Settings::update(cx, |s| {
                                                s.quick_terminal.folder = Some(path);
                                                s.quick_terminal.start = TerminalStart::Folder;
                                            })
                                        })
                                        .ok();
                                    }
                                })
                                .detach();
                            })),
                    ),
            )
            .child(div().mt_1().child(note(format!(
                "Without a window up, {}'s front window's folder is your home folder. File › Open Here in the Notch's Terminal takes it to a window's folder at any time.",
                crate::APP_NAME
            ))))
            .child(self.render_look(&t, window, cx))
            .child(heading("Good to know:"))
            .child(div().flex().flex_col().gap_1().children([
                note("Drag to select, ⌘C to copy, ⌘V to paste, ⌘A for all. Scroll to see what's gone by; ⌘F finds in it; ⌘K clears it. ⇧↩ types a new line (in Claude Code's prompt).".into()),
                note("⌘-click a link, or a file or folder a program printed (src/main.rs:42): links open in your browser, folders in F2, files in their app.".into()),
                note("Programs that ask for the mouse get it (vim, htop, tmux); hold ⇧ to select anyway.".into()),
                note("Drop files on it to type their paths. ⌘O opens the shell's folder in F2.".into()),
                note("⌘T for a new tab (in the same folder), ⌘W to close one, ⌘1… or ⌘⇧[ ⌘⇧] to go between them: they're beside the notch. ⌘+ and ⌘− for bigger or smaller text, ⌘0 back.".into()),
                note("exit ends the shell (its tab); the next time, a new one starts. Right-click for all this in a menu.".into()),
                note("Typing (Icelandic, Korean) treats it as Terminal: English, unless you switch by hand.".into()),
            ]))
            .when(t.on && !settings.beta, |d| d.child(div().mt_3().child(note("Beta features are off (Settings › Advanced): it's not there.".into()))))
            .into_any_element()
    }

    /// Look: a preview, importing, the themes, yours changed, and the
    /// font, cursor, background and size.
    fn render_look(&mut self, t: &QuickTerminal, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette;
        let look = t.look.clone();
        let heading = |text: &'static str| div().mt_3().mb_1().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(palette.secondary).child(text);
        let note = |text: String| div().text_size(px(11.)).text_color(palette.secondary).child(text);
        let label = |text: &'static str| div().w(px(92.)).flex_none().text_size(px(12.)).child(text);
        let button = |id: ElementId, text: String| glass(24., &palette).id(id).px_3().text_size(px(12.)).child(text);
        let chip = |id: ElementId, text: String, on: bool| div().id(id).px_3().py(px(3.)).rounded_md().text_size(px(12.)).when(on, |d| d.bg(palette.glass_selected)).child(text);
        let colors = theme::chosen(t);
        let family = crate::terminal::font_family(look.font_family.as_deref());
        let sources = self.look.sources.get_or_insert_with(|| Source::ALL.into_iter().filter(|s| s.found()).collect()).clone();
        // (Looked for once, off the main thread; the menu has them once
        // they're found.)
        if self.look.fonts.is_none() {
            self.look.fonts = Some(Vec::new());
            cx.spawn(async move |this, cx| {
                let fonts = cx.background_executor().spawn(async { macos::monospace_fonts() }).await;
                this.update(cx, |this, cx| {
                    this.look.fonts = Some(fonts);
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
        let fonts = self.look.fonts.clone().unwrap_or_default();
        if self.look.search.is_none() {
            let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search themes"));
            self.look._watch.push(cx.observe(&search, |_, _, cx| cx.notify()));
            self.look._watch.push(cx.subscribe_in(&search, window, |this, _, e: &InputEvent, window, cx| {
                if matches!(e, InputEvent::Focus) {
                    this.set_recording(None, window, cx);
                }
            }));
            self.look.search = Some(search);
        }
        let search = self.look.search.clone().expect("made above");
        let query = search.read(cx).value().trim().to_lowercase();
        let yours = t.themes.iter().any(|own| own.name == look.theme);

        // The themes: yours first, then those it comes with; as searched.
        let listed: Vec<(SharedString, Colors, bool)> = t
            .themes
            .iter()
            .filter_map(|own| Some((SharedString::from(own.name.clone()), Colors::of(own)?, true)))
            .chain(theme::bundled().iter().map(|(name, c)| (SharedString::from(name.clone()), *c, false)))
            .filter(|(name, _, _)| query.is_empty() || name.to_lowercase().contains(&query))
            .collect();
        // (Scrolled to the chosen theme: to the middle, the first time;
        // else into sight, if it's been chosen since.)
        if self.look.shown.as_deref() != Some(&look.theme)
            && let Some(at) = listed.iter().position(|(name, _, _)| *name == look.theme)
        {
            let strategy = if self.look.shown.is_none() { ScrollStrategy::Center } else { ScrollStrategy::Nearest };
            self.look.scroll.scroll_to_item(at, strategy);
            self.look.shown = Some(look.theme.clone());
        }
        let listed = Arc::new(listed);
        let chosen = SharedString::from(look.theme.clone());
        let list = {
            let listed = listed.clone();
            uniform_list(
                "terminal-themes",
                listed.len(),
                cx.processor(move |this, range: std::ops::Range<usize>, _, _| {
                    let palette = this.palette;
                    range
                        .map(|i| {
                            let (name, c, own) = listed[i].clone();
                            let on = name == chosen;
                            div()
                                .id(("terminal-theme", i))
                                .h(px(30.))
                                .px_2()
                                .flex()
                                .items_center()
                                .gap_2()
                                .rounded_md()
                                .when(on, |d| d.bg(palette.glass_selected))
                                .child(swatch(&c, &palette))
                                .child(div().flex_1().text_size(px(12.)).truncate().child(name.clone()))
                                .when(own, |d| d.child(div().text_size(px(11.)).text_color(palette.secondary).child("Yours")))
                                .on_click(move |_, _, cx| {
                                    let name = name.to_string();
                                    Settings::update(cx, |s| s.quick_terminal.look.theme = name)
                                })
                                .into_any_element()
                        })
                        .collect()
                }),
            )
            .track_scroll(&self.look.scroll)
            .h(px(212.))
        };

        let import_row = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .child(label("Import from"))
            .children(sources.iter().map(|&source| {
                button(("terminal-import", source as usize).into(), source.name().to_string()).on_click(cx.listener(move |this, _, _, cx| this.import_look(source, cx)))
            }));

        let theme_actions = div()
            .flex()
            .gap_2()
            .mt_2()
            .when(!yours, |d| {
                let from = look.theme.clone();
                d.child(button("terminal-make-own".into(), format!("Make Your Own from {from}…")).on_click(cx.listener(move |this, _, window, cx| this.make_own(&from, window, cx))))
            })
            .when(yours && self.look.editor.is_none(), |d| {
                let name = look.theme.clone();
                d.child(button("terminal-edit".into(), "Change Its Colors…".into()).on_click(cx.listener(move |this, _, window, cx| this.edit_theme(name.clone(), window, cx))))
            })
            .when(yours, |d| {
                let name = look.theme.clone();
                d.child(button("terminal-delete".into(), "Delete".into()).on_click(cx.listener(move |this, _, _, cx| this.delete_theme(name.clone(), cx))))
            });

        let editor = self.look.editor.as_ref().map(|editor| {
            let picker = |slot: Slot| {
                let state = editor.pickers.iter().find(|(s, _)| *s == slot).map(|(_, p)| p.clone());
                div().flex().items_center().gap_2().w(px(150.)).py(px(2.)).children(state.map(|p| ColorPicker::new(&p))).child(div().text_size(px(12.)).child(slot.label()))
            };
            let bank = |from: usize| div().flex().flex_wrap().children((from..from + 8).map(|i| picker(Slot::Color(i)).w(px(112.))));
            div()
                .mt_2()
                .p_3()
                .rounded_lg()
                .border_1()
                .border_color(palette.glass_border)
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(label("Name"))
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
                                .child(div().flex_1().child(Input::new(&editor.name).appearance(false).text_size(px(12.)))),
                        )
                        .child(button("terminal-done".into(), "Done".into()).on_click(cx.listener(|this, _, _, cx| this.done_editing(cx)))),
                )
                .child(preview(&colors, &look, family.clone(), &palette))
                .child(div().flex().flex_wrap().children([Slot::Background, Slot::Foreground, Slot::Cursor, Slot::CursorText, Slot::Selection, Slot::SelectedText].map(picker)))
                .child(note("The 16 colors programs choose from:".into()))
                .child(bank(0))
                .child(note("Bright:".into()))
                .child(bank(8))
        });

        let font_label = look.font_family.clone().unwrap_or_else(|| format!("Automatic ({family})"));
        let wanted_font = look.font_family.clone();
        let font_button = button("terminal-font".into(), font_label).gap_2().child(symbol("chevron.up.chevron.down", 9., SEMIBOLD, palette.secondary, window)).on_click(cx.listener(
            move |_, _, _, cx| {
                let mut items = vec![
                    macos::MenuItem { title: Some("Automatic (JetBrains Mono, else Menlo)".into()), tag: 0, enabled: true, checked: wanted_font.is_none(), ..Default::default() },
                    macos::MenuItem { title: None, ..Default::default() },
                ];
                items.extend(fonts.iter().enumerate().map(|(i, f)| macos::MenuItem {
                    title: Some(f.clone()),
                    tag: i as i32 + 1,
                    enabled: true,
                    checked: wanted_font.as_deref() == Some(f.as_str()),
                    ..Default::default()
                }));
                let fonts = fonts.clone();
                // (Not inside this event's handling: the menu runs its own loop.)
                cx.spawn(async move |_, cx| {
                    let Some(tag) = macos::popup_menu(&items) else { return };
                    let family = (tag > 0).then(|| fonts.get(tag as usize - 1).cloned()).flatten();
                    cx.update(|cx| Settings::update(cx, |s| s.quick_terminal.look.font_family = family));
                })
                .detach();
            },
        ));
        let size = look.font_size;
        let step = |id: &'static str, text: &'static str, by: f32| {
            button(id.into(), text.into()).on_click(move |_, _, cx| Settings::update(cx, |s| s.quick_terminal.look.font_size = (size.round() + by).clamp(8., 32.)))
        };
        let row = || div().flex().flex_wrap().items_center().gap_1().py(px(2.));
        let opacities = [100, 95, 90, 85, 80, 70];
        let widths = [("Narrow", 0.5), ("Medium", 0.62), ("Wide", 0.8), ("Full", 1.)];
        let heights = [("Short", 0.33), ("Medium", 0.48), ("Tall", 0.66), ("Nearly All", 0.9)];
        let cursors = [("Block", TerminalCursor::Block), ("Bar", TerminalCursor::Bar), ("Underline", TerminalCursor::Underline)];

        div()
            .when(!t.on, |d| d.opacity(0.45))
            .child(heading("Look:"))
            .child(preview(&colors, &look, family, &palette))
            .when(!sources.is_empty(), |d| {
                d.child(div().mt_3().child(import_row)).child(div().pl(px(92.)).mt_1().child(note(
                    "Its colors become a theme of yours, named for it, and chosen; its font, size, cursor and background too, where it says. \
                     Ghostty: its config and the theme it names. Terminal and iTerm2: the default profile."
                        .into(),
                )))
            })
            .when_some(self.look.said.clone(), |d, said| d.child(div().pl(px(92.)).mt_1().text_size(px(12.)).child(said)))
            .child(heading("Theme:"))
            .child(
                div()
                    .h(px(26.))
                    .px_2()
                    .mb_1()
                    .rounded_md()
                    .border_1()
                    .border_color(palette.glass_border)
                    .bg(palette.glass)
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(symbol("magnifyingglass", 11., 0., palette.secondary, window))
                    .child(div().flex_1().child(Input::new(&search).appearance(false).text_size(px(12.)))),
            )
            .child(div().rounded_lg().border_1().border_color(palette.glass_border).p_1().child(list))
            .child(note(format!("{} themes, from iTerm2-Color-Schemes (as Ghostty's are), and yours.", theme::bundled().len())))
            .child(theme_actions)
            .children(editor)
            .child(heading("Text:"))
            .child(row().child(label("Font")).child(font_button))
            .child(
                row()
                    .child(label("Size"))
                    .child(step("terminal-smaller", "−", -1.))
                    .child(div().w(px(48.)).text_center().text_size(px(12.)).child(format!("{} pt", (size * 10.).round() / 10.)))
                    .child(step("terminal-bigger", "+", 1.)),
            )
            .child(
                row().child(label("Cursor")).children(cursors.map(|(name, shape)| {
                    chip(("terminal-cursor", shape as usize).into(), name.into(), look.cursor == shape).on_click(move |_, _, cx| Settings::update(cx, |s| s.quick_terminal.look.cursor = shape))
                })),
            )
            .child(
                row().child(div().w(px(92.))).child(
                    div()
                        .id("terminal-blink")
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
                                .when(look.blink, |d| d.bg(palette.accent).child(symbol("checkmark", 8., SEMIBOLD, palette.selected_text, window))),
                        )
                        .child(div().text_size(px(12.)).child("Blinking"))
                        .on_click(|_, _, cx| Settings::update(cx, |s| s.quick_terminal.look.blink = !s.quick_terminal.look.blink)),
                ),
            )
            .child(heading("Window:"))
            .child(row().child(label("Background")).children(opacities.map(|n| {
                let on = (look.opacity * 100.).round() as i32 == n;
                chip(("terminal-opacity", n as usize).into(), if n == 100 { "Solid".into() } else { format!("{n}%") }, on)
                    .on_click(move |_, _, cx| Settings::update(cx, |s| s.quick_terminal.look.opacity = n as f32 / 100.))
            })))
            .child(row().child(label("Width")).children(widths.into_iter().enumerate().map(|(i, (name, w))| {
                chip(("terminal-width", i).into(), name.into(), (look.width - w).abs() < 0.01).on_click(move |_, _, cx| Settings::update(cx, |s| s.quick_terminal.look.width = w))
            })))
            .child(row().child(label("Height")).children(heights.into_iter().enumerate().map(|(i, (name, h))| {
                chip(("terminal-height", i).into(), name.into(), (look.height - h).abs() < 0.01).on_click(move |_, _, cx| Settings::update(cx, |s| s.quick_terminal.look.height = h))
            })))
            .child(div().pl(px(92.)).child(note("Of the screen it comes down on; the next time it comes down.".into())))
            .into_any_element()
    }
}

/// A theme at a glance: its background, a little of its text, and its
/// colors.
fn swatch(c: &Colors, palette: &Palette) -> Div {
    div()
        .w(px(76.))
        .h(px(20.))
        .flex_none()
        .rounded_sm()
        .border_1()
        .border_color(palette.glass_border)
        .bg(rgb(c.background))
        .flex()
        .items_center()
        .gap(px(3.))
        .px(px(5.))
        .child(div().text_size(px(10.)).text_color(rgb(c.foreground)).child("ab"))
        .children([1, 2, 3, 4, 5, 6].map(|i| div().size(px(6.)).rounded_full().bg(rgb(c.palette[i]))))
}

/// The terminal as it'd look: a prompt, `ls`, `git status`, a selection,
/// the cursor; and its 16 colors.
fn preview(c: &Colors, look: &TerminalLook, family: String, palette: &Palette) -> Div {
    let size = look.font_size.clamp(8., 18.);
    let line_height = (size * 1.25).round();
    let span = |text: &'static str, color: u32| div().text_color(rgb(color)).child(text);
    let line = || div().flex().whitespace_nowrap().h(px(line_height));
    let p = &c.palette;
    let prompt = || line().child(span("~/Projects/f2", p[4])).child(span(" main", p[5])).child(span(" ❯ ", p[2]));
    let selection = c.selection.unwrap_or(theme::dim(c.foreground));
    let cell = px((size * 0.6).round());
    let cursor_color = c.cursor.unwrap_or(c.foreground);
    let cursor = match look.cursor {
        TerminalCursor::Block => div().w(cell).h(px(line_height)).bg(rgb(cursor_color)),
        TerminalCursor::Bar => div().w(px(2.)).h(px(line_height)).bg(rgb(cursor_color)),
        TerminalCursor::Underline => div().w(cell).h(px(line_height)).flex().items_end().child(div().w_full().h(px(2.)).bg(rgb(cursor_color))),
    };
    let strip = |from: usize| div().flex().gap(px(3.)).children((from..from + 8).map(|i| div().size(px(12.)).rounded_sm().bg(rgb(p[i]))));
    div()
        .rounded_lg()
        .border_1()
        .border_color(palette.glass_border)
        .bg(Hsla::from(rgb(c.background)).opacity(look.opacity))
        .p_3()
        .flex()
        .justify_between()
        .gap_3()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .font_family(SharedString::from(family))
                .text_size(px(size))
                .line_height(px(line_height))
                .text_color(rgb(c.foreground))
                .child(prompt().child(span("ls", c.foreground)))
                .child(
                    line()
                        .child(span("Cargo.toml  ", c.foreground))
                        .child(div().text_color(rgb(p[4])).font_weight(FontWeight::BOLD).child("assets  src"))
                        .child(span("  ", c.foreground))
                        .child(div().bg(rgb(selection)).text_color(rgb(c.selected_text.unwrap_or(c.foreground))).child("README.md")),
                )
                .child(prompt().child(span("git status --short", c.foreground)))
                .child(line().child(span(" M ", p[1])).child(span("src/terminal.rs", c.foreground)))
                .child(line().child(span("?? ", p[1])).child(span("notes.txt", p[8])))
                .child(prompt().child(cursor)),
        )
        .child(div().flex_none().flex().flex_col().justify_center().gap(px(3.)).child(strip(0)).child(strip(8)))
}
