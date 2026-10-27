//! This Mac, for Quick Open: "about this mac" (its model, chip, memory,
//! macOS, serial number: what a support call asks), "storage" (free space
//! at a glance), "my ip" (its address on the network here, and to the
//! internet).

use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::Value;

/// What the Mac is.
#[derive(Clone, Debug, Default)]
pub struct About {
    /// "MacBook Pro (14-inch, Nov 2023)", or "MacBook Pro".
    pub model: String,
    /// "Mac15,7".
    pub identifier: String,
    pub chip: String,
    pub memory: String,
    pub serial: String,
}

pub fn about_asked(q: &str) -> bool {
    let lower = q.trim().trim_end_matches('?').to_lowercase();
    matches!(
        lower.as_str(),
        "about this mac" | "about mac" | "about my mac" | "my mac" | "this mac" | "mac info" | "serial number" | "serial" | "mac model" | "model" | "what mac do i have" | "which mac do i have" | "specs" | "mac specs" | "system info" | "my specs"
    )
}

/// What the Mac is (System Information's, asked once: it's slowish).
pub fn about() -> About {
    static ABOUT: OnceLock<About> = OnceLock::new();
    ABOUT
        .get_or_init(|| {
            let json = Command::new("/usr/sbin/system_profiler").args(["SPHardwareDataType", "-json"]).output().ok().and_then(|o| serde_json::from_slice::<Value>(&o.stdout).ok());
            let hw = json.as_ref().and_then(|j| j["SPHardwareDataType"].get(0)).cloned().unwrap_or(Value::Null);
            let field = |key: &str| hw[key].as_str().unwrap_or("").to_string();
            let chip = [field("chip_type"), field("cpu_type")].into_iter().find(|c| !c.is_empty()).unwrap_or_default();
            About { model: marketing_name().unwrap_or_else(|| field("machine_name")), identifier: field("machine_model"), chip, memory: field("physical_memory"), serial: field("serial_number") }
        })
        .clone()
}

/// "MacBook Pro (14-inch, Nov 2023)": as About This Mac says it, from what
/// it keeps (none till it's been opened once).
fn marketing_name() -> Option<String> {
    let out = Command::new("/usr/bin/defaults").args(["read", "com.apple.SystemProfiler", "CPU Names"]).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let value = text.split('=').nth(1)?;
    let name = value.trim().trim_end_matches(['}', ';', '\n', ' ']).trim_end_matches(';').trim().trim_matches('"').to_string();
    (!name.is_empty()).then_some(name)
}

pub fn storage_asked(q: &str) -> bool {
    let lower = q.trim().trim_end_matches('?').to_lowercase();
    matches!(
        lower.as_str(),
        "storage" | "disk space" | "free space" | "space left" | "how much space" | "how much space do i have" | "how much space is left" | "space" | "hard drive space" | "free disk space" | "storage left"
    )
}

/// The startup disk's free space (what macOS counts as free: purgeable
/// too) and its size, in bytes.
pub fn storage() -> Option<(u64, u64)> {
    let free = crate::macos::available_bytes(std::path::Path::new("/"))?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    let root = std::ffi::CString::new("/").ok()?;
    (unsafe { libc::statvfs(root.as_ptr(), &mut stat) } == 0).then(|| (free, stat.f_blocks as u64 * stat.f_frsize as u64))
}

pub fn ip_asked(q: &str) -> bool {
    let lower = q.trim().trim_end_matches('?').to_lowercase();
    matches!(lower.as_str(), "my ip" | "ip" | "ip address" | "my ip address" | "what is my ip" | "whats my ip" | "what's my ip" | "public ip" | "local ip")
}

/// The Mac's addresses on the networks it's on (IPv4): its interface
/// ("en0") and the address, Wi-Fi's (or Ethernet's) first.
pub fn local_ips() -> Vec<(String, String)> {
    let mut found = Vec::new();
    unsafe {
        let mut list: *mut libc::ifaddrs = std::ptr::null_mut();
        if libc::getifaddrs(&mut list) != 0 {
            return found;
        }
        let mut at = list;
        while !at.is_null() {
            let ifa = &*at;
            let up = ifa.ifa_flags & (libc::IFF_UP as u32) != 0 && ifa.ifa_flags & (libc::IFF_LOOPBACK as u32) == 0;
            if up && !ifa.ifa_addr.is_null() && (*ifa.ifa_addr).sa_family as i32 == libc::AF_INET {
                let addr = &*(ifa.ifa_addr as *const libc::sockaddr_in);
                let ip = std::net::Ipv4Addr::from(u32::from_be(addr.sin_addr.s_addr));
                let name = std::ffi::CStr::from_ptr(ifa.ifa_name).to_string_lossy().into_owned();
                if !ip.is_link_local() {
                    found.push((name, ip.to_string()));
                }
            }
            at = ifa.ifa_next;
        }
        libc::freeifaddrs(list);
    }
    found.sort_by_key(|(name, _)| (!name.starts_with("en"), name.clone()));
    found
}

static PUBLIC: Mutex<Option<(Instant, Option<String>)>> = Mutex::new(None);

/// The address the internet sees (ipify's answer: kept a minute). Waits
/// on the network: not on the main thread.
pub fn public_ip() -> Option<String> {
    if let Some((when, ip)) = PUBLIC.lock().unwrap().as_ref()
        && when.elapsed() < Duration::from_secs(60)
    {
        return ip.clone();
    }
    let ip = Command::new("/usr/bin/curl")
        .args(["-fsS", "--max-time", "3", "https://api.ipify.org"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|ip| ip.parse::<std::net::IpAddr>().is_ok());
    *PUBLIC.lock().unwrap() = Some((Instant::now(), ip.clone()));
    ip
}

#[cfg(test)]
mod tests {
    #[test]
    fn asked() {
        assert!(super::about_asked("About this Mac"));
        assert!(super::storage_asked("how much space is left?"));
        assert!(super::ip_asked("what's my ip"));
        assert!(!super::ip_asked("iphone"));
    }

    #[test]
    fn here() {
        let (free, size) = super::storage().unwrap();
        assert!(free > 0 && size > free);
        assert!(!super::about().model.is_empty(), "a model");
    }
}
