//! Quick Open's Recent, as Raycast's: what was chosen there (an app, a file
//! or folder, a command, a website), newest first. Kept in the data folder
//! as soon as it's chosen, so a quit (or a crash) loses none of it.

use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// How many are kept (more than are shown: some go missing, moved or
/// deleted).
const KEPT: usize = 40;

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Chosen {
    Path(PathBuf),
    /// A command, by its key (see `commands::by_key`).
    Command(String),
}

static LIST: Mutex<Option<Vec<Chosen>>> = Mutex::new(None);

fn file() -> PathBuf {
    crate::settings::data_dir().join("quick-open-recent.json")
}

fn load() -> Vec<Chosen> {
    std::fs::read(file()).ok().and_then(|text| serde_json::from_slice(&text).ok()).unwrap_or_default()
}

/// What was chosen, newest first.
pub fn list() -> Vec<Chosen> {
    LIST.lock().unwrap().get_or_insert_with(load).clone()
}

/// Remember that `chosen` was chosen (first, now: once in the list).
pub fn chose(chosen: Chosen) {
    let mut list = LIST.lock().unwrap();
    let list = list.get_or_insert_with(load);
    list.retain(|c| *c != chosen);
    list.insert(0, chosen);
    list.truncate(KEPT);
    let file = file();
    let tmp = file.with_extension("new");
    let saved = serde_json::to_vec_pretty(&*list)
        .map_err(std::io::Error::other)
        .and_then(|text| {
            if let Some(dir) = file.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::write(&tmp, text)
        })
        .and_then(|_| std::fs::rename(&tmp, &file));
    if let Err(e) = saved {
        eprintln!("couldn't save Quick Open's Recent: {e}");
    }
}
