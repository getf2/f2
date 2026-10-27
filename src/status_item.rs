//! Finder2 in the menu bar. Like Finder it's always running: ⌘Q only closes
//! its windows. This item opens a window, a folder or the search again, and
//! is where it's quit. Beside it, the soonest timer's countdown and a cup
//! while the Mac's kept awake; atop its menu, each to cancel.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use futures::StreamExt;
use futures::channel::mpsc::{UnboundedSender, unbounded};
use gpui::*;

use crate::macos::{self, OPEN_AT_LOGIN, StatusEntry};
use crate::{APP_NAME, browser};

const NEW_WINDOW: i32 = 0;
const SEARCH: i32 = 1;
const SETTINGS: i32 = 2;
const QUIT: i32 = 3;
const DISK_USAGE: i32 = 4;
const QUICK_OPEN: i32 = 5;
const WELCOME: i32 = 6;
const ABOUT: i32 = 7;
const UPDATES: i32 = 8;
const SHOT_HISTORY: i32 = 9;
/// The screenshots' tags start here, in `shots::Action::ALL` order.
const SHOTS: i32 = 30;
/// The places' tags start here, in `places()` order.
const PLACES: i32 = 10;
/// What's going on's tags (atop the menu: see `refresh`) start here.
const GOING_ON: i32 = 1001;

/// What each of the items atop the menu stops.
#[derive(Clone, Copy)]
enum Stops {
    Timer(u64),
    Awake,
    Recording,
    Stopwatch,
}

static GOING: Mutex<Vec<Stops>> = Mutex::new(Vec::new());
/// Ticking (a second at a time) while anything's going on.
static TICKING: AtomicBool = AtomicBool::new(false);

/// Beside the icon and atop the menu, what's going on as it is now; and,
/// while anything is, a tick a second (timers rung, keeping awake stopped
/// when it's time).
pub fn refresh(cx: &mut App) {
    let timers = crate::timers::list();
    let awake = crate::awake::state();
    let now = crate::timers::now();
    let recording = crate::shots::record::elapsed();
    let stopwatch = crate::timers::stopwatch();
    macos::status_title(&title(&timers, awake.is_some(), recording, stopwatch, now));
    let mut items = Vec::new();
    let mut going = Vec::new();
    if recording.is_some() {
        items.push(("Stop Recording".to_string(), "stop.circle", GOING_ON + going.len() as i32));
        going.push(Stops::Recording);
    }
    // (Its time's beside the icon: the menu's items aren't rebuilt as it runs.)
    if stopwatch.is_some() {
        items.push(("Stop Stopwatch".to_string(), "stopwatch", GOING_ON + going.len() as i32));
        going.push(Stops::Stopwatch);
    }
    for timer in &timers {
        let at = macos::zone_format(&macos::local_zone(), timer.ends, "jmm");
        let kind = if timer.alarm { "Alarm" } else { "Timer" };
        let what = if timer.label.is_empty() { kind.to_string() } else { format!("“{}” {kind}", timer.label) };
        let (label, symbol) = if timer.every > 0 {
            (format!("Stop “{}” (Every {})", timer.label, crate::timers::till(timer.every)), "figure.walk")
        } else if timer.alarm {
            (format!("Cancel {what} ({at})"), "alarm")
        } else {
            (format!("Cancel {what} (Ends {at})"), "timer")
        };
        items.push((label, symbol, GOING_ON + going.len() as i32));
        going.push(Stops::Timer(timer.id));
    }
    if let Some(until) = awake {
        items.push((format!("Stop Keeping Awake ({})", crate::awake::till(until)), "cup.and.saucer", GOING_ON + going.len() as i32));
        going.push(Stops::Awake);
    }
    // (Only when that's changed: it's rebuilt as the menu's open otherwise.)
    let changed = {
        let mut known = GOING.lock().unwrap();
        let same = known.len() == going.len()
            && known.iter().zip(&going).all(|(a, b)| matches!((a, b), (Stops::Timer(x), Stops::Timer(y)) if x == y) || matches!((a, b), (Stops::Awake, Stops::Awake) | (Stops::Recording, Stops::Recording) | (Stops::Stopwatch, Stops::Stopwatch)));
        *known = going;
        !same
    };
    if changed {
        macos::status_extra(&items);
    }
    if (!timers.is_empty() || awake.is_some() || recording.is_some() || stopwatch.is_some()) && !TICKING.swap(true, Ordering::SeqCst) {
        cx.spawn(async move |cx| {
            loop {
                // (On the second, so the countdown steps evenly.)
                let ms = 1000 - (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_millis()) as u64);
                cx.background_executor().timer(std::time::Duration::from_millis(ms.max(50))).await;
                let more = cx.update(|cx| {
                    crate::timers::tick(cx);
                    crate::awake::tick(cx);
                    refresh_title_only(cx)
                });
                if !more {
                    TICKING.store(false, Ordering::SeqCst);
                    cx.update(refresh);
                    break;
                }
            }
        })
        .detach();
    }
}

/// Beside the icon: ● and how long it's been recording, a cup while the
/// Mac's kept awake, the stopwatch, and what `going_on` says.
fn title(timers: &[crate::timers::Timer], awake: bool, recording: Option<u64>, stopwatch: Option<(std::time::Duration, bool)>, now: i64) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(secs) = recording {
        parts.push(format!("● {}", crate::shots::record::clock(secs)));
    }
    if awake {
        parts.push("☕".into());
    }
    if let Some((ran, _)) = stopwatch {
        parts.push(format!("⏱ {}", crate::timers::countdown(ran.as_secs() as i64)));
    }
    let mut title = parts.join(" ");
    going_on(&mut title, timers, now);
    title
}

/// After those: the soonest timer's countdown; else, the next alarm's time
/// ("⏰ 7:00": an alarm hours off isn't counted down). (Not a break's: it's
/// due again and again.)
fn going_on(title: &mut String, timers: &[crate::timers::Timer], now: i64) {
    let shown = match timers.iter().find(|t| !t.alarm && t.every == 0) {
        Some(soonest) => crate::timers::countdown(soonest.ends - now),
        None => match timers.iter().find(|t| t.alarm) {
            Some(alarm) => format!("⏰ {}", macos::zone_format(&macos::local_zone(), alarm.ends, "jmm")),
            None => return,
        },
    };
    if !title.is_empty() {
        title.push(' ');
    }
    title.push_str(&shown);
}

/// The countdown, as the clock ticks (the menu's items stay, unless what's
/// going on changed). Whether anything still is.
fn refresh_title_only(cx: &mut App) -> bool {
    let (timers, awake, recording, stopwatch) = (crate::timers::list(), crate::awake::state(), crate::shots::record::elapsed(), crate::timers::stopwatch());
    let known = GOING.lock().unwrap().len();
    if timers.len() + usize::from(awake.is_some()) + usize::from(recording.is_some()) + usize::from(stopwatch.is_some()) != known {
        refresh(cx);
    } else {
        macos::status_title(&title(&timers, awake.is_some(), recording, stopwatch, crate::timers::now()));
    }
    !timers.is_empty() || awake.is_some() || recording.is_some() || stopwatch.is_some()
}

static CHOSEN: OnceLock<UnboundedSender<i32>> = OnceLock::new();

extern "C" fn on_chosen(tag: i32) {
    if let Some(tx) = CHOSEN.get() {
        let _ = tx.unbounded_send(tag);
    }
}

/// The folders it goes to, as the sidebar shows them.
fn places() -> Vec<(String, &'static str, PathBuf)> {
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| "/".into()));
    let user = home.file_name().map_or("Home".into(), |n| n.to_string_lossy().into_owned());
    vec![
        ("Applications".into(), "a.circle", PathBuf::from("/Applications")),
        ("Desktop".into(), "menubar.dock.rectangle", home.join("Desktop")),
        ("Documents".into(), "doc", home.join("Documents")),
        ("Downloads".into(), "arrow.down.circle", home.join("Downloads")),
        (user, "house", home),
    ]
}

pub fn install(cx: &mut App) {
    let places = places();
    let mut entries: Vec<StatusEntry> = vec![
        Some((format!("New {APP_NAME} Window"), Some("macwindow"), NEW_WINDOW)),
        Some(("Quick Open…".into(), Some("bolt"), QUICK_OPEN)),
        Some(("Search This Mac…".into(), Some("magnifyingglass"), SEARCH)),
        Some(("Disk Usage".into(), Some("chart.pie"), DISK_USAGE)),
        None,
    ];
    entries.extend(crate::shots::Action::ALL.iter().zip(SHOTS..).map(|(a, tag)| Some((a.title().to_string(), Some(a.symbol()), tag))));
    entries.push(Some(("Screenshot History…".into(), Some("photo.on.rectangle"), SHOT_HISTORY)));
    entries.push(None);
    entries.extend(places.iter().zip(PLACES..).map(|((name, symbol, _), tag)| Some((name.clone(), Some(*symbol), tag))));
    entries.extend([
        None,
        Some(("Settings…".into(), Some("gearshape"), SETTINGS)),
        Some((format!("Welcome to {APP_NAME}…"), Some("hand.wave"), WELCOME)),
        Some((format!("About {APP_NAME}"), Some("info.circle"), ABOUT)),
        Some(("Check for Updates…".into(), Some("arrow.down.circle"), UPDATES)),
        Some(("Open at Login".into(), None, OPEN_AT_LOGIN)),
        None,
        Some((format!("Quit {APP_NAME}"), None, QUIT)),
    ]);
    let (tx, mut rx) = unbounded();
    let _ = CHOSEN.set(tx);
    macos::install_status_item(&entries, on_chosen);
    // (Chosen inside the menu's own event loop: acted on once it's closed.)
    cx.spawn(async move |cx| {
        while let Some(tag) = rx.next().await {
            cx.update(|cx| choose(tag, &places, cx));
        }
    })
    .detach();
}

fn choose(tag: i32, places: &[(String, &'static str, PathBuf)], cx: &mut App) {
    match tag {
        NEW_WINDOW => {
            browser::open(crate::finder::new_window_folder(cx), None, cx);
            cx.activate(true);
        }
        SEARCH => browser::show(None, true, cx),
        SETTINGS => {
            crate::settings_window::open(cx);
            cx.activate(true);
        }
        QUIT => cx.quit(),
        DISK_USAGE => crate::disk_usage::open(cx),
        QUICK_OPEN => crate::quick_panel::show(None, cx),
        WELCOME => crate::onboarding::open(cx),
        ABOUT => crate::about::show(),
        UPDATES => crate::about::check(cx),
        SHOT_HISTORY => crate::quick_panel::show_mode(crate::quick_panel::Mode::Screenshots, cx),
        tag if tag >= GOING_ON => {
            let stops = GOING.lock().unwrap().get((tag - GOING_ON) as usize).copied();
            match stops {
                Some(Stops::Timer(id)) => crate::timers::cancel(id, cx),
                Some(Stops::Awake) => crate::awake::stop(None, cx),
                Some(Stops::Recording) => crate::shots::record::stop(),
                Some(Stops::Stopwatch) => crate::timers::stopwatch_do(crate::timers::Watch::Stop, cx),
                None => {}
            }
        }
        tag if (SHOTS..SHOTS + crate::shots::Action::ALL.len() as i32).contains(&tag) => {
            crate::shots::capture(crate::shots::Action::ALL[(tag - SHOTS) as usize], cx);
        }
        tag => {
            if let Some((_, _, path)) = usize::try_from(tag - PLACES).ok().and_then(|i| places.get(i)) {
                browser::show(Some(path.clone()), false, cx);
            }
        }
    }
}
