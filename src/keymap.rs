//! F2's own keys, in its windows and its menus: Finder's, unless Settings
//! change them (`window_keys`). Finder's are kept as F2 started with them;
//! the keymap is made again from them with the changes over them (and the
//! menu bar, which shows them, is set again).

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::OnceLock;

use gpui::*;

use crate::about::{About, CheckForUpdates};
use crate::browser::{CloseTab, CopyToOtherPane, MoveToOtherPane, NewTab, NewWindow, NextTab, OpenInOtherPane, PrevTab, SwapPanes, SwitchPane, ToggleDualPane};
use crate::finder::*;
use crate::menus::{HideApp, HideOthers, Minimize, Zoom};
use crate::settings::Settings;
use crate::{CloseAllWindows, OpenDiskUsage, ShowWelcome};

/// One of F2's keys that can be changed: the menu it's in, its title, what
/// it does.
pub struct Key {
    pub section: &'static str,
    pub title: String,
    pub action: Box<dyn Action>,
}

impl Key {
    pub fn name(&self) -> &'static str {
        self.action.name()
    }
}

/// Each, in the menu bar's order (and the tags', which are only keys).
pub fn all() -> Vec<Key> {
    let bin = crate::macos::bin();
    let mut keys = Vec::new();
    let mut add = |section: &'static str, items: Vec<(String, Box<dyn Action>)>| {
        keys.extend(items.into_iter().map(|(title, action)| Key { section, title, action }));
    };
    macro_rules! items {
        ($($title:expr => $action:expr),* $(,)?) => { vec![$(($title.to_string(), Box::new($action) as Box<dyn Action>)),*] };
    }
    add(
        crate::APP_NAME,
        items![
            format!("About {}", crate::APP_NAME) => About,
            "Check for Updates…" => CheckForUpdates,
            format!("Welcome to {}…", crate::APP_NAME) => ShowWelcome,
            "Settings…" => OpenSettings,
            format!("Empty {bin}") => EmptyTrash,
            format!("Hide {}", crate::APP_NAME) => HideApp,
            "Hide Others" => HideOthers,
            "Close All Windows" => CloseAllWindows,
        ],
    );
    add(
        "File",
        items![
            "New Window" => NewWindow,
            "New Folder" => NewFolder,
            "New Folder with Selection" => NewFolderWithSelection,
            "New File" => NewFile,
            "New Tab" => NewTab,
            "Open" => OpenSelected,
            "Open in Terminal" => OpenInTerminal,
            "Open in Editor" => OpenInEditor,
            "Close Window" => CloseTab,
            "Get Info" => GetInfo,
            "Show Inspector" => ShowInspector,
            "Rename" => RenameSelected,
            "Duplicate" => Duplicate,
            "Copy to Other Pane" => CopyToOtherPane,
            "Move to Other Pane" => MoveToOtherPane,
            "Quick Look" => TogglePreview,
            "Show Original" => RevealSelected,
            "Add to Sidebar" => AddToSidebar,
            format!("Move to {bin}") => MoveToTrash,
            "Delete Immediately…" => DeleteImmediately,
            "Eject" => Eject,
            "Find" => FocusSearch,
            "Quick Open…" => QuickOpen,
        ],
    );
    add(
        "Edit",
        items![
            "Undo" => Undo,
            "Redo" => Redo,
            "Cut" => CutItems,
            "Copy" => CopyItem,
            "Paste" => PasteItems,
            "Move Item Here" => MoveItemsHere,
            "Copy as Pathname" => CopyPathname,
            "Select All" => SelectAll,
        ],
    );
    add(
        "View",
        items![
            "as Icons" => ViewAsIcons,
            "as List" => ViewAsList,
            "as Columns" => ViewAsColumns,
            "as Gallery" => ViewAsGallery,
            "Show or Hide Sidebar" => ToggleSidebar,
            "Show or Hide Dual Pane" => ToggleDualPane,
            "Switch to Other Pane" => SwitchPane,
            "Swap Panes" => SwapPanes,
            "Open in Other Pane" => OpenInOtherPane,
            "Show or Hide Path Bar" => TogglePathBar,
            "Show or Hide Status Bar" => ToggleStatusBar,
            "Show or Hide Preview" => TogglePreviewPane,
            "Show or Hide Hidden Files" => ToggleHidden,
            "Customize Toolbar…" => CustomizeToolbar,
            "Tidy Up" => TidyUp,
            "Show View Options" => ShowViewOptions,
        ],
    );
    add(
        "Go",
        items![
            "Back" => GoBack,
            "Forward" => GoForward,
            "Enclosing Folder" => GoUp,
            "Recents" => GoRecents,
            "Documents" => GoDocuments,
            "Desktop" => GoDesktop,
            "Downloads" => GoDownloads,
            "Home" => GoHome,
            "Computer" => GoComputer,
            "AirDrop" => GoAirDrop,
            "Network" => GoNetwork,
            "iCloud Drive" => GoICloud,
            "Applications" => GoApplications,
            "Utilities" => GoUtilities,
            "Library" => GoLibrary,
            bin => GoTrash,
            "Go to Folder…" => GoToFolder,
            "Connect to Server…" => ConnectToServer,
        ],
    );
    add(
        "Window",
        items![
            "Minimize" => Minimize,
            "Zoom" => Zoom,
            "Disk Usage" => OpenDiskUsage,
            "Show Previous Tab" => PrevTab,
            "Show Next Tab" => NextTab,
        ],
    );
    let [red, orange, yellow, green, blue, purple, gray] = TAG_COLORS.map(|(name, _)| name);
    add(
        "Tags",
        items![
            red => Tag1,
            orange => Tag2,
            yellow => Tag3,
            green => Tag4,
            blue => Tag5,
            purple => Tag6,
            gray => Tag7,
            "No Tags" => ClearTags,
        ],
    );
    keys
}

/// Each's keys as F2 started with them (Finder's), by action.
struct Defaults(HashMap<&'static str, Vec<KeyBinding>>);

impl Global for Defaults {}

/// Those as text ("cmd-shift-n"), for Settings to show.
static DEFAULT_KEYS: OnceLock<HashMap<&'static str, Vec<String>>> = OnceLock::new();

/// All bindings, put aside while a key's recorded.
struct Aside(Vec<KeyBinding>);

impl Global for Aside {}

/// Keep Finder's keys, and put Settings' changes over them (again when
/// they change). After every other key's bound.
pub fn init(cx: &mut App) {
    let names: HashSet<&'static str> = all().iter().map(Key::name).collect();
    let mut defaults: HashMap<&'static str, Vec<KeyBinding>> = HashMap::new();
    for binding in cx.key_bindings().borrow().bindings() {
        let name = binding.action().name();
        if names.contains(name) {
            defaults.entry(name).or_default().push(binding.clone());
        }
    }
    let _ = DEFAULT_KEYS.set(defaults.iter().map(|(name, bindings)| (*name, bindings.iter().map(text).collect())).collect());
    cx.set_global(Defaults(defaults));
    let mut applied = None;
    let mut apply_changed = move |cx: &mut App| {
        let wanted = Settings::get(cx).window_keys.clone();
        if applied.as_ref() != Some(&wanted) && cx.try_global::<Aside>().is_none() {
            // (Nothing changed yet: nothing to do.)
            if applied.is_some() || !wanted.is_empty() {
                apply(cx);
            }
            applied = Some(wanted);
        }
    };
    apply_changed(cx);
    cx.observe_global::<Settings>(apply_changed).detach();
}

/// A binding's keys as text: "cmd-shift-n".
fn text(binding: &KeyBinding) -> String {
    binding.keystrokes().iter().map(|k| k.unparse()).collect::<Vec<_>>().join(" ")
}

/// Its keys as F2 started.
pub fn default_keys(name: &str) -> Vec<String> {
    DEFAULT_KEYS.get().and_then(|d| d.get(name)).cloned().unwrap_or_default()
}

/// Its keys now: Settings' (none, maybe), else Finder's.
pub fn keys(name: &str, settings: &Settings) -> Vec<String> {
    settings.window_keys.get(name).cloned().unwrap_or_else(|| default_keys(name))
}

/// Bind F2's keys as Settings have them, the rest as they are; and the
/// menu bar again, to show them.
fn apply(cx: &mut App) {
    let table = all();
    let changed = Settings::get(cx).window_keys.clone();
    let ours: HashSet<&'static str> = table.iter().map(Key::name).collect();
    let mut bindings: Vec<KeyBinding> = cx.key_bindings().borrow().bindings().filter(|b| !ours.contains(b.action().name())).cloned().collect();
    let defaults = &cx.global::<Defaults>().0;
    for key in &table {
        let name = key.name();
        let had = defaults.get(name).map(Vec::as_slice).unwrap_or_default();
        let Some(wanted) = changed.get(name) else {
            bindings.extend(had.iter().cloned());
            continue;
        };
        // (In the windows Finder's key worked in: F2's, its tabs', its
        // two panes'.)
        let context = had.first().and_then(KeyBinding::predicate).or_else(|| {
            key.action.name().starts_with("finder::").then(|| Rc::new(KeyBindingContextPredicate::parse(crate::finder::CONTEXT).unwrap()))
        });
        for keys in wanted {
            match KeyBinding::load(keys, key.action.boxed_clone(), context.clone(), false, None, &DummyKeyboardMapper) {
                Ok(binding) => bindings.push(binding),
                Err(e) => eprintln!("{name}'s key {keys} isn't one: {e}"),
            }
        }
    }
    cx.clear_key_bindings();
    cx.bind_keys(bindings);
    crate::menus::menu_bar(cx);
}

/// A key pressed, as F2's own can be: with ⌘, ⌃ or ⌥, or one that types
/// nothing (Return, ⌫, an arrow, F5); not one alone that types (typing a
/// name selects it).
pub fn from_keystroke(keystroke: &Keystroke) -> Option<String> {
    let key = keystroke.key.as_str();
    let m = keystroke.modifiers;
    let typeless = matches!(key, "enter" | "backspace" | "delete" | "tab" | "up" | "down" | "left" | "right" | "home" | "end" | "pageup" | "pagedown")
        || key.strip_prefix('f').is_some_and(|n| n.parse::<u8>().is_ok_and(|n| (1..=20).contains(&n)));
    if key.is_empty() || !(m.platform || m.control || m.alt || typeless) {
        return None;
    }
    Some(Keystroke { modifiers: Modifiers { function: false, ..m }, key: key.to_string(), key_char: None }.unparse())
}

/// While a key's recorded in Settings, none: pressed, it'd be done (⌘W
/// closing Settings), not recorded. Then all again, as Settings have them.
pub fn suspend(suspended: bool, cx: &mut App) {
    if suspended {
        if cx.try_global::<Aside>().is_none() {
            let all: Vec<KeyBinding> = cx.key_bindings().borrow().bindings().cloned().collect();
            cx.set_global(Aside(all));
            cx.clear_key_bindings();
            crate::menus::menu_bar(cx);
        }
    } else if cx.has_global::<Aside>() {
        let Aside(all) = cx.remove_global::<Aside>();
        cx.clear_key_bindings();
        cx.bind_keys(all);
        apply(cx);
    }
}
