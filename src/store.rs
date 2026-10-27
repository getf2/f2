//! The extension store: extensions from github.com/getf2/extensions (F2's
//! own and others'), published to GitHub Pages. Quick Open's Extension
//! Store (a page of F2's own) lists them, to install, update and take out.
//! Its list is signed (Ed25519) by a key F2 has (`KEYS`), and each file
//! installed is checked against it (SHA-256): what's installed is what was
//! merged there, whoever serves it. Those installed are in Extensions, a
//! folder each named by its id, and in settings' `store_installed`; what
//! they keep (passwords, storage) is kept as the store's (`kept_as`).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{Value, json};

use crate::extensions::{self, Extension, Tell};
use crate::settings::Settings;

/// Where it's published.
const HOME: &str = "https://getf2.github.io/extensions/";
/// The keys its list may be signed by (Ed25519's, base64): a new one's
/// added here, in an F2 that's out, before it's used.
const KEYS: [&str; 1] = ["bnHwDMOGx6BkP1VIBQJUPkJ6wXs10hLLWlyWuJE0wjQ="];
/// An extension's source, to read.
const SOURCE: &str = "https://github.com/getf2/extensions/tree/main/extensions/";
/// How long its list is kept before it's asked for again.
const FRESH: Duration = Duration::from_secs(10 * 60);

/// An extension, as the store's list has it.
#[derive(Deserialize, Clone, Debug)]
pub struct Listed {
    pub id: String,
    pub version: String,
    pub api: u32,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub author: String,
    #[serde(default)]
    pub network: Vec<String>,
    #[serde(default)]
    pub commands: Vec<ListedCommand>,
    #[serde(default)]
    pub preferences: Vec<ListedPreference>,
    pub updated: Option<String>,
    pub path: String,
    pub files: Vec<ListedFile>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ListedCommand {
    pub id: String,
    pub title: String,
    pub keyword: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ListedPreference {
    pub title: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub required: bool,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ListedFile {
    pub name: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Deserialize)]
struct List {
    extensions: Vec<Listed>,
}

/// What's at `url` (waits on the network: not on the main thread).
fn get(url: &str) -> Result<Vec<u8>, String> {
    let out = std::process::Command::new("/usr/bin/curl").args(["-fsSL", "--max-time", "30", url]).output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(out.stdout)
    } else {
        Err(format!("The store couldn't be reached ({}).", String::from_utf8_lossy(&out.stderr).trim().trim_start_matches("curl: ")))
    }
}

/// `list` is signed (`signature`: base64) by one of `keys`.
fn signed(list: &[u8], signature: &[u8], keys: &[&str]) -> bool {
    let Some(signature) = base64(std::str::from_utf8(signature).unwrap_or_default().trim()) else { return false };
    keys.iter().filter_map(|k| base64(k)).any(|key| ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, key).verify(list, &signature).is_ok())
}

fn base64(text: &str) -> Option<Vec<u8>> {
    const DIGITS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let (mut out, mut bits, mut n) = (Vec::new(), 0u32, 0u32);
    for c in text.trim_end_matches('=').bytes() {
        n = n << 6 | DIGITS.iter().position(|d| *d == c)? as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((n >> bits) as u8);
        }
    }
    Some(out)
}

fn sha256(bytes: &[u8]) -> String {
    ring::digest::digest(&ring::digest::SHA256, bytes).as_ref().iter().map(|b| format!("{b:02x}")).collect()
}

/// The store's list: fetched and its signature checked, or kept from in
/// the last few minutes (unless `fresh`). Waits on the network.
pub fn list(fresh: bool) -> Result<Vec<Listed>, String> {
    static KEPT: Mutex<Option<(Instant, Vec<Listed>)>> = Mutex::new(None);
    if !fresh
        && let Some((at, list)) = KEPT.lock().unwrap().as_ref()
        && at.elapsed() < FRESH
    {
        return Ok(list.clone());
    }
    let (bytes, signature) = (get(&format!("{HOME}v1/index.json"))?, get(&format!("{HOME}v1/index.json.sig"))?);
    if !signed(&bytes, &signature, &KEYS) {
        return Err("The store's list isn't signed as F2 expects, so it wasn't used. A newer F2 may know its key.".into());
    }
    let list: List = serde_json::from_slice(&bytes).map_err(|e| format!("The store's list couldn't be read: {e}."))?;
    *KEPT.lock().unwrap() = Some((Instant::now(), list.extensions.clone()));
    Ok(list.extensions)
}

/// `listed` installed (or updated): each of its files fetched and checked
/// against the list, into a folder of its own in Extensions, put in place
/// of what was there at once. Waits on the network.
pub fn install(listed: &Listed) -> Result<(), String> {
    if !extensions::valid_id(&listed.id) {
        return Err(format!("“{}” isn't an id.", listed.id));
    }
    if listed.api > extensions::API {
        return Err(format!("{} is for a newer F2: update F2, then install it.", listed.name));
    }
    let folder = extensions::folder();
    let partial = folder.join(format!(".installing-{}", listed.id));
    let _ = std::fs::remove_dir_all(&partial);
    std::fs::create_dir_all(&partial).map_err(|e| e.to_string())?;
    let fetched = (|| {
        for file in &listed.files {
            // (A file of its own: in its folder, not anywhere else.)
            if file.name.is_empty() || file.name.contains('/') || file.name.starts_with('.') {
                return Err(format!("{}'s list names a file “{}”, which F2 won't write.", listed.name, file.name));
            }
            let bytes = get(&format!("{HOME}{}{}", listed.path, file.name))?;
            if bytes.len() as u64 != file.size || sha256(&bytes) != file.sha256 {
                return Err(format!("{}'s {} isn't what the store's list says it is, so it wasn't installed.", listed.name, file.name));
            }
            std::fs::write(partial.join(&file.name), bytes).map_err(|e| e.to_string())?;
        }
        let manifest: Value = serde_json::from_slice(&std::fs::read(partial.join("f2.json")).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        if manifest["id"] != listed.id.as_str() || manifest["version"] != listed.version.as_str() {
            return Err(format!("{}'s manifest isn't the one the store's list says.", listed.name));
        }
        Ok(())
    })();
    if let Err(why) = fetched {
        let _ = std::fs::remove_dir_all(&partial);
        return Err(why);
    }
    let dest = folder.join(&listed.id);
    if dest.symlink_metadata().is_ok() {
        std::fs::remove_dir_all(&dest).map_err(|e| e.to_string())?;
    }
    std::fs::rename(&partial, &dest).map_err(|e| e.to_string())?;
    extensions::look_again();
    Ok(())
}

/// What's installed where Extensions has it: those from the store (by id:
/// their versions, and the hosts they reach), and the ids of the others.
struct Here {
    store: BTreeMap<String, (String, Vec<String>)>,
    local: BTreeMap<String, String>,
}

fn here() -> Here {
    let (exts, _) = extensions::every();
    let mut here = Here { store: BTreeMap::new(), local: BTreeMap::new() };
    for e in exts {
        let network = e.manifest.permissions.network.clone();
        if e.from_store() {
            here.store.insert(e.id.clone(), (e.manifest.version.clone().unwrap_or_default(), network));
        } else {
            here.local.insert(e.id.clone(), e.dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
        }
    }
    here
}

/// "1.10.0" after "1.9.2".
fn newer(a: &str, b: &str) -> bool {
    let parts = |v: &str| v.split('.').map(|p| p.parse::<u64>().unwrap_or(0)).collect::<Vec<_>>();
    parts(a) > parts(b)
}

// Quick Open's page.

/// The page of F2's own that's the store: `browse`, and its rows' actions.
pub fn extension() -> Arc<Extension> {
    static IT: OnceLock<Arc<Extension>> = OnceLock::new();
    IT.get_or_init(|| {
        let manifest = json!({
            "api": 1,
            "id": "f2.store",
            "name": "Extension Store",
            "icon": "puzzlepiece.extension.fill",
            "color": "#bf5af2",
            "commands": [
                { "id": "browse", "title": "Extension Store" },
                { "id": "install", "title": "Install", "hidden": true },
                { "id": "uninstall", "title": "Uninstall", "hidden": true },
                { "id": "settings", "title": "Settings", "hidden": true },
                { "id": "open", "title": "Open", "hidden": true }
            ]
        });
        Arc::new(Extension { id: "f2.store".into(), dir: Default::default(), manifest: serde_json::from_value(manifest).expect("the store's manifest"), native: Some(page) })
    })
    .clone()
}

fn page(command: &str, args: &Value, _query: &str, tell: Tell, cx: &mut gpui::App) {
    let id = args.get("id").and_then(Value::as_str).unwrap_or_default().to_string();
    match command {
        "browse" => {
            tell.say(json!({ "loading": "Getting the store's list…", "placeholder": "Search the store…" }));
            let here = here();
            cx.background_executor()
                .spawn(async move {
                    match list(false) {
                        Ok(list) => tell.say(json!({ "items": rows(&list, &here), "loading": false })),
                        Err(why) => tell.say(json!({ "error": why, "loading": false })),
                    }
                })
                .detach();
        }
        "install" => {
            let here = here();
            if let Some(folder) = here.local.get(&id) {
                return tell.say(json!({ "notice": { "title": "Not Installed", "text": format!("Extensions has one with its id already, in {folder}: take that out first.") } }));
            }
            tell.say(json!({ "loading": "Installing…" }));
            cx.spawn(async move |cx| {
                let done = cx
                    .background_executor()
                    .spawn(async move {
                        let listed = list(false)?.into_iter().find(|l| l.id == id).ok_or("The store doesn't have it now.")?;
                        install(&listed).map(|()| listed)
                    })
                    .await;
                cx.update(|cx| match done {
                    Ok(listed) => {
                        Settings::update(cx, |s| {
                            s.store_installed.insert(listed.id.clone(), listed.version.clone());
                        });
                        let how = match listed.commands.iter().find_map(|c| c.keyword.clone()) {
                            Some(keyword) => format!("Type “{keyword}” in Quick Open, or its commands' names."),
                            None => "Type its commands' names in Quick Open.".into(),
                        };
                        tell.say(json!({ "notice": { "title": format!("Installed {}", listed.name), "text": how }, "refresh": true, "loading": false }));
                    }
                    Err(why) => tell.say(json!({ "notice": { "title": "Couldn't Install It", "text": why }, "loading": false })),
                });
            })
            .detach();
        }
        "uninstall" => {
            let Some(ext) = extensions::every().0.into_iter().find(|e| e.id == id && e.from_store()) else {
                return tell.say(json!({ "notice": "It isn't installed from the store." }));
            };
            let name = ext.manifest.name.clone();
            ext.forget_passwords();
            if let Err(e) = std::fs::remove_dir_all(&ext.dir) {
                return tell.say(json!({ "notice": { "title": format!("Couldn't Uninstall {name}"), "text": e.to_string() } }));
            }
            let _ = std::fs::remove_file(crate::settings::data_dir().join("Extension Storage").join(format!("store.{id}.json")));
            extensions::forget_shown(&id);
            extensions::look_again();
            // (Once this page's done: settings changing calls back into it.)
            cx.defer(move |cx| {
                Settings::update(cx, |s| {
                    s.store_installed.remove(&id);
                    s.extension_preferences.remove(&id);
                    s.disabled_extensions.retain(|d| *d != id);
                })
            });
            tell.say(json!({ "notice": format!("Uninstalled {name}"), "refresh": true }));
        }
        "settings" => {
            tell.say(json!({ "close": true }));
            cx.defer(move |cx| {
                crate::settings_window::open_extension(&id, cx);
                cx.activate(true);
            });
        }
        "open" => {
            let command = args.get("command").and_then(Value::as_str).unwrap_or_default().to_string();
            cx.defer(move |cx| crate::quick_panel::show_extension(&id, &command, cx));
        }
        _ => tell.say(json!({ "error": format!("The store has no \"{command}\".") })),
    }
}

/// Its rows: updates, those installed, then the rest, each with what it
/// does and may do beside it.
fn rows(list: &[Listed], here: &Here) -> Vec<Value> {
    let mut rows: Vec<(u8, Value)> = list.iter().map(|l| row(l, here)).collect();
    rows.sort_by_key(|(order, _)| *order);
    rows.into_iter().map(|(_, row)| row).collect()
}

fn row(l: &Listed, here: &Here) -> (u8, Value) {
    let source = json!({ "title": "View Source", "open": format!("{SOURCE}{}", l.id), "icon": "chevron.left.forwardslash.chevron.right" });
    let settings = json!({ "title": "Settings…", "run": { "command": "settings", "args": { "id": l.id } }, "icon": "gearshape" });
    let uninstall = json!({ "title": "Uninstall", "run": { "command": "uninstall", "args": { "id": l.id } }, "icon": "trash", "confirm": format!("Uninstall {}?", l.name), "destructive": true });
    let open = l.commands.first().map(|c| json!({ "title": format!("Open {}", c.title), "run": { "command": "open", "args": { "id": l.id, "command": c.id } }, "icon": "arrow.up.forward.app" }));
    let install = json!({ "title": "Install", "run": { "command": "install", "args": { "id": l.id } }, "icon": "arrow.down.circle" });
    let by = if l.author == "f2" { "F2".to_string() } else { format!("@{}", l.author) };
    let (order, section, accessory, actions, note) = if l.api > extensions::API {
        (3, "In the Store", "Needs a newer F2".to_string(), vec![source], Some("It's for a newer F2: update F2 to install it.".to_string()))
    } else if let Some((version, network)) = here.store.get(&l.id) {
        if newer(&l.version, version) {
            // (Reaching more than it did: said first.)
            let more: Vec<&String> = l.network.iter().filter(|h| !network.contains(h)).collect();
            let mut update = json!({ "title": format!("Update to {}", l.version), "run": { "command": "install", "args": { "id": l.id } }, "icon": "arrow.down.circle" });
            if !more.is_empty() {
                update["confirm"] = json!(format!("It'll reach {} too: update?", more.iter().map(|h| h.as_str()).collect::<Vec<_>>().join(", ")));
            }
            (0, "Updates", format!("{version} → {}", l.version), vec![update, settings, source, uninstall], None)
        } else {
            (1, "Installed", "Installed".into(), [open, Some(settings), Some(source), Some(uninstall)].into_iter().flatten().collect(), None)
        }
    } else if let Some(folder) = here.local.get(&l.id) {
        let mine = format!("You have your own copy of it, in Extensions › {folder}: the store's can't be installed beside it (take yours out first).");
        (1, "Installed", "Yours".into(), [open, Some(source)].into_iter().flatten().collect(), Some(mine))
    } else {
        (2, "In the Store", by.clone(), vec![install, source], None)
    };
    let commands: String = l.commands.iter().map(|c| format!("\n- **{}**{}", c.title, c.keyword.as_ref().map(|k| format!(", or type `{k}` first")).unwrap_or_default())).collect();
    let asks: Vec<String> = l.preferences.iter().filter(|p| p.required).map(|p| if p.kind == "password" { format!("{} (kept in your keychain)", p.title) } else { p.title.clone() }).collect();
    let reaches = if l.network.is_empty() { "Nothing on the internet: it works on your Mac alone.".to_string() } else { l.network.iter().map(|h| format!("`{h}`")).collect::<Vec<_>>().join(", ") };
    let markdown = format!(
        "## {}\n\n{}\n\n**Its commands**{commands}\n\n**It may reach:** {reaches}{}{}",
        l.name,
        l.description,
        if asks.is_empty() { String::new() } else { format!("\n\n**It asks for:** {}", asks.join(", ")) },
        note.map(|n| format!("\n\n*{n}*")).unwrap_or_default(),
    );
    let mut facts = vec![json!({ "label": "Version", "text": l.version }), json!({ "label": "By", "text": by })];
    if let Some(day) = l.updated.as_deref().and_then(|u| u.get(..10)) {
        facts.push(json!({ "label": "Updated", "text": day }));
    }
    let icon = l.icon.clone().filter(|i| !i.contains('/') && !i.ends_with(".png")).unwrap_or_else(|| "puzzlepiece.extension.fill".into());
    let keywords: Vec<String> = [l.id.clone(), l.author.clone()].into_iter().chain(l.commands.iter().map(|c| c.title.clone())).chain(l.commands.iter().filter_map(|c| c.keyword.clone())).collect();
    (
        order,
        json!({
            "title": l.name,
            "subtitle": l.description,
            "icon": icon,
            "color": l.color,
            "section": section,
            "accessory": accessory,
            "keywords": keywords,
            "detail": { "markdown": markdown, "metadata": facts },
            "actions": actions,
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_signed_by_its_keys_only() {
        let rng = ring::rand::SystemRandom::new();
        let pkcs8 = ring::signature::Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
        let pair = ring::signature::Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap();
        use ring::signature::KeyPair;
        let b64 = |bytes: &[u8]| {
            const DIGITS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            bytes.chunks(3).map(|c| {
                let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
                (0..4).map(|i| if i <= c.len() { DIGITS[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' }).collect::<String>()
            }).collect::<String>()
        };
        let key = b64(pair.public_key().as_ref());
        assert_eq!(base64(&key).unwrap(), pair.public_key().as_ref());
        let list = br#"{"extensions": []}"#;
        let signature = b64(pair.sign(list).as_ref());
        assert!(signed(list, signature.as_bytes(), &[&key]));
        assert!(!signed(br#"{"extensions": [1]}"#, signature.as_bytes(), &[&key]));
        assert!(!signed(list, signature.as_bytes(), &KEYS));
        assert!(!signed(list, b"not base64!", &[&key]));
        assert_eq!(sha256(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert!(newer("1.10.0", "1.9.2") && !newer("1.0.0", "1.0.0") && !newer("0.9.0", "1.0.0"));
    }

    /// The store's own list, as published: `cargo test --release -- --ignored store_list_live`.
    #[test]
    #[ignore]
    fn store_list_live() {
        let list = list(true).unwrap();
        assert!(list.iter().any(|l| l.id == "f2.wikipedia"));
        println!("{} extensions", list.len());
    }
}
