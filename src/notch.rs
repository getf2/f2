//! The notch, as Alcove and the iPhone's Dynamic Island do it (drawn by
//! src/macos/notch.m): beside the notch, what's going on (a timer counting
//! down, the stopwatch, a screen recording, dictation listening, what's
//! playing); notes come down from it instead of the corner; pointed at, it
//! opens to show all of that, to stop or skip, and the last notes. Screens
//! without a notch get one at the top while there's something to show. Off
//! unless Settings › Notch turns it on.

use std::ffi::{CStr, c_char};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use futures::StreamExt;
use futures::channel::mpsc::{UnboundedSender, unbounded};
use gpui::App;
use serde_json::{Value, json};

use crate::macos::{self, MediaKey};
use crate::music::{self, Player};
use crate::settings::{Notch, Settings};

/// Timers' color (the stopwatch's too), recording's, keeping awake's.
const ORANGE: u32 = 0xff9f0a;
const RED: u32 = 0xff453a;
const BROWN: u32 = 0xac8e68;

/// What's stopped from it: a timer, by its own id, or these.
const AWAKE: i64 = -1;
const RECORDING: i64 = -2;
const STOPWATCH: i64 = -3;

/// As notch.m has them: a thing stopped, play or pause, the next song or
/// the one before, the player shown.
const STOP: i32 = 1;
const PLAY_PAUSE: i32 = 2;
const NEXT: i32 = 3;
const PREVIOUS: i32 = 4;
const PLAYER: i32 = 5;

/// Settings' notch, as it was last put up.
static APPLIED: Mutex<Option<Notch>> = Mutex::new(None);
static CHOSEN: OnceLock<UnboundedSender<(i32, i64)>> = OnceLock::new();
static HEARD: OnceLock<UnboundedSender<String>> = OnceLock::new();

/// What's playing, as its player last said; its artwork, once it's asked
/// (only if F2 may ask it: see `music::now`).
#[derive(Clone)]
struct Song {
    player: Player,
    title: String,
    artist: String,
    playing: bool,
    art: Option<PathBuf>,
}

static SONG: Mutex<Option<Song>> = Mutex::new(None);

pub fn init(cx: &mut App) {
    let (tx, mut rx) = unbounded();
    let _ = CHOSEN.set(tx);
    cx.spawn(async move |cx| {
        while let Some((what, id)) = rx.next().await {
            cx.update(|cx| chosen(what, id, cx));
        }
    })
    .detach();
    let (tx, mut rx) = unbounded::<String>();
    let _ = HEARD.set(tx);
    cx.spawn(async move |cx| {
        while let Some(said) = rx.next().await {
            cx.update(|cx| playing(&said, cx));
        }
    })
    .detach();
    apply(cx);
    cx.observe_global::<Settings>(apply).detach();
}

fn apply(cx: &mut App) {
    let wanted = Settings::get(cx).notch.clone();
    let was = {
        let mut applied = APPLIED.lock().unwrap();
        if applied.as_ref() == Some(&wanted) {
            return;
        }
        applied.replace(wanted.clone())
    };
    macos::notch_setup(wanted.on, wanted.other_screens, chose);
    let listen = wanted.on && wanted.music;
    if was.is_none_or(|w| (w.on && w.music) != listen) {
        macos::notch_music(listen.then_some(heard as extern "C" fn(*const c_char)));
        *SONG.lock().unwrap() = None;
        if listen {
            artwork(None, cx);
        }
    }
    refresh(cx);
}

/// A note from the notch (if it's on, notes come from it, and the screen
/// with the pointer can show it), with its color, else its symbol on a
/// tile: false if the corner's to show it.
pub fn note(title: &str, detail: &str, color: Option<u32>, symbol: (&str, u32), link: Option<&str>) -> bool {
    let notes = APPLIED.lock().unwrap().as_ref().is_some_and(|n| n.on && n.notes);
    notes && macos::notch_note(title, detail, color, symbol, link)
}

/// What's going on, to the notch: as the menu bar item shows it (and as
/// often: see `status_item::refresh`), and what's playing.
pub fn refresh(cx: &mut App) {
    let Some(notch) = APPLIED.lock().unwrap().clone().filter(|n| n.on) else { return };
    let _ = cx;
    let now = crate::timers::now();
    let timers = crate::timers::list();
    let recording = crate::shots::record::elapsed();
    let stopwatch = crate::timers::stopwatch();
    let mut going = Vec::new();
    if let Some(secs) = recording {
        going.push(json!({ "id": RECORDING, "symbol": "record.circle.fill", "tint": RED, "title": "Recording the Screen", "text": crate::shots::record::clock(secs) }));
    }
    if let Some((ran, running)) = stopwatch {
        let title = if running { "Stopwatch" } else { "Stopwatch (Paused)" };
        going.push(json!({ "id": STOPWATCH, "symbol": "stopwatch.fill", "tint": ORANGE, "title": title, "text": crate::timers::countdown(ran.as_secs() as i64) }));
    }
    for timer in &timers {
        let named = |kind: &str| if timer.label.is_empty() { kind.to_string() } else { timer.label.clone() };
        let (symbol, title, text) = if timer.every > 0 {
            ("figure.walk", named("Break"), crate::timers::countdown(timer.ends - now))
        } else if timer.alarm {
            ("alarm.fill", named("Alarm"), macos::zone_format(&macos::local_zone(), timer.ends, "jmm"))
        } else {
            ("timer", named("Timer"), crate::timers::countdown(timer.ends - now))
        };
        going.push(json!({ "id": timer.id as i64, "symbol": symbol, "tint": ORANGE, "title": title, "text": text }));
    }
    if let Some(until) = crate::awake::state() {
        going.push(json!({ "id": AWAKE, "symbol": "cup.and.saucer.fill", "tint": BROWN, "title": format!("Keeping the Mac Awake {}", crate::awake::till(until)), "text": "" }));
    }
    // (Beside the notch: one of them, as the menu bar item's title has it.)
    let live = if let Some(secs) = recording {
        json!({ "symbol": "record.circle.fill", "tint": RED, "text": crate::shots::record::clock(secs) })
    } else if let Some((ran, _)) = stopwatch {
        json!({ "symbol": "stopwatch.fill", "tint": ORANGE, "text": crate::timers::countdown(ran.as_secs() as i64) })
    } else if let Some(soonest) = timers.iter().find(|t| !t.alarm && t.every == 0) {
        json!({ "symbol": "timer", "tint": ORANGE, "text": crate::timers::countdown(soonest.ends - now) })
    } else {
        Value::Null
    };
    let song = if notch.music { SONG.lock().unwrap().clone() } else { None };
    let music = song.map(|s| json!({ "title": s.title, "artist": s.artist, "playing": s.playing, "art": s.art.map(|p| p.to_string_lossy().into_owned()) }));
    macos::notch_live(&json!({ "live": live, "going": going, "music": music }).to_string());
}

extern "C" fn chose(what: i32, id: i64) {
    if let Some(tx) = CHOSEN.get() {
        let _ = tx.unbounded_send((what, id));
    }
}

fn chosen(what: i32, id: i64, cx: &mut App) {
    match what {
        STOP => match id {
            AWAKE => crate::awake::stop(None, cx),
            RECORDING => crate::shots::record::stop(),
            STOPWATCH => crate::timers::stopwatch_do(crate::timers::Watch::Stop, cx),
            id => crate::timers::cancel(id as u64, cx),
        },
        PLAY_PAUSE => crate::media::press(MediaKey::Play, cx),
        NEXT => crate::media::press(MediaKey::Next, cx),
        PREVIOUS => crate::media::press(MediaKey::Previous, cx),
        PLAYER => {
            if let Some(app) = SONG.lock().unwrap().as_ref().and_then(|s| s.player.app()) {
                cx.open_with_system(&app);
            }
        }
        _ => {}
    }
    crate::status_item::refresh(cx);
}

extern "C" fn heard(json: *const c_char) {
    if json.is_null() {
        return;
    }
    let said = unsafe { CStr::from_ptr(json) }.to_string_lossy().into_owned();
    if let Some(tx) = HEARD.get() {
        let _ = tx.unbounded_send(said);
    }
}

/// Music or Spotify said what it's playing (or that it's paused, or
/// stopped).
fn playing(said: &str, cx: &mut App) {
    let Ok(said) = serde_json::from_str::<Value>(said) else { return };
    let text = |key: &str| said[key].as_str().unwrap_or_default().to_string();
    let player = if text("player") == "Spotify" { Player::Spotify } else { Player::Music };
    let state = text("state");
    let (title, artist) = (text("title"), text("artist"));
    let changed = {
        let mut song = SONG.lock().unwrap();
        // (One paused or stopped while the other plays: that one stays.)
        if state != "Playing" && song.as_ref().is_some_and(|s| s.player != player && s.playing) {
            return;
        }
        if state == "Stopped" || title.is_empty() {
            *song = None;
            false
        } else {
            let same = song.as_ref().is_some_and(|s| s.player == player && s.title == title && s.artist == artist);
            let art = if same { song.as_ref().and_then(|s| s.art.clone()) } else { None };
            *song = Some(Song { player, title, artist, playing: state == "Playing", art });
            !same
        }
    };
    refresh(cx);
    if changed {
        artwork(Some(player), cx);
    }
}

/// What's playing asked for (with its artwork): of `player`, if F2 may ask
/// it (else nothing's asked); None: whichever's playing, as F2 starts.
fn artwork(player: Option<Player>, cx: &mut App) {
    if player.is_some_and(|p| macos::automation(p.bundle(), false) != 0) {
        return;
    }
    cx.spawn(async move |cx| {
        let now = cx.background_executor().spawn(async { music::now() }).await;
        let music::Now::Playing(found) = now else { return };
        {
            let mut song = SONG.lock().unwrap();
            match song.as_mut() {
                Some(s) if s.player == found.player && s.title == found.title => s.art = found.artwork,
                Some(_) => return,
                None if player.is_none() => {
                    *song = Some(Song { player: found.player, title: found.title, artist: found.artist, playing: found.playing, art: found.artwork })
                }
                None => return,
            }
        }
        cx.update(refresh);
    })
    .detach();
}
