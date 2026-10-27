//! Folder Rules: a folder kept in order on its own, as Hazel does. A rule is
//! a folder, what in it it's for (kinds, a word in the name, a size, how
//! long it's been there or gone unopened) and what's done with what fits:
//! moved to a folder, sorted into Tidy Up's folders beside it, or put in the
//! Trash. Only what's loose in the folder itself is looked at (files, not
//! folders or what's in them), once it's done arriving. Nothing's replaced
//! (a name taken is numbered) and nothing's deleted: the Trash at most, and
//! only after some days. What each rule did is kept (`Log`), to undo from
//! Settings › Folder Rules; what's put back that way is left alone after.

use std::collections::{HashMap, HashSet};
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use gpui::{App, Global};
use serde::{Deserialize, Serialize};

use crate::settings::Settings;
use crate::tidy;

/// What's done with what fits a rule.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Then {
    /// Into this folder (made if it isn't there).
    Move(PathBuf),
    /// Into folders of their kinds beside them, as Tidy Up does.
    Sort,
    Trash,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rule {
    pub on: bool,
    pub folder: PathBuf,
    /// Tidy Up's kinds it's for (none: any file).
    pub kinds: Vec<String>,
    /// A word in the name, any case (empty: any name).
    pub named: String,
    /// At least this big, in MB.
    pub larger_mb: Option<u32>,
    /// In the folder this many days or more.
    pub added_days: Option<u32>,
    /// Not opened for this many days (and in the folder as long).
    pub unopened_days: Option<u32>,
    pub then: Then,
}

impl Default for Rule {
    fn default() -> Rule {
        Rule {
            on: true,
            folder: home().join("Downloads"),
            kinds: Vec::new(),
            named: String::new(),
            larger_mb: None,
            added_days: None,
            unopened_days: None,
            then: Then::Sort,
        }
    }
}

/// How long a file's left after it last changed or arrived, so that what's
/// still being downloaded or copied in is done first.
const SETTLE: i64 = 5;
const DAY: i64 = 86_400;

pub fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map_or_else(|| PathBuf::from("/"), PathBuf::from)
}

/// "a week", "3 days".
pub fn days(n: u32) -> String {
    match n {
        1 => "a day".into(),
        7 => "a week".into(),
        14 => "two weeks".into(),
        30 | 31 => "a month".into(),
        365 => "a year".into(),
        n => format!("{n} days"),
    }
}

/// A folder as it's called: "Downloads", "the Trash".
pub fn folder_name(path: &Path) -> String {
    path.file_name().map_or_else(|| path.to_string_lossy().into_owned(), |n| n.to_string_lossy().into_owned())
}

/// The starters offered in Settings: (what it's called, the rule).
pub fn starters() -> Vec<(&'static str, Rule)> {
    let home = home();
    let downloads = home.join("Downloads");
    vec![
        (
            "Sort Downloads into folders after a day",
            Rule { folder: downloads.clone(), added_days: Some(1), then: Then::Sort, ..Rule::default() },
        ),
        (
            "Installers from Downloads to the Trash after a week",
            Rule { folder: downloads.clone(), kinds: vec!["Installers".into()], added_days: Some(7), then: Then::Trash, ..Rule::default() },
        ),
        (
            "Invoices from Downloads into Documents",
            Rule {
                folder: downloads.clone(),
                kinds: vec!["Documents".into()],
                named: "invoice".into(),
                then: Then::Move(home.join("Documents/Invoices")),
                ..Rule::default()
            },
        ),
        (
            "Old screenshots from the Desktop into Pictures",
            Rule {
                folder: home.join("Desktop"),
                kinds: vec![tidy::SCREENSHOTS.into()],
                added_days: Some(14),
                then: Then::Move(home.join("Pictures/Screenshots")),
                ..Rule::default()
            },
        ),
    ]
}

impl Rule {
    /// What's wrong with it, if it can't be kept as it is.
    pub fn problem(&self) -> Option<&'static str> {
        if !self.folder.is_dir() {
            return Some("That folder isn't there.");
        }
        match &self.then {
            Then::Trash if self.added_days.unwrap_or(0) == 0 && self.unopened_days.unwrap_or(0) == 0 => {
                Some("To put things in the Trash, a rule needs some days, so nothing goes the moment it arrives.")
            }
            Then::Move(to) if to.as_os_str().is_empty() => Some("Choose the folder to move them to."),
            Then::Move(to) if *to == self.folder => Some("That's the folder the rule is for: choose another to move them to."),
            _ => None,
        }
    }

    /// In words, without its folder: "Installers, in it a week → Trash".
    pub fn words(&self) -> String {
        let mut what = match self.kinds.as_slice() {
            [] => "Files".to_string(),
            [one] => one.clone(),
            [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
        };
        let named = self.named.trim();
        if !named.is_empty() {
            what = format!("{what} named “{named}”");
        }
        let mut parts = vec![what];
        if let Some(mb) = self.larger_mb {
            parts.push(format!("over {mb} MB"));
        }
        if let Some(d) = self.added_days.filter(|&d| d > 0) {
            parts.push(format!("added over {} ago", days(d)));
        }
        if let Some(d) = self.unopened_days.filter(|&d| d > 0) {
            parts.push(format!("not opened in {}", days(d)));
        }
        let then = match &self.then {
            Then::Move(to) => folder_name(to),
            Then::Sort => "folders by kind".into(),
            Then::Trash => crate::macos::bin().into(),
        };
        format!("{} → {then}", parts.join(", "))
    }

    /// Whether `path`, loose in the folder, fits at `now`. None: it might,
    /// but it's still arriving (looked at again soon).
    fn fits(&self, path: &Path, meta: &std::fs::Metadata, now: i64) -> Option<bool> {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let partial = path.extension().is_some_and(|e| tidy::PARTIAL.contains(&e.to_string_lossy().to_lowercase().as_str()));
        if !meta.is_file() || name.starts_with('.') || name == "Icon\r" || partial {
            return Some(false);
        }
        if !self.kinds.is_empty() && !tidy::kind_of(path).is_some_and(|k| self.kinds.iter().any(|x| x == k)) {
            return Some(false);
        }
        let named = self.named.trim().to_lowercase();
        if !named.is_empty() && !name.to_lowercase().contains(&named) {
            return Some(false);
        }
        if self.larger_mb.is_some_and(|mb| meta.len() < mb as u64 * 1_000_000) {
            return Some(false);
        }
        // (When it was put here: Finder's Date Added, else when it last
        // changed place.)
        let added = crate::listing::added_time(path).unwrap_or(meta.ctime());
        if now - added.max(meta.mtime()) < SETTLE {
            return None;
        }
        let days = self.added_days.unwrap_or(0).max(self.unopened_days.unwrap_or(0)) as i64;
        if now - added < days * DAY {
            return Some(false);
        }
        if let Some(d) = self.unopened_days.filter(|&d| d > 0)
            && crate::macos::last_opened(path).is_some_and(|opened| now - opened < d as i64 * DAY)
        {
            return Some(false);
        }
        Some(true)
    }

    /// What it would do at `now`: each file that fits and where it'd go
    /// (None: the Trash), leaving out what's `spared`; and whether any
    /// was still arriving.
    pub fn plan(&self, now: i64, spared: &HashSet<PathBuf>) -> (Vec<(PathBuf, Option<PathBuf>)>, bool) {
        let mut files: Vec<PathBuf> = std::fs::read_dir(&self.folder).into_iter().flatten().flatten().map(|e| e.path()).collect();
        files.sort();
        let mut plan = Vec::new();
        let mut taken = HashSet::new();
        let mut arriving = false;
        for path in files {
            let Ok(meta) = path.symlink_metadata() else { continue };
            if spared.contains(&path) {
                continue;
            }
            match self.fits(&path, &meta, now) {
                None => arriving = true,
                Some(false) => {}
                Some(true) => {
                    let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
                    let to = match &self.then {
                        Then::Trash => None,
                        Then::Move(folder) => Some(tidy::free_name(folder, &name, &taken)),
                        Then::Sort => match tidy::kind_of(&path) {
                            Some(kind) => Some(tidy::free_name(&self.folder.join(kind), &name, &taken)),
                            None => continue,
                        },
                    };
                    if let Some(to) = &to {
                        taken.insert(to.clone());
                    }
                    plan.push((path, to));
                }
            }
        }
        (plan, arriving)
    }

    /// Do it: what moved where (the Trash too) and the folders made for
    /// it; on the first thing that couldn't be done, that, and the rest
    /// left for next time. And whether any was still arriving.
    pub fn run(&self, now: i64, spared: &HashSet<PathBuf>) -> (Done, bool) {
        let (plan, arriving) = self.plan(now, spared);
        let mut done = Done::default();
        for (from, to) in plan {
            let result = match to {
                Some(to) => made(to.parent().unwrap_or(&self.folder)).and_then(|made| {
                    done.created.extend(made);
                    crate::fileops::move_item(&from, &to).map(|()| to)
                }),
                None => crate::macos::trash(&from).map_err(io::Error::other),
            };
            match result {
                Ok(to) => done.moved.push((from, to)),
                Err(e) => {
                    let name = from.file_name().unwrap_or_default().to_string_lossy().into_owned();
                    done.error = Some(format!("Couldn't move “{name}”: {e}"));
                    break;
                }
            }
        }
        (done, arriving)
    }
}

/// `folder` made if it isn't there: the outermost folder made, if one was.
fn made(folder: &Path) -> io::Result<Option<PathBuf>> {
    if folder.is_dir() {
        return Ok(None);
    }
    let outermost = folder.ancestors().take_while(|a| a.symlink_metadata().is_err()).last().map(Path::to_path_buf);
    std::fs::create_dir_all(folder)?;
    Ok(outermost)
}

#[derive(Default, Debug)]
pub struct Done {
    pub moved: Vec<(PathBuf, PathBuf)>,
    pub created: Vec<PathBuf>,
    pub error: Option<String>,
}

/// What a rule did, once.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub at: i64,
    pub folder: PathBuf,
    /// The rule, in words, as it was then.
    pub rule: String,
    pub moved: Vec<(PathBuf, PathBuf)>,
    pub created: Vec<PathBuf>,
    pub undone: bool,
}

impl Entry {
    /// "Moved 3 files from Downloads to the Trash".
    pub fn words(&self) -> String {
        let n = crate::listing::count(self.moved.len(), "file");
        let to: HashSet<&Path> = self.moved.iter().filter_map(|(_, to)| to.parent()).collect();
        let into = if self.moved.iter().all(|(_, to)| crate::finder::is_trashed(to)) {
            format!("to the {}", crate::macos::bin())
        } else if to.len() == 1 {
            format!("into {}", folder_name(to.into_iter().next().unwrap()))
        } else {
            format!("into {} folders", to.len())
        };
        format!("Moved {n} from {} {into}", folder_name(&self.folder))
    }
}

/// What the rules did lately (kept between launches), and what Undo put
/// back, to be left alone.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Log {
    pub entries: Vec<Entry>,
    pub spared: Vec<PathBuf>,
    /// Why a rule couldn't do something, by the rule (not kept).
    #[serde(skip)]
    pub trouble: HashMap<String, String>,
}

impl Global for Log {}

/// How many of what was done are kept.
const KEPT: usize = 100;

fn log_file() -> PathBuf {
    crate::settings::data_dir().join("rules-log.json")
}

impl Log {
    fn load() -> Log {
        std::fs::read(log_file()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    fn save(&self) {
        if let Ok(json) = serde_json::to_vec(self) {
            let file = log_file();
            std::thread::spawn(move || {
                let _ = std::fs::create_dir_all(file.parent().unwrap_or(Path::new("/")));
                let _ = std::fs::write(file, json);
            });
        }
    }
}

/// What a rule's trouble is kept by.
pub fn key(rule: &Rule) -> String {
    format!("{rule:?}")
}

/// Why the rule couldn't do something, last time it tried.
pub fn trouble(rule: &Rule, cx: &App) -> Option<String> {
    cx.try_global::<Log>().and_then(|l| l.trouble.get(&key(rule)).cloned())
}

/// Put back what the log's entry `at` moved (from the Trash too), and
/// leave it alone after.
pub fn undo(at: i64, cx: &mut App) {
    let Some(entry) = cx.try_global::<Log>().and_then(|l| l.entries.iter().find(|e| e.at == at && !e.undone).cloned()) else { return };
    cx.spawn(async move |cx| {
        let (moved, created) = (entry.moved.clone(), entry.created.clone());
        let result = cx.background_executor().spawn(async move { crate::finder::revert_outside(moved, created) }).await;
        cx.update(|cx| {
            let log = cx.global_mut::<Log>();
            match result {
                Ok(()) => {
                    if let Some(e) = log.entries.iter_mut().find(|e| e.at == at) {
                        e.undone = true;
                    }
                    log.spared.extend(entry.moved.iter().map(|(from, _)| from.clone()));
                }
                Err(e) => crate::browser::notice(format!("Couldn't undo: {e}"), cx),
            }
            cx.global::<Log>().save();
        });
    })
    .detach();
}

/// When a rule was last run, and how its folder was then.
struct Seen {
    changed: Option<SystemTime>,
    at: Instant,
    arriving: bool,
}

/// A rule's folder is looked at again when it's changed, when something
/// in it was still arriving, and every minute anyway (for what's been
/// there long enough by then).
const AGAIN: Duration = Duration::from_secs(60);

/// Run the rules that are on, now and then (see `AGAIN`).
pub fn watch(cx: &mut App) {
    cx.set_global(Log::load());
    cx.spawn(async move |cx| {
        let mut seen: HashMap<String, Seen> = HashMap::new();
        loop {
            cx.background_executor().timer(Duration::from_secs(5)).await;
            let (rules, spared) = cx.update(|cx| {
                let rules: Vec<Rule> = Settings::get(cx).rules.iter().filter(|r| r.on).cloned().collect();
                (rules, cx.global::<Log>().spared.iter().cloned().collect::<HashSet<_>>())
            });
            if rules.is_empty() {
                continue;
            }
            let (ran, kept) = cx
                .background_executor()
                .spawn(async move {
                    let mut ran = Vec::new();
                    for rule in rules {
                        let key = key(&rule);
                        let changed = rule.folder.metadata().and_then(|m| m.modified()).ok();
                        let due = seen.get(&key).is_none_or(|s| s.changed != changed || s.arriving || s.at.elapsed() >= AGAIN);
                        if !due {
                            continue;
                        }
                        let (done, arriving) = rule.run(now(), &spared);
                        // (Its folder as it is after: what it did changes it.)
                        let changed = rule.folder.metadata().and_then(|m| m.modified()).ok();
                        seen.insert(key.clone(), Seen { changed, at: Instant::now(), arriving });
                        ran.push((rule, done));
                    }
                    // (Of what Undo put back, what's still there.)
                    let kept: Vec<PathBuf> = spared.into_iter().filter(|p| p.symlink_metadata().is_ok()).collect();
                    (ran, (kept, seen))
                })
                .await;
            let (kept, back) = kept;
            seen = back;
            cx.update(|cx| {
                let log = cx.global_mut::<Log>();
                let mut changed = log.spared.len() != kept.len();
                log.spared = kept;
                for (rule, done) in ran {
                    let key = key(&rule);
                    let trouble = match done.error {
                        Some(e) => log.trouble.insert(key, e).is_none(),
                        None => log.trouble.remove(&key).is_some(),
                    };
                    changed |= trouble;
                    if done.moved.is_empty() {
                        continue;
                    }
                    let trashed: Vec<_> = done.moved.iter().filter(|(_, to)| crate::finder::is_trashed(to)).cloned().collect();
                    if !trashed.is_empty() {
                        crate::finder::remember_trashed(&trashed);
                    }
                    eprintln!("rules: {} in {}: moved {}", rule.words(), rule.folder.display(), done.moved.len());
                    // (Each a second apart at least: Undo finds it by when.)
                    let at = now().max(log.entries.last().map_or(0, |e| e.at + 1));
                    log.entries.push(Entry { at, folder: rule.folder.clone(), rule: rule.words(), moved: done.moved, created: done.created, undone: false });
                    changed = true;
                }
                if log.entries.len() > KEPT {
                    let extra = log.entries.len() - KEPT;
                    log.entries.drain(..extra);
                }
                if changed {
                    cx.global::<Log>().save();
                    // (Settings › Folder Rules shows it.)
                    cx.refresh_windows();
                }
            });
        }
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{DAY, Rule, Then, now};

    fn folder(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("f2-rules-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.canonicalize().unwrap()
    }

    fn names(plan: &[(std::path::PathBuf, Option<std::path::PathBuf>)]) -> Vec<String> {
        plan.iter().map(|(p, _)| p.file_name().unwrap().to_string_lossy().into_owned()).collect()
    }

    #[test]
    fn what_fits() {
        let dir = folder("fits");
        std::fs::create_dir_all(dir.join("Project")).unwrap();
        for name in ["Zoom.dmg", "Invoice March.pdf", "notes.pdf", "cat.jpg", "big.zip.download", ".hidden.dmg"] {
            std::fs::write(dir.join(name), "x").unwrap();
        }
        std::fs::write(dir.join("Big.mov"), vec![0u8; 2_000_000]).unwrap();
        let none = HashSet::new();
        let later = now() + 10;

        // Just arrived: left until it's settled.
        let all = Rule { folder: dir.clone(), ..Rule::default() };
        let (plan, arriving) = all.plan(now(), &none);
        assert!(plan.is_empty() && arriving, "{plan:?}");

        // Sort: what Tidy Up knows, not folders, hidden files or downloads still coming.
        let (plan, arriving) = all.plan(later, &none);
        assert!(!arriving);
        assert_eq!(names(&plan), ["Big.mov", "Invoice March.pdf", "Zoom.dmg", "cat.jpg", "notes.pdf"]);
        assert_eq!(plan[2].1.as_deref(), Some(dir.join("Installers/Zoom.dmg").as_path()));

        let installers = Rule { kinds: vec!["Installers".into()], added_days: Some(7), then: Then::Trash, ..all.clone() };
        assert!(installers.plan(later, &none).0.is_empty(), "not a week yet");
        let (plan, _) = installers.plan(now() + 8 * DAY, &none);
        assert_eq!(names(&plan), ["Zoom.dmg"]);
        assert_eq!(plan[0].1, None, "to the Trash");

        let invoices = Rule { kinds: vec!["Documents".into()], named: "INVOICE".into(), then: Then::Move(dir.join("Invoices")), ..all.clone() };
        assert_eq!(names(&invoices.plan(later, &none).0), ["Invoice March.pdf"]);

        let big = Rule { larger_mb: Some(1), ..all.clone() };
        assert_eq!(names(&big.plan(later, &none).0), ["Big.mov"]);

        // What Undo put back is left alone.
        let spared: HashSet<_> = [dir.join("cat.jpg")].into();
        assert!(!names(&all.plan(later, &spared).0).contains(&"cat.jpg".to_string()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn moves_without_replacing() {
        let dir = folder("moves");
        let to = dir.join("Out/Invoices");
        std::fs::create_dir_all(dir.join("In")).unwrap();
        std::fs::write(dir.join("In/Invoice.pdf"), "new").unwrap();
        std::fs::write(dir.join("In/Invoice 2.pdf"), "two").unwrap();
        let rule = Rule { folder: dir.join("In"), named: "invoice".into(), then: Then::Move(to.clone()), ..Rule::default() };
        let (done, _) = rule.run(now() + 10, &HashSet::new());
        assert!(done.error.is_none(), "{done:?}");
        assert_eq!(done.created, [dir.join("Out")], "the outermost folder made");
        assert_eq!(std::fs::read_to_string(to.join("Invoice.pdf")).unwrap(), "new");
        assert_eq!(std::fs::read_to_string(to.join("Invoice 2.pdf")).unwrap(), "two");
        // Again, with one of the names there: numbered, nothing replaced.
        std::fs::write(dir.join("In/Invoice.pdf"), "newer").unwrap();
        let (done, _) = rule.run(now() + 10, &HashSet::new());
        assert!(done.created.is_empty());
        assert_eq!(done.moved[0].1, to.join("Invoice 3.pdf"));
        assert_eq!(std::fs::read_to_string(to.join("Invoice.pdf")).unwrap(), "new", "not replaced");
        // And undone, as Settings' Undo does it: back, and the folder made gone.
        crate::finder::revert_outside(done.moved, done.created).unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("In/Invoice.pdf")).unwrap(), "newer");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn needs_days_for_the_trash() {
        let dir = folder("problems");
        let trash = Rule { folder: dir.clone(), then: Then::Trash, ..Rule::default() };
        assert!(trash.problem().is_some());
        assert!(Rule { added_days: Some(7), ..trash.clone() }.problem().is_none());
        assert!(Rule { then: Then::Move(dir.clone()), ..trash.clone() }.problem().is_some(), "into itself");
        assert!(Rule { folder: dir.join("nope"), then: Then::Sort, ..trash }.problem().is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn in_words() {
        let rule = Rule { kinds: vec!["Installers".into()], added_days: Some(7), then: Then::Trash, ..Rule::default() };
        assert_eq!(rule.words(), format!("Installers, added over a week ago → {}", crate::macos::bin()));
        let rule = Rule { kinds: vec!["Images".into(), "Videos".into()], named: "trip".into(), larger_mb: Some(50), then: Then::Sort, ..Rule::default() };
        assert_eq!(rule.words(), "Images and Videos named “trip”, over 50 MB → folders by kind");
    }
}
