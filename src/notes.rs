//! Quick notes from Quick Open: "note buy milk", "new note call the
//! plumber", "jot down …" go into Apple Notes (its default folder), through
//! its AppleScript (macOS asks once whether F2 may control Notes).

use crate::toggles::Note;

/// The note asked for: "note buy milk" (None: not a note's words; Some(""),
/// "note" alone, to say what).
pub fn asked(q: &str) -> Option<String> {
    let trimmed = q.trim();
    let lower = trimmed.to_lowercase();
    if matches!(lower.as_str(), "note" | "new note" | "add note" | "take a note" | "jot") {
        return Some(String::new());
    }
    const LEADS: &[&str] = &["new note ", "add note ", "add a note ", "take a note ", "make a note ", "jot down ", "note to self ", "note: ", "note ", "jot "];
    let lead = LEADS.iter().find(|l| lower.starts_with(*l))?;
    Some(trimmed.get(lead.len()..)?.trim().to_string()).filter(|text| !text.is_empty())
}

/// A string for AppleScript, in quotes.
fn quoted(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Notes keeps a note's body as HTML: its first line its title.
fn html(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Add it to Notes, and say how that went (waits on Notes: not on the main
/// thread).
pub fn add(text: &str) -> Note {
    let script = format!("tell application \"Notes\" to make new note with properties {{body:{}}}", quoted(&format!("<div>{}</div>", html(text))));
    let out = std::process::Command::new("/usr/bin/osascript").args(["-e", &script]).output();
    match out {
        Ok(out) if out.status.success() => ("Added to Notes".into(), text.to_string(), None),
        Ok(out) => {
            let why = String::from_utf8_lossy(&out.stderr).to_string();
            // (-1743: not allowed to control Notes.)
            if why.contains("-1743") {
                (
                    "Notes Is Off Limits".into(),
                    format!("System Settings > Privacy & Security > Automation can let {} control Notes.", crate::APP_NAME),
                    Some("x-apple.systempreferences:com.apple.preference.security?Privacy_Automation"),
                )
            } else {
                ("Couldn't Add the Note".into(), why.trim().to_string(), None)
            }
        }
        Err(e) => ("Couldn't Add the Note".into(), e.to_string(), None),
    }
}

#[cfg(test)]
mod tests {
    use super::{asked, html, quoted};

    #[test]
    fn notes() {
        assert_eq!(asked("note buy milk"), Some("buy milk".into()));
        assert_eq!(asked("New note Call the plumber"), Some("Call the plumber".into()));
        assert_eq!(asked("jot down the wifi password"), Some("the wifi password".into()));
        assert_eq!(asked("note"), Some(String::new()));
        assert_eq!(asked("notes"), None, "the app");
        assert_eq!(asked("notebook"), None);
    }

    #[test]
    fn escaped() {
        assert_eq!(quoted(r#"say "hi" \o/"#), r#""say \"hi\" \\o/""#);
        assert_eq!(html("salt & <pepper>"), "salt &amp; &lt;pepper&gt;");
    }
}
