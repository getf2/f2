//! Box drawing and blocks (U+2500 to U+259F), drawn as shapes, not a
//! font's glyphs, as Ghostty does: so lines meet from cell to cell, and
//! a box (Claude Code's, htop's, tmux's) is whole whatever the font or
//! the line height. Light, heavy and double lines, dashes, round corners,
//! diagonals, eighths, quadrants and shades.

use gpui::{Bounds, Hsla, PathBuilder, Pixels, point, px, size};

/// A shape to fill: a rectangle, or a stroked path.
pub enum Shape {
    Rect(Bounds<Pixels>, Hsla),
    Path(gpui::Path<Pixels>, Hsla),
}

/// Lines from a cell's middle, for U+2500 to U+257F, four a character:
/// up, right, down, left; 0 none, 1 light, 2 heavy, 3 double. "." for
/// dashes, round corners and diagonals, drawn otherwise.
const LINES: &str = concat!(
    "0101020210102020", "................................", //
    "0110021001200220", "0011001200210022", "1100120021002200", "1001100220012002", //
    "1110121021101120", "2120221012202220", "1011101220111021", "2021201210222022", //
    "0111011202110212", "0121012202210222", "1101110212011202", "2101210222012202", //
    "1111111212111212", "2111112121212112", "2211112212212212", "1222212222212222", //
    "................", //
    "0303303003100130", "0330001300310033", "1300310033001003", "3001300313103130", //
    "3330101330313033", "0313013103331303", "3101330313133131", "3333", //
    "................", "............", //
    "0001100001000010", "0002200002000020", "0201102001022010",
);

/// How `c` is drawn in the cell at `cell`, in `color`, if F2 draws it
/// itself.
pub fn draw(c: char, cell: Bounds<Pixels>, color: Hsla) -> Option<Vec<Shape>> {
    let code = c as u32;
    if !(0x2500..=0x259f).contains(&code) {
        return None;
    }
    let (x, y) = (f32::from(cell.origin.x), f32::from(cell.origin.y));
    let (w, h) = (f32::from(cell.size.width), f32::from(cell.size.height));
    let (cx, cy) = (x + w / 2., y + h / 2.);
    let light = (w / 7.).round().max(1.);
    let heavy = light * 2.;
    let rect = |x: f32, y: f32, w: f32, h: f32| Shape::Rect(Bounds::new(point(px(x), px(y)), size(px(w.max(0.)), px(h.max(0.)))), color);
    let mut out = Vec::new();
    if code >= 0x2580 {
        // Blocks: eighths, halves, quadrants, shades.
        let (eighth_w, eighth_h) = (w / 8., h / 8.);
        match code {
            0x2580 => out.push(rect(x, y, w, h / 2.)),
            0x2581..=0x2588 => {
                let n = (code - 0x2580) as f32;
                out.push(rect(x, y + h - eighth_h * n, w, eighth_h * n));
            }
            0x2589..=0x258f => {
                let n = (0x2590 - code) as f32;
                out.push(rect(x, y, eighth_w * n, h));
            }
            0x2590 => out.push(rect(cx, y, w / 2., h)),
            0x2591..=0x2593 => {
                let alpha = (code - 0x2590) as f32 / 4.;
                return Some(vec![Shape::Rect(cell, color.opacity(alpha))]);
            }
            0x2594 => out.push(rect(x, y, w, eighth_h)),
            0x2595 => out.push(rect(x + w - eighth_w, y, eighth_w, h)),
            _ => {
                // (Quadrants: upper left 1, upper right 2, lower left 4,
                // lower right 8.)
                const QUADRANTS: [u8; 10] = [4, 8, 1, 13, 9, 7, 11, 2, 6, 14];
                let bits = QUADRANTS[(code - 0x2596) as usize];
                for (bit, qx, qy) in [(1, x, y), (2, cx, y), (4, x, cy), (8, cx, cy)] {
                    if bits & bit != 0 {
                        out.push(rect(qx, qy, w / 2., h / 2.));
                    }
                }
            }
        }
        return Some(out);
    }
    let index = (code - 0x2500) as usize * 4;
    let lines = LINES.get(index..index + 4).unwrap_or("....");
    if lines != "...." {
        let weights: Vec<u8> = lines.bytes().map(|b| b - b'0').collect();
        for (side, &weight) in weights.iter().enumerate() {
            let t = if weight == 2 { heavy } else { light };
            // (Double: two lines, a light line's width either side of the
            // middle.)
            let offsets: &[f32] = match weight {
                0 => &[],
                3 => &[-light, light],
                _ => &[0.],
            };
            for &o in offsets {
                let reach = if weight == 3 { light * 1.5 } else { t / 2. };
                match side {
                    0 => out.push(rect(cx + o - t / 2., y, t, cy - y + reach)),
                    1 => out.push(rect(cx - reach, cy + o - t / 2., x + w - cx + reach, t)),
                    2 => out.push(rect(cx + o - t / 2., cy - reach, t, y + h - cy + reach)),
                    _ => out.push(rect(x, cy + o - t / 2., cx - x + reach, t)),
                }
            }
        }
        return Some(out);
    }
    match code {
        // Dashes: (how many, heavy, vertical).
        0x2504..=0x250b | 0x254c..=0x254f => {
            let (n, heavy_line, vertical) = match code {
                0x2504..=0x250b => (if code < 0x2508 { 3 } else { 4 }, code % 2 == 1, (code - 0x2504) % 4 >= 2),
                _ => (2, code % 2 == 1, code >= 0x254e),
            };
            let t = if heavy_line { heavy } else { light };
            let length = if vertical { h } else { w } / n as f32;
            for i in 0..n {
                let start = length * i as f32 + length * 0.2;
                if vertical {
                    out.push(rect(cx - t / 2., y + start, t, length * 0.6));
                } else {
                    out.push(rect(x + start, cy - t / 2., length * 0.6, t));
                }
            }
        }
        // Round corners: a curve between two edges' middles.
        0x256d..=0x2570 => {
            let (from, to) = match code {
                0x256d => (point(px(x + w), px(cy)), point(px(cx), px(y + h))),
                0x256e => (point(px(x), px(cy)), point(px(cx), px(y + h))),
                0x256f => (point(px(x), px(cy)), point(px(cx), px(y))),
                _ => (point(px(x + w), px(cy)), point(px(cx), px(y))),
            };
            let mut path = PathBuilder::stroke(px(light));
            path.move_to(from);
            path.curve_to(to, point(px(cx), px(cy)));
            out.extend(path.build().ok().map(|p| Shape::Path(p, color)));
        }
        // Diagonals.
        _ => {
            let mut path = PathBuilder::stroke(px(light));
            if code != 0x2572 {
                path.move_to(point(px(x), px(y + h)));
                path.line_to(point(px(x + w), px(y)));
            }
            if code != 0x2571 {
                path.move_to(point(px(x), px(y)));
                path.line_to(point(px(x + w), px(y + h)));
            }
            out.extend(path.build().ok().map(|p| Shape::Path(p, color)));
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table() {
        assert_eq!(LINES.len(), 128 * 4, "one entry each, U+2500 to U+257F");
        let at = |c: char| &LINES[(c as usize - 0x2500) * 4..][..4];
        assert_eq!((at('─'), at('│'), at('┌'), at('┘'), at('┼')), ("0101", "1010", "0110", "1001", "1111"));
        assert_eq!((at('┿'), at('╋'), at('╔'), at('╬'), at('╿')), ("1212", "2222", "0330", "3333", "2010"));
        assert_eq!((at('┄'), at('╭'), at('╳')), ("....", "....", "...."));
        let cell = Bounds::new(point(px(0.), px(0.)), size(px(8.), px(16.)));
        assert!(draw('a', cell, gpui::black()).is_none());
        assert_eq!(draw('█', cell, gpui::black()).map(|s| s.len()), Some(1));
        assert_eq!(draw('▚', cell, gpui::black()).map(|s| s.len()), Some(2));
    }
}
