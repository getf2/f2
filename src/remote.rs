//! Servers over SSH in F2's windows (SFTP: see `sftp`). A server's folder
//! has a stand-in here, in the cache: `~/Library/Caches/F2/Servers/<server>/
//! <its path there>`, which windows show and go about as any folder, while
//! what's in it is listed from the server, and what's done to it (a new
//! folder, a name, deleting, copying in and out) is done there. A file
//! opened is copied to its stand-in first, so apps and previews read it
//! there; saved, it goes back to the server.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use crate::fileops::Progress;
use crate::listing::Entry;
use crate::sftp::{Dest, Error, Session};

/// The cache folder the stand-ins are in.
pub fn root() -> PathBuf {
    let home = std::env::var_os("HOME").map_or_else(|| PathBuf::from("/tmp"), PathBuf::from);
    home.join("Library/Caches/F2/Servers")
}

/// A server's name for its stand-in folder: ssh's destination, and a port
/// if one's given ("pi", "me@box.example.com", "box#2222").
fn key_of(dest: &Dest) -> String {
    let host = dest.host.replace('/', "_");
    match dest.port {
        Some(port) => format!("{host}#{port}"),
        None => host,
    }
}

/// Servers connected to (or tried), by key: ssh's options too (tests').
static KNOWN: Mutex<Vec<Dest>> = Mutex::new(Vec::new());

fn dest_of(key: &str) -> Dest {
    if let Some(d) = KNOWN.lock().unwrap().iter().find(|d| key_of(d) == key) {
        return d.clone();
    }
    match key.rsplit_once('#') {
        Some((host, port)) if port.parse::<u16>().is_ok() => Dest { host: host.into(), port: port.parse().ok(), options: Vec::new() },
        _ => Dest { host: key.into(), port: None, options: Vec::new() },
    }
}

/// Where a server's `path` stands in here.
pub fn local_of(dest: &Dest, path: &Path) -> PathBuf {
    let rel = path.strip_prefix("/").unwrap_or(path);
    root().join(key_of(dest)).join(rel)
}

/// A server's folder or file, as windows have it.
#[derive(Clone, Debug, PartialEq)]
pub struct Here {
    /// What it's called: ssh's destination.
    pub server: String,
    pub dest: Dest,
    /// Its path there.
    pub path: PathBuf,
}

/// `local` is a server's stand-in: which server, and its path there.
pub fn here(local: &Path) -> Option<Here> {
    let rest = local.strip_prefix(root()).ok()?;
    let mut parts = rest.components();
    let key = parts.next()?.as_os_str().to_string_lossy().into_owned();
    let path = Path::new("/").join(parts.as_path());
    let dest = dest_of(&key);
    Some(Here { server: dest.host.clone(), dest, path })
}

pub fn is_remote(local: &Path) -> bool {
    local.starts_with(root())
}

/// One server's connection, shared by its windows (a copy has its own).
struct Conn {
    dest: Dest,
    session: Mutex<Option<Session>>,
}

static CONNS: Mutex<Vec<Arc<Conn>>> = Mutex::new(Vec::new());

fn conn(dest: &Dest) -> Arc<Conn> {
    let mut conns = CONNS.lock().unwrap();
    if let Some(c) = conns.iter().find(|c| c.dest == *dest) {
        return c.clone();
    }
    let c = Arc::new(Conn { dest: dest.clone(), session: Mutex::new(None) });
    conns.push(c.clone());
    c
}

impl Conn {
    /// `f` on the session, connecting first if need be. A read that finds
    /// the connection gone (asleep, the network changed) connects again
    /// and tries once more; a change doesn't, as it may have happened.
    fn with<T>(&self, again: bool, mut f: impl FnMut(&mut Session) -> Result<T, Error>) -> Result<T, String> {
        let mut s = self.session.lock().unwrap_or_else(|e| e.into_inner());
        for tried in 0..2 {
            if s.is_none() {
                *s = Some(Session::connect(&self.dest).map_err(|e| e.to_string())?);
            }
            match f(s.as_mut().unwrap()) {
                Err(Error::Lost(why)) => {
                    *s = None;
                    if !again || tried == 1 {
                        return Err(why);
                    }
                }
                r => return r.map_err(|e| e.to_string()),
            }
        }
        unreachable!()
    }
}

/// Connect to `dest`: where its folder you log in to stands in here.
/// Waits (a login, maybe asked for): not on the main thread.
pub fn connect(dest: Dest) -> Result<PathBuf, String> {
    {
        let mut known = KNOWN.lock().unwrap();
        known.retain(|d| key_of(d) != key_of(&dest));
        known.push(dest.clone());
    }
    let home = conn(&dest).with(true, |s| s.realpath(Path::new(".")))?;
    let local = local_of(&dest, &home);
    let _ = std::fs::create_dir_all(&local);
    Ok(local)
}

/// What's in a server's folder (by its stand-in), as entries standing in
/// here.
pub fn list(local: &Path) -> Result<Vec<Entry>, String> {
    let here = here(local).ok_or("Not a server's folder.")?;
    let items = conn(&here.dest).with(true, |s| s.list(&here.path))?;
    Ok(items
        .into_iter()
        .map(|item| {
            let name = item.name.to_string_lossy().into_owned();
            Entry {
                path: local.join(&item.name),
                hidden: name.starts_with('.'),
                is_dir: item.attrs.is_dir() || item.to_dir,
                size: item.attrs.size.unwrap_or(0),
                mtime: item.attrs.mtime.unwrap_or(0) as i64,
                alias: item.attrs.is_link(),
                name: name.into(),
                ..Default::default()
            }
        })
        .collect())
}

/// A file stands in here as the server has it: copied, unless the copy's
/// already as the server's (its size and time the same). For opening it.
pub fn fetch(local: &Path) -> Result<(), String> {
    let here = here(local).ok_or("Not a server's file.")?;
    let attrs = conn(&here.dest).with(true, |s| s.stat(&here.path))?;
    if attrs.is_dir() {
        return Ok(());
    }
    if let Ok(m) = std::fs::metadata(local) {
        let mtime = m.modified().ok().and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok()).map(|d| d.as_secs() as u32);
        if Some(m.len()) == attrs.size && mtime == attrs.mtime {
            return Ok(());
        }
    }
    if let Some(parent) = local.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    // (Into a file beside it, then in its place: an app never opens half.)
    let part = local.with_file_name(format!(".{}.f2-part", local.file_name().unwrap_or_default().to_string_lossy()));
    let got = conn(&here.dest).with(true, |s| s.download(&here.path, &part, &mut |_| true))?;
    if let Some(mtime) = got.mtime {
        stamp(&part, mtime);
    }
    std::fs::rename(&part, local).map_err(|e| e.to_string())?;
    Ok(())
}

fn stamp(local: &Path, mtime: u32) {
    if let Ok(f) = std::fs::File::options().write(true).open(local) {
        let _ = f.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(mtime as u64));
    }
}

/// A new folder in a server's folder, "untitled folder" (or "untitled
/// folder 2"...): its stand-in.
pub fn new_folder(local_dir: &Path) -> Result<PathBuf, String> {
    let here = here(local_dir).ok_or("Not a server's folder.")?;
    let names: Vec<String> = list(local_dir)?.into_iter().map(|e| e.name.to_string()).collect();
    let name = (1..).map(|n| if n == 1 { "untitled folder".to_string() } else { format!("untitled folder {n}") }).find(|n| !names.contains(n)).unwrap();
    conn(&here.dest).with(false, |s| s.mkdir(&here.path.join(&name)))?;
    Ok(local_dir.join(name))
}

/// A new, empty file in a server's folder, "untitled" (or "untitled 2"...).
pub fn new_file(local_dir: &Path) -> Result<PathBuf, String> {
    let here = here(local_dir).ok_or("Not a server's folder.")?;
    let names: Vec<String> = list(local_dir)?.into_iter().map(|e| e.name.to_string()).collect();
    let name = (1..).map(|n| if n == 1 { "untitled".to_string() } else { format!("untitled {n}") }).find(|n| !names.contains(n)).unwrap();
    let empty = std::env::temp_dir().join(format!("f2-empty-{}", std::process::id()));
    std::fs::write(&empty, b"").map_err(|e| e.to_string())?;
    let made = conn(&here.dest).with(false, |s| s.upload(&empty, &here.path.join(&name), &mut |_| true));
    let _ = std::fs::remove_file(&empty);
    made?;
    Ok(local_dir.join(name))
}

/// Which of `paths` have names already taken in `dir` (a server's folder,
/// or the Mac's).
pub fn taken(paths: &[PathBuf], dir: &Path) -> Result<Vec<PathBuf>, String> {
    let names: Vec<String> = if is_remote(dir) {
        list(dir)?.into_iter().map(|e| e.name.to_string()).collect()
    } else {
        std::fs::read_dir(dir).map_err(|e| e.to_string())?.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()
    };
    Ok(paths.iter().filter(|p| p.file_name().is_some_and(|n| names.iter().any(|m| *m == n.to_string_lossy()))).cloned().collect())
}

/// Move server items into another folder on the same server (renamed
/// there).
pub fn move_within(locals: &[PathBuf], local_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let dir = here(local_dir).ok_or("Not a server's folder.")?;
    let mut made = Vec::new();
    for local in locals {
        let from = here(local).ok_or("Not a server's item.")?;
        let name = from.path.file_name().unwrap_or_default().to_os_string();
        if dir.path.starts_with(&from.path) {
            return Err("A folder can't go inside itself.".into());
        }
        conn(&dir.dest).with(false, |s| s.rename(&from.path, &dir.path.join(&name)))?;
        made.push(local_dir.join(name));
    }
    Ok(made)
}

/// Copy server items into a folder on a server (another, or the same):
/// down to a folder here for the while, then up.
pub fn server_to_server(locals: &[PathBuf], local_dir: &Path, progress: &Progress) -> Result<Vec<PathBuf>, String> {
    let tmp = std::env::temp_dir().join(format!("f2-between-{}-{}", std::process::id(), SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_nanos())));
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let result = download(locals, &tmp, progress).and_then(|down| upload(&down, local_dir, progress));
    let _ = std::fs::remove_dir_all(&tmp);
    result
}

/// A shell's single quotes round `s`.
fn quoted(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// A terminal (the app `app`) logged in to the server, in its folder.
pub fn open_terminal(at: &Here, app: &Path) -> Result<(), String> {
    terminal(&at.dest, Some(&at.path), app)
}

/// A terminal (the app `app`) logged in to `dest`, in `folder` there (else
/// where you log in).
pub fn terminal(dest: &Dest, folder: Option<&Path>, app: &Path) -> Result<(), String> {
    let script = terminal_script(dest, folder);
    let dir = root().parent().unwrap_or(Path::new("/tmp")).join("ssh");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file = dir.join(format!("{}.command", key_of(dest)));
    std::fs::write(&file, script).map_err(|e| e.to_string())?;
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    // (Each terminal takes a command its own way; Terminal and iTerm run a
    // .command file they're given.)
    let name = app.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    let args: Option<Vec<String>> = if name.contains("ghostty") || name.contains("alacritty") {
        Some(vec!["-e".into(), file.display().to_string()])
    } else if name.contains("kitty") {
        Some(vec![file.display().to_string()])
    } else if name.contains("wezterm") {
        Some(vec!["start".into(), "--".into(), file.display().to_string()])
    } else {
        None
    };
    let mut open = std::process::Command::new("/usr/bin/open");
    match args {
        Some(args) => open.arg("-na").arg(app).arg("--args").args(args),
        None => open.arg("-a").arg(app).arg(&file),
    };
    let status = open.status().map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("The terminal didn't open.".into());
    }
    Ok(())
}

/// The script a terminal runs for `terminal`: ssh, logged in, in `folder`.
fn terminal_script(dest: &Dest, folder: Option<&Path>) -> String {
    let at = Here { server: dest.host.clone(), dest: dest.clone(), path: folder.map(Path::to_path_buf).unwrap_or_default() };
    let mut ssh = vec!["/usr/bin/ssh".to_string(), "-t".into()];
    if let Some(config) = std::env::var_os("NEW_FINDER_SSH_CONFIG") {
        ssh.extend(["-F".to_string(), config.to_string_lossy().into_owned()]);
    }
    ssh.extend(at.dest.options.iter().cloned());
    if let Some(port) = at.dest.port {
        ssh.extend(["-p".to_string(), port.to_string()]);
    }
    ssh.extend(["--".to_string(), at.dest.host.clone()]);
    if !at.path.as_os_str().is_empty() {
        ssh.push(format!("cd {} && exec \"$SHELL\" -l", quoted(&at.path.to_string_lossy())));
    }
    format!("#!/bin/sh\nexec {}\n", ssh.iter().map(|a| quoted(a)).collect::<Vec<_>>().join(" "))
}

/// ~/.ssh/config's Hosts (see `hosts`), read again at most every few
/// seconds: the sidebar asks each time it's drawn.
pub fn hosts_now() -> Vec<String> {
    static READ: Mutex<Option<(std::time::Instant, Vec<String>)>> = Mutex::new(None);
    let mut read = READ.lock().unwrap();
    if let Some((at, hosts)) = read.as_ref()
        && at.elapsed() < Duration::from_secs(10)
    {
        return hosts.clone();
    }
    let found = hosts();
    *read = Some((std::time::Instant::now(), found.clone()));
    found
}

/// Rename a server's item (not over another).
pub fn rename(local: &Path, name: &str) -> Result<PathBuf, String> {
    let here = here(local).ok_or("Not a server's item.")?;
    if name.is_empty() || name.contains('/') {
        return Err("A name can't be empty or have a \"/\" in it.".into());
    }
    let to = here.path.with_file_name(name);
    conn(&here.dest).with(false, |s| match s.lstat(&to) {
        Ok(_) => Err(Error::Status(4, format!("There's already an item called \"{name}\" there."))),
        Err(_) => s.rename(&here.path, &to),
    })?;
    // (A copy here goes with it.)
    let moved = local.with_file_name(name);
    if local.exists() {
        let _ = std::fs::rename(local, &moved);
    }
    Ok(moved)
}

/// Delete server items for good (a server has no Trash), folders with all
/// that's in them.
pub fn delete(locals: &[PathBuf]) -> Result<(), String> {
    for local in locals {
        let here = here(local).ok_or("Not a server's item.")?;
        if here.path == Path::new("/") {
            return Err("That's the whole server.".into());
        }
        conn(&here.dest).with(false, |s| delete_tree(s, &here.path))?;
        let _ = if local.is_dir() { std::fs::remove_dir_all(local) } else { std::fs::remove_file(local) };
    }
    Ok(())
}

fn delete_tree(s: &mut Session, path: &Path) -> Result<(), Error> {
    let attrs = s.lstat(path)?;
    if !attrs.is_dir() {
        return s.remove(path);
    }
    for item in s.list(path)? {
        delete_tree(s, &path.join(&item.name))?;
    }
    s.rmdir(path)
}

/// A copy's bytes so far (of all of it, from `base`), and whether it's
/// been stopped.
fn going(progress: &Progress, base: u64, done: u64) -> bool {
    progress.done.store(base + done, std::sync::atomic::Ordering::Relaxed);
    !progress.cancel.load(std::sync::atomic::Ordering::Relaxed)
}

/// Copy server items (by their stand-ins) into the local folder `dir`,
/// folders with all that's in them; what's there already of the same name
/// is replaced. On a session of its own (browsing goes on meanwhile). The
/// paths made.
pub fn download(locals: &[PathBuf], dir: &Path, progress: &Progress) -> Result<Vec<PathBuf>, String> {
    let Some(first) = locals.first().and_then(|l| here(l)) else { return Ok(Vec::new()) };
    let mut s = Session::connect(&first.dest).map_err(|e| e.to_string())?;
    // What there is, first (for the total).
    let mut plan: Vec<(PathBuf, PathBuf, bool, u64)> = Vec::new();
    let mut made = Vec::new();
    for local in locals {
        let here = here(local).ok_or("Not a server's item.")?;
        let to = dir.join(here.path.file_name().unwrap_or_default());
        made.push(to.clone());
        walk(&mut s, &here.path, &to, &mut plan).map_err(|e| e.to_string())?;
    }
    progress.total.store(plan.iter().map(|p| p.3).sum(), std::sync::atomic::Ordering::Relaxed);
    let mut base = 0;
    for (from, to, folder, size) in plan {
        if folder {
            std::fs::create_dir_all(&to).map_err(|e| e.to_string())?;
            continue;
        }
        let attrs = s.download(&from, &to, &mut |done| going(progress, base, done)).map_err(|e| {
            let _ = std::fs::remove_file(&to);
            e.to_string()
        })?;
        if let Some(mtime) = attrs.mtime {
            stamp(&to, mtime);
        }
        base += size;
    }
    Ok(made)
}

/// Each folder (made before what's in it) and file under `from`, with
/// where it goes under `to`, and its size.
fn walk(s: &mut Session, from: &Path, to: &Path, plan: &mut Vec<(PathBuf, PathBuf, bool, u64)>) -> Result<(), Error> {
    let attrs = s.stat(from)?;
    if !attrs.is_dir() {
        plan.push((from.to_path_buf(), to.to_path_buf(), false, attrs.size.unwrap_or(0)));
        return Ok(());
    }
    plan.push((from.to_path_buf(), to.to_path_buf(), true, 0));
    for item in s.list(from)? {
        // (A link to a folder isn't gone into: it may lead round again.)
        if item.attrs.is_link() && item.to_dir {
            continue;
        }
        walk(s, &from.join(&item.name), &to.join(&item.name), plan)?;
    }
    Ok(())
}

/// Copy local items into a server's folder (by its stand-in), folders with
/// all that's in them; what's there of the same name is replaced. The
/// stand-ins of what's made.
pub fn upload(paths: &[PathBuf], local_dir: &Path, progress: &Progress) -> Result<Vec<PathBuf>, String> {
    let here = here(local_dir).ok_or("Not a server's folder.")?;
    let mut s = Session::connect(&here.dest).map_err(|e| e.to_string())?;
    let mut plan: Vec<(PathBuf, PathBuf, bool, u64)> = Vec::new();
    let mut made = Vec::new();
    for path in paths {
        let name = path.file_name().unwrap_or_default();
        made.push(local_dir.join(name));
        for entry in walkdir(path) {
            let rel = entry.0.strip_prefix(path.parent().unwrap_or(Path::new("/"))).unwrap_or(&entry.0).to_path_buf();
            plan.push((entry.0.clone(), here.path.join(rel), entry.1, entry.2));
        }
    }
    progress.total.store(plan.iter().map(|p| p.3).sum(), std::sync::atomic::Ordering::Relaxed);
    let mut base = 0;
    for (from, to, folder, size) in plan {
        if folder {
            match s.mkdir(&to) {
                Ok(()) => {}
                // (There already: what's copied goes in it.)
                Err(_) if s.stat(&to).is_ok_and(|a| a.is_dir()) => {}
                Err(e) => return Err(e.to_string()),
            }
            continue;
        }
        s.upload(&from, &to, &mut |done| going(progress, base, done)).map_err(|e| e.to_string())?;
        base += size;
    }
    Ok(made)
}

/// A local item and all under it (folders first): path, a folder, size.
/// Links aren't followed.
fn walkdir(path: &Path) -> Vec<(PathBuf, bool, u64)> {
    let mut out = Vec::new();
    let Ok(meta) = std::fs::symlink_metadata(path) else { return out };
    if !meta.is_dir() {
        if meta.is_file() {
            out.push((path.to_path_buf(), false, meta.len()));
        }
        return out;
    }
    out.push((path.to_path_buf(), true, 0));
    let mut children: Vec<PathBuf> = std::fs::read_dir(path).into_iter().flatten().flatten().map(|e| e.path()).collect();
    children.sort();
    for child in children {
        out.extend(walkdir(&child));
    }
    out
}

/// Files opened from servers, kept watching: saved here, they go back
/// (see `watch`). Their stand-in, and its time when last in step.
static OPENED: Mutex<Vec<(PathBuf, SystemTime)>> = Mutex::new(Vec::new());

/// Watch an opened file (just fetched): saved, it goes back to the server.
pub fn opened(local: &Path) {
    let Ok(time) = std::fs::metadata(local).and_then(|m| m.modified()) else { return };
    let mut opened = OPENED.lock().unwrap();
    opened.retain(|(p, _)| p != local);
    opened.push((local.to_path_buf(), time));
}

/// What happened to a file saved back: its stand-in, and an error if it
/// didn't go.
pub type Saved = (PathBuf, Result<(), String>);

/// Look at opened files now and then (every second): one saved (its time
/// changed, and then stayed a moment, so a save in progress isn't caught
/// halfway) is copied back to its server, and `told` hears of it.
pub fn watch(told: impl Fn(Saved) + Send + 'static) {
    std::thread::Builder::new()
        .name("f2-remote-saves".into())
        .spawn(move || {
            let mut seen: HashMap<PathBuf, SystemTime> = HashMap::new();
            loop {
                std::thread::sleep(Duration::from_secs(1));
                let files: Vec<(PathBuf, SystemTime)> = OPENED.lock().unwrap().clone();
                for (local, synced) in files {
                    let Ok(now) = std::fs::metadata(&local).and_then(|m| m.modified()) else { continue };
                    if now == synced {
                        seen.remove(&local);
                        continue;
                    }
                    // (Changed: once it's been the same for a second, it's saved.)
                    if seen.get(&local) != Some(&now) {
                        seen.insert(local.clone(), now);
                        continue;
                    }
                    seen.remove(&local);
                    let result = send_back(&local);
                    if result.is_ok()
                        && let Ok(time) = std::fs::metadata(&local).and_then(|m| m.modified())
                        && let Some(entry) = OPENED.lock().unwrap().iter_mut().find(|(p, _)| *p == local)
                    {
                        entry.1 = time;
                    }
                    told((local, result));
                }
            }
        })
        .ok();
}

fn send_back(local: &Path) -> Result<(), String> {
    let here = here(local).ok_or("Not a server's file.")?;
    let progress = Progress::default();
    let mut s = Session::connect(&here.dest).map_err(|e| e.to_string())?;
    s.upload(local, &here.path, &mut |done| going(&progress, 0, done)).map_err(|e| e.to_string())?;
    // (Its time as the server has it now, so it isn't fetched again.)
    if let Ok(attrs) = s.stat(&here.path)
        && let Some(mtime) = attrs.mtime
    {
        stamp(local, mtime);
    }
    Ok(())
}

/// The Hosts of ~/.ssh/config, and of the files it Includes (not patterns:
/// "*", "?" or "!"), in the order they're there.
pub fn hosts() -> Vec<String> {
    let home = std::env::var_os("HOME").map_or_else(|| PathBuf::from("/"), PathBuf::from);
    let mut out = Vec::new();
    let config = std::env::var_os("NEW_FINDER_SSH_CONFIG").map_or_else(|| home.join(".ssh/config"), PathBuf::from);
    read_hosts(&config, &home, &mut out, 0);
    let mut seen = std::collections::HashSet::new();
    out.retain(|h| seen.insert(h.clone()));
    out
}

fn read_hosts(file: &Path, home: &Path, out: &mut Vec<String>, depth: usize) {
    let Ok(text) = std::fs::read_to_string(file) else { return };
    for line in text.lines() {
        let line = line.trim();
        let (word, rest) = line.split_once(|c: char| c.is_whitespace() || c == '=').unwrap_or((line, ""));
        let rest = rest.trim().trim_start_matches('=').trim();
        if word.eq_ignore_ascii_case("host") {
            out.extend(rest.split_whitespace().filter(|h| !h.contains(['*', '?', '!'])).map(str::to_string));
        } else if word.eq_ignore_ascii_case("include") && depth < 4 {
            for pattern in rest.split_whitespace() {
                let path = match pattern.strip_prefix("~/") {
                    Some(r) => home.join(r),
                    None if pattern.starts_with('/') => PathBuf::from(pattern),
                    None => home.join(".ssh").join(pattern),
                };
                for found in glob(&path) {
                    read_hosts(&found, home, out, depth + 1);
                }
            }
        }
    }
}

/// The files `path` names, a "*" in its last part matching any name there.
fn glob(path: &Path) -> Vec<PathBuf> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let Some((before, after)) = name.split_once('*') else { return vec![path.to_path_buf()] };
    let mut found: Vec<PathBuf> = std::fs::read_dir(path.parent().unwrap_or(Path::new("/")))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().map(|n| n.to_string_lossy()).is_some_and(|n| n.starts_with(before) && n.ends_with(after)))
        .collect();
    found.sort();
    found
}

/// What's typed in Connect to Server ("ssh://me@box:2222/var/www", "sftp://pi",
/// or a Host of ~/.ssh/config), as ssh's destination and a folder to open
/// there. None if it's another kind of address (smb://...).
pub fn parse(address: &str) -> Option<(Dest, Option<PathBuf>)> {
    let a = address.trim();
    let rest = a.strip_prefix("ssh://").or_else(|| a.strip_prefix("sftp://"))?;
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], Some(PathBuf::from(&rest[i..])).filter(|p| p != Path::new("/"))),
        None => (rest, None),
    };
    // (A port after the host: "[::1]:22" or "box:2222".)
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) if !h.ends_with(']') || authority.starts_with('[') => match p.parse::<u16>() {
            Ok(port) => (h.trim_start_matches('[').trim_end_matches(']'), Some(port)),
            Err(_) => (authority, None),
        },
        _ => (authority, None),
    };
    if host.is_empty() {
        return None;
    }
    Some((Dest { host: host.to_string(), port, options: Vec::new() }, path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses() {
        let (d, p) = parse("ssh://me@box.example.com:2222/var/www").unwrap();
        assert_eq!((d.host.as_str(), d.port, p), ("me@box.example.com", Some(2222), Some(PathBuf::from("/var/www"))));
        let (d, p) = parse("sftp://pi").unwrap();
        assert_eq!((d.host.as_str(), d.port, p), ("pi", None, None));
        assert!(parse("smb://nas/share").is_none());
        assert!(parse("ssh://").is_none());
        let dest = Dest { host: "box".into(), port: Some(2222), options: Vec::new() };
        let local = local_of(&dest, Path::new("/home/me/x.txt"));
        assert!(local.ends_with("Servers/box#2222/home/me/x.txt"));
        let h = here(&local).unwrap();
        assert_eq!((h.dest.port, h.path), (Some(2222), PathBuf::from("/home/me/x.txt")));
        assert_eq!(here(&local_of(&dest, Path::new("/"))).unwrap().path, PathBuf::from("/"));
        assert!(here(Path::new("/Users")).is_none());
    }

    #[test]
    fn quoting() {
        assert_eq!(quoted("a b"), "'a b'");
        assert_eq!(quoted("it's"), "'it'\\''s'");
        let out = std::process::Command::new("/bin/sh").arg("-c").arg(format!("printf %s {}", quoted("x'y $HOME \"z\""))).output().unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout), "x'y $HOME \"z\"");
    }

    #[test]
    fn ssh_config_hosts() {
        let dir = std::env::temp_dir().join(format!("nf-sshconfig-{}", std::process::id()));
        std::fs::create_dir_all(dir.join(".ssh/conf.d")).unwrap();
        std::fs::write(dir.join(".ssh/config"), "Host pi box\n  User me\nHost *.internal !x\nInclude conf.d/*.conf\nHost=web\n").unwrap();
        std::fs::write(dir.join(".ssh/conf.d/a.conf"), "Host nas\n").unwrap();
        let mut out = Vec::new();
        read_hosts(&dir.join(".ssh/config"), &dir, &mut out, 0);
        assert_eq!(out, ["pi", "box", "nas", "web"]);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// The terminal's script logs in, in the folder (its name quoted: a
    /// space and a quote in it), against a throwaway sshd.
    /// `cargo test -- --ignored --nocapture terminal_script_logs_in`
    #[test]
    #[ignore]
    fn terminal_script_logs_in() {
        let srv = crate::sftp::tests::server("term");
        let folder = srv.dir.join("files/it's here");
        std::fs::create_dir_all(&folder).unwrap();
        let script = terminal_script(&srv.dest, Some(&folder));
        let file = srv.dir.join("ssh.command");
        std::fs::write(&file, &script).unwrap();
        let mut child = std::process::Command::new("/bin/sh")
            .arg(&file)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        use std::io::Write;
        child.stdin.take().unwrap().write_all(b"pwd\nexit\n").unwrap();
        let out = child.wait_with_output().unwrap();
        let said = String::from_utf8_lossy(&out.stdout);
        println!("{script}\n{said}");
        assert!(said.lines().any(|l| l.trim() == folder.to_string_lossy()), "logged in, in the folder");
    }

    /// A server's folders and files, end to end (a throwaway sshd: see
    /// `sftp::tests::server`): connected, listed, a folder made and named,
    /// a file opened (fetched), saved back, copied both ways, deleted.
    /// `cargo test -- --ignored --nocapture remote_end_to_end`
    #[test]
    #[ignore]
    fn remote_end_to_end() {
        let srv = crate::sftp::tests::server("remote");
        let files = srv.dir.join("files");
        std::fs::create_dir_all(files.join("docs")).unwrap();
        std::fs::write(files.join("docs/notes.txt"), b"first").unwrap();
        let home = connect(srv.dest.clone()).unwrap();
        assert!(is_remote(&home));
        let here_dir = local_of(&srv.dest, &files);
        let names = |dir: &Path| {
            let mut n: Vec<String> = list(dir).unwrap().into_iter().map(|e| format!("{}{}", e.name, if e.is_dir { "/" } else { "" })).collect();
            n.sort();
            n
        };
        assert_eq!(names(&here_dir), ["docs/"]);
        let made = new_folder(&here_dir).unwrap();
        assert!(files.join("untitled folder").is_dir());
        let renamed = rename(&made, "Renamed").unwrap();
        assert!(files.join("Renamed").is_dir() && renamed.ends_with("Renamed"));
        assert!(rename(&renamed, "docs").is_err(), "not over another");

        // Opened: fetched to its stand-in; saved, it goes back.
        let notes = here_dir.join("docs/notes.txt");
        fetch(&notes).unwrap();
        assert_eq!(std::fs::read(&notes).unwrap(), b"first");
        let (tx, rx) = std::sync::mpsc::channel();
        watch(move |saved| {
            let _ = tx.send(saved);
        });
        opened(&notes);
        std::thread::sleep(Duration::from_millis(1100));
        std::fs::write(&notes, b"edited here").unwrap();
        let (path, result) = rx.recv_timeout(Duration::from_secs(10)).unwrap();
        assert!(result.is_ok() && path == notes);
        assert_eq!(std::fs::read(files.join("docs/notes.txt")).unwrap(), b"edited here");
        // (Changed there: fetched again.)
        std::thread::sleep(Duration::from_millis(1100));
        std::fs::write(files.join("docs/notes.txt"), b"edited there").unwrap();
        fetch(&notes).unwrap();
        assert_eq!(std::fs::read(&notes).unwrap(), b"edited there");

        // Copied out, and in.
        let out = srv.dir.join("out");
        std::fs::create_dir_all(&out).unwrap();
        let progress = Progress::default();
        let got = download(&[here_dir.join("docs")], &out, &progress).unwrap();
        assert_eq!(got, [out.join("docs")]);
        assert_eq!(std::fs::read(out.join("docs/notes.txt")).unwrap(), b"edited there");
        std::fs::create_dir_all(out.join("upme/inner")).unwrap();
        std::fs::write(out.join("upme/inner/x.bin"), vec![7u8; 100_000]).unwrap();
        let put = upload(&[out.join("upme")], &here_dir, &progress).unwrap();
        assert_eq!(put, [here_dir.join("upme")]);
        assert_eq!(std::fs::read(files.join("upme/inner/x.bin")).unwrap().len(), 100_000);
        assert_eq!(progress.done.load(std::sync::atomic::Ordering::Relaxed), 100_000);

        delete(&[here_dir.join("upme"), here_dir.join("Renamed")]).unwrap();
        assert!(!files.join("upme").exists() && !files.join("Renamed").exists());
        assert!(delete(&[local_of(&srv.dest, Path::new("/"))]).is_err(), "not the whole server");
        let _ = std::fs::remove_dir_all(root().join(key_of(&srv.dest)));
    }
}
