//! Which screen something comes up on: the pointer's (the screenshots'
//! windows, Quick Open's by default), the front window's, or the main one.
//! macOS is asked which: GPUI's displays' bounds all start at 0, 0, so
//! where the pointer is can't tell them apart.

use std::rc::Rc;

use gpui::{App, DisplayId, PlatformDisplay};

use crate::settings::QuickScreen;

/// The screen the pointer's on (else the main one).
pub fn with_pointer(cx: &App) -> Option<Rc<dyn PlatformDisplay>> {
    by_id(crate::macos::screen_with_pointer(), cx).or_else(|| cx.primary_display())
}

/// The screen Quick Open comes up on, as Settings has it.
pub fn for_quick_open(which: QuickScreen, cx: &App) -> Option<Rc<dyn PlatformDisplay>> {
    match which {
        QuickScreen::Pointer => with_pointer(cx),
        // (No window up, as on the desktop: the pointer's.)
        QuickScreen::ActiveWindow => by_id(crate::macos::screen_with_front_window(), cx).or_else(|| with_pointer(cx)),
        QuickScreen::Main => cx.primary_display(),
    }
}

fn by_id(id: Option<u32>, cx: &App) -> Option<Rc<dyn PlatformDisplay>> {
    cx.find_display(DisplayId::new(id? as u64))
}
