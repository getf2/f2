//! "latest download" in Quick Open: what came into Downloads last, newest
//! first (by when it came in, as Finder's Date Added), to open or show.

use std::path::PathBuf;

/// Whether the latest downloads are asked for.
pub fn asked(q: &str) -> bool {
    let lower = q.trim().to_lowercase();
    matches!(
        lower.as_str(),
        "latest download" | "latest downloads" | "last download" | "last downloads" | "newest download" | "recent downloads" | "recent download" | "last downloaded" | "my downloads" | "new downloads"
    )
}

/// The newest `n` in Downloads (not what's still coming in, nor hidden).
pub fn latest(n: usize) -> Vec<PathBuf> {
    let Some(home) = std::env::var_os("HOME") else { return Vec::new() };
    let folder = PathBuf::from(home).join("Downloads");
    let mut found: Vec<(i64, PathBuf)> = std::fs::read_dir(&folder)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            !name.starts_with('.') && !name.ends_with(".download") && !name.ends_with(".crdownload") && !name.ends_with(".part")
        })
        .map(|p| (crate::listing::added_time(&p).unwrap_or(0), p))
        .collect();
    found.sort_by(|a, b| b.0.cmp(&a.0));
    found.into_iter().take(n).map(|(_, p)| p).collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn asked() {
        assert!(super::asked("Latest download"));
        assert!(super::asked("recent downloads"));
        assert!(!super::asked("downloads"), "the folder");
    }
}
