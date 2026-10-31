//! The window's actions and their keys, as Finder's, and what they do.

use std::path::PathBuf;

use gpui::*;
use gpui_kit::component::input::{Escape, MoveDown, MoveUp};

use super::menu::Command;
use super::places::Virtual;
use super::{Finder, View, home, icloud};
use crate::fileops;
use crate::macos;
use crate::settings::Settings;

actions!(
    finder,
    [
        GoBack,
        GoForward,
        GoUp,
        OpenSelected,
        RevealSelected,
        SelectNext,
        SelectPrev,
        SelectLeft,
        SelectRight,
        SelectFirst,
        SelectLast,
        PageUp,
        PageDown,
        ExpandAll,
        CollapseAll,
        TogglePreview,
        ViewAsIcons,
        ViewAsList,
        ViewAsColumns,
    ViewAsGallery,
        FocusSearch,
    QuickOpen,
    OpenInTerminal,
    OpenInNotchTerminal,
    OpenInEditor,
        SearchTips,
        ClearSearch,
        ToggleHidden,
        SelectAll,
        ExtendNext,
        ExtendPrev,
        Undo,
        Redo,
        MoveItemsHere,
        MoveToTrash,
        DeleteImmediately,
        EmptyTrash,
        Duplicate,
        GetInfo,
        ShowInspector,
        CopyItem,
        CutItems,
        CopyPathname,
        PasteItems,
        NewFolder,
        NewFile,
        NewFolderWithSelection,
        AddToSidebar,
        Eject,
        RenameSelected,
        OpenSettings,
        TogglePathBar,
        ToggleStatusBar,
        TogglePreviewPane,
        ToggleSidebar,
        ShowViewOptions,
    TidyUp,
        CustomizeToolbar,
        GoRecents,
        GoComputer,
        GoAirDrop,
        GoNetwork,
        GoICloud,
        GoUtilities,
        GoLibrary,
        GoTrash,
        GoToFolder,
        ConnectToServer,
        GoHome,
        GoDesktop,
        GoDocuments,
        GoDownloads,
        GoApplications,
        Tag1,
        Tag2,
        Tag3,
        Tag4,
        Tag5,
        Tag6,
        Tag7,
        ClearTags,
    ]
);

pub const CONTEXT: &str = "Finder";

pub fn bind_keys(cx: &mut App) {
    let ctx = Some(CONTEXT);
    cx.bind_keys([
        KeyBinding::new("cmd-[", GoBack, ctx),
        KeyBinding::new("cmd-]", GoForward, ctx),
        KeyBinding::new("cmd-up", GoUp, ctx),
        KeyBinding::new("cmd-down", OpenSelected, ctx),
        KeyBinding::new("cmd-o", OpenSelected, ctx),
        KeyBinding::new("cmd-p", QuickOpen, ctx),
        KeyBinding::new("cmd-alt-t", OpenInTerminal, ctx),
        KeyBinding::new("cmd-alt-e", OpenInEditor, ctx),
        // Return renames, as in Finder; ⌘O and ⌘↓ open.
        KeyBinding::new("enter", RenameSelected, ctx),
        KeyBinding::new("cmd-r", RevealSelected, ctx),
        KeyBinding::new("down", SelectNext, ctx),
        KeyBinding::new("up", SelectPrev, ctx),
        KeyBinding::new("left", SelectLeft, ctx),
        KeyBinding::new("right", SelectRight, ctx),
        KeyBinding::new("home", SelectFirst, ctx),
        KeyBinding::new("end", SelectLast, ctx),
        KeyBinding::new("alt-up", SelectFirst, ctx),
        KeyBinding::new("alt-down", SelectLast, ctx),
        KeyBinding::new("pageup", PageUp, ctx),
        KeyBinding::new("pagedown", PageDown, ctx),
        KeyBinding::new("alt-right", ExpandAll, ctx),
        KeyBinding::new("alt-left", CollapseAll, ctx),
        KeyBinding::new("shift-down", ExtendNext, ctx),
        KeyBinding::new("shift-up", ExtendPrev, ctx),
        KeyBinding::new("cmd-a", SelectAll, ctx),
        KeyBinding::new("cmd-z", Undo, ctx),
        KeyBinding::new("cmd-shift-z", Redo, ctx),
        KeyBinding::new("cmd-alt-v", MoveItemsHere, ctx),
        KeyBinding::new("cmd-y", TogglePreview, ctx),
        KeyBinding::new("cmd-1", ViewAsIcons, ctx),
        KeyBinding::new("cmd-2", ViewAsList, ctx),
        KeyBinding::new("cmd-3", ViewAsColumns, ctx),
        KeyBinding::new("cmd-4", ViewAsGallery, ctx),
        KeyBinding::new("cmd-f", FocusSearch, ctx),
        KeyBinding::new("escape", ClearSearch, ctx),
        // ⌘⇧. arrives with the shift folded into the key.
        KeyBinding::new("cmd->", ToggleHidden, ctx),
        KeyBinding::new("cmd-backspace", MoveToTrash, ctx),
        KeyBinding::new("cmd-alt-backspace", DeleteImmediately, ctx),
        KeyBinding::new("cmd-shift-backspace", EmptyTrash, ctx),
        KeyBinding::new("cmd-d", Duplicate, ctx),
        KeyBinding::new("cmd-i", GetInfo, ctx),
        KeyBinding::new("cmd-alt-i", ShowInspector, ctx),
        KeyBinding::new("cmd-c", CopyItem, ctx),
        KeyBinding::new("cmd-x", CutItems, ctx),
        KeyBinding::new("cmd-alt-c", CopyPathname, ctx),
        KeyBinding::new("cmd-v", PasteItems, ctx),
        KeyBinding::new("cmd-shift-n", NewFolder, ctx),
        KeyBinding::new("cmd-alt-n", NewFile, ctx),
        KeyBinding::new("ctrl-cmd-n", NewFolderWithSelection, ctx),
        KeyBinding::new("ctrl-cmd-t", AddToSidebar, ctx),
        KeyBinding::new("cmd-e", Eject, ctx),
        KeyBinding::new("cmd-,", OpenSettings, None),
        KeyBinding::new("cmd-alt-p", TogglePathBar, ctx),
        KeyBinding::new("cmd-/", ToggleStatusBar, ctx),
        KeyBinding::new("cmd-shift-p", TogglePreviewPane, ctx),
        KeyBinding::new("ctrl-cmd-s", ToggleSidebar, ctx),
        KeyBinding::new("cmd-j", ShowViewOptions, ctx),
        KeyBinding::new("cmd-shift-f", GoRecents, ctx),
        KeyBinding::new("cmd-shift-c", GoComputer, ctx),
        KeyBinding::new("cmd-shift-r", GoAirDrop, ctx),
        KeyBinding::new("cmd-shift-k", GoNetwork, ctx),
        KeyBinding::new("cmd-shift-i", GoICloud, ctx),
        KeyBinding::new("cmd-shift-u", GoUtilities, ctx),
        KeyBinding::new("cmd-shift-l", GoLibrary, ctx),
        KeyBinding::new("cmd-shift-g", GoToFolder, ctx),
        KeyBinding::new("cmd-k", ConnectToServer, ctx),
        KeyBinding::new("cmd-shift-h", GoHome, ctx),
        KeyBinding::new("cmd-shift-d", GoDesktop, ctx),
        KeyBinding::new("cmd-shift-o", GoDocuments, ctx),
        KeyBinding::new("cmd-alt-l", GoDownloads, ctx),
        KeyBinding::new("cmd-shift-a", GoApplications, ctx),
        KeyBinding::new("ctrl-1", Tag1, ctx),
        KeyBinding::new("ctrl-2", Tag2, ctx),
        KeyBinding::new("ctrl-3", Tag3, ctx),
        KeyBinding::new("ctrl-4", Tag4, ctx),
        KeyBinding::new("ctrl-5", Tag5, ctx),
        KeyBinding::new("ctrl-6", Tag6, ctx),
        KeyBinding::new("ctrl-7", Tag7, ctx),
        KeyBinding::new("ctrl-0", ClearTags, ctx),
    ]);
}

impl Finder {
    /// Wire the window's actions and keys onto its root.
    pub(super) fn with_actions(&self, root: Div, cx: &mut Context<Self>) -> Div {
        // Run only with the files focused: in a text field (the search, a
        // name being edited) these keys are the field's.
        fn files<A: Action>(
            cx: &mut Context<Finder>,
            f: impl Fn(&mut Finder, &mut Window, &mut Context<Finder>) + 'static,
        ) -> impl Fn(&A, &mut Window, &mut App) + 'static {
            cx.listener(move |this, _: &A, window, cx| {
                if this.focus.is_focused(window) {
                    f(this, window, cx)
                }
            })
        }
        root.on_action(cx.listener(Self::go_back))
            .on_action(cx.listener(Self::go_forward))
            .on_action(cx.listener(Self::go_up))
            .on_action(cx.listener(Self::toggle_hidden))
            .on_action(files::<OpenSelected>(cx, |this, window, cx| this.open_selected(window, cx)))
            // ⌘R: an alias's original; in search results, where the result
            // is; else show it in Finder itself.
            .on_action(cx.listener(|this, _: &RevealSelected, window, cx| {
                let entry = this.selected_entry().cloned();
                let command = match entry {
                    Some(e) if e.alias => Command::ShowOriginal(e.path),
                    Some(e) if this.flat() => Command::ShowInEnclosingFolder(e.path),
                    Some(e) => Command::ShowInFinder(e.path),
                    None => Command::ShowInFinder(this.cwd.clone()),
                };
                this.run(command, window, cx);
            }))
            .on_action(files::<MoveToTrash>(cx, |this, window, cx| this.on_selected(Command::Trash, window, cx)))
            .on_action(files::<DeleteImmediately>(cx, |this, window, cx| {
                this.on_selected(Command::DeleteImmediately, window, cx)
            }))
            .on_action(cx.listener(|this, _: &EmptyTrash, _, cx| this.ask_empty_trash(cx)))
            .on_action(files::<Duplicate>(cx, |this, window, cx| this.on_selected(Command::Duplicate, window, cx)))
            .on_action(cx.listener(|this, _: &GetInfo, window, cx| {
                let mut entries = this.selection();
                if entries.is_empty() {
                    entries.push(super::menu::folder_entry(&this.cwd));
                }
                this.run(Command::GetInfo(entries), window, cx);
            }))
            .on_action(cx.listener(|this, _: &ShowInspector, _, cx| crate::info::toggle_inspector(this.selection(), cx)))
            .on_action(files::<CopyItem>(cx, |this, window, cx| this.on_selected(Command::Copy, window, cx)))
            .on_action(files::<CutItems>(cx, |this, window, cx| this.on_selected(Command::Cut, window, cx)))
            .on_action(files::<CopyPathname>(cx, |this, window, cx| this.on_selected(Command::CopyPath, window, cx)))
            .on_action(files::<MoveItemsHere>(cx, |this, window, cx| {
                if !this.flat() {
                    this.run(Command::MoveHere, window, cx)
                }
            }))
            .on_action(files::<SelectAll>(cx, |this, _, cx| this.select_all(cx)))
            .on_action(files::<ExtendNext>(cx, |this, _, cx| this.extend_by(1, cx)))
            .on_action(files::<ExtendPrev>(cx, |this, _, cx| this.extend_by(-1, cx)))
            .on_action(files::<Undo>(cx, |this, _, cx| this.undo(false, cx)))
            .on_action(files::<Redo>(cx, |this, _, cx| this.undo(true, cx)))
            .on_action(files::<PasteItems>(cx, |this, window, cx| {
                if !this.flat() {
                    this.run(Command::Paste, window, cx)
                }
            }))
            .on_action(cx.listener(|this, _: &NewFolder, window, cx| {
                if !this.flat() {
                    this.run(Command::NewFolder, window, cx)
                }
            }))
            .on_action(cx.listener(|this, _: &NewFile, window, cx| {
                if !this.flat() && !this.in_trash() {
                    this.run(Command::NewFile, window, cx)
                }
            }))
            .on_action(files::<NewFolderWithSelection>(cx, |this, window, cx| {
                if !this.flat() {
                    this.on_selected(Command::NewFolderWith, window, cx)
                }
            }))
            .on_action(cx.listener(|this, _: &AddToSidebar, _, cx| {
                let folders: Vec<PathBuf> = this.selection().into_iter().filter(|e| e.is_dir).map(|e| e.path).collect();
                let folders = if folders.is_empty() { vec![this.cwd.clone()] } else { folders };
                this.add_to_sidebar(folders, cx);
            }))
            // ⌘E: the selected volume, or the one the folder is on.
            .on_action(cx.listener(|this, _: &Eject, window, cx| {
                let selected = this.selected_entry().map(|e| e.path.clone()).filter(|p| macos::volume_flags(p).1);
                let current =
                    this.locations.iter().filter(|p| p.ejectable).filter_map(|p| p.folder()).find(|p| this.cwd.starts_with(p)).cloned();
                if let Some(volume) = selected.or(current) {
                    this.run(Command::Eject(volume), window, cx);
                }
            }))
            .on_action(files::<SelectNext>(cx, |this, _, cx| this.select_row(1, cx)))
            .on_action(files::<SelectPrev>(cx, |this, _, cx| this.select_row(-1, cx)))
            .on_action(files::<SelectLeft>(cx, |this, window, cx| this.select_side(false, window, cx)))
            .on_action(files::<SelectRight>(cx, |this, window, cx| this.select_side(true, window, cx)))
            .on_action(files::<SelectFirst>(cx, |this, _, cx| this.select_end(false, cx)))
            .on_action(files::<SelectLast>(cx, |this, _, cx| this.select_end(true, cx)))
            .on_action(files::<PageUp>(cx, |this, window, cx| this.page(-1, window, cx)))
            .on_action(files::<PageDown>(cx, |this, window, cx| this.page(1, window, cx)))
            .on_action(files::<ExpandAll>(cx, |this, _, cx| this.expand_all(true, cx)))
            .on_action(files::<CollapseAll>(cx, |this, _, cx| this.expand_all(false, cx)))
            .on_action(cx.listener(|this, _: &TogglePreview, _, cx| this.toggle_preview(cx)))
            .on_action(cx.listener(|this, _: &ViewAsIcons, _, cx| this.set_view(View::Icons, cx)))
            .on_action(cx.listener(|this, _: &ViewAsList, _, cx| this.set_view(View::List, cx)))
            .on_action(cx.listener(|this, _: &ViewAsColumns, _, cx| this.set_view(View::Columns, cx)))
            .on_action(cx.listener(|this, _: &ViewAsGallery, _, cx| this.set_view(View::Gallery, cx)))
            .on_action(cx.listener(|_, _: &QuickOpen, window, cx| {
                // (Floating, over this window: folders open here. Once this
                // tab's done being updated: Quick Open reads it.)
                let origin = (window.window_handle(), cx.entity().downgrade());
                cx.defer(move |cx| crate::quick_panel::show(Some(origin), cx));
            }))
            .on_action(cx.listener(|this, _: &OpenInTerminal, _, cx| this.open_in(crate::tools::Kind::Terminal, cx)))
            .on_action(cx.listener(|this, _: &OpenInNotchTerminal, _, cx| this.open_in_notch_terminal(cx)))
            .on_action(cx.listener(|this, _: &OpenInEditor, _, cx| this.open_in(crate::tools::Kind::Editor, cx)))
            .on_action(cx.listener(|this, _: &FocusSearch, window, cx| this.focus_search(window, cx)))
            .on_action(cx.listener(|this, _: &SearchTips, window, cx| this.show_search_tips(window, cx)))
            .on_action(cx.listener(|this, _: &ClearSearch, window, cx| this.escape(window, cx)))
            .on_action(cx.listener(|this, _: &GoHome, window, cx| this.go_place(home(), window, cx)))
            .on_action(cx.listener(|this, _: &GoDesktop, window, cx| this.go_place(home().join("Desktop"), window, cx)))
            .on_action(cx.listener(|this, _: &GoDocuments, window, cx| this.go_place(home().join("Documents"), window, cx)))
            .on_action(cx.listener(|this, _: &GoDownloads, window, cx| this.go_place(home().join("Downloads"), window, cx)))
            .on_action(cx.listener(|this, _: &GoApplications, window, cx| this.go_place("/Applications".into(), window, cx)))
            .on_action(cx.listener(|this, _: &GoUtilities, window, cx| this.go_place("/Applications/Utilities".into(), window, cx)))
            .on_action(cx.listener(|this, _: &GoLibrary, window, cx| this.go_place(home().join("Library"), window, cx)))
            // Finder's Computer lists the volumes.
            .on_action(cx.listener(|this, _: &GoComputer, window, cx| this.go_place("/Volumes".into(), window, cx)))
            .on_action(cx.listener(|this, _: &GoICloud, window, cx| this.go_place(icloud(), window, cx)))
            .on_action(cx.listener(|this, _: &GoTrash, window, cx| this.go_virtual(Virtual::Trash, window, cx)))
            .on_action(cx.listener(|this, _: &GoRecents, window, cx| this.go_virtual(Virtual::Recents, window, cx)))
            .on_action(cx.listener(|this, _: &GoNetwork, window, cx| this.go_virtual(Virtual::Network, window, cx)))
            .on_action(cx.listener(|this, _: &GoAirDrop, window, cx| this.go_virtual(Virtual::AirDrop, window, cx)))
            .on_action(cx.listener(|this, _: &GoToFolder, window, cx| this.open_go_to_folder(window, cx)))
            .on_action(cx.listener(|this, _: &ConnectToServer, window, cx| this.open_connect(window, cx)))
            .on_action(cx.listener(|this, _: &TidyUp, _, cx| this.tidy_up(cx)))
            .on_action(cx.listener(|this, _: &ShowViewOptions, _, cx| this.toggle_sheet(super::sheets::Sheet::ViewOptions, cx)))
            .on_action(cx.listener(|this, _: &CustomizeToolbar, _, cx| this.toggle_sheet(super::sheets::Sheet::Toolbar, cx)))
            .on_action(cx.listener(|this, _: &RenameSelected, window, cx| {
                // Return answers Finder's question: Replace.
                if this.conflicts.is_some() {
                    return this.resolve(Some(fileops::Resolve::Replace), cx);
                }
                if this.confirm_sheet(cx) {
                    return;
                }
                if !this.focus.is_focused(window) {
                    return;
                }
                // Only one selected item, the one the keyboard is on; or
                // several, all at once.
                if this.chosen.len() > 1 {
                    let paths = this.selection_paths();
                    this.open_rename_items(paths, window, cx);
                } else if this.chosen.len() == 1
                    && let Some(entry) = this.selected_entry().filter(|e| this.chosen.contains(&e.path))
                {
                    let path = entry.path.clone();
                    this.start_rename(path, window, cx);
                }
            }))
            .on_action(cx.listener(|_, _: &TogglePathBar, _, cx| Settings::update(cx, |s| s.show_path_bar = !s.show_path_bar)))
            .on_action(cx.listener(|_, _: &ToggleStatusBar, _, cx| Settings::update(cx, |s| s.show_status_bar = !s.show_status_bar)))
            .on_action(cx.listener(|_, _: &TogglePreviewPane, _, cx| {
                Settings::update(cx, |s| s.show_preview_pane = !s.show_preview_pane)
            }))
            .on_action(cx.listener(|_, _: &ToggleSidebar, _, cx| Settings::update(cx, |s| s.show_sidebar = !s.show_sidebar)))
            .on_action(files::<Tag1>(cx, |this, _, cx| this.tag_shortcut(0, cx)))
            .on_action(files::<Tag2>(cx, |this, _, cx| this.tag_shortcut(1, cx)))
            .on_action(files::<Tag3>(cx, |this, _, cx| this.tag_shortcut(2, cx)))
            .on_action(files::<Tag4>(cx, |this, _, cx| this.tag_shortcut(3, cx)))
            .on_action(files::<Tag5>(cx, |this, _, cx| this.tag_shortcut(4, cx)))
            .on_action(files::<Tag6>(cx, |this, _, cx| this.tag_shortcut(5, cx)))
            .on_action(files::<Tag7>(cx, |this, _, cx| this.tag_shortcut(6, cx)))
            .on_action(files::<ClearTags>(cx, |this, _, cx| this.clear_tags(cx)))
            // Arrow keys and escape in the search field drive the files (not
            // while renaming: those keys are the name field's then).
            .capture_action(cx.listener(|this, _: &MoveDown, _, cx| {
                if this.renaming.is_none() && this.overlay.is_none() {
                    this.select_row(1, cx);
                    cx.stop_propagation();
                }
            }))
            .capture_action(cx.listener(|this, _: &MoveUp, _, cx| {
                if this.renaming.is_none() && this.overlay.is_none() {
                    this.select_row(-1, cx);
                    cx.stop_propagation();
                }
            }))
            .capture_action(cx.listener(|this, _: &Escape, window, cx| {
                this.escape(window, cx);
                cx.stop_propagation();
            }))
            // Space looks; typing selects by name (or searches, if asked).
            // Only with the files focused, not a text field.
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let m = &event.keystroke.modifiers;
                if m.platform || m.control || m.alt || m.function || !this.focus.is_focused(window) {
                    return;
                }
                if event.keystroke.key == "space" {
                    this.toggle_preview(cx);
                    cx.stop_propagation();
                    return;
                }
                let Some(text) = event.keystroke.key_char.clone().filter(|c| !c.trim().is_empty() && !c.chars().any(char::is_control))
                else {
                    return;
                };
                cx.stop_propagation();
                if Settings::get(cx).typing_searches {
                    this.query.update(cx, |q, cx| {
                        q.set_value(text, window, cx);
                        q.focus(window, cx);
                    });
                    this.run_search(cx);
                } else {
                    this.type_select(&text, cx);
                }
            }))
    }
}
