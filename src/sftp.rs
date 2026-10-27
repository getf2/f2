//! SFTP (version 3, as OpenSSH's sftp-server speaks it) over the Mac's own
//! ssh, `ssh -s <host> sftp`: the user's ~/.ssh/config, keys, agent (1Password's
//! too) and known hosts all count, as in Terminal. A password, a key's
//! passphrase or a new host's key is asked for in F2's own dialog (see
//! `askpass`), as ssh has no terminal to ask in. Connections to a host share
//! one login (ssh's ControlMaster): a second session for a copy doesn't ask
//! again.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};

const INIT: u8 = 1;
const VERSION: u8 = 2;
const OPEN: u8 = 3;
const CLOSE: u8 = 4;
const READ: u8 = 5;
const WRITE: u8 = 6;
const LSTAT: u8 = 7;
const SETSTAT: u8 = 9;
const OPENDIR: u8 = 11;
const READDIR: u8 = 12;
const REMOVE: u8 = 13;
const MKDIR: u8 = 14;
const RMDIR: u8 = 15;
const REALPATH: u8 = 16;
const STAT: u8 = 17;
const RENAME: u8 = 18;
const STATUS: u8 = 101;
const HANDLE: u8 = 102;
const DATA: u8 = 103;
const NAME: u8 = 104;
const ATTRS: u8 = 105;
const EXTENDED: u8 = 200;

const FX_EOF: u32 = 1;
const FX_NO_SUCH_FILE: u32 = 2;
const FX_PERMISSION_DENIED: u32 = 3;

const ATTR_SIZE: u32 = 0x1;
const ATTR_UIDGID: u32 = 0x2;
const ATTR_PERMISSIONS: u32 = 0x4;
const ATTR_ACMODTIME: u32 = 0x8;
const ATTR_EXTENDED: u32 = 0x8000_0000;

const OPEN_READ: u32 = 0x1;
const OPEN_WRITE: u32 = 0x2;
const OPEN_CREATE: u32 = 0x8;
const OPEN_TRUNCATE: u32 = 0x10;

/// Bytes a read or write asks for, and how many are on their way at once
/// (OpenSSH's sftp: 32 KB, 64 of them).
const CHUNK: u32 = 32 * 1024;
const IN_FLIGHT: usize = 64;

/// A packet longer than this isn't SFTP: something printed first (a login
/// script on the server, say).
const LONGEST: u32 = 4 << 20;

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// The server's answer: no such file, permission denied...
    Status(u32, String),
    /// The connection's gone, or never was: ssh's own words, if it said any.
    Lost(String),
}

impl Error {
    pub fn missing(&self) -> bool {
        matches!(self, Error::Status(FX_NO_SUCH_FILE, _))
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Status(FX_NO_SUCH_FILE, _) => write!(f, "It isn't there."),
            Error::Status(FX_PERMISSION_DENIED, _) => write!(f, "The server doesn't allow it (permission denied)."),
            Error::Status(_, m) if !m.is_empty() => write!(f, "The server said: {m}"),
            Error::Status(code, _) => write!(f, "The server refused (error {code})."),
            Error::Lost(why) => write!(f, "{why}"),
        }
    }
}

type Result<T> = std::result::Result<T, Error>;

/// What the server says about a file (what it sent of it).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Attrs {
    pub size: Option<u64>,
    pub perm: Option<u32>,
    pub mtime: Option<u32>,
}

impl Attrs {
    pub fn is_dir(&self) -> bool {
        self.perm.is_some_and(|p| p & 0o170000 == 0o040000)
    }

    pub fn is_link(&self) -> bool {
        self.perm.is_some_and(|p| p & 0o170000 == 0o120000)
    }
}

/// A folder's item.
#[derive(Clone, Debug)]
pub struct Item {
    pub name: PathBuf,
    pub attrs: Attrs,
    /// A link to a folder (followed: its own attributes are the link's).
    pub to_dir: bool,
}

/// How to reach a server: ssh's destination (a Host of ~/.ssh/config, or
/// `user@host`), a port if not ssh's, and options for ssh (tests').
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Dest {
    pub host: String,
    pub port: Option<u16>,
    pub options: Vec<String>,
}

/// The program that asks for passwords (F2 itself, see `askpass`), or none:
/// then ssh may only use keys and the agent.
static ASKPASS: Mutex<Option<PathBuf>> = Mutex::new(None);

pub fn set_askpass(program: Option<PathBuf>) {
    *ASKPASS.lock().unwrap() = program;
}

/// Where ssh keeps the shared logins (one per host: see ControlMaster).
fn control_dir() -> PathBuf {
    let home = std::env::var_os("HOME").map_or_else(|| PathBuf::from("/tmp"), PathBuf::from);
    home.join("Library/Caches/F2/ssh")
}

/// The ssh command for `dest` (its subsystem `sftp`, or else `command`).
pub fn ssh_command(dest: &Dest, subsystem: bool) -> Command {
    let control = control_dir();
    if std::fs::create_dir_all(&control).is_ok() {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&control, std::fs::Permissions::from_mode(0o700));
    }
    let mut c = Command::new("/usr/bin/ssh");
    if subsystem {
        c.arg("-s");
    }
    c.args(["-x", "-T"]);
    // (NEW_FINDER_SSH_CONFIG: an ssh config of its own instead of
    // ~/.ssh/config's, for testing.)
    if let Some(config) = std::env::var_os("NEW_FINDER_SSH_CONFIG") {
        c.arg("-F").arg(config);
    }
    // (ssh keeps an option's first value: the caller's, then these.)
    c.args(&dest.options);
    for o in ["ServerAliveInterval=15", "ServerAliveCountMax=4", "ConnectTimeout=15", "ControlMaster=auto", "ControlPersist=120"] {
        c.args(["-o", o]);
    }
    c.arg("-o").arg(format!("ControlPath={}/%C", control.display()));
    match ASKPASS.lock().unwrap().clone() {
        Some(program) => {
            c.env("SSH_ASKPASS", program).env("SSH_ASKPASS_REQUIRE", "force").env("F2_ASKPASS", "1");
        }
        None => {
            c.args(["-o", "BatchMode=yes"]).env_remove("SSH_ASKPASS");
        }
    }
    if let Some(port) = dest.port {
        c.args(["-p", &port.to_string()]);
    }
    c.arg("--").arg(&dest.host);
    c
}

/// One SFTP session: an ssh process, spoken to on its stdin and stdout.
pub struct Session {
    child: Child,
    tx: ChildStdin,
    rx: BufReader<ChildStdout>,
    next: u32,
    /// What ssh said on stderr (why it failed, if it did).
    said: Arc<Mutex<String>>,
    /// Extensions the server offers (posix-rename@openssh.com...).
    extensions: Vec<String>,
    /// Answers read while waiting for another's.
    early: HashMap<u32, (u8, Vec<u8>)>,
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Session {
    /// Log in (asking for what it needs) and start SFTP.
    pub fn connect(dest: &Dest) -> Result<Session> {
        let mut child = ssh_command(dest, true)
            .arg("sftp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| Error::Lost(format!("ssh couldn't start: {e}")))?;
        let said = Arc::new(Mutex::new(String::new()));
        if let Some(mut err) = child.stderr.take() {
            let said = said.clone();
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                while let Ok(n) = err.read(&mut buf) {
                    if n == 0 {
                        break;
                    }
                    let mut s = said.lock().unwrap();
                    s.push_str(&String::from_utf8_lossy(&buf[..n]));
                    // (The last of it is what matters.)
                    if s.len() > 8192 {
                        let cut = s.len() - 4096;
                        let cut = (cut..s.len()).find(|&i| s.is_char_boundary(i)).unwrap_or(cut);
                        s.drain(..cut);
                    }
                }
            });
        }
        let tx = child.stdin.take().unwrap();
        let rx = BufReader::with_capacity(256 * 1024, child.stdout.take().unwrap());
        let mut s = Session { child, tx, rx, next: 1, said, extensions: Vec::new(), early: HashMap::new() };
        s.handshake()?;
        Ok(s)
    }

    fn handshake(&mut self) -> Result<()> {
        let mut p = Vec::new();
        put_u32(&mut p, 3);
        self.write_packet(INIT, &p)?;
        let (kind, body) = self.read_packet()?;
        if kind != VERSION {
            return Err(Error::Lost("The server didn't start SFTP.".into()));
        }
        let mut r = Reader(&body);
        let _version = r.u32()?;
        while !r.0.is_empty() {
            let name = r.bytes()?;
            let _data = r.bytes()?;
            self.extensions.push(String::from_utf8_lossy(name).into_owned());
        }
        Ok(())
    }

    /// Why the connection's gone, as ssh put it.
    fn lost(&self, fallback: &str) -> Error {
        // (Its stderr is read on a thread: a moment for its last words.)
        std::thread::sleep(std::time::Duration::from_millis(50));
        let said = self.said.lock().unwrap();
        let line = said.lines().rev().map(str::trim).find(|l| !l.is_empty() && !l.starts_with("Warning: Permanently added"));
        Error::Lost(match line {
            Some(l) => explain(l),
            None => fallback.to_string(),
        })
    }

    fn write_packet(&mut self, kind: u8, body: &[u8]) -> Result<()> {
        let mut p = Vec::with_capacity(body.len() + 5);
        put_u32(&mut p, body.len() as u32 + 1);
        p.push(kind);
        p.extend_from_slice(body);
        if self.tx.write_all(&p).and_then(|_| self.tx.flush()).is_err() {
            return Err(self.lost("The connection closed."));
        }
        Ok(())
    }

    fn read_packet(&mut self) -> Result<(u8, Vec<u8>)> {
        let mut len = [0u8; 4];
        if self.rx.read_exact(&mut len).is_err() {
            return Err(self.lost("The connection closed."));
        }
        let len = u32::from_be_bytes(len);
        if len == 0 || len > LONGEST {
            return Err(Error::Lost("The server printed something before SFTP started (a login script there, maybe).".into()));
        }
        let mut body = vec![0u8; len as usize];
        if self.rx.read_exact(&mut body).is_err() {
            return Err(self.lost("The connection closed."));
        }
        let kind = body.remove(0);
        Ok((kind, body))
    }

    /// Send a request (its id first), and say which id it has.
    fn send(&mut self, kind: u8, body: &[u8]) -> Result<u32> {
        let id = self.next;
        self.next = self.next.wrapping_add(1);
        let mut p = Vec::with_capacity(body.len() + 4);
        put_u32(&mut p, id);
        p.extend_from_slice(body);
        self.write_packet(kind, &p)?;
        Ok(id)
    }

    /// The answer to request `id` (others read meanwhile are kept).
    fn answer(&mut self, id: u32) -> Result<(u8, Vec<u8>)> {
        if let Some(a) = self.early.remove(&id) {
            return Ok(a);
        }
        loop {
            let (kind, body) = self.read_packet()?;
            let mut r = Reader(&body);
            let got = r.u32()?;
            let rest = r.0.to_vec();
            if got == id {
                return Ok((kind, rest));
            }
            self.early.insert(got, (kind, rest));
        }
    }

    fn call(&mut self, kind: u8, body: &[u8]) -> Result<(u8, Vec<u8>)> {
        let id = self.send(kind, body)?;
        self.answer(id)
    }

    /// A request answered with a status: OK, or the error.
    fn ok(&mut self, kind: u8, body: &[u8]) -> Result<()> {
        let (k, b) = self.call(kind, body)?;
        status(k, &b)
    }

    fn handle(&mut self, kind: u8, body: &[u8]) -> Result<Vec<u8>> {
        match self.call(kind, body)? {
            (HANDLE, b) => Ok(Reader(&b).bytes()?.to_vec()),
            (k, b) => Err(status(k, &b).err().unwrap_or(Error::Status(0, "no handle".into()))),
        }
    }

    fn close(&mut self, handle: &[u8]) -> Result<()> {
        let mut p = Vec::new();
        put_bytes(&mut p, handle);
        self.ok(CLOSE, &p)
    }

    /// The absolute path `path` is (`.`: the folder you log in to).
    pub fn realpath(&mut self, path: &Path) -> Result<PathBuf> {
        let mut p = Vec::new();
        put_path(&mut p, path);
        match self.call(REALPATH, &p)? {
            (NAME, b) => {
                let mut r = Reader(&b);
                let _count = r.u32()?;
                Ok(PathBuf::from(std::ffi::OsString::from_vec(r.bytes()?.to_vec())))
            }
            (k, b) => Err(status(k, &b).err().unwrap_or(Error::Status(0, String::new()))),
        }
    }

    pub fn stat(&mut self, path: &Path) -> Result<Attrs> {
        self.attrs(STAT, path)
    }

    pub fn lstat(&mut self, path: &Path) -> Result<Attrs> {
        self.attrs(LSTAT, path)
    }

    fn attrs(&mut self, kind: u8, path: &Path) -> Result<Attrs> {
        let mut p = Vec::new();
        put_path(&mut p, path);
        match self.call(kind, &p)? {
            (ATTRS, b) => Reader(&b).attrs(),
            (k, b) => Err(status(k, &b).err().unwrap_or(Error::Status(0, String::new()))),
        }
    }

    /// What's in the folder `dir` (not "." or ".."); a link to a folder
    /// says so.
    pub fn list(&mut self, dir: &Path) -> Result<Vec<Item>> {
        let mut p = Vec::new();
        put_path(&mut p, dir);
        let handle = self.handle(OPENDIR, &p)?;
        let mut items = Vec::new();
        let mut q = Vec::new();
        put_bytes(&mut q, &handle);
        let result = loop {
            match self.call(READDIR, &q) {
                Ok((NAME, b)) => {
                    let mut r = Reader(&b);
                    let count = r.u32()?;
                    for _ in 0..count {
                        let name = r.bytes()?.to_vec();
                        let _long = r.bytes()?;
                        let attrs = r.attrs()?;
                        if name == b"." || name == b".." {
                            continue;
                        }
                        items.push(Item { name: PathBuf::from(std::ffi::OsString::from_vec(name)), attrs, to_dir: false });
                    }
                }
                Ok((STATUS, b)) if Reader(&b).u32().ok() == Some(FX_EOF) => break Ok(()),
                Ok((k, b)) => break status(k, &b),
                Err(e) => return Err(e),
            }
        };
        let _ = self.close(&handle);
        result?;
        // (Links followed, all asked at once.)
        let links: Vec<usize> = (0..items.len()).filter(|&i| items[i].attrs.is_link()).collect();
        let mut ids = Vec::new();
        for &i in &links {
            let mut p = Vec::new();
            put_path(&mut p, &dir.join(&items[i].name));
            ids.push(self.send(STAT, &p)?);
        }
        for (i, id) in links.into_iter().zip(ids) {
            if let Ok((ATTRS, b)) = self.answer(id) {
                items[i].to_dir = Reader(&b).attrs().is_ok_and(|a| a.is_dir());
            }
        }
        Ok(items)
    }

    pub fn mkdir(&mut self, path: &Path) -> Result<()> {
        let mut p = Vec::new();
        put_path(&mut p, path);
        put_u32(&mut p, 0);
        self.ok(MKDIR, &p)
    }

    pub fn rmdir(&mut self, path: &Path) -> Result<()> {
        let mut p = Vec::new();
        put_path(&mut p, path);
        self.ok(RMDIR, &p)
    }

    pub fn remove(&mut self, path: &Path) -> Result<()> {
        let mut p = Vec::new();
        put_path(&mut p, path);
        self.ok(REMOVE, &p)
    }

    /// Rename `from` to `to` (not over something there: SFTP's rename
    /// refuses that, which is what's wanted).
    pub fn rename(&mut self, from: &Path, to: &Path) -> Result<()> {
        let mut p = Vec::new();
        put_path(&mut p, from);
        put_path(&mut p, to);
        self.ok(RENAME, &p)
    }

    /// Rename `from` to `to`, over what's there (OpenSSH's posix-rename;
    /// without it, what's there is removed first).
    pub fn replace(&mut self, from: &Path, to: &Path) -> Result<()> {
        if self.extensions.iter().any(|e| e == "posix-rename@openssh.com") {
            let mut p = Vec::new();
            put_bytes(&mut p, b"posix-rename@openssh.com");
            put_path(&mut p, from);
            put_path(&mut p, to);
            return self.ok(EXTENDED, &p);
        }
        match self.remove(to) {
            Err(e) if !e.missing() => return Err(e),
            _ => {}
        }
        self.rename(from, to)
    }

    /// Set a file's modification time.
    pub fn set_mtime(&mut self, path: &Path, mtime: u32) -> Result<()> {
        let mut p = Vec::new();
        put_path(&mut p, path);
        put_u32(&mut p, ATTR_ACMODTIME);
        put_u32(&mut p, mtime);
        put_u32(&mut p, mtime);
        self.ok(SETSTAT, &p)
    }

    /// Copy the file `remote` to `local` (made or replaced), many reads
    /// on their way at once. `progress` is told the bytes as they come, and
    /// stops it by returning false.
    pub fn download(&mut self, remote: &Path, local: &Path, progress: &mut dyn FnMut(u64) -> bool) -> Result<Attrs> {
        let attrs = self.stat(remote)?;
        let mut p = Vec::new();
        put_path(&mut p, remote);
        put_u32(&mut p, OPEN_READ);
        put_u32(&mut p, 0);
        let handle = self.handle(OPEN, &p)?;
        let file = File::create(local).map_err(|e| Error::Lost(format!("Couldn't write {}: {e}", local.display())));
        let file = match file {
            Ok(f) => f,
            Err(e) => {
                let _ = self.close(&handle);
                return Err(e);
            }
        };
        let result = self.read_into(&handle, &file, progress);
        let _ = self.close(&handle);
        result?;
        Ok(attrs)
    }

    fn read_into(&mut self, handle: &[u8], file: &File, progress: &mut dyn FnMut(u64) -> bool) -> Result<()> {
        // Requests out: id → (offset, length). Reads past the end come back
        // EOF; a short one is asked for again from where it stopped.
        let mut out: HashMap<u32, (u64, u32)> = HashMap::new();
        let mut order: std::collections::VecDeque<u32> = Default::default();
        let mut next = 0u64;
        let mut ended = false;
        let mut done = 0u64;
        let ask = |s: &mut Session, out: &mut HashMap<u32, (u64, u32)>, order: &mut std::collections::VecDeque<u32>, at: u64, len: u32| -> Result<()> {
            let mut p = Vec::new();
            put_bytes(&mut p, handle);
            put_u64(&mut p, at);
            put_u32(&mut p, len);
            let id = s.send(READ, &p)?;
            out.insert(id, (at, len));
            order.push_back(id);
            Ok(())
        };
        while !ended && out.len() < IN_FLIGHT {
            ask(self, &mut out, &mut order, next, CHUNK)?;
            next += CHUNK as u64;
            if out.len() >= 8 {
                break;
            }
        }
        while let Some(id) = order.pop_front() {
            let (at, len) = out.remove(&id).unwrap();
            match self.answer(id)? {
                (DATA, b) => {
                    let data = Reader(&b).bytes()?;
                    file.write_all_at(data, at).map_err(|e| Error::Lost(format!("Couldn't write the copy: {e}")))?;
                    done += data.len() as u64;
                    if !progress(done) {
                        return Err(Error::Lost("Stopped.".into()));
                    }
                    if (data.len() as u32) < len && !data.is_empty() {
                        ask(self, &mut out, &mut order, at + data.len() as u64, len - data.len() as u32)?;
                    }
                }
                (STATUS, b) if Reader(&b).u32().ok() == Some(FX_EOF) => ended = true,
                (k, b) => status(k, &b)?,
            }
            // (More asked for as answers come, up to IN_FLIGHT.)
            while !ended && out.len() < IN_FLIGHT {
                ask(self, &mut out, &mut order, next, CHUNK)?;
                next += CHUNK as u64;
            }
        }
        Ok(())
    }

    /// Copy the file `local` to `remote` (made or replaced). Written to a
    /// file beside it first, then put in its place: a copy that fails
    /// halfway leaves what was there as it was.
    pub fn upload(&mut self, local: &Path, remote: &Path, progress: &mut dyn FnMut(u64) -> bool) -> Result<()> {
        let file = File::open(local).map_err(|e| Error::Lost(format!("Couldn't read {}: {e}", local.display())))?;
        let name = remote.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let part = remote.with_file_name(format!(".{name}.f2-part"));
        let mut p = Vec::new();
        put_path(&mut p, &part);
        put_u32(&mut p, OPEN_WRITE | OPEN_CREATE | OPEN_TRUNCATE);
        // (Its permissions: the original's, if it's being replaced.)
        let perm = self.stat(remote).ok().and_then(|a| a.perm).map(|p| p & 0o7777);
        match perm {
            Some(perm) => {
                put_u32(&mut p, ATTR_PERMISSIONS);
                put_u32(&mut p, perm);
            }
            None => put_u32(&mut p, 0),
        }
        let handle = self.handle(OPEN, &p)?;
        let result = self.write_from(&handle, &file, progress);
        let closed = self.close(&handle);
        if let Err(e) = result.and(closed) {
            let _ = self.remove(&part);
            return Err(e);
        }
        if let Some(mtime) = file.metadata().ok().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()) {
            let _ = self.set_mtime(&part, mtime.as_secs() as u32);
        }
        self.replace(&part, remote).inspect_err(|_| {
            let _ = self.remove(&part);
        })
    }

    fn write_from(&mut self, handle: &[u8], file: &File, progress: &mut dyn FnMut(u64) -> bool) -> Result<()> {
        let mut order: std::collections::VecDeque<(u32, u64)> = Default::default();
        let mut at = 0u64;
        let mut done = 0u64;
        let mut buf = vec![0u8; CHUNK as usize];
        let mut ended = false;
        loop {
            while !ended && order.len() < IN_FLIGHT {
                let n = file.read_at(&mut buf, at).map_err(|e| Error::Lost(format!("Couldn't read the file: {e}")))?;
                if n == 0 {
                    ended = true;
                    break;
                }
                let mut p = Vec::with_capacity(n + 64);
                put_bytes(&mut p, handle);
                put_u64(&mut p, at);
                put_bytes(&mut p, &buf[..n]);
                let id = self.send(WRITE, &p)?;
                order.push_back((id, n as u64));
                at += n as u64;
            }
            let Some((id, n)) = order.pop_front() else { return Ok(()) };
            let (k, b) = self.answer(id)?;
            status(k, &b)?;
            done += n;
            if !progress(done) {
                return Err(Error::Lost("Stopped.".into()));
            }
        }
    }
}

/// ssh's last words, said plainly where they're known.
fn explain(line: &str) -> String {
    let l = line.to_lowercase();
    if l.contains("permission denied") {
        "The server didn't accept the login (wrong password, or no key it knows).".into()
    } else if l.contains("could not resolve hostname") {
        "There's no server by that name (or no internet connection).".into()
    } else if l.contains("no route to host") || l.contains("network is unreachable") {
        "It can't be reached from this network (no internet connection, or it's behind a VPN).".into()
    } else if l.contains("connection refused") {
        "It's there, but not taking SSH connections: is SSH (Remote Login) on there, and on this port?".into()
    } else if l.contains("timed out") {
        "It didn't answer: it may be off, or out of reach from this network (behind a VPN, say).".into()
    } else if l.contains("host key verification failed") {
        "The server's key wasn't accepted, so it wasn't connected to.".into()
    } else if l.contains("remote host identification has changed") {
        "The server's key has changed since you last connected: see ~/.ssh/known_hosts.".into()
    } else if l.contains("subsystem request failed") {
        "The server doesn't offer SFTP.".into()
    } else {
        line.to_string()
    }
}

fn status(kind: u8, body: &[u8]) -> Result<()> {
    if kind != STATUS {
        return Err(Error::Status(0, format!("unexpected answer {kind}")));
    }
    let mut r = Reader(body);
    let code = r.u32()?;
    if code == 0 {
        return Ok(());
    }
    let message = r.bytes().map(|m| String::from_utf8_lossy(m).into_owned()).unwrap_or_default();
    Err(Error::Status(code, message))
}

fn put_u32(p: &mut Vec<u8>, v: u32) {
    p.extend_from_slice(&v.to_be_bytes());
}

fn put_u64(p: &mut Vec<u8>, v: u64) {
    p.extend_from_slice(&v.to_be_bytes());
}

fn put_bytes(p: &mut Vec<u8>, b: &[u8]) {
    put_u32(p, b.len() as u32);
    p.extend_from_slice(b);
}

fn put_path(p: &mut Vec<u8>, path: &Path) {
    put_bytes(p, path.as_os_str().as_bytes());
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        if self.0.len() < n {
            return Err(Error::Status(5, "a malformed answer".into()));
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Ok(a)
    }

    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_be_bytes(self.take(8)?.try_into().unwrap()))
    }

    fn bytes(&mut self) -> Result<&'a [u8]> {
        let n = self.u32()? as usize;
        self.take(n)
    }

    fn attrs(&mut self) -> Result<Attrs> {
        let flags = self.u32()?;
        let mut a = Attrs::default();
        if flags & ATTR_SIZE != 0 {
            a.size = Some(self.u64()?);
        }
        if flags & ATTR_UIDGID != 0 {
            self.u32()?;
            self.u32()?;
        }
        if flags & ATTR_PERMISSIONS != 0 {
            a.perm = Some(self.u32()?);
        }
        if flags & ATTR_ACMODTIME != 0 {
            self.u32()?;
            a.mtime = Some(self.u32()?);
        }
        if flags & ATTR_EXTENDED != 0 {
            for _ in 0..self.u32()? {
                self.bytes()?;
                self.bytes()?;
            }
        }
        Ok(a)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A throwaway sshd of the user's own, on 127.0.0.1 only, with its own
    /// keys, serving `root` (see `server`): the real ssh and sftp-server,
    /// end to end. Skipped where sshd can't run.
    pub struct Server {
        pub dir: PathBuf,
        pub dest: Dest,
        pid: u32,
    }

    impl Drop for Server {
        fn drop(&mut self) {
            unsafe { libc::kill(self.pid as i32, libc::SIGTERM) };
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    pub fn server(name: &str) -> Server {
        let dir = std::env::temp_dir().join(format!("nf-sshd-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("files")).unwrap();
        let dir = dir.canonicalize().unwrap();
        for key in ["host", "client"] {
            let ok = Command::new("ssh-keygen").args(["-q", "-t", "ed25519", "-N", "", "-f"]).arg(dir.join(key)).status().unwrap();
            assert!(ok.success());
        }
        std::fs::copy(dir.join("client.pub"), dir.join("authorized_keys")).unwrap();
        let port = 20000 + (std::process::id() % 20000) as u16 + name.len() as u16;
        let config = format!(
            "ListenAddress 127.0.0.1\nPort {port}\nHostKey {d}/host\nPidFile {d}/sshd.pid\nAuthorizedKeysFile {d}/authorized_keys\nPasswordAuthentication no\nKbdInteractiveAuthentication no\nUsePAM no\nStrictModes no\nSubsystem sftp /usr/libexec/sftp-server\n",
            d = dir.display()
        );
        std::fs::write(dir.join("sshd_config"), config).unwrap();
        let ok = Command::new("/usr/sbin/sshd").arg("-f").arg(dir.join("sshd_config")).arg("-E").arg(dir.join("sshd.log")).status().unwrap();
        assert!(ok.success(), "sshd didn't start");
        let mut pid = None;
        for _ in 0..50 {
            if let Some(p) = std::fs::read_to_string(dir.join("sshd.pid")).ok().and_then(|s| s.trim().parse().ok()) {
                pid = Some(p);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let options = [
            "-i".to_string(),
            dir.join("client").display().to_string(),
            "-F".into(),
            "/dev/null".into(),
            "-o".into(),
            "IdentitiesOnly=yes".into(),
            "-o".into(),
            "IdentityAgent=none".into(),
            "-o".into(),
            format!("UserKnownHostsFile={}/known_hosts", dir.display()),
            "-o".into(),
            "StrictHostKeyChecking=accept-new".into(),
            // (Its own login, not one shared with another test's.)
            "-o".into(),
            "ControlMaster=no".into(),
        ];
        Server { dest: Dest { host: "127.0.0.1".into(), port: Some(port), options: options.to_vec() }, dir, pid: pid.expect("sshd's pid") }
    }

    /// `cargo test -- --ignored --nocapture sftp_end_to_end`
    #[test]
    #[ignore]
    fn sftp_end_to_end() {
        let srv = server("e2e");
        let files = srv.dir.join("files");
        std::fs::create_dir_all(files.join("sub")).unwrap();
        std::fs::write(files.join("a.txt"), b"hello").unwrap();
        std::os::unix::fs::symlink(files.join("sub"), files.join("link-to-sub")).unwrap();
        let mut s = Session::connect(&srv.dest).unwrap();
        let home = s.realpath(Path::new(".")).unwrap();
        println!("home {}", home.display());
        let mut items = s.list(&files).unwrap();
        items.sort_by(|a, b| a.name.cmp(&b.name));
        let names: Vec<_> = items.iter().map(|i| i.name.to_string_lossy().into_owned()).collect();
        assert_eq!(names, ["a.txt", "link-to-sub", "sub"]);
        assert_eq!(items[0].attrs.size, Some(5));
        assert!(items[2].attrs.is_dir() && items[1].attrs.is_link() && items[1].to_dir);
        assert!(s.list(&files.join("nope")).unwrap_err().missing());

        // Both ways, bigger than a read's worth, many on their way at once.
        let big: Vec<u8> = (0..5_000_000u32).map(|i| (i * 7 % 251) as u8).collect();
        let local = srv.dir.join("big-local");
        std::fs::write(&local, &big).unwrap();
        let t = std::time::Instant::now();
        s.upload(&local, &files.join("big"), &mut |_| true).unwrap();
        println!("up 5 MB in {:.0?}", t.elapsed());
        assert_eq!(std::fs::read(files.join("big")).unwrap(), big);
        assert!(!files.join(".big.f2-part").exists());
        let back = srv.dir.join("big-back");
        let t = std::time::Instant::now();
        let attrs = s.download(&files.join("big"), &back, &mut |_| true).unwrap();
        println!("down 5 MB in {:.0?}", t.elapsed());
        assert_eq!(std::fs::read(&back).unwrap(), big);
        assert_eq!(attrs.size, Some(big.len() as u64));
        // (Replacing keeps the permissions.)
        std::fs::write(&local, b"new").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(files.join("a.txt"), std::fs::Permissions::from_mode(0o640)).unwrap();
        s.upload(&local, &files.join("a.txt"), &mut |_| true).unwrap();
        assert_eq!(std::fs::read(files.join("a.txt")).unwrap(), b"new");
        assert_eq!(std::fs::metadata(files.join("a.txt")).unwrap().permissions().mode() & 0o777, 0o640);

        s.mkdir(&files.join("made")).unwrap();
        s.rename(&files.join("made"), &files.join("renamed")).unwrap();
        assert!(files.join("renamed").is_dir());
        assert!(s.rename(&files.join("a.txt"), &files.join("big")).is_err(), "not over what's there");
        s.rmdir(&files.join("renamed")).unwrap();
        s.remove(&files.join("big")).unwrap();
        assert!(!files.join("big").exists());
        // A stopped copy leaves nothing behind.
        let stopped = s.upload(&back, &files.join("half"), &mut |done| done < 100_000);
        assert!(stopped.is_err() && !files.join("half").exists() && !files.join(".half.f2-part").exists());
    }

    /// A login refused says why.
    #[test]
    #[ignore]
    fn sftp_refused() {
        let srv = server("refused");
        let mut dest = srv.dest.clone();
        dest.options[1] = srv.dir.join("host").display().to_string();
        let e = Session::connect(&dest).err().unwrap();
        println!("{e}");
        assert!(e.to_string().contains("didn't accept the login"));
        dest.port = Some(1);
        let e = Session::connect(&dest).err().unwrap();
        println!("{e}");
        assert!(e.to_string().contains("not taking SSH connections"));
    }
}
