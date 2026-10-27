//! Whether the internet's down, for Quick Open: "is the internet down",
//! "am i online", "wifi not working". Checked a step at a time, as you'd
//! check it: the Mac on a network (Wi-Fi or a cable), the router answering,
//! names being found (DNS), and the internet past it (Apple's page that
//! says "Success", as macOS checks; a Wi-Fi network's sign-in page says
//! something else). Then what's wrong, said plainly, and what to do.

use std::process::Command;
use std::sync::OnceLock;
use std::time::Instant;

pub fn asked(q: &str) -> bool {
    let lower = q.trim().trim_end_matches(['?', '!', '.']).to_lowercase().replace('’', "'");
    const ASKED: &[&str] = &[
        "is the internet down",
        "is internet down",
        "internet down",
        "is my internet down",
        "is my internet working",
        "is the internet working",
        "is internet working",
        "internet not working",
        "my internet is not working",
        "my internet isn't working",
        "no internet",
        "am i online",
        "am i connected",
        "am i offline",
        "is it me or the internet",
        "is wifi down",
        "is the wifi down",
        "is wi-fi down",
        "wifi not working",
        "wi-fi not working",
        "wifi down",
        "check internet",
        "check my internet",
        "check connection",
        "check my connection",
        "internet check",
        "connection check",
        "network check",
        "online",
        "offline",
        "인터넷",
        "인터넷 안됨",
        "인터넷 안 돼",
    ];
    ASKED.contains(&lower.as_str())
}

/// What was found, a step at a time.
#[derive(Clone, Debug, Default)]
pub struct Checked {
    /// The network the Mac's on: its kind ("Wi-Fi", "Ethernet") and its
    /// address on it.
    pub network: Option<(String, String)>,
    /// The router's address, and how long it took to answer (None: it
    /// didn't; some never do).
    pub router: Option<(String, Option<f64>)>,
    /// Whether names are found (DNS).
    pub names: bool,
    /// The internet: how long Apple's page took (ms), if it came.
    pub internet: Option<f64>,
    /// A Wi-Fi network's sign-in page came instead.
    pub sign_in: bool,
}

/// What a network interface is ("en0": "Wi-Fi"), as the Mac's hardware
/// ports say.
fn kind_of(interface: &str) -> String {
    static PORTS: OnceLock<Vec<(String, String)>> = OnceLock::new();
    let ports = PORTS.get_or_init(|| {
        let out = Command::new("/usr/sbin/networksetup").arg("-listallhardwareports").output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
        let mut ports = Vec::new();
        let mut name = None;
        for line in out.lines() {
            if let Some(n) = line.strip_prefix("Hardware Port: ") {
                name = Some(n.trim().to_string());
            } else if let (Some(device), Some(n)) = (line.strip_prefix("Device: "), name.take()) {
                ports.push((device.trim().to_string(), n));
            }
        }
        ports
    });
    match ports.iter().find(|(device, _)| device == interface) {
        Some((_, name)) if name.starts_with("Ethernet") || name.contains("LAN") => "Ethernet".into(),
        Some((_, name)) => name.clone(),
        None if interface.starts_with("utun") || interface.starts_with("ipsec") => "VPN".into(),
        None => interface.to_string(),
    }
}

/// The default route's router and interface.
fn default_route() -> Option<(String, String)> {
    let out = Command::new("/sbin/route").args(["-n", "get", "default"]).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let field = |name: &str| text.lines().find_map(|l| l.trim().strip_prefix(name).map(|v| v.trim().to_string()));
    Some((field("gateway:")?, field("interface:")?))
}

/// How long `host` takes to answer a ping (ms), if it does in a second.
fn ping(host: &str) -> Option<f64> {
    let out = Command::new("/sbin/ping").args(["-c", "1", "-t", "1", "-n", host]).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let at = text.find("time=")?;
    text[at + 5..].split_whitespace().next()?.parse().ok()
}

/// Apple's page, as macOS fetches it to tell it's online: how long it took
/// (ms) and whether it said "Success" (a sign-in page doesn't); or curl's
/// exit code.
fn apple() -> Result<(f64, bool), i32> {
    let started = Instant::now();
    let out = Command::new("/usr/bin/curl").args(["-sS", "--max-time", "5", "-A", "CaptiveNetworkSupport", "http://captive.apple.com/hotspot-detect.html"]).output().map_err(|_| -1)?;
    if !out.status.success() {
        return Err(out.status.code().unwrap_or(-1));
    }
    Ok((started.elapsed().as_secs_f64() * 1000., String::from_utf8_lossy(&out.stdout).contains("Success")))
}

/// Check, a step at a time. (Waits on the network, up to a few seconds:
/// not on the main thread.)
pub fn check() -> Checked {
    let mut checked = Checked::default();
    let route = default_route();
    let ips = crate::this_mac::local_ips();
    // (The network the default route goes by, else the first.)
    let on = route.as_ref().and_then(|(_, interface)| ips.iter().find(|(name, _)| name == interface)).or(ips.first());
    checked.network = on.map(|(interface, ip)| (kind_of(interface), ip.clone()));
    if checked.network.is_none() {
        return checked;
    }
    checked.router = route.filter(|(gateway, _)| gateway.parse::<std::net::IpAddr>().is_ok()).map(|(gateway, _)| {
        let answered = ping(&gateway);
        (gateway, answered)
    });
    match apple() {
        Ok((ms, success)) => {
            checked.names = true;
            checked.internet = success.then_some(ms);
            checked.sign_in = !success;
        }
        // (6: the name wasn't found. Then whether the internet's there by
        // its address alone: names are what's down.)
        Err(6) => {
            checked.names = false;
            let started = Instant::now();
            let reached = Command::new("/usr/bin/curl").args(["-sS", "-o", "/dev/null", "--max-time", "4", "http://1.1.1.1"]).output().is_ok_and(|o| o.status.success());
            checked.internet = reached.then(|| started.elapsed().as_secs_f64() * 1000.);
        }
        Err(_) => checked.names = true,
    }
    checked
}

/// What's wrong (or isn't), plainly, what to do, and how bad: 0 fine, 1
/// half (slow, a sign-in), 2 down.
pub fn verdict(c: &Checked) -> (String, String, u8) {
    let router_quiet = matches!(c.router, Some((_, None)));
    match (&c.network, c.internet, c.sign_in, c.names) {
        (None, ..) => ("You're not connected".into(), "Not on Wi-Fi or a cable: turn Wi-Fi on, or join a network".into(), 2),
        (_, _, true, _) => ("Sign in to this Wi-Fi network".into(), "It wants you to sign in (or agree to its terms) before the internet works".into(), 1),
        (_, Some(_), _, false) => ("Websites' names aren't being found".into(), "The internet's there, but its address book (DNS) isn't answering: restart your router, or try another DNS".into(), 2),
        (_, Some(ms), ..) if ms > 1500. => ("You're online, but it's slow".into(), format!("The internet answered in {}: a speed test says more", millis(ms)), 1),
        (_, Some(ms), ..) => ("You're online".into(), format!("The internet answered in {}", millis(ms)), 0),
        (_, None, ..) if router_quiet => ("Your router isn't answering".into(), "Restart it: unplug it, wait ten seconds, plug it back in".into(), 2),
        (_, None, ..) if c.router.is_none() => ("The internet's down".into(), "This network has no way out to the internet: check the router".into(), 2),
        (_, None, ..) => ("The internet's down".into(), "Your router's answering, but the internet past it isn't: likely your provider. Restarting the router may help".into(), 2),
    }
}

/// "24 ms", "1.2 s".
pub fn millis(ms: f64) -> String {
    if ms >= 1000. { format!("{:.1} s", ms / 1000.) } else { format!("{ms:.0} ms") }
}

#[cfg(test)]
mod tests {
    use super::{Checked, asked, verdict};

    #[test]
    fn asking() {
        assert!(asked("Is the internet down?"));
        assert!(asked("am i online"));
        assert!(!asked("internet explorer"));
    }

    #[test]
    fn verdicts() {
        let on = || Some(("Wi-Fi".to_string(), "192.168.0.12".to_string()));
        let router = |ms: Option<f64>| Some(("192.168.0.1".to_string(), ms));
        let said = |c: Checked| verdict(&c).0;
        assert_eq!(said(Checked::default()), "You're not connected");
        assert_eq!(said(Checked { network: on(), router: router(Some(3.)), names: true, internet: Some(40.), sign_in: false }), "You're online");
        assert_eq!(said(Checked { network: on(), router: router(None), names: true, internet: None, sign_in: false }), "Your router isn't answering");
        assert_eq!(said(Checked { network: on(), router: router(Some(3.)), names: true, internet: None, sign_in: false }), "The internet's down");
        assert_eq!(said(Checked { network: on(), router: router(Some(3.)), names: true, internet: None, sign_in: true }), "Sign in to this Wi-Fi network");
        assert_eq!(said(Checked { network: on(), router: router(Some(3.)), names: false, internet: Some(30.), sign_in: false }), "Websites' names aren't being found");
    }
}
