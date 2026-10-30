//! Finder's .DS_Store files, read (never written): where icons were put by
//! hand in icon view ("Iloc", an icon's center), and its View Options
//! ("icvp": whether the icons are arranged freely, their size, a picture or
//! color behind them); and the window Finder shows the folder in ("bwsp":
//! its size, its toolbar and sidebar hidden), as a disk image's is laid
//! out, its app beside an arrow to Applications. The format is
//! Finder's own and undocumented, as others have worked it out (Mark
//! Mentovai's notes, the ds_store Python package): a buddy allocator whose
//! "DSDB" block is a B-tree of records, each a file's name, a four-letter
//! code and a typed value.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// What a folder's .DS_Store says of its icons.
#[derive(Debug, Default, PartialEq)]
pub struct Store {
    /// Icons' centers, in points from the top left, by name.
    pub positions: HashMap<String, (f32, f32)>,
    /// Its icons are arranged freely (Sort By: None).
    pub free: bool,
    pub look: Look,
    pub window: Option<Window>,
}

/// Icon view's look, as View Options set it.
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Look {
    pub background: Background,
    pub icon_size: Option<f32>,
    pub text_size: Option<f32>,
}

#[derive(Debug, Default, PartialEq, Clone)]
pub enum Background {
    #[default]
    None,
    /// Red, green and blue, 0 to 1.
    Color([f32; 3]),
    /// A picture, where its alias says it is: its path on its volume
    /// ("/.background/arrow.png"), and its name.
    Picture { path: Option<String>, name: Option<String> },
}

/// The window Finder shows the folder in.
#[derive(Debug, PartialEq, Clone)]
pub struct Window {
    /// Its size, title bar and all, in points.
    pub size: (f32, f32),
    pub toolbar: bool,
    pub sidebar: bool,
}

impl Store {
    /// The background picture's file, if there's one: where the alias says
    /// on the volume `dir` is on, else by its name in `dir` or its hidden
    /// .background folder (where disk image tools put it), else the
    /// picture there.
    pub fn picture(&self, dir: &Path) -> Option<PathBuf> {
        let Background::Picture { path, name } = &self.look.background else { return None };
        let volume = volume_of(dir);
        let mut tries: Vec<PathBuf> = Vec::new();
        if let (Some(volume), Some(path)) = (&volume, path) {
            tries.push(volume.join(path.trim_start_matches('/')));
        }
        if let Some(name) = name {
            tries.push(dir.join(name));
            tries.push(dir.join(".background").join(name));
        }
        if let Some(found) = tries.into_iter().find(|p| p.is_file() && p.starts_with(volume.as_deref().unwrap_or(Path::new("/")))) {
            return Some(found);
        }
        let mut pictures: Vec<PathBuf> = std::fs::read_dir(dir.join(".background"))
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| ["png", "tif", "tiff", "jpg", "jpeg", "gif", "pdf", "heic"].contains(&e.to_string_lossy().to_lowercase().as_str())))
            .collect();
        pictures.sort();
        pictures.into_iter().next()
    }
}

/// The volume a folder is on: where it's mounted.
fn volume_of(dir: &Path) -> Option<PathBuf> {
    let path = std::ffi::CString::new(dir.as_os_str().as_encoded_bytes()).ok()?;
    let mut fs: libc::statfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statfs(path.as_ptr(), &mut fs) } != 0 {
        return None;
    }
    let mount = unsafe { std::ffi::CStr::from_ptr(fs.f_mntonname.as_ptr()) };
    Some(PathBuf::from(mount.to_string_lossy().into_owned()))
}

/// Where an alias (an Alias Manager record, version 2 or 3: Finder's
/// backgroundImageAlias) points: its path on its volume, and its name.
fn alias_target(alias: &[u8]) -> (Option<String>, Option<String>) {
    let u16_at = |at: usize| alias.get(at..at + 2).map(|b| u16::from_be_bytes([b[0], b[1]]));
    // (Version 2 has the name in it, then its extras; 3, only extras.)
    let (mut name, mut at) = match u16_at(6) {
        Some(2) => {
            let len = *alias.get(50).unwrap_or(&0) as usize;
            (alias.get(51..51 + len.min(63)).map(|b| String::from_utf8_lossy(b).into_owned()), 150)
        }
        Some(3) => (None, 58),
        _ => return (None, None),
    };
    let mut path = None;
    while let (Some(tag), Some(len)) = (u16_at(at), u16_at(at + 2)) {
        if tag == 0xffff {
            break;
        }
        let Some(value) = alias.get(at + 4..at + 4 + len as usize) else { break };
        match tag {
            // The path on the volume, as POSIX writes it.
            18 => path = Some(String::from_utf8_lossy(value).into_owned()),
            // Its name, in UTF-16 (after its count).
            14 if value.len() >= 2 => {
                let units: Vec<u16> = value[2..].chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
                name = Some(String::from_utf16_lossy(&units));
            }
            _ => {}
        }
        at += 4 + len as usize + (len as usize & 1);
    }
    let name = name.filter(|n| !n.is_empty()).or_else(|| path.as_deref().and_then(|p| p.rsplit('/').next()).map(str::to_string));
    (path, name)
}

/// Hex ("0a1b…") as bytes.
fn unhex(hex: &str) -> Option<Vec<u8>> {
    (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok()).collect()
}

/// "{{200, 160}, {640, 432}}": its numbers.
fn numbers(text: &str) -> Vec<f32> {
    text.split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-')).filter_map(|n| n.parse().ok()).collect()
}

/// Bigger than this, it isn't one Finder wrote.
const MAX_SIZE: u64 = 4 << 20;

pub fn read(dir: &Path) -> Option<Store> {
    let file = dir.join(".DS_Store");
    if std::fs::metadata(&file).ok()?.len() > MAX_SIZE {
        return None;
    }
    parse(&std::fs::read(file).ok()?)
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at.checked_add(4)?)?.try_into().ok()?))
}

struct File<'a> {
    bytes: &'a [u8],
    /// The blocks' addresses: an offset (from byte 4) and a size, as
    /// `offset | log2(size)`.
    blocks: Vec<u32>,
}

impl<'a> File<'a> {
    fn block(&self, id: u32) -> Option<&'a [u8]> {
        let address = *self.blocks.get(id as usize)?;
        let (offset, size) = ((address & !0x1f) as usize + 4, 1usize.checked_shl(address & 0x1f)?);
        self.bytes.get(offset..offset.checked_add(size)?)
    }
}

pub fn parse(bytes: &[u8]) -> Option<Store> {
    // The header: 1, "Bud1", then where the allocator's own block is.
    if u32_at(bytes, 0)? != 1 || bytes.get(4..8)? != b"Bud1" {
        return None;
    }
    let (root, root_size) = (u32_at(bytes, 8)? as usize + 4, u32_at(bytes, 12)? as usize);
    let info = bytes.get(root..root.checked_add(root_size)?)?;
    let count = u32_at(info, 0)? as usize;
    if count > 1 << 16 {
        return None;
    }
    let blocks = (0..count).map(|i| u32_at(info, 8 + i * 4)).collect::<Option<Vec<_>>>()?;
    // (The addresses take up whole pages of 256.)
    let mut at = 8 + count.div_ceil(256).max(1) * 256 * 4;
    let names = u32_at(info, at)?;
    at += 4;
    let mut master = None;
    for _ in 0..names.min(64) {
        let len = *info.get(at)? as usize;
        let name = info.get(at + 1..at + 1 + len)?;
        let id = u32_at(info, at + 1 + len)?;
        at += 1 + len + 4;
        if name == b"DSDB" {
            master = Some(id);
        }
    }
    let file = File { bytes, blocks };
    let master = file.block(master?)?;
    let (root_node, levels) = (u32_at(master, 0)?, u32_at(master, 4)?);
    if levels > 16 {
        return None;
    }
    let mut store = Store::default();
    let mut seen = std::collections::HashSet::new();
    walk(&file, root_node, levels + 1, &mut seen, &mut store)?;
    Some(store)
}

/// A node of the tree, and those under it (`depth` at most, each once).
fn walk(file: &File, id: u32, depth: u32, seen: &mut std::collections::HashSet<u32>, store: &mut Store) -> Option<()> {
    if depth == 0 || !seen.insert(id) {
        return None;
    }
    let node = file.block(id)?;
    let (next, count) = (u32_at(node, 0)?, u32_at(node, 4)?);
    let mut at = 8;
    for _ in 0..count.min(1 << 16) {
        if next != 0 {
            let child = u32_at(node, at)?;
            at += 4;
            walk(file, child, depth - 1, seen, store)?;
        }
        at = record(node, at, store)?;
    }
    if next != 0 {
        walk(file, next, depth - 1, seen, store)?;
    }
    Some(())
}

/// A record at `at`: its name, code and value (kept if it's one wanted).
/// Where the next one starts.
fn record(node: &[u8], at: usize, store: &mut Store) -> Option<usize> {
    let len = u32_at(node, at)? as usize;
    let units: Vec<u16> = node.get(at + 4..at + 4 + len * 2)?.chunks(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
    let name = String::from_utf16_lossy(&units);
    let mut at = at + 4 + len * 2;
    let code = node.get(at..at + 4)?;
    let kind = node.get(at + 4..at + 8)?;
    at += 8;
    let value: &[u8] = match kind {
        b"long" | b"shor" | b"type" => node.get(at..at + 4)?,
        b"bool" => node.get(at..at + 1)?,
        b"comp" | b"dutc" => node.get(at..at + 8)?,
        b"blob" => {
            let n = u32_at(node, at)? as usize;
            at += 4;
            node.get(at..at.checked_add(n)?)?
        }
        b"ustr" => {
            let n = u32_at(node, at)? as usize * 2;
            at += 4;
            node.get(at..at.checked_add(n)?)?
        }
        _ => return None,
    };
    let next = at + value.len();
    match code {
        b"Iloc" if value.len() >= 8 => {
            let (x, y) = (u32_at(value, 0)?, u32_at(value, 4)?);
            store.positions.insert(name, (x as f32, y as f32));
        }
        b"icvp" if name == "." && kind == b"blob" => {
            let Some(plist) = crate::macos::plist_json(value) else { return Some(next) };
            let number = |key: &str| plist.get(key).and_then(serde_json::Value::as_f64).map(|n| n as f32);
            store.free = plist.get("arrangeBy").and_then(|a| a.as_str()) == Some("none");
            store.look.icon_size = number("iconSize").filter(|s| *s > 0.);
            store.look.text_size = number("textSize").filter(|s| *s > 0.);
            store.look.background = match number("backgroundType") {
                Some(1.) => Background::Color([
                    number("backgroundColorRed").unwrap_or(1.),
                    number("backgroundColorGreen").unwrap_or(1.),
                    number("backgroundColorBlue").unwrap_or(1.),
                ]),
                Some(2.) => {
                    let alias = plist.get("backgroundImageAlias").and_then(|a| a.as_str()).and_then(unhex).unwrap_or_default();
                    let (path, name) = alias_target(&alias);
                    Background::Picture { path, name }
                }
                _ => Background::None,
            };
        }
        b"bwsp" if name == "." && kind == b"blob" => {
            let Some(plist) = crate::macos::plist_json(value) else { return Some(next) };
            let shown = |key: &str| plist.get(key).and_then(serde_json::Value::as_bool).unwrap_or(true);
            let bounds = plist.get("WindowBounds").and_then(|b| b.as_str()).map(numbers).unwrap_or_default();
            if let [_, _, w, h] = bounds[..]
                && w > 0.
                && h > 0.
            {
                store.window = Some(Window { size: (w, h), toolbar: shown("ShowToolbar"), sidebar: shown("ShowSidebar") });
            }
        }
        _ => {}
    }
    Some(next)
}

#[cfg(test)]
mod tests {
    use super::parse;

    /// Written by the ds_store package (as dmgbuild writes them): three
    /// icons placed, the folder arranged freely.
    #[test]
    fn reads() {
        let store = parse(include_bytes!("../tests/fixtures/DS_Store")).unwrap();
        assert!(store.free);
        assert_eq!(store.positions.get("Invoices"), Some(&(120., 80.)));
        assert_eq!(store.positions.get("Photos"), Some(&(260., 80.)));
        assert_eq!(store.positions.get("Résumé.pdf"), Some(&(400., 210.)));
        assert_eq!(store.positions.len(), 3);
    }

    /// Real ones, by Finder: F2_DS_STORES, a file listing their paths.
    #[test]
    #[ignore]
    fn real() {
        let list = std::fs::read_to_string(std::env::var("F2_DS_STORES").unwrap()).unwrap();
        for path in list.lines() {
            let bytes = std::fs::read(path).unwrap();
            let store = parse(&bytes);
            println!("{} · {:?}", bytes.len(), store.as_ref().map(|s| (s.positions.len(), s.free)));
            assert!(store.is_some(), "{path}");
        }
    }

    /// F2's own DMG's (dmgbuild's): F2 beside Applications, over a picture,
    /// in a window with no toolbar or sidebar.
    #[test]
    fn disk_image() {
        use super::{Background, Window};
        let store = parse(include_bytes!("../tests/fixtures/DS_Store-dmg")).unwrap();
        assert!(store.free);
        assert_eq!(store.positions.get("F2.app"), Some(&(160., 175.)));
        assert_eq!(store.positions.get("Applications"), Some(&(480., 175.)));
        assert_eq!(store.look.icon_size, Some(128.));
        assert_eq!(store.look.text_size, Some(13.));
        assert_eq!(store.look.background, Background::Picture { path: Some("/.background.tiff".into()), name: Some(".background.tiff".into()) });
        assert_eq!(store.window, Some(Window { size: (640., 432.), toolbar: false, sidebar: false }));
    }

    /// Version 3 aliases have no name in them, only extras.
    #[test]
    fn alias_v3() {
        let mut alias = vec![0u8; 58];
        alias[6..8].copy_from_slice(&3u16.to_be_bytes());
        let path = b"/.background/arrow.png";
        alias.extend_from_slice(&18u16.to_be_bytes());
        alias.extend_from_slice(&(path.len() as u16).to_be_bytes());
        alias.extend_from_slice(path);
        alias.extend_from_slice(&[0xff, 0xff, 0, 0]);
        assert_eq!(super::alias_target(&alias), (Some("/.background/arrow.png".into()), Some("arrow.png".into())));
        assert_eq!(super::alias_target(b"nonsense"), (None, None));
    }

    #[test]
    fn refuses() {
        assert_eq!(parse(b""), None);
        assert_eq!(parse(b"\0\0\0\x01Bud1"), None);
        let mut bad = include_bytes!("../tests/fixtures/DS_Store").to_vec();
        bad.truncate(600);
        assert_eq!(parse(&bad), None);
    }
}
