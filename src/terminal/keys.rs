//! Keys as a terminal sends them to the program in it: those that aren't
//! text as xterm sends them (arrows, Return, ⌃C), and macOS's own on
//! top (⌥← and ⌥→ a word, ⌘← and ⌘→ the line's ends, ⌘⌫ the line). Text,
//! and what an input method makes, goes as itself, through GPUI's input.

use gpui::Modifiers;

/// What `key` (as GPUI names it) with `m` sends, if it isn't text. `app`:
/// the program asked for cursor keys' application mode (vim, less).
pub fn bytes(key: &str, m: &Modifiers, app: bool) -> Option<Vec<u8>> {
    // (xterm's modifier parameter: 1, and shift 1, alt 2, control 4.)
    let param = 1 + m.shift as u8 + 2 * m.alt as u8 + 4 * m.control as u8;
    let cursor = |end: char| -> Vec<u8> {
        match (param, app) {
            (1, true) => format!("\x1bO{end}"),
            (1, false) => format!("\x1b[{end}"),
            _ => format!("\x1b[1;{param}{end}"),
        }
        .into_bytes()
    };
    let tilde = |n: u8| -> Vec<u8> { if param > 1 { format!("\x1b[{n};{param}~") } else { format!("\x1b[{n}~") }.into_bytes() };
    let function = |end: char| -> Vec<u8> { if param > 1 { format!("\x1b[1;{param}{end}") } else { format!("\x1bO{end}") }.into_bytes() };
    let word = m.alt && !m.control && !m.shift && !m.platform;
    Some(match key {
        "left" if word => b"\x1bb".to_vec(),
        "right" if word => b"\x1bf".to_vec(),
        "left" if m.platform => b"\x01".to_vec(),
        "right" if m.platform => b"\x05".to_vec(),
        "up" => cursor('A'),
        "down" => cursor('B'),
        "right" => cursor('C'),
        "left" => cursor('D'),
        "home" => cursor('H'),
        "end" => cursor('F'),
        "pageup" => tilde(5),
        "pagedown" => tilde(6),
        "delete" => tilde(3),
        "insert" => tilde(2),
        "f1" => function('P'),
        "f2" => function('Q'),
        "f3" => function('R'),
        "f4" => function('S'),
        "f5" => tilde(15),
        "f6" => tilde(17),
        "f7" => tilde(18),
        "f8" => tilde(19),
        "f9" => tilde(20),
        "f10" => tilde(21),
        "f11" => tilde(23),
        "f12" => tilde(24),
        "enter" if m.alt => b"\x1b\r".to_vec(),
        "enter" => b"\r".to_vec(),
        "tab" if m.shift => b"\x1b[Z".to_vec(),
        "tab" => b"\t".to_vec(),
        "escape" => b"\x1b".to_vec(),
        "backspace" if m.platform => b"\x15".to_vec(),
        "backspace" if m.alt => b"\x17".to_vec(),
        "backspace" if m.control => b"\x08".to_vec(),
        "backspace" => b"\x7f".to_vec(),
        "space" if m.control => b"\x00".to_vec(),
        _ if m.control && !m.platform => {
            let byte = control(key)?;
            if m.alt { vec![0x1b, byte] } else { vec![byte] }
        }
        _ => return None,
    })
}

/// ⌃ and a key: its control character (⌃C is 3).
fn control(key: &str) -> Option<u8> {
    let mut chars = key.chars();
    let (Some(c), None) = (chars.next(), chars.next()) else { return None };
    Some(match c.to_ascii_lowercase() {
        c @ 'a'..='z' => c as u8 - b'a' + 1,
        '@' | '2' => 0,
        '[' | '3' => 0x1b,
        '\\' | '4' => 0x1c,
        ']' | '5' => 0x1d,
        '^' | '6' => 0x1e,
        '_' | '-' | '7' => 0x1f,
        '8' | '?' => 0x7f,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sends() {
        let none = Modifiers::default();
        let ctrl = Modifiers { control: true, ..Modifiers::default() };
        let alt = Modifiers { alt: true, ..Modifiers::default() };
        assert_eq!(bytes("c", &ctrl, false), Some(vec![3]));
        assert_eq!(bytes("up", &none, false), Some(b"\x1b[A".to_vec()));
        assert_eq!(bytes("up", &none, true), Some(b"\x1bOA".to_vec()), "vim's mode");
        assert_eq!(bytes("left", &alt, false), Some(b"\x1bb".to_vec()));
        assert_eq!(bytes("right", &ctrl, false), Some(b"\x1b[1;5C".to_vec()));
        assert_eq!(bytes("enter", &none, false), Some(b"\r".to_vec()));
        assert_eq!(bytes("a", &none, false), None, "text, through the input");
        assert_eq!(bytes("a", &alt, false), None, "⌥ types macOS's characters");
    }
}
