//! Extensions: what Quick Open shows that isn't F2's own. Each is a folder
//! in Extensions (beside the settings) holding a manifest, f2.json, and a
//! program in any language, which F2 runs to ask what to show: it writes
//! what's wanted to the program's stdin (JSON), and reads what it says on
//! its stdout as it says it, JSON values one after another (it's loading,
//! these rows, this page, a notice…). EXTENSIONS.md says what goes between
//! them. Those in JavaScript run in F2's sandbox; a program, only with
//! Developer Mode on. The store's (src/store.rs) are JavaScript.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The version of the extension API this F2 speaks: what manifests say
/// they're written for (`api`). One written for a later one isn't run.
pub const API: u32 = 1;

/// An extension, as its manifest has it.
#[derive(Debug)]
pub struct Extension {
    /// What it's known by (in shortcuts' keys, its settings, the cache):
    /// its manifest's `id`, whatever its folder's called.
    pub id: String,
    pub dir: PathBuf,
    pub manifest: Manifest,
    /// A page of F2's own (Colors), not in Extensions: its code says what
    /// its commands show, as a script would, at once.
    pub native: Option<Native>,
}

/// What a page of F2's own says, for a command (with its arguments, and
/// what's typed): told to `tell`, as an extension's run says it, now or
/// later (off the main thread too). The run's over when `tell`'s dropped.
pub type Native = fn(command: &str, args: &Value, query: &str, tell: Tell, cx: &mut gpui::App);

/// Where a page of F2's own says what it shows (see `Native`).
pub struct Tell(pub(crate) futures::channel::mpsc::UnboundedSender<Heard>);

impl Tell {
    /// A message, as an extension's (`{"items": …}`, `{"error": …}`…).
    pub fn say(&self, message: Value) {
        match serde_json::from_value(message) {
            Ok(message) => {
                let _ = self.0.unbounded_send(Heard::Said(message));
            }
            Err(e) => {
                let _ = self.0.unbounded_send(Heard::Garbled(e.to_string()));
            }
        }
    }
}

impl Drop for Tell {
    fn drop(&mut self) {
        let _ = self.0.unbounded_send(Heard::Ended { ok: true, output: String::new() });
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct Manifest {
    /// The extension API it's written for (see `API`).
    #[serde(default)]
    pub api: Option<u32>,
    /// Its author's, a dot, then its own name: "jane.linear" (lowercase
    /// letters, digits and hyphens). A store's say they're whoever's
    /// GitHub account that is; "f2." is F2's own.
    #[serde(default)]
    pub id: Option<String>,
    /// Its version ("1.2.0"): the store's updates are by it.
    #[serde(default)]
    pub version: Option<String>,
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// An SF Symbol's name, an emoji, or an image in its folder.
    #[serde(default)]
    pub icon: Option<String>,
    /// Its symbols' tiles' color ("#24292f").
    #[serde(default)]
    pub color: Option<String>,
    /// What runs: a program and its arguments, in its folder. (Not for
    /// one in JavaScript, which F2 runs itself: see `runtime`.)
    #[serde(default)]
    pub run: Option<Program>,
    /// "javascript": its `main` (index.js) run by F2's own JavaScript host,
    /// shut in a sandbox, nothing to install (see src/macos/jshost.m): what
    /// a store's extensions are to be.
    #[serde(default)]
    pub runtime: Option<String>,
    #[serde(default)]
    pub main: Option<String>,
    /// What one in JavaScript may do beyond showing things.
    #[serde(default)]
    pub permissions: Permissions,
    #[serde(default)]
    pub commands: Vec<Spec>,
    /// Its settings, in Settings › Extensions: what it's given each time
    /// it runs (passwords kept in the keychain, the rest with F2's
    /// settings).
    #[serde(default)]
    pub preferences: Vec<Preference>,
}

/// One of its settings.
#[derive(Deserialize, Clone, Debug)]
pub struct Preference {
    pub id: String,
    pub title: String,
    #[serde(rename = "type", default)]
    pub kind: PrefKind,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub placeholder: String,
    /// It doesn't run without it: opening a command goes to its settings.
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub default: Option<Value>,
    /// A dropdown's.
    #[serde(default)]
    pub options: Vec<Choice>,
    /// Where to get one (a token).
    #[serde(default)]
    pub link: Option<String>,
}

#[derive(Deserialize, Clone, Copy, Debug, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PrefKind {
    #[default]
    Text,
    /// Kept in the keychain, not shown as it's typed.
    Password,
    Checkbox,
    Dropdown,
    Number,
}

/// A dropdown's choice: its value, and what it's called (else its value).
#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum Choice {
    Plain(String),
    Titled { value: String, title: String },
}

impl Choice {
    pub fn value(&self) -> &str {
        match self {
            Choice::Plain(v) | Choice::Titled { value: v, .. } => v,
        }
    }

    pub fn title(&self) -> &str {
        match self {
            Choice::Plain(v) | Choice::Titled { title: v, .. } => v,
        }
    }
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Permissions {
    /// The hosts it may fetch from, over https ("api.github.com", or
    /// "*.github.com" for any under it).
    pub network: Vec<String>,
}

impl Manifest {
    /// It's run by F2's JavaScript host.
    pub fn javascript(&self) -> bool {
        self.runtime.as_deref().is_some_and(|r| r.eq_ignore_ascii_case("javascript"))
    }

    fn argv(&self) -> Vec<String> {
        self.run.as_ref().map(Program::argv).unwrap_or_default()
    }
}

#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum Program {
    Line(String),
    Args(Vec<String>),
}

impl Program {
    fn argv(&self) -> Vec<String> {
        match self {
            Program::Line(line) => line.split_whitespace().map(String::from).collect(),
            Program::Args(args) => args.clone(),
        }
    }
}

/// One of its commands: in Quick Open's results by its title (and its
/// keyword), opening a page of what it shows.
#[derive(Deserialize, Clone, Debug)]
pub struct Spec {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub subtitle: String,
    #[serde(default)]
    pub icon: Option<String>,
    /// Typed first ("gh f2"), its rows are looked through for the rest.
    #[serde(default)]
    pub keyword: Option<String>,
    /// How long what it showed stays fresh ("30m", "2h"): opened again
    /// within it, it isn't run (⌘R runs it). None: run each time, what it
    /// showed last shown meanwhile.
    #[serde(default)]
    pub refresh: Option<String>,
    /// Who looks through it as you type: F2 (the default), or the program,
    /// asked again for each query ("search": "script").
    #[serde(default)]
    pub search: Search,
    /// Its rows in Quick Open's own results too, found by what's typed.
    #[serde(default)]
    pub root: bool,
    /// Not a command of its own: only opened from another page (`push`).
    #[serde(default)]
    pub hidden: bool,
}

#[derive(Deserialize, Clone, Copy, Debug, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Search {
    #[default]
    F2,
    Script,
}

impl Spec {
    pub fn fresh_for(&self) -> Option<Duration> {
        self.refresh.as_deref().and_then(duration)
    }
}

/// "30m", "2h", "45s", "1d".
fn duration(text: &str) -> Option<Duration> {
    let text = text.trim();
    let (n, unit) = text.split_at(text.find(|c: char| !c.is_ascii_digit())?);
    let n: u64 = n.parse().ok()?;
    Some(Duration::from_secs(
        n * match unit.trim() {
            "s" => 1,
            "m" => 60,
            "h" => 3600,
            "d" => 86400,
            _ => return None,
        },
    ))
}

// What the program says.

/// A message, one of the JSON values the program prints: any of these,
/// together or apart.
#[derive(Deserialize, Default, Debug)]
#[serde(default)]
pub struct Message {
    /// It's working on it: what it's doing ("Fetching your repos…"), or
    /// true; false: done.
    pub loading: Option<Loading>,
    /// How far along, 0 to 1.
    pub progress: Option<f32>,
    /// The page's title, and its field's placeholder.
    pub title: Option<String>,
    pub placeholder: Option<String>,
    /// The page's rows (all of them), or more after them.
    pub items: Option<Vec<Row>>,
    pub append: Option<Vec<Row>>,
    /// The page as one thing to read, not a list.
    pub detail: Option<Detail>,
    pub notice: Option<Notice>,
    pub error: Option<String>,
    /// Done at once: a link (or a file) opened, text copied.
    pub open: Option<String>,
    pub copy: Option<String>,
    /// The page asked again; Quick Open closed; back a page.
    pub refresh: bool,
    pub close: bool,
    pub pop: bool,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum Loading {
    Said(String),
    On(bool),
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum Notice {
    Text(String),
    Titled {
        title: String,
        #[serde(default)]
        text: String,
    },
}

/// A row of a page.
#[derive(Deserialize, Serialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Row {
    pub title: String,
    pub subtitle: String,
    pub icon: Option<String>,
    /// Its symbol's tile's color (else the extension's).
    pub color: Option<String>,
    /// The heading it's under (rows with the same one, together).
    pub section: Option<String>,
    /// On the right ("2 PRs", "3h ago").
    pub accessory: Option<String>,
    pub tags: Vec<Tag>,
    /// What else it's found by.
    pub keywords: Vec<String>,
    /// Shown beside the list while it's picked.
    pub detail: Option<Detail>,
    /// The first is Return's, the second ⌘Return's, all in ⌘K.
    pub actions: Vec<Action>,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Tag {
    pub text: String,
    pub color: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Detail {
    /// A swatch of it, above its text (a color's page).
    pub color: Option<String>,
    pub markdown: String,
    /// Facts down the side: a label and its text.
    pub metadata: Vec<Fact>,
    /// A detail page's own actions (a row's detail uses the row's).
    pub actions: Vec<Action>,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Fact {
    pub label: String,
    pub text: String,
    pub color: Option<String>,
}

/// What an action does: one of these.
#[derive(Deserialize, Serialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Action {
    pub title: String,
    pub icon: Option<String>,
    /// A link, or a file or folder (opened in its app, a folder in F2).
    pub open: Option<String>,
    pub copy: Option<String>,
    /// A file or folder shown in F2.
    pub reveal: Option<String>,
    /// Another page: a command of the extension's, with arguments.
    pub push: Option<Call>,
    /// The program run to do something (what it says then: a notice, the
    /// page asked again…), the page staying.
    pub run: Option<Call>,
    /// Asked first: Return again does it.
    pub confirm: Option<String>,
    /// It can't be taken back: in red.
    pub destructive: bool,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default, PartialEq)]
pub struct Call {
    pub command: String,
    #[serde(default)]
    pub args: Value,
}

// The extensions there are.

/// The folder they're in.
pub fn folder() -> PathBuf {
    crate::settings::data_dir().join("Extensions")
}

/// One that couldn't be read: its folder, and why.
#[derive(Debug, Clone)]
pub struct Broken {
    pub dir: PathBuf,
    pub why: String,
}

/// The extensions Quick Open has: those turned on (a program only with
/// Developer Mode on), and those whose manifests couldn't be read.
pub fn all() -> (Vec<Arc<Extension>>, Vec<Broken>) {
    let (exts, broken) = every();
    let developer = SYNCED.lock().unwrap().developer;
    (exts.into_iter().filter(|e| enabled(&e.id) && (developer || !e.program())).collect(), broken)
}

/// When the folder was last looked at: again at most once a second
/// (Quick Open asks as it draws), unless one was just made or changed.
static LOOKED: Mutex<Option<Instant>> = Mutex::new(None);

/// The folder looked at again the next time it's asked about.
pub fn look_again() {
    *LOOKED.lock().unwrap() = None;
}

/// Every extension, those turned off too (Settings lists them).
pub fn every() -> (Vec<Arc<Extension>>, Vec<Broken>) {
    static KEPT: Mutex<Option<(Vec<(PathBuf, SystemTime)>, Vec<Arc<Extension>>, Vec<Broken>)>> = Mutex::new(None);
    {
        let mut looked = LOOKED.lock().unwrap();
        if looked.is_some_and(|at| at.elapsed() < Duration::from_secs(1))
            && let Some((_, exts, broken)) = KEPT.lock().unwrap().as_ref()
        {
            return (exts.clone(), broken.clone());
        }
        *looked = Some(Instant::now());
    }
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(folder()).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    dirs.sort();
    // (Each manifest's time: none changed, the ones read before.)
    let stamps: Vec<(PathBuf, SystemTime)> = dirs.iter().map(|d| (d.clone(), std::fs::metadata(d.join("f2.json")).and_then(|m| m.modified()).unwrap_or(UNIX_EPOCH))).collect();
    let mut kept = KEPT.lock().unwrap();
    if let Some((was, exts, broken)) = kept.as_ref()
        && *was == stamps
    {
        return (exts.clone(), broken.clone());
    }
    let (mut exts, mut broken): (Vec<Arc<Extension>>, Vec<Broken>) = (Vec::new(), Vec::new());
    for dir in dirs {
        if dir.file_name().is_none_or(|n| n.to_string_lossy().starts_with('.')) {
            continue;
        }
        let manifest = dir.join("f2.json");
        if !manifest.exists() {
            continue;
        }
        let read = std::fs::read(&manifest).map_err(|e| e.to_string()).and_then(|bytes| serde_json::from_slice::<Manifest>(&bytes).map_err(|e| e.to_string()));
        match read.and_then(|manifest| checked(manifest, &dir)) {
            Ok((id, manifest)) => match exts.iter().find(|e| e.id == id) {
                // (Two can't be one: the second's broken.)
                Some(first) => {
                    let why = format!("f2.json: its \"id\" ({id}) is the one in {} too", first.dir.file_name().unwrap_or_default().to_string_lossy());
                    broken.push(Broken { dir, why });
                }
                None => exts.push(Arc::new(Extension { id, dir, manifest, native: None })),
            },
            Err(why) => broken.push(Broken { dir, why: format!("f2.json: {why}") }),
        }
    }
    *kept = Some((stamps, exts.clone(), broken.clone()));
    (exts, broken)
}

/// `manifest` as one that can run (in `dir`): its id, or what's wrong.
fn checked(manifest: Manifest, dir: &Path) -> Result<(String, Manifest), String> {
    let id = match (manifest.api, manifest.id.clone()) {
        (None, _) => return Err(format!("it doesn't say which version of F2's extension API it's written for: \"api\": {API}")),
        (Some(api), _) if api > API => return Err(format!("it's written for a newer F2 (extension API {api}; this one's is {API}): update F2")),
        (Some(0), _) => return Err("\"api\" starts at 1".into()),
        (_, None) => return Err("it has no \"id\": yours, a dot, then its name (\"jane.linear\")".into()),
        (_, Some(id)) if !valid_id(&id) => return Err(format!("\"{id}\" isn't an id: yours, a dot, then its name (\"jane.linear\"), in lowercase letters, digits and hyphens")),
        (_, Some(id)) => id,
    };
    let main = manifest.main.as_deref().unwrap_or("index.js");
    if manifest.javascript() && !dir.join(main).is_file() {
        return Err(format!("its \"main\" ({main}) isn't in its folder"));
    }
    if !manifest.javascript() && manifest.argv().is_empty() {
        return Err("\"run\" says nothing to run".into());
    }
    Ok((id, manifest))
}

/// "jane.linear": two names, a dot between, each of lowercase letters,
/// digits and hyphens (not first).
pub fn valid_id(id: &str) -> bool {
    let Some((author, name)) = id.split_once('.') else { return false };
    let part = |p: &str| p.chars().next().is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit()) && p.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    id.len() <= 64 && part(author) && part(name)
}

// A new one.

/// What a new one starts as (extensions/template), F2's API's types and
/// f2.json's schema (an editor's completions), and the whole reference.
const TEMPLATE: [(&str, &str); 7] = [
    ("f2.json", include_str!("../extensions/template/f2.json")),
    ("index.js", include_str!("../extensions/template/index.js")),
    ("jsconfig.json", include_str!("../extensions/template/jsconfig.json")),
    ("README.md", include_str!("../extensions/template/README.md")),
    ("f2.d.ts", include_str!("../extensions/js/f2.d.ts")),
    ("f2.schema.json", include_str!("../extensions/js/f2.schema.json")),
    ("EXTENSIONS.md", include_str!("../EXTENSIONS.md")),
];

/// A new extension called `name`, in JavaScript, ready to run: a folder
/// in Extensions with a command showing a few rows. Its id and folder.
pub fn create(name: &str) -> Result<(String, PathBuf), String> {
    let taken: Vec<String> = every().0.iter().map(|e| e.id.clone()).collect();
    let made = create_in(&folder(), name, &author(), &taken)?;
    look_again();
    Ok(made)
}

fn create_in(dir: &Path, name: &str, author: &str, taken: &[String]) -> Result<(String, PathBuf), String> {
    let name = name.trim();
    let base = slug(name);
    if base.is_empty() {
        return Err("Its name needs a letter or a digit in it.".into());
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    // (A folder of its own, and an id: "my-thing", else "my-thing-2"…)
    for n in 1..100 {
        let slug = if n == 1 { base.clone() } else { format!("{base}-{n}") };
        let id = format!("{author}.{slug}");
        let folder = dir.join(&slug);
        if taken.contains(&id) || folder.exists() {
            continue;
        }
        std::fs::create_dir(&folder).map_err(|e| format!("Couldn't make its folder: {e}"))?;
        let quoted = |text: &str| serde_json::to_string(text).unwrap_or_default();
        for (file, text) in TEMPLATE {
            let text = match file {
                // (Into JSON as JSON strings; elsewhere as they are.)
                "f2.json" => text.replace("\"__ID__\"", &quoted(&id)).replace("\"__NAME__\"", &quoted(name)),
                "index.js" | "README.md" => text.replace("__NAME__", name),
                _ => text.to_string(),
            };
            std::fs::write(folder.join(file), text).map_err(|e| format!("Couldn't write its {file}: {e}"))?;
        }
        return Ok((id, folder));
    }
    Err(format!("There are already extensions called {name}."))
}

/// `name` as an id's part: "My Thing!" is "my-thing".
fn slug(name: &str) -> String {
    let mut slug = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    slug.truncate(40);
    slug.trim_end_matches('-').to_string()
}

/// Whose new ones are: your account's name ("dagurleo"), as an id's part.
fn author() -> String {
    let home = std::env::var("HOME").ok().and_then(|h| Path::new(&h).file_name().map(|n| n.to_string_lossy().into_owned()));
    let user = std::env::var("USER").ok().or_else(|| std::env::var("LOGNAME").ok()).or(home).unwrap_or_default();
    Some(slug(&user)).filter(|u| !u.is_empty()).unwrap_or_else(|| "me".into())
}

pub fn by_id(id: &str) -> Option<Arc<Extension>> {
    all().0.into_iter().find(|e| e.id == id)
}

/// One, turned off or not.
pub fn any_by_id(id: &str) -> Option<Arc<Extension>> {
    every().0.into_iter().find(|e| e.id == id)
}

impl Extension {
    pub fn spec(&self, command: &str) -> Option<&Spec> {
        self.manifest.commands.iter().find(|c| c.id == command)
    }

    /// An image of its own, by its name in its folder.
    pub fn file(&self, name: &str) -> Option<PathBuf> {
        let path = self.dir.join(name);
        path.canonicalize().ok().filter(|p| p.starts_with(self.dir.canonicalize().unwrap_or_else(|_| self.dir.clone())) && p.is_file())
    }

    /// When it was last changed: its manifest, and its script (or the
    /// files in its folder its program's `run` names). Saved, a page of it
    /// runs again.
    pub fn changed(&self) -> SystemTime {
        if self.native.is_some() {
            return UNIX_EPOCH;
        }
        let mut files = vec![self.dir.join("f2.json")];
        if self.manifest.javascript() {
            files.push(self.dir.join(self.manifest.main.as_deref().unwrap_or("index.js")));
        } else {
            files.extend(self.manifest.argv().iter().map(|a| self.dir.join(a)));
        }
        files.iter().filter_map(|f| std::fs::metadata(f).and_then(|m| m.modified()).ok()).max().unwrap_or(UNIX_EPOCH)
    }

    /// It runs a program of its own, as you, not a script in F2's sandbox.
    pub fn program(&self) -> bool {
        self.native.is_none() && !self.manifest.javascript()
    }

    /// It may run: a script, in F2's sandbox, always; a program only with
    /// Developer Mode on, once you've allowed it (in this folder).
    pub fn may_run(&self) -> bool {
        self.may_run_by(&SYNCED.lock().unwrap())
    }

    fn may_run_by(&self, synced: &Synced) -> bool {
        !self.program() || (synced.developer && synced.allowed.get(&self.id).is_some_and(|dir| Path::new(dir) == self.dir))
    }

    /// What its passwords and storage are kept under: not its id alone, so
    /// one you've been sent can't say it's one from the store to get at
    /// what that one kept. The store's are those F2 installed (settings'
    /// `store_installed`), in their folders; the rest are "local".
    fn kept_as(&self) -> String {
        format!("{}.{}", if self.from_store() { "store" } else { "local" }, self.id)
    }

    /// F2 installed it from the store, where it is.
    pub fn from_store(&self) -> bool {
        self.native.is_none() && synced().store.contains_key(&self.id) && self.dir == folder().join(&self.id)
    }

    fn service(&self) -> String {
        format!("F2 Extension {}", self.kept_as())
    }

    /// Its settings, as its manifest has them.
    pub fn preferences(&self) -> &[Preference] {
        &self.manifest.preferences
    }

    /// Its password `id`, from the keychain.
    fn password(&self, id: &str) -> Option<String> {
        crate::macos::keychain_get(&self.service(), id)
    }

    /// What `pref` is set to (or its default): None if it's not (or it's
    /// empty).
    pub fn value(&self, pref: &Preference) -> Option<Value> {
        let value = match pref.kind {
            PrefKind::Password => self.password(&pref.id).map(Value::String),
            _ => synced().values.get(&self.id).and_then(|v| v.get(&pref.id)).cloned().or_else(|| pref.default.clone()),
        };
        let value = value.or_else(|| match pref.kind {
            PrefKind::Checkbox => Some(Value::Bool(false)),
            PrefKind::Dropdown => pref.options.first().map(|c| Value::String(c.value().to_string())),
            _ => None,
        });
        value.filter(|v| !v.is_null() && v.as_str().is_none_or(|s| !s.trim().is_empty()))
    }

    /// All of its settings' values, as it's given them.
    pub fn values(&self) -> serde_json::Map<String, Value> {
        self.preferences().iter().filter_map(|p| Some((p.id.clone(), self.value(p)?))).collect()
    }

    /// The settings it needs that aren't set.
    pub fn missing(&self) -> Vec<Preference> {
        self.preferences().iter().filter(|p| p.required && self.value(p).is_none()).cloned().collect()
    }

    /// A password kept in the keychain (or, empty, forgotten).
    pub fn keep_password(&self, id: &str, password: &str) -> bool {
        if password.is_empty() {
            crate::macos::keychain_delete(&self.service(), id);
            return true;
        }
        crate::macos::keychain_set(&self.service(), id, password)
    }

    pub fn has_password(&self, id: &str) -> bool {
        self.password(id).is_some()
    }

    fn passwords(&self) -> Vec<String> {
        self.preferences().iter().filter(|p| p.kind == PrefKind::Password).map(|p| p.id.clone()).collect()
    }

    pub fn forget_passwords(&self) {
        for id in self.passwords() {
            crate::macos::keychain_delete(&self.service(), &id);
        }
    }

    pub fn has_passwords(&self) -> bool {
        self.passwords().iter().any(|id| self.has_password(id))
    }
}

// What Settings say of extensions, kept here too: what's run off the main
// thread reads it (see `sync`).

#[derive(Default, Clone)]
struct Synced {
    values: std::collections::BTreeMap<String, std::collections::BTreeMap<String, Value>>,
    disabled: Vec<String>,
    /// Developer Mode: programs run (those allowed).
    developer: bool,
    /// The programs allowed to run: their ids, and the folders they were in.
    allowed: std::collections::BTreeMap<String, String>,
    /// Those installed from the store: their ids, and versions.
    store: std::collections::BTreeMap<String, String>,
}

fn synced() -> Synced {
    SYNCED.lock().unwrap().clone()
}

static SYNCED: Mutex<Synced> = Mutex::new(Synced { values: std::collections::BTreeMap::new(), disabled: Vec::new(), developer: false, allowed: std::collections::BTreeMap::new(), store: std::collections::BTreeMap::new() });

/// Settings' extension preferences, which are off, and which programs may
/// run, as they are now (when F2 starts, and whenever they change).
pub fn sync(settings: &crate::settings::Settings) {
    *SYNCED.lock().unwrap() = Synced {
        values: settings.extension_preferences.clone(),
        disabled: settings.disabled_extensions.clone(),
        developer: settings.extension_developer_mode,
        allowed: settings.allowed_extension_programs.clone(),
        store: settings.store_installed.clone(),
    };
}

pub fn enabled(id: &str) -> bool {
    !SYNCED.lock().unwrap().disabled.iter().any(|d| d == id)
}

/// What it showed last, forgotten (its settings changed: it may show
/// something else now).
pub fn forget_shown(id: &str) {
    let prefix = format!("{id}+");
    memory().lock().unwrap().retain(|key, _| !key.starts_with(&prefix));
    let dir = crate::settings::data_dir().join("Extension Cache");
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        if entry.file_name().to_string_lossy().starts_with(&prefix) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// The tiles' color when it doesn't say.
const GRAY: u32 = 0x636366;

impl Extension {
    pub fn tint(&self) -> u32 {
        self.manifest.color.as_deref().and_then(color).unwrap_or(GRAY)
    }

    /// An icon it names (a command's, a row's; else its own): an image in
    /// its folder, or an SF Symbol on a tile of `tint` (else its color).
    pub fn icon(&self, name: Option<&str>, tint: Option<&str>) -> crate::commands::Icon {
        let name = name.or(self.manifest.icon.as_deref()).unwrap_or("puzzlepiece.extension.fill");
        if is_image(name)
            && let Some(file) = self.file(name)
        {
            return crate::commands::Icon::Image(file);
        }
        // (An emoji isn't a symbol: the extension's own then.)
        let name = if name.chars().next().is_some_and(|c| !c.is_ascii()) { "puzzlepiece.extension.fill" } else { name };
        crate::commands::Icon::Tile(interned(name), tint.and_then(color).unwrap_or_else(|| self.tint()))
    }

    /// `spec` as a command in Quick Open's results.
    pub fn command(&self, spec: &Spec) -> crate::commands::Command {
        crate::commands::Command {
            title: spec.title.clone(),
            subtitle: if spec.subtitle.is_empty() { self.manifest.name.clone() } else { spec.subtitle.clone() },
            kind: "Command",
            icon: self.icon(spec.icon.as_deref(), None),
            run: crate::commands::Run::Extension { ext: self.id.clone(), command: spec.id.clone() },
            confirm: None,
            key: Some(format!("ext:{}/{}", self.id, spec.id)),
        }
    }
}

pub fn is_image(name: &str) -> bool {
    let lower = name.to_lowercase();
    [".png", ".jpg", ".jpeg", ".gif", ".webp", ".svg", ".icns", ".tiff", ".heic"].iter().any(|e| lower.ends_with(e))
}

/// The command Recent (or a shortcut) knows as "ext:<id>/<command>".
pub fn by_key(key: &str) -> Option<crate::commands::Command> {
    let (id, command) = key.strip_prefix("ext:")?.split_once('/')?;
    let ext = by_id(id)?;
    Some(ext.command(ext.spec(command)?))
}

// Running it.

/// What's heard from a run.
pub enum Heard {
    Said(Message),
    /// It printed plain text, not JSON: all of it (a page of it).
    Text(String),
    /// What it printed that isn't a message: why not.
    Garbled(String),
    /// It's done: whether it went well, and what it printed on stderr (its
    /// end, if it's long).
    Ended { ok: bool, output: String },
}

/// A run going on: stopped when this is dropped.
pub struct Running {
    pid: i32,
    done: Arc<AtomicBool>,
}

impl Running {
    pub fn stop(&self) {
        if !self.done.load(Ordering::SeqCst) {
            // (Its group: what it started too, `gh` and the like. Again a
            // moment later: one it was starting just then, mid-fork, isn't
            // told the first time, and would go on, its parent gone.)
            let pid = self.pid;
            unsafe { libc::kill(-pid, libc::SIGTERM) };
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(100));
                unsafe { libc::kill(-pid, libc::SIGTERM) };
            });
        }
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        self.stop();
    }
}

#[derive(Serialize)]
struct Request<'a> {
    command: &'a str,
    args: &'a Value,
    query: &'a str,
    /// Its settings' values.
    preferences: &'a serde_json::Map<String, Value>,
    /// F2's version.
    f2: &'static str,
}

/// Run `ext` for `command` (with `args`, and what's typed): what it says
/// comes on the channel as it says it, then that it's ended.
pub fn call(ext: &Extension, command: &str, args: &Value, query: &str) -> Result<(Running, futures::channel::mpsc::UnboundedReceiver<Heard>), String> {
    if ext.manifest.javascript() {
        return call_js(ext, command, args, query);
    }
    if !ext.may_run() {
        return Err(format!("{} runs a program on your Mac: it needs Developer Mode on, and to be allowed, in Settings › Extensions.", ext.manifest.name));
    }
    call_program(ext, command, args, query)
}

/// Run `ext`'s program (allowed or not).
fn call_program(ext: &Extension, command: &str, args: &Value, query: &str) -> Result<(Running, futures::channel::mpsc::UnboundedReceiver<Heard>), String> {
    let argv = ext.manifest.argv();
    let path = path();
    let program = find_program(&argv[0], &ext.dir, &path).ok_or_else(|| format!("Can't find {} to run (looked in its folder and on your PATH).", argv[0]))?;
    let mut run = Command::new(&program);
    run.args(&argv[1..])
        .current_dir(&ext.dir)
        .env("PATH", &path)
        .env("F2_EXTENSION", &ext.id)
        .env("F2_EXTENSION_DIR", &ext.dir)
        .env("F2_COMMAND", command)
        .env("F2_VERSION", env!("CARGO_PKG_VERSION"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    // (Its passwords as environment variables too, by their ids.)
    let preferences = ext.values();
    for id in ext.passwords() {
        if let Some(Value::String(value)) = preferences.get(&id) {
            run.env(&id, value);
        }
    }
    let mut child = run.spawn().map_err(|e| format!("Couldn't run {}: {e}", program.display()))?;
    let args = if args.is_null() { Value::Object(Default::default()) } else { args.clone() };
    let request = serde_json::to_vec(&Request { command, args: &args, query, preferences: &preferences, f2: env!("CARGO_PKG_VERSION") }).unwrap_or_default();
    // (Written and closed: one that doesn't read it isn't held up.)
    if let Some(mut stdin) = child.stdin.take() {
        std::thread::spawn(move || {
            let _ = stdin.write_all(&request);
        });
    }
    let (tell, heard) = futures::channel::mpsc::unbounded();
    let done = Arc::new(AtomicBool::new(false));
    let running = Running { pid: child.id() as i32, done: done.clone() };
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    std::thread::spawn(move || {
        // (Its stderr read alongside, so a chatty one doesn't fill the pipe
        // and stop; only its end kept.)
        let errors = std::thread::spawn(move || {
            let mut kept = Vec::new();
            if let Some(mut stderr) = stderr {
                let mut chunk = [0u8; 8192];
                while let Ok(n) = stderr.read(&mut chunk) {
                    if n == 0 {
                        break;
                    }
                    kept.extend_from_slice(&chunk[..n]);
                    if kept.len() > 64 * 1024 {
                        kept.drain(..kept.len() - 32 * 1024);
                    }
                }
            }
            String::from_utf8_lossy(&kept).into_owned()
        });
        if let Some(stdout) = stdout {
            read_messages(BufReader::new(stdout), |heard| tell.unbounded_send(heard).is_ok());
        }
        let status = child.wait();
        done.store(true, Ordering::SeqCst);
        let output = errors.join().unwrap_or_default();
        let _ = tell.unbounded_send(Heard::Ended { ok: status.is_ok_and(|s| s.success()), output });
    });
    Ok((running, heard))
}

/// The messages in what a program prints, each told as it's read (till
/// `tell` says stop). Plain text, not JSON, is told all at once at the end.
fn read_messages(mut out: impl BufRead, mut tell: impl FnMut(Heard) -> bool) {
    // (What it starts with: JSON's `{`, or text.)
    let first = loop {
        match out.fill_buf() {
            Ok([]) | Err(_) => return,
            Ok(buf) => match buf.iter().position(|b| !b.is_ascii_whitespace()) {
                Some(i) => {
                    let b = buf[i];
                    out.consume(i);
                    break b;
                }
                None => {
                    let n = buf.len();
                    out.consume(n);
                }
            },
        }
    };
    if first != b'{' {
        let mut text = String::new();
        let _ = out.read_to_string(&mut text);
        tell(Heard::Text(text));
        return;
    }
    for value in serde_json::Deserializer::from_reader(out).into_iter::<Value>() {
        let heard = match value {
            Ok(value) => match serde_json::from_value::<Message>(value) {
                Ok(message) => Heard::Said(message),
                Err(e) => Heard::Garbled(format!("A message F2 doesn't understand: {e}")),
            },
            Err(e) => {
                tell(Heard::Garbled(format!("Output that isn't JSON: {e}")));
                return;
            }
        };
        if !tell(heard) {
            return;
        }
    }
}

/// The API scripts have (`f2`, `fetch`, timers), given to the host first.
const PRELUDE: &str = include_str!("js_host.js");

/// What one in JavaScript may ask F2 for: by its manifest.
#[derive(Clone)]
struct Allowed {
    /// What its storage is kept under (`Extension::kept_as`).
    kept_as: String,
    network: Vec<String>,
}

/// Run `ext`'s script for `command` in F2's JavaScript host: F2 itself
/// started again (F2_EXTENSION_HOST, nothing else of F2's environment),
/// which shuts itself in before the script runs. What the script asks F2
/// to do (fetch, its storage) is done here, if its manifest
/// allows it; what it says comes on the channel, as `call`'s does.
fn call_js(ext: &Extension, command: &str, args: &Value, query: &str) -> Result<(Running, futures::channel::mpsc::UnboundedReceiver<Heard>), String> {
    let main = ext.manifest.main.as_deref().unwrap_or("index.js");
    let file = ext.file(main).ok_or_else(|| format!("{main} isn't in its folder."))?;
    let script = std::fs::read_to_string(&file).map_err(|e| format!("Couldn't read {main}: {e}"))?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut child = Command::new(exe)
        .env_clear()
        .env("F2_EXTENSION_HOST", "1")
        .current_dir("/")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
        .map_err(|e| format!("Couldn't start F2's JavaScript host: {e}"))?;
    let args = if args.is_null() { Value::Object(Default::default()) } else { args.clone() };
    let start = serde_json::json!({ "prelude": PRELUDE, "script": script, "main": main, "command": command, "args": args, "query": query, "preferences": ext.values() });
    let stdin = Arc::new(Mutex::new(child.stdin.take()));
    tell_host(&stdin, &start);
    let allowed = Allowed { kept_as: ext.kept_as(), network: ext.manifest.permissions.network.clone() };
    let (tell, heard) = futures::channel::mpsc::unbounded();
    let done = Arc::new(AtomicBool::new(false));
    let running = Running { pid: child.id() as i32, done: done.clone() };
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    std::thread::spawn(move || {
        let errors = std::thread::spawn(move || {
            let mut kept = String::new();
            if let Some(stderr) = stderr {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    kept.push_str(&line);
                    kept.push('\n');
                    if kept.len() > 64 * 1024 {
                        kept.drain(..kept.len() - 32 * 1024);
                    }
                }
            }
            kept
        });
        if let Some(stdout) = stdout {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                let Ok(value) = serde_json::from_str::<Value>(&line) else { continue };
                if let Some(said) = value.get("say") {
                    let heard = match serde_json::from_value::<Message>(said.clone()) {
                        Ok(message) => Heard::Said(message),
                        Err(e) => Heard::Garbled(format!("A message F2 doesn't understand: {e}")),
                    };
                    if tell.unbounded_send(heard).is_err() {
                        break;
                    }
                } else if let Some(id) = value.get("call").cloned() {
                    // (Each on its own: a script can wait on several at once.)
                    let (allowed, stdin) = (allowed.clone(), stdin.clone());
                    std::thread::spawn(move || {
                        let reply = match asked(&allowed, &value) {
                            Ok(result) => serde_json::json!({ "reply": id, "result": result }),
                            Err(why) => serde_json::json!({ "reply": id, "error": why }),
                        };
                        tell_host(&stdin, &reply);
                    });
                }
            }
        }
        // (Its stdin closed: told nothing more, it ends.)
        stdin.lock().unwrap().take();
        let status = child.wait();
        done.store(true, Ordering::SeqCst);
        let output = errors.join().unwrap_or_default();
        let _ = tell.unbounded_send(Heard::Ended { ok: status.is_ok_and(|s| s.success()), output });
    });
    Ok((running, heard))
}

/// A line of JSON to the host.
fn tell_host(stdin: &Mutex<Option<std::process::ChildStdin>>, value: &Value) {
    if let Some(stdin) = stdin.lock().unwrap().as_mut() {
        let mut line = serde_json::to_vec(value).unwrap_or_default();
        line.push(b'\n');
        let _ = stdin.write_all(&line).and_then(|_| stdin.flush());
    }
}

/// What a script asked F2 to do, done, if its manifest allows it.
fn asked(allowed: &Allowed, call: &Value) -> Result<Value, String> {
    let text = |key: &str| call.get(key).and_then(Value::as_str).map(str::to_string);
    match call.get("op").and_then(Value::as_str).unwrap_or_default() {
        "fetch" => {
            let request = serde_json::json!({ "url": call.get("url"), "method": call.get("method"), "headers": call.get("headers"), "body": call.get("body") });
            let answer = crate::macos::extension_fetch(&request.to_string(), &serde_json::to_string(&allowed.network).unwrap_or_default());
            let answer: Value = serde_json::from_str(&answer).map_err(|_| "No answer.".to_string())?;
            match answer.get("error").and_then(Value::as_str) {
                Some(why) => Err(why.to_string()),
                None => Ok(answer),
            }
        }
        op @ ("storage.get" | "storage.set" | "storage.delete") => {
            let key = text("key").ok_or("A key is a string.")?;
            storage(&allowed.kept_as, op, &key, call.get("value").cloned().unwrap_or(Value::Null))
        }
        op => Err(format!("F2 doesn't do \"{op}\".")),
    }
}

/// What each extension keeps between runs (`f2.storage`): a JSON file of
/// its own, beside the settings, of 5 MB at most.
fn storage(id: &str, op: &str, key: &str, value: Value) -> Result<Value, String> {
    storage_in(&crate::settings::data_dir().join("Extension Storage"), id, op, key, value)
}

fn storage_in(dir: &Path, id: &str, op: &str, key: &str, value: Value) -> Result<Value, String> {
    static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());
    let _held = ONE_AT_A_TIME.lock().unwrap();
    let file = dir.join(format!("{id}.json"));
    let mut kept: serde_json::Map<String, Value> = std::fs::read(&file).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    match op {
        "storage.get" => return Ok(kept.get(key).cloned().unwrap_or(Value::Null)),
        "storage.set" => {
            kept.insert(key.to_string(), value);
        }
        _ => {
            kept.remove(key);
        }
    }
    let bytes = serde_json::to_vec(&kept).map_err(|e| e.to_string())?;
    if bytes.len() > 5 << 20 {
        return Err("Its storage would be over 5 MB.".into());
    }
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::fs::write(&file, bytes).map_err(|e| e.to_string())?;
    Ok(Value::Null)
}

/// `name` as a program to run: in the extension's folder, or on `path`.
fn find_program(name: &str, dir: &Path, path: &str) -> Option<PathBuf> {
    let runnable = |p: &Path| std::fs::metadata(p).is_ok_and(|m| m.is_file() && std::os::unix::fs::PermissionsExt::mode(&m.permissions()) & 0o111 != 0);
    if name.contains('/') {
        let p = if Path::new(name).is_absolute() { PathBuf::from(name) } else { dir.join(name) };
        return runnable(&p).then_some(p);
    }
    let here = dir.join(name);
    if runnable(&here) {
        return Some(here);
    }
    path.split(':').map(|d| Path::new(d).join(name)).find(|p| runnable(p))
}

/// The PATH programs run with: your shell's (a login shell's, asked once),
/// and where the usual tools go besides (Homebrew's, bun's…), as an app
/// opened from the Dock gets only the system's.
pub fn path() -> String {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        let home = std::env::var("HOME").unwrap_or_default();
        let mut dirs: Vec<String> = login_path().map(|p| p.split(':').filter(|d| !d.is_empty()).map(String::from).collect()).unwrap_or_default();
        let usual = ["/opt/homebrew/bin", "/opt/homebrew/sbin", "/usr/local/bin", "~/.bun/bin", "~/.deno/bin", "~/.local/bin", "~/.cargo/bin", "/usr/bin", "/bin", "/usr/sbin", "/sbin"];
        for dir in usual {
            let dir = dir.replacen('~', &home, 1);
            if !dirs.contains(&dir) && Path::new(&dir).is_dir() {
                dirs.push(dir);
            }
        }
        dirs.join(":")
    })
    .clone()
}

/// A login shell's PATH (not an interactive one's: that could start
/// anything), in a few seconds at most.
fn login_path() -> Option<String> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
    let mut child = Command::new(shell)
        .args(["-lc", "printf '\\n__F2_PATH__%s__F2_PATH__\\n' \"$PATH\""])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .ok()?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() < Duration::from_secs(3) => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                unsafe { libc::kill(-(child.id() as i32), libc::SIGKILL) };
                let _ = child.wait();
                return None;
            }
        }
    }
    let mut out = String::new();
    child.stdout.take()?.read_to_string(&mut out).ok()?;
    let (_, rest) = out.split_once("__F2_PATH__")?;
    let (path, _) = rest.split_once("__F2_PATH__")?;
    Some(path.to_string())
}

/// The PATH worked out ahead, off the main thread, so the first run isn't
/// held up by a slow shell.
pub fn warm() {
    std::thread::spawn(|| {
        path();
    });
}

// What a page showed last: shown at once next time, while it's asked again.

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Shown {
    /// When (seconds since 1970).
    pub at: u64,
    pub title: Option<String>,
    pub placeholder: Option<String>,
    /// Each in an Arc: shared by what's kept and the pages open on it,
    /// not copied for each.
    pub items: Vec<Arc<Row>>,
    pub detail: Option<Detail>,
}

impl Shown {
    pub fn age(&self) -> Duration {
        Duration::from_secs(now().saturating_sub(self.at))
    }
}

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn cache_key(ext: &str, command: &str, args: &Value) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    serde_json::to_string(args).unwrap_or_default().hash(&mut hasher);
    // (A "+" after its id: not in one, so one's never taken for another's.)
    format!("{ext}+{command}-{:016x}", hasher.finish())
}

fn cache_file(key: &str) -> PathBuf {
    crate::settings::data_dir().join("Extension Cache").join(format!("{key}.json"))
}

fn memory() -> &'static Mutex<HashMap<String, Arc<Shown>>> {
    static MEMORY: OnceLock<Mutex<HashMap<String, Arc<Shown>>>> = OnceLock::new();
    MEMORY.get_or_init(Default::default)
}

/// What `command` (with `args`) showed last, if anything.
pub fn cached(ext: &str, command: &str, args: &Value) -> Option<Arc<Shown>> {
    let key = cache_key(ext, command, args);
    if let Some(shown) = memory().lock().unwrap().get(&key) {
        return Some(shown.clone());
    }
    let shown: Shown = serde_json::from_slice(&std::fs::read(cache_file(&key)).ok()?).ok()?;
    let shown = Arc::new(shown);
    memory().lock().unwrap().insert(key, shown.clone());
    Some(shown)
}

/// What `command` showed, kept (on disk too, off the main thread).
pub fn keep(ext: &str, command: &str, args: &Value, shown: Shown) {
    let key = cache_key(ext, command, args);
    let file = cache_file(&key);
    let bytes = serde_json::to_vec(&shown).unwrap_or_default();
    memory().lock().unwrap().insert(key, Arc::new(shown));
    std::thread::spawn(move || {
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(file, bytes);
    });
}

/// What a page is given by what's heard, folded in (taken from it, not
/// copied): its rows (all of them anew, or more), its detail, its title.
/// Whether that changed what's shown.
pub fn fold(shown: &mut Shown, message: &mut Message, stale: &mut bool) -> bool {
    let mut changed = false;
    if let Some(title) = message.title.take() {
        shown.title = Some(title);
        changed = true;
    }
    if let Some(placeholder) = message.placeholder.take() {
        shown.placeholder = Some(placeholder);
    }
    if let Some(items) = message.items.take() {
        shown.items = items.into_iter().map(Arc::new).collect();
        shown.detail = None;
        *stale = false;
        changed = true;
    }
    if let Some(more) = message.append.take() {
        if std::mem::take(stale) {
            shown.items.clear();
            shown.detail = None;
        }
        shown.items.extend(more.into_iter().map(Arc::new));
        changed = true;
    }
    if let Some(detail) = message.detail.take() {
        shown.detail = Some(detail);
        shown.items.clear();
        *stale = false;
        changed = true;
    }
    changed
}

/// A root command's rows kept fresh while Quick Open's away: run if what
/// it showed is older than it stays fresh for (an hour, if it doesn't
/// say), in the background, what it shows kept. Once at a time each.
pub fn freshen(ext: Arc<Extension>, spec: Spec) {
    static GOING: Mutex<Vec<String>> = Mutex::new(Vec::new());
    std::thread::spawn(move || {
        let args = Value::Null;
        let fresh_for = spec.fresh_for().unwrap_or(Duration::from_secs(3600));
        if cached(&ext.id, &spec.id, &args).is_some_and(|s| s.age() < fresh_for) || !ext.may_run() || !ext.missing().is_empty() {
            return;
        }
        let key = cache_key(&ext.id, &spec.id, &args);
        {
            let mut going = GOING.lock().unwrap();
            if going.contains(&key) {
                return;
            }
            going.push(key.clone());
        }
        if let Ok((running, heard)) = call(&ext, &spec.id, &args, "") {
            let mut shown = Shown::default();
            let (mut stale, mut failed) = (true, false);
            for heard in futures::executor::block_on_stream(heard) {
                match heard {
                    Heard::Said(mut message) => {
                        failed |= message.error.is_some();
                        fold(&mut shown, &mut message, &mut stale);
                    }
                    Heard::Text(_) | Heard::Garbled(_) => failed = true,
                    Heard::Ended { ok, .. } => {
                        if ok && !failed && !stale {
                            shown.at = now();
                            keep(&ext.id, &spec.id, &args, shown);
                        }
                        break;
                    }
                }
            }
            drop(running);
        }
        GOING.lock().unwrap().retain(|k| *k != key);
    });
}

/// A color from "#rrggbb" (or "#rgb", or a few names).
pub fn color(text: &str) -> Option<u32> {
    let named = match text.to_lowercase().as_str() {
        "red" => Some(0xff453a),
        "orange" => Some(0xff9f0a),
        "yellow" => Some(0xffd60a),
        "green" => Some(0x30d158),
        "mint" => Some(0x63e6e2),
        "teal" => Some(0x40c8e0),
        "blue" => Some(0x0a84ff),
        "indigo" => Some(0x5e5ce6),
        "purple" => Some(0xbf5af2),
        "pink" => Some(0xff375f),
        "gray" | "grey" => Some(0x8e8e93),
        "brown" => Some(0xac8e68),
        _ => None,
    };
    if named.is_some() {
        return named;
    }
    let hex = text.trim().strip_prefix('#')?;
    match hex.len() {
        6 => u32::from_str_radix(hex, 16).ok(),
        3 => {
            let v = u32::from_str_radix(hex, 16).ok()?;
            let (r, g, b) = ((v >> 8) & 0xf, (v >> 4) & 0xf, v & 0xf);
            Some((r * 17) << 16 | (g * 17) << 8 | b * 17)
        }
        _ => None,
    }
}

/// A symbol's name kept for good, as GPUI's symbols want theirs (a few
/// dozen names at most).
pub fn interned(name: &str) -> &'static str {
    static NAMES: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
    let mut names = NAMES.lock().unwrap();
    if let Some(found) = names.iter().find(|n| **n == name) {
        return found;
    }
    let kept: &'static str = Box::leak(name.to_string().into_boxed_str());
    names.push(kept);
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    fn heard(out: &str) -> Vec<String> {
        let mut said = Vec::new();
        read_messages(out.as_bytes(), |h| {
            said.push(match h {
                Heard::Said(m) => format!(
                    "said loading={:?} items={} append={} detail={} notice={}",
                    m.loading.map(|l| match l {
                        Loading::Said(s) => s,
                        Loading::On(b) => b.to_string(),
                    }),
                    m.items.map_or(0, |i| i.len()),
                    m.append.map_or(0, |i| i.len()),
                    m.detail.is_some(),
                    m.notice.is_some()
                ),
                Heard::Text(text) => format!("text {}", text.trim()),
                Heard::Garbled(text) => format!("garbled {}", text.trim()),
                Heard::Ended { .. } => "ended".into(),
            });
            true
        });
        said
    }

    #[test]
    fn messages_as_they_come() {
        // One a line, or pretty-printed, or one after another on a line.
        let out = "{\"loading\": \"Fetching…\"}\n{\"append\": [{\"title\": \"a\"}]}{\"append\": [{\"title\": \"b\"}]}\n{\n  \"items\": [{\"title\": \"a\"}, {\"title\": \"b\"}],\n  \"loading\": false\n}\n";
        assert_eq!(
            heard(out),
            [
                "said loading=Some(\"Fetching…\") items=0 append=0 detail=false notice=false",
                "said loading=None items=0 append=1 detail=false notice=false",
                "said loading=None items=0 append=1 detail=false notice=false",
                "said loading=Some(\"false\") items=2 append=0 detail=false notice=false",
            ]
        );
        // Plain text: all of it, once.
        assert_eq!(heard("Done.\nAll of it.\n"), ["text Done.\nAll of it."]);
        // JSON gone wrong halfway: what came before, then why.
        let said = heard("{\"notice\": \"hi\"}\n{\"items\": [");
        assert_eq!(said[0], "said loading=None items=0 append=0 detail=false notice=true");
        assert!(said[1].starts_with("garbled Output that isn't JSON"));
        // A message with the wrong shape.
        assert!(heard("{\"items\": 3}")[0].starts_with("garbled A message F2 doesn't understand"));
        assert!(heard("").is_empty());
    }

    #[test]
    fn rows_folded_in() {
        let msg = |json: &str| serde_json::from_str::<Message>(json).unwrap();
        // What was shown before (cached) stays till the first rows come,
        // then goes: the rest are added to those.
        let mut shown = Shown { items: vec![Arc::new(Row { title: "old".into(), ..Default::default() })], ..Default::default() };
        let mut stale = true;
        assert!(!fold(&mut shown, &mut msg("{\"loading\": \"…\"}"), &mut stale));
        assert_eq!(shown.items.len(), 1);
        fold(&mut shown, &mut msg("{\"append\": [{\"title\": \"a\"}]}"), &mut stale);
        fold(&mut shown, &mut msg("{\"append\": [{\"title\": \"b\"}]}"), &mut stale);
        assert_eq!(shown.items.iter().map(|r| r.title.as_str()).collect::<Vec<_>>(), ["a", "b"]);
        assert!(!stale);
        fold(&mut shown, &mut msg("{\"detail\": {\"markdown\": \"# Hi\"}, \"title\": \"Page\"}"), &mut stale);
        assert!(shown.items.is_empty() && shown.detail.is_some());
        assert_eq!(shown.title.as_deref(), Some("Page"));
    }

    #[test]
    fn manifests() {
        let m: Manifest = serde_json::from_str(
            r##"{"api": 1, "id": "f2.github", "name": "GitHub", "run": ["python3", "main.py"], "color": "#24292f",
                "commands": [{"id": "repos", "title": "Search Repos", "keyword": "gh", "refresh": "30m", "root": true},
                             {"id": "prs", "title": "Pull Requests", "hidden": true, "search": "script"}]}"##,
        )
        .unwrap();
        assert_eq!(m.argv(), ["python3", "main.py"]);
        assert_eq!(m.commands[0].fresh_for(), Some(Duration::from_secs(1800)));
        assert_eq!(m.commands[1].search, Search::Script);
        assert!(m.commands[1].hidden && m.commands[0].root);
        assert_eq!(checked(m, Path::new("/")).unwrap().0, "f2.github");
        let line: Manifest = serde_json::from_str(r#"{"name": "X", "run": "bun index.ts"}"#).unwrap();
        assert_eq!(line.argv(), ["bun", "index.ts"]);
        assert_eq!(duration("2h"), Some(Duration::from_secs(7200)));
        assert_eq!(duration("soon"), None);
        assert_eq!(color("#24292f"), Some(0x24292f));
        assert_eq!(color("#fff"), Some(0xffffff));
        assert_eq!(color("green"), Some(0x30d158));
    }

    #[test]
    fn manifests_say_what_they_are() {
        let check = |json: &str| checked(serde_json::from_str(json).unwrap(), Path::new("/")).map(|(id, _)| id);
        assert_eq!(check(r#"{"api": 1, "id": "jane.linear", "name": "L", "run": "x"}"#).unwrap(), "jane.linear");
        assert_eq!(check(r#"{"api": 1, "id": "j-2.my-ext", "name": "L", "run": "x"}"#).unwrap(), "j-2.my-ext");
        // Which API, and that this F2 has it.
        assert!(check(r#"{"id": "jane.linear", "name": "L", "run": "x"}"#).unwrap_err().contains("\"api\": 1"));
        assert!(check(r#"{"api": 2, "id": "jane.linear", "name": "L", "run": "x"}"#).unwrap_err().contains("newer F2"));
        // Whose, and what.
        assert!(check(r#"{"api": 1, "name": "L", "run": "x"}"#).unwrap_err().contains("no \"id\""));
        for id in ["linear", "Jane.linear", "jane.lin.ear", ".linear", "jane.", "jane.-linear", "jane linear.x", "jane.línear"] {
            assert!(check(&format!(r#"{{"api": 1, "id": "{id}", "name": "L", "run": "x"}}"#)).unwrap_err().contains("isn't an id"), "{id}");
        }
        // Something to run.
        assert!(check(r#"{"api": 1, "id": "jane.linear", "name": "L", "run": []}"#).unwrap_err().contains("nothing to run"));
        assert!(check(r#"{"api": 1, "id": "jane.linear", "name": "L", "runtime": "javascript", "main": "nope.js"}"#).unwrap_err().contains("isn't in its folder"));
    }

    #[test]
    fn new_ones_made_ready_to_run() {
        let dir = std::env::temp_dir().join(format!("f2-ext-new-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // Its id: whose, then its name's; its name as it is, quotes and all.
        let (id, folder) = create_in(&dir, "  My \"Quoted\" Thing! ", "jane", &[]).unwrap();
        assert_eq!(id, "jane.my-quoted-thing");
        assert_eq!(folder, dir.join("my-quoted-thing"));
        let manifest: Manifest = serde_json::from_slice(&std::fs::read(folder.join("f2.json")).unwrap()).unwrap();
        assert_eq!(manifest.name, "My \"Quoted\" Thing!");
        assert_eq!(manifest.commands[0].title, "My \"Quoted\" Thing!");
        let (checked_id, manifest) = checked(manifest, &folder).unwrap();
        assert_eq!(checked_id, id);
        assert!(manifest.javascript());
        for file in ["index.js", "jsconfig.json", "README.md", "f2.d.ts", "f2.schema.json", "EXTENSIONS.md"] {
            assert!(folder.join(file).is_file(), "{file}");
        }
        assert!(std::fs::read_to_string(folder.join("index.js")).unwrap().contains("What My \"Quoted\" Thing!'s commands show"));
        // Another of the name, or of an id that's taken: one of its own.
        assert_eq!(create_in(&dir, "My Quoted Thing", "jane", &[]).unwrap().0, "jane.my-quoted-thing-2");
        assert_eq!(create_in(&dir, "Linear", "jane", &["jane.linear".into()]).unwrap().0, "jane.linear-2");
        assert!(create_in(&dir, " ¿? ", "jane", &[]).is_err());
        assert_eq!(slug("Halló Heimur"), "hall-heimur");
        assert_eq!(slug("--GitHub  PRs--"), "github-prs");
        // Saved, it's changed.
        let ext = Extension { id, dir: folder.clone(), manifest, native: None };
        let was = ext.changed();
        std::fs::File::options().write(true).open(folder.join("index.js")).unwrap().set_modified(was + Duration::from_secs(5)).unwrap();
        assert!(ext.changed() > was);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn manifests_are_as_the_schema_says() {
        // (Every field of every manifest here, the template's too, is one
        // the schema knows: it's kept up to date.)
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("extensions");
        let schema: Value = serde_json::from_slice(&std::fs::read(root.join("js/f2.schema.json")).unwrap()).unwrap();
        let known = |at: &Value| at["properties"].as_object().unwrap().keys().cloned().collect::<Vec<_>>();
        let (top, command, preference) = (known(&schema), known(&schema["definitions"]["command"]), known(&schema["definitions"]["preference"]));
        // (The template, the programs, and any in js/: the store's own are
        // in its repo, github.com/getf2/extensions, checked there.)
        let mut manifests: Vec<String> = ["template", "github", "cloudflare"].map(String::from).to_vec();
        let mut ids = Vec::new();
        for dir in std::fs::read_dir(root.join("js")).unwrap().flatten().map(|e| e.path()).filter(|p| p.join("f2.json").is_file()) {
            let name = format!("js/{}", dir.file_name().unwrap().to_string_lossy());
            // (Each runs as F2 reads it, by an id of its own.)
            let manifest: Manifest = serde_json::from_slice(&std::fs::read(dir.join("f2.json")).unwrap()).unwrap();
            let (id, manifest) = checked(manifest, &dir).unwrap_or_else(|why| panic!("{name}: {why}"));
            assert!(manifest.javascript(), "{name}");
            assert!(!ids.contains(&id), "{name}: {id} twice");
            ids.push(id);
            manifests.push(name);
        }
        for m in manifests {
            let manifest: Value = serde_json::from_slice(&std::fs::read(root.join(&m).join("f2.json")).unwrap()).unwrap();
            let unknown = |keys: &serde_json::Map<String, Value>, known: &[String]| keys.keys().filter(|k| !known.contains(k)).cloned().collect::<Vec<_>>();
            assert!(unknown(manifest.as_object().unwrap(), &top).is_empty(), "{m}");
            for c in manifest["commands"].as_array().unwrap() {
                assert!(unknown(c.as_object().unwrap(), &command).is_empty(), "{m}");
            }
            for p in manifest["preferences"].as_array().into_iter().flatten() {
                assert!(unknown(p.as_object().unwrap(), &preference).is_empty(), "{m}");
            }
        }
    }

    #[test]
    fn schema_on_the_site_is_this() {
        // (getf2.com/schemas/f2.json, where the schema says it's at: the
        // site's copy, kept the same.)
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert_eq!(
            std::fs::read(root.join("site/public/schemas/f2.json")).unwrap(),
            std::fs::read(root.join("extensions/js/f2.schema.json")).unwrap(),
            "copy extensions/js/f2.schema.json to site/public/schemas/f2.json"
        );
    }

    #[test]
    fn programs_run_only_when_allowed() {
        let program = |id: &str, dir: &str, run: &str| Extension { id: id.into(), dir: dir.into(), manifest: serde_json::from_str(run).unwrap(), native: None };
        let github = program("f2.github", "/x/github", r#"{"name": "G", "run": "python3 main.py"}"#);
        let npm = program("f2.npm", "/x/npm", r#"{"name": "N", "runtime": "javascript"}"#);
        let mut synced = Synced::default();
        // Scripts, always; programs, not without Developer Mode…
        assert!(npm.may_run_by(&synced) && !github.may_run_by(&synced));
        synced.allowed.insert("f2.github".into(), "/x/github".into());
        assert!(!github.may_run_by(&synced));
        // …and then only once they're allowed, where they were.
        synced.developer = true;
        assert!(github.may_run_by(&synced));
        let moved = program("f2.github", "/y/github", r#"{"name": "G", "run": "python3 main.py"}"#);
        assert!(!moved.may_run_by(&synced));
        synced.allowed.clear();
        assert!(!github.may_run_by(&synced));
        // What they keep is kept as theirs, a local one's.
        assert_eq!(github.service(), "F2 Extension local.f2.github");
        assert_ne!(cache_key("f2.cf", "x", &Value::Null)[..6], cache_key("f2.cf-old", "x", &Value::Null)[..6]);
    }

    #[test]
    fn preferences_set_or_not() {
        let ext = Extension {
            id: "prefs-test".into(),
            dir: std::env::temp_dir(),
            manifest: serde_json::from_str(
                r#"{"name": "P", "run": "x",
                    "preferences": [
                        {"id": "size", "title": "Results", "type": "dropdown", "options": [{"value": "10", "title": "Ten"}, "25"], "default": "25"},
                        {"id": "pages", "title": "Pages", "type": "checkbox"},
                        {"id": "name", "title": "Name", "required": true},
                        {"id": "limit", "title": "Limit", "type": "number", "default": 5}
                    ]}"#,
            )
            .unwrap(),
            native: None,
        };
        let prefs = ext.preferences();
        assert_eq!(prefs.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["size", "pages", "name", "limit"]);
        assert_eq!(prefs[0].options[0].title(), "Ten");
        assert_eq!(prefs[0].options[1].value(), "25");
        // Nothing set: the defaults (a checkbox off), and what's required missing.
        let mut settings = crate::settings::Settings::default();
        sync(&settings);
        let values = ext.values();
        assert_eq!(values["size"], "25");
        assert_eq!(values["pages"], false);
        assert_eq!(values["limit"], 5);
        assert!(!values.contains_key("name"));
        assert_eq!(ext.missing().iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["name"]);
        // Set: those; an empty text is as good as none.
        let mut set = std::collections::BTreeMap::new();
        set.insert("size".to_string(), serde_json::json!("10"));
        set.insert("pages".to_string(), serde_json::json!(true));
        set.insert("name".to_string(), serde_json::json!("  "));
        settings.extension_preferences.insert("prefs-test".into(), set.clone());
        sync(&settings);
        assert_eq!(ext.values()["size"], "10");
        assert_eq!(ext.values()["pages"], true);
        assert_eq!(ext.missing().len(), 1);
        set.insert("name".to_string(), serde_json::json!("Ana"));
        settings.extension_preferences.insert("prefs-test".into(), set);
        sync(&settings);
        assert!(ext.missing().is_empty());
        // Turned off, and on again.
        assert!(enabled("prefs-test"));
        settings.disabled_extensions.push("prefs-test".into());
        sync(&settings);
        assert!(!enabled("prefs-test"));
        sync(&crate::settings::Settings::default());
        assert!(enabled("prefs-test"));
    }

    #[test]
    fn scripts_only_get_what_their_manifests_allow() {
        let allowed = Allowed { kept_as: "local.test.nothing".into(), network: vec!["api.example.com".into(), "*.example.org".into()] };
        let fetch = |url: &str| asked(&allowed, &serde_json::json!({ "op": "fetch", "url": url, "method": "GET" }));
        // (All refused before anything's sent.)
        assert!(fetch("https://evil.com/steal").unwrap_err().contains("isn't one of the hosts"));
        assert!(fetch("https://api.example.com.evil.com/").unwrap_err().contains("isn't one of the hosts"));
        assert!(fetch("https://notexample.org/").unwrap_err().contains("isn't one of the hosts"));
        assert!(fetch("http://api.example.com/").unwrap_err().contains("Only https"));
        assert!(fetch("file:///etc/passwd").unwrap_err().contains("Only https"));
        assert!(fetch("not a url").is_err());
        // (Its passwords come with its preferences: there's nothing to ask.)
        assert!(asked(&allowed, &serde_json::json!({ "op": "secret", "id": "TOKEN" })).unwrap_err().contains("doesn't do"));
        assert!(asked(&allowed, &serde_json::json!({ "op": "run", "command": "rm" })).unwrap_err().contains("doesn't do"));
    }

    #[test]
    fn storage_kept_by_key() {
        let dir = std::env::temp_dir().join(format!("f2-ext-storage-{}", std::process::id()));
        let put = |op: &str, key: &str, value: Value| storage_in(&dir, "x", op, key, value).unwrap();
        assert_eq!(put("storage.get", "a", Value::Null), Value::Null);
        put("storage.set", "a", serde_json::json!({ "n": 1 }));
        assert_eq!(put("storage.get", "a", Value::Null), serde_json::json!({ "n": 1 }));
        put("storage.delete", "a", Value::Null);
        assert_eq!(put("storage.get", "a", Value::Null), Value::Null);
        assert!(storage_in(&dir, "x", "storage.set", "big", Value::String("x".repeat(6 << 20))).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn runs_and_streams() {
        // A program in its folder: what it's asked is on its stdin, and
        // what it says comes as it says it; its stderr is kept.
        let dir = std::env::temp_dir().join(format!("f2-ext-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("main.sh");
        std::fs::write(&script, "#!/bin/sh\ncat > asked.json\necho '{\"loading\": \"Working\"}'\necho \"{\\\"items\\\": [{\\\"title\\\": \\\"$F2_COMMAND\\\"}]}\"\necho oops >&2\n").unwrap();
        std::fs::set_permissions(&script, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        let ext = Extension { id: "test".into(), dir: dir.clone(), manifest: serde_json::from_str(r#"{"name": "Test", "run": "./main.sh"}"#).unwrap(), native: None };
        // (Not allowed to run, it isn't.)
        assert!(call(&ext, "list", &Value::Null, "").err().unwrap().contains("Developer Mode"));
        let (_running, heard) = call_program(&ext, "list", &serde_json::json!({"a": 1}), "hello").unwrap();
        let heard: Vec<Heard> = futures::executor::block_on_stream(heard).collect();
        let Heard::Said(first) = &heard[0] else { panic!("not a message") };
        assert!(matches!(&first.loading, Some(Loading::Said(s)) if s == "Working"));
        let Heard::Said(second) = &heard[1] else { panic!("not a message") };
        assert_eq!(second.items.as_ref().unwrap()[0].title, "list");
        let asked: Value = serde_json::from_slice(&std::fs::read(dir.join("asked.json")).unwrap()).unwrap();
        assert_eq!(asked["query"], "hello");
        assert_eq!(asked["args"]["a"], 1);
        assert_eq!(asked["command"], "list");
        let Heard::Ended { ok, output } = &heard[2] else { panic!("not the end") };
        assert!(ok);
        assert_eq!(output.trim(), "oops");
        // One that isn't there says so.
        let missing = Extension { id: "x".into(), dir: dir.clone(), manifest: serde_json::from_str(r#"{"name": "X", "run": "./nope.sh"}"#).unwrap(), native: None };
        assert!(call_program(&missing, "list", &Value::Null, "").err().unwrap().contains("Can't find"));
        // One stopped is stopped, what it started too.
        std::fs::write(&script, "#!/bin/sh\necho '{\"loading\": true}'\nsleep 30\n").unwrap();
        let (running, heard) = call_program(&ext, "list", &Value::Null, "").unwrap();
        let started = Instant::now();
        let mut heard = futures::executor::block_on_stream(heard);
        assert!(matches!(heard.next(), Some(Heard::Said(_))));
        running.stop();
        assert!(matches!(heard.next(), Some(Heard::Ended { ok: false, .. })));
        assert!(started.elapsed() < Duration::from_secs(5));
        let _ = std::fs::remove_dir_all(dir);
    }
}
