//! Sheets over the window, as Finder's: Go to Folder (⇧⌘G), Connect to
//! Server (⌘K), View Options (⌘J), Customize Toolbar, Rename (several
//! items), and the questions before erasing things or changing an extension.

use std::path::{Path, PathBuf};

use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::input::{Input, InputEvent, InputState};

use super::{Finder, View, home};
use crate::listing::SortKey;
use crate::settings::{Column, GroupBy, Settings, Tool};
use crate::style::{SEMIBOLD, glass, symbol};

pub(super) enum Sheet {
    GoToFolder { input: Entity<InputState>, _subscription: Subscription },
    Connect { input: Entity<InputState>, _subscription: Subscription },
    /// A server over SSH to add (or change: see `servers`).
    AddServer(super::servers::AddServer),
    ViewOptions,
    Toolbar,
    EmptyTrash,
    DeleteNow(Vec<PathBuf>),
    /// Renaming changes the extension: keep it, or use the new one.
    Extension { path: PathBuf, name: String },
    /// The first time, without it: ask for Full Disk Access.
    FullDiskAccess,
    RenameItems(super::rename_items::RenameItems),
    /// A folder's color, and a symbol or an emoji on it.
    Customize(super::customize::Customize),
    /// What a copy or move left out ("copied", "moved"), each with why:
    /// the rest was done.
    LeftOut { verb: &'static str, items: Vec<(PathBuf, String)> },
}

impl Sheet {
    fn same_kind(&self, other: &Sheet) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

/// "~/x" and "x" (from home) as full paths.
pub(crate) fn expand(text: &str) -> PathBuf {
    let text = text.trim();
    match text.strip_prefix('~') {
        Some(rest) => home().join(rest.trim_start_matches('/')),
        None if text.starts_with('/') => PathBuf::from(text),
        None => home().join(text),
    }
}

/// Folders whose names start with what's typed after the last "/".
pub(crate) fn completions(text: &str) -> Vec<PathBuf> {
    let full = expand(text);
    let (dir, start) = if text.ends_with('/') {
        (full.clone(), String::new())
    } else {
        (full.parent().map(Path::to_path_buf).unwrap_or_default(), full.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default())
    };
    let mut found: Vec<PathBuf> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.path())
        .filter(|p| {
            let name = p.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
            !name.starts_with('.') && name.starts_with(&start)
        })
        .collect();
    found.sort();
    found.truncate(8);
    found
}

/// A path as typed: home as "~".
fn tilde(path: &Path) -> String {
    match path.strip_prefix(home()) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".into(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

impl Finder {
    pub(super) fn toggle_sheet(&mut self, sheet: Sheet, cx: &mut Context<Self>) {
        let open = self.overlay.as_ref().is_some_and(|s| s.same_kind(&sheet));
        self.overlay = (!open).then_some(sheet);
        cx.notify();
    }

    /// Return in a question: its default button. Whether there was one.
    pub(super) fn confirm_sheet(&mut self, cx: &mut Context<Self>) -> bool {
        match self.overlay.take() {
            Some(Sheet::EmptyTrash) => self.empty_trash(cx),
            Some(Sheet::DeleteNow(paths)) => self.delete_now(paths, cx),
            Some(Sheet::Extension { path, name }) => self.rename_to(path, name, cx),
            Some(Sheet::FullDiskAccess) => cx.open_url(crate::macos::FULL_DISK_ACCESS_SETTINGS),
            Some(sheet @ Sheet::RenameItems(_)) => {
                self.overlay = Some(sheet);
                return false;
            }
            other => {
                self.overlay = other;
                return false;
            }
        }
        cx.notify();
        true
    }

    pub(super) fn open_go_to_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Enter a path, like ~/Documents"));
        let sub = cx.subscribe_in(&input, window, |this, input, event: &InputEvent, window, cx| match event {
            InputEvent::PressEnter { .. } => {
                let text = input.read(cx).value().to_string();
                this.go_to_typed(&text, window, cx);
            }
            InputEvent::Change => cx.notify(),
            _ => {}
        });
        input.update(cx, |i, cx| i.focus(window, cx));
        self.overlay = Some(Sheet::GoToFolder { input, _subscription: sub });
        cx.notify();
    }

    fn go_to_typed(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        if text.trim().is_empty() {
            return;
        }
        let mut path = expand(text);
        // A start of a name: its first match, as typed completion would.
        if !path.exists()
            && let Some(first) = completions(text).into_iter().next()
        {
            path = first;
        }
        self.overlay = None;
        if path.is_dir() {
            self.go_place(path, window, cx);
        } else if path.exists()
            && let Some(parent) = path.parent()
        {
            self.navigate(parent.to_path_buf(), Some(path.clone()), window, cx);
            window.focus(&self.focus, cx);
        } else {
            self.show_notice(format!("The folder “{}” can't be found.", text.trim()), cx);
        }
    }

    pub(super) fn open_connect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| InputState::new(window, cx).default_value("smb://").placeholder("smb://server/share, or ssh://user@host"));
        let sub = cx.subscribe_in(&input, window, |this, input, event: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { .. } = event {
                let text = input.read(cx).value().to_string();
                this.connect(&text, window, cx);
            }
        });
        input.update(cx, |i, cx| i.focus(window, cx));
        self.overlay = Some(Sheet::Connect { input, _subscription: sub });
        cx.notify();
    }

    /// Connect to a server: macOS mounts it (asking for a name and password
    /// itself), and it shows up under Locations.
    fn connect(&mut self, address: &str, window: &mut Window, cx: &mut Context<Self>) {
        let address = address.trim().to_string();
        if address.len() < 4 {
            return;
        }
        // (Over SSH: F2's own, in this window.)
        if let Some((dest, path)) = crate::remote::parse(&address) {
            self.overlay = None;
            window.focus(&self.focus, cx);
            return self.connect_server(dest, path, window, cx);
        }
        let address = if address.contains("://") { address } else { format!("smb://{address}") };
        cx.open_url(&address);
        Settings::update(cx, |s| {
            s.recent_servers.retain(|a| *a != address);
            s.recent_servers.insert(0, address.clone());
            s.recent_servers.truncate(10);
        });
        self.overlay = None;
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Change an item's name, asking first if that changes its extension.
    pub(super) fn rename_checking_extension(&mut self, path: PathBuf, name: String, cx: &mut Context<Self>) {
        let ext = |p: &Path| p.extension().map(|e| e.to_string_lossy().into_owned());
        let changes = !path.is_dir() && ext(&path) != ext(Path::new(&name)) && ext(&path).is_some();
        if changes && Settings::get(cx).warn_extension_change {
            self.overlay = Some(Sheet::Extension { path, name });
            cx.notify();
        } else {
            self.rename_to(path, name, cx);
        }
    }

    pub(super) fn render_sheet(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let palette = self.palette;
        let Some(overlay) = self.overlay.as_ref() else {
            self.motions.sheet_down("sheet");
            return None;
        };
        let body = match overlay {
            Sheet::ViewOptions => return Some(self.render_view_options(window, cx)),
            Sheet::RenameItems(_) => self.render_rename_items(window, cx),
            Sheet::AddServer(_) => self.render_add_server(window, cx),
            Sheet::Customize(_) => self.render_customize(window, cx),
            Sheet::GoToFolder { input, .. } => {
                let text = input.read(cx).value().to_string();
                let list = if text.trim().is_empty() { Settings::get(cx).recent_folders.clone() } else { completions(&text) };
                let title = if text.trim().is_empty() { "Recent Folders" } else { "Folders" };
                card(&palette)
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Go to Folder"))
                    .child(Input::new(input).small())
                    .child(div().text_size(px(11.)).text_color(palette.secondary).child(title))
                    .children(list.into_iter().enumerate().map(|(i, path)| {
                        let shown = tilde(&path);
                        div()
                            .id(("go", i))
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .text_size(px(12.))
                            .hover(|s| s.bg(palette.glass_selected))
                            .child(shown)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.overlay = None;
                                this.go_place(path.clone(), window, cx);
                            }))
                    }))
                    .child(buttons().child(button("cancel", "Cancel", false, &palette, cx, |this, window, cx| this.escape(window, cx))).child(
                        button("go", "Go", true, &palette, cx, |this, window, cx| {
                            if let Some(Sheet::GoToFolder { input, .. }) = &this.overlay {
                                let text = input.read(cx).value().to_string();
                                this.go_to_typed(&text, window, cx);
                            }
                        }),
                    ))
            }
            Sheet::Connect { input, .. } => {
                let recent = Settings::get(cx).recent_servers.clone();
                card(&palette)
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Connect to Server"))
                    .child(Input::new(input).small())
                    .when(!recent.is_empty(), |d| d.child(div().text_size(px(11.)).text_color(palette.secondary).child("Recent Servers")))
                    .children(recent.into_iter().enumerate().map(|(i, address)| {
                        let shown = address.clone();
                        div()
                            .id(("server", i))
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .text_size(px(12.))
                            .hover(|s| s.bg(palette.glass_selected))
                            .child(shown)
                            .on_click(cx.listener(move |this, _, window, cx| this.connect(&address, window, cx)))
                    }))
                    .child(
                        buttons()
                            .child(button("browse", "Browse", false, &palette, cx, |this, window, cx| {
                                this.overlay = None;
                                this.go_virtual(super::Virtual::Network, window, cx);
                            }))
                            .child(div().flex_1())
                            .child(button("cancel", "Cancel", false, &palette, cx, |this, window, cx| this.escape(window, cx)))
                            .child(button("connect", "Connect", true, &palette, cx, |this, window, cx| {
                                if let Some(Sheet::Connect { input, .. }) = &this.overlay {
                                    let text = input.read(cx).value().to_string();
                                    this.connect(&text, window, cx);
                                }
                            })),
                    )
            }
            Sheet::Toolbar => {
                let shown = Settings::get(cx).toolbar.clone();
                card(&palette)
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Customize Toolbar"))
                    .child(div().text_size(px(12.)).text_color(palette.secondary).child("Choose what the toolbar shows."))
                    .children(Tool::ALL.into_iter().map(|tool| {
                        let on = shown.contains(&tool);
                        checkbox(SharedString::from(format!("tool-{tool:?}")), tool.title(), on, &palette, window, cx, move |_, cx| {
                            Settings::update(cx, |s| {
                                if on {
                                    s.toolbar.retain(|t| *t != tool);
                                } else {
                                    s.toolbar.push(tool);
                                    s.toolbar.sort_by_key(|t| Tool::ALL.iter().position(|a| a == t));
                                }
                            })
                        })
                    }))
                    .child(buttons().child(button("defaults", "Use Defaults", false, &palette, cx, |_, _, cx| {
                        Settings::update(cx, |s| s.toolbar = crate::settings::Settings::default().toolbar)
                    })).child(div().flex_1()).child(button("done", "Done", true, &palette, cx, |this, window, cx| this.escape(window, cx))))
            }
            Sheet::LeftOut { verb, items } => {
                let title = match items.len() {
                    1 => format!("“{}” couldn't be {verb}", items[0].0.file_name().unwrap_or_default().to_string_lossy()),
                    n => format!("{} items couldn't be {verb}", crate::listing::thousands(n as u64)),
                };
                let rows = items.iter().take(500).map(|(path, why)| {
                    div()
                        .py_1()
                        .flex()
                        .flex_col()
                        .child(div().text_size(px(12.)).font_weight(FontWeight::MEDIUM).truncate().child(path.file_name().unwrap_or_default().to_string_lossy().into_owned()))
                        .child(div().text_size(px(11.)).text_color(palette.secondary).truncate().child(why.clone()))
                        .child(div().text_size(px(11.)).text_color(palette.tertiary).text_ellipsis_middle().child(path.parent().map(|p| p.display().to_string()).unwrap_or_default()))
                });
                let these = if *verb == "moved" { "These were left where they are:" } else { "These weren't:" };
                question(&palette, &title, &format!("Everything else was {verb}. {these}"))
                    .child(div().id("left-out").max_h(px(240.)).overflow_y_scroll().flex().flex_col().children(rows))
                    .child(buttons().child(button("ok", "OK", true, &palette, cx, |this, window, cx| this.escape(window, cx))))
            }
            Sheet::EmptyTrash => question(
                &palette,
                &format!("Are you sure you want to permanently erase the items in the {}?", crate::macos::bin()),
                "You can't undo this action.",
            )
            .child(buttons().child(button("cancel", "Cancel", false, &palette, cx, |this, window, cx| this.escape(window, cx))).child(button(
                "empty",
                format!("Empty {}", crate::macos::bin()),
                true,
                &palette,
                cx,
                |this, _, cx| {
                    this.overlay = None;
                    this.empty_trash(cx);
                },
            ))),
            Sheet::FullDiskAccess => question(
                &palette,
                &format!("Give {} Full Disk Access?", crate::APP_NAME),
                &format!(
                    "With it, macOS won't ask about Desktop, Documents, Downloads and other guarded folders one by one, and search covers the whole disk. Turn on {0} in the list (if it isn't there, add it with +), then let System Settings reopen {0}.",
                    crate::APP_NAME
                ),
            )
            .child(buttons().child(button("later", "Not Now", false, &palette, cx, |this, window, cx| this.escape(window, cx))).child(button(
                "open",
                "Open System Settings",
                true,
                &palette,
                cx,
                |this, _, cx| {
                    this.overlay = None;
                    cx.open_url(crate::macos::FULL_DISK_ACCESS_SETTINGS);
                    cx.notify();
                },
            ))),
            Sheet::DeleteNow(paths) => {
                let what = match paths.as_slice() {
                    [one] => format!("“{}”", one.file_name().unwrap_or_default().to_string_lossy()),
                    many => format!("these {} items", many.len()),
                };
                // (On a server: there's no Trash there.)
                let server = paths.first().and_then(|p| crate::remote::here(p)).map(|h| h.server);
                let detail = match &server {
                    Some(server) => format!("It's on {server}, which has no Trash: it will be deleted for good. You can't undo this action."),
                    None => "This item will be deleted immediately. You can't undo this action.".to_string(),
                };
                question(&palette, &format!("Are you sure you want to delete {what}?"), &detail)
                .child(buttons().child(button("cancel", "Cancel", false, &palette, cx, |this, window, cx| this.escape(window, cx))).child(button(
                    "delete",
                    "Delete",
                    true,
                    &palette,
                    cx,
                    |this, _, cx| {
                        if let Some(Sheet::DeleteNow(paths)) = this.overlay.take() {
                            this.delete_now(paths, cx);
                        }
                    },
                )))
            }
            Sheet::Extension { path, name } => {
                let ext = |p: &Path| p.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
                let (old, new) = (ext(path), ext(Path::new(name)));
                let keep_label: SharedString = format!("Keep {old}").into();
                let use_label: SharedString = if new.is_empty() { "Remove".into() } else { format!("Use {new}").into() };
                question(
                    &palette,
                    &format!("Are you sure you want to change the extension from “{old}” to “{}”?", if new.is_empty() { "(none)" } else { &new }),
                    "If you make this change, your document may open in a different app.",
                )
                .child(
                    buttons()
                        .child(button("keep", keep_label, false, &palette, cx, |this, _, cx| {
                            if let Some(Sheet::Extension { path, name }) = this.overlay.take() {
                                // The new name, with the old extension.
                                let stem = Path::new(&name).file_stem().unwrap_or_default().to_string_lossy().into_owned();
                                let ext = path.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default();
                                this.rename_to(path, format!("{stem}.{ext}"), cx);
                            }
                        }))
                        .child(button("use", use_label, true, &palette, cx, |this, _, cx| {
                            if let Some(Sheet::Extension { path, name }) = this.overlay.take() {
                                this.rename_to(path, name, cx);
                            }
                        })),
                )
            }
        };
        Some(self.sheet_motion("sheet", body.into_any_element()))
    }

    /// ⌘J: how this folder is shown. A panel at the right, as Finder's.
    fn render_view_options(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette;
        let settings = Settings::get(cx).clone();
        let segment = |label: &'static str, on: bool, f: fn(&mut Finder, &mut Context<Finder>), cx: &mut Context<Self>| {
            div()
                .id(label)
                .px_2()
                .py(px(3.))
                .rounded_md()
                .text_size(px(12.))
                .when(on, |d| d.bg(palette.glass_selected))
                .child(label)
                .on_click(cx.listener(move |this, _, _, cx| f(this, cx)))
        };
        let heading = |text: &'static str| div().mt_2().text_size(px(11.)).font_weight(FontWeight::SEMIBOLD).text_color(palette.secondary).child(text);
        let row = || div().flex().flex_wrap().gap_1();
        let stepper = |id: &'static str, label: String, minus: fn(&mut Finder, &mut Context<Finder>), plus: fn(&mut Finder, &mut Context<Finder>), cx: &mut Context<Self>| {
            div()
                .flex()
                .items_center()
                .gap_2()
                .text_size(px(12.))
                .child(div().flex_1().child(label))
                .child(glass(22., &palette).id(SharedString::from(format!("{id}-"))).px_2().child("−").on_click(cx.listener(move |this, _, _, cx| minus(this, cx))))
                .child(glass(22., &palette).id(SharedString::from(format!("{id}+"))).px_2().child("+").on_click(cx.listener(move |this, _, _, cx| plus(this, cx))))
        };
        let views = row()
            .child(segment("Icons", self.view == View::Icons, |t, cx| t.set_view(View::Icons, cx), cx))
            .child(segment("List", self.view == View::List, |t, cx| t.set_view(View::List, cx), cx))
            .child(segment("Columns", self.view == View::Columns, |t, cx| t.set_view(View::Columns, cx), cx))
            .child(segment("Gallery", self.view == View::Gallery, |t, cx| t.set_view(View::Gallery, cx), cx));
        let groups = row().children(
            [
                (GroupBy::None, "None"),
                (GroupBy::Kind, "Kind"),
                (GroupBy::DateModified, "Date Modified"),
                (GroupBy::DateAdded, "Date Added"),
                (GroupBy::Size, "Size"),
                (GroupBy::Tags, "Tags"),
            ]
            .into_iter()
            .map(|(g, label)| {
                div()
                    .id(SharedString::from(format!("group-{label}")))
                    .px_2()
                    .py(px(3.))
                    .rounded_md()
                    .text_size(px(12.))
                    .when(self.group == g, |d| d.bg(palette.glass_selected))
                    .child(label)
                    .on_click(cx.listener(move |this, _, _, cx| this.set_group(g, cx)))
            }),
        );
        let sorts = row().children(
            [
                (SortKey::Name, "Name"),
                (SortKey::Kind, "Kind"),
                (SortKey::Modified, "Date Modified"),
                (SortKey::Created, "Date Created"),
                (SortKey::Added, "Date Added"),
                (SortKey::LastOpened, "Date Last Opened"),
                (SortKey::Size, "Size"),
            ]
            .into_iter()
            .map(|(k, label)| {
                div()
                    .id(SharedString::from(format!("sort-{label}")))
                    .px_2()
                    .py(px(3.))
                    .rounded_md()
                    .text_size(px(12.))
                    .when(self.sort.0 == k, |d| d.bg(palette.glass_selected))
                    .child(label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if this.sort.0 != k {
                            this.set_sort(k, cx)
                        }
                    }))
            }),
        );
        let columns = div().flex().flex_col().gap_1().children(Column::ALL.into_iter().map(|col| {
            let on = settings.columns.iter().any(|(c, _)| *c == col);
            checkbox(SharedString::from(format!("col-{col:?}")), col.title(), on, &palette, window, cx, move |this, cx| this.toggle_column(col, cx))
        }));
        div()
            .absolute()
            .top(px(52.))
            .right(px(12.))
            .w(px(260.))
            .flex()
            .flex_col()
            .gap_1()
            .p_4()
            .rounded(px(16.))
            .bg(palette.sidebar)
            .border_1()
            .border_color(palette.glass_border)
            .shadow_lg()
            .occlude()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(super::display_name(&self.cwd)))
                    .child(
                        div()
                            .id("close-options")
                            .child(symbol("xmark", 10., SEMIBOLD, palette.secondary, window))
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_sheet(Sheet::ViewOptions, cx))),
                    ),
            )
            .child(heading("View"))
            .child(views)
            .child(heading("Group By"))
            .child(groups)
            .child(heading("Sort By"))
            .child(sorts)
            .when(self.view == View::Icons, |d| {
                d.child(heading("Icons")).child(stepper(
                    "icon",
                    format!("Icon size: {}", self.icon_size as u32),
                    |t, cx| t.set_sizes(-16., 0., cx),
                    |t, cx| t.set_sizes(16., 0., cx),
                    cx,
                ))
            })
            .child(stepper("text", format!("Text size: {}", self.text_size as u32), |t, cx| t.set_sizes(0., -1., cx), |t, cx| t.set_sizes(0., 1., cx), cx))
            .when(self.view == View::List, |d| d.child(heading("Show Columns")).child(columns))
            .child(checkbox("sizes", "Calculate all sizes", settings.calculate_sizes, &palette, window, cx, |this, cx| {
                Settings::update(cx, |s| s.calculate_sizes = !s.calculate_sizes);
                if Settings::get(cx).calculate_sizes {
                    this.work_out_sizes(cx);
                }
            }))
            .child(div().mt_2().child(button("defaults", "Use as Defaults", false, &palette, cx, |this, _, cx| {
                let fv = this.folder_view();
                Settings::update(cx, |s| {
                    s.default_view = fv;
                    s.folder_views.clear();
                });
                this.show_notice("This view is now the default for folders.", cx);
            })))
            .into_any_element()
    }

    fn set_sizes(&mut self, icon: f32, text: f32, cx: &mut Context<Self>) {
        self.icon_size = (self.icon_size + icon).clamp(32., 128.);
        self.text_size = (self.text_size + text).clamp(10., 16.);
        self.save_folder_view(cx);
        cx.notify();
    }

    /// Show or hide a list column.
    pub(super) fn toggle_column(&mut self, col: Column, cx: &mut Context<Self>) {
        Settings::update(cx, |s| {
            if s.columns.iter().any(|(c, _)| *c == col) {
                s.columns.retain(|(c, _)| *c != col);
            } else {
                s.columns.push((col, col.default_width()));
            }
        });
        if col == Column::DateLastOpened && self.listing.iter().all(|e| e.opened == 0) && !self.flat() {
            let keep = self.selected_entry().map(|e| e.path.clone());
            self.listed = None;
            self.load(keep, cx);
        }
        cx.notify();
    }
}

pub(super) fn card(palette: &crate::style::Palette) -> Div {
    div()
        .mt(px(60.))
        .w(px(440.))
        // (A half-width pane is narrower.)
        .max_w(relative(0.92))
        .flex()
        .flex_col()
        .gap_2()
        .p_5()
        .rounded(px(18.))
        .bg(palette.sidebar)
        .border_1()
        .border_color(palette.glass_border)
        .shadow_2xl()
}

fn question(palette: &crate::style::Palette, title: &str, detail: &str) -> Div {
    card(palette)
        .gap_3()
        .child(div().font_weight(FontWeight::SEMIBOLD).child(title.to_string()))
        .child(div().text_size(px(12.)).text_color(palette.secondary).child(detail.to_string()))
}

pub(super) fn buttons() -> Div {
    div().mt_2().flex().justify_end().gap_2()
}

pub(super) fn button(
    id: &'static str,
    label: impl Into<SharedString>,
    primary: bool,
    palette: &crate::style::Palette,
    cx: &mut Context<Finder>,
    f: impl Fn(&mut Finder, &mut Window, &mut Context<Finder>) + 'static,
) -> Stateful<Div> {
    let palette = *palette;
    glass(26., &palette)
        .id(id)
        .px_4()
        .text_size(px(12.))
        .when(primary, |d| d.bg(palette.accent).text_color(palette.selected_text).border_color(palette.accent))
        .child(label.into())
        .on_click(cx.listener(move |this, _, window, cx| f(this, window, cx)))
}

fn checkbox(
    id: impl Into<SharedString>,
    label: &'static str,
    on: bool,
    palette: &crate::style::Palette,
    window: &Window,
    cx: &mut Context<Finder>,
    f: impl Fn(&mut Finder, &mut Context<Finder>) + 'static,
) -> Stateful<Div> {
    let palette = *palette;
    div()
        .id(ElementId::Name(id.into()))
        .flex()
        .items_center()
        .gap_2()
        .text_size(px(12.))
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
        .on_click(cx.listener(move |this, _, _, cx| {
            f(this, cx);
            cx.notify();
        }))
}
