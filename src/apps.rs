//! The apps Quick Open launches, as Raycast's launcher does: what's in the
//! Applications folders (and the system's own, Finder's among them),
//! found by the start of their name or of a word in it, their initials
//! ("vsc": Visual Studio Code), or their letters in order; by their names
//! in the languages the Mac reads or types too ("メモ", "메모": Notes),
//! hiragana for katakana ("めも") and full-width letters for plain.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

pub struct App {
    /// Its name as Finder shows it ("Visual Studio Code"; "メモ" on a Mac
    /// in Japanese).
    pub name: String,
    pub path: PathBuf,
    /// Its folder's name, lowercase ("notes"): one app, once.
    key: String,
    /// Each of its names, folded (see `fold`), with its initials ("vsc").
    names: Vec<(String, String)>,
}

/// Full-width letters, digits and spaces as plain ones ("１００ｕｓｄ" as
/// "100usd"), as a Japanese keyboard may type them (and a no-break space
/// as a space: "나의 찾기", Find My, has one).
pub fn plain_width(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '！'..='～' => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
            '\u{3000}' | '\u{a0}' => ' ',
            c => c,
        })
        .collect()
}

/// For matching: lowercase, katakana as hiragana ("メモ" as "めも"), and
/// full-width letters and digits as plain ("ｍｅｍｏ" as "memo"), as a
/// Japanese keyboard types them.
pub fn fold(text: &str) -> String {
    plain_width(text).chars().map(|c| if ('ァ'..='ヶ').contains(&c) { char::from_u32(c as u32 - 0x60).unwrap_or(c) } else { c }).flat_map(char::to_lowercase).collect()
}

fn initials(lower: &str) -> String {
    lower.split(|c: char| !c.is_alphanumeric()).filter_map(|w| w.chars().next()).collect()
}

/// Where apps are, and how deep to look in each (Applications holds
/// folders of apps: "Utilities", "Microsoft Office"...).
fn places() -> Vec<(PathBuf, usize)> {
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    vec![
        ("/Applications".into(), 2),
        ("/System/Applications".into(), 2),
        (home.join("Applications"), 2),
        ("/System/Library/CoreServices/Applications".into(), 1),
        ("/System/Library/CoreServices/Finder.app".into(), 0),
    ]
}

fn scan() -> Vec<App> {
    let languages = crate::macos::mac_languages();
    let mut apps = Vec::new();
    for (place, depth) in places() {
        collect(&place, depth, &languages, &mut apps);
    }
    apps.sort_by(|a, b| a.key.cmp(&b.key));
    // (The same app twice, in two places: the first.)
    apps.dedup_by(|a, b| a.key == b.key);
    apps
}

/// An app's names (see `macos::app_names`), read again only once it's
/// changed (or the languages have).
fn names_of(path: &Path, languages: &[String]) -> Vec<String> {
    type Known = HashMap<PathBuf, (Option<SystemTime>, Vec<String>, Vec<String>)>;
    static KNOWN: Mutex<Option<Known>> = Mutex::new(None);
    let changed = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    let mut known = KNOWN.lock().unwrap();
    let known = known.get_or_insert_with(HashMap::new);
    if let Some((at, langs, names)) = known.get(path)
        && *at == changed
        && langs == languages
    {
        return names.clone();
    }
    let names = crate::macos::app_names(path, languages);
    known.insert(path.to_path_buf(), (changed, languages.to_vec(), names.clone()));
    names
}

fn collect(path: &Path, depth: usize, languages: &[String], apps: &mut Vec<App>) {
    if path.extension().is_some_and(|e| e == "app") {
        let stem = path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
        let mut all = names_of(path, languages);
        let name = all.first().cloned().unwrap_or_else(|| stem.clone());
        all.push(stem.clone());
        let mut names: Vec<(String, String)> = Vec::new();
        for n in all {
            let folded = fold(&n);
            if !names.iter().any(|(f, _)| *f == folded) {
                let initials = initials(&folded);
                names.push((folded, initials));
            }
        }
        apps.push(App { name, path: path.to_path_buf(), key: stem.to_lowercase(), names });
        return;
    }
    if depth == 0 {
        return;
    }
    for entry in std::fs::read_dir(path).into_iter().flatten().flatten() {
        match entry.file_type() {
            Ok(t) if t.is_dir() => collect(&entry.path(), depth - 1, languages, apps),
            // (A link, where it leads: Safari's, in /Applications, into the
            // system's Cryptex, where it runs from. Its icon then isn't a
            // link's.)
            Ok(t) if t.is_symlink() => {
                if let Ok(real) = entry.path().canonicalize()
                    && real.is_dir()
                {
                    collect(&real, depth - 1, languages, apps);
                }
            }
            _ => {}
        }
    }
}

/// What Finder calls the app at `path` ("メモ"), if it's one Quick Open
/// knows (else None: its file's name will do).
pub fn name_of(path: &Path) -> Option<String> {
    installed().iter().find(|a| a.path == path).map(|a| a.name.clone())
}

/// The apps, looked for again at most every half minute (new installs).
pub fn installed() -> Arc<Vec<App>> {
    static FOUND: Mutex<Option<(Instant, Arc<Vec<App>>)>> = Mutex::new(None);
    let mut found = FOUND.lock().unwrap();
    match &*found {
        Some((at, apps)) if at.elapsed() < Duration::from_secs(30) => apps.clone(),
        _ => {
            let apps = Arc::new(scan());
            *found = Some((Instant::now(), apps.clone()));
            apps
        }
    }
}

/// How well `query` (lowercase) matches `name` (an app's, a command's),
/// if it does at all.
pub fn name_score(name: &str, query: &str) -> Option<f32> {
    let folded = fold(name);
    scored(&folded, &initials(&folded), &fold(query))
}

/// Its best name's score.
fn score(app: &App, query: &str) -> Option<f32> {
    app.names.iter().filter_map(|(name, initials)| scored(name, initials, query)).fold(None, |best: Option<f32>, s| Some(best.map_or(s, |b| b.max(s))))
}

fn scored(name: &str, initials: &str, query: &str) -> Option<f32> {
    if name == query {
        return Some(100.);
    }
    if name.starts_with(query) {
        return Some(80.);
    }
    if name.split(|c: char| !c.is_alphanumeric()).any(|w| w.starts_with(query)) {
        return Some(60.);
    }
    if initials.starts_with(query) && query.len() > 1 {
        return Some(55.);
    }
    if name.contains(query) {
        return Some(40.);
    }
    // Its letters in order ("sfri": Safari), from the start of a word and
    // close together (not "calc" in World of Warcraft Launcher).
    let mut chars = name.char_indices();
    let mut spread = 0;
    let mut last: Option<usize> = None;
    for q in query.chars() {
        let (at, _) = chars.find(|&(_, c)| c == q)?;
        match last {
            None if at > 0 && name[..at].chars().last().is_some_and(char::is_alphanumeric) => return None,
            None => {}
            Some(l) => spread += at - l - 1,
        }
        last = Some(at);
    }
    (spread <= query.len()).then_some(20. - spread as f32)
}

/// The apps matching `query`, best first, with how well their best name
/// matches; `lately` says how much each was used of late (fsearch's
/// frecency), which counts too.
pub fn find(query: &str, limit: usize, lately: impl Fn(&Path) -> f32) -> Vec<(f32, String, PathBuf)> {
    let query = fold(query.trim());
    if query.is_empty() {
        return Vec::new();
    }
    let apps = installed();
    let mut found: Vec<(f32, f32, &App)> =
        apps.iter().filter_map(|app| score(app, &query).map(|s| (s + lately(&app.path).min(10.) * 3., s, app))).collect();
    found.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.2.key.cmp(&b.2.key)));
    found.into_iter().take(limit).map(|(_, s, app)| (s, app.name.clone(), app.path.clone())).collect()
}

#[cfg(test)]
mod tests {
    /// How long looking for the apps takes, first and again (read only).
    /// `rtk proxy cargo test --release -- --ignored --nocapture scan_time`
    #[test]
    #[ignore]
    fn scan_time() {
        for round in 0..3 {
            let t = std::time::Instant::now();
            let n = super::scan().len();
            println!("scan {round}: {n} apps in {:?}", t.elapsed());
        }
    }

    use super::{App, fold, initials, score};

    fn app(name: &str) -> App {
        app_in(name, &[])
    }

    /// An app called `name`, and also `others`.
    fn app_in(name: &str, others: &[&str]) -> App {
        let names = std::iter::once(name).chain(others.iter().copied()).map(|n| (fold(n), initials(&fold(n)))).collect();
        App { name: name.into(), path: Default::default(), key: name.to_lowercase(), names }
    }

    #[test]
    fn other_languages() {
        let notes = app_in("Notes", &["メモ", "메모"]);
        assert_eq!(score(&notes, &fold("メモ")), Some(100.));
        assert_eq!(score(&notes, &fold("めも")), Some(100.), "hiragana for katakana");
        assert_eq!(score(&notes, &fold("메")), Some(80.));
        assert_eq!(score(&notes, &fold("ｎｏｔ")), Some(80.), "full-width letters");
        assert_eq!(score(&notes, "not"), Some(80.));
        let settings = app_in("System Settings", &["システム設定"]);
        assert!(score(&settings, &fold("設定")).is_some());
    }

    #[test]
    fn names_here() {
        // (As the Mac has them: Notes is メモ in Japanese, whatever the Mac's in.)
        let names = crate::macos::app_names(std::path::Path::new("/System/Applications/Notes.app"), &["ja".into(), "ko".into()]);
        assert!(names.contains(&"メモ".to_string()) && names.contains(&"메모".to_string()), "{names:?}");
    }

    #[test]
    fn matches() {
        let vsc = app("Visual Studio Code");
        assert!(score(&vsc, "vis").unwrap() > score(&vsc, "stu").unwrap());
        assert!(score(&vsc, "vsc").is_some());
        assert!(score(&vsc, "code").is_some());
        assert!(score(&app("Safari"), "sfri").is_some());
        assert!(score(&app("Safari"), "xyz").is_none());
        assert!(score(&app("World of Warcraft Launcher"), "calc").is_none());
        // Spread too far apart isn't a match.
        assert!(score(&app("System Settings"), "sg").is_none() || score(&app("System Settings"), "sg").unwrap() < 20.);
        assert_eq!(score(&app("Zed"), "zed"), Some(100.));
    }
}
