//! A screenshot pinned over everything (a reference to work from, as
//! CleanShot's pins): at its own size, or smaller to fit, in the middle of
//! the screen the pointer's on. Drag it anywhere; double-click it to edit
//! it; × (over it) puts it away.

use std::path::PathBuf;

use gpui::prelude::FluentBuilder as _;
use gpui::*;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

use crate::macos;
use crate::style::{SEMIBOLD, symbol};

/// Room around it for its shadow.
const MARGIN: f32 = 16.;

pub fn open(path: PathBuf, cx: &mut App) {
    let Some(display) = crate::screens::with_pointer(cx) else { return };
    let visible = display.visible_bounds();
    let (w, h, scale) = macos::image_info(&path).unwrap_or((800, 600, 2.));
    // Its size in points, at most 60% of the screen's.
    let (w, h) = (w as f32 / scale.max(1.), h as f32 / scale.max(1.));
    let fit = (f32::from(visible.size.width) * 0.6 / w).min(f32::from(visible.size.height) * 0.6 / h).min(1.);
    let size = size(px(w * fit + 2. * MARGIN), px(h * fit + 2. * MARGIN));
    let bounds = Bounds { origin: visible.center() - point(size.width / 2., size.height / 2.), size };
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: None,
        kind: WindowKind::PopUp,
        focus: false,
        is_movable: true,
        is_resizable: false,
        is_minimizable: false,
        display_id: Some(display.id()),
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    };
    match gpui_kit::open_window(options, cx, |_, cx| cx.new(|_| Pinned { path, hovered: false, native: false })) {
        Ok((handle, _)) => {
            handle
                .update(cx, |_, window, cx| {
                    if let Some(Some(root)) = window.root::<gpui_kit::base::Root>() {
                        root.update(cx, |root, _| root.style().background = Some(transparent_black().into()));
                    }
                })
                .ok();
        }
        Err(e) => eprintln!("couldn't pin the screenshot: {e}"),
    }
}

struct Pinned {
    path: PathBuf,
    hovered: bool,
    native: bool,
}

impl Render for Pinned {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !std::mem::replace(&mut self.native, true)
            && let Ok(handle) = window.window_handle()
            && let RawWindowHandle::AppKit(appkit) = handle.as_raw()
        {
            let view = appkit.ns_view.as_ptr();
            cx.spawn(async move |_, _| macos::panel_bare(view)).detach();
        }
        div().size_full().p(px(MARGIN)).child(
            div()
                .id("pinned")
                .size_full()
                .relative()
                .rounded(px(8.))
                .overflow_hidden()
                .border_1()
                .border_color(white().opacity(0.2))
                .shadow(vec![BoxShadow { color: black().opacity(0.35), offset: point(px(0.), px(6.)), blur_radius: px(14.), spread_radius: px(0.), inset: false }])
                .child(img(self.path.clone()).size_full().object_fit(ObjectFit::Fill).rounded(px(7.)))
                .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                    this.hovered = *hovered;
                    cx.notify();
                }))
                .on_mouse_down(MouseButton::Left, |e, window, _| {
                    if e.click_count < 2 {
                        window.start_window_move();
                    }
                })
                .on_click(cx.listener(|this, e: &ClickEvent, window, cx| {
                    if e.click_count() == 2 {
                        let path = this.path.clone();
                        window.remove_window();
                        super::edit(path, cx);
                    }
                }))
                .child(
                    div()
                        .id("unpin")
                        .absolute()
                        .top(px(8.))
                        .left(px(8.))
                        .size(px(22.))
                        .rounded_full()
                        .bg(black().opacity(0.6))
                        .border_1()
                        .border_color(white().opacity(0.3))
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(!self.hovered, |d| d.invisible())
                        .child(symbol("xmark", 10., SEMIBOLD, white(), window))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(|_, window, _| window.remove_window()),
                ),
        )
    }
}
