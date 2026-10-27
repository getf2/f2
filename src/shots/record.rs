//! Screen recording, as macOS's own `screencapture -v` records: the screen
//! the pointer's on, or an area dragged out (F2's picker: macOS's won't
//! record video),
//! with the microphone's sound and clicks shown if Settings say. Stopped
//! from the menu bar item (● and how long, beside it), Quick Open, or its
//! shortcut again; then made an MP4 (by `avconvert`, as it is: not encoded
//! again), saved where screenshots are saved (the Desktop, else), and shown
//! in the corner.

use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use gpui::App;

use crate::macos;
use crate::settings::Settings;

struct Going {
    child: Child,
    /// Where `screencapture` writes it (a QuickTime movie), till it's done.
    file: PathBuf,
    started: Instant,
    /// When it started, for its name.
    at: chrono::DateTime<chrono::Local>,
}

static GOING: Mutex<Option<Going>> = Mutex::new(None);

/// How long it's been recording (None: it isn't).
pub fn elapsed() -> Option<u64> {
    GOING.lock().unwrap().as_ref().map(|g| g.started.elapsed().as_secs())
}

/// A recording as a looping GIF beside it ("… at 10.15.00.gif"), for where
/// a video won't play: 800 pixels wide at most, up to 12 frames a second
/// and 300 at most (a long one gets fewer a second, as few as 4; past 75
/// seconds, only its start). Each frame with its own colors, written as
/// it's read: only one's in memory. Waits: not on the main thread.
pub fn to_gif(video: &Path) -> Result<PathBuf, String> {
    use std::io::BufWriter;
    let gif_path = video.with_extension("gif");
    let partial = video.with_extension("gif.part");
    let mut file = Some(BufWriter::new(std::fs::File::create(&partial).map_err(|e| e.to_string())?));
    let mut encoder: Option<gif::Encoder<BufWriter<std::fs::File>>> = None;
    let mut failed = None;
    let read = macos::video_frames(video, 800, 300, |rgba, w, h, secs| {
        if failed.is_some() {
            return;
        }
        let mut write = || -> Result<(), gif::EncodingError> {
            if encoder.is_none()
                && let Some(file) = file.take()
            {
                let mut started = gif::Encoder::new(file, w as u16, h as u16, &[])?;
                started.set_repeat(gif::Repeat::Infinite)?;
                encoder = Some(started);
            }
            let mut frame = gif::Frame::from_rgba_speed(w as u16, h as u16, rgba, 10);
            frame.delay = (secs * 100.).round().max(2.) as u16;
            match encoder.as_mut() {
                Some(encoder) => encoder.write_frame(&frame),
                None => Ok(()),
            }
        };
        if let Err(e) = write() {
            failed = Some(e.to_string());
        }
    });
    let finished = match (read, failed, encoder) {
        (Err(e), ..) | (Ok(()), Some(e), _) => Err(e),
        (Ok(()), None, None) => Err("its frames couldn't be read".to_string()),
        (Ok(()), None, Some(encoder)) => encoder.into_inner().and_then(|mut w| std::io::Write::flush(&mut w).map_err(Into::into)).map_err(|e| e.to_string()),
    };
    match finished.and_then(|()| std::fs::rename(&partial, &gif_path).map_err(|e| e.to_string())) {
        Ok(()) => Ok(gif_path),
        Err(e) => {
            let _ = std::fs::remove_file(&partial);
            Err(e)
        }
    }
}

/// "1:05", "12:40", "1:02:03".
pub fn clock(secs: u64) -> String {
    let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

/// Start recording (the screen, or an area), or stop if it is.
pub fn toggle(area: bool, cx: &mut App) {
    if elapsed().is_some() {
        return stop();
    }
    if !area {
        return start(None, cx);
    }
    // (macOS's own picker won't record video: F2's, then the area's
    // recorded as a rectangle.)
    let (tx, rx) = futures::channel::oneshot::channel();
    macos::pick_area(tx);
    cx.spawn(async move |cx| {
        if let Ok(Some(area)) = rx.await {
            cx.update(|cx| start(Some(area), cx));
        }
    })
    .detach();
}

/// Stop: `screencapture` finishes the movie when it's told to (as ⌃C
/// does), and `watch` takes it from there.
pub fn stop() {
    if let Some(going) = GOING.lock().unwrap().as_ref() {
        unsafe { libc::kill(going.child.id() as i32, libc::SIGINT) };
    }
}

/// Record the area (x, y, width, height: see `macos::pick_area`), or the
/// screen the pointer's on.
fn start(area: Option<(f64, f64, f64, f64)>, cx: &mut App) {
    if !macos::capture_allowed() {
        macos::capture_request();
        super::preview::note(
            "Screen Recording is off".into(),
            format!("Turn {} on in System Settings > Privacy & Security > Screen & System Audio Recording.", crate::APP_NAME),
            None,
            Some("x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"),
            cx,
        );
        return;
    }
    let shots = Settings::get(cx).shots.clone();
    let dir = std::env::temp_dir().join("F2 Recordings");
    let _ = std::fs::create_dir_all(&dir);
    let at = chrono::Local::now();
    let file = dir.join(format!("{}.mov", at.format("%Y%m%d-%H%M%S")));
    let mut args: Vec<String> = vec!["-v".into(), "-x".into()];
    if shots.record_clicks {
        args.push("-k".into());
    }
    if shots.record_audio {
        args.push("-g".into());
    }
    if let Some((x, y, w, h)) = area {
        args.push(format!("-R{x},{y},{w},{h}"));
    } else if let Some(display) = crate::screens::with_pointer(cx) {
        let (x, y, w, h) = macos::display_frame(u64::from(display.id()) as u32);
        args.push(format!("-R{x},{y},{w},{h}"));
    }
    args.push(file.to_string_lossy().into_owned());
    match Command::new("/usr/sbin/screencapture").args(&args).spawn() {
        Ok(child) => {
            *GOING.lock().unwrap() = Some(Going { child, file, started: Instant::now(), at });
            crate::status_item::refresh(cx);
            // (No note to say so: it'd be in the recording. The menu bar
            // item's ● says it.)
            watch(cx);
        }
        Err(e) => super::preview::note("Couldn't Record".into(), e.to_string(), None, None, cx),
    }
}

/// Till `screencapture` is done (stopped): then the MP4, saved and shown.
fn watch(cx: &mut App) {
    cx.spawn(async move |cx| {
        let done = loop {
            cx.background_executor().timer(Duration::from_millis(400)).await;
            let mut going = GOING.lock().unwrap();
            if going.as_mut().is_none_or(|g| g.child.try_wait().ok().flatten().is_some()) {
                break going.take();
            }
        };
        cx.update(crate::status_item::refresh);
        let Some(done) = done else { return };
        let folder = cx.update(|cx| Settings::get(cx).shots.folder.clone()).unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join("Desktop"));
        let secs = done.started.elapsed().as_secs();
        let saved = cx.background_executor().spawn(async move { finish(&done.file, &folder, done.at) }).await;
        cx.update(|cx| match saved {
            Some((video, thumbnail)) => super::preview::video(video, thumbnail, secs, cx),
            None => super::preview::note("Nothing Was Recorded".into(), String::new(), None, None, cx),
        });
    })
    .detach();
}

/// The movie made an MP4 in `folder`, named as macOS names recordings, and
/// its first frame (for the corner). None: there's nothing in it.
fn finish(movie: &Path, folder: &Path, at: chrono::DateTime<chrono::Local>) -> Option<(PathBuf, Option<PathBuf>)> {
    if std::fs::metadata(movie).map_or(0, |m| m.len()) == 0 {
        let _ = std::fs::remove_file(movie);
        return None;
    }
    let _ = std::fs::create_dir_all(folder);
    let name = at.format("Screen Recording %Y-%m-%d at %H.%M.%S").to_string();
    let out = crate::fileops::free_name(folder, &name, "mp4");
    let converted = Command::new("/usr/bin/avconvert")
        .args(["--preset", "PresetPassthrough", "--source"])
        .arg(movie)
        .arg("--output")
        .arg(&out)
        .output()
        .is_ok_and(|o| o.status.success() && out.exists());
    let video = if converted {
        let _ = std::fs::remove_file(movie);
        out
    } else {
        // (As it was, then: a QuickTime movie.)
        let mov = crate::fileops::free_name(folder, &name, "mov");
        std::fs::rename(movie, &mov).ok()?;
        mov
    };
    let frames = std::env::temp_dir().join("F2 Recordings");
    let _ = std::fs::create_dir_all(&frames);
    let thumbnail = frames.join(format!("{name}.png"));
    let thumbnail = macos::video_thumbnail(&video, &thumbnail, 480).then_some(thumbnail);
    Some((video, thumbnail))
}

#[cfg(test)]
mod tests {
    /// A video as a GIF, and the most memory it took:
    /// `F2_TEST_VIDEO=/path/to.mp4 cargo test --release -- --ignored --nocapture gif_of_a_video`
    #[test]
    #[ignore]
    fn gif_of_a_video() {
        let Some(video) = std::env::var_os("F2_TEST_VIDEO").map(std::path::PathBuf::from) else { return };
        let t = std::time::Instant::now();
        let gif = super::to_gif(&video).unwrap();
        let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
        unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) };
        println!("{} in {:?}: {:.1} MB, most memory {:.0} MB", gif.display(), t.elapsed(), std::fs::metadata(&gif).unwrap().len() as f64 / 1e6, usage.ru_maxrss as f64 / 1e6);
    }

    /// A movie made an MP4, with a frame of it: `RECORD_MOVIE=/some/a.mov
    /// cargo test --release -- --ignored movie_finished`
    #[test]
    #[ignore]
    fn movie_finished() {
        let Some(movie) = std::env::var_os("RECORD_MOVIE").map(std::path::PathBuf::from) else { return };
        let folder = movie.parent().unwrap().join("out");
        let (video, frame) = super::finish(&movie, &folder, chrono::Local::now()).expect("saved");
        println!("{} {:?}", video.display(), frame);
        assert!(video.extension().unwrap() == "mp4" && video.exists() && !movie.exists(), "an MP4, the movie gone");
        assert!(frame.is_some_and(|f| f.exists()), "a frame");
    }

    #[test]
    fn clocks() {
        assert_eq!(super::clock(65), "1:05");
        assert_eq!(super::clock(760), "12:40");
        assert_eq!(super::clock(3723), "1:02:03");
    }
}

