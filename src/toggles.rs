//! Wi-Fi, Bluetooth and Do Not Disturb, turned on and off from Quick Open.
//! Wi-Fi by CoreWLAN; Bluetooth as System Settings turns it (macOS asks
//! whether F2 may, the first time); Do Not Disturb by a shortcut of F2's,
//! "F2 Do Not Disturb" (in the app: Shortcuts adds it, the first time), as
//! macOS has no other way for an app to. Each says how it went in a note.
//! (Not on the main thread: they wait on the system.)

use std::io::Write;
use std::process::{Command, Stdio};

/// The shortcut Do Not Disturb is turned by, and its file in the app.
pub const SHORTCUT: &str = "F2 Do Not Disturb";

/// A note's title, what it says, and a link it opens.
pub type Note = (String, String, Option<&'static str>);

const BLUETOOTH_PRIVACY: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_Bluetooth";

fn on_off(on: bool) -> &'static str {
    if on { "On" } else { "Off" }
}

pub fn wifi() -> Note {
    let Some(on) = crate::macos::wifi(None) else { return ("No Wi-Fi".into(), "This Mac has no Wi-Fi.".into(), None) };
    let now = crate::macos::wifi(Some(!on));
    if now == Some(on) {
        // (An administrator may have to: System Settings can ask for one.)
        return (format!("Wi-Fi Is Still {}", on_off(on)), "macOS didn't let it change: System Settings > Wi-Fi can.".into(), None);
    }
    (format!("Wi-Fi {}", on_off(!on)), String::new(), None)
}

/// "Magic Keyboard and Magic Mouse".
fn and(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// Bluetooth, on or off. Off with a keyboard or mouse connected over it
/// only when `sure` (asked first: see `commands::live`).
pub fn bluetooth(sure: bool) -> Note {
    let refused = || ("Bluetooth Is Off Limits".into(), format!("{} may not use Bluetooth: System Settings > Privacy & Security > Bluetooth can allow it.", crate::APP_NAME), Some(BLUETOOTH_PRIVACY));
    if crate::macos::bluetooth_allowed() == Some(false) {
        return refused();
    }
    // (The first time, macOS asks here: what's read before it's allowed
    // isn't so.)
    crate::macos::bluetooth(None);
    match crate::macos::bluetooth_allowed() {
        Some(false) => return refused(),
        None => return ("Allow Bluetooth".into(), format!("macOS asks whether {} may use Bluetooth: allow it, then try again.", crate::APP_NAME), None),
        Some(true) => {}
    }
    let on = crate::macos::bluetooth(None);
    if on && !sure {
        let inputs = crate::macos::bluetooth_inputs();
        if !inputs.is_empty() {
            return ("Bluetooth Is Still On".into(), format!("{} would stop working. Turn it off again to go ahead.", and(&inputs)), None);
        }
    }
    crate::macos::bluetooth(Some(!on));
    // (It takes a moment.)
    let mut now = on;
    for _ in 0..20 {
        std::thread::sleep(std::time::Duration::from_millis(150));
        now = crate::macos::bluetooth(None);
        if now != on {
            break;
        }
    }
    if now == on {
        return (format!("Bluetooth Is Still {}", on_off(on)), "macOS didn't let it change: Control Center can.".into(), None);
    }
    (format!("Bluetooth {}", on_off(now)), String::new(), None)
}

/// Run a shortcut with `input`: what it gave back, if it ran.
fn shortcut(name: &str, input: &str) -> Option<String> {
    let mut child = Command::new("/usr/bin/shortcuts").args(["run", name]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    child.stdin.take()?.write_all(input.as_bytes()).ok()?;
    let out = child.wait_with_output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Whether the shortcut's been added (asks Shortcuts: not on the main
/// thread).
pub fn have_shortcut() -> bool {
    Command::new("/usr/bin/shortcuts").arg("list").output().is_ok_and(|out| String::from_utf8_lossy(&out.stdout).lines().any(|l| l.trim() == SHORTCUT))
}

/// Hand the shortcut to Shortcuts, which asks to add it. Whether it's in
/// this copy of the app to hand.
pub fn offer_shortcut() -> bool {
    match crate::macos::resource(&format!("{SHORTCUT}.shortcut")) {
        Some(file) => Command::new("/usr/bin/open").arg(file).status().is_ok_and(|s| s.success()),
        None => false,
    }
}

/// Do Not Disturb (or the Focus that's on) off, or Do Not Disturb on; the
/// first time, Shortcuts asked to add the shortcut that does it.
pub fn do_not_disturb() -> Note {
    if !have_shortcut() {
        return match crate::macos::resource(&format!("{SHORTCUT}.shortcut")) {
            Some(file) => {
                let _ = Command::new("/usr/bin/open").arg(file).status();
                (
                    "Add the Shortcut".into(),
                    format!("macOS turns Focus on and off only through Shortcuts: add “{SHORTCUT}”, then turn Do Not Disturb on again."),
                    None,
                )
            }
            None => ("Do Not Disturb".into(), format!("“{SHORTCUT}” comes with {}.app, and this copy hasn't it.", crate::APP_NAME), None),
        };
    }
    let Some(focus) = shortcut(SHORTCUT, "status") else {
        return ("Couldn't Change Focus".into(), format!("Shortcuts couldn't run “{SHORTCUT}”."), None);
    };
    if focus.is_empty() {
        shortcut(SHORTCUT, "on");
        ("Do Not Disturb On".into(), String::new(), None)
    } else {
        shortcut(SHORTCUT, "off");
        (format!("{focus} Off"), String::new(), None)
    }
}
