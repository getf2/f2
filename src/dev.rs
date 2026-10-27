//! Developer commands for Quick Open, as Raycast's: what's listening on a
//! port ("kill port 3000", "ports"), git repos (on GitHub, their branch),
//! the clipboard's text as JSON, Base64 or URL encoding, a UUID, and Unix
//! timestamps read as dates.

use std::path::{Path, PathBuf};

/// A process listening on a TCP port.
#[derive(Clone, Debug, PartialEq)]
pub struct Listener {
    pub pid: i32,
    pub command: String,
    pub port: u16,
    /// An app's (in an .app's Contents/MacOS) or macOS's own (in /System,
    /// /usr): not a server of yours to stop.
    pub app: bool,
}

/// Where process `pid`'s program is.
fn program(pid: i32) -> Option<String> {
    let mut buf = vec![0u8; 4096];
    let n = unsafe { libc::proc_pidpath(pid, buf.as_mut_ptr().cast(), buf.len() as u32) };
    (n > 0).then(|| String::from_utf8_lossy(&buf[..n as usize]).into_owned())
}

fn is_app(program: &str) -> bool {
    program.contains(".app/Contents/MacOS/") || ["/System/", "/usr/bin/", "/usr/sbin/", "/usr/libexec/"].iter().any(|p| program.starts_with(p))
}

/// What's listening on TCP ports (lsof's, the user's own processes).
pub fn listeners() -> Vec<Listener> {
    let Ok(out) = std::process::Command::new("/usr/sbin/lsof").args(["-nP", "-iTCP", "-sTCP:LISTEN", "-F", "pcn"]).output() else {
        return Vec::new();
    };
    let mut found = parse_lsof(&String::from_utf8_lossy(&out.stdout));
    for l in &mut found {
        l.app = program(l.pid).is_some_and(|p| is_app(&p));
    }
    found
}

/// lsof's `-F pcn`: "p" a process, "c" its command, "n" an address
/// (":3000", "127.0.0.1:5432", "[::1]:8080").
fn parse_lsof(text: &str) -> Vec<Listener> {
    let mut found: Vec<Listener> = Vec::new();
    let (mut pid, mut command) = (0, String::new());
    for line in text.lines() {
        let (tag, rest) = line.split_at(line.len().min(1));
        match tag {
            "p" => pid = rest.parse().unwrap_or(0),
            "c" => command = rest.to_string(),
            "n" => {
                if let Some(port) = rest.rsplit(':').next().and_then(|p| p.parse::<u16>().ok()) {
                    let listener = Listener { pid, command: command.clone(), port, app: false };
                    // (IPv4 and IPv6 both: once.)
                    if !found.contains(&listener) {
                        found.push(listener);
                    }
                }
            }
            _ => {}
        }
    }
    found.sort_by_key(|l| (l.port, l.pid));
    found
}

/// Whether `query` asks about ports: Some(Some(port)) for one ("kill port
/// 3000", "port 3000", "kill 3000", ":3000"), Some(None) for all ("ports",
/// "kill port", "listening ports").
pub fn port_query(query: &str) -> Option<Option<u16>> {
    let q = query.trim().to_lowercase();
    let words: Vec<&str> = q.split_whitespace().collect();
    let number = |w: &str| w.trim_start_matches(':').parse::<u16>().ok().filter(|p| *p > 0);
    match words.as_slice() {
        ["ports"] | ["port"] | ["kill", "port"] | ["listening", "ports"] | ["listening"] => Some(None),
        [one] if one.starts_with(':') => number(one).map(Some),
        ["kill", "port", n] | ["port", n] | ["kill", n] | ["kill", "port", ":", n] => number(n).map(Some),
        _ => None,
    }
}

/// Stop process `pid`: asked to (SIGTERM), then made to if it's still there
/// two seconds on (SIGKILL), as kill-port does.
pub fn stop(pid: i32) {
    std::thread::spawn(move || {
        unsafe { libc::kill(pid, libc::SIGTERM) };
        std::thread::sleep(std::time::Duration::from_secs(2));
        if unsafe { libc::kill(pid, 0) } == 0 {
            unsafe { libc::kill(pid, libc::SIGKILL) };
        }
    });
}

/// A git repo: where, its name, its branch, and its page on the web (from
/// its "origin" remote), if any.
#[derive(Clone, Debug, PartialEq)]
pub struct Repo {
    pub root: PathBuf,
    pub name: String,
    pub branch: Option<String>,
    pub web: Option<String>,
}

/// The repo `path` is in (or is), if any.
pub fn repo_at(path: &Path) -> Option<Repo> {
    let root = path.ancestors().find(|d| d.join(".git").exists())?.to_path_buf();
    let mut git = root.join(".git");
    // (A worktree's ".git" is a file pointing at its own git folder.)
    if git.is_file() {
        let text = std::fs::read_to_string(&git).ok()?;
        git = root.join(text.trim().strip_prefix("gitdir:")?.trim());
    }
    let head = std::fs::read_to_string(git.join("HEAD")).unwrap_or_default();
    let branch = head.trim().strip_prefix("ref: refs/heads/").map(str::to_string);
    // A worktree's config is its common folder's.
    let common = std::fs::read_to_string(git.join("commondir")).ok().map(|c| git.join(c.trim())).unwrap_or_else(|| git.clone());
    let config = std::fs::read_to_string(common.join("config")).unwrap_or_default();
    let web = origin(&config).and_then(|url| web_url(&url));
    let name = root.file_name().map_or_else(|| root.display().to_string(), |n| n.to_string_lossy().into_owned());
    Some(Repo { root, name, branch, web })
}

/// The "origin" remote's URL in a git config.
fn origin(config: &str) -> Option<String> {
    let mut inside = false;
    for line in config.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            inside = line == "[remote \"origin\"]";
        } else if inside && let Some(url) = line.strip_prefix("url").map(str::trim).and_then(|r| r.strip_prefix('=')) {
            return Some(url.trim().to_string());
        }
    }
    None
}

/// A remote's page: "git@github.com:me/repo.git" and
/// "https://github.com/me/repo.git" are both "https://github.com/me/repo".
pub fn web_url(remote: &str) -> Option<String> {
    let r = remote.trim().trim_end_matches('/');
    let r = r.strip_suffix(".git").unwrap_or(r);
    if let Some(rest) = r.strip_prefix("git@") {
        let (host, path) = rest.split_once(':')?;
        return Some(format!("https://{host}/{path}"));
    }
    if let Some(rest) = r.strip_prefix("ssh://") {
        let rest = rest.split_once('@').map_or(rest, |(_, r)| r);
        let (host, path) = rest.split_once('/')?;
        let host = host.split(':').next()?;
        return Some(format!("https://{host}/{path}"));
    }
    if r.starts_with("https://") || r.starts_with("http://") {
        // (Without a user in it: "https://me@github.com/…".)
        let (scheme, rest) = r.split_once("://")?;
        let rest = rest.split_once('@').map_or(rest, |(_, r)| r);
        return Some(format!("{scheme}://{rest}"));
    }
    None
}

/// What can be done to the clipboard's text.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Tool {
    FormatJson,
    MinifyJson,
    Base64Encode,
    Base64Decode,
    UrlEncode,
    UrlDecode,
}

impl Tool {
    pub const ALL: [Tool; 6] = [Tool::FormatJson, Tool::MinifyJson, Tool::Base64Encode, Tool::Base64Decode, Tool::UrlEncode, Tool::UrlDecode];

    /// Its title, words it's found by, symbol.
    pub fn about(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Tool::FormatJson => ("Format JSON", "json format pretty print beautify clipboard", "curlybraces"),
            Tool::MinifyJson => ("Minify JSON", "json minify compact clipboard", "curlybraces"),
            Tool::Base64Encode => ("Encode Base64", "base64 encode clipboard", "lock"),
            Tool::Base64Decode => ("Decode Base64", "base64 decode clipboard", "lock.open"),
            Tool::UrlEncode => ("Encode URL", "url encode percent escape clipboard", "link"),
            Tool::UrlDecode => ("Decode URL", "url decode percent unescape clipboard", "link"),
        }
    }

    pub fn apply(self, text: &str) -> Result<String, String> {
        match self {
            Tool::FormatJson | Tool::MinifyJson => {
                let value: serde_json::Value = serde_json::from_str(text.trim()).map_err(|e| format!("That's not JSON: {e}"))?;
                Ok(if self == Tool::FormatJson { serde_json::to_string_pretty(&value) } else { serde_json::to_string(&value) }.map_err(|e| e.to_string())?)
            }
            Tool::Base64Encode => Ok(base64_encode(text.as_bytes())),
            Tool::Base64Decode => {
                let bytes = base64_decode(text.trim()).ok_or("That's not Base64.")?;
                String::from_utf8(bytes).map_err(|_| "That's Base64, but not of text.".to_string())
            }
            Tool::UrlEncode => Ok(url_encode(text)),
            Tool::UrlDecode => url_decode(text.trim()).ok_or_else(|| "That's not URL-encoded text.".to_string()),
        }
    }
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, &b)| n | (b as u32) << (16 - 8 * i));
        for i in 0..4 {
            out.push(if i <= chunk.len() { BASE64[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    out
}

/// Standard or URL-safe, padded or not.
pub fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let (mut n, mut bits) = (0u32, 0);
    for c in text.bytes().filter(|c| !c.is_ascii_whitespace() && *c != b'=') {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        };
        n = n << 6 | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((n >> bits & 0xff) as u8);
        }
    }
    Some(out)
}

pub fn url_encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

pub fn url_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
                out.push(u8::from_str_radix(hex, 16).ok()?);
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

/// A random (version 4) UUID.
pub fn uuid() -> String {
    let mut b = [0u8; 16];
    unsafe { libc::arc4random_buf(b.as_mut_ptr().cast(), b.len()) };
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

/// A Unix timestamp typed (seconds, or milliseconds): the date it is, as
/// shown and as copied (ISO 8601).
pub fn timestamp(query: &str) -> Option<(String, String)> {
    let q = query.trim();
    if !(q.len() == 10 || q.len() == 13) || !q.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n: i64 = q.parse().ok()?;
    let secs = if q.len() == 13 { n / 1000 } else { n };
    let at = chrono::DateTime::from_timestamp(secs, 0)?.with_timezone(&chrono::Local);
    Some((at.format("%a %-d %b %Y, %H:%M:%S").to_string(), at.to_rfc3339()))
}

#[cfg(test)]
mod tests {
    use super::{Tool, base64_decode, base64_encode, origin, parse_lsof, port_query, timestamp, url_decode, uuid, web_url};

    #[test]
    fn ports() {
        let lsof = "p123\ncnode\nn*:3000\nn[::1]:3000\np99\ncpostgres\nn127.0.0.1:5432\n";
        let found = parse_lsof(lsof);
        assert_eq!(found.len(), 2, "IPv4 and IPv6 of one: once");
        assert_eq!((found[0].port, found[0].command.as_str(), found[0].pid), (3000, "node", 123));
        assert_eq!(port_query("kill port 3000"), Some(Some(3000)));
        assert_eq!(port_query(":8080"), Some(Some(8080)));
        assert_eq!(port_query("kill 5432"), Some(Some(5432)));
        assert_eq!(port_query("ports"), Some(None));
        assert_eq!(port_query("port wine"), None);
        assert_eq!(port_query("3000"), None, "a number alone is a sum");
        assert!(super::is_app("/Applications/T3 Code.app/Contents/MacOS/T3 Code"));
        assert!(super::is_app("/System/Library/CoreServices/ControlCenter.app/Contents/MacOS/ControlCenter"));
        assert!(!super::is_app("/opt/homebrew/bin/redis-server"));
        assert!(!super::is_app("/Applications/Postgres.app/Contents/Versions/17/bin/postgres"), "a server an app ships");
    }

    #[test]
    fn repos() {
        assert_eq!(web_url("git@github.com:me/repo.git").as_deref(), Some("https://github.com/me/repo"));
        assert_eq!(web_url("https://me@github.com/me/repo.git").as_deref(), Some("https://github.com/me/repo"));
        assert_eq!(web_url("ssh://git@gitlab.com:22/me/repo.git").as_deref(), Some("https://gitlab.com/me/repo"));
        let config = "[core]\n\tbare = false\n[remote \"origin\"]\n\turl = git@github.com:me/r.git\n\tfetch = +refs/heads/*\n";
        assert_eq!(origin(config).as_deref(), Some("git@github.com:me/r.git"));
    }

    #[test]
    fn text_tools() {
        assert_eq!(base64_encode(b"Finder2!"), "RmluZGVyMiE=");
        assert_eq!(base64_decode("RmluZGVyMiE").as_deref(), Some(&b"Finder2!"[..]));
        assert_eq!(Tool::FormatJson.apply("{\"a\":[1,2]}").unwrap(), "{\n  \"a\": [\n    1,\n    2\n  ]\n}");
        assert!(Tool::MinifyJson.apply("not json").is_err());
        assert_eq!(url_decode("a%20b+c").as_deref(), Some("a b c"));
        assert_eq!(Tool::UrlEncode.apply("a b/c").unwrap(), "a%20b%2Fc");
        let id = uuid();
        assert_eq!((id.len(), &id[14..15]), (36, "4"));
        assert!(timestamp("1792461091").is_some());
        assert!(timestamp("1792461091000").is_some(), "milliseconds");
        assert!(timestamp("12345").is_none());
    }
}
