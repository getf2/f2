//! The shell under a terminal, as macOS sees it: its folder, and what it's
//! running now (vim, cargo, claude), for a tab's name, ⌘-click's paths,
//! "Open in F2" and a new tab's folder. What's in front is the pty's
//! foreground process group (tcgetpgrp, as tmux asks). The shell itself is
//! login's child (alacritty_terminal starts a login shell through
//! /usr/bin/login, as Terminal does): what's in front, or its forebear.

use std::ffi::{CStr, c_void};
use std::fs::File;
use std::os::fd::AsRawFd;
use std::path::PathBuf;

pub struct Shell {
    /// login's.
    pid: i32,
    /// The pty (a copy of its file: the terminal's thread has its own).
    pty: File,
}

impl Shell {
    pub fn new(pid: u32, pty: &File) -> Option<Shell> {
        Some(Shell { pid: i32::try_from(pid).ok()?, pty: pty.try_clone().ok()? })
    }

    /// The process in front: the shell's own, at its prompt.
    fn front(&self) -> i32 {
        let group = unsafe { libc::tcgetpgrp(self.pty.as_raw_fd()) };
        if group > 0 { group } else { self.pid }
    }

    /// The shell, and what's in front: the shell's the one of what's in
    /// front and its forebears that login started.
    fn shell_and_front(&self) -> (i32, i32) {
        let front = self.front();
        let mut pid = front;
        for _ in 0..16 {
            match parent_of(pid) {
                Some(parent) if parent == self.pid => return (pid, front),
                Some(parent) if parent > 1 => pid = parent,
                _ => break,
            }
        }
        (front, front)
    }

    /// Whether it's running something (not at its prompt).
    pub fn busy(&self) -> bool {
        let (shell, front) = self.shell_and_front();
        shell != front
    }

    /// The folder of what's in front (where its paths are from), else the
    /// shell's.
    pub fn folder(&self) -> Option<PathBuf> {
        let (shell, front) = self.shell_and_front();
        folder_of(front).or_else(|| folder_of(shell))
    }

    /// The shell's own folder (where `cd` took it).
    pub fn shell_folder(&self) -> Option<PathBuf> {
        folder_of(self.shell_and_front().0)
    }

    /// What it's running, if anything ("vim").
    pub fn running(&self) -> Option<String> {
        let (shell, front) = self.shell_and_front();
        (front != shell).then(|| name_of(front)).flatten()
    }
}

fn parent_of(pid: i32) -> Option<i32> {
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<libc::proc_bsdinfo>() as i32;
    let got = unsafe { libc::proc_pidinfo(pid, libc::PROC_PIDTBSDINFO, 0, (&raw mut info).cast::<c_void>(), size) };
    (got == size).then_some(info.pbi_ppid as i32)
}

fn folder_of(pid: i32) -> Option<PathBuf> {
    let mut info: libc::proc_vnodepathinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<libc::proc_vnodepathinfo>() as i32;
    let got = unsafe { libc::proc_pidinfo(pid, libc::PROC_PIDVNODEPATHINFO, 0, (&raw mut info).cast::<c_void>(), size) };
    if got != size {
        return None;
    }
    // (MAXPATHLEN's characters, as libc lays them out: 32 of 32.)
    let path = unsafe { CStr::from_ptr(info.pvi_cdir.vip_path.as_ptr().cast()) };
    let path = PathBuf::from(path.to_string_lossy().into_owned());
    path.is_absolute().then_some(path)
}

fn name_of(pid: i32) -> Option<String> {
    let mut buffer = [0u8; 256];
    let n = unsafe { libc::proc_name(pid, buffer.as_mut_ptr().cast(), buffer.len() as u32) };
    (n > 0).then(|| String::from_utf8_lossy(&buffer[..n as usize]).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_process() {
        let me = std::process::id() as i32;
        assert_eq!(folder_of(me), std::env::current_dir().ok());
        assert!(name_of(me).is_some_and(|n| n.starts_with("new_finder") || n.starts_with("new-finder")), "{:?}", name_of(me));
        assert_eq!(parent_of(me), Some(std::os::unix::process::parent_id() as i32));
    }
}
