//! What's plugged in and paired, for Quick Open: "battery" (the Mac's, and
//! the AirPods', mouse's and keyboard's), where the sound goes ("airpods",
//! "speakers", "sound output"), and Bluetooth devices connected and
//! disconnected ("connect airpods"). Paired devices and their batteries are
//! as System Information has them (`system_profiler`: no permission
//! needed to look); connecting is IOBluetooth's (macOS asks the first time).

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::macos;
use crate::toggles::Note;

/// A Bluetooth device paired with the Mac.
#[derive(Clone, Debug)]
pub struct Paired {
    pub name: String,
    pub address: String,
    pub connected: bool,
    /// "Headphones", "Mouse", "Keyboard", "Magic Trackpad", "Speaker"…
    pub kind: String,
    /// Its batteries ("Left", "Right", "Case", or "" for its one), in percent.
    pub batteries: Vec<(String, u8)>,
}

impl Paired {
    /// Its symbol.
    pub fn symbol(&self) -> &'static str {
        let kind = self.kind.to_lowercase();
        let name = self.name.to_lowercase();
        if name.contains("airpods max") {
            "airpodsmax"
        } else if name.contains("airpods") || (kind == "headphones" && self.batteries.iter().any(|(b, _)| b == "Case")) {
            "airpodspro"
        } else if kind.contains("headphones") || kind.contains("headset") {
            "headphones"
        } else if kind.contains("mouse") {
            "computermouse.fill"
        } else if kind.contains("trackpad") {
            "hand.point.up.left.fill"
        } else if kind.contains("keyboard") {
            "keyboard.fill"
        } else if kind.contains("speaker") {
            "hifispeaker.fill"
        } else if kind.contains("gamepad") {
            "gamecontroller.fill"
        } else {
            "dot.radiowaves.left.and.right"
        }
    }

    /// The words it's also found by: "airpods" for any earbuds with a case
    /// (yours may be called "Leopods").
    pub fn words(&self) -> String {
        let kind = self.kind.to_lowercase();
        let mut words = format!("{kind} bluetooth");
        if self.symbol().starts_with("airpods") {
            words.push_str(" airpods headphones earbuds");
        } else if kind.contains("headphones") {
            words.push_str(" headphones earbuds");
        }
        words
    }

    /// "Left 73% · Right 74% · Case 23%", or "80%".
    pub fn charge(&self) -> String {
        self.batteries.iter().map(|(which, p)| if which.is_empty() { format!("{p}%") } else { format!("{which} {p}%") }).collect::<Vec<_>>().join(" · ")
    }
}

/// How long the paired devices are kept, as looked up (`system_profiler`
/// takes a fifth of a second: not each keystroke).
const KEPT: Duration = Duration::from_secs(10);

static PAIRED: Mutex<Option<(Instant, Vec<Paired>)>> = Mutex::new(None);

/// The paired devices, connected first (waits on `system_profiler`: not on
/// the main thread).
pub fn paired() -> Vec<Paired> {
    if let Some((when, kept)) = PAIRED.lock().unwrap().as_ref()
        && when.elapsed() < KEPT
    {
        return kept.clone();
    }
    let found = std::process::Command::new("/usr/sbin/system_profiler")
        .args(["SPBluetoothDataType", "-json"])
        .output()
        .ok()
        .and_then(|out| serde_json::from_slice::<Value>(&out.stdout).ok())
        .map(|json| read_paired(&json))
        .unwrap_or_default();
    *PAIRED.lock().unwrap() = Some((Instant::now(), found.clone()));
    found
}

/// Forget the paired devices as looked up (one's been connected).
fn forget() {
    *PAIRED.lock().unwrap() = None;
}

fn read_paired(json: &Value) -> Vec<Paired> {
    let Some(bluetooth) = json["SPBluetoothDataType"].get(0) else { return Vec::new() };
    let percent = |v: &Value| v.as_str().and_then(|s| s.trim_end_matches('%').trim().parse::<u8>().ok());
    let mut all = Vec::new();
    for (group, connected) in [("device_connected", true), ("device_not_connected", false)] {
        for device in bluetooth[group].as_array().into_iter().flatten() {
            let Some(object) = device.as_object() else { continue };
            for (name, info) in object {
                let Some(address) = info["device_address"].as_str() else { continue };
                let batteries = [("", "device_batteryLevelMain"), ("Left", "device_batteryLevelLeft"), ("Right", "device_batteryLevelRight"), ("Case", "device_batteryLevelCase")]
                    .into_iter()
                    .filter_map(|(which, key)| percent(&info[key]).map(|p| (which.to_string(), p)))
                    .collect();
                all.push(Paired { name: name.clone(), address: address.to_string(), connected, kind: info["device_minorType"].as_str().unwrap_or("").to_string(), batteries });
            }
        }
    }
    all
}

/// Whether the battery's asked about: "battery", "airpods battery", "how
/// much battery".
pub fn battery_asked(q: &str) -> bool {
    let lower = q.trim().trim_end_matches('?').to_lowercase();
    matches!(lower.as_str(), "battery" | "batteries" | "battery level" | "battery life" | "how much battery" | "how much battery do i have" | "battery status")
        || (lower.ends_with(" battery") && lower.split_whitespace().count() <= 4)
}

/// The Mac's battery, said: "87%", "Charging · full in 1 h 20 min".
pub fn mac_battery() -> Option<(String, String, &'static str)> {
    let b = macos::battery()?;
    let time = |m: i32| if m >= 60 { format!("{} h {} min", m / 60, m % 60) } else { format!("{m} min") };
    let about = match (b.charging, b.plugged, b.minutes) {
        (true, _, Some(m)) if m > 0 => format!("Charging · full in {}", time(m)),
        (true, _, _) => "Charging".into(),
        (false, true, _) if b.percent >= 95 => "Fully charged".into(),
        (false, true, _) => "On power, not charging".into(),
        (false, false, Some(m)) if m > 0 => format!("On battery · {} left", time(m)),
        (false, false, _) => "On battery".into(),
    };
    let symbol = match (b.charging || b.plugged, b.percent) {
        (true, _) => "battery.100percent.bolt",
        (_, 88..) => "battery.100percent",
        (_, 63..) => "battery.75percent",
        (_, 38..) => "battery.50percent",
        (_, 13..) => "battery.25percent",
        _ => "battery.0percent",
    };
    Some((format!("{}%", b.percent), about, symbol))
}

/// Sound outputs asked for, with their symbols and what they are ("Built-
/// in", "Bluetooth"), the one in use first: all of them for "sound output"
/// or "speakers"; earbuds and headphones for "airpods" (yours may be called
/// "Leopods"); one by its name ("leopods", "play sound on acton").
pub fn outputs_for(q: &str) -> Vec<(macos::Output, &'static str, &'static str)> {
    let lower = q.trim().to_lowercase();
    let said = ["play sound on ", "play sound through ", "play through ", "switch sound to ", "sound on ", "output to ", "audio to "].iter().find_map(|p| lower.strip_prefix(p));
    let wanted = said.unwrap_or(&lower).trim();
    let all = said.is_none()
        && matches!(wanted, "sound output" | "audio output" | "output" | "outputs" | "sound outputs" | "audio outputs" | "speaker" | "speakers" | "switch output" | "change output" | "sound device");
    let earbuds = matches!(wanted, "airpods" | "airpods pro" | "airpods max" | "headphones" | "earbuds" | "earphones" | "buds");
    // (Else only an output's own name, four letters of it at least: "mac"
    // isn't asking for the MacBook's speakers.)
    if !(all || earbuds || said.is_some() || wanted.chars().count() >= 4) {
        return Vec::new();
    }
    let outputs = macos::audio_outputs();
    let paired = if earbuds && outputs.iter().any(|o| o.kind == "bluetooth") { paired() } else { Vec::new() };
    let mut found: Vec<(macos::Output, &'static str, &'static str)> = outputs
        .into_iter()
        .filter_map(|o| {
            let device = paired.iter().find(|p| p.name == o.name);
            let (symbol, what) = match o.kind.as_str() {
                "builtin" => ("speaker.wave.2.fill", "Built-in"),
                "bluetooth" => (device.map_or("headphones", Paired::symbol), "Bluetooth"),
                "airplay" => ("airplayaudio", "AirPlay"),
                "display" => ("display", "Display"),
                "usb" => ("cable.connector", "USB"),
                _ => ("speaker.wave.2.fill", "Output"),
            };
            let named = crate::apps::name_score(&o.name, wanted).is_some_and(|s| s >= if said.is_some() { 60. } else { 80. });
            let wanted_here = if all {
                true
            } else if earbuds {
                device.is_some_and(|d| d.words().contains("headphones")) || o.name.to_lowercase().contains(wanted.split(' ').next().unwrap_or(wanted))
            } else {
                // (Not the Mac's own for its name's start: "macbook".)
                named && (said.is_some() || o.kind != "builtin")
            };
            wanted_here.then_some((o, symbol, what))
        })
        .collect();
    found.sort_by_key(|(o, ..)| !o.used);
    found
}

/// Paired devices to connect or disconnect, for "connect airpods",
/// "disconnect mouse", "connect" (all). (Connecting: those not connected.)
pub fn to_connect(q: &str) -> Option<(bool, Vec<Paired>)> {
    let lower = q.trim().to_lowercase();
    let (connect, rest) = if let Some(rest) = lower.strip_prefix("disconnect") {
        (false, rest)
    } else if let Some(rest) = lower.strip_prefix("connect") {
        (true, rest)
    } else {
        return None;
    };
    // ("connect to my airpods".)
    let rest = rest.trim();
    let rest = rest.strip_prefix("to ").unwrap_or(rest).trim();
    let rest = rest.strip_prefix("my ").unwrap_or(rest).trim();
    if !(rest.is_empty() || lower.starts_with("connect ") || lower.starts_with("disconnect ")) {
        // ("connections", "connectivity".)
        return None;
    }
    // (Accessories only, headphones and speakers first: not a phone, an iPad
    // or another Mac, which System Information gives no kind.)
    let mut devices: Vec<Paired> = paired()
        .into_iter()
        .filter(|d| d.connected != connect && !d.kind.is_empty())
        .filter(|d| rest.is_empty() || crate::commands::matches(&d.name, &d.words(), rest).is_some_and(|s| s >= 60.))
        .collect();
    devices.sort_by_key(|d| {
        let kind = d.kind.to_lowercase();
        if kind.contains("headphones") || kind.contains("headset") { 0 } else if kind.contains("speaker") { 1 } else { 2 }
    });
    Some((connect, devices))
}

/// Connect to a device (or disconnect it), and say how it went (waits for
/// it: not on the main thread).
pub fn connect(address: &str, name: &str, connect: bool) -> Note {
    const PRIVACY: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_Bluetooth";
    match macos::bluetooth_allowed() {
        Some(false) => {
            return ("Bluetooth Is Off Limits".into(), format!("System Settings > Privacy & Security > Bluetooth can allow {}.", crate::APP_NAME), Some(PRIVACY));
        }
        None => {
            // (Touching it is what makes macOS ask.)
            macos::bluetooth_devices();
            return ("Allow Bluetooth".into(), format!("Once {} may use Bluetooth, ask again.", crate::APP_NAME), None);
        }
        Some(true) => {}
    }
    if !macos::bluetooth(None) {
        return ("Bluetooth Is Off".into(), "Turn it on first: “turn bluetooth on”.".into(), None);
    }
    macos::bluetooth_connect(address, connect);
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(if connect { 12 } else { 4 }) {
        std::thread::sleep(Duration::from_millis(400));
        if macos::bluetooth_connected(address) == connect {
            forget();
            return (format!("{} {name}", if connect { "Connected to" } else { "Disconnected" }), String::new(), None);
        }
    }
    forget();
    if connect {
        (format!("Couldn't Connect to {name}"), "Is it on, nearby, and not playing for another device?".into(), None)
    } else {
        (format!("Couldn't Disconnect {name}"), String::new(), None)
    }
}

#[cfg(test)]
mod tests {
    use super::{battery_asked, read_paired};

    #[test]
    fn asked() {
        assert!(battery_asked("battery"));
        assert!(battery_asked("AirPods battery"));
        assert!(battery_asked("how much battery?"));
        assert!(!battery_asked("battery chargers for sale near me"));
        assert!(!battery_asked("batteryless"));
    }

    #[test]
    fn paired_read() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{"SPBluetoothDataType":[{"device_connected":[{"Leopods":{"device_address":"AA:BB","device_batteryLevelLeft":"73%","device_batteryLevelRight":"74%","device_batteryLevelCase":"23%","device_minorType":"Headphones"}}],
            "device_not_connected":[{"Rat":{"device_address":"CC:DD","device_minorType":"Mouse"}},{"No Address":{}}]}]}"#,
        )
        .unwrap();
        let paired = read_paired(&json);
        assert_eq!(paired.len(), 2);
        assert_eq!((paired[0].name.as_str(), paired[0].connected, paired[0].charge().as_str()), ("Leopods", true, "Left 73% · Right 74% · Case 23%"));
        assert_eq!(paired[0].symbol(), "airpodspro", "earbuds with a case");
        assert!(paired[0].words().contains("airpods"));
        assert_eq!((paired[1].symbol(), paired[1].connected), ("computermouse.fill", false));
    }
}
