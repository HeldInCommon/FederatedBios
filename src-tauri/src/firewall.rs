//! A hint about this computer's firewall for the room page (decision record
//! 0027, D5), read without admin rights. The app never changes the firewall;
//! it only says what to do when the firewall would keep the room out.
//!
//! - **macOS:** `socketfilterfw` says whether the firewall is on, and whether
//!   it blocks all incoming connections (which shuts out the room even for
//!   allowed apps).
//! - **Windows:** the network's profile. Windows treats a newly joined Wi-Fi
//!   as Public, which blocks people in the room.
//! - **Linux:** no hint: reading `ufw`'s state needs `sudo`.
//!
//! The readers run a command and wait at most three seconds; the hints are
//! pure functions of the command's output, tested below with sample output.

use std::io::Read;
use std::time::Duration;

/// The hint for a Mac, from `socketfilterfw --getglobalstate` (`global`)
/// and `--getblockall` (`block_all`). `None` when the firewall is off. Pure.
pub fn mac_hint(global: &str, block_all: &str) -> Option<String> {
    let on = global.contains("enabled") || global.contains("State = 1") || global.contains("State = 2");
    if !on {
        return None;
    }
    // Newer macOS says "block all state set to enabled|disabled"; older
    // versions "Block all INCOMING connections DISABLED!" or "… set to block
    // all non-essential incoming connections".
    let block_all = block_all.to_lowercase();
    if block_all.contains("block all") && !block_all.contains("disabled") {
        return Some(
            "This Mac's firewall blocks all incoming connections, so nobody in the room can reach you. \
             In System Settings → Network → Firewall → Options, turn off \"Block all incoming connections\"."
                .into(),
        );
    }
    Some(
        "This Mac's firewall is on. When macOS asks whether hic-commons may accept incoming network \
         connections, choose Allow. If you chose Deny: System Settings → Network → Firewall → Options → \
         hic-commons → Allow incoming connections."
            .into(),
    )
}

/// The hint for Windows, from the network's category (`Public`, `Private`
/// or `DomainAuthenticated`, one per line for each connected network).
/// `None` when no network is Public. Pure.
pub fn windows_hint(categories: &str) -> Option<String> {
    let public = categories.lines().any(|line| line.trim().eq_ignore_ascii_case("public"));
    public.then(|| {
        "Windows treats this network as Public, which keeps people in the room out. If it's a network \
         you trust, set it to Private: Settings → Network & internet → Wi-Fi → this network → Network \
         profile type → Private. When Windows asks about hic-commons, allow it on private networks."
            .into()
    })
}

/// Run `program` with `args` and return what it printed, waiting at most
/// three seconds (a program that hangs is stopped). `None` if it can't run.
/// Only reads; changes nothing.
fn output_of(program: &str, args: &[&str]) -> Option<String> {
    let mut child = crate::network::quiet_command(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    for _ in 0..30 {
        if let Ok(Some(_)) = child.try_wait() {
            let mut out = String::new();
            child.stdout.take()?.read_to_string(&mut out).ok()?;
            return Some(out);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = child.kill();
    let _ = child.wait();
    None
}

/// This computer's firewall hint, if there's anything to say. Only reads.
#[cfg(target_os = "macos")]
pub fn read_hint() -> Option<String> {
    /// macOS's own firewall tool; reading its state needs no admin rights.
    const TOOL: &str = "/usr/libexec/ApplicationFirewall/socketfilterfw";
    let global = output_of(TOOL, &["--getglobalstate"])?;
    let block_all = output_of(TOOL, &["--getblockall"]).unwrap_or_default();
    mac_hint(&global, &block_all)
}

/// This computer's firewall hint, if there's anything to say. Only reads.
#[cfg(target_os = "windows")]
pub fn read_hint() -> Option<String> {
    let categories =
        output_of("powershell", &["-NoProfile", "-Command", "(Get-NetConnectionProfile).NetworkCategory"])?;
    windows_hint(&categories)
}

/// This computer's firewall hint: none on Linux, where reading the firewall
/// needs `sudo` (the guide covers `ufw` and `firewalld`).
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn read_hint() -> Option<String> {
    // Kept so the reader above is used on every platform's build.
    let _ = output_of;
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mac_with_the_firewall_on_is_told_to_allow_hic_commons() {
        let hint = mac_hint("Firewall is enabled. (State = 1)", "Firewall has block all state set to disabled.");
        assert!(hint.unwrap().contains("choose Allow"));
    }

    #[test]
    fn a_mac_blocking_everything_is_told_the_room_cant_reach_it() {
        let hint = mac_hint(
            "Firewall is enabled. (State = 1)",
            "Firewall is set to block all non-essential incoming connections",
        );
        assert!(hint.unwrap().contains("Block all incoming connections"));
    }

    #[test]
    fn every_way_macos_says_block_all_is_off_is_understood() {
        let on = "Firewall is enabled. (State = 1)";
        for off in ["Firewall has block all state set to disabled.", "Block all INCOMING connections DISABLED!"] {
            assert!(mac_hint(on, off).unwrap().contains("choose Allow"), "{off}");
        }
        assert!(mac_hint(on, "Firewall has block all state set to enabled.").unwrap().contains("Block all"));
    }

    #[test]
    fn a_mac_with_the_firewall_off_needs_no_hint() {
        assert_eq!(mac_hint("Firewall is disabled. (State = 0)", ""), None);
    }

    #[test]
    fn a_public_network_on_windows_gets_a_hint() {
        assert!(windows_hint("Public\r\n").unwrap().contains("set it to Private"));
        assert!(windows_hint("Private\nPublic\n").is_some(), "any public network");
        assert_eq!(windows_hint("Private\r\n"), None);
        assert_eq!(windows_hint("DomainAuthenticated"), None);
        assert_eq!(windows_hint(""), None);
    }
}
