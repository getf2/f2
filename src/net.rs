//! The internet and Wi-Fi, for Quick Open: "speed test" (macOS's own
//! `networkQuality`: download, upload and how responsive the line is), and
//! "wifi password": a network's password (from the System keychain: macOS
//! asks for yours), with a QR code a phone joins by. Which network the Mac's
//! on macOS tells only apps allowed Location; without it, those it's joined
//! before.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::toggles::Note;

/// A speed test's outcome: download and upload (megabits a second), and
/// responsiveness (round trips a minute: Apple's RPM).
#[derive(Clone, Copy, Debug)]
pub struct Speed {
    pub down: f64,
    pub up: f64,
    pub rpm: f64,
}

static LAST: Mutex<Option<(Instant, Speed)>> = Mutex::new(None);
static TESTING: AtomicBool = AtomicBool::new(false);

pub fn speed_asked(q: &str) -> bool {
    let lower = q.trim().trim_end_matches('?').to_lowercase();
    matches!(
        lower.as_str(),
        "speed test" | "speedtest" | "internet speed" | "internet speed test" | "wifi speed" | "wi-fi speed" | "network speed" | "test internet" | "test my internet" | "how fast is my internet" | "connection speed"
    )
}

/// The last test's, if it's from the last half hour, and how long ago.
pub fn last() -> Option<(Speed, Duration)> {
    LAST.lock().unwrap().as_ref().filter(|(when, _)| when.elapsed() < Duration::from_secs(1800)).map(|(when, speed)| (*speed, when.elapsed()))
}

pub fn testing() -> bool {
    TESTING.load(Ordering::SeqCst)
}

/// "Fast", "OK", "Slow": as a person would say it.
fn verdict(s: &Speed) -> &'static str {
    match s.down {
        d if d >= 100. => "Fast",
        d if d >= 25. => "Good",
        d if d >= 5. => "OK for most things",
        _ => "Slow",
    }
}

/// "↓ 245 Mbps  ↑ 38 Mbps".
pub fn said(s: &Speed) -> String {
    let mbps = |v: f64| if v >= 10. { format!("{:.0}", v) } else { format!("{:.1}", v) };
    format!("↓ {} Mbps  ↑ {} Mbps", mbps(s.down), mbps(s.up))
}

/// What's under it: "Fast · responsive (1,234 RPM)".
pub fn about(s: &Speed) -> String {
    let responsive = match s.rpm {
        r if r >= 800. => "very responsive",
        r if r >= 300. => "responsive",
        r if r > 0. => "slow to respond (calls may lag)",
        _ => "",
    };
    if responsive.is_empty() { verdict(s).to_string() } else { format!("{} · {responsive}", verdict(s)) }
}

/// Test the connection (some 15 to 30 seconds: not on the main thread),
/// and say how it went.
pub fn speed_test() -> Note {
    if TESTING.swap(true, Ordering::SeqCst) {
        return ("A Speed Test Is Running".into(), "It says when it's done.".into(), None);
    }
    let out = std::process::Command::new("/usr/bin/networkQuality").arg("-c").output();
    TESTING.store(false, Ordering::SeqCst);
    let json = out.ok().filter(|o| o.status.success()).and_then(|o| serde_json::from_slice::<Value>(&o.stdout).ok());
    let Some(json) = json else {
        return ("The Speed Test Didn't Finish".into(), "Is the Mac online?".into(), None);
    };
    let number = |key: &str| json[key].as_f64().unwrap_or(0.);
    let speed = Speed { down: number("dl_throughput") / 1e6, up: number("ul_throughput") / 1e6, rpm: number("responsiveness") };
    *LAST.lock().unwrap() = Some((Instant::now(), speed));
    (said(&speed), about(&speed), None)
}

pub fn wifi_asked(q: &str) -> bool {
    let lower = q.trim().trim_end_matches('?').to_lowercase().replace("wi-fi", "wifi");
    matches!(
        lower.as_str(),
        "wifi password" | "wifi passwords" | "wifi pass" | "wifi code" | "wifi qr" | "wifi qr code" | "share wifi" | "share my wifi" | "what's the wifi password" | "whats the wifi password" | "what is the wifi password" | "wifi key" | "password for wifi"
    )
}

/// Passwords looked up while F2's been running (not kept on disk), by
/// network.
static KNOWN: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

pub fn known_password(network: &str) -> Option<String> {
    KNOWN.lock().unwrap().as_ref()?.get(network).cloned()
}

/// The networks to show: the one the Mac's on (if macOS says), then those
/// joined before (at most `limit`). Waits on `networksetup`: not on the
/// main thread.
pub fn networks(limit: usize) -> (Option<String>, Vec<String>) {
    let (current, device) = crate::macos::wifi_network();
    let device = device.unwrap_or_else(|| "en0".into());
    let joined = std::process::Command::new("/usr/sbin/networksetup")
        .args(["-listpreferredwirelessnetworks", &device])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).lines().skip(1).map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect::<Vec<_>>())
        .unwrap_or_default();
    let before = joined.into_iter().filter(|n| Some(n) != current.as_ref()).take(limit).collect();
    (current, before)
}

/// Look up a network's password (macOS asks for yours: the System keychain
/// keeps them), keeping it till F2 quits. Waits for you: not on the main
/// thread.
pub fn look_up(network: &str) -> Result<String, String> {
    let out = std::process::Command::new("/usr/bin/security")
        .args(["find-generic-password", "-D", "AirPort network password", "-a", network, "-w"])
        .output()
        .map_err(|e| e.to_string())?;
    let password = String::from_utf8_lossy(&out.stdout).trim_end_matches('\n').to_string();
    if !out.status.success() || password.is_empty() {
        let why = String::from_utf8_lossy(&out.stderr);
        return Err(if why.contains("could not be found") {
            format!("The Mac doesn't keep a password for “{network}”.")
        } else {
            "It wasn't allowed: macOS asks for your Mac's password to show it.".into()
        });
    }
    KNOWN.lock().unwrap().get_or_insert_with(HashMap::new).insert(network.to_string(), password.clone());
    Ok(password)
}

/// What a phone's camera reads to join: "WIFI:T:WPA;S:Home;P:…;;".
pub fn join_code(network: &str, password: &str) -> String {
    let escape = |s: &str| s.chars().fold(String::new(), |mut out, c| {
        if matches!(c, '\\' | ';' | ',' | ':' | '"') {
            out.push('\\');
        }
        out.push(c);
        out
    });
    if password.is_empty() { format!("WIFI:T:nopass;S:{};;", escape(network)) } else { format!("WIFI:T:WPA;S:{};P:{};;", escape(network), escape(password)) }
}

#[cfg(test)]
mod tests {
    use super::{Speed, about, join_code, said, speed_asked, wifi_asked};

    #[test]
    fn asked() {
        assert!(speed_asked("speed test"));
        assert!(speed_asked("How fast is my internet?"));
        assert!(!speed_asked("speed"));
        assert!(wifi_asked("Wi-Fi password"));
        assert!(wifi_asked("share wifi"));
        assert!(!wifi_asked("wifi"));
    }

    #[test]
    fn spoken() {
        let s = Speed { down: 245.4, up: 8.26, rpm: 1200. };
        assert_eq!(said(&s), "↓ 245 Mbps  ↑ 8.3 Mbps");
        assert_eq!(about(&s), "Fast · very responsive");
    }

    #[test]
    fn codes() {
        assert_eq!(join_code("Home", "p@ss;word"), "WIFI:T:WPA;S:Home;P:p@ss\\;word;;");
        assert_eq!(join_code("Café: 2", ""), "WIFI:T:nopass;S:Café\\: 2;;");
    }
}
