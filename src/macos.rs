//! Calls into `src/macos/*.m`: icons, thumbnails, SF Symbols, system colors
//! and file kinds as AppKit, Quick Look and UTType give them; native menus;
//! and what Finder does to files through AppKit (Trash, pasteboard, aliases,
//! Open With, ejecting).

use std::ffi::{CStr, CString, c_char};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use gpui::{Hsla, RenderImage, Rgba, Size, size};
use image::{Frame, ImageBuffer};

unsafe extern "C" {
    fn nf_file_icon(path: *const c_char, px: u32) -> *mut u8;
    fn nf_type_icon(ext: *const c_char, is_dir: bool, px: u32) -> *mut u8;
    fn nf_thumbnail(path: *const c_char, px: u32, w: *mut u32, h: *mut u32) -> *mut u8;
    fn nf_symbol(name: *const c_char, pt: f32, weight: f32, scale: f32, rgba: *const f32, w: *mut u32, h: *mut u32)
    -> *mut u8;
    fn nf_system_colors(dark: bool, out: *mut f32);
    fn nf_kind(ext: *const c_char, is_dir: bool) -> *mut c_char;
    fn nf_available_bytes(path: *const c_char) -> u64;
    fn nf_is_package(ext: *const c_char) -> bool;
    fn nf_prefers_british() -> bool;
    fn nf_locale_currency() -> *mut c_char;
    fn nf_popup_menu(items: *const RawMenuItem, count: usize) -> i32;
    fn nf_trash(path: *const c_char, trashed: *mut *mut c_char, denied: *mut bool) -> *mut c_char;
    fn nf_play_ui_sound(path: *const c_char);
    fn nf_tags(path: *const c_char) -> *mut c_char;
    fn nf_set_tags(path: *const c_char, names: *const c_char) -> *mut c_char;
    fn nf_resolve_alias(path: *const c_char) -> *mut c_char;
    fn nf_copy_files(paths: *const *const c_char, count: usize);
    fn nf_copy_text(text: *const c_char);
    fn nf_pasted_files() -> *mut c_char;
    fn nf_make_alias(path: *const c_char, alias: *const c_char) -> *mut c_char;
    fn nf_apps_for(path: *const c_char) -> *mut c_char;
    fn nf_open_with(path: *const c_char, app: *const c_char);
    fn nf_volume_flags(path: *const c_char) -> u32;
    fn nf_eject(path: *const c_char) -> *mut c_char;
    fn nf_has_hidden_extension(path: *const c_char) -> bool;
    fn nf_set_hidden_extension(path: *const c_char, hidden: bool) -> *mut c_char;
    fn nf_is_locked(path: *const c_char) -> bool;
    fn nf_set_locked(path: *const c_char, locked: bool) -> *mut c_char;
    fn nf_cloud_state(path: *const c_char) -> i32;
    fn nf_download(path: *const c_char) -> *mut c_char;
    fn nf_evict(path: *const c_char) -> *mut c_char;
    fn nf_last_opened(path: *const c_char) -> f64;
    fn nf_more_info(path: *const c_char) -> *mut c_char;
    fn nf_comment(path: *const c_char) -> *mut c_char;
    fn nf_set_comment(path: *const c_char, text: *const c_char) -> *mut c_char;
    fn nf_set_default_app(path: *const c_char, app: *const c_char) -> *mut c_char;
    fn nf_default_app(path: *const c_char) -> *mut c_char;
    fn nf_set_icon(path: *const c_char, clear: bool) -> *mut c_char;
    fn nf_saved_search(path: *const c_char) -> *mut c_char;
    fn nf_write_saved_search(path: *const c_char, query: *const c_char, scope: *const c_char) -> *mut c_char;
    fn nf_quicklook_visible() -> bool;
    fn nf_quicklook_show(paths: *const c_char, index: std::ffi::c_long);
    fn nf_quicklook_close();
    fn nf_airdrop(paths: *const c_char) -> bool;
    fn nf_images_to_pdf(paths: *const c_char, out: *const c_char) -> *mut c_char;
    fn nf_accept_promises(view: *mut std::ffi::c_void, token: u64, cb: extern "C" fn(u64, *const c_char, f64, f64));
    fn nf_drag_out(view: *mut std::ffi::c_void, token: u64, cb: extern "C" fn(u64, *const c_char));
    fn nf_item_pressed();
    fn nf_front_finder_windows(ids: *mut u32, max: i32) -> i32;
    fn nf_read_only_image_volumes() -> *mut c_char;
    fn nf_finder_windows() -> *mut c_char;
    fn nf_close_finder_windows(ids: *const i64, n: i32);
    fn nf_on_finder_forward(came_forward: extern "C" fn());
    fn nf_app_path(bundle_id: *const c_char) -> *mut c_char;
    fn nf_running_apps() -> *mut c_char;
    fn nf_quit_app(pid: i32, force: bool) -> bool;
    fn nf_hide_app(pid: i32) -> bool;
    fn nf_modifiers_held() -> bool;
    fn nf_automation(bundle: *const c_char, ask: bool) -> i32;
    fn nf_folder_icon_tinted(path: *const c_char, px: u32, rgb: u32) -> *mut u8;
    fn nf_tag_tinting() -> bool;
    fn nf_emoji_palette();
    fn nf_plist_json(data: *const u8, len: usize) -> *mut c_char;
    fn nf_disk_images_ours() -> bool;
    fn nf_set_disk_images_ours(ours: bool) -> *mut c_char;
    fn nf_titlebar_height() -> f64;
    fn nf_yield_to(pid: i32);
    fn nf_clean_keyboard(seconds: i32) -> bool;
    fn nf_lock_screen() -> bool;
    fn nf_panel_setup(view: *mut std::ffi::c_void, radius: f64);
    fn nf_panel_bare(view: *mut std::ffi::c_void);
    fn nf_panel_take_focus(view: *mut std::ffi::c_void);
    fn nf_panel_give_back() -> bool;
    fn nf_in_front() -> bool;
    fn nf_panel_fit(view: *mut std::ffi::c_void, height: f64);
    fn nf_panel_drag(view: *mut std::ffi::c_void, tallest: f64, home: f64, moved: extern "C" fn(f64, f64));
    fn nf_panel_present(view: *mut std::ffi::c_void);
    fn nf_panel_prepare(view: *mut std::ffi::c_void, radius: f64, height: f64);
    fn nf_screen_with_pointer() -> u32;
    fn nf_screen_with_front_window() -> u32;
    fn nf_display_frame(display: u32, x: *mut f64, y: *mut f64, w: *mut f64, h: *mut f64);
    fn nf_hot_keys_listen(pressed: extern "C" fn(u32));
    fn nf_updates_start() -> bool;
    fn nf_set_drawables(count: u32);
    fn nf_sha256_file(path: *const c_char, out: *mut u8, stop: *const bool) -> bool;
    fn nf_private_size(path: *const c_char) -> i64;
    fn nf_compress_pdf(input: *const c_char, out: *const c_char) -> *mut c_char;
    fn nf_updates_check() -> bool;
    fn nf_about(credits: *const c_char);
    fn nf_bundle_value(key: *const c_char) -> *mut c_char;
    fn nf_resource(name: *const c_char) -> *mut c_char;
    fn nf_os_version() -> *mut c_char;
    fn nf_login_item_on() -> bool;
    fn nf_login_item_set(on: bool) -> bool;
    fn nf_symbolic_hotkey_on(id: i32) -> bool;
    fn nf_snippets_watch(list: *const c_char, on_match: extern "C" fn(i32));
    fn nf_type_backspaces(n: i32);
    fn nf_post_left(n: i32);
    fn nf_clipboard_save();
    fn nf_clipboard_restore();
    fn nf_clipboard_text() -> *mut c_char;
    fn nf_front_document() -> *mut c_char;
    fn nf_window_get(x: *mut f64, y: *mut f64, w: *mut f64, h: *mut f64, pid: *mut i32, key: *mut u64) -> bool;
    fn nf_window_set(x: f64, y: f64, w: f64, h: f64) -> bool;
    fn nf_window_full_screen() -> bool;
    fn nf_window_drag(keys: u64) -> bool;
    fn nf_askpass(prompt: *const c_char, kind: *const c_char) -> *mut c_char;
    fn nf_js_host() -> i32;
    fn nf_extension_fetch(request: *const c_char, allowed: *const c_char) -> *mut c_char;
    fn nf_screens(out: *mut f64, max: i32) -> i32;
    fn nf_pasteboard_count() -> i64;
    fn nf_pasteboard_access() -> i32;
    fn nf_pasteboard_take(image_out: *const c_char) -> *mut c_char;
    fn nf_front_app() -> *mut c_char;
    fn nf_accessibility(prompt: bool) -> bool;
    fn nf_post_paste();
    fn nf_capture_allowed() -> bool;
    fn nf_capture_request() -> bool;
    fn nf_image_info(path: *const c_char, w: *mut u32, h: *mut u32, scale: *mut f64) -> bool;
    fn nf_copy_image(path: *const c_char) -> bool;
    fn nf_recognize_text(path: *const c_char) -> *mut c_char;
    fn nf_pick_color(picked: extern "C" fn(*const c_char));
    fn nf_pixelate_file(input: *const c_char, out: *const c_char, block: f64) -> bool;
    fn nf_render_shot(spec: *const c_char) -> *mut c_char;
    fn nf_system_screenshot_keys() -> u32;
    fn nf_hot_key_set(id: u32, key_code: u32, modifiers: u32) -> bool;
    fn nf_status_item_install(entries: *const RawStatusEntry, count: usize, chosen: extern "C" fn(i32), tooltip: *const c_char);
    fn nf_status_title(title: *const c_char);
    fn nf_status_extra(entries: *const RawStatusEntry, count: usize);
    fn nf_play_sound(name: *const c_char, times: i32);
    fn nf_keep_awake(on: bool) -> bool;
    fn nf_volume(set: f32) -> f32;
    fn nf_muted(set: i32) -> i32;
    fn nf_media_key(key: i32);
    fn nf_brightness(set: f32) -> f32;
    fn nf_calendar_access() -> i32;
    fn nf_fahrenheit() -> bool;
    fn nf_qr_png(text: *const c_char, out: *const c_char, px: i32) -> bool;
    fn nf_read_qr(path: *const c_char) -> *mut c_char;
    fn nf_calendar_request() -> bool;
    fn nf_reminders_access() -> i32;
    fn nf_reminders_request() -> bool;
    fn nf_add_event(title: *const c_char, start: f64, end: f64, all_day: bool) -> *mut c_char;
    fn nf_events(from: f64, to: f64) -> *mut c_char;
}

/// `nf_status_entry` in `status.m`.
#[repr(C)]
struct RawStatusEntry {
    title: *const c_char,
    symbol: *const c_char,
    tag: i32,
}

/// An item of the menu bar item's menu: None for a separator, else its
/// title, SF Symbol and tag (`OPEN_AT_LOGIN` for that toggle, which macOS
/// keeps itself).
pub type StatusEntry = Option<(String, Option<&'static str>, i32)>;

pub const OPEN_AT_LOGIN: i32 = -1;

/// Put the app's item in the menu bar; `chosen` gets the tag of what's
/// picked from its menu (on the main thread).
pub fn install_status_item(entries: &[StatusEntry], chosen: extern "C" fn(i32)) {
    let strings: Vec<Option<(CString, Option<CString>)>> = entries
        .iter()
        .map(|e| e.as_ref().map(|(title, symbol, _)| (CString::new(title.as_str()).unwrap_or_default(), symbol.and_then(|s| CString::new(s).ok()))))
        .collect();
    let raw: Vec<RawStatusEntry> = entries
        .iter()
        .zip(&strings)
        .map(|(e, s)| match (e, s) {
            (Some((_, _, tag)), Some((title, symbol))) => RawStatusEntry {
                title: title.as_ptr(),
                symbol: symbol.as_ref().map_or(std::ptr::null(), |s| s.as_ptr()),
                tag: *tag,
            },
            _ => RawStatusEntry { title: std::ptr::null(), symbol: std::ptr::null(), tag: 0 },
        })
        .collect();
    let tooltip = CString::new(crate::APP_NAME).unwrap_or_default();
    unsafe { nf_status_item_install(raw.as_ptr(), raw.len(), chosen, tooltip.as_ptr()) }
}

/// What's shown beside the menu bar item's icon ("" for nothing).
pub fn status_title(title: &str) {
    let title = CString::new(title).unwrap_or_default();
    unsafe { nf_status_title(title.as_ptr()) }
}

/// Items atop the menu bar item's menu (title, SF Symbol, tag of 1000 and
/// up), in place of the last ones.
pub fn status_extra(entries: &[(String, &'static str, i32)]) {
    let strings: Vec<(CString, CString)> = entries.iter().map(|(t, s, _)| (CString::new(t.as_str()).unwrap_or_default(), CString::new(*s).unwrap_or_default())).collect();
    let raw: Vec<RawStatusEntry> = entries.iter().zip(&strings).map(|((_, _, tag), (t, s))| RawStatusEntry { title: t.as_ptr(), symbol: s.as_ptr(), tag: *tag }).collect();
    unsafe { nf_status_extra(raw.as_ptr(), raw.len()) }
}

/// A system sound ("Glass"), `times` times.
pub fn play_sound(name: &str, times: i32) {
    let name = CString::new(name).unwrap_or_default();
    unsafe { nf_play_sound(name.as_ptr(), times) }
}

/// Keep the Mac awake (and its display), or let it sleep. Whether it's kept
/// awake now.
pub fn keep_awake(on: bool) -> bool {
    unsafe { nf_keep_awake(on) }
}

/// The sound's volume, 0 to 1 (None: no output to set), set first to `set`.
pub fn volume(set: Option<f32>) -> Option<f32> {
    let v = unsafe { nf_volume(set.unwrap_or(-1.)) };
    (v >= 0.).then_some(v)
}

/// Whether the sound's muted (None: it can't be), muted or not first.
pub fn muted(set: Option<bool>) -> Option<bool> {
    match unsafe { nf_muted(set.map_or(-1, i32::from)) } {
        -1 => None,
        m => Some(m == 1),
    }
}

/// The keyboard's media keys: play/pause, next, previous.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MediaKey {
    Play = 16,
    Next = 17,
    Previous = 18,
}

/// A media key, pressed (as the keyboard does it: to whatever's playing).
pub fn media_key(key: MediaKey) {
    unsafe { nf_media_key(key as i32) }
}

/// The built-in screen's brightness, 0 to 1 (None: no built-in screen, or
/// it can't be set), set first to `set`.
pub fn brightness(set: Option<f32>) -> Option<f32> {
    let v = unsafe { nf_brightness(set.unwrap_or(-1.)) };
    (v >= 0.).then_some(v)
}

/// A QR code of `text`, as a PNG at `out`, about `px` pixels square.
pub fn qr_png(text: &str, out: &Path, px: i32) -> bool {
    let (Ok(text), Some(out)) = (CString::new(text), c_path(out)) else { return false };
    unsafe { nf_qr_png(text.as_ptr(), out.as_ptr(), px) }
}

/// What the QR codes (and barcodes) in an image say, a line each.
pub fn read_qr(path: &Path) -> Option<String> {
    let path = c_path(path)?;
    take(unsafe { nf_read_qr(path.as_ptr()) })
}

/// Whether temperatures are in Fahrenheit here (System Settings says, or
/// the region).
pub fn fahrenheit() -> bool {
    unsafe { nf_fahrenheit() }
}

/// Whether F2 may see the calendar: Some(true) yes, Some(false) no (or only
/// add to it), None not asked yet.
pub fn calendar_access() -> Option<bool> {
    match unsafe { nf_calendar_access() } {
        0 => None,
        access => Some(access == 1),
    }
}

/// Ask for the calendar (macOS asks the first time), waiting: not on the
/// main thread. Whether it's allowed.
pub fn calendar_request() -> bool {
    unsafe { nf_calendar_request() }
}

/// Whether F2 may add to Reminders: Some(true) yes, Some(false) no, None
/// not asked yet.
pub fn reminders_access() -> Option<bool> {
    match unsafe { nf_reminders_access() } {
        0 => None,
        access => Some(access == 1),
    }
}

/// Ask for Reminders (macOS asks the first time), waiting: not on the main
/// thread. Whether it's allowed.
pub fn reminders_request() -> bool {
    unsafe { nf_reminders_request() }
}

/// Add an event to the default calendar (asking for it first, if need be:
/// not on the main thread). Err("denied") if the calendar's off limits.
pub fn add_event(title: &str, start: i64, end: i64, all_day: bool) -> Result<(), String> {
    let title = CString::new(title).map_err(|e| e.to_string())?;
    match take(unsafe { nf_add_event(title.as_ptr(), start as f64, end as f64, all_day) }) {
        None => Ok(()),
        Some(why) => Err(why),
    }
}

/// The events from `from` to `to` (and those under way then), soonest first:
/// start, end, all day, title, calendar, a video call's link. None without
/// access.
pub fn events(from: i64, to: i64) -> Option<Vec<(i64, i64, bool, String, String, Option<String>)>> {
    let text = take(unsafe { nf_events(from as f64, to as f64) })?;
    Some(
        text.lines()
            .filter_map(|l| {
                let f: Vec<&str> = l.split('\t').collect();
                let [start, end, all_day, title, calendar, link] = f[..] else { return None };
                Some((start.parse().ok()?, end.parse().ok()?, all_day == "1", title.to_string(), calendar.to_string(), Some(link.to_string()).filter(|l| !l.is_empty())))
            })
            .collect(),
    )
}

/// An item in iCloud Drive or a cloud storage folder: whether it's here.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cloud {
    Current,
    NotDownloaded,
    Downloading,
    Uploading,
}

/// `NSEventModifierFlags` for menu key equivalents.
pub const COMMAND: u64 = 1 << 20;
pub const OPTION: u64 = 1 << 19;
pub const SHIFT: u64 = 1 << 17;

/// `nf_menu_item` in `menu.m`.
#[repr(C)]
struct RawMenuItem {
    title: *const c_char,
    icon: *const c_char,
    share: *const c_char,
    key: *const c_char,
    modifiers: u64,
    tag: i32,
    depth: u8,
    enabled: bool,
    checked: bool,
    alternate: bool,
    swatch: u8,
    import_to: *const c_char,
}

/// One line of a native menu (see [`popup_menu`]).
#[derive(Default)]
pub struct MenuItem {
    /// `None`: a separator.
    pub title: Option<String>,
    /// Returned when chosen; -1 opens a submenu of the items after it at
    /// depth 1.
    pub tag: i32,
    pub depth: u8,
    pub key: &'static str,
    pub modifiers: u64,
    pub enabled: bool,
    pub checked: bool,
    pub alternate: bool,
    /// A file (an app) whose icon to show.
    pub icon: Option<PathBuf>,
    /// The system's Share… item for these files, in place of this one.
    pub share: Vec<PathBuf>,
    /// A tag color dot (see [`tags`]), or 0.
    pub swatch: u8,
    /// Continuity Camera's Import from iPhone or iPad, in place of this
    /// one: what comes is saved into this folder.
    pub import_to: Option<PathBuf>,
}

impl MenuItem {
    pub fn key(&mut self, key: &'static str, modifiers: u64) -> &mut Self {
        (self.key, self.modifiers) = (key, modifiers);
        self
    }

    pub fn enabled(&mut self, enabled: bool) -> &mut Self {
        self.enabled = enabled;
        self
    }

    pub fn checked(&mut self, checked: bool) -> &mut Self {
        self.checked = checked;
        self
    }

    /// Shown in place of the item before it while ⌥ is held.
    pub fn alternate(&mut self) -> &mut Self {
        self.alternate = true;
        self
    }

    pub fn icon(&mut self, path: PathBuf) -> &mut Self {
        self.icon = Some(path);
        self
    }

    pub fn swatch(&mut self, color: u8) -> &mut Self {
        self.swatch = color;
        self
    }
}

/// Show a native menu at the mouse and wait for it to close: the tag of
/// the item chosen, if any. It runs its own event loop, so call it while
/// no GPUI state is borrowed (from a spawned task, not an event handler).
pub fn popup_menu(items: &[MenuItem]) -> Option<u32> {
    let cstr = |s: &str| CString::new(s).unwrap_or_default();
    let share = |paths: &[PathBuf]| {
        let joined: Vec<String> = paths.iter().map(|p| p.to_string_lossy().into_owned()).collect();
        (!paths.is_empty()).then(|| cstr(&joined.join("\n")))
    };
    let strings: Vec<(Option<CString>, Option<CString>, Option<CString>, CString, Option<CString>)> = items
        .iter()
        .map(|i| (i.title.as_deref().map(cstr), i.icon.as_deref().and_then(c_path), share(&i.share), cstr(i.key), i.import_to.as_deref().and_then(c_path)))
        .collect();
    let ptr = |s: &Option<CString>| s.as_ref().map_or(std::ptr::null(), |s| s.as_ptr());
    let raw: Vec<RawMenuItem> = items
        .iter()
        .zip(&strings)
        .map(|(i, (title, icon, share, key, import_to))| RawMenuItem {
            title: ptr(title),
            icon: ptr(icon),
            share: ptr(share),
            key: key.as_ptr(),
            modifiers: i.modifiers,
            tag: i.tag,
            depth: i.depth,
            enabled: i.enabled,
            checked: i.checked,
            alternate: i.alternate,
            swatch: i.swatch,
            import_to: ptr(import_to),
        })
        .collect();
    let chosen = unsafe { nf_popup_menu(raw.as_ptr(), raw.len()) };
    u32::try_from(chosen).ok()
}

/// A malloc'd string from the `.m` side, freed.
fn take(s: *mut c_char) -> Option<String> {
    if s.is_null() {
        return None;
    }
    let out = unsafe { CStr::from_ptr(s) }.to_string_lossy().into_owned();
    unsafe { libc::free(s.cast()) };
    Some(out)
}

/// NULL for success, else the error's message.
fn outcome(error: *mut c_char) -> Result<(), String> {
    take(error).map_or(Ok(()), Err)
}

fn lines(s: *mut c_char) -> Vec<PathBuf> {
    take(s).map_or_else(Vec::new, |s| s.lines().map(PathBuf::from).collect())
}

/// Finder's sounds.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Sound {
    /// Moved to the Trash (⌘⌫, the menus).
    MoveToTrash,
    /// Dropped on the Trash.
    DragToTrash,
    EmptyTrash,
    /// Files dropped somewhere (F2's own, not Finder's: Finder's drops are
    /// silent).
    Drop,
}

/// Play `sound` as macOS's own interface sounds play: at the alert volume,
/// and not at all with "Play user interface sound effects" off.
pub fn play(sound: Sound) {
    const SOUNDS: &str = "/System/Library/Components/CoreAudio.component/Contents/SharedSupport/SystemSounds";
    let file = match sound {
        Sound::MoveToTrash => format!("{SOUNDS}/finder/move to trash.aif"),
        Sound::DragToTrash => format!("{SOUNDS}/dock/drag to trash.aif"),
        Sound::EmptyTrash => format!("{SOUNDS}/finder/empty trash.aif"),
        Sound::Drop => "/System/Library/Sounds/Pop.aiff".into(),
    };
    if let Ok(path) = CString::new(file) {
        unsafe { nf_play_ui_sound(path.as_ptr()) }
    }
}

/// Move to the Trash; where it went (to put it back from).
pub fn trash(path: &Path) -> Result<PathBuf, String> {
    try_trash(path).map_err(|(e, _)| e)
}

/// Move to the Trash, or why not, and whether it's because it isn't this
/// user's to move (an app an administrator installed): Finder can, with an
/// administrator's password (see `finder_trash`).
pub fn try_trash(path: &Path) -> Result<PathBuf, (String, bool)> {
    let path = c_path(path).ok_or(("bad path".to_string(), false))?;
    let (mut trashed, mut denied) = (std::ptr::null_mut(), false);
    outcome(unsafe { nf_trash(path.as_ptr(), &mut trashed, &mut denied) }).map_err(|e| (e, denied))?;
    take(trashed).map(PathBuf::from).ok_or_else(|| (format!("it went missing on its way to the {}", bin()), false))
}

/// Move them to the Trash, where each went or why not: those that aren't
/// this user's to move, all at once by Finder, which asks for an
/// administrator's password (or Touch ID) once, as it does when you do it
/// there. Waits for that: not on the main thread.
pub fn trash_all(paths: &[PathBuf]) -> Vec<Result<PathBuf, String>> {
    let mut done: Vec<Result<PathBuf, String>> = Vec::with_capacity(paths.len());
    let mut denied = Vec::new();
    for (i, path) in paths.iter().enumerate() {
        match try_trash(path) {
            Ok(to) => done.push(Ok(to)),
            Err((e, true)) => {
                denied.push(i);
                done.push(Err(e));
            }
            Err((e, false)) => done.push(Err(e)),
        }
    }
    if !denied.is_empty() {
        let theirs: Vec<PathBuf> = denied.iter().map(|&i| paths[i].clone()).collect();
        for (i, result) in denied.into_iter().zip(finder_trash(&theirs)) {
            done[i] = result;
        }
    }
    done
}

/// Finder's script for `finder_trash`: where each went, a line each.
fn finder_trash_script(paths: &[PathBuf]) -> String {
    let quoted = |p: &Path| format!("\"{}\"", p.to_string_lossy().replace('\\', "\\\\").replace('"', "\\\""));
    let items: Vec<String> = paths.iter().map(|p| format!("(POSIX file {}) as alias", quoted(p))).collect();
    format!(
        r#"set these to {{{}}}
set out to ""
tell application "Finder" to set moved to delete these
if class of moved is not list then set moved to {{moved}}
repeat with m in moved
    try
        set out to out & POSIX path of (m as alias) & linefeed
    on error
        set out to out & linefeed
    end try
end repeat
return out"#,
        items.join(", ")
    )
}

/// Have Finder move them to the Trash, asking for an administrator's
/// password (or Touch ID) once if it needs one: where each went. Waits
/// for that: not on the main thread.
pub fn finder_trash(paths: &[PathBuf]) -> Vec<Result<PathBuf, String>> {
    let script = finder_trash_script(paths);
    let failed = |e: String| paths.iter().map(|_| Err(e.clone())).collect();
    let out = match std::process::Command::new("/usr/bin/osascript").args(["-e", &script]).output() {
        Ok(out) => out,
        Err(e) => return failed(e.to_string()),
    };
    if !out.status.success() {
        let said = String::from_utf8_lossy(&out.stderr).to_string();
        return failed(if said.contains("(-128)") {
            "it takes an administrator's password, and that was cancelled".into()
        } else if said.contains("(-1743)") {
            format!("{APP_NAME} may not ask Finder to: allow it in System Settings › Privacy & Security › Automation", APP_NAME = crate::APP_NAME)
        } else {
            said.rsplit("execution error: ").next().unwrap_or(&said).trim().to_string()
        });
    }
    // (Where each went, as Finder says; else where it'd be.)
    let home = std::env::var_os("HOME").map_or_else(|| PathBuf::from("/"), PathBuf::from);
    let mut said = String::from_utf8_lossy(&out.stdout).into_owned().lines().map(str::to_string).collect::<Vec<_>>().into_iter();
    paths
        .iter()
        .map(|p| {
            let to = said.next().map(|l| l.trim().trim_end_matches('/').to_string()).filter(|l| !l.is_empty());
            Ok(to.map(PathBuf::from).unwrap_or_else(|| home.join(".Trash").join(p.file_name().unwrap_or_default())))
        })
        .collect()
}

/// A file's tags: name, and Finder's color number (0 none, 1 gray, 2 green,
/// 3 purple, 4 blue, 5 yellow, 6 red, 7 orange).
pub fn tags(path: &Path) -> Vec<(String, u8)> {
    let Some(text) = c_path(path).and_then(|p| take(unsafe { nf_tags(p.as_ptr()) })) else { return Vec::new() };
    text.lines()
        .filter_map(|line| {
            let (name, color) = line.split_once('\t')?;
            Some((name.to_string(), color.parse().unwrap_or(0)))
        })
        .collect()
}

pub fn set_tags(path: &Path, names: &[String]) -> Result<(), String> {
    let path = c_path(path).ok_or("bad path")?;
    let names = CString::new(names.join("\n")).map_err(|e| e.to_string())?;
    outcome(unsafe { nf_set_tags(path.as_ptr(), names.as_ptr()) })
}

/// Where an alias or symlink leads, if anywhere.
pub fn resolve_alias(path: &Path) -> Option<PathBuf> {
    let path = c_path(path)?;
    take(unsafe { nf_resolve_alias(path.as_ptr()) }).map(PathBuf::from)
}

/// Copy files as Finder does: they can be pasted in Finder, or here.
pub fn copy_files(paths: &[&Path]) {
    let paths: Vec<CString> = paths.iter().filter_map(|p| c_path(p)).collect();
    let raw: Vec<*const c_char> = paths.iter().map(|p| p.as_ptr()).collect();
    unsafe { nf_copy_files(raw.as_ptr(), raw.len()) };
}

pub fn copy_text(text: &str) {
    if let Ok(text) = CString::new(text) {
        unsafe { nf_copy_text(text.as_ptr()) };
    }
}

/// Files on the pasteboard (copied here or in Finder).
pub fn pasted_files() -> Vec<PathBuf> {
    lines(unsafe { nf_pasted_files() })
}

pub fn make_alias(path: &Path, alias: &Path) -> Result<(), String> {
    let (path, alias) = (c_path(path).ok_or("bad path")?, c_path(alias).ok_or("bad path")?);
    outcome(unsafe { nf_make_alias(path.as_ptr(), alias.as_ptr()) })
}

/// Apps that can open `path`, its default app first.
pub fn apps_for(path: &Path) -> Vec<PathBuf> {
    c_path(path).map_or_else(Vec::new, |path| lines(unsafe { nf_apps_for(path.as_ptr()) }))
}

pub fn open_with(path: &Path, app: &Path) {
    if let (Some(path), Some(app)) = (c_path(path), c_path(app)) {
        unsafe { nf_open_with(path.as_ptr(), app.as_ptr()) };
    }
}

/// Whether a volume is mounted at `path`, and whether it can be ejected
/// (disk images, removable and network drives).
pub fn volume_flags(path: &Path) -> (bool, bool) {
    let flags = c_path(path).map_or(0, |path| unsafe { nf_volume_flags(path.as_ptr()) });
    (flags & 1 != 0, flags & 2 != 0)
}

/// Eject as Finder does: unmount and let go of the disk, detaching a disk
/// image entirely (AppKit's unmount-and-eject leaves an APFS image
/// attached). A network volume isn't a disk to `diskutil`: that one AppKit
/// unmounts. Blocks until done (or refused).
pub fn eject(path: &Path) -> Result<(), String> {
    let out = std::process::Command::new("/usr/sbin/diskutil").arg("eject").arg(path).output();
    if out.is_ok_and(|o| o.status.success()) {
        return Ok(());
    }
    let path = c_path(path).ok_or("bad path")?;
    outcome(unsafe { nf_eject(path.as_ptr()) })
}

fn c_path(path: &Path) -> Option<CString> {
    CString::new(path.as_os_str().as_bytes()).ok()
}

/// The icon Finder shows for this file: a folder's or app's own, or its
/// type's. `px` square.
pub fn file_icon(path: &Path, px: u32) -> Option<RenderImage> {
    let path = c_path(path)?;
    into_image(unsafe { nf_file_icon(path.as_ptr(), px) }, px, px)
}

/// A folder's icon in a tag's color (0xRRGGBB), as Finder 26 tints tagged
/// folders (an emoji on it left as it is).
pub fn folder_icon_tinted(path: &Path, px: u32, rgb: u32) -> Option<RenderImage> {
    let path = c_path(path)?;
    into_image(unsafe { nf_folder_icon_tinted(path.as_ptr(), px, rgb) }, px, px)
}

/// A property list (binary or XML) as JSON: its data as hex strings, its
/// dates as seconds since 1970.
pub fn plist_json(data: &[u8]) -> Option<serde_json::Value> {
    serde_json::from_str(&take(unsafe { nf_plist_json(data.as_ptr(), data.len()) })?).ok()
}

/// Whether disk images open in F2 (else in macOS's DiskImageMounter).
pub fn disk_images_ours() -> bool {
    unsafe { nf_disk_images_ours() }
}

/// Open .dmg files in F2, or in DiskImageMounter again. macOS asks you
/// first, and this waits for your answer: not on the main thread.
pub fn set_disk_images_ours(ours: bool) -> Result<(), String> {
    outcome(unsafe { nf_set_disk_images_ours(ours) })
}

/// Let the process `pid` (F2's own dialog, a process of its own) come
/// forward, as the app in front has to since macOS 14.
pub fn yield_to(pid: u32) {
    unsafe { nf_yield_to(pid as i32) }
}

/// How tall a window's title bar is, with no toolbar.
pub fn titlebar_height() -> f32 {
    unsafe { nf_titlebar_height() as f32 }
}

/// macOS's Emoji & Symbols viewer (it types into what has the keyboard).
pub fn emoji_palette() {
    unsafe { nf_emoji_palette() }
}

/// Whether tagged folders take their tags' colors (Finder's Settings > Tags
/// > "Tint folders based on tags").
pub fn tag_tinting() -> bool {
    unsafe { nf_tag_tinting() }
}

/// The icon for files with this extension, or for a plain folder.
pub fn type_icon(ext: &str, is_dir: bool, px: u32) -> Option<RenderImage> {
    let ext = CString::new(ext).ok()?;
    into_image(unsafe { nf_type_icon(ext.as_ptr(), is_dir, px) }, px, px)
}

/// A Quick Look thumbnail at most `px` on its longer side. Blocks.
pub fn thumbnail(path: &Path, px: u32) -> Option<RenderImage> {
    let path = c_path(path)?;
    let (mut w, mut h) = (0, 0);
    let data = unsafe { nf_thumbnail(path.as_ptr(), px, &mut w, &mut h) };
    into_image(data, w, h)
}

/// An SF Symbol `pt` points tall in `color`, drawn at `scale` pixels per
/// point; with its size in points. `weight` as `NSFontWeight` (0 regular,
/// 0.23 medium, 0.3 semibold).
pub fn symbol(name: &str, pt: f32, weight: f32, color: Hsla, scale: f32) -> Option<(RenderImage, Size<f32>)> {
    let name = CString::new(name).ok()?;
    let Rgba { r, g, b, a } = color.to_rgb();
    let (mut w, mut h) = (0, 0);
    let data = unsafe { nf_symbol(name.as_ptr(), pt, weight, scale, [r, g, b, a].as_ptr(), &mut w, &mut h) };
    Some((into_image(data, w, h)?, size(w as f32 / scale, h as f32 / scale)))
}

/// The system's named colors for light or dark (see `nf_system_colors` for
/// which), as RGBA.
pub fn system_colors(dark: bool) -> [Rgba; 9] {
    let mut c = [0f32; 9 * 4];
    unsafe { nf_system_colors(dark, c.as_mut_ptr()) };
    std::array::from_fn(|i| Rgba { r: c[i * 4], g: c[i * 4 + 1], b: c[i * 4 + 2], a: c[i * 4 + 3] })
}

/// Finder's "Kind" for an extension ("Markdown text file"), or a folder's.
pub fn kind(ext: &str, is_dir: bool) -> Option<String> {
    let ext = CString::new(ext).ok()?;
    take(unsafe { nf_kind(ext.as_ptr(), is_dir) })
}

/// What the Trash is called: "Trash", or "Rubbish" for anyone whose Mac
/// speaks British English (a little joke: macOS itself says "Bin").
pub fn bin() -> &'static str {
    static BIN: std::sync::OnceLock<&str> = std::sync::OnceLock::new();
    BIN.get_or_init(|| if unsafe { nf_prefers_british() } { "Rubbish" } else { "Trash" })
}

/// This Mac's currency, as its region has it ("KRW").
pub fn local_currency() -> Option<String> {
    take(unsafe { nf_locale_currency() })
}

/// Whether the app has Full Disk Access (as of its launch: macOS applies
/// a grant when the app next starts).
pub fn full_disk_access() -> bool {
    static ACCESS: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ACCESS.get_or_init(fsearch::has_full_disk_access)
}

/// System Settings at Privacy & Security > Full Disk Access.
pub const FULL_DISK_ACCESS_SETTINGS: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles";

/// Whether folders with this extension are packages (opened, not browsed).
pub fn is_package(ext: &str) -> bool {
    CString::new(ext).is_ok_and(|ext| unsafe { nf_is_package(ext.as_ptr()) })
}

/// Space available on the volume holding `path`, counted as Finder does.
pub fn available_bytes(path: &Path) -> Option<u64> {
    let path = c_path(path)?;
    Some(unsafe { nf_available_bytes(path.as_ptr()) }).filter(|&n| n > 0)
}

/// Take a malloc'd premultiplied BGRA buffer from the `.m` side; GPUI wants
/// straight alpha.
fn into_image(data: *mut u8, w: u32, h: u32) -> Option<RenderImage> {
    if data.is_null() {
        return None;
    }
    let len = w as usize * h as usize * 4;
    let mut pixels = unsafe { std::slice::from_raw_parts(data, len) }.to_vec();
    unsafe { libc::free(data.cast()) };
    for px in pixels.chunks_exact_mut(4) {
        let a = px[3] as u32;
        if a != 0 && a != 255 {
            for c in &mut px[..3] {
                *c = (*c as u32 * 255 / a).min(255) as u8;
            }
        }
    }
    let buffer = ImageBuffer::from_raw(w, h, pixels)?;
    Some(RenderImage::new(vec![Frame::new(buffer)]))
}

fn joined(paths: &[PathBuf]) -> Option<CString> {
    let lines: Vec<String> = paths.iter().map(|p| p.to_string_lossy().into_owned()).collect();
    CString::new(lines.join("\n")).ok()
}

fn flag(path: &Path, f: unsafe extern "C" fn(*const c_char) -> bool) -> bool {
    c_path(path).is_some_and(|p| unsafe { f(p.as_ptr()) })
}

fn call(path: &Path, f: impl FnOnce(*const c_char) -> *mut c_char) -> Result<(), String> {
    let path = c_path(path).ok_or("bad path")?;
    outcome(f(path.as_ptr()))
}

pub fn has_hidden_extension(path: &Path) -> bool {
    flag(path, nf_has_hidden_extension)
}

pub fn set_hidden_extension(path: &Path, hidden: bool) -> Result<(), String> {
    call(path, |p| unsafe { nf_set_hidden_extension(p, hidden) })
}

pub fn is_locked(path: &Path) -> bool {
    flag(path, nf_is_locked)
}

pub fn set_locked(path: &Path, locked: bool) -> Result<(), String> {
    call(path, |p| unsafe { nf_set_locked(p, locked) })
}

pub fn cloud_state(path: &Path) -> Option<Cloud> {
    let state = c_path(path).map_or(0, |p| unsafe { nf_cloud_state(p.as_ptr()) });
    match state {
        1 => Some(Cloud::Current),
        2 => Some(Cloud::NotDownloaded),
        3 => Some(Cloud::Downloading),
        4 => Some(Cloud::Uploading),
        _ => None,
    }
}

/// Download an item from iCloud (or a cloud provider) now.
pub fn download(path: &Path) -> Result<(), String> {
    call(path, |p| unsafe { nf_download(p) })
}

/// Remove the local copy, keeping it in the cloud.
pub fn evict(path: &Path) -> Result<(), String> {
    call(path, |p| unsafe { nf_evict(p) })
}

/// When it was last opened (seconds since 1970), as Spotlight knows.
pub fn last_opened(path: &Path) -> Option<i64> {
    let t = c_path(path).map_or(-1., |p| unsafe { nf_last_opened(p.as_ptr()) });
    (t >= 0.).then_some(t as i64)
}

/// Get Info's More Info: (label, value).
pub fn more_info(path: &Path) -> Vec<(String, String)> {
    let Some(text) = c_path(path).and_then(|p| take(unsafe { nf_more_info(p.as_ptr()) })) else { return Vec::new() };
    text.lines().filter_map(|l| l.split_once('\t')).map(|(a, b)| (a.to_string(), b.to_string())).collect()
}

pub fn comment(path: &Path) -> String {
    c_path(path).and_then(|p| take(unsafe { nf_comment(p.as_ptr()) })).unwrap_or_default()
}

pub fn set_comment(path: &Path, text: &str) -> Result<(), String> {
    let text = CString::new(text).map_err(|e| e.to_string())?;
    call(path, |p| unsafe { nf_set_comment(p, text.as_ptr()) })
}

/// Open every file of `path`'s kind with `app` (Change All).
pub fn set_default_app(path: &Path, app: &Path) -> Result<(), String> {
    let app = c_path(app).ok_or("bad path")?;
    call(path, |p| unsafe { nf_set_default_app(p, app.as_ptr()) })
}

pub fn default_app(path: &Path) -> Option<PathBuf> {
    c_path(path).and_then(|p| take(unsafe { nf_default_app(p.as_ptr()) })).map(PathBuf::from)
}

/// Use the image on the clipboard as the icon; or, with `clear`, the
/// usual one again.
pub fn set_icon(path: &Path, clear: bool) -> Result<(), String> {
    call(path, |p| unsafe { nf_set_icon(p, clear) })
}

/// A Smart Folder's Spotlight query and the folders it looks in.
pub fn saved_search(path: &Path) -> Option<(String, Vec<PathBuf>)> {
    let text = c_path(path).and_then(|p| take(unsafe { nf_saved_search(p.as_ptr()) }))?;
    let mut lines = text.lines();
    let query = lines.next()?.to_string();
    Some((query, lines.map(PathBuf::from).collect()))
}

pub fn write_saved_search(path: &Path, query: &str, scope: Option<&Path>) -> Result<(), String> {
    let query = CString::new(query).map_err(|e| e.to_string())?;
    let scope = scope.and_then(c_path).unwrap_or_default();
    call(path, |p| unsafe { nf_write_saved_search(p, query.as_ptr(), scope.as_ptr()) })
}

pub fn quicklook_visible() -> bool {
    unsafe { nf_quicklook_visible() }
}

/// Show these in Quick Look's panel, `index` first (or update it).
pub fn quicklook_show(paths: &[PathBuf], index: usize) {
    if let Some(list) = joined(paths) {
        unsafe { nf_quicklook_show(list.as_ptr(), index as std::ffi::c_long) };
    }
}

pub fn quicklook_close() {
    unsafe { nf_quicklook_close() }
}

/// Quick Look's panel made, not shown: the first look of the session isn't
/// the slow one then. On the main thread.
pub fn quicklook_warm() {
    unsafe { nf_quicklook_warm() }
}

/// An image file as ImageIO reads it (HEIC, camera raw…), turned as its
/// EXIF says, `max` pixels on its longer side at most. Off the main thread
/// is fine.
/// A folder's background picture, drawn for a screen of `scale`, and how
/// big it's shown, in points. Off the main thread is fine.
pub fn picture_bitmap(path: &Path, scale: f32) -> Option<(RenderImage, (f32, f32))> {
    let path = c_path(path)?;
    let (mut pw, mut ph, mut w, mut h) = (0., 0., 0, 0);
    let data = unsafe { nf_picture_bitmap(path.as_ptr(), scale as f64, &mut pw, &mut ph, &mut w, &mut h) };
    Some((into_image(data, w, h)?, (pw as f32, ph as f32)))
}

pub fn image_bitmap(path: &Path, max: u32) -> Option<RenderImage> {
    let path = c_path(path)?;
    let (mut w, mut h) = (0, 0);
    let data = unsafe { nf_image_bitmap(path.as_ptr(), max as i32, &mut w, &mut h) };
    into_image(data, w, h)
}

/// A PDF's pages `first..first + most`, `width` pixels wide each, and how
/// many it has (None: it can't be read, or it's locked). Off the main
/// thread is fine.
pub fn pdf_pages(path: &Path, first: usize, most: usize, width: u32) -> Option<(usize, Vec<RenderImage>)> {
    extern "C" fn page(ctx: *mut std::ffi::c_void, data: *mut u8, w: u32, h: u32) {
        let pages = unsafe { &mut *(ctx as *mut Vec<RenderImage>) };
        pages.extend(into_image(data, w, h));
    }
    let path = c_path(path)?;
    let mut pages: Vec<RenderImage> = Vec::new();
    let count = unsafe { nf_pdf_pages(path.as_ptr(), first as _, most as _, width as i32, (&raw mut pages).cast(), page) };
    (count >= 0).then_some((count as usize, pages))
}

/// Accept files promised by other apps (Mail, Photos...) dropped on the
/// window holding `view` (its AppKit view); `on_drop` gets them, staged in
/// a temporary folder, with the window's `token`.
pub fn accept_promises(view: *mut std::ffi::c_void, token: u64, on_drop: extern "C" fn(u64, *const c_char, f64, f64)) {
    unsafe { nf_accept_promises(view, token, on_drop) }
}

/// Files dragged out of the window holding `view` go to other apps as out
/// of Finder's (see dragout.m); `on_trash` gets those dropped on the Dock's
/// Trash, for F2 to trash, with the window's `token`.
pub fn drag_out(view: *mut std::ffi::c_void, token: u64, on_trash: extern "C" fn(u64, *const c_char)) {
    unsafe { nf_drag_out(view, token, on_trash) }
}

/// A press on an item (one that can be dragged out), said while it's
/// handled: its window, in the background, stays there meanwhile.
pub fn item_pressed() {
    unsafe { nf_item_pressed() }
}

/// Paths from the `.m` side, one per line.
pub fn split_paths(text: *const c_char) -> Vec<PathBuf> {
    if text.is_null() {
        return Vec::new();
    }
    let text = unsafe { CStr::from_ptr(text) }.to_string_lossy();
    text.lines().map(PathBuf::from).collect()
}

/// Ask Finder (with AppleScript) what's in the home folder's Trash, which
/// only apps with Full Disk Access may list. macOS asks once whether this
/// app may control Finder.
pub fn finder_trash_items() -> Vec<PathBuf> {
    let script = r#"set out to ""
tell application "Finder"
    repeat with anItem in (get items of trash)
        set out to out & POSIX path of (anItem as alias) & linefeed
    end repeat
end tell
return out"#;
    let Ok(out) = std::process::Command::new("/usr/bin/osascript").args(["-e", script]).output() else { return Vec::new() };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| PathBuf::from(l.trim_end_matches('/')))
        .collect()
}

/// Make a window (by its NSView) Quick Open's panel: no title bar, the
/// blur behind it rounded to `radius`.
pub fn panel_setup(view: *mut std::ffi::c_void, radius: f64) {
    unsafe { nf_panel_setup(view, radius) }
}

/// F2 made the active app while Quick Open's up (input methods compose
/// only in the active app), the app in front noted to give it back.
pub fn panel_take_focus(view: *mut std::ffi::c_void) {
    unsafe { nf_panel_take_focus(view) }
}

/// Give the app Quick Open took focus from its place back (if F2 still has
/// it). Whether it did.
pub fn panel_give_back() -> bool {
    unsafe { nf_panel_give_back() }
}

/// Whether F2 is the app in front.
pub fn in_front() -> bool {
    unsafe { nf_in_front() }
}

/// Once the app in front isn't F2 (Quick Open given back), or a while has
/// gone: what's sent to "the app in front" (⌘V, a window's place) is then
/// the app's it was.
pub async fn given_back(executor: &gpui::BackgroundExecutor) {
    for _ in 0..40 {
        if !in_front() {
            return;
        }
        executor.timer(std::time::Duration::from_millis(15)).await;
    }
}

/// No title bar, as `panel_setup`, but nothing behind: the window's clear,
/// and what's in it draws its own card and shadow.
pub fn panel_bare(view: *mut std::ffi::c_void) {
    unsafe { nf_panel_bare(view) }
}

/// Make the panel `height` points tall, its top edge staying put.
pub fn panel_fit(view: *mut std::ffi::c_void, height: f64) {
    unsafe { nf_panel_fit(view, height) }
}

/// Drag the panel, from a press on it just now: it follows the pointer
/// once it's moved a little, snapping to its screen's middle and to `home`
/// (its top's default, as a fraction of the screen's height), and stays
/// where there's room for it `tallest`. Where it's left goes to `moved`,
/// as fractions of its screen (its middle across, its top down).
pub fn panel_drag(view: *mut std::ffi::c_void, tallest: f32, home: f32, moved: extern "C" fn(f64, f64)) {
    unsafe { nf_panel_drag(view, tallest as f64, home as f64, moved) }
}

/// `panel_setup`, for a panel made hidden: `height` points tall, not drawn
/// till `panel_present`.
pub fn panel_prepare(view: *mut std::ffi::c_void, radius: f64, height: f64) {
    unsafe { nf_panel_prepare(view, radius, height) }
}

/// A panel made hidden drawn now, then put in front: on screen whole.
pub fn panel_present(view: *mut std::ffi::c_void) {
    unsafe { nf_panel_present(view) }
}

/// The screen the pointer's on (its display id).
pub fn screen_with_pointer() -> Option<u32> {
    Some(unsafe { nf_screen_with_pointer() }).filter(|&id| id != 0)
}

/// The screen the app in front's front window is on: None with none up.
pub fn screen_with_front_window() -> Option<u32> {
    Some(unsafe { nf_screen_with_front_window() }).filter(|&id| id != 0)
}

/// A display's place and size (x, y, width, height), in points from the
/// main one's top left.
pub fn display_frame(display: u32) -> (f64, f64, f64, f64) {
    let (mut x, mut y, mut w, mut h) = (0., 0., 0., 0.);
    unsafe { nf_display_frame(display, &mut x, &mut y, &mut w, &mut h) };
    (x, y, w, h)
}

/// The window in front (its app's focused one): where it is, and a key for
/// it while it's open. None without one (or without Accessibility).
pub fn window_get() -> Option<(crate::windows::Frame, u64)> {
    let (mut x, mut y, mut w, mut h, mut pid, mut key) = (0., 0., 0., 0., 0, 0);
    unsafe { nf_window_get(&mut x, &mut y, &mut w, &mut h, &mut pid, &mut key) }
        .then(|| (crate::windows::Frame { x: x as f32, y: y as f32, w: w as f32, h: h as f32 }, key))
}

/// Move and size the window in front.
pub fn window_set(to: crate::windows::Frame) -> bool {
    unsafe { nf_window_set(to.x as f64, to.y as f64, to.w as f64, to.h as f64) }
}

/// The window in front, into full screen or out.
pub fn window_full_screen() -> bool {
    unsafe { nf_window_full_screen() }
}

/// Dragging windows from anywhere in them with `keys` held (CGEventFlags;
/// 0: off). False if it can't be yet: no Accessibility.
pub fn window_drag(keys: u64) -> bool {
    unsafe { nf_window_drag(keys) }
}

/// Each screen's room for windows (less the menu bar and the Dock), and
/// all of it.
pub fn screens() -> Vec<(crate::windows::Frame, crate::windows::Frame)> {
    let mut out = [0f64; 8 * 16];
    let n = unsafe { nf_screens(out.as_mut_ptr(), 16) }.max(0) as usize;
    let frame = |o: &[f64]| crate::windows::Frame { x: o[0] as f32, y: o[1] as f32, w: o[2] as f32, h: o[3] as f32 };
    out.chunks(8).take(n).map(|o| (frame(&o[..4]), frame(&o[4..]))).collect()
}

/// Listen for snippets' `keywords` typed in any app (none: stop);
/// `on_match` gets the index of the one typed (with Accessibility).
pub fn snippets_watch(keywords: &[String], on_match: extern "C" fn(i32)) {
    let list = CString::new(keywords.join("\n").replace('\0', "")).unwrap_or_default();
    unsafe { nf_snippets_watch(list.as_ptr(), on_match) }
}

/// ⌫ `n` times, to the app in front.
pub fn type_backspaces(n: usize) {
    unsafe { nf_type_backspaces(n as i32) }
}

/// ← `n` times, to the app in front.
pub fn post_left(n: usize) {
    unsafe { nf_post_left(n as i32) }
}

/// Keep what's on the clipboard (all of it), to put back after.
pub fn clipboard_save() {
    unsafe { nf_clipboard_save() }
}

pub fn clipboard_restore() {
    unsafe { nf_clipboard_restore() }
}

/// The clipboard's text, if any.
pub fn clipboard_text() -> Option<String> {
    take(unsafe { nf_clipboard_text() })
}

/// F2 as an extension's JavaScript host (see src/macos/jshost.m): what
/// it exits with.
pub fn js_host() -> i32 {
    unsafe { nf_js_host() }
}

/// A request for an extension (JSON: url, method, headers, body), to one of
/// the hosts `allowed` (a JSON array): its answer as JSON, {status, url,
/// headers, body} or {error}. Waits for it: not on the main thread.
pub fn extension_fetch(request: &str, allowed: &str) -> String {
    let (Ok(request), Ok(allowed)) = (CString::new(request), CString::new(allowed)) else { return r#"{"error": "That request can't be sent."}"#.into() };
    take(unsafe { nf_extension_fetch(request.as_ptr(), allowed.as_ptr()) }).unwrap_or_else(|| r#"{"error": "No answer."}"#.into())
}

/// The document open in the app in front (an editor's file), if any.
/// ssh's question (`prompt`; `kind`, SSH_ASKPASS_PROMPT's), asked in a
/// dialog: the answer, or None if it's cancelled. See `sftp`.
pub fn askpass(prompt: &str, kind: Option<&str>) -> Option<String> {
    let prompt = CString::new(prompt).unwrap_or_default();
    let kind = kind.and_then(|k| CString::new(k).ok());
    take(unsafe { nf_askpass(prompt.as_ptr(), kind.as_ref().map_or(std::ptr::null(), |k| k.as_ptr())) })
}

pub fn front_document() -> Option<PathBuf> {
    take(unsafe { nf_front_document() }).map(PathBuf::from)
}

/// A file's SHA-256; None if it can't be read, or `stop` turns true while
/// it's being read.
pub fn sha256_file(path: &Path, stop: &std::sync::atomic::AtomicBool) -> Option<[u8; 32]> {
    let path = c_path(path)?;
    let mut out = [0u8; 32];
    // (Safety: an AtomicBool is a bool in memory; it's only read there.)
    let ok = unsafe { nf_sha256_file(path.as_ptr(), out.as_mut_ptr(), stop.as_ptr() as *const bool) };
    ok.then_some(out)
}

/// A PDF made smaller (its images as JPEG, sized for the screen) at `out`.
pub fn compress_pdf(input: &Path, out: &Path) -> Result<(), String> {
    let (input, out) = (c_path(input).ok_or("bad path")?, c_path(out).ok_or("bad path")?);
    outcome(unsafe { nf_compress_pdf(input.as_ptr(), out.as_ptr()) })
}

/// What only this file takes on the disk (the blocks no APFS clone of it
/// shares), if the disk can say.
pub fn private_size(path: &Path) -> Option<u64> {
    let path = c_path(path)?;
    u64::try_from(unsafe { nf_private_size(path.as_ptr()) }).ok()
}

/// Frames each window keeps for drawing (2 or 3; GPUI's own is 3): for the
/// windows open, and each one as it comes to the front.
pub fn set_drawables(count: u32) {
    unsafe { nf_set_drawables(count) }
}

/// Start Sparkle (if this build has it, and is set up for updates).
/// Whether updates are on.
pub fn updates_start() -> bool {
    unsafe { nf_updates_start() }
}

/// Look for an update now (Sparkle's window). False if updates are off.
pub fn updates_check() -> bool {
    unsafe { nf_updates_check() }
}

/// macOS's About panel, `credits` under the version.
pub fn about(credits: &str) {
    if let Ok(credits) = CString::new(credits) {
        unsafe { nf_about(credits.as_ptr()) }
    }
}

/// One of the app's Info.plist values ("CFBundleVersion"), if it's text.
pub fn bundle_value(key: &str) -> Option<String> {
    let key = CString::new(key).ok()?;
    take(unsafe { nf_bundle_value(key.as_ptr()) })
}

/// The app's own resource `name`, if it has it (a bundled build).
pub fn resource(name: &str) -> Option<PathBuf> {
    let name = CString::new(name).ok()?;
    take(unsafe { nf_resource(name.as_ptr()) }).map(PathBuf::from)
}

/// macOS's version: "Version 26.5 (Build 25F71)".
pub fn os_version() -> String {
    take(unsafe { nf_os_version() }).unwrap_or_default()
}

/// Whether Finder2 opens at login.
pub fn login_item_on() -> bool {
    unsafe { nf_login_item_on() }
}

/// Open at login, or not. Whether that worked.
pub fn set_login_item(on: bool) -> bool {
    unsafe { nf_login_item_set(on) }
}

/// Whether Spotlight still has ⌘Space (macOS's shortcut 64).
pub fn spotlight_shortcut_on() -> bool {
    unsafe { nf_symbolic_hotkey_on(64) }
}

/// Whether the app has Full Disk Access now (`full_disk_access` is as of
/// launch).
pub fn full_disk_access_now() -> bool {
    fsearch::has_full_disk_access()
}

/// The clipboard's change count: it goes up with each copy (anyone's).
pub fn pasteboard_count() -> i64 {
    unsafe { nf_pasteboard_count() }
}

/// What macOS does when this app reads the clipboard by itself (macOS
/// 15.4+): see `PasteAccess`.
pub fn pasteboard_access() -> PasteAccess {
    match unsafe { nf_pasteboard_access() } {
        0 => PasteAccess::NotAsked,
        1 => PasteAccess::Ask,
        3 => PasteAccess::Denied,
        _ => PasteAccess::Allowed,
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PasteAccess {
    /// It'll ask the first time.
    NotAsked,
    Ask,
    Allowed,
    Denied,
}

/// What's on the clipboard, as JSON (see `nf_pasteboard_take`); an image
/// on it is written to `image_out`.
pub fn pasteboard_take(image_out: &Path) -> Option<serde_json::Value> {
    let out = c_path(image_out)?;
    let json = take(unsafe { nf_pasteboard_take(out.as_ptr()) })?;
    serde_json::from_str(&json).ok()
}

/// The app in front: its name and bundle.
pub fn front_app() -> Option<(String, PathBuf)> {
    let text = take(unsafe { nf_front_app() })?;
    let (name, path) = text.split_once('\t')?;
    Some((name.to_string(), PathBuf::from(path)))
}

/// Whether this app may send keys to others (Accessibility); `prompt`:
/// ask macOS to let it, if it may not (it asks once).
pub fn accessibility(prompt: bool) -> bool {
    unsafe { nf_accessibility(prompt) }
}

/// ⌘V, to the app in front (with Accessibility).
pub fn post_paste() {
    unsafe { nf_post_paste() }
}

/// Whether ⌘, ⇧, ⌥ or ⌃ is held down now (a shortcut's, still).
pub fn modifiers_held() -> bool {
    unsafe { nf_modifiers_held() }
}

/// Whether this app may take screenshots (Screen Recording, in System
/// Settings > Privacy & Security).
pub fn capture_allowed() -> bool {
    unsafe { nf_capture_allowed() }
}

/// Ask to (macOS asks once; after that, only System Settings can allow it).
pub fn capture_request() -> bool {
    unsafe { nf_capture_request() }
}

/// An image's size in pixels, and its pixels per point (2 for a Retina
/// screenshot).
pub fn image_info(path: &Path) -> Option<(u32, u32, f32)> {
    let path = c_path(path)?;
    let (mut w, mut h, mut scale) = (0, 0, 1.);
    unsafe { nf_image_info(path.as_ptr(), &mut w, &mut h, &mut scale) }.then_some((w, h, scale as f32))
}

/// An image file onto the clipboard (as PNG and TIFF).
pub fn copy_image(path: &Path) -> bool {
    c_path(path).is_some_and(|path| unsafe { nf_copy_image(path.as_ptr()) })
}

/// The text in an image, a line each (Vision's), if any.
pub fn recognize_text(path: &Path) -> Option<String> {
    let path = c_path(path)?;
    take(unsafe { nf_recognize_text(path.as_ptr()) })
}

static PICKED: Mutex<Option<futures::channel::oneshot::Sender<Option<String>>>> = Mutex::new(None);

extern "C" fn on_picked(hex: *const c_char) {
    let hex = (!hex.is_null()).then(|| unsafe { CStr::from_ptr(hex) }.to_string_lossy().into_owned());
    if let Some(tx) = PICKED.lock().unwrap().take() {
        let _ = tx.send(hex);
    }
}

/// macOS's magnifier, to pick a color on screen: `picked` gets it as
/// "#RRGGBB" (None if put away).
pub fn pick_color(picked: futures::channel::oneshot::Sender<Option<String>>) {
    *PICKED.lock().unwrap() = Some(picked);
    unsafe { nf_pick_color(on_picked) }
}

/// `input`, pixelated all over in squares of `block` pixels, into `out`.
pub fn pixelate_file(input: &Path, out: &Path, block: f32) -> bool {
    match (c_path(input), c_path(out)) {
        (Some(input), Some(out)) => unsafe { nf_pixelate_file(input.as_ptr(), out.as_ptr(), block as f64) },
        _ => false,
    }
}

/// Draw an edited screenshot as `spec` says (see `shots::marks::spec`).
pub fn render_shot(spec: &str) -> Result<(), String> {
    let spec = CString::new(spec).map_err(|e| e.to_string())?;
    outcome(unsafe { nf_render_shot(spec.as_ptr()) })
}

/// Which of macOS's own screenshot shortcuts are on, by its ids (28 ⇧⌘3,
/// 29 ⌃⇧⌘3, 30 ⇧⌘4, 31 ⌃⇧⌘4, 184 ⇧⌘5).
pub fn system_screenshot_keys() -> Vec<u32> {
    let bits = unsafe { nf_system_screenshot_keys() };
    [28, 29, 30, 31, 184].into_iter().enumerate().filter(|(i, _)| bits & (1 << i) != 0).map(|(_, id)| id).collect()
}

/// Hear the shortcuts that work in any app (see `hot_key_set`): `pressed`
/// gets the id of the one pressed.
pub fn hot_keys_listen(pressed: extern "C" fn(u32)) {
    unsafe { nf_hot_keys_listen(pressed) }
}

/// Set shortcut `id` (that works in any app) to a key code and Carbon
/// modifiers, or None for none. False if another app holds it.
pub fn hot_key_set(id: u32, key: Option<(u32, u32)>) -> bool {
    let (code, modifiers) = key.unwrap_or((u32::MAX, 0));
    unsafe { nf_hot_key_set(id, code, modifiers) }
}

/// The apps running with a Dock icon (not this one): pid, name, bundle.
pub fn running_apps() -> Vec<(i32, String, PathBuf)> {
    let text = take(unsafe { nf_running_apps() }).unwrap_or_default();
    text.lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\t');
            let pid = parts.next()?.parse().ok()?;
            Some((pid, parts.next()?.to_string(), PathBuf::from(parts.next()?)))
        })
        .collect()
}

/// Whether F2 may send Apple Events to the app `bundle` (see
/// `music::Allowed`), macOS asking first if `ask` (it waits: not on the
/// main thread).
pub fn automation(bundle: &str, ask: bool) -> i32 {
    let Ok(bundle) = CString::new(bundle) else { return -1 };
    unsafe { nf_automation(bundle.as_ptr(), ask) }
}

/// Cleaning mode: keys and clicks off for `seconds` (with Accessibility),
/// a panel counting down. Whether it started.
pub fn clean_keyboard(seconds: u32) -> bool {
    unsafe { nf_clean_keyboard(seconds.min(600) as i32) }
}

/// Ask an app to quit (it may ask to save), or with `force` end it.
pub fn quit_app(pid: i32, force: bool) -> bool {
    unsafe { nf_quit_app(pid, force) }
}

/// Hide an app (as ⌘H does).
pub fn hide_app(pid: i32) -> bool {
    unsafe { nf_hide_app(pid) }
}

/// Lock the screen, as Control Center's Lock Screen does.
pub fn lock_screen() -> bool {
    unsafe { nf_lock_screen() }
}

/// Where the app with `bundle_id` is installed.
pub fn app_path(bundle_id: &str) -> Option<PathBuf> {
    let id = CString::new(bundle_id).ok()?;
    take(unsafe { nf_app_path(id.as_ptr()) }).map(PathBuf::from)
}

/// While Finder is the app in front, its ordinary windows on screen (their
/// window numbers); None when it isn't.
pub fn front_finder_windows() -> Option<Vec<u32>> {
    let mut ids = [0u32; 64];
    let n = unsafe { nf_front_finder_windows(ids.as_mut_ptr(), ids.len() as i32) };
    usize::try_from(n).ok().map(|n| ids[..n.min(ids.len())].to_vec())
}

/// Where read-only disk images (apps' DMGs) are mounted.
pub fn read_only_image_volumes() -> Vec<PathBuf> {
    lines(unsafe { nf_read_only_image_volumes() })
}

/// One of Finder's windows: its number (AppleScript's), its folder (None
/// for one that isn't of one: Recents, the computer, what its Dock icon
/// opens, say), its name.
#[derive(Debug, PartialEq)]
pub struct FinderWindow {
    pub id: i64,
    pub folder: Option<PathBuf>,
    pub name: String,
}

/// Finder's windows, front to back; None if Finder couldn't be asked.
/// macOS asks once whether this app may control Finder.
pub fn finder_windows() -> Option<Vec<FinderWindow>> {
    take(unsafe { nf_finder_windows() }).map(|out| finder_windows_in(&out))
}

/// Finder's answer: "id<tab>folder<tab>name" a line, the folder as Finder
/// gives it (with a slash at the end).
fn finder_windows_in(out: &str) -> Vec<FinderWindow> {
    out.lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\t');
            let id = parts.next()?.trim().parse().ok()?;
            let folder = parts.next()?;
            let folder = (!folder.is_empty()).then(|| PathBuf::from(if folder.len() > 1 { folder.trim_end_matches('/') } else { folder }));
            Some(FinderWindow { id, folder, name: parts.next().unwrap_or_default().to_string() })
        })
        .collect()
}

/// Close these of Finder's windows.
pub fn close_finder_windows(ids: &[i64]) {
    unsafe { nf_close_finder_windows(ids.as_ptr(), ids.len() as i32) }
}

/// `came_forward` (on the main thread) each time Finder comes to the front.
pub fn on_finder_forward(came_forward: extern "C" fn()) {
    unsafe { nf_on_finder_forward(came_forward) }
}

/// Have Finder empty the Trash (for when we may not touch it ourselves).
pub fn finder_empty_trash() -> Result<(), String> {
    let out = std::process::Command::new("/usr/bin/osascript")
        .args(["-e", "tell application \"Finder\" to empty trash"])
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&out.stderr).trim().to_string()) }
}

/// Send files with AirDrop (the system's window of who's nearby). False if
/// AirDrop can't take them. Call it outside any GPUI update: it shows UI.
pub fn airdrop(paths: &[PathBuf]) -> bool {
    joined(paths).is_some_and(|list| unsafe { nf_airdrop(list.as_ptr()) })
}

/// One PDF from images (a page each) and PDFs (all their pages).
pub fn images_to_pdf(paths: &[PathBuf], out: &Path) -> Result<(), String> {
    let list = joined(paths).ok_or("bad path")?;
    call(out, |o| unsafe { nf_images_to_pdf(list.as_ptr(), o) })
}

unsafe extern "C" {
    fn nf_zone_offset(zone: *const c_char, at: f64, offset: *mut i32) -> bool;
    fn nf_local_zone() -> *mut c_char;
    fn nf_zone_format(zone: *const c_char, at: f64, template: *const c_char) -> *mut c_char;
    fn nf_preferred_languages() -> *mut c_char;
    fn nf_define(word: *const c_char) -> *mut c_char;
    fn nf_wifi(set: i32) -> i32;
    fn nf_mac_languages() -> *mut c_char;
    fn nf_app_names(app: *const c_char, languages: *const c_char) -> *mut c_char;
    fn nf_bluetooth_allowed() -> i32;
    fn nf_bluetooth(set: i32) -> i32;
    fn nf_bluetooth_inputs() -> *mut c_char;
    fn nf_add_reminder(title: *const c_char, due: f64, timed: bool) -> *mut c_char;
    fn nf_reminders_due(limit: i32) -> *mut c_char;
    fn nf_reminder_done(identifier: *const c_char) -> *mut c_char;
    fn nf_can_remove_background() -> bool;
    fn nf_remove_background(path: *const c_char, out: *const c_char) -> *mut c_char;
    fn nf_shown_size(path: *const c_char, w: *mut u32, h: *mut u32, alpha: *mut bool) -> bool;
    fn nf_resize_image(path: *const c_char, longest: i32, kind: *const c_char, out: *const c_char) -> *mut c_char;
    fn nf_pdf_text(path: *const c_char) -> *mut c_char;
    fn nf_copy_secret(text: *const c_char, seconds: f64);
    fn nf_contacts_access() -> i32;
    fn nf_contacts_request() -> bool;
    fn nf_contacts_find(name: *const c_char, limit: i32) -> *mut c_char;
    fn nf_battery(percent: *mut i32, charging: *mut bool, plugged: *mut bool, minutes: *mut i32) -> bool;
    fn nf_audio_outputs() -> *mut c_char;
    fn nf_set_audio_output(device: u32) -> bool;
    fn nf_bluetooth_devices() -> *mut c_char;
    fn nf_bluetooth_connect(address: *const c_char, connect: bool);
    fn nf_bluetooth_connected(address: *const c_char) -> bool;
    fn nf_spell(word: *const c_char, british: bool) -> *mut c_char;
    fn nf_fix_spelling(text: *const c_char, unready: *mut bool) -> *mut c_char;
    fn nf_synonyms(word: *const c_char, british: bool) -> *mut c_char;
    fn nf_location_access() -> i32;
    fn nf_location_request();
    fn nf_wifi_network(device: *mut *mut c_char) -> *mut c_char;
    fn nf_night_shift(set: i32) -> i32;
    fn nf_desktop_icons(set: i32) -> i32;
    fn nf_show_desktop() -> bool;
    fn nf_selected_text(pid: i32) -> *mut c_char;
    fn nf_front_pid() -> i32;
    fn nf_windows() -> *mut c_char;
    fn nf_window_focus(pid: i32, number: u32) -> bool;
    fn nf_post_copy();
    fn nf_pasteboard_changes() -> std::ffi::c_long;
    fn nf_panel_took_from() -> i32;
    fn nf_import_listen(listener: extern "C" fn(*const c_char));
    fn nf_continuity_setup();
    fn nf_video_thumbnail(path: *const c_char, out: *const c_char, max: i32) -> bool;
    fn nf_read_rgba(path: *const c_char, w: *mut usize, h: *mut usize) -> *mut u8;
    fn nf_image_bitmap(path: *const c_char, max: i32, w: *mut u32, h: *mut u32) -> *mut u8;
    fn nf_picture_bitmap(path: *const c_char, scale: f64, pw: *mut f64, ph: *mut f64, w: *mut u32, h: *mut u32) -> *mut u8;
    fn nf_pdf_pages(
        path: *const c_char,
        first: std::ffi::c_long,
        most: std::ffi::c_long,
        width: i32,
        ctx: *mut std::ffi::c_void,
        page: extern "C" fn(*mut std::ffi::c_void, *mut u8, u32, u32),
    ) -> std::ffi::c_long;
    fn nf_quicklook_warm();
    fn nf_write_png(path: *const c_char, rgba: *const u8, w: usize, h: usize) -> bool;
    fn nf_pointer(x: *mut f64, y: *mut f64);
    fn nf_move_pointer(x: f64, y: f64);
    fn nf_scroll_at(x: f64, y: f64, dy: i32);
    fn nf_video_frames(
        video: *const c_char,
        width: i32,
        most: i32,
        ctx: *mut std::ffi::c_void,
        frame: extern "C" fn(*mut std::ffi::c_void, *const u8, usize, usize, usize, f64),
    ) -> *mut c_char;
    fn nf_pick_area(picked: extern "C" fn(f64, f64, f64, f64));
    fn nf_share_file(path: *const c_char, email: bool) -> bool;
    fn nf_set_wallpaper(path: *const c_char) -> *mut c_char;
    fn nf_region_code() -> *mut c_char;
    // (translate.swift's.)
    fn nf_language_of(text: *const c_char) -> *mut c_char;
    fn nf_translate(text: *const c_char, from: *const c_char, to: *const c_char, seconds: f64) -> *mut c_char;
    fn nf_keychain_set(service: *const c_char, account: *const c_char, secret: *const c_char) -> bool;
    fn nf_keychain_get(service: *const c_char, account: *const c_char) -> *mut c_char;
    fn nf_keychain_delete(service: *const c_char, account: *const c_char);
    // (dictation.m's.)
    fn nf_dictation_listen(heard: Option<extern "C" fn(i32, u16, u64, bool) -> bool>) -> bool;
    fn nf_key_is_down(code: u16) -> bool;
    fn nf_mic_access() -> i32;
    fn nf_mic_request();
    fn nf_mic_start(wanted: *const c_char);
    fn nf_mic_inputs() -> *mut c_char;
    fn nf_mic_chosen(wanted: *const c_char) -> *mut c_char;
    fn nf_mic_stop(heard: Option<extern "C" fn(*const f32, usize)>);
    fn nf_mic_error() -> *mut c_char;
    fn nf_dictation_hud(mode: i32);
    // (notch.m's, for the terminal.)
    fn nf_notch_band(display: u32, width: *mut f64, height: *mut f64);
    fn nf_terminal_window(view: *mut std::ffi::c_void, display: u32, width: f64, height: f64);
    // (terminal.m's.)
    fn nf_terminal_look(iterm2: bool) -> *mut c_char;
    fn nf_monospace_fonts() -> *mut c_char;
    fn nf_has_font_family(name: *const c_char) -> bool;
    fn nf_mouse_pressed() -> bool;
    // (typing.m's.)
    fn nf_typing_layout(id: *const c_char) -> *mut c_char;
    fn nf_typing_listen(heard: Option<extern "C" fn(i32, u16, u64, *mut TypingOut) -> bool>, changed: Option<extern "C" fn(i32, *const c_char)>) -> bool;
    // (notch.m's.)
    fn nf_notch_setup(on: bool, bare: bool, chose: extern "C" fn(i32, i64));
    fn nf_notch_live(json: *const c_char);
    fn nf_notch_note(title: *const c_char, detail: *const c_char, color: i64, symbol: *const c_char, tile: u32, link: *const c_char) -> bool;
    fn nf_notch_music(heard: Option<extern "C" fn(*const c_char)>);
    // (speech.swift's.)
    fn nf_speech_available() -> bool;
    fn nf_speech_locales() -> *mut c_char;
    fn nf_speech_resolve(id: *const c_char) -> *mut c_char;
    fn nf_speech_prepare(id: *const c_char) -> *mut c_char;
    fn nf_speech_progress() -> f64;
    fn nf_speech_warm(id: *const c_char);
    fn nf_speech_transcribe(id: *const c_char, samples: *const f32, count: usize, error: *mut *mut c_char) -> *mut c_char;
}

/// The notch on screen `display`: its width (0: it has none), and how
/// tall the band at the screen's top is (the notch, else the menu bar).
pub fn notch_band(display: u32) -> (f32, f32) {
    let (mut width, mut height) = (0., 0.);
    unsafe { nf_notch_band(display, &mut width, &mut height) };
    (width as f32, height as f32)
}

/// The notch's terminal's window set up: over the menu bar, at the top of
/// `display`, in its middle, `width` by `height`; on every space.
pub fn terminal_window(view: *mut std::ffi::c_void, display: u32, width: f32, height: f32) {
    unsafe { nf_terminal_window(view, display, width as f64, height as f64) }
}

/// Terminal's default profile's look (or iTerm2's), as JSON (src/macos/
/// terminal.m's nf_terminal_look).
pub fn terminal_look(iterm2: bool) -> Option<String> {
    take(unsafe { nf_terminal_look(iterm2) })
}

/// The monospaced fonts' families (on any thread).
pub fn monospace_fonts() -> Vec<String> {
    take(unsafe { nf_monospace_fonts() }).map(|s| s.lines().map(str::to_string).collect()).unwrap_or_default()
}

/// Whether a mouse button's held now.
pub fn mouse_pressed() -> bool {
    unsafe { nf_mouse_pressed() }
}

/// Whether a font family of that name is there (quick: not every font
/// listed, as GPUI's `all_font_names` does).
pub fn has_font_family(name: &str) -> bool {
    CString::new(name).is_ok_and(|c| unsafe { nf_has_font_family(c.as_ptr()) })
}

/// What Typing's tap types in place of a key (src/macos/typing.m's
/// nf_typing_out): `back` characters taken back (`guard` typed first, if
/// it isn't 0, and taken back too), then `text`, then the key itself after
/// all (`key`).
#[repr(C)]
pub struct TypingOut {
    pub back: u32,
    pub guard: u16,
    pub length: u32,
    pub text: [u16; 256],
    pub key: bool,
}

/// The keys a keyboard layout types (`id`'s, "com.apple.keylayout.Icelandic";
/// None: the Mac's own now, if it's a layout for English), as JSON: see
/// typing.m. On the main thread.
pub fn typing_layout(id: Option<&str>) -> Option<String> {
    let id = id.map(|id| CString::new(id).unwrap_or_default());
    take(unsafe { nf_typing_layout(id.as_ref().map_or(std::ptr::null(), |id| id.as_ptr())) })
}

/// Hear keys pressed and clicks in any app for Typing (with Accessibility):
/// `heard` gets each, on a thread of its own (1: a key down; 0: a click; 2:
/// a key F2 sent itself; its code; the modifiers), true keeping it from the
/// app and typing what it filled in instead. `changed` hears of the input
/// source changing (0) and another app coming to the front (1, with its
/// bundle id: at once too), on the main thread. None: stop. False if it
/// can't.
pub fn typing_listen(heard: Option<extern "C" fn(i32, u16, u64, *mut TypingOut) -> bool>, changed: Option<extern "C" fn(i32, *const c_char)>) -> bool {
    unsafe { nf_typing_listen(heard, changed) }
}

/// Hear every key in any app (with Accessibility): `heard` gets each, on
/// a thread of its own (what it is, 0: a modifier changed, 1: down, 2: up;
/// its code; the modifiers; whether it's repeating), true keeping it from
/// the app. None: stop. False if it can't. On the main thread.
pub fn dictation_listen(heard: Option<extern "C" fn(i32, u16, u64, bool) -> bool>) -> bool {
    unsafe { nf_dictation_listen(heard) }
}

/// Whether a key's down now (by its virtual key code).
pub fn key_is_down(code: u16) -> bool {
    unsafe { nf_key_is_down(code) }
}

/// The microphone's allowed (None: not asked yet).
pub fn mic_access() -> Option<bool> {
    match unsafe { nf_mic_access() } {
        -1 => None,
        n => Some(n == 1),
    }
}

/// Ask for the microphone (macOS asks, once).
pub fn mic_request() {
    unsafe { nf_mic_request() }
}

/// Start hearing the microphone `wanted` (a microphone's UID, "system":
/// Sound settings', "": by itself; see `mic_chosen`). See `mic_error`.
pub fn mic_start(wanted: &str) {
    let wanted = CString::new(wanted).unwrap_or_default();
    unsafe { nf_mic_start(wanted.as_ptr()) }
}

/// A microphone: its UID (it keeps it), name, kind ("builtin",
/// "bluetooth", "iphone", "usb", "other"), and whether it's Sound
/// settings' input.
#[derive(Clone, PartialEq, Debug)]
pub struct Input {
    pub uid: String,
    pub name: String,
    pub kind: String,
    pub default: bool,
}

pub fn mic_inputs() -> Vec<Input> {
    take(unsafe { nf_mic_inputs() })
        .unwrap_or_default()
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            Some(Input { uid: parts.next()?.into(), name: parts.next()?.into(), kind: parts.next()?.into(), default: parts.next()? == "1" })
        })
        .collect()
}

/// The UID of the microphone `wanted` comes to now: Sound settings' input,
/// unless that's Bluetooth headphones (they'd switch to call quality), then
/// the Mac's own (unless its lid's closed) or another.
pub fn mic_chosen(wanted: &str) -> Option<String> {
    let wanted = CString::new(wanted).ok()?;
    take(unsafe { nf_mic_chosen(wanted.as_ptr()) })
}

/// Stop hearing it (after what was asked before): `heard` gets what it
/// heard, 16 kHz mono, on a queue of its own (None: it's thrown away).
/// Returns at once.
pub fn mic_stop(heard: Option<extern "C" fn(*const f32, usize)>) {
    unsafe { nf_mic_stop(heard) }
}

/// Why the microphone didn't start, if it didn't.
pub fn mic_error() -> Option<String> {
    take(unsafe { nf_mic_error() })
}

/// Dictation's pill, at the bottom of the screen.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Hud {
    Hidden = 0,
    Listening = 1,
    HandsFree = 2,
    Working = 3,
}

pub fn dictation_hud(hud: Hud) {
    unsafe { nf_dictation_hud(hud as i32) }
}

/// The notch (notch.m): on or off; on screens without one too (`bare`);
/// and what's chosen in it (see `notch::chose`). On the main thread, as
/// the rest of these.
pub fn notch_setup(on: bool, bare: bool, chose: extern "C" fn(i32, i64)) {
    unsafe { nf_notch_setup(on, bare, chose) }
}

/// What's going on, for the notch: JSON, as notch.m's `nf_notch_live`
/// says.
pub fn notch_live(json: &str) {
    let json = CString::new(json).unwrap_or_default();
    unsafe { nf_notch_live(json.as_ptr()) }
}

/// A note, down from the notch on the screen with the pointer, with its
/// color, else a symbol on a tile (that color): false if that screen can't
/// show it.
pub fn notch_note(title: &str, detail: &str, color: Option<u32>, (symbol, tile): (&str, u32), link: Option<&str>) -> bool {
    let (title, detail, symbol) = (CString::new(title).unwrap_or_default(), CString::new(detail).unwrap_or_default(), CString::new(symbol).unwrap_or_default());
    let link = link.and_then(|l| CString::new(l).ok());
    unsafe {
        nf_notch_note(
            title.as_ptr(),
            detail.as_ptr(),
            color.map_or(-1, i64::from),
            symbol.as_ptr(),
            tile,
            link.as_ref().map_or(std::ptr::null(), |l| l.as_ptr()),
        )
    }
}

/// What Music and Spotify say is playing, as it changes (JSON: player,
/// state, title, artist, album). None: no more.
pub fn notch_music(heard: Option<extern "C" fn(*const c_char)>) {
    unsafe { nf_notch_music(heard) }
}

/// Whether Apple's speech recognition runs here (macOS 26).
pub fn speech_available() -> bool {
    unsafe { nf_speech_available() }
}

/// The languages it takes: code, name, and whether its model's here.
/// Waits: not on the main thread.
pub fn speech_locales() -> Vec<(String, String, bool)> {
    take(unsafe { nf_speech_locales() })
        .unwrap_or_default()
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            Some((parts.next()?.to_string(), parts.next()?.to_string(), parts.next() == Some("1")))
        })
        .collect()
}

/// The language it'd use for `id` ("": this Mac's). Waits.
pub fn speech_resolve(id: &str) -> Option<String> {
    let id = CString::new(id).ok()?;
    take(unsafe { nf_speech_resolve(id.as_ptr()) })
}

/// Ready for `id`: its model downloaded. Waits.
pub fn speech_prepare(id: &str) -> Result<(), String> {
    let id = CString::new(id).map_err(|e| e.to_string())?;
    match take(unsafe { nf_speech_prepare(id.as_ptr()) }) {
        None => Ok(()),
        Some(why) => Err(why),
    }
}

/// How far a model's download has got (0 to 1), if one's going.
pub fn speech_progress() -> Option<f64> {
    Some(unsafe { nf_speech_progress() }).filter(|p| *p >= 0.)
}

pub fn speech_warm(id: &str) {
    if let Ok(id) = CString::new(id) {
        unsafe { nf_speech_warm(id.as_ptr()) }
    }
}

/// What's said in `audio` (16 kHz mono), in `id`. Waits.
pub fn speech_transcribe(id: &str, audio: &[f32]) -> Result<String, String> {
    let id = CString::new(id).map_err(|e| e.to_string())?;
    let mut error = std::ptr::null_mut();
    match take(unsafe { nf_speech_transcribe(id.as_ptr(), audio.as_ptr(), audio.len(), &mut error) }) {
        Some(text) => Ok(text),
        None => Err(take(error).unwrap_or_else(|| "It didn't say why.".into())),
    }
}

/// A time zone's offset from UTC at `at` (seconds since 1970), in seconds:
/// `zone` by its name ("Asia/Tokyo") or "offset:<seconds>". None if macOS
/// doesn't know it.
pub fn zone_offset(zone: &str, at: i64) -> Option<i32> {
    let zone = CString::new(zone).ok()?;
    let mut offset = 0;
    unsafe { nf_zone_offset(zone.as_ptr(), at as f64, &mut offset) }.then_some(offset)
}

/// This Mac's time zone's name.
pub fn local_zone() -> String {
    take(unsafe { nf_local_zone() }).unwrap_or_else(|| "UTC".into())
}

/// `at` in `zone`, as this Mac writes it, from a template of what to show
/// ("jmm": "17:05", or "5:05 PM" with 12-hour time).
pub fn zone_format(zone: &str, at: i64, template: &str) -> String {
    let (Ok(zone), Ok(template)) = (CString::new(zone), CString::new(template)) else { return String::new() };
    take(unsafe { nf_zone_format(zone.as_ptr(), at as f64, template.as_ptr()) }).unwrap_or_default()
}

/// The languages this Mac is set to, first first ("en-US").
pub fn preferred_languages() -> Vec<String> {
    take(unsafe { nf_preferred_languages() }).map_or_else(Vec::new, |l| l.split(',').filter(|s| !s.is_empty()).map(String::from).collect())
}

/// The languages this Mac reads or types: the ones it's set to, then its
/// input menu's keyboards' ("en", "ko", "ja"). Looked up again only on the
/// main thread (elsewhere: as it was last looked up).
pub fn mac_languages() -> Vec<String> {
    take(unsafe { nf_mac_languages() }).map_or_else(Vec::new, |l| l.split(',').filter(|s| !s.is_empty()).map(String::from).collect())
}

/// What Finder calls an app ("メモ", on a Mac in Japanese), then what it
/// calls itself in `languages`, for those it has.
pub fn app_names(app: &Path, languages: &[String]) -> Vec<String> {
    let (Some(app), Ok(languages)) = (c_path(app), CString::new(languages.join(","))) else { return Vec::new() };
    take(unsafe { nf_app_names(app.as_ptr(), languages.as_ptr()) }).map_or_else(Vec::new, |l| l.lines().filter(|s| !s.is_empty()).map(String::from).collect())
}

/// The Dictionary's text for a word, if it has the word.
pub fn define(word: &str) -> Option<String> {
    let word = CString::new(word).ok()?;
    take(unsafe { nf_define(word.as_ptr()) })
}

/// Wi-Fi's power (None: no Wi-Fi), turned on or off first with `set`.
pub fn wifi(set: Option<bool>) -> Option<bool> {
    match unsafe { nf_wifi(set.map_or(-1, i32::from)) } {
        -1 => None,
        on => Some(on == 1),
    }
}

/// Whether F2 may use Bluetooth: Some(true) yes, Some(false) no, None not
/// asked yet.
pub fn bluetooth_allowed() -> Option<bool> {
    match unsafe { nf_bluetooth_allowed() } {
        0 => None,
        allowed => Some(allowed == 1),
    }
}

/// Bluetooth's power, turned on or off first with `set`. (The first time,
/// macOS asks whether F2 may.)
pub fn bluetooth(set: Option<bool>) -> bool {
    unsafe { nf_bluetooth(set.map_or(-1, i32::from)) == 1 }
}

/// The keyboards, mice and trackpads connected over Bluetooth, by name.
pub fn bluetooth_inputs() -> Vec<String> {
    take(unsafe { nf_bluetooth_inputs() }).map_or_else(Vec::new, |l| l.lines().filter(|s| !s.is_empty()).map(String::from).collect())
}

/// Add a reminder to Reminders' default list, due at `due` (seconds since
/// 1970), with its time and an alert (`timed`) or on that day. Asks for
/// access the first time, and waits: not on the main thread. Err("denied")
/// if Reminders is off limits.
pub fn add_reminder(title: &str, due: Option<i64>, timed: bool) -> Result<(), String> {
    let title = CString::new(title).map_err(|e| e.to_string())?;
    match take(unsafe { nf_add_reminder(title.as_ptr(), due.map_or(-1., |d| d as f64), timed) }) {
        None => Ok(()),
        Some(why) => Err(why),
    }
}

/// A reminder not done yet.
#[derive(Clone, Debug)]
pub struct Todo {
    pub id: String,
    pub title: String,
    /// When it's due (seconds since 1970), and whether at that time.
    pub due: Option<i64>,
    pub timed: bool,
    pub list: String,
}

/// Reminders not done: due by the end of today (overdue first), then those
/// with no date, at most `limit`. None: Reminders is off limits. (Waits on
/// Reminders: not on the main thread.)
pub fn reminders_due(limit: i32) -> Option<Vec<Todo>> {
    let text = take(unsafe { nf_reminders_due(limit) })?;
    Some(
        text.lines()
            .filter_map(|line| {
                let mut parts = line.split('\t');
                let id = parts.next()?.to_string();
                let title = parts.next()?.to_string();
                let due = parts.next()?.parse::<f64>().ok().filter(|d| *d >= 0.).map(|d| d as i64);
                let timed = parts.next()? == "1";
                let list = parts.next().unwrap_or("").to_string();
                Some(Todo { id, title, due, timed, list })
            })
            .collect(),
    )
}

/// A reminder checked off.
pub fn reminder_done(id: &str) -> Result<(), String> {
    let id = CString::new(id).map_err(|e| e.to_string())?;
    match take(unsafe { nf_reminder_done(id.as_ptr()) }) {
        None => Ok(()),
        Some(why) => Err(why),
    }
}

/// What `text` is in (a BCP 47 code: "es"), as NaturalLanguage guesses.
pub fn language_of(text: &str) -> Option<String> {
    let text = CString::new(text).ok()?;
    take(unsafe { nf_language_of(text.as_ptr()) })
}

/// `text` translated from `from` into `to` on this Mac (Apple's Translation,
/// macOS 26), waiting up to `seconds`: not on the main thread. Err: why not,
/// "unsupported" (not a pair Apple translates), "not-installed" (its
/// languages aren't downloaded), "old-macos", "timed-out", or the error.
pub fn translate(text: &str, from: &str, to: &str, seconds: f64) -> Result<String, String> {
    let (Ok(text), Ok(from), Ok(to)) = (CString::new(text), CString::new(from), CString::new(to)) else { return Err("bad text".into()) };
    let out = take(unsafe { nf_translate(text.as_ptr(), from.as_ptr(), to.as_ptr(), seconds) }).unwrap_or_default();
    match out.strip_prefix('=') {
        Some(done) => Ok(done.to_string()),
        None => Err(out),
    }
}

/// `secret` kept in the login keychain, as F2's password item `service`,
/// `account` (replacing what was). Whether that worked.
pub fn keychain_set(service: &str, account: &str, secret: &str) -> bool {
    let (Ok(service), Ok(account), Ok(secret)) = (CString::new(service), CString::new(account), CString::new(secret)) else { return false };
    unsafe { nf_keychain_set(service.as_ptr(), account.as_ptr(), secret.as_ptr()) }
}

/// What's kept in the keychain as `service`, `account` (F2's own).
pub fn keychain_get(service: &str, account: &str) -> Option<String> {
    let (Ok(service), Ok(account)) = (CString::new(service), CString::new(account)) else { return None };
    take(unsafe { nf_keychain_get(service.as_ptr(), account.as_ptr()) })
}

pub fn keychain_delete(service: &str, account: &str) {
    if let (Ok(service), Ok(account)) = (CString::new(service), CString::new(account)) {
        unsafe { nf_keychain_delete(service.as_ptr(), account.as_ptr()) }
    }
}

/// Whether a photo's background can be taken out here (macOS 14 and later).
pub fn can_remove_background() -> bool {
    unsafe { nf_can_remove_background() }
}

/// The photo at `path` with its background taken out, as a PNG at `out`
/// (Vision's: not on the main thread).
pub fn remove_background(path: &Path, out: &Path) -> Result<(), String> {
    let (path, out) = (c_path(path).ok_or("bad path")?, c_path(out).ok_or("bad path")?);
    take(unsafe { nf_remove_background(path.as_ptr(), out.as_ptr()) }).map_or(Ok(()), Err)
}

/// An image's size in pixels as it's shown (a phone's photo turned
/// upright), and whether it can be see-through. Reads only its header.
pub fn shown_size(path: &Path) -> Option<(u32, u32, bool)> {
    let path = c_path(path)?;
    let (mut w, mut h, mut alpha) = (0, 0, false);
    unsafe { nf_shown_size(path.as_ptr(), &mut w, &mut h, &mut alpha) }.then_some((w, h, alpha))
}

/// `path` resized to `longest` pixels on its longer side, upright, as
/// `kind` (a UTI) at `out`, its metadata kept. (Off the main thread.)
pub fn resize_image(path: &Path, longest: u32, kind: &str, out: &Path) -> Result<(), String> {
    let (path, out, kind) = (c_path(path).ok_or("bad path")?, c_path(out).ok_or("bad path")?, CString::new(kind).map_err(|e| e.to_string())?);
    take(unsafe { nf_resize_image(path.as_ptr(), longest as i32, kind.as_ptr(), out.as_ptr()) }).map_or(Ok(()), Err)
}

/// A PDF's text: what's typed in it, else read off its pages (a scan).
pub fn pdf_text(path: &Path) -> Option<String> {
    let path = c_path(path)?;
    take(unsafe { nf_pdf_text(path.as_ptr()) })
}

/// Copy a password, out of clipboard histories' way, and off the clipboard
/// again after `seconds` (unless something's been copied since).
pub fn copy_secret(text: &str, seconds: f64) {
    if let Ok(text) = CString::new(text) {
        unsafe { nf_copy_secret(text.as_ptr(), seconds) }
    }
}

/// Whether F2 may look at Contacts: Some(true) yes, Some(false) no, None
/// not asked yet.
pub fn contacts_access() -> Option<bool> {
    match unsafe { nf_contacts_access() } {
        0 => None,
        access => Some(access == 1),
    }
}

/// Ask for Contacts (macOS asks the first time), waiting: not on the main
/// thread. Whether it's allowed.
pub fn contacts_request() -> bool {
    unsafe { nf_contacts_request() }
}

/// A way to reach someone: a phone number or an email address.
#[derive(Clone, Debug, PartialEq)]
pub struct Reach {
    pub name: String,
    pub email: bool,
    /// "mobile", "home", "work", as Contacts says it.
    pub label: String,
    pub value: String,
}

/// The phone numbers and email addresses of the people named like `name`
/// (None: Contacts can't be looked at).
pub fn contacts_find(name: &str, limit: usize) -> Option<Vec<Reach>> {
    let name = CString::new(name).ok()?;
    let found = take(unsafe { nf_contacts_find(name.as_ptr(), limit as i32) })?;
    Some(
        found
            .lines()
            .filter_map(|line| {
                let mut parts = line.splitn(4, '\t');
                let (name, kind, label, value) = (parts.next()?, parts.next()?, parts.next()?, parts.next()?);
                Some(Reach { name: name.into(), email: kind == "email", label: label.into(), value: value.into() })
            })
            .collect(),
    )
}

/// The Mac's battery.
#[derive(Clone, Copy, Debug)]
pub struct Battery {
    pub percent: i32,
    pub charging: bool,
    pub plugged: bool,
    /// Till it's full (charging) or empty; None: not known yet.
    pub minutes: Option<i32>,
}

/// The Mac's battery (None: a Mac without one).
pub fn battery() -> Option<Battery> {
    let (mut percent, mut charging, mut plugged, mut minutes) = (0, false, false, -1);
    unsafe { nf_battery(&mut percent, &mut charging, &mut plugged, &mut minutes) }.then_some(Battery { percent, charging, plugged, minutes: (minutes >= 0).then_some(minutes) })
}

/// A sound output: its CoreAudio id, name, kind ("builtin", "bluetooth",
/// "airplay", "display", "usb", "other"), and whether it's the one in use.
#[derive(Clone, Debug)]
pub struct Output {
    pub id: u32,
    pub name: String,
    pub kind: String,
    pub used: bool,
}

pub fn audio_outputs() -> Vec<Output> {
    take(unsafe { nf_audio_outputs() })
        .unwrap_or_default()
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            Some(Output { id: parts.next()?.parse().ok()?, name: parts.next()?.into(), kind: parts.next()?.into(), used: parts.next()? == "1" })
        })
        .collect()
}

/// Play sound through this output. Whether it took.
pub fn set_audio_output(id: u32) -> bool {
    unsafe { nf_set_audio_output(id) }
}

/// The Bluetooth devices paired with the Mac: address, name, whether it's
/// connected. (Only once Bluetooth's allowed: else macOS asks.)
pub fn bluetooth_devices() -> Vec<(String, String, bool)> {
    take(unsafe { nf_bluetooth_devices() })
        .unwrap_or_default()
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            Some((parts.next()?.to_string(), parts.next()?.to_string(), parts.next()? == "1"))
        })
        .collect()
}

/// Connect to a paired device, or disconnect it (connecting goes on after:
/// see `bluetooth_connected`).
pub fn bluetooth_connect(address: &str, connect: bool) {
    if let Ok(address) = CString::new(address) {
        unsafe { nf_bluetooth_connect(address.as_ptr(), connect) }
    }
}

pub fn bluetooth_connected(address: &str) -> bool {
    CString::new(address).is_ok_and(|a| unsafe { nf_bluetooth_connected(a.as_ptr()) })
}

/// Spelling: Ok if `word` is spelled right, else Err(its guesses, the
/// likeliest first).
pub fn spell(word: &str) -> Result<(), Vec<String>> {
    let Ok(word) = CString::new(word) else { return Ok(()) };
    let out = take(unsafe { nf_spell(word.as_ptr(), nf_prefers_british()) }).unwrap_or_else(|| "1".into());
    let mut lines = out.lines();
    match lines.next() {
        Some("0") => Err(lines.map(String::from).collect()),
        _ => Ok(()),
    }
}

/// `text` with its misspellings fixed (macOS's spell checker's surest
/// corrections, in its language as you have it; names and code left
/// alone), and each fix, from and to. None: nothing to fix. `patient`:
/// asked again a moment later if the spell checker, just woken, didn't
/// have a correction yet. Not on the main thread.
pub fn fix_spelling(text: &str, patient: bool) -> Option<(String, Vec<(String, String)>)> {
    #[derive(serde::Deserialize)]
    struct Fixed {
        text: String,
        fixes: Vec<(String, String)>,
    }
    let text = CString::new(text).ok()?;
    let once = || {
        let mut unready = false;
        let out = take(unsafe { nf_fix_spelling(text.as_ptr(), &mut unready) });
        (out, unready)
    };
    let (mut out, unready) = once();
    if unready && patient {
        std::thread::sleep(std::time::Duration::from_millis(300));
        out = once().0;
    }
    let fixed: Fixed = serde_json::from_str(&out?).ok()?;
    Some((fixed.text, fixed.fixes))
}

/// A word's entry in macOS's thesaurus, as text (None: not in it).
pub fn synonyms(word: &str) -> Option<String> {
    let word = CString::new(word).ok()?;
    take(unsafe { nf_synonyms(word.as_ptr(), nf_prefers_british()) })
}

/// Whether F2 may know where the Mac is (only for the Wi-Fi network's
/// name): Some(true) yes, Some(false) no, None not asked yet.
pub fn location_access() -> Option<bool> {
    match unsafe { nf_location_access() } {
        0 => None,
        access => Some(access == 1),
    }
}

/// Ask for it (macOS asks, the first time).
pub fn location_request() {
    unsafe { nf_location_request() }
}

/// The Wi-Fi network the Mac's on (None: none, or Location's not allowed),
/// and the Wi-Fi's interface ("en0").
pub fn wifi_network() -> (Option<String>, Option<String>) {
    let mut device = std::ptr::null_mut();
    let name = take(unsafe { nf_wifi_network(&mut device) });
    (name, take(device))
}

/// Night Shift: Some(on), None if this Mac hasn't it. `set` turns it on
/// or off first.
pub fn night_shift(set: Option<bool>) -> Option<bool> {
    match unsafe { nf_night_shift(set.map_or(-1, i32::from)) } {
        -1 => None,
        on => Some(on == 1),
    }
}

/// Whether the desktop shows its icons; `set` hides or shows them first
/// (Finder restarts for it).
pub fn desktop_icons(set: Option<bool>) -> bool {
    unsafe { nf_desktop_icons(set.map_or(-1, i32::from)) == 1 }
}

/// Mission Control's Show Desktop (again: the windows back).
pub fn show_desktop() -> bool {
    unsafe { nf_show_desktop() }
}

/// The text selected in the app `pid`, if it says (through Accessibility).
pub fn selected_text(pid: i32) -> Option<String> {
    take(unsafe { nf_selected_text(pid) }).filter(|t| !t.trim().is_empty())
}

/// A video's frames, one at a time and in order: each as RGBA, its width
/// and height, and how long it shows; `width` pixels wide at most, up to 12
/// a second, no more than `most` (see `nf_video_frames`). Only one's in
/// memory at a time. Waits: not on the main thread.
pub fn video_frames(video: &Path, width: u32, most: u32, each: impl FnMut(&mut [u8], usize, usize, f64)) -> Result<(), String> {
    type Each<'a> = std::sync::Mutex<Box<dyn FnMut(&mut [u8], usize, usize, f64) + 'a>>;
    extern "C" fn frame(ctx: *mut std::ffi::c_void, rgba: *const u8, w: usize, h: usize, stride: usize, secs: f64) {
        let each = unsafe { &*(ctx as *const Each) };
        // (Its rows packed: a bitmap's can be padded.)
        let mut packed = Vec::with_capacity(w * h * 4);
        for row in 0..h {
            packed.extend_from_slice(unsafe { std::slice::from_raw_parts(rgba.add(row * stride), w * 4) });
        }
        if let Ok(mut each) = each.lock() {
            each(&mut packed, w, h, secs);
        }
    }
    let video = c_path(video).ok_or("bad path")?;
    let each: Each = std::sync::Mutex::new(Box::new(each));
    outcome(unsafe { nf_video_frames(video.as_ptr(), width as i32, most as i32, &each as *const Each as *mut std::ffi::c_void, frame) })
}

/// An image file's pixels: sRGB RGBA, rows packed, and its width and
/// height. None if it can't be read.
pub fn read_rgba(path: &Path) -> Option<(Vec<u8>, usize, usize)> {
    let path = c_path(path)?;
    let (mut w, mut h) = (0usize, 0usize);
    let data = unsafe { nf_read_rgba(path.as_ptr(), &mut w, &mut h) };
    if data.is_null() {
        return None;
    }
    let pixels = unsafe { std::slice::from_raw_parts(data, w * h * 4) }.to_vec();
    unsafe { libc::free(data.cast()) };
    Some((pixels, w, h))
}

/// sRGB RGBA pixels (rows packed) as a PNG.
pub fn write_png(path: &Path, rgba: &[u8], w: usize, h: usize) -> Result<(), String> {
    let path = c_path(path).ok_or("bad path")?;
    if rgba.len() != w * h * 4 {
        return Err("the picture's the wrong size".into());
    }
    if unsafe { nf_write_png(path.as_ptr(), rgba.as_ptr(), w, h) } { Ok(()) } else { Err("it couldn't be saved".into()) }
}

/// Where the pointer is (points from the main screen's top left).
pub fn pointer() -> (f64, f64) {
    let (mut x, mut y) = (0., 0.);
    unsafe { nf_pointer(&mut x, &mut y) };
    (x, y)
}

pub fn move_pointer(x: f64, y: f64) {
    unsafe { nf_move_pointer(x, y) }
}

/// Scroll what's under (x, y) by `dy` points (negative: down the page),
/// as a trackpad would (with Accessibility).
pub fn scroll_at(x: f64, y: f64, dy: i32) {
    unsafe { nf_scroll_at(x, y, dy) }
}

/// One of the apps' windows (see `app_windows`): its app, its number (the
/// window server's), its title, and whether it's minimized or its app
/// hidden.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct AppWindow {
    pub pid: i32,
    pub id: u32,
    pub title: String,
    pub app: String,
    pub path: PathBuf,
    pub minimized: bool,
    pub hidden: bool,
    /// On another desktop.
    #[serde(default)]
    pub elsewhere: bool,
}

/// The apps' windows, front to back as they're stacked; then those
/// minimized, hidden or on another desktop. None without Accessibility.
/// It asks each app (the first time a while): not on the main thread.
pub fn app_windows() -> Option<Vec<AppWindow>> {
    let listed = take(unsafe { nf_windows() })?;
    // (Unreadable: none listed, said in the log; not "no Accessibility".)
    Some(serde_json::from_str(&listed).unwrap_or_else(|e| {
        eprintln!("windows: couldn't read the list: {e}");
        Vec::new()
    }))
}

/// Bring an app's window forward (see `app_windows`), its app in front:
/// whether it's still there. Not on the main thread.
pub fn focus_window(pid: i32, id: u32) -> bool {
    unsafe { nf_window_focus(pid, id) }
}

/// The app in front, and the one Quick Open took the keyboard from (0:
/// none).
pub fn front_pid() -> i32 {
    unsafe { nf_front_pid() }
}

pub fn panel_took_from() -> i32 {
    unsafe { nf_panel_took_from() }
}

/// ⌘C to the app in front; the clipboard's change count, to tell it took.
pub fn post_copy() {
    unsafe { nf_post_copy() }
}

pub fn pasteboard_changes() -> i64 {
    unsafe { nf_pasteboard_changes() as i64 }
}

/// Told of each photo or scan Continuity Camera's saved (see `MenuItem`'s
/// `import_to`), on the main thread.
pub fn import_listen(listener: extern "C" fn(*const c_char)) {
    unsafe { nf_import_listen(listener) }
}

/// Continuity Camera's item kept in the menu bar (hidden: macOS fills in
/// only that one, with your devices), for the right-click menus to borrow.
/// Once the menu bar's up.
pub fn continuity_setup() {
    unsafe { nf_continuity_setup() }
}

/// A video's frame, half a second in, as a PNG at `out` (`max` pixels on
/// its long side). Whether it's made (not on the main thread: it waits).
pub fn video_thumbnail(path: &Path, out: &Path, max: i32) -> bool {
    match (c_path(path), c_path(out)) {
        (Some(path), Some(out)) => unsafe { nf_video_thumbnail(path.as_ptr(), out.as_ptr(), max) },
        _ => false,
    }
}

static AREA: Mutex<Option<futures::channel::oneshot::Sender<Option<(f64, f64, f64, f64)>>>> = Mutex::new(None);

extern "C" fn on_area(x: f64, y: f64, w: f64, h: f64) {
    if let Some(tx) = AREA.lock().unwrap().take() {
        let _ = tx.send((w > 0. && h > 0.).then_some((x, y, w, h)));
    }
}

/// An area dragged out over the screens (to record): x, y, width and height
/// in points from the main screen's top left; None if put away.
pub fn pick_area(picked: futures::channel::oneshot::Sender<Option<(f64, f64, f64, f64)>>) {
    *AREA.lock().unwrap() = Some(picked);
    unsafe { nf_pick_area(on_area) }
}

/// Send a file by AirDrop (macOS's picker of who's nearby), or in a new
/// email with it attached. Whether it could be.
pub fn share_file(path: &Path, email: bool) -> bool {
    c_path(path).is_some_and(|p| unsafe { nf_share_file(p.as_ptr(), email) })
}

/// An image as the desktop picture, on every screen.
pub fn set_wallpaper(path: &Path) -> Result<(), String> {
    let path = c_path(path).ok_or("bad path")?;
    take(unsafe { nf_set_wallpaper(path.as_ptr()) }).map_or(Ok(()), Err)
}

/// The Mac's region ("JP", "IS", "US").
pub fn region_code() -> Option<String> {
    take(unsafe { nf_region_code() })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    /// Finder's script compiles (compiled only: nothing's asked of Finder),
    /// quotes and backslashes in names and all.
    #[test]
    fn finder_trash_script_compiles() {
        let script = super::finder_trash_script(&[PathBuf::from("/Applications/Say \"hi\".app"), PathBuf::from("/tmp/a\\b")]);
        assert!(script.contains(r#"(POSIX file "/Applications/Say \"hi\".app") as alias, (POSIX file "/tmp/a\\b") as alias"#), "{script}");
        let out = std::env::temp_dir().join(format!("f2-trash-script-{}.scpt", std::process::id()));
        let compiled = std::process::Command::new("/usr/bin/osacompile").arg("-o").arg(&out).args(["-e", &script]).output().unwrap();
        let _ = std::fs::remove_file(&out);
        assert!(compiled.status.success(), "{}", String::from_utf8_lossy(&compiled.stderr));
    }

    /// What isn't this user's to move is said to be so (for Finder to
    /// move, with a password): here, a file in a folder that can't be
    /// written to. (It isn't moved: nothing reaches the Trash.)
    #[test]
    fn trash_says_when_denied() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("f2-denied-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("kept.txt");
        std::fs::write(&file, "x").unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
        let result = super::try_trash(&file);
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        let still = file.exists();
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(matches!(result, Err((_, true))), "{result:?}");
        assert!(still, "not moved");
    }

    /// This Mac's windows, read: `cargo test --release -- --ignored --nocapture windows_here`
    #[test]
    #[ignore]
    fn windows_here() {
        println!("trusted: {}", super::accessibility(false));
        let raw = super::take(unsafe { super::nf_windows() });
        println!("raw: {:?}", raw.as_ref().map(|r| r.len()));
        if let Some(raw) = &raw {
            let parsed: Result<Vec<super::AppWindow>, _> = serde_json::from_str(raw);
            println!("parsed: {:?}", parsed.as_ref().map(Vec::len).map_err(|e| e.to_string()));
            for w in parsed.iter().flatten() {
                println!("  {:<22} title {:>3} chars{}{}{}", w.app, w.title.chars().count(), if w.elsewhere { " · elsewhere" } else { "" }, if w.minimized { " · minimized" } else { "" }, if w.hidden { " · hidden" } else { "" });
            }
        }
    }

    #[test]
    fn finder_windows_as_finder_says_them() {
        let out = "41\t/Users/me/Documents/\tDocuments\n42\t\tRecents\n43\t/\tMacintosh HD\n44\t/Volumes/A\tb/\tA\tb\nnot a window\n";
        let windows = super::finder_windows_in(out);
        let w = |id, folder: Option<&str>, name: &str| super::FinderWindow { id, folder: folder.map(PathBuf::from), name: name.into() };
        assert_eq!(windows[0], w(41, Some("/Users/me/Documents"), "Documents"));
        assert_eq!(windows[1], w(42, None, "Recents"), "not a folder");
        assert_eq!(windows[2], w(43, Some("/"), "Macintosh HD"), "the disk's top kept as /");
        assert_eq!(windows.len(), 4, "a line that isn't one left out");
    }
}
