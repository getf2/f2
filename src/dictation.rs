//! Dictation, after Hex's (https://github.com/anomalyco/hex): hold a key,
//! or two, in any app, speak, let go, and what was said is pasted where
//! you're typing. Pressed twice, it keeps listening hands-free until it's
//! pressed again; or (Settings' Press) pressed once to start and again to
//! finish. Escape stops it, pasting nothing. The words come from
//! Apple's speech recognition, on this Mac (macOS 26: see `Engine`); the
//! microphone, the keys and the pill that shows it listening are
//! src/macos/dictation.m's.
//!
//! The shortcut is one key held alone (Right ⌥, Fn, a function key) or two
//! together (⌃⌥, ⌥Space): as `Shortcut` names them ("right-option",
//! "control+option", "option+space"). Hearing it takes Accessibility, as
//! pasting does.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use futures::StreamExt;
use futures::channel::mpsc::{UnboundedSender, unbounded};
use gpui::App;

use crate::macos;
use crate::settings::{DictationMode, Settings};

/// What turns speech into text: Apple's (macOS 26's own) now. Another (a
/// model F2 downloads, as Hex's Parakeet) would be one more of these: it's
/// given the same audio, 16 kHz mono (`SAMPLE_RATE`).
pub trait Engine: Sync {
    /// Whether it works on this Mac.
    fn available(&self) -> bool;
    /// The languages it takes: their codes (BCP 47), names, and whether
    /// each is ready on this Mac. Waits.
    fn languages(&self) -> Vec<Language>;
    /// The language it'd use for `wanted` (None: this Mac's), if it has
    /// one like it. Waits.
    fn resolve(&self, wanted: Option<&str>) -> Option<String>;
    /// Ready for `language`: allowed, its model downloaded (the first time:
    /// a while). Waits; quick once it's ready.
    fn prepare(&self, language: &str) -> Result<(), String>;
    /// How far preparing's got (0 to 1), while it downloads.
    fn progress(&self) -> Option<f64>;
    /// Audio's coming: make transcribing it quicker. Returns at once.
    fn warm(&self, _language: &str) {}
    /// What's said in `audio`. Waits.
    fn transcribe(&self, audio: &[f32], language: &str) -> Result<String, String>;
}

pub const SAMPLE_RATE: usize = 16_000;

/// Its tile's color.
pub const COLOR: u32 = 0xff375f;

#[derive(Clone, PartialEq, Debug)]
pub struct Language {
    pub code: String,
    pub name: String,
    pub ready: bool,
}

/// Apple's: SpeechAnalyzer (src/macos/speech.swift).
struct AppleSpeech;

impl Engine for AppleSpeech {
    fn available(&self) -> bool {
        macos::speech_available()
    }

    fn languages(&self) -> Vec<Language> {
        macos::speech_locales()
            .into_iter()
            .map(|(code, name, ready)| Language { code, name, ready })
            .collect()
    }

    fn resolve(&self, wanted: Option<&str>) -> Option<String> {
        macos::speech_resolve(wanted.unwrap_or(""))
    }

    fn prepare(&self, language: &str) -> Result<(), String> {
        macos::speech_prepare(language)
    }

    fn progress(&self) -> Option<f64> {
        macos::speech_progress()
    }

    fn warm(&self, language: &str) {
        macos::speech_warm(language);
    }

    fn transcribe(&self, audio: &[f32], language: &str) -> Result<String, String> {
        macos::speech_transcribe(language, audio)
    }
}

pub fn engine() -> &'static dyn Engine {
    &AppleSpeech
}

/// Whether this Mac can dictate (macOS 26).
pub fn available() -> bool {
    static AVAILABLE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *AVAILABLE.get_or_init(|| engine().available())
}

// ---- The shortcut.

#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum Modifier {
    Control,
    Option,
    Shift,
    Command,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum Side {
    Either,
    Left,
    Right,
}

/// A key in a shortcut. (In this order in one: Fn, ⌃, ⌥, ⇧, ⌘, the key.)
#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum Key {
    Fn,
    Modifier(Modifier, Side),
    /// Any other, by its virtual key code.
    Code(u16),
}

impl Modifier {
    const ALL: [Modifier; 4] = [Modifier::Control, Modifier::Option, Modifier::Shift, Modifier::Command];

    fn name(self) -> &'static str {
        match self {
            Modifier::Control => "control",
            Modifier::Option => "option",
            Modifier::Shift => "shift",
            Modifier::Command => "command",
        }
    }

    fn sign(self) -> &'static str {
        match self {
            Modifier::Control => "⌃",
            Modifier::Option => "⌥",
            Modifier::Shift => "⇧",
            Modifier::Command => "⌘",
        }
    }

    /// Its left and right keys' codes, and the bits events' flags have
    /// for each (NX_DEVICEL…KEYMASK), and for either.
    fn keys(self) -> (u16, u16, u64, u64, u64) {
        match self {
            Modifier::Control => (59, 62, 0x1, 0x2000, 0x40000),
            Modifier::Option => (58, 61, 0x20, 0x40, 0x80000),
            Modifier::Shift => (56, 60, 0x2, 0x4, 0x20000),
            Modifier::Command => (55, 54, 0x8, 0x10, 0x100000),
        }
    }

    /// Its bit in `Held::mods`, for a side.
    fn bit(self, right: bool) -> u8 {
        1 << (self as u8 * 2 + right as u8)
    }
}

const FN_CODE: u16 = 63;
const FN_FLAG: u64 = 0x800000;
const ESCAPE: u16 = 53;

/// The keys besides modifiers a shortcut can have, by name (as GPUI names
/// them, F13 to F20 too) and code.
fn key_code(name: &str) -> Option<u16> {
    const MORE: [(&str, u16); 8] = [("f13", 105), ("f14", 107), ("f15", 113), ("f16", 106), ("f17", 64), ("f18", 79), ("f19", 80), ("f20", 90)];
    MORE.iter().find(|(n, _)| *n == name).map(|(_, c)| *c).or_else(|| crate::hotkeys::key_code(name).map(|c| c as u16))
}

fn key_name(code: u16) -> Option<String> {
    let names = ('a'..='z').chain('0'..='9').chain("-=[];',./\\`".chars()).map(String::from);
    let more = ["space", "tab", "enter", "backspace", "delete", "escape", "left", "right", "up", "down", "home", "end", "pageup", "pagedown"].map(String::from);
    names.chain(more).chain((1..=20).map(|n| format!("f{n}"))).find(|name| key_code(name) == Some(code))
}

fn is_function(code: u16) -> bool {
    key_name(code).is_some_and(|n| n.len() > 1 && n.starts_with('f') && n[1..].parse::<u8>().is_ok())
}

/// The key, or two keys, held to dictate.
#[derive(Clone, PartialEq, Debug)]
pub struct Shortcut(Vec<Key>);

pub const DEFAULT_SHORTCUT: &str = "right-option";

impl Shortcut {
    /// "right-option", "control+option", "option+space", "fn", "f5".
    pub fn parse(s: &str) -> Option<Shortcut> {
        let mut keys = Vec::new();
        for part in s.split('+').map(str::trim) {
            let (side, rest) = match part.split_once('-') {
                Some(("left", rest)) => (Side::Left, rest),
                Some(("right", rest)) => (Side::Right, rest),
                _ => (Side::Either, part),
            };
            let key = if let Some(m) = Modifier::ALL.into_iter().find(|m| m.name() == rest) {
                Key::Modifier(m, side)
            } else if side != Side::Either {
                return None;
            } else if part == "fn" {
                Key::Fn
            } else {
                Key::Code(key_code(part)?)
            };
            keys.push(key);
        }
        keys.sort();
        keys.dedup();
        (!keys.is_empty() && keys.len() <= 2).then_some(Shortcut(keys))
    }

    /// As Settings keep it.
    pub fn name(&self) -> String {
        let name = |k: &Key| match *k {
            Key::Fn => "fn".to_string(),
            Key::Modifier(m, Side::Either) => m.name().to_string(),
            Key::Modifier(m, Side::Left) => format!("left-{}", m.name()),
            Key::Modifier(m, Side::Right) => format!("right-{}", m.name()),
            Key::Code(c) => key_name(c).unwrap_or_default(),
        };
        self.0.iter().map(name).collect::<Vec<_>>().join("+")
    }

    /// As it's shown: "Right ⌥", "⌃⌥", "⌥Space", "Fn".
    pub fn label(&self) -> String {
        let words = self.0.iter().any(|k| matches!(k, Key::Fn | Key::Modifier(_, Side::Left | Side::Right)));
        let label = |k: &Key| match *k {
            Key::Fn => "Fn".to_string(),
            Key::Modifier(m, Side::Either) => m.sign().to_string(),
            Key::Modifier(m, Side::Left) => format!("Left {}", m.sign()),
            Key::Modifier(m, Side::Right) => format!("Right {}", m.sign()),
            Key::Code(c) => key_name(c).map(|n| crate::hotkeys::label(&n)).unwrap_or_else(|| format!("Key {c}")),
        };
        self.0.iter().map(label).collect::<Vec<_>>().join(if words { " " } else { "" })
    }

    fn has_code(&self, code: u16) -> bool {
        self.0.contains(&Key::Code(code))
    }

    /// Each of its keys is held.
    fn all_held(&self, held: &Held) -> bool {
        self.0.iter().all(|k| match *k {
            Key::Fn => held.fn_key,
            Key::Modifier(m, Side::Left) => held.mods & m.bit(false) != 0,
            Key::Modifier(m, Side::Right) => held.mods & m.bit(true) != 0,
            Key::Modifier(m, Side::Either) => held.mods & (m.bit(false) | m.bit(true)) != 0,
            Key::Code(c) => held.keys.contains(&c),
        })
    }

    /// Its keys are held, and nothing else.
    fn matches(&self, held: &Held) -> bool {
        if !self.all_held(held) {
            return false;
        }
        let mut mods = held.mods;
        for k in &self.0 {
            if let Key::Modifier(m, side) = *k {
                match side {
                    Side::Left => mods &= !m.bit(false),
                    Side::Right => mods &= !m.bit(true),
                    Side::Either => mods &= !(m.bit(false) | m.bit(true)),
                }
            }
        }
        mods == 0 && (!held.fn_key || self.0.contains(&Key::Fn)) && held.keys.iter().all(|&c| self.has_code(c))
    }
}

/// A key event, as the tap hears it.
#[derive(Clone, Copy, Debug)]
pub struct Event {
    /// 0: a modifier changed, 1: down, 2: up.
    pub kind: i32,
    pub code: u16,
    pub flags: u64,
    pub repeat: bool,
}

impl Event {
    fn down(&self) -> bool {
        self.kind == 1
    }

    fn up(&self) -> bool {
        self.kind == 2
    }
}

/// What's held down now.
#[derive(Clone, Default, PartialEq, Debug)]
struct Held {
    /// Modifiers, by side (see `Modifier::bit`).
    mods: u8,
    fn_key: bool,
    /// Other keys, by code.
    keys: Vec<u16>,
}

impl Held {
    /// After `e`. (`down` says if a key's still down: one whose letting go
    /// wasn't heard, as in a password field, is forgotten.)
    fn update(&mut self, e: Event, down: impl Fn(u16) -> bool) {
        // (The flags have every modifier's state, by side; an event without
        // the side's bits, as some keyboards send: the left.)
        self.mods = 0;
        for m in Modifier::ALL {
            let (_, _, left, right, either) = m.keys();
            let (l, r) = (e.flags & left != 0, e.flags & right != 0);
            if l || (e.flags & either != 0 && !r) {
                self.mods |= m.bit(false);
            }
            if r {
                self.mods |= m.bit(true);
            }
        }
        match e.kind {
            // (Fn's flag is on with arrows and function keys too: only its
            // own key says.)
            0 if e.code == FN_CODE => self.fn_key = e.flags & FN_FLAG != 0,
            1 if is_modifier(e.code) => {}
            1 => {
                self.keys.retain(|&k| k == e.code || down(k));
                if !self.keys.contains(&e.code) {
                    self.keys.push(e.code);
                }
            }
            2 => self.keys.retain(|&k| k != e.code),
            _ => {}
        }
    }

    fn is_empty(&self) -> bool {
        self.mods == 0 && !self.fn_key && self.keys.is_empty()
    }

    /// Each key held, by side.
    fn each(&self) -> Vec<Key> {
        let mut out = Vec::new();
        if self.fn_key {
            out.push(Key::Fn);
        }
        for m in Modifier::ALL {
            for (right, side) in [(false, Side::Left), (true, Side::Right)] {
                if self.mods & m.bit(right) != 0 {
                    out.push(Key::Modifier(m, side));
                }
            }
        }
        out.extend(self.keys.iter().map(|&c| Key::Code(c)));
        out
    }
}

fn is_modifier(code: u16) -> bool {
    (54..=63).contains(&code)
}

// ---- Recording a shortcut, in Settings.

#[derive(Clone, PartialEq, Debug)]
pub enum Recorded {
    Set(Shortcut),
    Cancel,
    /// Not one it can be (why): still recording.
    Invalid(&'static str),
}

/// The keys pressed for a shortcut: once they're all let go, what they
/// were.
#[derive(Default)]
struct Recorder {
    held: Held,
    seen: Vec<Key>,
}

impl Recorder {
    fn key(&mut self, e: Event, down: impl Fn(u16) -> bool) -> Option<Recorded> {
        if e.repeat {
            return None;
        }
        self.held.update(e, down);
        for k in self.held.each() {
            if !self.seen.contains(&k) {
                self.seen.push(k);
            }
        }
        if !self.held.is_empty() || self.seen.is_empty() {
            return None;
        }
        let seen = std::mem::take(&mut self.seen);
        Some(shortcut_of(seen))
    }
}

/// What these keys, pressed together, make.
fn shortcut_of(mut keys: Vec<Key>) -> Recorded {
    keys.sort();
    if keys == [Key::Code(ESCAPE)] {
        return Recorded::Cancel;
    }
    if keys.len() > 2 {
        return Recorded::Invalid("Two keys at most.");
    }
    let codes: Vec<u16> = keys.iter().filter_map(|k| if let Key::Code(c) = *k { Some(c) } else { None }).collect();
    if codes.iter().any(|&c| key_name(c).is_none()) {
        return Recorded::Invalid("That key can't be used.");
    }
    match (keys.len(), codes.len()) {
        (1, 1) if !is_function(codes[0]) => return Recorded::Invalid("Alone, that key would type: hold a modifier with it (⌃, ⌥, ⇧, ⌘ or Fn)."),
        (2, 2) => return Recorded::Invalid("Hold a modifier with it (⌃, ⌥, ⇧, ⌘ or Fn)."),
        _ => {}
    }
    // (Two together: either side, unless they're one modifier's two.)
    if keys.len() == 2 {
        let same = matches!((keys[0], keys[1]), (Key::Modifier(a, _), Key::Modifier(b, _)) if a == b);
        if !same {
            for k in &mut keys {
                if let Key::Modifier(m, _) = *k {
                    *k = Key::Modifier(m, Side::Either);
                }
            }
        }
    }
    keys.sort();
    Recorded::Set(Shortcut(keys))
}

// ---- Hold to talk, press twice to keep listening.

/// What the keys say to do.
#[derive(Clone, PartialEq, Debug)]
pub enum Do {
    /// Start listening.
    Start,
    /// Keep listening once it's let go (pressed twice).
    Lock,
    /// Stop listening, and paste what was said.
    Finish,
    /// Stop listening, pasting nothing.
    Cancel,
    /// A shortcut recorded in Settings (or not).
    Recorded(Recorded),
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Phase {
    Idle,
    /// Held down since.
    Held(Instant),
    /// Pressed and let go quickly, at: pressed again soon, it keeps
    /// listening (the microphone's on meanwhile).
    Tapped(Instant),
    /// Listening hands-free.
    Locked,
}

/// Shorter than this, a press is a tap.
const TAP: Duration = Duration::from_millis(300);
/// A second press this soon after a tap keeps it listening.
const AGAIN: Duration = Duration::from_millis(400);
/// It stops by itself after this long.
const LONGEST: Duration = Duration::from_secs(10 * 60);

struct Machine {
    shortcut: Option<Shortcut>,
    mode: DictationMode,
    phase: Phase,
    started: Instant,
    held: Held,
    /// The shortcut was held, as of the last event.
    matched: bool,
    /// Stopped while it's still held (another key with it, escape): it
    /// starts again only once it's let go.
    spent: bool,
}

impl Machine {
    fn new(shortcut: Option<Shortcut>, mode: DictationMode) -> Machine {
        Machine { shortcut, mode, phase: Phase::Idle, started: Instant::now(), held: Held::default(), matched: false, spent: false }
    }

    fn listening(&self) -> bool {
        self.phase != Phase::Idle
    }

    /// A key event: what to do, and whether to keep it from the app.
    fn key(&mut self, e: Event, now: Instant, down: impl Fn(u16) -> bool) -> (Option<Do>, bool) {
        let Some(shortcut) = self.shortcut.clone() else { return (None, false) };
        let before = self.held.clone();
        self.held.update(e, down);
        let was = self.matched;
        let is = shortcut.matches(&self.held);
        self.matched = is;
        if self.spent && !shortcut.all_held(&self.held) {
            self.spent = false;
        }
        let part = (e.down() || e.up()) && shortcut.has_code(e.code);
        if e.down() && e.code == ESCAPE && self.listening() && !shortcut.has_code(ESCAPE) {
            self.phase = Phase::Idle;
            self.spent = shortcut.all_held(&self.held);
            return (Some(Do::Cancel), true);
        }
        let mut what = None;
        if !was && is && !self.spent {
            what = match self.phase {
                Phase::Idle => {
                    self.phase = Phase::Held(now);
                    self.started = now;
                    Some(Do::Start)
                }
                Phase::Tapped(at) if now.duration_since(at) <= AGAIN => {
                    self.phase = Phase::Locked;
                    Some(Do::Lock)
                }
                Phase::Tapped(_) => {
                    self.phase = Phase::Held(now);
                    None
                }
                Phase::Locked => {
                    self.phase = Phase::Idle;
                    Some(Do::Finish)
                }
                Phase::Held(_) => None,
            };
        } else if was && !is {
            if let Phase::Held(since) = self.phase {
                what = if shortcut.all_held(&self.held) {
                    // (Another key with it: a shortcut of the app's, ⌥E.)
                    self.phase = Phase::Idle;
                    self.spent = true;
                    Some(Do::Cancel)
                } else if self.mode == DictationMode::Press {
                    // (Pressed: listening till it's pressed again.)
                    self.phase = Phase::Locked;
                    Some(Do::Lock)
                } else if now.duration_since(since) < TAP {
                    self.phase = Phase::Tapped(now);
                    None
                } else {
                    self.phase = Phase::Idle;
                    Some(Do::Finish)
                };
            }
        } else if matches!(self.phase, Phase::Tapped(_)) && pressed_more(&before, &self.held, e) {
            // (Something else pressed after a tap: that wasn't for this.)
            self.phase = Phase::Idle;
            what = Some(Do::Cancel);
        }
        (what, part && (was || is))
    }

    /// Time going by: a tap not followed by another, or listening too long.
    fn tick(&mut self, now: Instant) -> Option<Do> {
        match self.phase {
            Phase::Tapped(at) if now.duration_since(at) > AGAIN => {
                self.phase = Phase::Idle;
                Some(Do::Cancel)
            }
            Phase::Held(_) | Phase::Locked if now.duration_since(self.started) > LONGEST => {
                self.phase = Phase::Idle;
                Some(Do::Finish)
            }
            _ => None,
        }
    }
}

/// Whether `e` pressed a key that wasn't held before.
fn pressed_more(before: &Held, after: &Held, e: Event) -> bool {
    (e.down() && !e.repeat) || after.mods & !before.mods != 0 || (after.fn_key && !before.fn_key)
}

// ---- Listening, and what's said.

static MACHINE: Mutex<Option<Machine>> = Mutex::new(None);
static RECORDER: Mutex<Option<Recorder>> = Mutex::new(None);
static TODO: std::sync::OnceLock<UnboundedSender<Do>> = std::sync::OnceLock::new();
/// The tap's on.
static LISTENING: AtomicBool = AtomicBool::new(false);
/// Transcriptions not done yet.
static WORKING: AtomicUsize = AtomicUsize::new(0);
static LAST: Mutex<Option<String>> = Mutex::new(None);
/// What's heard, to transcribe (in order), and the language Settings say.
static HEARD: std::sync::OnceLock<std::sync::mpsc::Sender<Vec<f32>>> = std::sync::OnceLock::new();
static LANGUAGE: Mutex<Option<String>> = Mutex::new(None);
static STATUS: Mutex<Status> = Mutex::new(Status { language: None, here: None, preparing: None, error: None, languages: Vec::new(), hint: None });

/// How it's doing, for Settings.
#[derive(Clone, Default)]
pub struct Status {
    /// The language it uses (its code), once found.
    pub language: Option<String>,
    /// This Mac's language, as it takes it.
    pub here: Option<String>,
    /// Getting ready: how far its model's download has got (0 to 1), if
    /// it's downloading.
    pub preparing: Option<Option<f64>>,
    /// Why it couldn't get ready.
    pub error: Option<String>,
    /// The languages it takes.
    pub languages: Vec<Language>,
    /// Why the shortcut just pressed in Settings can't be one.
    pub hint: Option<&'static str>,
}

pub fn status() -> Status {
    STATUS.lock().unwrap().clone()
}

/// What was last dictated.
pub fn last() -> Option<String> {
    LAST.lock().unwrap().clone()
}

extern "C" fn on_key(kind: i32, code: u16, flags: u64, repeat: bool) -> bool {
    let e = Event { kind, code, flags, repeat };
    if let Some(recorder) = RECORDER.lock().unwrap().as_mut() {
        if let Some(done) = recorder.key(e, macos::key_is_down) {
            send(Do::Recorded(done));
        }
        // (Keys pressed for the shortcut go nowhere else.)
        return kind != 0;
    }
    let (what, keep) = match MACHINE.lock().unwrap().as_mut() {
        Some(machine) => machine.key(e, Instant::now(), macos::key_is_down),
        None => (None, false),
    };
    if let Some(what) = what {
        send(what);
    }
    keep
}

/// Record the shortcut in Settings: the next keys pressed (on), or not.
pub fn record(on: bool, cx: &mut App) {
    let was = RECORDER.lock().unwrap().is_some();
    if was == on {
        return;
    }
    *RECORDER.lock().unwrap() = on.then(Recorder::default);
    STATUS.lock().unwrap().hint = None;
    crate::hotkeys::suspend(on);
    listen(cx);
    cx.refresh_windows();
}

pub fn recording() -> bool {
    RECORDER.lock().unwrap().is_some()
}

/// The tap on while there's a shortcut to hear (or one's being recorded),
/// once Accessibility's allowed.
fn listen(cx: &mut App) {
    let s = &Settings::get(cx).dictation;
    let wanted = (s.on && Shortcut::parse(&s.shortcut).is_some()) || recording();
    let on = LISTENING.load(Ordering::SeqCst);
    if wanted && !on && macos::accessibility(false) {
        LISTENING.store(macos::dictation_listen(Some(on_key)), Ordering::SeqCst);
    } else if !wanted && on {
        macos::dictation_listen(None);
        LISTENING.store(false, Ordering::SeqCst);
    }
}

fn send(what: Do) {
    if let Some(tx) = TODO.get() {
        let _ = tx.unbounded_send(what);
    }
}

fn hud_after() -> macos::Hud {
    if MACHINE.lock().unwrap().as_ref().is_some_and(Machine::listening) {
        macos::Hud::Listening
    } else if WORKING.load(Ordering::SeqCst) > 0 {
        macos::Hud::Working
    } else {
        macos::Hud::Hidden
    }
}

/// Back to idle (after it couldn't start).
fn reset() {
    if let Some(machine) = MACHINE.lock().unwrap().as_mut() {
        machine.phase = Phase::Idle;
    }
}

/// What the microphone heard, once it's stopped: to be transcribed.
extern "C" fn heard(samples: *const f32, count: usize) {
    let audio = if samples.is_null() { Vec::new() } else { unsafe { std::slice::from_raw_parts(samples, count) }.to_vec() };
    if let Some(tx) = HEARD.get() {
        let _ = tx.send(audio);
    }
}

fn act(what: Do, cx: &mut App) {
    match what {
        Do::Start => {
            match macos::mic_access() {
                Some(true) => {}
                None => {
                    reset();
                    macos::mic_request();
                    return;
                }
                Some(false) => {
                    reset();
                    crate::shots::note(
                        format!("Allow {} to Use the Microphone", crate::APP_NAME),
                        "Dictation hears you through it: turn it on in System Settings > Privacy & Security > Microphone.".into(),
                        Some("x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"),
                        cx,
                    );
                    return;
                }
            }
            macos::mic_start(Settings::get(cx).dictation.microphone.as_deref().unwrap_or(""));
            macos::dictation_hud(macos::Hud::Listening);
            if let Some(language) = STATUS.lock().unwrap().language.clone() {
                engine().warm(&language);
            }
            // (As time goes by: a tap not followed by another, or listening
            // too long. And whether the microphone started.)
            cx.spawn(async move |cx| {
                let mut checked = false;
                loop {
                    cx.background_executor().timer(Duration::from_millis(100)).await;
                    if !checked {
                        checked = true;
                        if let Some(why) = macos::mic_error() {
                            eprintln!("dictation: the microphone didn't start: {why}");
                            reset();
                            macos::mic_stop(None);
                            cx.update(|cx| {
                                macos::dictation_hud(hud_after());
                                crate::shots::note("Couldn't Start Dictation".into(), why, None, cx);
                            });
                            return;
                        }
                    }
                    let mut machine = MACHINE.lock().unwrap();
                    let Some(m) = machine.as_mut() else { return };
                    if let Some(what) = m.tick(Instant::now()) {
                        drop(machine);
                        send(what);
                        return;
                    }
                    if !m.listening() {
                        return;
                    }
                }
            })
            .detach();
        }
        Do::Lock => macos::dictation_hud(macos::Hud::HandsFree),
        Do::Finish => {
            WORKING.fetch_add(1, Ordering::SeqCst);
            macos::dictation_hud(macos::Hud::Working);
            macos::mic_stop(Some(heard));
        }
        Do::Cancel => {
            macos::mic_stop(None);
            macos::dictation_hud(hud_after());
        }
        Do::Recorded(Recorded::Set(shortcut)) => {
            record(false, cx);
            Settings::update(cx, |s| {
                s.dictation.shortcut = shortcut.name();
                s.dictation.on = true;
            });
        }
        Do::Recorded(Recorded::Cancel) => record(false, cx),
        Do::Recorded(Recorded::Invalid(why)) => {
            STATUS.lock().unwrap().hint = Some(why);
            cx.refresh_windows();
        }
    }
}

/// Transcribed: pasted where you're typing (with the clipboard put back).
fn done(said: Result<String, String>, cx: &mut App) {
    WORKING.fetch_sub(1, Ordering::SeqCst);
    macos::dictation_hud(hud_after());
    match said {
        Ok(text) if !text.trim().is_empty() => {
            *LAST.lock().unwrap() = Some(text.clone());
            std::thread::spawn(move || crate::snippets::paste_in(text, 0));
        }
        Ok(_) => {}
        Err(why) => {
            eprintln!("dictation: {why}");
            crate::shots::note("Couldn't Dictate".into(), why, None, cx)
        }
    }
}

/// The language to use, and it ready (its model downloaded), in the
/// background; Settings shows how it goes.
fn get_ready(wanted: Option<String>, cx: &mut App) {
    static GETTING: AtomicBool = AtomicBool::new(false);
    if GETTING.swap(true, Ordering::SeqCst) {
        return;
    }
    {
        let mut status = STATUS.lock().unwrap();
        status.preparing = Some(None);
        status.error = None;
    }
    cx.refresh_windows();
    cx.spawn(async move |cx| {
        let work = cx.background_executor().spawn(async move {
            let language = engine().resolve(wanted.as_deref()).or_else(|| engine().resolve(Some("en-US")));
            STATUS.lock().unwrap().language = language.clone();
            let Some(language) = language else { return Err("Apple's speech recognition doesn't take any language here.".to_string()) };
            engine().prepare(&language)
        });
        let mut work = std::pin::pin!(work);
        let outcome = loop {
            let tick = cx.background_executor().timer(Duration::from_millis(500));
            match futures::future::select(work.as_mut(), tick).await {
                futures::future::Either::Left((outcome, _)) => break outcome,
                futures::future::Either::Right(_) => {
                    STATUS.lock().unwrap().preparing = Some(engine().progress());
                    cx.update(|cx| cx.refresh_windows());
                }
            }
        };
        let languages = cx.background_executor().spawn(async { engine().languages() }).await;
        if let Err(why) = &outcome {
            eprintln!("dictation: not ready: {why}");
        }
        {
            let mut status = STATUS.lock().unwrap();
            status.preparing = None;
            status.error = outcome.err();
            status.languages = languages;
        }
        GETTING.store(false, Ordering::SeqCst);
        cx.update(|cx| cx.refresh_windows());
    })
    .detach();
}

pub fn init(cx: &mut App) {
    if !available() {
        return;
    }
    *MACHINE.lock().unwrap() = Some(Machine::new(None, DictationMode::Hold));
    let (tx, mut rx) = unbounded();
    let _ = TODO.set(tx);
    let (audio_tx, audio) = std::sync::mpsc::channel::<Vec<f32>>();
    let _ = HEARD.set(audio_tx);
    let (said_tx, mut said) = unbounded::<Result<String, String>>();
    // (One at a time, in order.)
    std::thread::spawn(move || {
        for audio in audio {
            // (A quarter of a second: nothing said.)
            let outcome = if audio.len() < SAMPLE_RATE / 4 {
                Ok(String::new())
            } else {
                let wanted = LANGUAGE.lock().unwrap().clone();
                engine()
                    .resolve(wanted.as_deref())
                    .or_else(|| engine().resolve(Some("en-US")))
                    .ok_or_else(|| "Apple's speech recognition doesn't take that language.".to_string())
                    .and_then(|language| engine().prepare(&language).and_then(|_| engine().transcribe(&audio, &language)))
            };
            let _ = said_tx.unbounded_send(outcome);
        }
    });
    cx.spawn(async move |cx| {
        while let Some(what) = rx.next().await {
            cx.update(|cx| act(what, cx));
        }
    })
    .detach();
    cx.spawn(async move |cx| {
        while let Some(outcome) = said.next().await {
            cx.update(|cx| done(outcome, cx));
        }
    })
    .detach();
    let mut seen: Option<crate::settings::Dictation> = None;
    let mut apply = move |cx: &mut App| {
        let s = Settings::get(cx).dictation.clone();
        LANGUAGE.lock().unwrap().clone_from(&s.language);
        let shortcut = s.on.then(|| Shortcut::parse(&s.shortcut)).flatten();
        let cancel = {
            let mut machine = MACHINE.lock().unwrap();
            let m = machine.get_or_insert_with(|| Machine::new(None, s.mode));
            let changed = m.shortcut != shortcut || m.mode != s.mode;
            let was = m.listening();
            if changed {
                *m = Machine::new(shortcut, s.mode);
            }
            changed && was
        };
        if cancel {
            send(Do::Cancel);
        }
        listen(cx);
        let before = seen.replace(s.clone());
        let newly = before.as_ref().is_none_or(|b| !b.on || b.language != s.language);
        if s.on && newly {
            if macos::mic_access().is_none() {
                macos::mic_request();
            }
            get_ready(s.language.clone(), cx);
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
    // (The languages, for Settings, once.)
    cx.spawn(async move |cx| {
        let (languages, here) = cx.background_executor().spawn(async { (engine().languages(), engine().resolve(None)) }).await;
        {
            let mut status = STATUS.lock().unwrap();
            status.languages = languages;
            status.here = here;
        }
        cx.update(|cx| cx.refresh_windows());
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none(_: u16) -> bool {
        false
    }

    fn hold(shortcut: Option<Shortcut>) -> Machine {
        Machine::new(shortcut, DictationMode::Hold)
    }

    #[test]
    fn press_to_start_and_again_to_finish() {
        let mut m = Machine::new(Shortcut::parse("right-command"), DictationMode::Press);
        let t = Instant::now();
        let ms = |n| t + Duration::from_millis(n);
        assert_eq!(m.key(flags(54, RIGHT_COMMAND), t, none).0, Some(Do::Start));
        assert_eq!(m.key(flags(54, 0), ms(100), none).0, Some(Do::Lock));
        // (Not a tap waiting for another: it listens on.)
        assert_eq!(m.tick(ms(2000)), None);
        assert_eq!(m.key(flags(54, RIGHT_COMMAND), ms(8000), none).0, Some(Do::Finish));
        assert_eq!(m.key(flags(54, 0), ms(8100), none).0, None);
        // (Held long, the same.)
        assert_eq!(m.key(flags(54, RIGHT_COMMAND), ms(9000), none).0, Some(Do::Start));
        assert_eq!(m.key(flags(54, 0), ms(12000), none).0, Some(Do::Lock));
        assert_eq!(m.key(down(ESCAPE, 0), ms(13000), none), (Some(Do::Cancel), true));
        assert_eq!(m.key(up(ESCAPE, 0), ms(13100), none).0, None);
        // (⌘C is the app's.)
        assert_eq!(m.key(flags(54, RIGHT_COMMAND), ms(14000), none).0, Some(Do::Start));
        assert_eq!(m.key(down(8, RIGHT_COMMAND), ms(14100), none).0, Some(Do::Cancel));
        assert_eq!(m.key(up(8, RIGHT_COMMAND), ms(14150), none).0, None);
        assert_eq!(m.key(flags(54, 0), ms(14200), none).0, None);
    }

    // (Flags as macOS gives them: Right ⌥, Left ⌃ and ⌥ together, ⌥.)
    const RIGHT_OPTION: u64 = 0x80000 | 0x40;
    const LEFT_OPTION: u64 = 0x80000 | 0x20;
    const CONTROL_OPTION: u64 = 0x40000 | 0x1 | 0x80000 | 0x20;
    const CONTROL: u64 = 0x40000 | 0x1;
    const RIGHT_COMMAND: u64 = 0x100000 | 0x10;

    fn flags(code: u16, flags: u64) -> Event {
        Event { kind: 0, code, flags, repeat: false }
    }

    fn down(code: u16, flags: u64) -> Event {
        Event { kind: 1, code, flags, repeat: false }
    }

    fn up(code: u16, flags: u64) -> Event {
        Event { kind: 2, code, flags, repeat: false }
    }

    #[test]
    fn shortcuts_by_name() {
        let parse = |s: &str| Shortcut::parse(s).map(|s| (s.name(), s.label()));
        assert_eq!(parse("right-option"), Some(("right-option".into(), "Right ⌥".into())));
        assert_eq!(parse("option+control"), Some(("control+option".into(), "⌃⌥".into())));
        assert_eq!(parse("option+space"), Some(("option+space".into(), "⌥Space".into())));
        assert_eq!(parse("fn"), Some(("fn".into(), "Fn".into())));
        assert_eq!(parse("f5"), Some(("f5".into(), "F5".into())));
        assert_eq!(parse("f18"), Some(("f18".into(), "F18".into())));
        assert_eq!(parse("fn+control"), Some(("fn+control".into(), "Fn ⌃".into())));
        assert_eq!(parse("left-command+right-command"), Some(("left-command+right-command".into(), "Left ⌘ Right ⌘".into())));
        assert_eq!(parse("right-space"), None);
        assert_eq!(parse("control+option+shift"), None);
        assert_eq!(parse(""), None);
    }

    #[test]
    fn hold_to_talk() {
        let mut m = hold(Shortcut::parse("right-option"));
        let t = Instant::now();
        assert_eq!(m.key(flags(61, RIGHT_OPTION), t, none), (Some(Do::Start), false));
        assert_eq!(m.key(flags(61, 0), t + Duration::from_secs(2), none), (Some(Do::Finish), false));
        // (The left one isn't it.)
        assert_eq!(m.key(flags(58, LEFT_OPTION), t + Duration::from_secs(3), none), (None, false));
    }

    #[test]
    fn another_key_with_it_is_the_apps() {
        let mut m = hold(Shortcut::parse("right-option"));
        let t = Instant::now();
        assert_eq!(m.key(flags(61, RIGHT_OPTION), t, none).0, Some(Do::Start));
        // (⌥E, for é.)
        assert_eq!(m.key(down(14, RIGHT_OPTION), t + Duration::from_millis(500), none), (Some(Do::Cancel), false));
        assert_eq!(m.key(up(14, RIGHT_OPTION), t + Duration::from_millis(600), none).0, None);
        assert_eq!(m.key(flags(61, 0), t + Duration::from_millis(700), none).0, None);
        // (Held with ⌘ first: not it either.)
        let command = 0x100000 | 0x8;
        assert_eq!(m.key(flags(55, command), t + Duration::from_secs(1), none).0, None);
        assert_eq!(m.key(flags(61, command | RIGHT_OPTION), t + Duration::from_secs(1), none).0, None);
    }

    #[test]
    fn press_twice_for_hands_free() {
        let mut m = hold(Shortcut::parse("right-option"));
        let t = Instant::now();
        let ms = |n| t + Duration::from_millis(n);
        assert_eq!(m.key(flags(61, RIGHT_OPTION), t, none).0, Some(Do::Start));
        assert_eq!(m.key(flags(61, 0), ms(120), none).0, None);
        assert_eq!(m.tick(ms(200)), None);
        assert_eq!(m.key(flags(61, RIGHT_OPTION), ms(300), none).0, Some(Do::Lock));
        assert_eq!(m.key(flags(61, 0), ms(400), none).0, None);
        // (Typing meanwhile is fine.)
        assert_eq!(m.key(down(0, 0), ms(5000), none).0, None);
        assert_eq!(m.key(up(0, 0), ms(5100), none).0, None);
        assert_eq!(m.key(flags(61, RIGHT_OPTION), ms(9000), none).0, Some(Do::Finish));
        assert_eq!(m.key(flags(61, 0), ms(9100), none).0, None);
    }

    #[test]
    fn a_tap_alone_is_nothing() {
        let mut m = hold(Shortcut::parse("right-option"));
        let t = Instant::now();
        let ms = |n| t + Duration::from_millis(n);
        assert_eq!(m.key(flags(61, RIGHT_OPTION), t, none).0, Some(Do::Start));
        assert_eq!(m.key(flags(61, 0), ms(100), none).0, None);
        assert_eq!(m.tick(ms(300)), None);
        assert_eq!(m.tick(ms(600)), Some(Do::Cancel));
        // (Or typing after it.)
        assert_eq!(m.key(flags(61, RIGHT_OPTION), ms(1000), none).0, Some(Do::Start));
        assert_eq!(m.key(flags(61, 0), ms(1100), none).0, None);
        assert_eq!(m.key(down(0, 0), ms(1200), none).0, Some(Do::Cancel));
    }

    #[test]
    fn escape_cancels() {
        let mut m = hold(Shortcut::parse("right-option"));
        let t = Instant::now();
        assert_eq!(m.key(flags(61, RIGHT_OPTION), t, none).0, Some(Do::Start));
        assert_eq!(m.key(down(ESCAPE, RIGHT_OPTION), t + Duration::from_secs(1), none), (Some(Do::Cancel), true));
        // (Still held as escape's let go: not again till it's let go too.)
        assert_eq!(m.key(up(ESCAPE, RIGHT_OPTION), t + Duration::from_millis(1100), none).0, None);
        assert_eq!(m.key(flags(61, 0), t + Duration::from_millis(1200), none).0, None);
        assert_eq!(m.key(flags(61, RIGHT_OPTION), t + Duration::from_millis(1500), none).0, Some(Do::Start));
        assert_eq!(m.key(down(ESCAPE, RIGHT_OPTION), t + Duration::from_millis(1600), none).0, Some(Do::Cancel));
        assert_eq!(m.key(flags(61, 0), t + Duration::from_millis(1700), none).0, None);
        // (Not listening: escape is the app's.)
        assert_eq!(m.key(down(ESCAPE, 0), t + Duration::from_secs(2), none), (None, false));
    }

    #[test]
    fn two_modifiers() {
        let mut m = hold(Shortcut::parse("control+option"));
        let t = Instant::now();
        assert_eq!(m.key(flags(59, CONTROL), t, none).0, None);
        assert_eq!(m.key(flags(58, CONTROL_OPTION), t, none).0, Some(Do::Start));
        assert_eq!(m.key(flags(58, CONTROL), t + Duration::from_secs(1), none).0, Some(Do::Finish));
        assert_eq!(m.key(flags(59, 0), t + Duration::from_secs(1), none).0, None);
    }

    #[test]
    fn a_modifier_and_a_key() {
        let mut m = hold(Shortcut::parse("option+space"));
        let t = Instant::now();
        assert_eq!(m.key(flags(58, LEFT_OPTION), t, none), (None, false));
        // (The space doesn't reach the app.)
        assert_eq!(m.key(down(49, LEFT_OPTION), t, none), (Some(Do::Start), true));
        assert_eq!(m.key(Event { repeat: true, ..down(49, LEFT_OPTION) }, t + Duration::from_millis(500), none), (None, true));
        assert_eq!(m.key(up(49, LEFT_OPTION), t + Duration::from_secs(1), none), (Some(Do::Finish), true));
        assert_eq!(m.key(flags(58, 0), t + Duration::from_secs(1), none), (None, false));
        // (A space typed alone is the app's.)
        assert_eq!(m.key(down(49, 0), t + Duration::from_secs(2), none), (None, false));
    }

    #[test]
    fn a_key_up_missed_is_forgotten() {
        let mut m = hold(Shortcut::parse("option+space"));
        let t = Instant::now();
        // (A held, its letting go not heard.)
        m.key(down(0, 0), t, none);
        m.key(flags(58, LEFT_OPTION), t, none);
        assert_eq!(m.key(down(49, LEFT_OPTION), t, none).0, Some(Do::Start));
    }

    #[test]
    fn fn_key() {
        let mut m = hold(Shortcut::parse("fn"));
        let t = Instant::now();
        assert_eq!(m.key(flags(FN_CODE, FN_FLAG), t, none).0, Some(Do::Start));
        // (Fn ←: Home.)
        assert_eq!(m.key(down(123, FN_FLAG), t + Duration::from_secs(1), none).0, Some(Do::Cancel));
    }

    #[test]
    fn longest() {
        let mut m = hold(Shortcut::parse("right-option"));
        let t = Instant::now();
        m.key(flags(61, RIGHT_OPTION), t, none);
        assert_eq!(m.tick(t + Duration::from_secs(60)), None);
        assert_eq!(m.tick(t + LONGEST + Duration::from_secs(1)), Some(Do::Finish));
    }

    #[test]
    fn recording_shortcuts() {
        let record = |events: &[Event]| {
            let mut r = Recorder::default();
            events.iter().filter_map(|e| r.key(*e, none)).last()
        };
        let set = |s: &str| Some(Recorded::Set(Shortcut::parse(s).unwrap()));
        assert_eq!(record(&[flags(61, RIGHT_OPTION), flags(61, 0)]), set("right-option"));
        assert_eq!(record(&[flags(59, CONTROL), flags(58, CONTROL_OPTION), flags(58, CONTROL), flags(59, 0)]), set("control+option"));
        assert_eq!(record(&[flags(58, LEFT_OPTION), down(49, LEFT_OPTION), up(49, LEFT_OPTION), flags(58, 0)]), set("option+space"));
        assert_eq!(record(&[flags(FN_CODE, FN_FLAG), flags(FN_CODE, 0)]), set("fn"));
        assert_eq!(record(&[down(96, 0), up(96, 0)]), set("f5"));
        assert_eq!(record(&[down(ESCAPE, 0), up(ESCAPE, 0)]), Some(Recorded::Cancel));
        assert!(matches!(record(&[down(0, 0), up(0, 0)]), Some(Recorded::Invalid(_))));
        assert!(matches!(record(&[down(0, 0), down(1, 0), up(0, 0), up(1, 0)]), Some(Recorded::Invalid(_))));
        let three = 0x40000 | 0x1 | 0x80000 | 0x20 | 0x20000 | 0x2;
        assert!(matches!(record(&[flags(59, CONTROL), flags(58, CONTROL_OPTION), flags(56, three), flags(56, 0)]), Some(Recorded::Invalid(_))));
    }
}
