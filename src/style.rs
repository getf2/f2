//! Finder's look: its colors in light and dark (the selection follows the
//! accent color), SF Symbols, and the glass capsules of the toolbar.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use gpui::*;

use crate::macos;

/// Geist (Vercel's, under the SIL Open Font License: see Help >
/// Acknowledgements), the website's type: the welcome's, and one of Quick
/// Open's fonts.
pub const GEIST: &str = "Geist";

pub fn load_fonts(cx: &mut App) {
    let fonts: [&'static [u8]; 4] = [
        include_bytes!("../assets/fonts/Geist-Regular.otf"),
        include_bytes!("../assets/fonts/Geist-Medium.otf"),
        include_bytes!("../assets/fonts/Geist-SemiBold.otf"),
        include_bytes!("../assets/fonts/Geist-Bold.otf"),
    ];
    if let Err(e) = cx.text_system().add_fonts(fonts.into_iter().map(std::borrow::Cow::Borrowed).collect()) {
        eprintln!("couldn't load Geist: {e}");
    }
}

pub const REGULAR: f32 = 0.;
pub const MEDIUM: f32 = 0.23;
pub const SEMIBOLD: f32 = 0.3;

#[derive(Clone, Copy)]
pub struct Palette {
    /// The window and the file area.
    pub background: Hsla,
    pub sidebar: Hsla,
    pub sidebar_border: Hsla,
    /// Every other row.
    pub stripe: Hsla,
    pub selection: Hsla,
    /// The selection while the files don't have focus.
    pub selection_inactive: Hsla,
    pub selected_text: Hsla,
    pub label: Hsla,
    pub secondary: Hsla,
    pub tertiary: Hsla,
    pub separator: Hsla,
    pub accent: Hsla,
    /// Toolbar capsules, their outline, and the chosen segment in one (or
    /// the current place in the sidebar).
    pub glass: Hsla,
    pub glass_border: Hsla,
    pub glass_selected: Hsla,
    pub shadow: Hsla,
}

impl Palette {
    pub fn for_window(window: &Window) -> Palette {
        Palette::new(matches!(window.appearance(), WindowAppearance::Dark | WindowAppearance::VibrantDark))
    }

    pub fn new(dark: bool) -> Palette {
        let [selection, selection_inactive, label, secondary, tertiary, _quaternary, separator, stripe, accent] =
            macos::system_colors(dark).map(Hsla::from);
        // Finder's glass windows on macOS 26 tint these with the desktop
        // behind them; these are close on a neutral one (dark: measured
        // from Finder's).
        let base = if dark {
            Palette {
                background: rgb(0x242527).into(),
                sidebar: rgb(0x1e1f21).into(),
                sidebar_border: white().opacity(0.12),
                glass: white().opacity(0.06),
                glass_border: white().opacity(0.1),
                glass_selected: white().opacity(0.13),
                shadow: black().opacity(0.3),
                ..Palette::BLANK
            }
        } else {
            Palette {
                background: white(),
                sidebar: rgb(0xf3f3f5).into(),
                sidebar_border: black().opacity(0.07),
                glass: white(),
                glass_border: black().opacity(0.09),
                glass_selected: black().opacity(0.07),
                shadow: black().opacity(0.08),
                ..Palette::BLANK
            }
        };
        Palette {
            selection,
            selection_inactive,
            selected_text: white(),
            label,
            secondary,
            tertiary,
            separator,
            stripe,
            accent,
            ..base
        }
    }

    const BLANK: Palette = Palette {
        background: Hsla::transparent_black(),
        sidebar: Hsla::transparent_black(),
        sidebar_border: Hsla::transparent_black(),
        stripe: Hsla::transparent_black(),
        selection: Hsla::transparent_black(),
        selection_inactive: Hsla::transparent_black(),
        selected_text: Hsla::transparent_black(),
        label: Hsla::transparent_black(),
        secondary: Hsla::transparent_black(),
        tertiary: Hsla::transparent_black(),
        separator: Hsla::transparent_black(),
        accent: Hsla::transparent_black(),
        glass: Hsla::transparent_black(),
        glass_border: Hsla::transparent_black(),
        glass_selected: Hsla::transparent_black(),
        shadow: Hsla::transparent_black(),
    };
}

/// Finder's tag colors by number (see `macos::tags`): gray, green, purple,
/// blue, yellow, red, orange.
pub fn tag_color(n: u8) -> Option<Hsla> {
    tag_rgb(n).map(|hex| rgb(hex).into())
}

/// A tag color's 0xRRGGBB (Finder's numbers: 1 gray to 7 orange).
pub fn tag_rgb(n: u8) -> Option<u32> {
    Some(match n {
        1 => 0x8e8e93,
        2 => 0x34c759,
        3 => 0xaf52de,
        4 => 0x0a84ff,
        5 => 0xffcc00,
        6 => 0xff3b30,
        7 => 0xff9500,
        _ => return None,
    })
}

type SymbolKey = (&'static str, u32, u32, u32, [u32; 4]);

thread_local! {
    /// Drawn once per name, size, weight, color and scale (a few dozen).
    static SYMBOLS: RefCell<HashMap<SymbolKey, Option<(Arc<RenderImage>, Size<f32>)>>> = RefCell::default();
}

/// An SF Symbol, `pt` points tall, in `color`.
pub fn symbol(name: &'static str, pt: f32, weight: f32, color: Hsla, window: &Window) -> AnyElement {
    let scale = window.scale_factor();
    let quantize = |v: f32| (v * 1000.) as u32;
    let key = (name, quantize(pt), quantize(weight), quantize(scale), [color.h, color.s, color.l, color.a].map(quantize));
    let drawn = SYMBOLS.with_borrow_mut(|symbols| {
        symbols
            .entry(key)
            .or_insert_with(|| macos::symbol(name, pt, weight, color, scale).map(|(image, size)| (Arc::new(image), size)))
            .clone()
    });
    match drawn {
        Some((image, size)) => img(image).w(px(size.width)).h(px(size.height)).flex_shrink_0().into_any_element(),
        None => div().size(px(pt)).flex_shrink_0().into_any_element(),
    }
}

/// A toolbar capsule: Finder's raised glass, `height` tall.
pub fn glass(height: f32, palette: &Palette) -> Div {
    div()
        .h(px(height))
        .flex()
        .items_center()
        .flex_shrink_0()
        .rounded_full()
        .bg(palette.glass)
        .border_1()
        .border_color(palette.glass_border)
        .shadow(vec![BoxShadow {
            color: palette.shadow,
            offset: point(px(0.), px(1.)),
            blur_radius: px(3.),
            spread_radius: px(0.),
            inset: false,
        }])
}
