//! The menu bar, laid out as Finder's: the app's, File, Edit, View, Go,
//! Window, Help. Items send the same actions as the keyboard shortcuts, to the
//! window in front; macOS shows each one's shortcut from the key bindings
//! (as Settings may have changed them: see `keymap`).

use gpui::*;

use crate::about::{About, Acknowledgements, CheckForUpdates, ShowLog};
use crate::{CloseAllWindows, OpenDiskUsage, Quit, ShowWelcome};
use crate::browser::{CloseTab, CopyToOtherPane, MoveToOtherPane, NewTab, NewWindow, NextTab, OpenInOtherPane, PrevTab, SwapPanes, SwitchPane, ToggleDualPane};
use crate::finder::*;
use crate::macos::bin;

actions!(menus, [Minimize, Zoom, HideApp, HideOthers, ShowAll]);

pub fn set(cx: &mut App) {
    cx.on_action(|_: &Minimize, cx| with_window(cx, |window| window.minimize_window()));
    cx.on_action(|_: &Zoom, cx| with_window(cx, |window| window.zoom_window()));
    cx.on_action(|_: &HideApp, cx| cx.hide());
    cx.on_action(|_: &HideOthers, cx| cx.hide_other_apps());
    cx.on_action(|_: &ShowAll, cx| cx.unhide_other_apps());
    cx.bind_keys([
        KeyBinding::new("cmd-m", Minimize, None),
        KeyBinding::new("cmd-h", HideApp, None),
        KeyBinding::new("cmd-alt-h", HideOthers, None),
    ]);
    menu_bar(cx);
}

/// The menu bar (again, when keys change: it shows them as they're bound).
pub fn menu_bar(cx: &mut App) {
    cx.set_menus(vec![
        Menu::new(crate::APP_NAME).items([
            MenuItem::action(format!("About {}", crate::APP_NAME), About),
            MenuItem::action("Check for Updates…", CheckForUpdates),
            MenuItem::separator(),
            MenuItem::action(format!("Welcome to {}…", crate::APP_NAME), ShowWelcome),
            MenuItem::separator(),
            MenuItem::action("Settings…", OpenSettings),
            MenuItem::action(format!("Empty {}", bin()), EmptyTrash),
            MenuItem::separator(),
            MenuItem::os_submenu("Services", SystemMenuType::Services),
            MenuItem::separator(),
            MenuItem::action(format!("Hide {}", crate::APP_NAME), HideApp),
            MenuItem::action("Hide Others", HideOthers),
            MenuItem::action("Show All", ShowAll),
            MenuItem::separator(),
            MenuItem::action("Close All Windows", CloseAllWindows),
            MenuItem::action(format!("Quit {}", crate::APP_NAME), Quit),
        ]),
        Menu::new("File").items([
            MenuItem::action("New Window", NewWindow),
            MenuItem::action("New Folder", NewFolder),
            MenuItem::action("New Folder with Selection", NewFolderWithSelection),
            MenuItem::action("New File", NewFile),
            MenuItem::action("New Tab", NewTab),
            MenuItem::action("Open", OpenSelected),
            MenuItem::action("Open in Terminal", OpenInTerminal),
            MenuItem::action("Open in Editor", OpenInEditor),
            MenuItem::action("Close Window", CloseTab),
            MenuItem::separator(),
            MenuItem::action("Get Info", GetInfo),
            MenuItem::action("Show Inspector", ShowInspector),
            MenuItem::action("Rename", RenameSelected),
            MenuItem::separator(),
            MenuItem::action("Duplicate", Duplicate),
            MenuItem::action("Copy to Other Pane", CopyToOtherPane),
            MenuItem::action("Move to Other Pane", MoveToOtherPane),
            MenuItem::action("Quick Look", TogglePreview),
            MenuItem::action("Show Original", RevealSelected),
            MenuItem::action("Add to Sidebar", AddToSidebar),
            MenuItem::separator(),
            MenuItem::action(format!("Move to {}", bin()), MoveToTrash),
            MenuItem::action("Delete Immediately…", DeleteImmediately),
            MenuItem::action("Eject", Eject),
            MenuItem::separator(),
            MenuItem::action("Find", FocusSearch),
            MenuItem::action("Quick Open…", QuickOpen),
        ]),
        Menu::new("Edit").items([
            MenuItem::os_action("Undo", Undo, OsAction::Undo),
            MenuItem::os_action("Redo", Redo, OsAction::Redo),
            MenuItem::separator(),
            MenuItem::os_action("Cut", CutItems, OsAction::Cut),
            MenuItem::os_action("Copy", CopyItem, OsAction::Copy),
            MenuItem::os_action("Paste", PasteItems, OsAction::Paste),
            MenuItem::action("Move Item Here", MoveItemsHere),
            MenuItem::action("Copy as Pathname", CopyPathname),
            MenuItem::os_action("Select All", SelectAll, OsAction::SelectAll),
        ]),
        Menu::new("View").items([
            MenuItem::action("as Icons", ViewAsIcons),
            MenuItem::action("as List", ViewAsList),
            MenuItem::action("as Columns", ViewAsColumns),
            MenuItem::action("as Gallery", ViewAsGallery),
            MenuItem::separator(),
            MenuItem::separator(),
            MenuItem::action("Show or Hide Sidebar", ToggleSidebar),
            MenuItem::action("Show or Hide Dual Pane", ToggleDualPane),
            MenuItem::action("Switch to Other Pane", SwitchPane),
            MenuItem::action("Swap Panes", SwapPanes),
            MenuItem::action("Open in Other Pane", OpenInOtherPane),
            MenuItem::action("Show or Hide Path Bar", TogglePathBar),
            MenuItem::action("Show or Hide Status Bar", ToggleStatusBar),
            MenuItem::action("Show or Hide Preview", TogglePreviewPane),
            MenuItem::separator(),
            MenuItem::action("Show or Hide Hidden Files", ToggleHidden),
            MenuItem::action("Customize Toolbar…", CustomizeToolbar),
            MenuItem::action("Tidy Up", TidyUp),
            MenuItem::action("Show View Options", ShowViewOptions),
        ]),
        Menu::new("Go").items([
            MenuItem::action("Back", GoBack),
            MenuItem::action("Forward", GoForward),
            MenuItem::action("Enclosing Folder", GoUp),
            MenuItem::separator(),
            MenuItem::action("Recents", GoRecents),
            MenuItem::action("Documents", GoDocuments),
            MenuItem::action("Desktop", GoDesktop),
            MenuItem::action("Downloads", GoDownloads),
            MenuItem::action("Home", GoHome),
            MenuItem::action("Computer", GoComputer),
            MenuItem::action("AirDrop", GoAirDrop),
            MenuItem::action("Network", GoNetwork),
            MenuItem::action("iCloud Drive", GoICloud),
            MenuItem::action("Applications", GoApplications),
            MenuItem::action("Utilities", GoUtilities),
            MenuItem::action("Library", GoLibrary),
            MenuItem::action(bin(), GoTrash),
            MenuItem::separator(),
            MenuItem::action("Go to Folder…", GoToFolder),
            MenuItem::action("Connect to Server…", ConnectToServer),
        ]),
        Menu::new("Window").items([
            MenuItem::action("Minimize", Minimize),
            MenuItem::action("Zoom", Zoom),
            MenuItem::separator(),
            MenuItem::action("Disk Usage", OpenDiskUsage),
            MenuItem::separator(),
            MenuItem::action("Show Previous Tab", PrevTab),
            MenuItem::action("Show Next Tab", NextTab),
        ]),
        Menu::new("Help").items([
            MenuItem::action("Search Tips", SearchTips),
            MenuItem::action(format!("Welcome to {}…", crate::APP_NAME), ShowWelcome),
            MenuItem::separator(),
            MenuItem::action("Send Feedback…", crate::reports::SendFeedback),
            MenuItem::separator(),
            MenuItem::action("Acknowledgements", Acknowledgements),
            MenuItem::action("Show Log File", ShowLog),
        ]),
    ]);
}

fn with_window(cx: &mut App, f: impl FnOnce(&mut Window)) {
    if let Some(handle) = cx.active_window() {
        handle.update(cx, |_, window, _| f(window)).ok();
    }
}
