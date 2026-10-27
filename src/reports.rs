//! Telling F2's maker what went wrong, as you say: a report of F2 having
//! quit unexpectedly (asked about at the next start; or always sent, or
//! never, as Settings › Advanced says), and feedback (Help › Send
//! Feedback…). They go to getf2.com only, which emails them on, and carry
//! only what's said where they're sent: F2's and macOS's versions, the
//! Mac's model and, for a crash, macOS's report of where in F2 it stopped,
//! with what could tell whose Mac it was taken out (its ids, your name in
//! its paths).

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_kit::component::input::{Input, InputEvent, InputState, Textarea, TextareaState};
use serde_json::{Value, json};

use crate::APP_NAME;
use crate::settings::{CrashReports, Settings};
use crate::style::{Palette, SEMIBOLD, glass, symbol};

actions!(reports, [SendFeedback]);

/// Where they go (NEW_FINDER_REPORTS: elsewhere, for testing).
fn endpoint(kind: &str) -> String {
    let base = std::env::var("NEW_FINDER_REPORTS").unwrap_or_else(|_| "https://getf2.com/api".into());
    format!("{}/{kind}", base.trim_end_matches('/'))
}

/// At most this many crashes are sent at once (the newest).
const AT_ONCE: usize = 5;

pub fn init(cx: &mut App) {
    cx.on_action(|_: &SendFeedback, cx| open_feedback(cx));
    // (A moment after starting, so it's not in the way of it.)
    cx.spawn(async move |cx| {
        cx.background_executor().timer(Duration::from_secs(6)).await;
        cx.update(check_crashes);
    })
    .detach();
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

fn modified(path: &Path) -> Option<i64> {
    let at = path.metadata().ok()?.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    Some(at.as_secs() as i64)
}

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

// --- Crashes. ---

/// One of macOS's reports of F2 having quit unexpectedly.
#[derive(Clone)]
struct Crash {
    path: PathBuf,
    at: i64,
}

/// This F2's crash reports (not a test copy's) since `since`, oldest first.
fn crashes_since(since: i64) -> Vec<Crash> {
    let prefix = format!("{APP_NAME}-");
    let Ok(dir) = std::fs::read_dir(home().join("Library/Logs/DiagnosticReports")) else { return Vec::new() };
    let mut found: Vec<Crash> = dir
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if !name.starts_with(&prefix) || !name.ends_with(".ips") {
                return None;
            }
            let path = e.path();
            let at = modified(&path).filter(|at| *at > since)?;
            let text = std::fs::read_to_string(&path).ok()?;
            let head: Value = serde_json::from_str(text.lines().next()?).ok()?;
            (head["bundleID"] == crate::takeover::BUNDLE_ID).then_some(Crash { path, at })
        })
        .collect();
    found.sort_by_key(|c| c.at);
    found
}

/// What says whose Mac a crash report came from, or is no use for
/// fixing it.
const LEFT_OUT: [&str; 11] = [
    "crashReporterKey",
    "sleepWakeUUID",
    "bootSessionUUID",
    "userID",
    "incident",
    "storeInfo",
    "trialInfo",
    "appleIntelligenceStatus",
    "pid",
    "parentPid",
    "coalitionID",
];

/// A crash report (macOS's: a line of JSON, then the rest) as it's sent:
/// without the ids that would say whose Mac it's from, and with your home
/// folder as ~ in its paths. None if it isn't one.
fn redacted(text: &str) -> Option<String> {
    let (head, body) = text.split_once('\n')?;
    let mut head: Value = serde_json::from_str(head).ok()?;
    let mut body: Value = serde_json::from_str(body).ok()?;
    head.as_object_mut()?.remove("incident_id");
    let fields = body.as_object_mut()?;
    for key in LEFT_OUT {
        fields.remove(key);
    }
    let text = format!("{head}\n{}", serde_json::to_string_pretty(&body).ok()?);
    Some(without_home(&text, &home()))
}

/// `text` with `home` (/Users/you) as ~.
fn without_home(text: &str, home: &Path) -> String {
    let home = home.to_string_lossy();
    if home.len() < 2 { text.to_string() } else { text.replace(home.as_ref(), "~") }
}

/// F2's own words about it, if it panicked just then (see `log`).
fn panic_near(at: i64) -> Option<String> {
    let file = crate::log::panic_file();
    let when = modified(&file)?;
    let text = std::fs::read_to_string(&file).ok()?;
    ((at - when).abs() <= 120).then(|| without_home(&text, &home()))
}

/// Look for crashes since the last look, and do with them as Settings say.
fn check_crashes(cx: &mut App) {
    let settings = Settings::get(cx);
    // (The first time: the last few days'.)
    let since = if settings.crashes_seen > 0 { settings.crashes_seen } else { now() - 3 * 24 * 3600 };
    let choice = settings.crash_reports;
    cx.spawn(async move |cx| {
        let crashes = cx.background_executor().spawn(async move { crashes_since(since) }).await;
        let Some(newest) = crashes.last().map(|c| c.at) else { return };
        cx.update(|cx| Settings::update(cx, |s| s.crashes_seen = newest));
        match choice {
            CrashReports::Never => {}
            CrashReports::Send => {
                let sent = cx.background_executor().spawn(async move { send_crashes(&crashes) }).await;
                if let Err(e) = sent {
                    eprintln!("reports: couldn't send a crash report: {e}");
                }
            }
            CrashReports::Ask => cx.update(|cx| ask(crashes, cx)),
        }
    })
    .detach();
}

/// Send these (the newest few). Waits: not on the main thread.
fn send_crashes(crashes: &[Crash]) -> Result<(), String> {
    for crash in crashes.iter().rev().take(AT_ONCE) {
        let Some(report) = std::fs::read_to_string(&crash.path).ok().as_deref().and_then(redacted) else { continue };
        let name = crash.path.file_name().unwrap_or_default().to_string_lossy().into_owned();
        send("crash", json!({ "name": name, "report": report, "panic": panic_near(crash.at) }))?;
    }
    let _ = std::fs::remove_file(crate::log::panic_file());
    Ok(())
}

// --- Sending. ---

/// The Mac's model ("MacBookPro18,3").
fn model() -> String {
    let mut buf = [0u8; 64];
    let mut len = buf.len();
    let ok = unsafe { libc::sysctlbyname(c"hw.model".as_ptr(), buf.as_mut_ptr().cast(), &mut len, std::ptr::null_mut(), 0) } == 0;
    if !ok {
        return String::new();
    }
    String::from_utf8_lossy(&buf[..len]).trim_end_matches('\0').to_string()
}

/// What's sent with each: F2's version, macOS's, the Mac's model.
fn about_this_mac() -> (String, String, String) {
    let macos = crate::macos::os_version();
    (crate::about::version(), macos.trim_start_matches("Version ").to_string(), model())
}

/// Send `what` to getf2.com, as `kind` (crash, feedback). Waits: not on the
/// main thread.
fn send(kind: &str, mut what: Value) -> Result<(), String> {
    let (version, macos, model) = about_this_mac();
    if let Some(fields) = what.as_object_mut() {
        fields.insert("version".into(), version.clone().into());
        fields.insert("macos".into(), macos.into());
        fields.insert("model".into(), model.into());
    }
    let mut child = Command::new("/usr/bin/curl")
        .args(["-sS", "--max-time", "30", "-H", "Content-Type: application/json", "-A", &format!("{APP_NAME}/{version}")])
        .args(["--data-binary", "@-", "-w", "\n%{http_code}", &endpoint(kind)])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(what.to_string().as_bytes()).map_err(|e| e.to_string())?;
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out.stdout);
    let (answer, status) = text.rsplit_once('\n').unwrap_or(("", &text));
    match status.trim().parse::<u16>().unwrap_or(0) {
        200..=299 => Ok(()),
        0 => Err("getf2.com couldn't be reached. Are you online?".into()),
        code => Err(serde_json::from_str::<Value>(answer)
            .ok()
            .and_then(|v| v["error"].as_str().map(String::from))
            .unwrap_or_else(|| format!("getf2.com answered {code}."))),
    }
}

/// How sending's going.
#[derive(Clone, PartialEq)]
enum Sending {
    No,
    Now,
    Sent,
    Failed(String),
}

fn button(id: &'static str, label: &'static str, primary: bool, palette: &Palette) -> Stateful<Div> {
    glass(28., palette)
        .id(id)
        .px_4()
        .text_size(px(13.))
        .when(primary, |d| d.bg(palette.accent).text_color(palette.selected_text).border_color(palette.accent))
        .child(label)
}

fn checkbox(id: &'static str, label: &'static str, on: bool, palette: &Palette, window: &Window) -> Stateful<Div> {
    div()
        .id(id)
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
}

/// Where sending's said, under a window's text.
fn said(sending: &Sending, palette: &Palette) -> Option<Div> {
    let (text, color) = match sending {
        Sending::No => return None,
        Sending::Now => ("Sending…".to_string(), palette.secondary),
        Sending::Sent => ("Sent. Thank you!".to_string(), palette.label),
        Sending::Failed(why) => (format!("Couldn't send it: {why}"), gpui::red()),
    };
    Some(div().text_size(px(12.)).text_color(color).child(text))
}

/// What's sent and why, said on getf2.com.
const PRIVACY: &str = "https://getf2.com/privacy";

fn privacy_link(palette: &Palette) -> Stateful<Div> {
    div()
        .id("privacy")
        .text_size(px(12.))
        .text_color(palette.accent)
        .cursor_pointer()
        .child("What's sent, and who sees it")
        .on_click(|_, _, cx| cx.open_url(PRIVACY))
}

/// Close `window` a moment after it's said it's sent.
fn close_soon(window: &mut Window, cx: &mut App) {
    window
        .spawn(cx, async move |cx| {
            cx.background_executor().timer(Duration::from_millis(1200)).await;
            cx.update(|window, _| window.remove_window()).ok();
        })
        .detach();
}

// --- Asking about a crash. ---

/// Ask whether to send these (in a window of its own, put up without
/// taking the keyboard from what you're doing).
fn ask(crashes: Vec<Crash>, cx: &mut App) {
    let bounds = Bounds::centered(None, size(px(480.), px(270.)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions { title: Some(format!("{APP_NAME} quit unexpectedly").into()), appears_transparent: true, traffic_light_position: Some(point(px(14.), px(14.))) }),
        is_resizable: false,
        focus: false,
        ..Default::default()
    };
    gpui_kit::open_window(options, cx, |window, cx| cx.new(|_| Ask { crashes, again: false, sending: Sending::No, palette: Palette::for_window(window) })).ok();
}

struct Ask {
    crashes: Vec<Crash>,
    /// Next time, the same without asking.
    again: bool,
    sending: Sending,
    palette: Palette,
}

impl Ask {
    fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.again {
            Settings::update(cx, |s| s.crash_reports = CrashReports::Send);
        }
        self.sending = Sending::Now;
        cx.notify();
        let crashes = self.crashes.clone();
        cx.spawn_in(window, async move |this, cx| {
            let sent = cx.background_executor().spawn(async move { send_crashes(&crashes) }).await;
            this.update_in(cx, |this, window, cx| {
                this.sending = match sent {
                    Ok(()) => {
                        close_soon(window, cx);
                        Sending::Sent
                    }
                    Err(why) => Sending::Failed(why),
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn dont(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.again {
            Settings::update(cx, |s| s.crash_reports = CrashReports::Never);
        }
        let _ = std::fs::remove_file(crate::log::panic_file());
        window.remove_window();
    }

    /// The newest, as it'd be sent, in TextEdit.
    fn show(&mut self, cx: &mut Context<Self>) {
        let Some(crash) = self.crashes.last() else { return };
        let Some(report) = std::fs::read_to_string(&crash.path).ok().as_deref().and_then(redacted) else { return };
        let panic = panic_near(crash.at).map(|p| format!("\n\nWhat F2 said as it stopped:\n{p}")).unwrap_or_default();
        let (version, macos, model) = about_this_mac();
        let text = format!("F2 {version}, macOS {macos}, {model}{panic}\n\nmacOS's report:\n{report}\n");
        let file = std::env::temp_dir().join(format!("{APP_NAME} crash report.txt"));
        if std::fs::write(&file, text).is_ok() {
            cx.open_with_system(&file);
        }
    }
}

impl Render for Ask {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette;
        let times = self.crashes.len();
        let title = if times > 1 { format!("{APP_NAME} quit unexpectedly {times} times") } else { format!("{APP_NAME} quit unexpectedly") };
        let busy = matches!(self.sending, Sending::Now | Sending::Sent);
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_3()
            .px_6()
            .pt(px(38.))
            .pb_5()
            .bg(palette.background)
            .text_color(palette.label)
            .text_size(px(13.))
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| {
                if e.keystroke.key == "escape" && !matches!(this.sending, Sending::Now) {
                    this.dont(window, cx);
                }
            }))
            .child(
                div()
                    .flex()
                    .gap_3()
                    // (macOS's alert yellow.)
                    .child(symbol("exclamationmark.triangle.fill", 26., SEMIBOLD, rgb(0xffcc00).into(), window))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(div().font_weight(FontWeight::SEMIBOLD).child(title))
                            .child(div().text_size(px(12.)).text_color(palette.secondary).child(format!(
                                "Send a report of it to {APP_NAME}'s maker, to help fix it? It says where in {APP_NAME} it stopped, {APP_NAME}'s and macOS's versions and your Mac's model: nothing of your files, and nothing that says who you are."
                            ))),
                    ),
            )
            .child(
                checkbox("again", "Next time, do the same without asking", self.again, &palette, window)
                    .ml(px(38.))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.again = !this.again;
                        cx.notify();
                    })),
            )
            .child(privacy_link(&palette).ml(px(38.)))
            .children(said(&self.sending, &palette).map(|d| d.ml(px(38.))))
            .child(div().flex_1())
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(button("show", "Show Report", false, &palette).on_click(cx.listener(|this, _, _, cx| this.show(cx))))
                    .child(div().flex_1())
                    .when(!busy, |d| {
                        d.child(button("dont", "Don't Send", false, &palette).on_click(cx.listener(|this, _, window, cx| this.dont(window, cx))))
                            .child(
                                button("send", if matches!(self.sending, Sending::Failed(_)) { "Try Again" } else { "Send Report" }, true, &palette)
                                    .on_click(cx.listener(|this, _, window, cx| this.send(window, cx))),
                            )
                    }),
            )
    }
}

// --- Feedback. ---

/// The Send Feedback window, if it's open.
struct FeedbackOpen(Option<AnyWindowHandle>);

impl Global for FeedbackOpen {}

/// Help › Send Feedback…: what's on your mind, to F2's maker.
pub fn open_feedback(cx: &mut App) {
    if let Some(handle) = cx.try_global::<FeedbackOpen>().and_then(|o| o.0)
        && crate::raise(handle, cx)
    {
        return;
    }
    let bounds = Bounds::centered(None, size(px(520.), px(440.)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions { title: Some("Send Feedback".into()), appears_transparent: true, traffic_light_position: Some(point(px(14.), px(14.))) }),
        is_resizable: false,
        ..Default::default()
    };
    if let Ok((handle, _)) = gpui_kit::open_window(options, cx, |window, cx| cx.new(|cx| Feedback::new(window, cx))) {
        cx.set_global(FeedbackOpen(Some(handle)));
    }
    cx.activate(true);
}

struct Feedback {
    message: Entity<TextareaState>,
    email: Entity<InputState>,
    sending: Sending,
    palette: Palette,
    about: String,
    _send_on_enter: Subscription,
}

impl Feedback {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let message = cx.new(|cx| TextareaState::new(window, cx).auto_grow(6, 10).placeholder("Something not working, or something you'd like F2 to do?"));
        // ⌘↩ sends.
        let send_on_enter = cx.subscribe_in(&message, window, |this: &mut Feedback, _, e: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { secondary: true, .. } = e {
                this.send(window, cx);
            }
        });
        let email = cx.new(|cx| InputState::new(window, cx).placeholder("Your email, if you'd like an answer (optional)"));
        message.update(cx, |m, cx| m.focus(window, cx));
        let (version, macos, model) = about_this_mac();
        let about = format!("Sent with {APP_NAME} {version}, macOS {macos} and your Mac's model ({model}).");
        Feedback { message, email, sending: Sending::No, palette: Palette::for_window(window), about, _send_on_enter: send_on_enter }
    }

    fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let message = self.message.read(cx).value().trim().to_string();
        let email = self.email.read(cx).value().trim().to_string();
        if message.is_empty() || matches!(self.sending, Sending::Now | Sending::Sent) {
            return;
        }
        if !email.is_empty() && !(email.contains('@') && email.contains('.')) {
            self.sending = Sending::Failed("that email doesn't look like one.".into());
            return cx.notify();
        }
        self.sending = Sending::Now;
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let what = json!({ "message": message, "email": (!email.is_empty()).then_some(email) });
            let sent = cx.background_executor().spawn(async move { send("feedback", what) }).await;
            this.update_in(cx, |this, window, cx| {
                this.sending = match sent {
                    Ok(()) => {
                        close_soon(window, cx);
                        Sending::Sent
                    }
                    Err(why) => Sending::Failed(why),
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

impl Render for Feedback {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette;
        let empty = self.message.read(cx).value().trim().is_empty();
        let busy = matches!(self.sending, Sending::Now | Sending::Sent);
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_3()
            .px_6()
            .pt(px(38.))
            .pb_5()
            .bg(palette.background)
            .text_color(palette.label)
            .text_size(px(13.))
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, window, _| {
                if e.keystroke.key == "escape" && !matches!(this.sending, Sending::Now) {
                    window.remove_window();
                }
            }))
            .child(div().font_weight(FontWeight::SEMIBOLD).text_size(px(15.)).child("Send Feedback"))
            .child(div().text_size(px(12.)).text_color(palette.secondary).child(format!(
                "It goes straight to {APP_NAME}'s maker, who reads every one."
            )))
            .child(Textarea::new(&self.message).text_size(px(13.)))
            .child(Input::new(&self.email))
            .child(div().text_size(px(11.)).text_color(palette.tertiary).child(self.about.clone()))
            .child(privacy_link(&palette).text_size(px(11.)))
            .children(said(&self.sending, &palette))
            .child(div().flex_1())
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .when(!busy, |d| {
                        d.child(button("cancel", "Cancel", false, &palette).on_click(cx.listener(|_, _, window, _| window.remove_window())))
                            .child(
                                button("send", "Send", true, &palette)
                                    .when(empty, |b| b.opacity(0.5))
                                    .on_click(cx.listener(|this, _, window, cx| this.send(window, cx))),
                            )
                    }),
            )
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    /// The ids that say whose Mac it was come out, and the home folder's
    /// name in paths; where it stopped stays.
    #[test]
    fn crash_reports_redacted() {
        let report = concat!(
            r#"{"app_name":"F2","bundleID":"dev.newfinder.NewFinder","incident_id":"ABC","slice_uuid":"S1"}"#,
            "\n",
            r#"{"crashReporterKey":"KEY","userID":501,"storeInfo":{"deviceIdentifierForVendor":"DEV"},"exception":{"type":"EXC_BREAKPOINT"},"procPath":"/Users/anna/Applications/F2.app/Contents/MacOS/F2"}"#
        );
        let out = super::redacted(report).unwrap();
        let out = super::without_home(&out, Path::new("/Users/anna"));
        for gone in ["KEY", "DEV", "ABC", "501", "anna"] {
            assert!(!out.contains(gone), "{gone} left in: {out}");
        }
        for kept in ["EXC_BREAKPOINT", "S1", "~/Applications/F2.app"] {
            assert!(out.contains(kept), "{kept} missing from: {out}");
        }
        assert!(super::redacted("not a report").is_none());
    }
}
