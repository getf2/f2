//! QR codes. "qr <link or text>" in Quick Open shows one, to scan with a
//! phone (the easy way to take a link to it); "qr" alone, of what was last
//! copied. Scan QR Code (a screenshot action: see `shots`) reads one off the
//! screen.

use std::hash::{Hash, Hasher};
use std::path::PathBuf;

/// What a QR code's asked of: Some(None), what was last copied.
pub fn asked(q: &str) -> Option<Option<String>> {
    let trimmed = q.trim();
    let lower = trimmed.to_lowercase();
    if matches!(lower.as_str(), "qr" | "qr code" | "qrcode") {
        return Some(None);
    }
    let prefix = ["qr code for ", "qr code of ", "qr code ", "qrcode ", "qr for ", "qr of ", "qr "].iter().find(|p| lower.starts_with(*p))?;
    let text = trimmed.get(prefix.len()..)?.trim();
    (!text.is_empty()).then(|| Some(text.to_string()))
}

/// What was last copied, as text (from the clipboard's history: so not
/// asked of macOS), if it's on.
pub fn last_copied() -> Option<String> {
    crate::clips::list().into_iter().find_map(|clip| match clip.what {
        crate::clips::What::Text(text) | crate::clips::What::Link(text) => Some(text),
        _ => None,
    })
}

/// A QR code of `text`, as a PNG (made once, then kept: under the data
/// folder's QR).
pub fn image(text: &str) -> Option<PathBuf> {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    let dir = crate::settings::data_dir().join("QR");
    std::fs::create_dir_all(&dir).ok()?;
    let file = dir.join(format!("{:016x}.png", hasher.finish()));
    (file.exists() || crate::macos::qr_png(text, &file, 512)).then_some(file)
}

#[cfg(test)]
mod tests {
    use super::asked;

    #[test]
    fn asked_of() {
        assert_eq!(asked("qr https://f2.dagurleo.com"), Some(Some("https://f2.dagurleo.com".into())));
        assert_eq!(asked("QR code for Hello There"), Some(Some("Hello There".into())));
        assert_eq!(asked("qr"), Some(None));
        assert_eq!(asked("qrious"), None);
    }

    #[test]
    fn made_and_read() {
        let dir = std::env::temp_dir().join(format!("f2-qr-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("code.png");
        assert!(crate::macos::qr_png("https://example.com/a?b=c", &file, 512));
        assert_eq!(crate::macos::read_qr(&file).as_deref(), Some("https://example.com/a?b=c"), "it reads back");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
