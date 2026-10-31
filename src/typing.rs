//! Typing Icelandic and Korean on ABC, after Vikey's Vietnamese
//! (vikey.app): stay on ABC, and the keys Icelandic has letters on ([ for
//! ð, ; for æ, / for þ, - for ö, ' then a vowel for á) type them when that
//! makes a word, as it ends: "/a[" is "það", "don't" stays "don't"; and
//! keys typed as Korean's 2-Set has them make Hangul: "dkssud" is "안녕".
//! Every other key is ABC's. The engine (`engine`, with `hangul` and
//! `words`) decides; keys are heard and typed by an event tap of its own
//! (src/macos/typing.m), which takes Accessibility. The shortcut (⌥Z) goes
//! to the next language by hand; in Terminal and code editors that's the
//! only way it leaves English. Each app keeps its language. Nothing's done
//! while the Mac's input source isn't a layout for English (the Icelandic
//! layout itself, Korean): that's the user's choice then.

mod engine;
mod hangul;
mod words;

use std::ffi::{CStr, c_char};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use gpui::App;

use engine::{Engine, Layout, Out};

use crate::macos;
use crate::settings::Settings;

pub const COLOR: u32 = 0x30b0c7;
pub const DEFAULT_SHORTCUT: &str = "alt-z";
const ICELANDIC: &str = "com.apple.keylayout.Icelandic";

static ENGINE: Mutex<Option<Engine>> = Mutex::new(None);
/// The shortcut that switches by hand: its key code and modifiers' flags.
static SHORTCUT: Mutex<Option<(u16, u64)>> = Mutex::new(None);
static SUSPENDED: AtomicBool = AtomicBool::new(false);
static LISTENING: AtomicBool = AtomicBool::new(false);
/// The app in front, as last heard; and whether the notch's terminal is
/// down (F2's own window, but code's typed in it).
static FRONT: Mutex<String> = Mutex::new(String::new());
static TERMINAL: AtomicBool = AtomicBool::new(false);

/// Events' modifier flags (CGEventFlags).
const CAPS_FLAG: u64 = 0x10000;
const SHIFT_FLAG: u64 = 0x20000;
const CONTROL_FLAG: u64 = 0x40000;
const OPTION_FLAG: u64 = 0x80000;
const COMMAND_FLAG: u64 = 0x100000;

/// Apps code's typed in: "a[" is code there more often than "að", so
/// they're Icelandic only by hand.
const CODE_APPS: [&str; 18] = [
    "com.apple.Terminal",
    "com.googlecode.iterm2",
    "com.mitchellh.ghostty",
    "dev.warp.Warp-Stable",
    "net.kovidgoyal.kitty",
    "org.alacritty",
    "com.github.wez.wezterm",
    "com.microsoft.VSCode",
    "com.microsoft.VSCodeInsiders",
    "com.todesktop.230313mzl4w4u92",
    "com.exafunction.windsurf",
    "dev.zed.Zed",
    "dev.zed.Zed-Preview",
    "com.apple.dt.Xcode",
    "com.sublimetext.4",
    "com.sublimetext.3",
    "com.panic.Nova",
    "com.barebones.bbedit",
];

fn is_code_app(id: &str) -> bool {
    CODE_APPS.contains(&id) || id.starts_with("com.jetbrains.")
}

/// A key down (`kind` 1), a click (0) or a key F2 sent itself (2), on the
/// tap's thread: true keeps it from the app, typing what's in `out`.
extern "C" fn heard(kind: i32, code: u16, flags: u64, out: *mut macos::TypingOut) -> bool {
    let mut engine = ENGINE.lock().unwrap();
    let Some(engine) = engine.as_mut() else { return false };
    if kind != 1 || SUSPENDED.load(Ordering::Relaxed) {
        engine.forget();
        return false;
    }
    let now = Instant::now();
    let held = flags & (CONTROL_FLAG | OPTION_FLAG | SHIFT_FLAG | COMMAND_FLAG);
    let result = if *SHORTCUT.lock().unwrap() == Some((code, held)) {
        engine.switch(now)
    } else {
        let mut mods = 0;
        for (flag, m) in [(SHIFT_FLAG, engine::SHIFT), (CAPS_FLAG, engine::CAPS), (OPTION_FLAG, engine::OPTION)] {
            if flags & flag != 0 {
                mods |= m;
            }
        }
        engine.key(code, mods, flags & (COMMAND_FLAG | CONTROL_FLAG) != 0, now)
    };
    let Out::Type { back, text, key } = result else { return false };
    let out = unsafe { &mut *out };
    let text: Vec<u16> = text.encode_utf16().collect();
    if text.len() > out.text.len() {
        engine.forget();
        return false;
    }
    out.back = back as u32;
    out.text[..text.len()].copy_from_slice(&text);
    out.length = text.len() as u32;
    out.key = key;
    // (As Vikey has it: a narrow space, but Sublime Text draws that, not a
    // zero-width non-joiner.)
    out.guard = match back {
        0 => 0,
        _ if engine.app().starts_with("com.sublimetext") => 0x200c,
        _ => 0x202f,
    };
    true
}

/// The input source changed (`kind` 0), or another app's in front (1,
/// `text` its bundle id). On the main thread.
extern "C" fn changed(kind: i32, text: *const c_char) {
    if kind == 0 {
        let english = macos::typing_layout(None).and_then(|json| Layout::parse(&json));
        if let Some(engine) = ENGINE.lock().unwrap().as_mut() {
            engine.set_english(english);
        }
        return;
    }
    if text.is_null() {
        return;
    }
    let app = unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned();
    FRONT.lock().unwrap().clone_from(&app);
    if let Some(engine) = ENGINE.lock().unwrap().as_mut() {
        place(engine, &app);
    }
}

/// The engine told which app it's typing in: the notch's terminal, while
/// it's down and F2's in front, is one of its own, as Terminal is.
fn place(engine: &mut Engine, app: &str) {
    let own = app == crate::takeover::BUNDLE_ID || std::env::current_exe().ok().and_then(|e| e.file_name().map(|n| n.to_string_lossy() == app)).unwrap_or(false);
    if own && TERMINAL.load(Ordering::SeqCst) {
        engine.set_app("F2 Terminal", false);
    } else {
        engine.set_app(app, !is_code_app(app));
    }
}

/// The notch's terminal came down (`up`), or went back up.
pub fn terminal_up(up: bool) {
    TERMINAL.store(up, Ordering::SeqCst);
    let front = FRONT.lock().unwrap().clone();
    if let Some(engine) = ENGINE.lock().unwrap().as_mut() {
        place(engine, &front);
    }
}

/// "alt-z" as its key code and the modifiers' flags events have with it.
fn shortcut_flags(shortcut: &str) -> Option<(u16, u64)> {
    let (code, carbon) = crate::hotkeys::parse(shortcut)?;
    let mut flags = 0;
    for (bit, flag) in [(0x100, COMMAND_FLAG), (0x200, SHIFT_FLAG), (0x800, OPTION_FLAG), (0x1000, CONTROL_FLAG)] {
        if carbon & bit != 0 {
            flags |= flag;
        }
    }
    Some((code as u16, flags))
}

/// Keys left alone (while Settings records a shortcut: the switch's would
/// be taken), or not.
pub fn suspend(suspended: bool) {
    SUSPENDED.store(suspended, Ordering::Relaxed);
}

/// Hear keys while it's on (again when Settings change, and once
/// Accessibility's allowed).
pub fn init(cx: &mut App) {
    let apply = move |cx: &mut App| {
        let settings = Settings::get(cx);
        let s = settings.typing.clone();
        // (In beta: with Settings' beta features on.)
        let wanted = settings.beta && s.on && (s.icelandic || s.korean);
        *SHORTCUT.lock().unwrap() = s.shortcut.as_deref().and_then(shortcut_flags);
        if let Some(engine) = ENGINE.lock().unwrap().as_mut() {
            engine.set_langs(s.icelandic, s.korean);
        }
        let on = LISTENING.load(Ordering::SeqCst);
        if wanted && !on && macos::accessibility(false) {
            let english = macos::typing_layout(None).and_then(|json| Layout::parse(&json));
            let made = ENGINE.lock().unwrap().is_some();
            if made {
                if let Some(engine) = ENGINE.lock().unwrap().as_mut() {
                    engine.set_english(english);
                }
            } else {
                let Some(icelandic) = macos::typing_layout(Some(ICELANDIC)).and_then(|json| Layout::parse(&json)) else { return };
                let mut engine = Engine::new(english, icelandic, words::Words::bundled());
                engine.set_langs(s.icelandic, s.korean);
                *ENGINE.lock().unwrap() = Some(engine);
            }
            LISTENING.store(macos::typing_listen(Some(heard), Some(changed)), Ordering::SeqCst);
        } else if !wanted && on {
            macos::typing_listen(None, None);
            LISTENING.store(false, Ordering::SeqCst);
            if let Some(engine) = ENGINE.lock().unwrap().as_mut() {
                engine.forget();
            }
        }
    };
    apply(cx);
    let apply = std::rc::Rc::new(std::cell::RefCell::new(apply));
    let again = apply.clone();
    cx.observe_global::<Settings>(move |cx| (again.borrow_mut())(cx)).detach();
    // (Accessibility may be allowed later: looked at now and then.)
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(Duration::from_secs(5)).await;
            cx.update(|cx| (apply.borrow_mut())(cx));
        }
    })
    .detach();
}
