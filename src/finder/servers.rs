//! Servers (over SSH: see `crate::remote`) in a window: connecting, and
//! what's done in a server's folder, done there. A server's folder stands
//! in here at a path in the cache, so the window goes about it as any
//! folder; what would act on the stand-in (opening, a name, deleting, a
//! new folder, copying in or out) comes here instead, and what can't be
//! done on a server yet says so.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Instant;

use gpui::{AppContext as _, Context, Entity, SharedString, Subscription, Window};
use gpui_kit::component::Sizable as _;
use gpui_kit::component::input::{Input, InputEvent, InputState};

use super::Finder;
use super::menu::Command;
use super::places::{Goes, Place};
use crate::fileops::{How, Progress};
use crate::listing::{Entry, count};
use crate::remote::{self, Here};
use crate::settings::Settings;
use crate::sftp::Dest;

/// The servers for the sidebar: ~/.ssh/config's Hosts (less those taken
/// out of it), then those added in F2.
pub(super) fn server_places(cx: &gpui::App) -> Vec<Place> {
    let settings = Settings::get(cx);
    let mut places: Vec<Place> = remote::hosts_now()
        .into_iter()
        .filter(|h| !settings.hidden_servers.contains(h))
        .map(|host| Place::new("server.rack", host.clone(), Goes::Server(Dest { host, port: None, options: Vec::new() })))
        .collect();
    for saved in &settings.servers {
        if let Some((dest, _)) = remote::parse(&saved.address) {
            places.push(Place::new("server.rack", saved.name.clone(), Goes::Server(dest)));
        }
    }
    places
}

/// The address a server's kept by ("ssh://me@box:2222/var/www").
pub(super) fn address_of(dest: &Dest, folder: Option<&std::path::Path>) -> String {
    let mut at = format!("ssh://{}", dest.host);
    if let Some(port) = dest.port {
        at += &format!(":{port}");
    }
    if let Some(folder) = folder {
        at += &folder.to_string_lossy();
    }
    at
}

/// Which of the servers added in F2 `dest` is, if it is one.
pub(super) fn saved_server(dest: &Dest, cx: &gpui::App) -> Option<usize> {
    Settings::get(cx).servers.iter().position(|k| remote::parse(&k.address).is_some_and(|(d, _)| d.host == dest.host && d.port == dest.port))
}

/// Add Server's fields (and which added one it changes, if one).
pub(crate) struct AddServer {
    name: Entity<InputState>,
    host: Entity<InputState>,
    user: Entity<InputState>,
    port: Entity<InputState>,
    folder: Entity<InputState>,
    editing: Option<usize>,
    error: Option<String>,
    _subscriptions: Vec<Subscription>,
}

/// The address Connect to Server keeps for `dest`.
fn address(dest: &Dest) -> String {
    match dest.port {
        Some(port) => format!("ssh://{}:{port}", dest.host),
        None => format!("ssh://{}", dest.host),
    }
}

/// A server being connected to, shown in the window meanwhile (see
/// `connect_server`).
pub(super) struct Connecting {
    pub(super) dest: Dest,
    pub(super) path: Option<PathBuf>,
    /// Why it couldn't, once it's given up.
    pub(super) error: Option<String>,
    /// Which try this is (an older one's answer is let be).
    pub(super) round: u64,
    pub(super) started: Instant,
}

impl Finder {
    /// The window shows a server's folder: which, and where there.
    pub(super) fn remote_here(&self) -> Option<Here> {
        if self.virtual_folder.is_some() {
            return None;
        }
        remote::here(&self.cwd)
    }

    /// Connect to a server (logging in, asked for what it needs): the
    /// window goes there at once, saying so (Cancel goes back), then shows
    /// the folder you log in to there, or `path`; if it can't, why, and Try
    /// Again.
    pub(crate) fn connect_server(&mut self, dest: Dest, path: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        let again = self.connecting.as_ref().is_some_and(|c| c.dest == dest);
        if !again {
            // (Back comes back here.)
            let here = self.here();
            self.back.push(here);
            self.forward.clear();
        }
        self.clear_query(window, cx);
        self.virtual_folder = None;
        // (Nothing of the folder left behind stays to be acted on.)
        self.listing.clear();
        self.rows.clear();
        self.lines.clear();
        self.expanded.clear();
        self.select_only(None);
        self.listed = None;
        self.error = None;
        self.notice = None;
        self.next_connect += 1;
        let round = self.next_connect;
        self.connecting = Some(Connecting { dest: dest.clone(), path: path.clone(), error: None, round, started: Instant::now() });
        self.title_dirty = true;
        window.focus(&self.focus, cx);
        cx.notify();
        // (Redrawn as it goes, for its dots and the seconds.)
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(std::time::Duration::from_millis(300)).await;
                let going = this.update(cx, |this, cx| {
                    let going = this.connecting.as_ref().is_some_and(|c| c.round == round && c.error.is_none());
                    if going {
                        cx.notify();
                    }
                    going
                });
                if !going.unwrap_or(false) {
                    break;
                }
            }
        })
        .detach();
        cx.spawn_in(window, async move |this, cx| {
            let to = dest.clone();
            let result = cx.background_executor().spawn(async move { remote::connect(to) }).await;
            this.update_in(cx, |this, window, cx| {
                // (Cancelled, or gone elsewhere, meanwhile: let be.)
                if this.connecting.as_ref().is_none_or(|c| c.round != round) {
                    return;
                }
                match result {
                    Ok(home) => {
                        let at = address(&dest);
                        // (One not in ~/.ssh/config nor added: kept, for the sidebar.)
                        let known = (remote::hosts_now().contains(&dest.host) && dest.port.is_none())
                            || Settings::get(cx).servers.iter().any(|s| remote::parse(&s.address).is_some_and(|(d, _)| d.host == dest.host && d.port == dest.port));
                        if !known {
                            Settings::update(cx, |s| {
                                s.servers.push(crate::settings::SavedServer { name: dest.host.clone(), address: at.clone() });
                            });
                        }
                        let folder = path.map_or(home, |p| remote::local_of(&dest, &p));
                        this.connecting = None;
                        this.go_to(super::Loc::Folder(folder), None, window, cx);
                        window.focus(&this.focus, cx);
                    }
                    Err(e) => {
                        eprintln!("ssh: couldn't connect to {}: {e}", dest.host);
                        if let Some(c) = this.connecting.as_mut() {
                            c.error = Some(e);
                        }
                        this.title_dirty = true;
                        cx.notify();
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    /// Add a server over SSH (or change one added: `editing`).
    pub(crate) fn open_add_server(&mut self, editing: Option<usize>, window: &mut Window, cx: &mut Context<Self>) {
        let saved = editing.and_then(|i| Settings::get(cx).servers.get(i).cloned());
        let (dest, folder) = saved.as_ref().and_then(|s| remote::parse(&s.address)).unzip();
        let (user, host) = match dest.as_ref().map(|d| d.host.clone()) {
            Some(h) => match h.split_once('@') {
                Some((u, h)) => (u.to_string(), h.to_string()),
                None => (String::new(), h),
            },
            None => (String::new(), String::new()),
        };
        let field = |value: String, placeholder: &str, window: &mut Window, cx: &mut Context<Self>| {
            let placeholder = placeholder.to_string();
            cx.new(|cx| InputState::new(window, cx).default_value(value).placeholder(placeholder))
        };
        let name = field(saved.as_ref().map(|s| s.name.clone()).unwrap_or_default(), "Optional: what to call it", window, cx);
        let host = field(host, "box.example.com, or 192.168.1.20", window, cx);
        let user = field(user, &std::env::var("USER").unwrap_or_default(), window, cx);
        let port = field(dest.as_ref().and_then(|d| d.port).map(|p| p.to_string()).unwrap_or_default(), "22", window, cx);
        let folder = field(folder.flatten().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default(), "Optional: the one you log in to", window, cx);
        // (Return in any of them: added, and connected to.)
        let subscriptions = [&name, &host, &user, &port, &folder]
            .into_iter()
            .map(|input| {
                cx.subscribe_in(input, window, |this, _, event: &InputEvent, window, cx| {
                    if let InputEvent::PressEnter { .. } = event {
                        this.submit_add_server(window, cx);
                    }
                })
            })
            .collect();
        host.update(cx, |i, cx| i.focus(window, cx));
        self.overlay = Some(super::sheets::Sheet::AddServer(AddServer { name, host, user, port, folder, editing, error: None, _subscriptions: subscriptions }));
        cx.notify();
    }

    /// Add Server's Connect: kept (or changed), and connected to.
    fn submit_add_server(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(super::sheets::Sheet::AddServer(add)) = self.overlay.as_mut() else { return };
        let text = |input: &Entity<InputState>, cx: &Context<Self>| input.read(cx).value().trim().to_string();
        let (name, mut host, mut user, mut port, folder) = (text(&add.name, cx), text(&add.host, cx), text(&add.user, cx), text(&add.port, cx), text(&add.folder, cx));
        // ("me@box:2222" in the address: its user and port.)
        if let Some((u, h)) = host.clone().split_once('@') {
            if user.is_empty() {
                user = u.to_string();
            }
            host = h.to_string();
        }
        if let Some((h, p)) = host.clone().rsplit_once(':')
            && !h.contains(':')
            && p.parse::<u16>().is_ok()
        {
            if port.is_empty() {
                port = p.to_string();
            }
            host = h.to_string();
        }
        let host = host.trim_start_matches("ssh://").trim_start_matches("sftp://").trim_end_matches('/').to_string();
        let error = if host.is_empty() {
            Some("Type the server's address.")
        } else if host.contains(char::is_whitespace) || host.contains('/') {
            Some("An address has no spaces or slashes in it.")
        } else if !port.is_empty() && port.parse::<u16>().map_or(true, |p| p == 0) {
            Some("The port is a number, like 22.")
        } else if !folder.is_empty() && !folder.starts_with('/') {
            Some("A folder there starts with “/”.")
        } else {
            None
        };
        if let Some(e) = error {
            add.error = Some(e.to_string());
            cx.notify();
            return;
        }
        let dest = Dest { host: if user.is_empty() { host.clone() } else { format!("{user}@{host}") }, port: port.parse().ok(), options: Vec::new() };
        let folder = (!folder.is_empty()).then(|| PathBuf::from(folder));
        let saved = crate::settings::SavedServer { name: if name.is_empty() { host } else { name }, address: address_of(&dest, folder.as_deref()) };
        let editing = add.editing;
        Settings::update(cx, |s| match editing.filter(|&i| i < s.servers.len()) {
            Some(i) => s.servers[i] = saved,
            None => {
                s.servers.retain(|k| k.address != saved.address);
                s.servers.push(saved);
            }
        });
        self.overlay = None;
        self.connect_server(dest, folder, window, cx);
    }

    /// Add Server's sheet.
    pub(super) fn render_add_server(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> gpui::Div {
        use super::sheets::{button, buttons, card};
        use gpui::{FontWeight, ParentElement, Styled, div, prelude::FluentBuilder, px};
        let palette = self.palette;
        let Some(super::sheets::Sheet::AddServer(add)) = self.overlay.as_ref() else { return div() };
        let row = |label: &'static str, input: &Entity<InputState>| {
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(div().w(px(64.)).text_size(px(12.)).text_color(palette.secondary).child(label))
                .child(div().flex_1().child(Input::new(input).small()))
        };
        let editing = add.editing.is_some();
        card(&palette)
            .child(div().font_weight(FontWeight::SEMIBOLD).child(if editing { "Edit Server" } else { "Add Server" }))
            .child(div().text_size(px(12.)).text_color(palette.secondary).child(
                "Over SSH, with your Mac's own ssh: your keys and ~/.ssh/config count. A password, if it asks for one, comes up in a dialog.",
            ))
            .child(row("Address", &add.host))
            .child(div().flex().gap_3().child(div().flex_1().child(row("User", &add.user))).child(div().w(px(150.)).child(row("Port", &add.port))))
            .child(row("Folder", &add.folder))
            .child(row("Name", &add.name))
            .when_some(add.error.clone(), |d, e| d.child(div().text_size(px(12.)).text_color(gpui::rgb(0xff453a)).child(e)))
            .child(
                buttons()
                    .child(button("add-cancel", "Cancel", false, &palette, cx, |this, window, cx| this.escape(window, cx)))
                    .child(button("add-connect", if editing { "Save and Connect" } else { "Connect" }, true, &palette, cx, |this, window, cx| {
                        this.submit_add_server(window, cx)
                    })),
            )
    }

    /// Take a server out of the sidebar: one added, forgotten; one of
    /// ~/.ssh/config's, left out (the file's left as it is).
    pub(super) fn remove_server(&mut self, dest: &Dest, cx: &mut Context<Self>) {
        let host = dest.host.clone();
        let port = dest.port;
        Settings::update(cx, |s| {
            let before = s.servers.len();
            s.servers.retain(|k| remote::parse(&k.address).is_none_or(|(d, _)| !(d.host == host && d.port == port)));
            if s.servers.len() == before && !s.hidden_servers.contains(&host) {
                s.hidden_servers.push(host.clone());
            }
        });
        cx.notify();
    }

    /// Stop connecting: back where the window was.
    pub(super) fn cancel_connect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.connecting.take().is_none() {
            return;
        }
        match self.back.pop() {
            Some(before) => self.go_to(before, None, window, cx),
            None => self.go_to(super::Loc::Folder(super::home()), None, window, cx),
        }
        cx.notify();
    }

    /// The window while it connects: the server, that it's connecting (and
    /// for how long, past a moment), Cancel; or why it couldn't, and Try
    /// Again.
    pub(super) fn render_connecting(&self, window: &mut Window, cx: &mut Context<Self>) -> gpui::AnyElement {
        use gpui::{FontWeight, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled, div, prelude::FluentBuilder, px};
        let palette = self.palette;
        let Some(c) = &self.connecting else { return div().into_any_element() };
        let host = c.dest.host.clone();
        let failed = c.error.clone();
        let secs = c.started.elapsed().as_secs();
        let dots = ".".repeat(1 + (c.started.elapsed().as_millis() / 400 % 3) as usize);
        let title = match &failed {
            Some(_) => format!("Couldn't connect to {host}"),
            None => format!("Connecting to {host}{dots}"),
        };
        let detail = match &failed {
            Some(e) => e.clone(),
            None if secs >= 3 => format!("Still trying ({secs} s). If it asks for a password or to trust a new server, that's in a dialog."),
            None => " ".to_string(),
        };
        let button = |id: &'static str, label: &'static str, primary: bool| {
            crate::style::glass(28., &palette)
                .id(id)
                .px_4()
                .text_size(px(13.))
                .when(primary, |d| d.bg(palette.accent).text_color(palette.selected_text))
                .child(label)
        };
        let dest = c.dest.clone();
        let path = c.path.clone();
        div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_2()
            .px_6()
            .child(crate::style::symbol("server.rack", 40., 0., if failed.is_some() { palette.tertiary } else { palette.secondary }, window))
            .child(div().mt_2().text_size(px(15.)).font_weight(FontWeight::SEMIBOLD).text_color(palette.label).child(title))
            .child(div().max_w(px(420.)).text_center().text_size(px(12.)).text_color(palette.secondary).child(detail))
            .child(
                div()
                    .mt_3()
                    .flex()
                    .gap_2()
                    .child(button("connect-cancel", if failed.is_some() { "Back" } else { "Cancel" }, false).on_click(cx.listener(|this, _, window, cx| this.cancel_connect(window, cx))))
                    .when(failed.is_some(), |d| {
                        d.child(button("connect-again", "Try Again", true).on_click(cx.listener(move |this, _, window, cx| {
                            this.connect_server(dest.clone(), path.clone(), window, cx)
                        })))
                    }),
            )
            .into_any_element()
    }

    /// Open a server's items: the first folder, here; files copied here
    /// first, then opened in their apps (saved there, they go back).
    pub(super) fn open_remote(&mut self, entries: Vec<Entry>, window: &mut Window, cx: &mut Context<Self>) {
        let (folders, files): (Vec<Entry>, Vec<Entry>) = entries.into_iter().partition(|e| e.is_dir);
        if let Some(folder) = folders.into_iter().next() {
            self.go_place(folder.path, window, cx);
        }
        for file in files {
            let name = file.name.clone();
            if file.size > 8 << 20 {
                self.show_notice(format!("Opening “{name}” ({})…", crate::listing::human_size(file.size)), cx);
            }
            self.fetch_then(file.path, cx, move |this, path, cx| {
                remote::opened(&path);
                cx.open_with_system(&path);
                this.notice = None;
                cx.notify();
            });
        }
    }

    /// `then`, once the server's file `path` is copied to its stand-in.
    pub(super) fn fetch_then(&mut self, path: PathBuf, cx: &mut Context<Self>, then: impl FnOnce(&mut Self, PathBuf, &mut Context<Self>) + 'static) {
        cx.spawn(async move |this, cx| {
            let target = path.clone();
            let fetched = cx.background_executor().spawn(async move { remote::fetch(&target) }).await;
            this.update(cx, |this, cx| match fetched {
                Ok(()) => then(this, path, cx),
                Err(e) => this.show_notice(format!("Couldn't open “{}”: {e}", path.file_name().unwrap_or_default().to_string_lossy()), cx),
            })
            .ok();
        })
        .detach();
    }

    /// A command on a server's items, or in a server's folder: done there,
    /// or said it can't be yet. False if it's not a server's: done as ever.
    pub(super) fn run_remote(&mut self, command: &Command, window: &mut Window, cx: &mut Context<Self>) -> bool {
        // (Connecting: the folder left behind isn't to be acted on.)
        if self.connecting.is_some() {
            return !matches!(
                command,
                Command::GoTo(_) | Command::Eject(_) | Command::RemoveFromSidebar(_) | Command::EmptyTrash | Command::ViewAs(_) | Command::OpenInTab(_)
            );
        }
        let here = self.remote_here();
        let on_server = |paths: &[PathBuf]| paths.iter().any(|p| remote::is_remote(p));
        let not_yet = |this: &mut Self, cx: &mut Context<Self>| {
            this.show_notice("That can't be done on a server yet.", cx);
            true
        };
        match command {
            Command::Open(entries) if entries.iter().any(|e| remote::is_remote(&e.path)) => {
                self.open_remote(entries.clone(), window, cx);
                true
            }
            Command::Trash(paths) | Command::DeleteImmediately(paths) if on_server(paths) => {
                // (A server has no Trash: deleted, once asked.)
                self.overlay = Some(super::sheets::Sheet::DeleteNow(paths.clone()));
                cx.notify();
                true
            }
            Command::NewFolder | Command::NewFile if here.is_some() => {
                let dir = self.cwd.clone();
                let folder = matches!(command, Command::NewFolder);
                cx.spawn(async move |this, cx| {
                    let made = cx.background_executor().spawn(async move { if folder { remote::new_folder(&dir) } else { remote::new_file(&dir) } }).await;
                    this.update(cx, |this, cx| match made {
                        Ok(path) => {
                            this.pending_select = vec![path.clone()];
                            this.pending_rename = Some(path.clone());
                            this.reload_servers(vec![this.cwd.clone()], cx);
                        }
                        Err(e) => this.show_notice(e, cx),
                    })
                    .ok();
                })
                .detach();
                true
            }
            Command::CopyPath(paths) if on_server(paths) => {
                let text: Vec<String> = paths.iter().filter_map(|p| remote::here(p)).map(|h| format!("{}:{}", h.server, h.path.display())).collect();
                crate::macos::copy_text(&text.join("\n"));
                true
            }
            Command::OpenIn(crate::tools::Kind::Terminal, paths) if on_server(paths) => {
                let Some(tool) = crate::tools::chosen(crate::tools::Kind::Terminal, Settings::get(cx)) else { return true };
                let at = paths.iter().find_map(|p| {
                    let entry = self.rows.iter().find(|e| e.path == *p);
                    let dir = if entry.is_some_and(|e| !e.is_dir) { p.parent()? } else { p };
                    remote::here(dir)
                });
                if let Some(at) = at
                    && let Err(e) = remote::open_terminal(&at, &tool.app)
                {
                    self.show_notice(e, cx);
                }
                true
            }
            // (These go on as ever: the window's own, or a stand-in's
            // path, which is what's wanted.)
            Command::OpenInTab(_)
            | Command::OpenInOtherPane(_)
            | Command::Rename(_)
            | Command::Copy(_)
            | Command::Cut(_)
            | Command::Paste
            | Command::MoveHere
            | Command::QuickLook
            | Command::AddToSidebar(_)
            | Command::RemoveFromSidebar(_)
            | Command::GoTo(_)
            | Command::ViewAs(_)
            | Command::SortBy(_)
            | Command::SortNone
            | Command::CleanUp(_)
            | Command::GroupBy(_)
            | Command::ViewOptions
            | Command::ToggleHidden
            | Command::EmptyTrash
            | Command::Eject(_) => false,
            Command::Open(_) => false,
            Command::TidyUp | Command::FolderRules if here.is_some() => not_yet(self, cx),
            Command::TidyUp | Command::FolderRules => false,
            Command::SetWallpaper(p) | Command::Uninstall(p) | Command::ShowOriginal(p) | Command::ShowInEnclosingFolder(p) | Command::ShowInFinder(p)
                if remote::is_remote(p) =>
            {
                not_yet(self, cx)
            }
            Command::OpenWith(paths, _)
            | Command::OpenIn(_, paths)
            | Command::PutBack(paths)
            | Command::RenameItems(paths)
            | Command::CustomizeFolders(paths)
            | Command::Compress(paths)
            | Command::Extract(paths, _)
            | Command::Duplicate(paths)
            | Command::MakeAlias(paths)
            | Command::Tag(paths, _, _)
            | Command::QuickAction(_, paths)
            | Command::Shortcut(_, paths)
            | Command::Download(paths)
            | Command::RemoveDownload(paths)
            | Command::NewFolderWith(paths)
                if on_server(paths) =>
            {
                not_yet(self, cx)
            }
            Command::GetInfo(entries) if entries.iter().any(|e| remote::is_remote(&e.path)) => not_yet(self, cx),
            _ => false,
        }
    }

    /// Server folders changed (by this window): every tab showing one lists
    /// it again (a server sends no word of changes). Once this window's
    /// done being updated.
    fn reload_servers(&mut self, dirs: Vec<PathBuf>, cx: &mut Context<Self>) {
        cx.defer(move |cx| crate::browser::reload_everywhere(dirs, cx));
    }

    /// Delete server items, once asked (see `Sheet::DeleteNow`).
    pub(super) fn delete_remote(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        let dirs: Vec<PathBuf> = paths.iter().filter_map(|p| p.parent().map(Into::into)).collect();
        cx.spawn(async move |this, cx| {
            let done = cx.background_executor().spawn(async move { remote::delete(&paths) }).await;
            this.update(cx, |this, cx| {
                if let Err(e) = done {
                    this.show_notice(e, cx);
                }
                this.reload_servers(dirs.clone(), cx);
            })
            .ok();
        })
        .detach();
    }

    /// Rename a server's item.
    pub(super) fn rename_remote(&mut self, old: PathBuf, name: String, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let from = old.clone();
            let done = cx.background_executor().spawn(async move { remote::rename(&from, &name) }).await;
            this.update(cx, |this, cx| match done {
                Ok(new) => {
                    this.pending_select = vec![new.clone()];
                    this.keep_place(&old, &new);
                    this.reload_servers(super::menu::parents([new]), cx);
                }
                Err(e) => this.show_notice(e, cx),
            })
            .ok();
        })
        .detach();
    }

    /// Copy (or move) to or from a server: up from the Mac, down to it, or
    /// from one server's folder to another. Names already taken where they
    /// go are left out, and said so (nothing's replaced without asking).
    pub(super) fn transfer_remote(&mut self, paths: Vec<PathBuf>, dir: PathBuf, how: How, cx: &mut Context<Self>) {
        let up = remote::is_remote(&dir);
        let from_server = paths.iter().all(|p| remote::is_remote(p));
        if !from_server && paths.iter().any(|p| remote::is_remote(p)) {
            return self.show_notice("Copy from the server and from the Mac one at a time.", cx);
        }
        // (Moved within one server: renamed there.)
        let same_server = up && from_server && paths.iter().all(|p| remote::here(p).map(|h| h.dest) == remote::here(&dir).map(|h| h.dest));
        let moving = how == How::Move;
        let n = paths.len();
        let what = if n == 1 { format!("“{}”", paths[0].file_name().unwrap_or_default().to_string_lossy()) } else { count(n, "item") };
        let label = format!("{} {what} to “{}”", if moving { "Moving" } else { "Copying" }, super::display_name(&dir));
        let progress = Arc::new(Progress::default());
        self.next_job += 1;
        let id = self.next_job;
        self.jobs.push(super::transfer::Job { id, label: label.into(), progress: progress.clone(), started: Instant::now() });
        self.tick_jobs(cx);
        let watched = progress.clone();
        let into = dir.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let taken = remote::taken(&paths, &into)?;
                    let go: Vec<PathBuf> = paths.iter().filter(|p| !taken.contains(p)).cloned().collect();
                    let made = match (from_server, up) {
                        _ if same_server && moving => remote::move_within(&go, &into)?,
                        (true, true) => remote::server_to_server(&go, &into, &progress)?,
                        (true, false) => remote::download(&go, &into, &progress)?,
                        (false, _) => remote::upload(&go, &into, &progress)?,
                    };
                    // (Moved: what was copied goes from where it was. On a
                    // server, deleted; on the Mac, to the Trash.)
                    if moving && !(same_server) {
                        if from_server {
                            remote::delete(&go)?;
                        } else {
                            if let Some(e) = crate::macos::trash_all(&go).into_iter().find_map(Result::err) {
                                return Err(format!("Copied, but couldn't move the originals to the Trash: {e}"));
                            }
                        }
                    }
                    Ok::<_, String>((made, taken))
                })
                .await;
            this.update(cx, |this, cx| {
                this.jobs.retain(|j| j.id != id);
                match result {
                    Ok((made, taken)) => {
                        if !taken.is_empty() {
                            let names: Vec<String> = taken.iter().map(|p| format!("“{}”", p.file_name().unwrap_or_default().to_string_lossy())).collect();
                            this.show_notice(format!("Left out, as the name's taken there: {}", names.join(", ")), cx);
                        }
                        this.select_when_listed(made);
                    }
                    Err(e) if e == "Stopped." => {
                        let (done, total) = (watched.done.load(Ordering::Relaxed), watched.total.load(Ordering::Relaxed));
                        this.show_notice(format!("Stopped: {} of {} copied.", crate::listing::human_size(done.min(total)), crate::listing::human_size(total)), cx);
                    }
                    Err(e) => this.show_notice(format!("Couldn't copy: {e}"), cx),
                }
                this.reload_servers(vec![dir.clone()], cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Space on a server's file: copied here first (a big one isn't), then
    /// shown.
    pub(super) fn peek_remote(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(entry) = self.selected_entry().cloned() else { return false };
        if !remote::is_remote(&entry.path) || entry.is_dir {
            return false;
        }
        if entry.size > 64 << 20 {
            self.show_notice(format!("“{}” is too big to look at from the server: open it instead.", entry.name), cx);
            return true;
        }
        let name: SharedString = entry.name.clone();
        self.fetch_then(entry.path, cx, move |this, _, cx| {
            // (Still the one selected: shown.)
            if this.selected_entry().is_some_and(|e| e.name == name) {
                this.toggle_preview_now(cx);
            }
        });
        true
    }
}
