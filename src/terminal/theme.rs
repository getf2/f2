//! The terminal's look, as Settings › Terminal has it (`TerminalLook`): a
//! theme it comes with (286 of iTerm2-Color-Schemes', as Ghostty's are:
//! assets/terminal/themes.tsv, scripts/terminal-themes.sh) or one of your
//! own (imported from another terminal, see `import`, or made in
//! Settings), its font, size, background's opacity and cursor.

use std::sync::OnceLock;

use crate::settings::{QuickTerminal, TerminalCursor, TerminalTheme};

/// A theme's colors, as 0xrrggbb.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Colors {
    pub palette: [u32; 16],
    pub background: u32,
    pub foreground: u32,
    pub cursor: Option<u32>,
    pub cursor_text: Option<u32>,
    pub selection: Option<u32>,
    pub selected_text: Option<u32>,
}

/// The terminal's look, as it's drawn.
pub struct Theme {
    /// The 256 colors (a theme's 16, then xterm's cube and grays).
    pub palette: [u32; 256],
    pub foreground: u32,
    pub background: u32,
    pub cursor: Option<u32>,
    pub cursor_text: Option<u32>,
    pub selection_background: Option<u32>,
    pub selection_foreground: Option<u32>,
    pub font_family: Option<String>,
    pub font_size: f32,
    /// The background's (0 to 1).
    pub opacity: f32,
    pub cursor_shape: TerminalCursor,
    pub blink: bool,
}

/// The themes it comes with, by name.
pub fn bundled() -> &'static [(String, Colors)] {
    static THEMES: OnceLock<Vec<(String, Colors)>> = OnceLock::new();
    THEMES.get_or_init(|| {
        include_str!("../../assets/terminal/themes.tsv")
            .lines()
            .filter(|l| !l.starts_with('#'))
            .filter_map(|line| {
                let f: Vec<&str> = line.split('\t').collect();
                let palette: Vec<u32> = f.get(7)?.split(',').filter_map(hex).collect();
                Some((
                    f[0].to_string(),
                    Colors {
                        palette: palette.try_into().ok()?,
                        background: hex(f[1])?,
                        foreground: hex(f[2])?,
                        cursor: hex(f[3]),
                        cursor_text: hex(f[4]),
                        selection: hex(f[5]),
                        selected_text: hex(f[6]),
                    },
                ))
            })
            .collect()
    })
}

/// The colors of the theme named so: one of yours, else one it comes with.
pub fn colors(name: &str, own: &[TerminalTheme]) -> Option<Colors> {
    own.iter().find(|t| t.name == name).and_then(Colors::of).or_else(|| bundled().iter().find(|(n, _)| n == name).map(|(_, c)| *c))
}

impl Colors {
    /// One of your themes' (None: not 16 colors, or not "#rrggbb").
    pub fn of(theme: &TerminalTheme) -> Option<Colors> {
        let palette: Vec<u32> = theme.palette.iter().filter_map(|c| hex(c)).collect();
        Some(Colors {
            palette: palette.try_into().ok()?,
            background: hex(&theme.background)?,
            foreground: hex(&theme.foreground)?,
            cursor: theme.cursor.as_deref().and_then(hex),
            cursor_text: theme.cursor_text.as_deref().and_then(hex),
            selection: theme.selection.as_deref().and_then(hex),
            selected_text: theme.selected_text.as_deref().and_then(hex),
        })
    }

    /// As one of your themes, named so.
    pub fn named(&self, name: &str) -> TerminalTheme {
        TerminalTheme {
            name: name.to_string(),
            background: to_hex(self.background),
            foreground: to_hex(self.foreground),
            palette: self.palette.iter().map(|c| to_hex(*c)).collect(),
            cursor: self.cursor.map(to_hex),
            cursor_text: self.cursor_text.map(to_hex),
            selection: self.selection.map(to_hex),
            selected_text: self.selected_text.map(to_hex),
        }
    }
}

/// The colors of the theme Settings have chosen (the one it comes with
/// first, if that's gone).
pub fn chosen(settings: &QuickTerminal) -> Colors {
    colors(&settings.look.theme, &settings.themes).or_else(|| colors("Carbonfox", &[])).unwrap_or(Colors {
        palette: [0x282828, 0xee5396, 0x25be6a, 0x08bdba, 0x78a9ff, 0xbe95ff, 0x33b1ff, 0xdfdfe0, 0x484848, 0xf16da6, 0x46c880, 0x2dc7c4, 0x8cb6ff, 0xc8a5ff, 0x52bdff, 0xe4e4e5],
        background: 0x161616,
        foreground: 0xf2f4f8,
        cursor: None,
        cursor_text: None,
        selection: None,
        selected_text: None,
    })
}

impl Theme {
    /// As Settings have it.
    pub fn of(settings: &QuickTerminal) -> Theme {
        let look = &settings.look;
        let colors = chosen(settings);
        let mut palette = [0; 256];
        palette[..16].copy_from_slice(&colors.palette);
        // (xterm's: a 6×6×6 cube, then 24 grays.)
        let level = |n: u32| if n == 0 { 0 } else { 55 + n * 40 };
        for i in 0..216u32 {
            palette[16 + i as usize] = level(i / 36) << 16 | level(i / 6 % 6) << 8 | level(i % 6);
        }
        for i in 0..24u32 {
            let g = 8 + i * 10;
            palette[232 + i as usize] = g << 16 | g << 8 | g;
        }
        Theme {
            palette,
            foreground: colors.foreground,
            background: colors.background,
            cursor: colors.cursor,
            cursor_text: colors.cursor_text,
            selection_background: colors.selection,
            selection_foreground: colors.selected_text,
            font_family: look.font_family.clone(),
            font_size: look.font_size.clamp(8., 32.),
            opacity: look.opacity.clamp(0.3, 1.),
            cursor_shape: look.cursor,
            blink: look.blink,
        }
    }
}

/// "#161616", "161616" or "0x161616".
pub fn hex(value: &str) -> Option<u32> {
    let value = value.trim().trim_matches(|c| c == '"' || c == '\'');
    let value = value.strip_prefix('#').or_else(|| value.strip_prefix("0x")).unwrap_or(value);
    (value.len() == 6).then(|| u32::from_str_radix(value, 16).ok()).flatten()
}

pub fn to_hex(color: u32) -> String {
    format!("#{color:06x}")
}

/// Two thirds as bright (dim text; the selection, where a theme has none).
pub fn dim(color: u32) -> u32 {
    let part = |shift: u32| ((color >> shift & 0xff) * 2 / 3) << shift;
    part(16) | part(8) | part(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comes_with() {
        let themes = bundled();
        assert!(themes.len() > 250, "{}", themes.len());
        let carbonfox = colors("Carbonfox", &[]).expect("Carbonfox");
        assert_eq!((carbonfox.background, carbonfox.palette[1]), (0x161616, 0xee5396));
        let mine = carbonfox.named("Mine");
        assert_eq!(Colors::of(&mine), Some(carbonfox), "yours, the same");
        assert_eq!(colors("Mine", &[mine.clone()]).map(|c| c.background), Some(0x161616));
        let t = Theme::of(&QuickTerminal::default());
        assert_eq!((t.background, t.palette[16 + 215]), (0x161616, 0xffffff));
        assert_eq!((hex("0xAABBCC"), hex("'#aabbcc'")), (Some(0xaabbcc), Some(0xaabbcc)));
    }
}
