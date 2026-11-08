//! Pointer settings of a profile (General tab) for this mouse only.
//!
//! hid.exe applies them system-wide when a profile becomes active (FUN_00410010):
//! SPI_SETMOUSE with acceleration level 1 = off, 2-4 = on (thresholds 8/4/2+8),
//! SPI_SETMOUSESPEED with the pointer speed 1-20 (10 = Windows default) and
//! SetDoubleClickTime. The wheel slider is only stored. Here they go to the
//! mouse's own input devices: KWin's per-device settings over D-Bus (Plasma,
//! which also saves them), otherwise xinput's libinput properties (X11).
//! Double-click time is one desktop-wide setting and is left alone.

use std::path::Path;
use std::process::Command;

use crate::device::Found;
use crate::model::Profile;

/// libinput values for a profile: (speed -1..1, flat profile, wheel factor).
pub fn values(p: &Profile) -> (f64, bool, f64) {
    let speed = ((p.pointer_speed as f64 - 10.0) / 10.0).clamp(-1.0, 1.0);
    // wheel slider 1 (default) keeps the normal speed, each step adds a third
    let scroll = (p.scroll_speed as f64 + 2.0) / 3.0;
    (speed, p.acceleration <= 1, scroll)
}

/// evdev nodes ("event8") of the pointer interfaces of the USB device `usb`.
fn pointer_nodes(usb: &Path) -> Vec<String> {
    let Ok(usb) = usb.canonicalize() else { return Vec::new() };
    let Ok(rd) = std::fs::read_dir("/sys/class/input") else { return Vec::new() };
    let mut out: Vec<String> = rd
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("event"))
        .filter(|e| e.path().join("device").canonicalize().is_ok_and(|d| d.starts_with(&usb)))
        .filter(|e| {
            // REL_X in the relative-axes bitmask: a pointer, not the keyboard/consumer interfaces
            let rel = std::fs::read_to_string(e.path().join("device/capabilities/rel")).unwrap_or_default();
            let last = rel.split_whitespace().last().unwrap_or("0");
            u64::from_str_radix(last, 16).is_ok_and(|b| b & 1 != 0)
        })
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    out.sort();
    out
}

fn run(cmd: &mut Command) -> bool {
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn kwin(node: &str, speed: f64, flat: bool, scroll: f64) -> bool {
    let path = format!("/org/kde/KWin/InputDevice/{node}");
    let set = |prop: &str, sig: &str, val: &str| {
        run(Command::new("busctl").args(["--user", "set-property", "org.kde.KWin", &path, "org.kde.KWin.InputDevice", prop, sig, val]))
    };
    set("pointerAcceleration", "d", &format!("{speed:.2}"))
        && set("pointerAccelerationProfileFlat", "b", if flat { "true" } else { "false" })
        && set("pointerAccelerationProfileAdaptive", "b", if flat { "false" } else { "true" })
        && set("scrollFactor", "d", &format!("{scroll:.2}"))
}

/// xinput id of the device whose "Device Node" is /dev/input/<node>.
fn xinput_id(node: &str) -> Option<String> {
    let out = Command::new("xinput").args(["list", "--id-only"]).output().ok()?;
    let want = format!("\"/dev/input/{node}\"");
    String::from_utf8_lossy(&out.stdout).split_whitespace().find_map(|id| {
        let props = Command::new("xinput").args(["list-props", id]).output().ok()?;
        String::from_utf8_lossy(&props.stdout).lines().any(|l| l.contains("Device Node") && l.contains(&want)).then(|| id.to_string())
    })
}

fn xinput(node: &str, speed: f64, flat: bool) -> bool {
    let Some(id) = xinput_id(node) else { return false };
    let (a, f) = if flat { ("0", "1") } else { ("1", "0") };
    // newer libinput drivers list a third (custom) profile
    let profile = run(Command::new("xinput").args(["set-prop", &id, "libinput Accel Profile Enabled", a, f, "0"]))
        || run(Command::new("xinput").args(["set-prop", &id, "libinput Accel Profile Enabled", a, f]));
    run(Command::new("xinput").args(["set-prop", &id, "libinput Accel Speed", &format!("{speed:.2}")])) && profile
}

/// Applies `p`'s speed, acceleration and wheel to the mouse `found`; returns what was done.
pub fn apply(found: &Found, p: &Profile) -> String {
    let (speed, flat, scroll) = values(p);
    let nodes = pointer_nodes(&found.usb);
    if nodes.is_empty() {
        return "pointer settings: no pointer interface found".into();
    }
    let x11 = std::env::var("XDG_SESSION_TYPE").is_ok_and(|t| t == "x11");
    let mut done = Vec::new();
    for n in &nodes {
        if !x11 && kwin(n, speed, flat, scroll) {
            done.push(format!("{n} (KWin)"));
        } else if xinput(n, speed, flat) {
            done.push(format!("{n} (xinput)"));
        }
    }
    if done.is_empty() {
        return "pointer settings: this desktop has no per-device mouse settings (KWin or X11 needed)".into();
    }
    format!("pointer settings speed {speed:.2} {} wheel x{scroll:.2}: {}", if flat { "flat" } else { "adaptive" }, done.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_map_to_neutral_speed_and_wheel() {
        let p = Profile::default();
        assert_eq!(values(&p), (0.0, true, 1.0));
        let p = Profile { pointer_speed: 20, acceleration: 3, scroll_speed: 4, ..Profile::default() };
        assert_eq!(values(&p), (1.0, false, 2.0));
        let p = Profile { pointer_speed: 1, ..Profile::default() };
        assert!((values(&p).0 + 0.9).abs() < 1e-9);
    }
}
