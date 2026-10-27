//! Colors: those picked off the screen (Pick Color, in `shots`) kept,
//! newest first, and any color written each way it's written (#hex, rgb(),
//! hsl(), oklch()…), to copy as wanted. Quick Open shows them on a page of
//! F2's own (`extension`), and a color typed in its search (`answer`).

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::extensions::{Extension, Row, Tell};

/// A color in sRGB, each part 0 to 1, and how opaque it is.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Color {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

/// Each way a color's written, by its id (in settings, `shots.color_format`)
/// and its name.
pub const FORMATS: [(&str, &str); 7] = [("hex", "HEX"), ("rgb", "RGB"), ("hsl", "HSL"), ("hsb", "HSB"), ("oklch", "OKLCH"), ("swiftui", "SwiftUI"), ("nscolor", "NSColor")];

impl Color {
    pub fn rgb8(&self) -> [u8; 3] {
        [self.r, self.g, self.b].map(|c| (c.clamp(0., 1.) * 255.).round() as u8)
    }

    /// As GPUI's colors are: 0xRRGGBB.
    pub fn u32(&self) -> u32 {
        let [r, g, b] = self.rgb8();
        (r as u32) << 16 | (g as u32) << 8 | b as u32
    }

    /// "#FF8800" (and its opacity after, if it has one: "#FF880080").
    pub fn hex(&self) -> String {
        let [r, g, b] = self.rgb8();
        let alpha = if self.a < 1. { format!("{:02X}", (self.a.clamp(0., 1.) * 255.).round() as u8) } else { String::new() };
        format!("#{r:02X}{g:02X}{b:02X}{alpha}")
    }

    /// Hue (degrees), saturation and lightness (0 to 1).
    fn hsl(&self) -> (f64, f64, f64) {
        let (max, min) = (self.r.max(self.g).max(self.b), self.r.min(self.g).min(self.b));
        let (l, d) = ((max + min) / 2., max - min);
        let s = if d == 0. { 0. } else { d / (1. - (2. * l - 1.).abs()) };
        (self.hue(max, d), s, l)
    }

    /// Hue, saturation and brightness.
    fn hsb(&self) -> (f64, f64, f64) {
        let max = self.r.max(self.g).max(self.b);
        let d = max - self.r.min(self.g).min(self.b);
        (self.hue(max, d), if max == 0. { 0. } else { d / max }, max)
    }

    fn hue(&self, max: f64, d: f64) -> f64 {
        if d == 0. {
            return 0.;
        }
        let h = if max == self.r {
            60. * ((self.g - self.b) / d).rem_euclid(6.)
        } else if max == self.g {
            60. * ((self.b - self.r) / d + 2.)
        } else {
            60. * ((self.r - self.g) / d + 4.)
        };
        h.rem_euclid(360.)
    }

    /// OKLab's lightness and a, b: how far apart colors look.
    fn oklab(&self) -> (f64, f64, f64) {
        let [r, g, b] = [self.r, self.g, self.b].map(linear);
        let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).cbrt();
        let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt();
        let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt();
        (
            0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
            1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
            0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s,
        )
    }

    fn from_oklab(l: f64, a: f64, b: f64, alpha: f64) -> Color {
        let l_ = (l + 0.3963377774 * a + 0.2158037573 * b).powi(3);
        let m_ = (l - 0.1055613458 * a - 0.0638541728 * b).powi(3);
        let s_ = (l - 0.0894841775 * a - 1.2914855480 * b).powi(3);
        let [r, g, b] = [
            4.0767416621 * l_ - 3.3077115913 * m_ + 0.2309699292 * s_,
            -1.2684380046 * l_ + 2.6097574011 * m_ - 0.3413193965 * s_,
            -0.0041960863 * l_ - 0.7034186147 * m_ + 1.7076147010 * s_,
        ]
        .map(|c| gamma(c).clamp(0., 1.));
        Color { r, g, b, a: alpha }
    }

    fn from_hsl(h: f64, s: f64, l: f64, a: f64) -> Color {
        let c = (1. - (2. * l - 1.).abs()) * s;
        Self::from_hue(h, c, l - c / 2., a)
    }

    fn from_hsb(h: f64, s: f64, v: f64, a: f64) -> Color {
        let c = v * s;
        Self::from_hue(h, c, v - c, a)
    }

    /// From a hue, its chroma, and what's added to each part.
    fn from_hue(h: f64, c: f64, m: f64, a: f64) -> Color {
        let h = h.rem_euclid(360.) / 60.;
        let x = c * (1. - (h.rem_euclid(2.) - 1.).abs());
        let (r, g, b) = match h as u32 {
            0 => (c, x, 0.),
            1 => (x, c, 0.),
            2 => (0., c, x),
            3 => (0., x, c),
            4 => (x, 0., c),
            _ => (c, 0., x),
        };
        Color { r: (r + m).clamp(0., 1.), g: (g + m).clamp(0., 1.), b: (b + m).clamp(0., 1.), a }
    }

    /// How light it is to the eye, as WCAG measures it (0 to 1).
    fn luminance(&self) -> f64 {
        let [r, g, b] = [self.r, self.g, self.b].map(linear);
        0.2126 * r + 0.7152 * g + 0.0722 * b
    }

    /// Its contrast with `other`, as WCAG has it: 1 to 21.
    pub fn contrast(&self, other: &Color) -> f64 {
        let (a, b) = (self.luminance(), other.luminance());
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    /// It written as `format` says (one of `FORMATS`' ids; else hex).
    pub fn written(&self, format: &str) -> String {
        let [r, g, b] = self.rgb8();
        let alpha = self.a < 1.;
        match format {
            "rgb" if alpha => format!("rgba({r}, {g}, {b}, {})", num(self.a, 2)),
            "rgb" => format!("rgb({r}, {g}, {b})"),
            "hsl" => {
                let (h, s, l) = self.hsl();
                let (h, s, l) = (h.round() as u32 % 360, (s * 100.).round(), (l * 100.).round());
                if alpha { format!("hsla({h}, {s}%, {l}%, {})", num(self.a, 2)) } else { format!("hsl({h}, {s}%, {l}%)") }
            }
            "hsb" => {
                let (h, s, v) = self.hsb();
                format!("hsb({}, {}%, {}%)", h.round() as u32 % 360, (s * 100.).round(), (v * 100.).round())
            }
            "oklch" => {
                let (l, a, b) = self.oklab();
                let c = a.hypot(b);
                // (A gray has no hue: 0.)
                let h = if c < 0.0005 { 0. } else { b.atan2(a).to_degrees().rem_euclid(360.) };
                let opacity = if alpha { format!(" / {}%", num(self.a * 100., 0)) } else { String::new() };
                format!("oklch({}% {} {}{opacity})", num(l * 100., 1), num(c, 3), num(h, 1))
            }
            "swiftui" => {
                let opacity = if alpha { format!(", opacity: {}", num(self.a, 3)) } else { String::new() };
                format!("Color(red: {}, green: {}, blue: {}{opacity})", num(self.r, 3), num(self.g, 3), num(self.b, 3))
            }
            "nscolor" => format!("NSColor(srgbRed: {}, green: {}, blue: {}, alpha: {})", num(self.r, 3), num(self.g, 3), num(self.b, 3), num(self.a, 3)),
            _ => self.hex(),
        }
    }

    /// The CSS color's name nearest it (by how they look), and whether
    /// it's that color.
    pub fn name(&self) -> (&'static str, bool) {
        static LABS: OnceLock<Vec<(&'static str, (f64, f64, f64))>> = OnceLock::new();
        let labs = LABS.get_or_init(|| NAMES.iter().map(|(name, hex)| (*name, from_u32(*hex).oklab())).collect());
        let (l, a, b) = self.oklab();
        let (name, far) = labs.iter().map(|(name, (l2, a2, b2))| (*name, (l - l2).powi(2) + (a - a2).powi(2) + (b - b2).powi(2))).fold(("Black", f64::MAX), |best, n| if n.1 < best.1 { n } else { best });
        (name, far < 1e-6)
    }

    /// Its name, as a row says it: "Tomato", or "Near Tomato".
    pub fn called(&self) -> String {
        match self.name() {
            (name, true) => name.to_string(),
            (name, false) => format!("Near {name}"),
        }
    }
}

/// sRGB's gamma taken off: light as it is.
fn linear(c: f64) -> f64 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn gamma(c: f64) -> f64 {
    if c <= 0.0031308 { 12.92 * c } else { 1.055 * c.max(0.).powf(1. / 2.4) - 0.055 }
}

fn from_u32(hex: u32) -> Color {
    let part = |shift: u32| ((hex >> shift) & 0xff) as f64 / 255.;
    Color { r: part(16), g: part(8), b: part(0), a: 1. }
}

/// `x` to `places` decimals, without the zeros at its end: "0.533", "1".
fn num(x: f64, places: usize) -> String {
    let s = format!("{x:.places$}");
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.') } else { &s };
    if s == "-0" { "0".into() } else { s.to_string() }
}

/// `text` as a color, if it's written as one: "#ff8800", "#f80", "rgb(255
/// 136 0)", "rgba(255, 136, 0, .5)", "hsl(32 100% 50%)", "hsb(32, 100%,
/// 100%)", "oklch(73% 0.18 54)", "oklab(…)", or a CSS color's name
/// ("tomato", "dark orange").
pub fn parse(text: &str) -> Option<Color> {
    let t = text.trim().to_lowercase();
    if let Some(hex) = t.strip_prefix('#').or_else(|| t.strip_prefix("0x")) {
        return from_hex(hex);
    }
    if let Some((name, rest)) = t.split_once('(') {
        let inner = rest.trim().strip_suffix(')')?;
        return function(name.trim(), inner);
    }
    let bare: String = t.chars().filter(|c| !c.is_whitespace()).collect();
    let bare = bare.replace("grey", "gray");
    NAMES.iter().find(|(name, _)| name.replace(' ', "").to_lowercase() == bare).map(|(_, hex)| from_u32(*hex))
}

fn from_hex(hex: &str) -> Option<Color> {
    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let digit = |i: usize| u8::from_str_radix(&hex[i..i + 1], 16).ok().map(|d| (d * 17) as f64 / 255.);
    let pair = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok().map(|d| d as f64 / 255.);
    match hex.len() {
        3 => Some(Color { r: digit(0)?, g: digit(1)?, b: digit(2)?, a: 1. }),
        4 => Some(Color { r: digit(0)?, g: digit(1)?, b: digit(2)?, a: digit(3)? }),
        6 => Some(Color { r: pair(0)?, g: pair(2)?, b: pair(4)?, a: 1. }),
        8 => Some(Color { r: pair(0)?, g: pair(2)?, b: pair(4)?, a: pair(6)? }),
        _ => None,
    }
}

/// A number as written in a color's function: "50%", "32deg", "0.5turn".
enum Part {
    Number(f64),
    Percent(f64),
}

fn function(name: &str, inner: &str) -> Option<Color> {
    let parts: Vec<Part> = inner
        .split(|c: char| c == ',' || c == '/' || c.is_whitespace())
        .filter(|p| !p.is_empty())
        .map(|p| {
            if let Some(n) = p.strip_suffix('%') {
                return n.parse().ok().map(Part::Percent);
            }
            let (n, scale) = match p {
                p if p.ends_with("deg") => (&p[..p.len() - 3], 1.),
                p if p.ends_with("turn") => (&p[..p.len() - 4], 360.),
                p if p.ends_with("rad") => (&p[..p.len() - 3], 180. / std::f64::consts::PI),
                p => (p, 1.),
            };
            n.parse::<f64>().ok().map(|n| Part::Number(n * scale))
        })
        .collect::<Option<_>>()?;
    if !(3..=4).contains(&parts.len()) {
        return None;
    }
    // (Each part as a share of `whole`: a percent of it, or a number of it.)
    let of = |p: &Part, whole: f64| match p {
        Part::Percent(n) => n / 100.,
        Part::Number(n) => n / whole,
    };
    let number = |p: &Part| match p {
        Part::Percent(n) | Part::Number(n) => *n,
    };
    let chroma = |p: &Part| match p {
        Part::Percent(n) => n / 100. * 0.4,
        Part::Number(n) => *n,
    };
    let alpha = parts.get(3).map_or(1., |p| of(p, 1.)).clamp(0., 1.);
    let unit = |p: &Part| of(p, 100.).clamp(0., 1.);
    let color = match name {
        "rgb" | "rgba" => Color { r: of(&parts[0], 255.).clamp(0., 1.), g: of(&parts[1], 255.).clamp(0., 1.), b: of(&parts[2], 255.).clamp(0., 1.), a: alpha },
        "hsl" | "hsla" => Color::from_hsl(number(&parts[0]), unit(&parts[1]), unit(&parts[2]), alpha),
        "hsb" | "hsv" | "hsba" => Color::from_hsb(number(&parts[0]), unit(&parts[1]), unit(&parts[2]), alpha),
        // (Their chroma's 100% is 0.4.)
        "oklch" => {
            let (c, h) = (chroma(&parts[1]), number(&parts[2]).to_radians());
            Color::from_oklab(of(&parts[0], 1.), c * h.cos(), c * h.sin(), alpha)
        }
        "oklab" => Color::from_oklab(of(&parts[0], 1.), chroma(&parts[1]), chroma(&parts[2]), alpha),
        _ => return None,
    };
    Some(color)
}

// Those picked, kept.

#[derive(Serialize, Deserialize, Clone)]
pub struct Kept {
    pub hex: String,
    /// When it was picked, in seconds since 1970.
    pub at: u64,
}

/// The most kept: the oldest go.
const MOST: usize = 200;

fn file() -> PathBuf {
    crate::settings::data_dir().join("Colors.json")
}

pub fn kept() -> Vec<Kept> {
    std::fs::read(file()).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
}

fn save(kept: &[Kept]) {
    if let Ok(bytes) = serde_json::to_vec_pretty(kept) {
        let _ = std::fs::write(file(), bytes);
    }
}

/// `hex` kept, first (picked again, it moves up).
pub fn keep(hex: &str) {
    let mut all = kept();
    all.retain(|k| !k.hex.eq_ignore_ascii_case(hex));
    let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    all.insert(0, Kept { hex: hex.to_uppercase(), at });
    all.truncate(MOST);
    save(&all);
}

/// A color just picked ("#RRGGBB"): kept, and what's copied for it (as
/// `format` says), and its color.
pub fn picked(hex: &str, format: &str) -> (String, Option<u32>) {
    keep(hex);
    match parse(hex) {
        Some(color) => (color.written(format), Some(color.u32())),
        None => (hex.to_string(), None),
    }
}

// Quick Open's page of them.

/// The page of F2's own that shows them (its commands: "colors", and its
/// rows' actions').
pub fn extension() -> Arc<Extension> {
    static IT: OnceLock<Arc<Extension>> = OnceLock::new();
    IT.get_or_init(|| {
        let manifest = json!({
            "api": 1,
            "id": "f2.colors",
            "name": "Colors",
            "icon": "eyedropper",
            "color": "#bf5af2",
            "commands": [
                { "id": "colors", "title": "Colors", "subtitle": "Picked lately" },
                { "id": "pick", "title": "Pick Color", "hidden": true },
                { "id": "keep", "title": "Keep Color", "hidden": true },
                { "id": "forget", "title": "Forget Color", "hidden": true },
                { "id": "forget-all", "title": "Forget All Colors", "hidden": true }
            ]
        });
        Arc::new(Extension { id: "f2.colors".into(), dir: PathBuf::new(), manifest: serde_json::from_value(manifest).expect("Colors' manifest"), native: Some(page) })
    })
    .clone()
}

/// What its commands say (see `extensions::Native`).
fn page(command: &str, args: &Value, _query: &str, tell: Tell, cx: &mut gpui::App) {
    let hex = args.get("hex").and_then(Value::as_str).unwrap_or_default().to_string();
    let said = match command {
        "colors" => {
            let format = crate::settings::Settings::get(cx).shots.color_format.clone();
            let mut rows = vec![json!({
                "title": "Pick a Color",
                "subtitle": "From anywhere on the screen",
                "icon": "eyedropper",
                "color": "#bf5af2",
                "keywords": ["pick", "new", "eyedropper", "sample"],
                "detail": { "markdown": "## Pick a Color\n\nReturn shows macOS's magnifier: click anywhere on the screen, and the color there is copied, and kept here.\n\nIt's copied as HEX, or as Settings › Screenshots says: RGB, HSL, HSB, OKLCH, SwiftUI or NSColor. Each color here can be copied every way.\n\nA color typed in Quick Open (#ff8800, rgb(…), hsl(…), oklch(…), or \"color tomato\") is shown every way it's written too." },
                "actions": [{ "title": "Pick a Color", "run": { "command": "pick" }, "icon": "eyedropper" }]
            })];
            rows.extend(kept().iter().filter_map(|k| Some(row(&parse(&k.hex)?, &format, Some(k.at)))));
            json!({ "items": rows, "placeholder": "Search colors you've picked…", "loading": false })
        }
        "pick" => {
            // (Once Quick Open's gone: not a color of its own picked.)
            cx.spawn(async |cx| {
                cx.background_executor().timer(std::time::Duration::from_millis(250)).await;
                cx.update(|cx| crate::shots::capture(crate::shots::Action::Color, cx));
            })
            .detach();
            json!({ "close": true })
        }
        "keep" => {
            keep(&hex);
            json!({ "notice": { "title": format!("Kept {hex}"), "text": "In Colors, in Quick Open." } })
        }
        "forget" => {
            save(&kept().into_iter().filter(|k| !k.hex.eq_ignore_ascii_case(&hex)).collect::<Vec<_>>());
            json!({ "refresh": true })
        }
        "forget-all" => {
            save(&[]);
            json!({ "refresh": true })
        }
        _ => json!({ "error": format!("Colors has no \"{command}\".") }),
    };
    tell.say(said);
}

/// A color's row: its swatch, its name, each way it's written to copy (as
/// `format` says first), and (one picked) when, and to forget it.
fn row(color: &Color, format: &str, picked: Option<u64>) -> Value {
    let hex = color.hex();
    let mut formats: Vec<(&str, &str)> = FORMATS.to_vec();
    formats.sort_by_key(|(id, _)| *id != format);
    let mut actions: Vec<Value> = formats.iter().map(|(id, name)| json!({ "title": format!("Copy {name}"), "copy": color.written(id), "icon": "doc.on.doc" })).collect();
    match picked {
        Some(_) => {
            actions.push(json!({ "title": "Forget This Color", "run": { "command": "forget", "args": { "hex": hex } }, "icon": "trash", "destructive": true }));
            actions.push(json!({ "title": "Forget All Colors", "run": { "command": "forget-all" }, "icon": "trash", "confirm": "Forget all the colors you've picked?", "destructive": true }));
        }
        None => actions.push(json!({ "title": "Keep in Colors", "run": { "command": "keep", "args": { "hex": hex } }, "icon": "plus.circle" })),
    }
    let white = Color { r: 1., g: 1., b: 1., a: 1. };
    let black = Color { r: 0., g: 0., b: 0., a: 1. };
    let graded = |ratio: f64| format!("{}:1{}", num(ratio, 1), if ratio >= 7. { " · AAA" } else if ratio >= 4.5 { " · AA" } else if ratio >= 3. { " · large text" } else { "" });
    let mut facts: Vec<Value> = FORMATS.iter().map(|(id, name)| json!({ "label": name, "text": color.written(id) })).collect();
    facts.push(json!({ "label": "On white", "text": graded(color.contrast(&white)) }));
    facts.push(json!({ "label": "On black", "text": graded(color.contrast(&black)) }));
    let name = color.called();
    json!({
        "title": hex,
        "subtitle": name,
        "icon": "drop.fill",
        "color": format!("#{:06X}", color.u32()),
        "section": picked.map(|_| "Picked Lately"),
        "accessory": picked.map(ago).unwrap_or_else(|| "Color".into()),
        "keywords": [name, color.written("rgb"), color.written("hsl")],
        // (Its "#" not markdown's heading.)
        "detail": { "color": format!("#{:06X}", color.u32()), "markdown": format!("## \\{hex}\n\n{name}"), "metadata": facts },
        "actions": actions,
    })
}

/// "3h ago".
fn ago(at: u64) -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let gone = now.saturating_sub(at);
    for (unit, size) in [("y", 31_536_000), ("mo", 2_592_000), ("d", 86_400), ("h", 3_600), ("m", 60)] {
        if gone >= size {
            return format!("{}{unit} ago", gone / size);
        }
    }
    "just now".into()
}

/// A color typed in Quick Open's search ("#ff8800", "rgb(…)", "color
/// tomato"): its row, to copy it as wanted, or keep it.
pub fn answer(q: &str, format: &str) -> Option<Row> {
    let q = q.trim();
    let named = q.strip_prefix("color ").or_else(|| q.strip_prefix("colour "));
    let color = match named {
        Some(name) => parse(name)?,
        None if q.starts_with('#') || q.contains('(') => parse(q)?,
        None => return None,
    };
    serde_json::from_value(row(&color, format, None)).ok()
}

/// The CSS colors, by name, as macOS's and the web's tools call them.
const NAMES: [(&str, u32); 141] = [
    ("Alice Blue", 0xF0F8FF),
    ("Antique White", 0xFAEBD7),
    ("Aqua", 0x00FFFF),
    ("Aquamarine", 0x7FFFD4),
    ("Azure", 0xF0FFFF),
    ("Beige", 0xF5F5DC),
    ("Bisque", 0xFFE4C4),
    ("Black", 0x000000),
    ("Blanched Almond", 0xFFEBCD),
    ("Blue", 0x0000FF),
    ("Blue Violet", 0x8A2BE2),
    ("Brown", 0xA52A2A),
    ("Burlywood", 0xDEB887),
    ("Cadet Blue", 0x5F9EA0),
    ("Chartreuse", 0x7FFF00),
    ("Chocolate", 0xD2691E),
    ("Coral", 0xFF7F50),
    ("Cornflower Blue", 0x6495ED),
    ("Cornsilk", 0xFFF8DC),
    ("Crimson", 0xDC143C),
    ("Cyan", 0x00FFFF),
    ("Dark Blue", 0x00008B),
    ("Dark Cyan", 0x008B8B),
    ("Dark Goldenrod", 0xB8860B),
    ("Dark Gray", 0xA9A9A9),
    ("Dark Green", 0x006400),
    ("Dark Khaki", 0xBDB76B),
    ("Dark Magenta", 0x8B008B),
    ("Dark Olive Green", 0x556B2F),
    ("Dark Orange", 0xFF8C00),
    ("Dark Orchid", 0x9932CC),
    ("Dark Red", 0x8B0000),
    ("Dark Salmon", 0xE9967A),
    ("Dark Sea Green", 0x8FBC8F),
    ("Dark Slate Blue", 0x483D8B),
    ("Dark Slate Gray", 0x2F4F4F),
    ("Dark Turquoise", 0x00CED1),
    ("Dark Violet", 0x9400D3),
    ("Deep Pink", 0xFF1493),
    ("Deep Sky Blue", 0x00BFFF),
    ("Dim Gray", 0x696969),
    ("Dodger Blue", 0x1E90FF),
    ("Firebrick", 0xB22222),
    ("Floral White", 0xFFFAF0),
    ("Forest Green", 0x228B22),
    ("Fuchsia", 0xFF00FF),
    ("Gainsboro", 0xDCDCDC),
    ("Ghost White", 0xF8F8FF),
    ("Gold", 0xFFD700),
    ("Goldenrod", 0xDAA520),
    ("Gray", 0x808080),
    ("Green", 0x008000),
    ("Green Yellow", 0xADFF2F),
    ("Honeydew", 0xF0FFF0),
    ("Hot Pink", 0xFF69B4),
    ("Indian Red", 0xCD5C5C),
    ("Indigo", 0x4B0082),
    ("Ivory", 0xFFFFF0),
    ("Khaki", 0xF0E68C),
    ("Lavender", 0xE6E6FA),
    ("Lavender Blush", 0xFFF0F5),
    ("Lawn Green", 0x7CFC00),
    ("Lemon Chiffon", 0xFFFACD),
    ("Light Blue", 0xADD8E6),
    ("Light Coral", 0xF08080),
    ("Light Cyan", 0xE0FFFF),
    ("Light Goldenrod Yellow", 0xFAFAD2),
    ("Light Gray", 0xD3D3D3),
    ("Light Green", 0x90EE90),
    ("Light Pink", 0xFFB6C1),
    ("Light Salmon", 0xFFA07A),
    ("Light Sea Green", 0x20B2AA),
    ("Light Sky Blue", 0x87CEFA),
    ("Light Slate Gray", 0x778899),
    ("Light Steel Blue", 0xB0C4DE),
    ("Light Yellow", 0xFFFFE0),
    ("Lime", 0x00FF00),
    ("Lime Green", 0x32CD32),
    ("Linen", 0xFAF0E6),
    ("Magenta", 0xFF00FF),
    ("Maroon", 0x800000),
    ("Medium Aquamarine", 0x66CDAA),
    ("Medium Blue", 0x0000CD),
    ("Medium Orchid", 0xBA55D3),
    ("Medium Purple", 0x9370DB),
    ("Medium Sea Green", 0x3CB371),
    ("Medium Slate Blue", 0x7B68EE),
    ("Medium Spring Green", 0x00FA9A),
    ("Medium Turquoise", 0x48D1CC),
    ("Medium Violet Red", 0xC71585),
    ("Midnight Blue", 0x191970),
    ("Mint Cream", 0xF5FFFA),
    ("Misty Rose", 0xFFE4E1),
    ("Moccasin", 0xFFE4B5),
    ("Navajo White", 0xFFDEAD),
    ("Navy", 0x000080),
    ("Old Lace", 0xFDF5E6),
    ("Olive", 0x808000),
    ("Olive Drab", 0x6B8E23),
    ("Orange", 0xFFA500),
    ("Orange Red", 0xFF4500),
    ("Orchid", 0xDA70D6),
    ("Pale Goldenrod", 0xEEE8AA),
    ("Pale Green", 0x98FB98),
    ("Pale Turquoise", 0xAFEEEE),
    ("Pale Violet Red", 0xDB7093),
    ("Papaya Whip", 0xFFEFD5),
    ("Peach Puff", 0xFFDAB9),
    ("Peru", 0xCD853F),
    ("Pink", 0xFFC0CB),
    ("Plum", 0xDDA0DD),
    ("Powder Blue", 0xB0E0E6),
    ("Purple", 0x800080),
    ("Rebecca Purple", 0x663399),
    ("Red", 0xFF0000),
    ("Rosy Brown", 0xBC8F8F),
    ("Royal Blue", 0x4169E1),
    ("Saddle Brown", 0x8B4513),
    ("Salmon", 0xFA8072),
    ("Sandy Brown", 0xF4A460),
    ("Sea Green", 0x2E8B57),
    ("Seashell", 0xFFF5EE),
    ("Sienna", 0xA0522D),
    ("Silver", 0xC0C0C0),
    ("Sky Blue", 0x87CEEB),
    ("Slate Blue", 0x6A5ACD),
    ("Slate Gray", 0x708090),
    ("Snow", 0xFFFAFA),
    ("Spring Green", 0x00FF7F),
    ("Steel Blue", 0x4682B4),
    ("Tan", 0xD2B48C),
    ("Teal", 0x008080),
    ("Thistle", 0xD8BFD8),
    ("Tomato", 0xFF6347),
    ("Turquoise", 0x40E0D0),
    ("Violet", 0xEE82EE),
    ("Wheat", 0xF5DEB3),
    ("White", 0xFFFFFF),
    ("White Smoke", 0xF5F5F5),
    ("Yellow", 0xFFFF00),
    ("Yellow Green", 0x9ACD32),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_read_every_way() {
        let orange = |c: Option<Color>| c.map(|c| c.hex());
        assert_eq!(orange(parse("#ff8800")), Some("#FF8800".into()));
        assert_eq!(orange(parse("#F80")), Some("#FF8800".into()));
        assert_eq!(orange(parse("0xFF8800")), Some("#FF8800".into()));
        assert_eq!(orange(parse("rgb(255, 136, 0)")), Some("#FF8800".into()));
        assert_eq!(orange(parse("rgb(255 136 0)")), Some("#FF8800".into()));
        assert_eq!(orange(parse("rgb(100% 53.33% 0%)")), Some("#FF8800".into()));
        assert_eq!(orange(parse("hsl(32, 100%, 50%)")), Some("#FF8800".into()));
        assert_eq!(orange(parse("hsl(32deg 100% 50%)")), Some("#FF8800".into()));
        assert_eq!(orange(parse("hsb(32, 100%, 100%)")), Some("#FF8800".into()));
        assert_eq!(orange(parse("rgba(255, 136, 0, 0.5)")), Some("#FF880080".into()));
        assert_eq!(orange(parse("rgb(255 136 0 / 50%)")), Some("#FF880080".into()));
        assert_eq!(orange(parse("tomato")), Some("#FF6347".into()));
        assert_eq!(orange(parse("Dark Slate Grey")), Some("#2F4F4F".into()));
        // (Tailwind's sky-500, as its theme writes it, and its hex.)
        assert_eq!(orange(parse("oklch(68.5% 0.169 237.323)")), Some("#00A6F4".into()));
        for bad in ["#ff888", "#gg8800", "rgb(1, 2)", "rgb(a, b, c)", "nothing", "hsl()", "#"] {
            assert_eq!(parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn colors_written_every_way() {
        let c = parse("#FF8800").unwrap();
        assert_eq!(c.written("hex"), "#FF8800");
        assert_eq!(c.written("rgb"), "rgb(255, 136, 0)");
        assert_eq!(c.written("hsl"), "hsl(32, 100%, 50%)");
        assert_eq!(c.written("hsb"), "hsb(32, 100%, 100%)");
        assert_eq!(c.written("swiftui"), "Color(red: 1, green: 0.533, blue: 0)");
        assert_eq!(c.written("nscolor"), "NSColor(srgbRed: 1, green: 0.533, blue: 0, alpha: 1)");
        assert_eq!(parse("#000").unwrap().written("oklch"), "oklch(0% 0 0)");
        assert_eq!(parse("#fff").unwrap().written("oklch"), "oklch(100% 0 0)");
        // (Each way, read back, the same color.)
        for hex in ["#FF8800", "#00A6F4", "#2F4F4F", "#7B68EE", "#123456", "#FEFEFE"] {
            let c = parse(hex).unwrap();
            for (id, _) in FORMATS.iter().filter(|(id, _)| !["swiftui", "nscolor"].contains(id)) {
                let back = parse(&c.written(id)).unwrap();
                let close = c.rgb8().iter().zip(back.rgb8()).all(|(a, b)| a.abs_diff(b) <= 2);
                assert!(close, "{hex} as {id}: {} is {}", c.written(id), back.hex());
            }
        }
        assert_eq!(parse("#808080").unwrap().written("hsl"), "hsl(0, 0%, 50%)");
        assert_eq!(parse("rgba(0, 0, 0, 0.25)").unwrap().written("oklch"), "oklch(0% 0 0 / 25%)");
    }

    #[test]
    fn colors_named_and_measured() {
        assert_eq!(parse("#FF6347").unwrap().called(), "Tomato");
        assert_eq!(parse("#FF8800").unwrap().called(), "Near Dark Orange");
        let (white, black) = (parse("#fff").unwrap(), parse("#000").unwrap());
        assert_eq!(num(white.contrast(&black), 1), "21");
        assert_eq!(num(parse("#777").unwrap().contrast(&white), 2), "4.48");
        assert_eq!(NAMES.iter().map(|(n, _)| n.replace(' ', "").to_lowercase()).collect::<std::collections::HashSet<_>>().len(), NAMES.len());
    }

    #[test]
    fn colors_typed_as_answers() {
        assert!(answer("#ff8800", "hex").is_some_and(|r| r.title == "#FF8800"));
        assert!(answer("color tomato", "hex").is_some());
        assert!(answer("rgb(1, 2, 3)", "hex").is_some());
        // (Not a word that's a color's name, alone: that's a search.)
        assert!(answer("tomato", "hex").is_none());
        assert!(answer("orange juice", "hex").is_none());
        assert!(answer("#hashtag", "hex").is_none());
        let row = answer("#ff8800", "rgb").unwrap();
        assert_eq!(row.actions[0].copy.as_deref(), Some("rgb(255, 136, 0)"));
    }
}
