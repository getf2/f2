//! F2's log: what it prints (warnings, errors, and a panic with its
//! backtrace) goes to Logs/F2.log in its data folder when it's opened as
//! an app; run from a terminal, it prints there. Over 1 MB at start-up,
//! it's moved to F2.1.log (the one before goes) and a new one begun.

use std::io::Write;
use std::os::fd::AsRawFd;
use std::path::PathBuf;

const MAX: u64 = 1 << 20;

pub fn file() -> PathBuf {
    crate::settings::data_dir().join("Logs").join(format!("{}.log", crate::APP_NAME))
}

/// The last panic, by itself (see `reports`).
pub fn panic_file() -> PathBuf {
    crate::settings::data_dir().join("Logs").join("panic.txt")
}

/// Into the log from now on (unless it's a terminal's).
pub fn init() {
    if unsafe { libc::isatty(libc::STDERR_FILENO) } == 1 {
        return;
    }
    let path = file();
    let Some(dir) = path.parent() else { return };
    let _ = std::fs::create_dir_all(dir);
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX) {
        let _ = std::fs::rename(&path, path.with_extension("1.log"));
    }
    let Ok(mut log) = std::fs::OpenOptions::new().create(true).append(true).open(&path) else { return };
    let _ = writeln!(
        log,
        "\n--- {} {}, macOS {}, {} ---",
        crate::APP_NAME,
        crate::about::version(),
        crate::macos::os_version().trim_start_matches("Version "),
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S %z")
    );
    unsafe {
        libc::dup2(log.as_raw_fd(), libc::STDOUT_FILENO);
        libc::dup2(log.as_raw_fd(), libc::STDERR_FILENO);
    }
    // (A panic's backtrace too, which the log is mostly for.)
    std::panic::set_hook(Box::new(|info| {
        let thread = std::thread::current();
        let said = format!(
            "{} panicked at {}: {info}\n{}",
            thread.name().unwrap_or("a thread"),
            chrono::Local::now().format("%H:%M:%S"),
            std::backtrace::Backtrace::force_capture()
        );
        eprintln!("{said}");
        // (Kept by itself too: sent with the crash report, if it's one.)
        let _ = std::fs::write(panic_file(), said);
    }));
}
