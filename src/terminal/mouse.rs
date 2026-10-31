//! The mouse, told to a program that asked for it (vim, htop, tmux,
//! lazygit), as xterm tells it: presses and releases (mode 1000), drags
//! too (1002), or every move (1003); in SGR's form (1006), else xterm's
//! own (bytes, or UTF-8 past column 95: 1005).

use alacritty_terminal::term::TermMode;
use gpui::Modifiers;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Button {
    Left,
    Middle,
    Right,
    /// A move with no button down (mode 1003).
    None,
    WheelUp,
    WheelDown,
}

impl Button {
    pub fn of(button: gpui::MouseButton) -> Option<Button> {
        match button {
            gpui::MouseButton::Left => Some(Button::Left),
            gpui::MouseButton::Middle => Some(Button::Middle),
            gpui::MouseButton::Right => Some(Button::Right),
            _ => None,
        }
    }
}

/// Whether a program has asked for the mouse.
pub fn wanted(mode: TermMode) -> bool {
    mode.intersects(TermMode::MOUSE_MODE)
}

/// What says `button` was pressed (or let go, or moved with it down: `moved`)
/// at the cell in `col` and `row` (from 0), as `mode` asks; None if it
/// can't be said (xterm's own form stops at column 223).
pub fn report(button: Button, pressed: bool, moved: bool, m: &Modifiers, col: usize, row: usize, mode: TermMode) -> Option<Vec<u8>> {
    let base = match button {
        Button::Left => 0,
        Button::Middle => 1,
        Button::Right => 2,
        Button::None => 3,
        Button::WheelUp => 64,
        Button::WheelDown => 65,
    };
    let wheel = matches!(button, Button::WheelUp | Button::WheelDown);
    let mods = 4 * m.shift as u32 + 8 * m.alt as u32 + 16 * m.control as u32 + if moved { 32 } else { 0 };
    let (x, y) = (col as u32 + 1, row as u32 + 1);
    if mode.contains(TermMode::SGR_MOUSE) {
        let end = if pressed || wheel || moved { 'M' } else { 'm' };
        return Some(format!("\x1b[<{};{x};{y}{end}", base + mods).into_bytes());
    }
    // (xterm's own: a release doesn't say which button.)
    let code = if pressed || wheel || moved { base } else { 3 } + mods;
    let mut out = b"\x1b[M".to_vec();
    out.push(32 + code as u8);
    for n in [x, y] {
        let n = 32 + n;
        if mode.contains(TermMode::UTF8_MOUSE) {
            let c = char::from_u32(n).filter(|_| n < 2048)?;
            let mut buffer = [0; 4];
            out.extend_from_slice(c.encode_utf8(&mut buffer).as_bytes());
        } else {
            out.push(u8::try_from(n).ok()?);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_xterm_says() {
        let none = Modifiers::default();
        let sgr = TermMode::MOUSE_REPORT_CLICK | TermMode::SGR_MOUSE;
        assert_eq!(report(Button::Left, true, false, &none, 0, 0, sgr).unwrap(), b"\x1b[<0;1;1M");
        assert_eq!(report(Button::Left, false, false, &none, 4, 2, sgr).unwrap(), b"\x1b[<0;5;3m");
        assert_eq!(report(Button::Left, true, true, &none, 4, 2, sgr).unwrap(), b"\x1b[<32;5;3M", "dragged");
        let ctrl = Modifiers { control: true, ..Modifiers::default() };
        assert_eq!(report(Button::WheelDown, true, false, &ctrl, 9, 9, sgr).unwrap(), b"\x1b[<81;10;10M");
        let plain = TermMode::MOUSE_REPORT_CLICK;
        assert_eq!(report(Button::Right, true, false, &none, 0, 0, plain).unwrap(), b"\x1b[M\x22\x21\x21");
        assert_eq!(report(Button::Right, false, false, &none, 0, 0, plain).unwrap(), b"\x1b[M\x23\x21\x21", "released: 3");
        assert_eq!(report(Button::Left, true, false, &none, 300, 0, plain), None, "past xterm's columns");
        let utf8 = plain | TermMode::UTF8_MOUSE;
        assert_eq!(report(Button::Left, true, false, &none, 300, 0, utf8).unwrap(), "\x1b[M \u{14d}!".as_bytes());
    }
}
