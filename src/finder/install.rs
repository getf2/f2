//! A disk image's app, installed from its window: a bar (under a DMG's
//! picture, or over its volume in an ordinary window) offers to copy the
//! app alone in it to Applications, as dragging it onto the arrow would;
//! then to open it, and to eject the image, moving it to the Trash too.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use gpui::prelude::FluentBuilder as _;
use gpui::*;

use super::Finder;
use super::items::file_image;
use crate::listing::Entry;
use crate::style::glass;

/// The disk image the volume shown is of, with an app alone in it.
pub(super) struct Image {
    file: PathBuf,
    volume: PathBuf,
    app: Entry,
    state: State,
}

enum State {
    Offered,
    /// Copying since then (or asking about the one already there).
    Copying(SystemTime),
    /// Copied: where it is now.
    Installed(PathBuf),
}

/// Where apps are installed. (A test copy, NEW_FINDER_SETTINGS set, can
/// install somewhere of its own with F2_APPLICATIONS: never in yours.)
fn applications() -> PathBuf {
    match std::env::var_os("F2_APPLICATIONS").filter(|_| std::env::var_os("NEW_FINDER_SETTINGS").is_some()) {
        Some(dir) => PathBuf::from(dir),
        None => PathBuf::from("/Applications"),
    }
}

/// Where an app's copy goes.
fn installed_at(app: &Path) -> PathBuf {
    applications().join(app.file_name().unwrap_or_default())
}

/// Eject `volume` (its window gone already), and move `file` (its image)
/// to the Trash after; what goes wrong, said in a note.
pub(super) fn eject_away(volume: PathBuf, file: Option<PathBuf>, cx: &mut App) {
    let name = super::display_name(&volume);
    cx.spawn(async move |cx| {
        let done = cx
            .background_executor()
            .spawn(async move {
                crate::macos::eject(&volume).map_err(|e| format!("“{name}” couldn't be ejected: {e}"))?;
                if let Some(file) = file {
                    crate::macos::trash(&file).map_err(|e| format!("It was ejected, but couldn't be moved to the {}: {e}", crate::macos::bin()))?;
                }
                Ok::<(), String>(())
            })
            .await;
        if let Err(why) = done {
            cx.update(|cx| crate::shots::note("Disk Image".into(), why, None, cx));
        }
    })
    .detach();
}

impl Finder {
    /// Whether the folder shown is a disk image's volume with an app alone
    /// in it (asked of hdiutil off the main thread): then the bar shows.
    pub(super) fn look_for_image(&mut self, cx: &mut Context<Self>) {
        self.image = None;
        let volume = self.cwd.clone();
        // (A volume's: in /Volumes, or one of F2's own windows for one.)
        if self.virtual_folder.is_some() || (volume.parent() != Some(Path::new("/Volumes")) && !self.laid_out) {
            return;
        }
        cx.spawn(async move |this, cx| {
            let found = cx
                .background_executor()
                .spawn({
                    let volume = volume.clone();
                    async move { Some((crate::disk_images::lone_app(&volume)?, crate::disk_images::image_of(&volume)?)) }
                })
                .await;
            let Some((app, file)) = found else { return };
            let Some(app) = Entry::from_path(app) else { return };
            this.update(cx, |this, cx| {
                if this.cwd == volume {
                    this.image = Some(Image { file, volume, app, state: State::Offered });
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// The app's copy, once copying's done: there (and new), or not (it
    /// was stopped), back to offering.
    fn check_install(&mut self) {
        let busy = !self.jobs.is_empty() || self.conflicts.is_some();
        let Some(image) = self.image.as_mut() else { return };
        let State::Copying(since) = image.state else { return };
        if busy {
            return;
        }
        let at = installed_at(&image.app.path);
        // (Its inode's change, not its date: a copy keeps the app's own.)
        let new = std::fs::metadata(&at).is_ok_and(|m| {
            use std::os::unix::fs::MetadataExt as _;
            let changed = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(m.ctime().max(0) as u64);
            changed + std::time::Duration::from_secs(2) >= since
        });
        image.state = if new { State::Installed(at) } else { State::Offered };
    }

    fn install(&mut self, cx: &mut Context<Self>) {
        let Some(image) = self.image.as_mut() else { return };
        image.state = State::Copying(SystemTime::now());
        let app = image.app.path.clone();
        self.transfer(vec![app], applications(), crate::fileops::How::Copy, cx);
        cx.notify();
    }

    /// Eject the disk image shown (and with `trash`, move its file to the
    /// Trash): its window closes first if it's laid out for it, else goes
    /// home. What goes wrong is said in a note.
    pub(super) fn eject_image(&mut self, trash: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(image) = self.image.take() else { return };
        if self.laid_out {
            window.remove_window();
        } else {
            self.go_place(super::home(), window, cx);
        }
        eject_away(image.volume, trash.then_some(image.file), cx);
    }

    /// The bar, if the volume shown has an app to install.
    pub(super) fn render_install_bar(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.check_install();
        let palette = self.palette;
        let image = self.image.as_ref()?;
        let name = image.app.path.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let px_icon = (20. * window.scale_factor()).round() as u32;
        let icon = file_image(self.icons.get(&image.app, px_icon), &image.app, 20., palette.secondary);
        let button = |id: &'static str, label: String, primary: bool| {
            glass(24., &palette)
                .id(id)
                .flex_shrink_0()
                .px_3()
                .role(Role::Button)
                .aria_label(label.clone())
                .when(primary, |b| b.bg(palette.accent).text_color(white()))
                .child(label)
        };
        let (said, under): (String, Option<&str>) = match &image.state {
            State::Offered => (format!("Install “{name}” in Applications?"), Some("Or drag it there yourself.")),
            State::Copying(_) => (format!("Installing “{name}”…"), None),
            State::Installed(_) => (format!("“{name}” is in Applications."), None),
        };
        let buttons: Vec<AnyElement> = match &image.state {
            State::Offered => vec![button("install-app", "Install".into(), true).on_click(cx.listener(|this, _, _, cx| this.install(cx))).into_any_element()],
            State::Copying(_) => Vec::new(),
            State::Installed(at) => {
                let at = at.clone();
                vec![
                    button("eject-image", "Eject".into(), false).on_click(cx.listener(|this, _, window, cx| this.eject_image(false, window, cx))).into_any_element(),
                    button("trash-image", "Eject and Move to Trash".into(), false)
                        .on_click(cx.listener(|this, _, window, cx| this.eject_image(true, window, cx)))
                        .into_any_element(),
                    button("open-app", format!("Open “{name}”"), true).on_click(move |_, _, cx| cx.open_with_system(&at)).into_any_element(),
                ]
            }
        };
        let laid_out = self.laid_out;
        let bar = div()
            .h(px(crate::disk_images::INSTALL_BAR))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_2()
            .px_4()
            .text_size(px(12.))
            .bg(palette.background)
            .border_color(palette.separator)
            .map(|d| if laid_out { d.border_t_1() } else { d.border_b_1() })
            .child(icon)
            .child(div().flex_shrink_0().font_weight(FontWeight::SEMIBOLD).child(said))
            .child(div().flex_1().min_w_0().truncate().text_color(palette.secondary).children(under))
            .children(buttons);
        Some(bar.into_any_element())
    }
}
