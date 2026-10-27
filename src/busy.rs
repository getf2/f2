//! What's slowing the Mac, for Quick Open: "what's slowing my mac", "why
//! is my mac slow", "cpu usage", "task manager". How busy it is (the
//! processor, the memory), and the apps using the most of either, each to
//! quit. Measured over a little over half a second, as Activity Monitor
//! measures it, with what an app has working for it counted as its own (a
//! browser's tabs, an editor's helpers: macOS says whose they are).

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub fn asked(q: &str) -> bool {
    let lower = q.trim().trim_end_matches(['?', '!', '.']).to_lowercase().replace('’', "'");
    const ASKED: &[&str] = &[
        "what's slowing my mac",
        "whats slowing my mac",
        "what is slowing my mac",
        "what's slowing down my mac",
        "what's slowing my mac down",
        "why is my mac slow",
        "why is my mac so slow",
        "my mac is slow",
        "mac is slow",
        "mac slow",
        "slow mac",
        "cpu",
        "cpu usage",
        "memory usage",
        "ram usage",
        "what's using my cpu",
        "what's using the cpu",
        "what's using cpu",
        "what's using memory",
        "what's using my memory",
        "what's using ram",
        "memory hogs",
        "cpu hogs",
        "task manager",
        "processes",
        "is my mac busy",
        "why is my fan loud",
        "why is my fan on",
        "why is my mac hot",
        "why is my mac so hot",
    ];
    ASKED.contains(&lower.as_str())
}

/// An app's share: its name, where it is, its process (to quit it), the
/// processor (percent of one core, as Activity Monitor says it) and its
/// memory (bytes, its footprint).
#[derive(Clone, Debug)]
pub struct Use {
    pub name: String,
    pub path: PathBuf,
    pub pid: i32,
    pub cpu: f64,
    pub memory: u64,
}

/// How busy the Mac is: the processor (percent of all of it), memory used
/// and all there is (bytes), the memory's pressure (0 fine, 1 tight, 2
/// critical), and the apps, busiest first.
#[derive(Clone, Debug)]
pub struct Busy {
    pub cpu: f64,
    pub used: u64,
    pub total: u64,
    pub pressure: u8,
    pub apps: Vec<Use>,
}

unsafe extern "C" {
    /// The process responsible for `pid`: the app a helper or a tab works
    /// for (as Activity Monitor groups them). libquarantine's, in libSystem.
    fn responsibility_get_pid_responsible_for_pid(pid: libc::pid_t) -> libc::pid_t;
}

/// Each process's processor time so far (mach ticks) and memory, of those
/// that are the user's (others' can't be looked at, nor quit).
fn processes() -> HashMap<i32, (u64, u64)> {
    let n = unsafe { libc::proc_listallpids(std::ptr::null_mut(), 0) };
    if n <= 0 {
        return HashMap::new();
    }
    let mut pids = vec![0i32; n as usize + 64];
    let n = unsafe { libc::proc_listallpids(pids.as_mut_ptr().cast(), (pids.len() * size_of::<i32>()) as i32) };
    pids.truncate(n.max(0) as usize);
    pids.into_iter()
        .filter(|pid| *pid > 0)
        .filter_map(|pid| {
            let mut info: libc::rusage_info_v4 = unsafe { std::mem::zeroed() };
            let got = unsafe { libc::proc_pid_rusage(pid, libc::RUSAGE_INFO_V4, (&raw mut info).cast()) };
            (got == 0).then_some((pid, (info.ri_user_time + info.ri_system_time, info.ri_phys_footprint)))
        })
        .collect()
}

/// The whole processor's ticks: busy, and all.
fn load() -> (u64, u64) {
    let mut info: libc::host_cpu_load_info = unsafe { std::mem::zeroed() };
    let mut count = (size_of::<libc::host_cpu_load_info>() / size_of::<libc::integer_t>()) as libc::mach_msg_type_number_t;
    #[allow(deprecated)]
    let host = unsafe { libc::mach_host_self() };
    let got = unsafe { libc::host_statistics64(host, libc::HOST_CPU_LOAD_INFO, (&raw mut info).cast(), &mut count) };
    if got != 0 {
        return (0, 0);
    }
    let [user, system, idle, nice] = info.cpu_ticks.map(u64::from);
    (user + system + nice, user + system + idle + nice)
}

/// Memory used (as Activity Monitor counts it: apps', wired, compressed)
/// and all there is, in bytes.
fn memory() -> (u64, u64) {
    let mut total: u64 = 0;
    let mut size = size_of::<u64>();
    unsafe { libc::sysctlbyname(c"hw.memsize".as_ptr(), (&raw mut total).cast(), &mut size, std::ptr::null_mut(), 0) };
    let mut vm: libc::vm_statistics64 = unsafe { std::mem::zeroed() };
    let mut count = (size_of::<libc::vm_statistics64>() / size_of::<libc::integer_t>()) as libc::mach_msg_type_number_t;
    #[allow(deprecated)]
    let host = unsafe { libc::mach_host_self() };
    if unsafe { libc::host_statistics64(host, libc::HOST_VM_INFO64, (&raw mut vm).cast(), &mut count) } != 0 {
        return (0, total);
    }
    let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) }.max(4096) as u64;
    let pages = u64::from(vm.internal_page_count.saturating_sub(vm.purgeable_count)) + u64::from(vm.wire_count) + u64::from(vm.compressor_page_count);
    ((pages * page).min(total), total)
}

/// The memory's pressure, as macOS sees it (0 fine, 1 tight, 2 critical).
fn pressure() -> u8 {
    let mut level: i32 = 0;
    let mut size = size_of::<i32>();
    unsafe { libc::sysctlbyname(c"kern.memorystatus_vm_pressure_level".as_ptr(), (&raw mut level).cast(), &mut size, std::ptr::null_mut(), 0) };
    match level {
        4 => 2,
        2 => 1,
        _ => 0,
    }
}

/// Mach ticks' nanoseconds: how many a tick is, as a fraction.
#[allow(deprecated)]
fn timebase() -> (f64, f64) {
    let mut base = libc::mach_timebase_info { numer: 1, denom: 1 };
    unsafe { libc::mach_timebase_info(&mut base) };
    (f64::from(base.numer), f64::from(base.denom.max(1)))
}

/// Measure it. (Takes a little over half a second: not on the main thread.)
pub fn measure() -> Busy {
    let apps: HashMap<i32, (String, PathBuf)> = crate::macos::running_apps().into_iter().map(|(pid, name, path)| (pid, (name, path))).collect();
    let me = std::process::id() as i32;
    let (before, load_before, started) = (processes(), load(), Instant::now());
    std::thread::sleep(Duration::from_millis(600));
    let (after, load_after, elapsed) = (processes(), load(), started.elapsed().as_nanos() as f64);
    let (numer, denom) = timebase();
    let ns = |ticks: u64| ticks as f64 * numer / denom;
    let mut shares: HashMap<i32, (f64, u64)> = HashMap::new();
    for (pid, (ticks, footprint)) in &after {
        let owner = match unsafe { responsibility_get_pid_responsible_for_pid(*pid) } {
            owner if owner > 0 => owner,
            _ => *pid,
        };
        if owner == me || !apps.contains_key(&owner) {
            continue;
        }
        // (One that's just started: its memory, not yet its time.)
        let ran = before.get(pid).map_or(0, |(was, _)| ticks.saturating_sub(*was));
        let share = shares.entry(owner).or_default();
        share.0 += ns(ran) / elapsed * 100.;
        share.1 += footprint;
    }
    let mut apps: Vec<Use> = shares
        .into_iter()
        .filter_map(|(pid, (cpu, memory))| {
            let (name, path) = apps.get(&pid)?.clone();
            Some(Use { name, path, pid, cpu, memory })
        })
        .collect();
    apps.sort_by(|a, b| b.cpu.total_cmp(&a.cpu).then(b.memory.cmp(&a.memory)));
    let cpu = match (load_after.0.saturating_sub(load_before.0), load_after.1.saturating_sub(load_before.1)) {
        (_, 0) => 0.,
        (busy, all) => busy as f64 / all as f64 * 100.,
    };
    let (used, total) = memory();
    Busy { cpu, used, total, pressure: pressure(), apps }
}

/// Those to show: the busiest (using the processor at all), then those
/// using the most memory, five at most.
pub fn heaviest(busy: &Busy) -> Vec<Use> {
    let mut shown: Vec<Use> = busy.apps.iter().filter(|a| a.cpu >= 1.).take(3).cloned().collect();
    let mut by_memory: Vec<&Use> = busy.apps.iter().collect();
    by_memory.sort_by_key(|a| std::cmp::Reverse(a.memory));
    for app in by_memory {
        if shown.len() >= 5 {
            break;
        }
        if !shown.iter().any(|s| s.pid == app.pid) {
            shown.push(app.clone());
        }
    }
    shown
}

/// How busy, in a word, and what to do: "Not busy", "Busy", "Very busy";
/// and how bad (0 to 2).
pub fn verdict(busy: &Busy) -> (&'static str, &'static str, u8) {
    let memory = busy.used as f64 / busy.total.max(1) as f64;
    match (busy.cpu, busy.pressure) {
        (_, 2) => ("Out of memory", "Quit an app or two you're not using", 2),
        (c, _) if c >= 80. => ("Very busy", "Something's working hard: quit what you don't need", 2),
        (_, 1) => ("Memory's tight", "Quitting an app you're not using frees some", 1),
        (c, _) if c >= 40. => ("Busy", "It's working on something", 1),
        _ if memory >= 0.9 => ("Not busy", "The memory's nearly all used, but macOS makes room as it needs", 0),
        _ => ("Not busy", "Nothing's slowing it down", 0),
    }
}

#[cfg(test)]
mod tests {
    use super::{asked, measure};

    #[test]
    fn asking() {
        assert!(asked("What's slowing my Mac?"));
        assert!(asked("task manager"));
        assert!(asked("why is my fan loud"));
        assert!(!asked("cpu-z"));
        assert!(!asked("activity monitor"), "the app");
    }

    #[test]
    fn measuring() {
        let busy = measure();
        assert!(busy.total > 0 && busy.used > 0 && busy.used <= busy.total);
        assert!((0. ..=100.).contains(&busy.cpu));
    }
}
