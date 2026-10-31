//! Another terminal's look, imported into the notch's terminal's: its
//! colors (a theme of your own, named for where it came from), and its
//! font, size, background's opacity and cursor, where it says. From
//! Ghostty's config (and the theme it names), Terminal's and iTerm2's
//! default profiles (src/macos/terminal.m reads them: their colors are
//! archived), Alacritty's TOML, Kitty's config (and what it includes) and
//! WezTerm's Lua (its color scheme by name, if F2 comes with it).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::theme::{Colors, bundled, hex};
use crate::settings::{QuickTerminal, TerminalCursor, TerminalTheme};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Source {
    Ghostty,
    Terminal,
    Iterm,
    Alacritty,
    Kitty,
    Wezterm,
}

/// What came: a theme (colors), or one F2 comes with (by name), and the
/// rest where it said.
#[derive(Default, Debug)]
pub struct Imported {
    pub theme: Option<TerminalTheme>,
    pub bundled: Option<String>,
    pub font_family: Option<String>,
    pub font_size: Option<f32>,
    pub opacity: Option<f32>,
    pub cursor: Option<TerminalCursor>,
    pub blink: Option<bool>,
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

fn config_home() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".config"))
}

impl Source {
    pub const ALL: [Source; 6] = [Source::Ghostty, Source::Terminal, Source::Iterm, Source::Alacritty, Source::Kitty, Source::Wezterm];

    pub fn name(self) -> &'static str {
        match self {
            Source::Ghostty => "Ghostty",
            Source::Terminal => "Terminal",
            Source::Iterm => "iTerm2",
            Source::Alacritty => "Alacritty",
            Source::Kitty => "Kitty",
            Source::Wezterm => "WezTerm",
        }
    }

    /// Its config files, the first there first.
    fn files(self) -> Vec<PathBuf> {
        let (config, home) = (config_home(), home());
        let candidates = match self {
            Source::Ghostty => {
                let support = home.join("Library/Application Support/com.mitchellh.ghostty");
                vec![config.join("ghostty/config"), config.join("ghostty/config.ghostty"), support.join("config"), support.join("config.ghostty")]
            }
            Source::Alacritty => vec![config.join("alacritty/alacritty.toml"), home.join(".alacritty.toml")],
            Source::Kitty => vec![config.join("kitty/kitty.conf")],
            Source::Wezterm => vec![home.join(".wezterm.lua"), config.join("wezterm/wezterm.lua")],
            Source::Terminal | Source::Iterm => Vec::new(),
        };
        candidates.into_iter().filter(|p| p.is_file()).collect()
    }

    /// Whether this Mac has it to import from.
    pub fn found(self) -> bool {
        match self {
            Source::Terminal => true,
            Source::Iterm => home().join("Library/Preferences/com.googlecode.iterm2.plist").is_file(),
            Source::Ghostty => !self.files().is_empty() || Path::new("/Applications/Ghostty.app").exists(),
            _ => !self.files().is_empty(),
        }
    }

    pub fn import(self) -> Result<Imported, String> {
        match self {
            Source::Ghostty => Ok(ghostty(&self.files())),
            Source::Terminal | Source::Iterm => {
                let json = crate::macos::terminal_look(self == Source::Iterm).ok_or_else(|| format!("{}'s settings couldn't be read.", self.name()))?;
                from_json(&json, self.name())
            }
            Source::Alacritty => alacritty(self.files().first().ok_or("Alacritty has no config.")?),
            Source::Kitty => Ok(kitty(self.files().first().ok_or("Kitty has no config.")?)),
            Source::Wezterm => Ok(wezterm(self.files().first().ok_or("WezTerm has no config.")?)),
        }
    }
}

/// What came put in Settings: the theme yours (the one of the same name
/// replaced) and chosen. What was taken, said.
pub fn apply(source: Source, imported: Imported, settings: &mut QuickTerminal) -> String {
    let mut took = Vec::new();
    if let Some(theme) = imported.theme {
        let name = theme.name.clone();
        settings.themes.retain(|t| t.name != name);
        settings.themes.push(theme);
        settings.look.theme = name.clone();
        took.push(format!("its colors (as “{name}”)"));
    } else if let Some(name) = imported.bundled {
        took.push(format!("its theme ({name})"));
        settings.look.theme = name;
    }
    if let Some(family) = imported.font_family {
        took.push(format!("its font ({family})"));
        settings.look.font_family = Some(family);
    }
    if let Some(size) = imported.font_size {
        settings.look.font_size = size;
        took.push(format!("its size ({size})"));
    }
    if let Some(opacity) = imported.opacity {
        settings.look.opacity = opacity.clamp(0.3, 1.);
    }
    if let Some(cursor) = imported.cursor {
        settings.look.cursor = cursor;
    }
    if let Some(blink) = imported.blink {
        settings.look.blink = blink;
    }
    if took.is_empty() {
        return format!("{} didn't say how it looks: nothing was taken.", source.name());
    }
    format!("From {}: {}.", source.name(), took.join(", "))
}

/// Colors read from settings ("palette", "background"…) into a theme, if
/// there are enough: a background, a foreground and the 16 colors (where
/// some are missing, `fallback`'s).
fn theme(name: String, colors: &HashMap<String, u32>, fallback: &Colors) -> Option<TerminalTheme> {
    let background = *colors.get("background")?;
    let foreground = *colors.get("foreground")?;
    let mut palette = fallback.palette;
    for (i, color) in palette.iter_mut().enumerate() {
        if let Some(c) = colors.get(&format!("color{i}")) {
            *color = *c;
        }
    }
    let c = Colors {
        palette,
        background,
        foreground,
        cursor: colors.get("cursor").copied(),
        cursor_text: colors.get("cursor_text").copied(),
        selection: colors.get("selection").copied(),
        selected_text: colors.get("selected_text").copied(),
    };
    Some(c.named(&name))
}

/// xterm's 16, where a terminal doesn't say.
const XTERM: Colors = Colors {
    palette: [0x000000, 0xcd0000, 0x00cd00, 0xcdcd00, 0x0000ee, 0xcd00cd, 0x00cdcd, 0xe5e5e5, 0x7f7f7f, 0xff0000, 0x00ff00, 0xffff00, 0x5c5cff, 0xff00ff, 0x00ffff, 0xffffff],
    background: 0x000000,
    foreground: 0xffffff,
    cursor: None,
    cursor_text: None,
    selection: None,
    selected_text: None,
};

// ---- Ghostty: `key = value`, the theme it names first.

fn ghostty(files: &[PathBuf]) -> Imported {
    let lines: Vec<String> = files.iter().filter_map(|p| std::fs::read_to_string(p).ok()).flat_map(|s| s.lines().map(str::to_string).collect::<Vec<_>>()).collect();
    let setting = |line: &str| -> Option<(String, String)> {
        let line = line.trim();
        if line.starts_with('#') {
            return None;
        }
        let (k, v) = line.split_once('=')?;
        Some((k.trim().to_string(), v.trim().trim_matches('"').to_string()))
    };
    let settings: Vec<(String, String)> = lines.iter().filter_map(|l| setting(l)).collect();
    let theme_name = settings.iter().rev().find(|(k, _)| k == "theme").map(|(_, v)| v.clone());
    let mut colors = HashMap::new();
    let mut read = |key: &str, value: &str| match key {
        "palette" => {
            if let Some((n, c)) = value.split_once('=')
                && let (Ok(n), Some(c)) = (n.trim().parse::<usize>(), hex(c))
                && n < 16
            {
                colors.insert(format!("color{n}"), c);
            }
        }
        "background" | "foreground" => _ = hex(value).map(|c| colors.insert(key.to_string(), c)),
        "cursor-color" => _ = hex(value).map(|c| colors.insert("cursor".into(), c)),
        "cursor-text" => _ = hex(value).map(|c| colors.insert("cursor_text".into(), c)),
        "selection-background" => _ = hex(value).map(|c| colors.insert("selection".into(), c)),
        "selection-foreground" => _ = hex(value).map(|c| colors.insert("selected_text".into(), c)),
        _ => {}
    };
    // (A light theme and a dark: the dark, as the notch is.)
    let theme_file = theme_name.as_ref().map(|name| name.split(',').find_map(|p| p.trim().strip_prefix("dark:")).unwrap_or(name).trim().to_string());
    let mut base = None;
    if let Some(name) = &theme_file {
        let dirs = [config_home().join("ghostty/themes"), PathBuf::from("/Applications/Ghostty.app/Contents/Resources/ghostty/themes"), home().join("Applications/Ghostty.app/Contents/Resources/ghostty/themes")];
        let file = if name.starts_with('/') { Some(PathBuf::from(name)) } else { dirs.iter().map(|d| d.join(name)).find(|p| p.is_file()) };
        match file.and_then(|f| std::fs::read_to_string(f).ok()) {
            Some(text) => text.lines().filter_map(setting).for_each(|(k, v)| read(&k, &v)),
            // (Not on this Mac: one F2 comes with, if it's of the same name.)
            None => base = bundled().iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(n, _)| n.clone()),
        }
    }
    for (k, v) in &settings {
        read(k, v);
    }
    let named = theme_file.map_or("Ghostty".to_string(), |t| format!("Ghostty: {t}"));
    let get = |key: &str| settings.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v.clone());
    let cursor = get("cursor-style").and_then(|s| match s.as_str() {
        "bar" => Some(TerminalCursor::Bar),
        "underline" => Some(TerminalCursor::Underline),
        "block" | "block_hollow" => Some(TerminalCursor::Block),
        _ => None,
    });
    Imported {
        theme: theme(named, &colors, &XTERM),
        bundled: base,
        // (Ghostty's own font, where it names none.)
        font_family: settings.iter().find(|(k, v)| k == "font-family" && !v.is_empty()).map(|(_, v)| v.clone()).or_else(|| Some("JetBrains Mono".into())),
        font_size: get("font-size").and_then(|s| s.parse().ok()).or(Some(13.)),
        opacity: get("background-opacity").and_then(|s| s.parse().ok()),
        cursor,
        blink: get("cursor-style-blink").map(|s| s == "true"),
    }
}

// ---- Terminal and iTerm2: what terminal.m read, as JSON.

#[derive(serde::Deserialize)]
struct Profile {
    name: String,
    #[serde(default)]
    colors: HashMap<String, String>,
    font_family: Option<String>,
    font_size: Option<f32>,
    opacity: Option<f32>,
    cursor: Option<String>,
    blink: Option<bool>,
}

fn from_json(json: &str, app: &str) -> Result<Imported, String> {
    let p: Profile = serde_json::from_str(json).map_err(|_| format!("{app}'s settings couldn't be read."))?;
    let colors: HashMap<String, u32> = p.colors.iter().filter_map(|(k, v)| Some((k.clone(), hex(v)?))).collect();
    Ok(Imported {
        theme: theme(format!("{app}: {}", p.name), &colors, &XTERM),
        bundled: None,
        font_family: p.font_family,
        font_size: p.font_size,
        opacity: p.opacity,
        cursor: p.cursor.as_deref().and_then(cursor_named),
        blink: p.blink,
    })
}

fn cursor_named(name: &str) -> Option<TerminalCursor> {
    match name.to_ascii_lowercase().as_str() {
        "block" | "box" => Some(TerminalCursor::Block),
        "bar" | "beam" | "vertical" => Some(TerminalCursor::Bar),
        "underline" => Some(TerminalCursor::Underline),
        _ => None,
    }
}

// ---- Alacritty: TOML, with what it imports under it.

fn alacritty(file: &Path) -> Result<Imported, String> {
    let read = |path: &Path| -> Option<toml::Table> { std::fs::read_to_string(path).ok()?.parse::<toml::Table>().ok() };
    let main = read(file).ok_or("Alacritty's config couldn't be read.")?;
    // (What it imports goes under it: its own settings win.)
    let imports = main.get("general").and_then(|g| g.get("import")).or_else(|| main.get("import")).and_then(|i| i.as_array()).cloned().unwrap_or_default();
    let mut tables: Vec<toml::Table> = imports
        .iter()
        .filter_map(|i| i.as_str())
        .map(|p| p.strip_prefix("~/").map_or_else(|| PathBuf::from(p), |rest| home().join(rest)))
        .map(|p| if p.is_relative() { file.parent().unwrap_or(Path::new("/")).join(p) } else { p })
        .filter_map(|p| read(&p))
        .collect();
    tables.push(main);
    let get = |path: &[&str]| -> Option<toml::Value> {
        tables.iter().rev().find_map(|t| {
            let mut v = t.get(path[0])?;
            for key in &path[1..] {
                v = v.get(key)?;
            }
            Some(v.clone())
        })
    };
    let color = |path: &[&str]| get(path).and_then(|v| v.as_str().and_then(hex));
    let mut colors = HashMap::new();
    for (key, path) in [("background", ["primary", "background"]), ("foreground", ["primary", "foreground"]), ("cursor", ["cursor", "cursor"]), ("cursor_text", ["cursor", "text"]), ("selection", ["selection", "background"]), ("selected_text", ["selection", "text"])] {
        if let Some(c) = color(&["colors", path[0], path[1]]) {
            colors.insert(key.to_string(), c);
        }
    }
    let names = ["black", "red", "green", "yellow", "blue", "magenta", "cyan", "white"];
    for (bank, offset) in [("normal", 0), ("bright", 8)] {
        for (i, name) in names.iter().enumerate() {
            if let Some(c) = color(&["colors", bank, name]) {
                colors.insert(format!("color{}", i + offset), c);
            }
        }
    }
    let number = |v: toml::Value| v.as_float().or_else(|| v.as_integer().map(|i| i as f64)).map(|f| f as f32);
    Ok(Imported {
        theme: theme("Alacritty".into(), &colors, &XTERM),
        bundled: None,
        font_family: get(&["font", "normal", "family"]).and_then(|v| v.as_str().map(str::to_string)),
        font_size: get(&["font", "size"]).and_then(number),
        opacity: get(&["window", "opacity"]).and_then(number),
        cursor: get(&["cursor", "style", "shape"]).or_else(|| get(&["cursor", "style"])).and_then(|v| v.as_str().and_then(cursor_named)),
        blink: get(&["cursor", "style", "blinking"]).and_then(|v| v.as_str().map(|b| b == "On" || b == "Always")),
    })
}

// ---- Kitty: `key value`, with `include`s.

fn kitty(file: &Path) -> Imported {
    let mut settings = Vec::new();
    fn read(path: &Path, depth: usize, out: &mut Vec<(String, String)>) {
        let Ok(text) = std::fs::read_to_string(path) else { return };
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (key, value) = line.split_once(char::is_whitespace).map_or((line, ""), |(k, v)| (k, v.trim()));
            if key == "include" && depth < 4 {
                let p = value.strip_prefix("~/").map_or_else(|| PathBuf::from(value), |rest| home().join(rest));
                let p = if p.is_relative() { path.parent().unwrap_or(Path::new("/")).join(p) } else { p };
                read(&p, depth + 1, out);
            } else {
                out.push((key.to_string(), value.to_string()));
            }
        }
    }
    read(file, 0, &mut settings);
    let get = |key: &str| settings.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v.clone());
    let mut colors = HashMap::new();
    for (key, name) in [("background", "background"), ("foreground", "foreground"), ("cursor", "cursor"), ("cursor_text_color", "cursor_text"), ("selection_background", "selection"), ("selection_foreground", "selected_text")] {
        if let Some(c) = get(key).as_deref().and_then(hex) {
            colors.insert(name.to_string(), c);
        }
    }
    for i in 0..16 {
        if let Some(c) = get(&format!("color{i}")).as_deref().and_then(hex) {
            colors.insert(format!("color{i}"), c);
        }
    }
    Imported {
        theme: theme("Kitty".into(), &colors, &XTERM),
        bundled: None,
        font_family: get("font_family").filter(|f| f != "monospace" && !f.is_empty()),
        font_size: get("font_size").and_then(|s| s.parse().ok()),
        opacity: get("background_opacity").and_then(|s| s.parse().ok()),
        cursor: get("cursor_shape").as_deref().and_then(cursor_named),
        blink: get("cursor_blink_interval").map(|s| s.parse::<f32>() != Ok(0.)),
    }
}

// ---- WezTerm: Lua, what's plain in it.

fn wezterm(file: &Path) -> Imported {
    let text = std::fs::read_to_string(file).unwrap_or_default();
    // `key = 'value'` or `key = 12`, outside comments.
    let value = |key: &str| -> Option<String> {
        text.lines().filter(|l| !l.trim_start().starts_with("--")).find_map(|line| {
            let at = line.find(key)?;
            let rest = line[at + key.len()..].trim_start().strip_prefix('=')?.trim();
            let rest = rest.trim_end_matches(',').trim();
            Some(rest.trim_matches(|c| c == '"' || c == '\'').to_string())
        })
    };
    let font = text.find("wezterm.font").and_then(|at| {
        let rest = &text[at..];
        let start = rest.find(['"', '\''])?;
        let quote = rest[start..].chars().next()?;
        let name = &rest[start + 1..];
        Some(name[..name.find(quote)?].to_string())
    });
    let scheme = value("color_scheme");
    Imported {
        theme: None,
        bundled: scheme.and_then(|s| bundled().iter().find(|(n, _)| n.eq_ignore_ascii_case(&s)).map(|(n, _)| n.clone())),
        font_family: font,
        font_size: value("font_size").and_then(|s| s.parse().ok()),
        opacity: value("window_background_opacity").and_then(|s| s.parse().ok()),
        cursor: value("default_cursor_style").and_then(|s| cursor_named(s.trim_start_matches("Blinking").trim_start_matches("Steady"))),
        blink: value("default_cursor_style").map(|s| s.starts_with("Blinking")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("f2-import-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (file, text) in files {
            let path = dir.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        dir
    }

    /// What this Mac's terminals would give (cargo test on_this_mac -- --ignored --nocapture).
    #[test]
    #[ignore]
    fn on_this_mac() {
        for source in Source::ALL.into_iter().filter(|s| s.found()) {
            println!("{}: {:#?}", source.name(), source.import());
        }
    }

    #[test]
    fn from_ghostty() {
        let dir = temp("ghostty", &[("config", "theme = /nowhere/none\nbackground = #101010\nforeground = #eeeeee\npalette = 1=#ff0000\nfont-family = Iosevka\nfont-size = 15\ncursor-style = bar\nbackground-opacity = 0.9\n")]);
        let i = ghostty(&[dir.join("config")]);
        let t = i.theme.unwrap();
        assert_eq!((t.name.as_str(), t.background.as_str(), t.palette[1].as_str()), ("Ghostty: /nowhere/none", "#101010", "#ff0000"));
        assert_eq!((i.font_family.as_deref(), i.font_size, i.cursor, i.opacity), (Some("Iosevka"), Some(15.), Some(TerminalCursor::Bar), Some(0.9)));
    }

    #[test]
    fn from_alacritty() {
        let dir = temp(
            "alacritty",
            &[
                ("theme.toml", "[colors.primary]\nbackground = '#000000'\nforeground = '#dddddd'\n[colors.normal]\nred = '#aa0000'\n"),
                ("alacritty.toml", "[general]\nimport = ['theme.toml']\n[colors.primary]\nbackground = '0x111111'\n[font]\nsize = 14.0\n[font.normal]\nfamily = 'Fira Code'\n[window]\nopacity = 0.95\n"),
            ],
        );
        let i = alacritty(&dir.join("alacritty.toml")).unwrap();
        let t = i.theme.unwrap();
        assert_eq!((t.background.as_str(), t.foreground.as_str(), t.palette[1].as_str()), ("#111111", "#dddddd", "#aa0000"), "its own over what it imports");
        assert_eq!((i.font_family.as_deref(), i.font_size, i.opacity), (Some("Fira Code"), Some(14.), Some(0.95)));
    }

    #[test]
    fn from_kitty_and_wezterm() {
        let dir = temp("kitty", &[("theme.conf", "background #202020\nforeground #f0f0f0\ncolor2 #00aa00\n"), ("kitty.conf", "include theme.conf\nfont_size 12.5\ncursor_shape beam\n")]);
        let i = kitty(&dir.join("kitty.conf"));
        assert_eq!(i.theme.unwrap().palette[2], "#00aa00");
        assert_eq!((i.font_size, i.cursor), (Some(12.5), Some(TerminalCursor::Bar)));
        let dir = temp("wezterm", &[(".wezterm.lua", "config.color_scheme = 'carbonfox'\nconfig.font = wezterm.font('JetBrains Mono')\nconfig.font_size = 14\n")]);
        let i = wezterm(&dir.join(".wezterm.lua"));
        assert_eq!((i.bundled.as_deref(), i.font_family.as_deref(), i.font_size), (Some("Carbonfox"), Some("JetBrains Mono"), Some(14.)));
    }
}
