//! The terminal and the code editor Finder2 opens things in ("Open in
//! Ghostty", "Open in Zed"): the ones chosen in Settings, else the first
//! installed of those it knows, in this order.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::settings::Settings;

const TERMINALS: &[&str] = &[
    "com.mitchellh.ghostty",
    "com.cmuxterm.app",
    "com.googlecode.iterm2",
    "dev.warp.Warp-Stable",
    "net.kovidgoyal.kitty",
    "com.github.wez.wezterm",
    "org.alacritty",
    "com.apple.Terminal",
];

const EDITORS: &[&str] = &[
    "dev.zed.Zed",
    "com.todesktop.230313mzl4w4u92",
    "com.microsoft.VSCode",
    "com.exafunction.windsurf",
    "com.sublimetext.4",
    "com.panic.Nova",
    "com.apple.dt.Xcode",
];

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    Terminal,
    Editor,
}

#[derive(Clone)]
pub struct Tool {
    pub id: &'static str,
    pub name: String,
    pub app: PathBuf,
}

/// Those of a kind installed (looked up once a run).
pub fn installed(kind: Kind) -> &'static [Tool] {
    static FOUND: OnceLock<[Vec<Tool>; 2]> = OnceLock::new();
    let found = FOUND.get_or_init(|| {
        let find = |ids: &[&'static str]| {
            ids.iter()
                .filter_map(|&id| {
                    let app = crate::macos::app_path(id)?;
                    let name = app.file_stem()?.to_string_lossy().into_owned();
                    Some(Tool { id, name, app })
                })
                .collect()
        };
        [find(TERMINALS), find(EDITORS)]
    });
    &found[kind as usize]
}

/// The one to use: Settings' choice if it's installed, else the first.
pub fn chosen(kind: Kind, settings: &Settings) -> Option<Tool> {
    let want = match kind {
        Kind::Terminal => &settings.terminal,
        Kind::Editor => &settings.editor,
    };
    let all = installed(kind);
    all.iter().find(|t| want.as_deref() == Some(t.id)).or_else(|| all.first()).cloned()
}

/// Open `paths` in it: a terminal at each folder (a file's own), an editor
/// on each.
pub fn open(tool: &Tool, kind: Kind, paths: &[PathBuf]) {
    let mut targets: Vec<&Path> = Vec::new();
    for path in paths {
        let target = if kind == Kind::Terminal && !path.is_dir() { path.parent().unwrap_or(path) } else { path };
        if !targets.contains(&target) {
            targets.push(target);
        }
    }
    for target in targets {
        crate::macos::open_with(target, &tool.app);
    }
}
