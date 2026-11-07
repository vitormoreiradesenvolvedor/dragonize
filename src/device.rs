//! Configuration of the Holtek-based mice over hidraw: the M801-RGB (04d9:fc58)
//! and the rest of the RDCfg family, which share this protocol (models.rs).
//!
//! Protocol recorded from the original driver (hid.exe -> HidDevice.dll), see
//! MECHANICS.md "Device protocol". Feature report 2 (16 bytes) / 3 (64 bytes):
//!   02 f5 00 / 02 f5 01                 open / close session
//!   ID f2 LO HI LEN + GET_FEATURE       read; reply ID 08 LO&~1 60+HI LEN 00 fa fa DATA
//!   ID f3 LO HI LEN 00 00 00 DATA       write
//!   02 f1 02 MASK                       commit (0x01 profile, 0x02 global, 0x04 buttons, 0x08, 0x10 dpi)
//!
//! Only fields whose encoding was verified on the device are written; every
//! other byte is left as the device holds it.

use std::fs::{File, OpenOptions};
use std::os::fd::AsRawFd;
use std::path::PathBuf;

use crate::model::{Action, FireKey, Func, Macro, MacroStep, PlayMode, Profile};
use crate::models::{self, Model};
/// Start of each on-device profile block.
pub const PROFILE_BASE: [u16; 5] = [0x40, 0x100, 0x1b0, 0x260, 0x310];
const ACTIVE_PROFILE: u16 = 0x2c;
const POLLING: u16 = 0x32; // + 2 * profile, value = report interval in ms
// Button slots (offsets from a profile base) depend on the model: models.rs.

/// DPI register values for 100..6200 CPI (index = cpi/100 - 1), from hid.exe.
const DPI_TABLE: [u8; 62] = [
    0, 4, 6, 9, 11, 13, 15, 18, 20, 22, 24, 27, 29, 31, 33, 36, 38, 40, 43, 45, 47, 49, 52, 54, 56, 58, 61,
    63, 65, 67, 70, 72, 74, 77, 79, 81, 83, 86, 88, 90, 92, 95, 97, 99, 102, 104, 106, 108, 111, 113, 115,
    117, 120, 122, 124, 127, 129, 131, 133, 135, 138, 140,
];

/// CPI -> 16-bit register. Above 6200 the 0x100 bit selects double resolution
/// and the low byte encodes cpi/2 (8000 -> 0x15a, 11800 -> 0x185, seen on the device).
pub fn dpi_encode(cpi: u32) -> u16 {
    if cpi <= 6200 {
        DPI_TABLE[(cpi / 100).clamp(1, 62) as usize - 1] as u16
    } else {
        0x100 | DPI_TABLE[(cpi / 200).clamp(1, 62) as usize - 1] as u16
    }
}

pub fn dpi_decode(v: u16) -> u32 {
    let low = (v & 0xff) as u8;
    let i = DPI_TABLE.iter().position(|&t| t >= low).unwrap_or(61);
    let cpi = (i as u32 + 1) * 100;
    if v & 0x100 != 0 { cpi * 2 } else { cpi }
}

/// Polling index used by the UI (0 = 125 Hz .. 3 = 1000 Hz) <-> interval in ms.
pub fn polling_encode(idx: usize) -> u8 {
    [8, 4, 2, 1][idx.min(3)]
}
pub fn polling_decode(ms: u8) -> usize {
    match ms {
        1 => 3,
        2 => 2,
        4 => 1,
        _ => 0,
    }
}

/// HID keyboard usage for an egui key name (single keys; combos are not
/// encodable on this firmware as far as verified).
pub fn key_usage(name: &str) -> Option<u8> {
    let n = name;
    if n.len() == 1 {
        let c = n.chars().next()?;
        if c.is_ascii_uppercase() {
            return Some(0x04 + (c as u8 - b'A'));
        }
        if c.is_ascii_digit() {
            return Some(if c == '0' { 0x27 } else { 0x1e + (c as u8 - b'1') });
        }
    }
    if let Some(f) = n.strip_prefix('F').and_then(|d| d.parse::<u8>().ok()) {
        if (1..=12).contains(&f) {
            return Some(0x3a + f - 1);
        }
        if (13..=24).contains(&f) {
            return Some(0x68 + f - 13);
        }
    }
    Some(match n {
        "Enter" => 0x28,
        "Escape" => 0x29,
        "Backspace" => 0x2a,
        "Tab" => 0x2b,
        "Space" => 0x2c,
        "Minus" => 0x2d,
        "Equals" | "Plus" => 0x2e,
        "OpenBracket" => 0x2f,
        "CloseBracket" => 0x30,
        "Backslash" => 0x31,
        "Semicolon" => 0x33,
        "Quote" => 0x34,
        "Backtick" => 0x35,
        "Comma" => 0x36,
        "Period" => 0x37,
        "Slash" => 0x38,
        "Insert" => 0x49,
        "Home" => 0x4a,
        "PageUp" => 0x4b,
        "Delete" => 0x4c,
        "End" => 0x4d,
        "PageDown" => 0x4e,
        "ArrowRight" => 0x4f,
        "ArrowLeft" => 0x50,
        "ArrowDown" => 0x51,
        "ArrowUp" => 0x52,
        "LCtrl" | "Ctrl" => 0xe0,
        "LShift" | "Shift" => 0xe1,
        "LAlt" | "Alt" => 0xe2,
        "LGui" | "Win" => 0xe3,
        "RCtrl" => 0xe4,
        "RShift" => 0xe5,
        "RAlt" => 0xe6,
        "RGui" => 0xe7,
        _ => return None,
    })
}

pub fn usage_key(u: u8) -> String {
    match u {
        0x04..=0x1d => ((b'A' + u - 0x04) as char).to_string(),
        0x1e..=0x26 => ((b'1' + u - 0x1e) as char).to_string(),
        0x27 => "0".into(),
        0x3a..=0x45 => format!("F{}", u - 0x3a + 1),
        0x68..=0x73 => format!("F{}", u - 0x68 + 13),
        0xe0 => "LCtrl".into(),
        0xe1 => "LShift".into(),
        0xe2 => "LAlt".into(),
        0xe3 => "LGui".into(),
        0xe4 => "RCtrl".into(),
        0xe5 => "RShift".into(),
        0xe6 => "RAlt".into(),
        0xe7 => "RGui".into(),
        _ => {
            for n in ["Enter", "Escape", "Backspace", "Tab", "Space", "Minus", "Equals", "OpenBracket", "CloseBracket",
                "Backslash", "Semicolon", "Quote", "Backtick", "Comma", "Period", "Slash", "Insert", "Home", "PageUp",
                "Delete", "End", "PageDown", "ArrowRight", "ArrowLeft", "ArrowDown", "ArrowUp"] {
                if key_usage(n) == Some(u) {
                    return n.into();
                }
            }
            format!("0x{u:02X}")
        }
    }
}

/// Consumer usages for MEDIA 1-7 (hid.exe table, ids 39-45); 8 "player" has no code.
const MEDIA_USAGE: [u8; 7] = [0xcd, 0xb7, 0xb6, 0xb5, 0xe9, 0xea, 0xe2];

/// The original driver writes Basic/Advanced as `8e 01 ff 10+n` / `8e 01 ff 18+n`:
/// the mouse then sends consumer usage 0x1fff, outside its descriptor's range
/// (0..0xfff), which only the vendor's hid.exe understands (it replays keystrokes
/// with keybd_event, hid.exe @0x411ae0). Linux drops that report, so the same
/// actions are written as what hid.exe would type (Ctrl+X ..., Alt+Tab, Alt+F4,
/// Win+E/D/L) or as the standard consumer usage (lo, hi) the OS handles itself.
/// Win+R becomes Alt+F2 (run dialog on KDE/GNOME), LockWorkStation Win+L.
const BASIC_CODES: [[u8; 4]; 8] = [
    [0x8f, 0x01, 0x1b, 0], // Ctrl+X
    [0x8f, 0x01, 0x06, 0], // Ctrl+C
    [0x8f, 0x01, 0x19, 0], // Ctrl+V
    [0x8f, 0x01, 0x04, 0], // Ctrl+A
    [0x8f, 0x01, 0x09, 0], // Ctrl+F
    [0x8f, 0x01, 0x11, 0], // Ctrl+N
    [0x8f, 0x01, 0x13, 0], // Ctrl+P
    [0x8f, 0x01, 0x16, 0], // Ctrl+S
];
const ADVANCED_CODES: [[u8; 4]; 14] = [
    [0x8f, 0x04, 0x2b, 0],    // Alt+Tab
    [0x8f, 0x04, 0x3d, 0],    // Alt+F4
    [0x8f, 0x08, 0x08, 0],    // Win+E
    [0x8f, 0x04, 0x3b, 0],    // Alt+F2
    [0x8f, 0x08, 0x07, 0],    // Win+D
    [0x8f, 0x08, 0x0f, 0],    // Win+L
    [0x8e, 0x01, 0x96, 0x01], // AL Internet Browser
    [0x8e, 0x01, 0x24, 0x02], // AC Back
    [0x8e, 0x01, 0x25, 0x02], // AC Forward
    [0x8e, 0x01, 0x26, 0x02], // AC Stop
    [0x8e, 0x01, 0x27, 0x02], // AC Refresh
    [0x90, 0x00, 0x3c, 0],    // F3 (hid.exe sends VK_F3)
    [0x8e, 0x01, 0x2a, 0x02], // AC Bookmarks
    [0x8e, 0x01, 0x8a, 0x01], // AL Email Reader
];

/// Modifier bits of the combo code `8f MASK KEY` (hid.exe FUN_00413990).
pub fn combo_mask(m: &str) -> Option<u8> {
    Some(match m {
        "CTRL" => 0x01,
        "SHIFT" => 0x02,
        "ALT" => 0x04,
        "WIN" => 0x08,
        _ => return None,
    })
}

/// Button function -> 4 device bytes, following the original driver's table
/// (hid.exe @0x47e3c0, pfd ButtonFunc id -> code) and its special cases for
/// keys, combos, fire key and macros. `None` = not encodable (device bytes kept).
pub fn encode_func(f: &Func) -> Option<[u8; 4]> {
    if models::current().is_sino() {
        return crate::sino::encode_func(f);
    }
    if models::current().is_compx() {
        return crate::compx::encode_func(f);
    }
    if models::current().is_du() {
        return crate::du::encode_func(f);
    }
    if models::current().is_xy() {
        return crate::xy::encode_func(f);
    }
    if models::current().is_bd() {
        return crate::bd::encode_func(f);
    }
    if models::current().is_mo() {
        return crate::mo::encode_func(f);
    }
    if models::current().is_dc() {
        return crate::dc::encode_func(f);
    }
    if models::current().is_px() {
        return crate::pxi::encode_func(f);
    }
    Some(match f {
        Func::Click => [0x81, 0, 0, 0],
        Func::Menu => [0x82, 0, 0, 0],
        Func::Middle => [0x83, 0, 0, 0],
        Func::Forward => [0x85, 0, 0, 0],
        Func::Back => [0x84, 0, 0, 0],
        Func::Key(k) => [0x90, 0, key_usage(k)?, 0],
        Func::Combo(spec) => {
            let parts: Vec<&str> = spec.split('+').collect();
            let (key, mods) = parts.split_last()?;
            let mut mask = 0;
            for m in mods {
                mask |= combo_mask(m)?;
            }
            [0x8f, mask, key_usage(key)?, 0]
        }
        Func::Basic(i) => *BASIC_CODES.get((*i as usize).checked_sub(1)?)?,
        Func::Advanced(i) => *ADVANCED_CODES.get((*i as usize).checked_sub(1)?)?,
        Func::Media(i) => [0x8e, 0x01, *MEDIA_USAGE.get((*i as usize).checked_sub(1)?)?, 0],
        Func::Dpi(1) => [0x88, 0, 0, 0],
        Func::Dpi(2) => [0x8a, 0, 0, 0],
        Func::Dpi(3) => [0x89, 0, 0, 0],
        Func::ProfileSwitch => [0x8d, 0, 0, 0],
        Func::Rate(1) => [0x97, 0, 0, 0],
        Func::Rate(2) => [0x98, 0, 0, 0],
        Func::ScrollUp => [0x8b, 0, 0, 0],
        Func::ScrollDown => [0x8c, 0, 0, 0],
        Func::LightSwitch => [0x9b, 0x04, 0, 0],
        Func::FireKey(fk) if fk.target < 3 => {
            [0x99, [0x81, 0x82, 0x84][fk.target as usize], fk.times.clamp(1, 255) as u8, (fk.delay_ms / 10).clamp(1, 255) as u8]
        }
        Func::Disabled => [0, 0, 0, 0],
        Func::Raw(b) => *b,
        _ => return None,
    })
}

pub fn decode_func(b: [u8; 4]) -> Option<Func> {
    if models::current().is_sino() {
        return crate::sino::decode_func(b);
    }
    if models::current().is_compx() {
        return crate::compx::decode_func(b);
    }
    if models::current().is_du() {
        return crate::du::decode_func(b);
    }
    if models::current().is_xy() {
        return crate::xy::decode_func(b);
    }
    if models::current().is_bd() {
        return crate::bd::decode_func(b);
    }
    if models::current().is_mo() {
        return crate::mo::decode_func(b);
    }
    if models::current().is_dc() {
        return crate::dc::decode_func(b);
    }
    if models::current().is_px() {
        return crate::pxi::decode_func(b);
    }
    if let Some(i) = BASIC_CODES.iter().position(|&c| c == b) {
        return Some(Func::Basic(i as u8 + 1));
    }
    if let Some(i) = ADVANCED_CODES.iter().position(|&c| c == b) {
        return Some(Func::Advanced(i as u8 + 1));
    }
    Some(match b {
        [0x81, ..] => Func::Click,
        [0x82, ..] => Func::Menu,
        [0x83, ..] => Func::Middle,
        [0x85, ..] => Func::Forward,
        [0x84, ..] => Func::Back,
        [0x88, ..] => Func::Dpi(1),
        [0x8a, ..] => Func::Dpi(2),
        [0x89, ..] => Func::Dpi(3),
        [0x8b, ..] => Func::ScrollUp,
        [0x8c, ..] => Func::ScrollDown,
        [0x8d, ..] => Func::ProfileSwitch,
        [0x97, ..] => Func::Rate(1),
        [0x98, ..] => Func::Rate(2),
        [0x9b, 0x04, ..] => Func::LightSwitch,
        [0x90, _, k, _] => Func::Key(usage_key(k)),
        [0x8f, mask, k, _] => {
            let mut spec = String::new();
            for (bit, name) in [(1, "CTRL"), (2, "SHIFT"), (4, "ALT"), (8, "WIN")] {
                if mask & bit != 0 {
                    spec.push_str(name);
                    spec.push('+');
                }
            }
            spec.push_str(&usage_key(k));
            Func::Combo(spec)
        }
        [0x8e, _, 0xff, c] if (0x11..=0x18).contains(&c) => Func::Basic(c - 0x10),
        [0x8e, _, 0xff, c] if (0x19..=0x26).contains(&c) => Func::Advanced(c - 0x18),
        [0x8e, _, u, 0] => Func::Media(MEDIA_USAGE.iter().position(|&m| m == u)? as u8 + 1),
        [0x99, t @ (0x81 | 0x82 | 0x84), times, delay] => Func::FireKey(FireKey {
            target: match t { 0x81 => 0, 0x82 => 1, _ => 2 },
            key: String::new(),
            times: times as u32,
            delay_ms: delay as u32 * 10,
        }),
        [0, 0, 0, 0] => Func::Disabled,
        _ => return None,
    })
}

// ------------------------------------------------------------------ access
//
// /dev/hidraw* and the usbfs node are root-only on most distributions. That is
// host policy no AppImage can bypass, so the app installs (once, through the
// desktop's polkit password prompt) a udev rule that grants the logged-in user
// access to this mouse only.

pub const UDEV_PATH: &str = "/etc/udev/rules.d/70-dragonize.rules";

/// One hidraw + usb line per supported vendor/product id (only those: 04d9 is
/// Holtek's id and is shared by unrelated keyboards and mice).
pub fn udev_rule() -> String {
    let mut r = String::from("# Dragonize: let the logged-in user configure the supported gaming mice\n");
    for (vid, pid) in models::product_ids() {
        r += &format!("SUBSYSTEM==\"hidraw\", ATTRS{{idVendor}}==\"{vid:04x}\", ATTRS{{idProduct}}==\"{pid:04x}\", TAG+=\"uaccess\"\n");
        r += &format!("SUBSYSTEM==\"usb\", ATTR{{idVendor}}==\"{vid:04x}\", ATTR{{idProduct}}==\"{pid:04x}\", TAG+=\"uaccess\"\n");
    }
    r
}

#[derive(Debug, PartialEq)]
pub enum Access {
    Ok,
    NotConnected,
    NoPermission(PathBuf),
}

/// Access to every plugged supported mouse: Ok only when all can be opened.
pub fn access() -> Access {
    let found = scan();
    if found.is_empty() {
        return Access::NotConnected;
    }
    for f in &found {
        match OpenOptions::new().read(true).write(true).open(&f.hidraw) {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => return Access::NoPermission(f.hidraw.clone()),
            Err(_) => return Access::NotConnected,
        }
    }
    Access::Ok
}

/// Installs the udev rule through pkexec and re-applies it to the plugged mouse.
pub fn install_udev_rule() -> Result<(), String> {
    let tmp = std::env::temp_dir().join(format!("m801-rgb-{}.rules", std::process::id()));
    std::fs::write(&tmp, udev_rule()).map_err(|e| e.to_string())?;
    let script = format!(
        "install -m 0644 \"$0\" {UDEV_PATH} && udevadm control --reload-rules && \
         udevadm trigger --action=change --subsystem-match=usb && \
         udevadm trigger --action=change --subsystem-match=hidraw && udevadm settle"
    );
    let r = std::process::Command::new("pkexec").arg("/bin/sh").arg("-c").arg(&script).arg(&tmp).status();
    let _ = std::fs::remove_file(&tmp);
    match r {
        Ok(st) if st.success() => {
            // the ACL is applied asynchronously after the trigger
            for _ in 0..30 {
                if !matches!(access(), Access::NoPermission(_)) {
                    return Ok(());
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err("udev rule installed, but access is still denied (no systemd-logind seat?)".into())
        }
        Ok(st) => Err(format!("pkexec exited with {st}")),
        Err(e) => Err(format!("pkexec not available: {e}")),
    }
}

/// Human-readable report of everything that decides whether the mouse can be used.
pub fn diagnose() -> String {
    use std::fmt::Write;
    let mut o = String::new();
    let read = |p: &str| std::fs::read_to_string(p).unwrap_or_default().trim().to_string();
    let _ = writeln!(o, "kernel: {}", read("/proc/sys/kernel/osrelease"));
    let _ = writeln!(o, "os: {}", read("/etc/os-release").lines().find(|l| l.starts_with("PRETTY_NAME")).unwrap_or(""));
    let found = scan();
    for f in &found {
        let rw = OpenOptions::new().read(true).write(true).open(&f.hidraw).map(|_| "read/write OK".to_string()).unwrap_or_else(|e| e.to_string());
        let _ = writeln!(o, "{}: {} ({:04x}:{:04x} rev {:04x}, usb {}): {}", f.hidraw.display(), f.model.name, f.model.vid, f.model.pid, f.bcd, f.usb_name(), rw);
    }
    if found.is_empty() {
        let _ = writeln!(o, "no supported mouse ({}) found on USB",
            models::product_ids().iter().map(|(v, p)| format!("{v:04x}:{p:04x}")).collect::<Vec<_>>().join(" "));
    }
    let _ = writeln!(o, "udev rule {}: {}", UDEV_PATH, if std::path::Path::new(UDEV_PATH).exists() { "installed" } else { "missing" });
    let _ = writeln!(o, "access: {:?}", access());
    o
}

/// A plugged supported mouse: its configuration interface and USB device.
#[derive(Clone, Debug)]
pub struct Found {
    pub hidraw: PathBuf,
    /// sysfs directory of the USB device (holds busnum/devnum/bcdDevice)
    pub usb: PathBuf,
    pub bcd: u16,
    pub model: &'static Model,
}

impl Found {
    /// Stable name of the USB port (e.g. "1-4"), survives a re-enumeration.
    pub fn usb_name(&self) -> String {
        self.usb.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
    }

    /// sysfs node of the hidraw interface (/sys/class/hidraw/hidrawN)
    pub fn hidraw_sys(&self) -> PathBuf {
        PathBuf::from("/sys/class/hidraw").join(self.hidraw.file_name().unwrap_or_default())
    }
}

/// Every plugged supported mouse, by its vendor-page (0xffa0) hidraw interface.
pub fn scan() -> Vec<Found> {
    let pids = models::product_ids();
    let mut out = Vec::new();
    // UI tests must not see (or open) a mouse that happens to be plugged in
    if cfg!(test) {
        return out;
    }
    let Ok(rd) = std::fs::read_dir("/sys/class/hidraw") else { return out };
    let mut nodes: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    nodes.sort();
    for n in nodes {
        let uevent = std::fs::read_to_string(n.join("device/uevent")).unwrap_or_default();
        let Some(id) = uevent.lines().find_map(|l| l.strip_prefix("HID_ID=")) else { continue };
        // HID_ID=0003:000004D9:0000FC58
        let mut it = id.split(':').skip(1).map(|h| u32::from_str_radix(h, 16).unwrap_or(0));
        let (vid, pid) = (it.next().unwrap_or(0), it.next().unwrap_or(0));
        if !pids.contains(&(vid as u16, pid as u16)) {
            continue;
        }
        let rd = std::fs::read(n.join("device/report_descriptor")).unwrap_or_default();
        let has = |pat: &[u8]| rd.windows(pat.len()).any(|w| w == pat);
        let mut proto = models::identify(vid as u16, pid as u16, 0).map(|m| m.proto);
        // 258a:1007 is an OemDrv mouse (S101-4-DE: command report 5), the CDrv8832 M712 (no
        // report 5) and the Lavawolf M701 (0x9a-byte general report 4): the descriptor tells which
        if matches!(proto, Some(models::Proto::Sino | models::Proto::Sino8832 | models::Proto::Sino8801)) {
            let want = if has(&[0x85, 0x05]) {
                models::Proto::Sino
            } else if crate::sino::feature_size(&rd, 4) == Some(0x9a) {
                models::Proto::Sino8801
            } else {
                models::Proto::Sino8832
            };
            if models::MODELS.iter().any(|m| m.vid == vid as u16 && m.pid == pid as u16 && m.proto == want) {
                proto = Some(want);
            }
        }
        let ok = match proto {
            // Sinowealth: the interface with feature reports 4 (config) and 5 (command)
            Some(models::Proto::Sino) => has(&[0x85, 0x04]) && has(&[0x85, 0x05]),
            Some(models::Proto::SinoBy24g) => has(&[0x85, 0x05]) && has(&[0x85, 0x08]),
            Some(models::Proto::Sino8832) => has(&[0x85, 0x04]) && has(&[0x85, 0x06]) && has(&[0x85, 0x08]),
            Some(models::Proto::Sino8801) => has(&[0x85, 0x04]) && has(&[0x85, 0x06]),
            // CompX: the one with the 17-byte report 8 (HIDUsb.dll's mi_01&col05)
            Some(models::Proto::Compx | models::Proto::CompxFeature) => has(&[0x85, 0x08]),
            // keyboards: command report 5 and the 1032-byte data report 6
            // (BYK901 builds: the interface with feature report 9, the key matrix)
            Some(models::Proto::SinoKbd) if models::identify(vid as u16, pid as u16, 0).and_then(|m| m.kb).is_some_and(|k| k.byk) => has(&[0x85, 0x09]),
            // QMK/VIA: the raw HID interface (usage page 0xff60)
            Some(models::Proto::SinoKbd) if models::identify(vid as u16, pid as u16, 0).and_then(|m| m.kb).is_some_and(|k| k.via.is_some()) => has(&[0x06, 0x60, 0xff]),
            // xyui: the 8-byte feature report 7
            Some(models::Proto::SinoKbd) if models::identify(vid as u16, pid as u16, 0).and_then(|m| m.kb).is_some_and(|k| k.xy) => has(&[0x85, 0x07]),
            // Jieli: USB interface 2 (witmodSdk GetHidInfoList "mi_02")
            Some(models::Proto::SinoKbd) if models::identify(vid as u16, pid as u16, 0).and_then(|m| m.kb).is_some_and(|k| k.jl.is_some()) => {
                std::fs::canonicalize(n.join("device")).ok().and_then(|p| p.parent().and_then(|i| i.file_name()).map(|f| f.to_string_lossy().ends_with(".2"))).unwrap_or(false)
            }
            // magnetic K729RGB-M: usage page 0xff00, usage 1 (FUN_0046a0b0)
            Some(models::Proto::SinoKbd) if models::identify(vid as u16, pid as u16, 0).and_then(|m| m.kb).is_some_and(|k| k.mg) => has(&[0x06, 0x00, 0xff, 0x09, 0x01]),
            // MosArt (K598-KNS / K599): the interface with report 8 and feature reports 0x14 / 0x15
            Some(models::Proto::SinoKbd) if models::identify(vid as u16, pid as u16, 0).and_then(|m| m.kb).is_some_and(|k| k.mk) => has(&[0x85, 0x14]),
            // HidServ: the 64-byte report interface on usage page 0xff1c, usage 0x92
            Some(models::Proto::SinoKbd) if models::identify(vid as u16, pid as u16, 0).and_then(|m| m.kb).is_some_and(|k| k.hs.is_some() || k.du) => {
                has(&[0x06, 0x1c, 0xff]) && has(&[0x09, 0x92])
            }
            Some(models::Proto::SinoKbd) => has(&[0x85, 0x05]) && has(&[0x85, 0x06]),
            // duilib mice: the HidServ interface (usage page 0xff1c, usage 0x92)
            Some(models::Proto::Duilib) => has(&[0x06, 0x1c, 0xff]) && has(&[0x09, 0x92]),
            // xyui mice: usage page 0xff00, usage 1 (DeviceDriver.exe FUN_00440e20)
            Some(models::Proto::Xyui | models::Proto::Bydui) => has(&[0x06, 0x00, 0xff, 0x09, 0x01]),
            // MosArt: the interface with output report 5
            Some(models::Proto::Mosart) => has(&[0x85, 0x05]),
            // libdc: the interface with feature reports 2..6 (16..2048 bytes)
            Some(models::Proto::Libdc) => has(&[0x85, 0x06]),
            // PixArt: the interface with feature report 5 (PXICtrl3169 HidD_SetFeature id 5)
            Some(models::Proto::Pixart) => has(&[0x85, 0x05]),
            _ => has(&[0x06, 0xa0, 0xff]),
        };
        if !ok {
            continue;
        }
        let Ok(dev) = std::fs::canonicalize(n.join("device")) else { continue };
        let Some(usb) = dev.ancestors().find(|a| a.join("bcdDevice").exists()).map(PathBuf::from) else { continue };
        let bcd = std::fs::read_to_string(usb.join("bcdDevice")).ok().and_then(|s| u16::from_str_radix(s.trim(), 16).ok()).unwrap_or(0);
        let Some(mut model) = models::identify(vid as u16, pid as u16, bcd) else { continue };
        if let Some(p) = proto.filter(|&p| p != model.proto && model.is_sino()) {
            model = models::MODELS.iter().find(|m| m.vid == vid as u16 && m.pid == pid as u16 && m.proto == p).unwrap_or(model);
        }
        let Some(name) = n.file_name() else { continue };
        let mut found = Found { hidraw: PathBuf::from("/dev").join(name), usb, bcd, model };
        // one dongle id, several mice: ask the mouse its CID / MID (OemDrv CheckPsd)
        if models::needs_psd(vid as u16, pid as u16) {
            if let Some((cid, mid)) = Device::open(&found).ok().and_then(|d| crate::compx::psd(&d)) {
                model = models::identify_psd(vid as u16, pid as u16, cid, mid).unwrap_or(model);
            }
            found.model = model;
        }
        // one id, dozens of keyboards: their password (KeyboardDrv / OemDrv GetPassword)
        if model.kb.is_some_and(|k| !k.byk && !k.xy && k.via.is_none() && k.hs.is_none() && k.jl.is_none() && !k.du && !k.mk && !k.mg) && models::MODELS.iter().filter(|m| m.vid == vid as u16 && m.pid == pid as u16).count() > 1 {
            if let Some(p) = Device::open(&found).ok().and_then(|d| crate::kbd::psd(&d)) {
                found.model = models::identify_kbd(vid as u16, pid as u16, p).unwrap_or(model);
            }
        }
        // HidServ: dozens of keyboards answer as 0c45:5004 and nothing tells them apart; one
        // entry per candidate, the device selection dialog lets the user name theirs. Jieli: the
        // candidates whose firmware name the keyboard answers (all of them without an answer)
        // duilib mice: a handful of ids for 15 mice; the selection dialog names the mouse
        // (xyui mice: one 2.4 GHz dongle id for all of them)
        // CDrvBY24G mice: M602-KS, M910-KS, M811-RGB-PRO and M690-PRO share 258a:002e / 002f
        // (old OemDrv mice without a CID / MID: M913, M915RGB-WL, M808-KS, M901P-KS on 25a7:fa08)
        let same = |m: &&models::Model| m.proto == model.proto && m.bcd == model.bcd && m.vid == vid as u16 && (m.pid == pid as u16 || m.alt_pids.contains(&(pid as u16)));
        let shared = model.proto == models::Proto::CompxFeature && model.bcd == (0, 0) && models::MODELS.iter().filter(same).count() > 1;
        if model.is_du() || model.is_xy() || model.is_mo() || model.is_dc() || model.proto == models::Proto::SinoBy24g || shared {
            for m in models::MODELS.iter().filter(same) {
                out.push(Found { model: m, ..found.clone() });
            }
            continue;
        }
        if model.kb.is_some_and(|k| k.hs.is_some() || k.jl.is_some() || k.du) {
            let same = |m: &&models::Model| m.vid == vid as u16 && (m.pid == pid as u16 || m.alt_pids.contains(&(pid as u16)));
            let name = if model.kb.is_some_and(|k| k.jl.is_some()) { Device::open(&found).ok().and_then(|d| crate::kbd::jl_name(&d)) } else { None };
            let named = |m: &&models::Model| name.as_ref().is_none_or(|n| m.kb.and_then(|k| k.jl).is_some_and(|l| l.contains(&n.as_str())));
            let mut cands: Vec<&models::Model> =
                models::MODELS.iter().filter(|m| same(m) && m.kb.is_some_and(|k| k.hs.is_some() || k.jl.is_some() || k.du) && named(m)).collect();
            if cands.is_empty() {
                cands = models::MODELS.iter().filter(|m| same(m) && m.kb.is_some_and(|k| k.jl.is_some())).collect();
            }
            for m in cands {
                out.push(Found { model: m, ..found.clone() });
            }
            continue;
        }
        out.push(found);
    }
    out
}

pub struct Device {
    f: File,
    pub path: PathBuf,
    pub found: Found,
}

fn ioc(nr: u64, size: usize) -> u64 {
    (3u64 << 30) | ((size as u64) << 16) | ((b'H' as u64) << 8) | nr
}

impl Device {
    pub fn open(found: &Found) -> std::io::Result<Device> {
        let f = OpenOptions::new().read(true).write(true).open(&found.hidraw)?;
        Ok(Device { f, path: found.hidraw.clone(), found: found.clone() })
    }

    /// The single plugged mouse, or the one on `usb` (port name) when given.
    pub fn open_first(usb: Option<&str>) -> std::io::Result<Device> {
        let all = scan();
        let f = match usb {
            Some(u) => all.iter().find(|f| f.usb_name() == u),
            None => all.first(),
        };
        Self::open(f.ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "mouse not connected"))?)
    }

    pub(crate) fn set_feature(&self, data: &[u8], size: usize) -> std::io::Result<()> {
        let mut buf = vec![0u8; size];
        buf[..data.len()].copy_from_slice(data);
        let r = unsafe { libc::ioctl(self.f.as_raw_fd(), ioc(0x06, size) as _, buf.as_mut_ptr()) };
        if r < 0 { Err(std::io::Error::last_os_error()) } else { Ok(()) }
    }

    pub(crate) fn get_feature(&self, id: u8, size: usize) -> std::io::Result<Vec<u8>> {
        let mut buf = vec![0u8; size];
        buf[0] = id;
        let r = unsafe { libc::ioctl(self.f.as_raw_fd(), ioc(0x07, size) as _, buf.as_mut_ptr()) };
        if r < 0 { Err(std::io::Error::last_os_error()) } else { Ok(buf) }
    }

    /// An output report through write(2) (CompX mice, compx.rs).
    pub(crate) fn write_report(&self, data: &[u8]) -> std::io::Result<()> {
        use std::io::Write;
        (&self.f).write_all(data)
    }

    /// The next input report, or None after `ms` milliseconds without one.
    pub(crate) fn read_report(&self, ms: i32) -> std::io::Result<Option<Vec<u8>>> {
        use std::io::Read;
        let mut fds = libc::pollfd { fd: self.f.as_raw_fd(), events: libc::POLLIN, revents: 0 };
        if unsafe { libc::poll(&mut fds, 1, ms) } <= 0 {
            return Ok(None);
        }
        let mut buf = vec![0u8; 64];
        let n = (&self.f).read(&mut buf)?;
        buf.truncate(n);
        Ok(Some(buf))
    }

    /// The later builds' write: feature report 4 (256 bytes) carrying up to 248
    /// bytes, [04][f3][addr lo][addr hi][len][0 0 0][data] (RDCfg build 9b650769
    /// FUN_00450550 for a profile block, FUN_0044bab0 for a macro slot).
    pub fn write_bulk(&self, addr: u16, data: &[u8]) -> std::io::Result<()> {
        let mut pkt = vec![4, 0xf3, addr as u8, (addr >> 8) as u8, data.len() as u8, 0, 0, 0];
        pkt.extend_from_slice(&data[..data.len().min(248)]);
        self.set_feature(&pkt, 256)?;
        std::thread::sleep(std::time::Duration::from_millis(50));
        Ok(())
    }

    fn report(n: usize) -> (u8, usize) {
        if n <= 8 { (2, 16) } else { (3, 64) }
    }

    pub fn session(&self, open: bool) -> std::io::Result<()> {
        self.set_feature(&[2, 0xf5, if open { 0 } else { 1 }], 16)
    }

    pub fn commit(&self, mask: u8) -> std::io::Result<()> {
        self.set_feature(&[2, 0xf1, 2, mask], 16)
    }

    pub fn read(&self, mut addr: u16, mut n: usize) -> std::io::Result<Vec<u8>> {
        let mut out = Vec::with_capacity(n);
        while n > 0 {
            let k = n.min(56);
            let (id, size) = Self::report(k);
            let mut ok = None;
            for attempt in 1..=20u64 {
                self.set_feature(&[id, 0xf2, addr as u8, (addr >> 8) as u8, k as u8], size)?;
                std::thread::sleep(std::time::Duration::from_millis(4 * attempt));
                let r = self.get_feature(id, size)?;
                // the reply echoes the even start address; data begins at `addr`
                if r[1] == 0x08 && r[2] == (addr as u8 & 0xfe) && r[3].wrapping_sub(0x60) == (addr >> 8) as u8 {
                    ok = Some(r);
                    break;
                }
            }
            let r = ok.ok_or_else(|| std::io::Error::other(format!("no reply reading {addr:#06x}")))?;
            out.extend_from_slice(&r[8..8 + k]);
            addr += k as u16;
            n -= k;
        }
        Ok(out)
    }

    pub fn write(&self, mut addr: u16, mut data: &[u8]) -> std::io::Result<()> {
        while !data.is_empty() {
            let k = data.len().min(56);
            let (id, size) = Self::report(k);
            let mut pkt = vec![id, 0xf3, addr as u8, (addr >> 8) as u8, k as u8, 0, 0, 0];
            pkt.extend_from_slice(&data[..k]);
            self.set_feature(&pkt, size)?;
            std::thread::sleep(std::time::Duration::from_millis(8));
            addr += k as u16;
            data = &data[k..];
        }
        Ok(())
    }
}

/// Re-enumerates the mouse (USBDEVFS_RESET on its usbfs node, no root needed
/// when the desktop grants seat access). The firmware only adopts a new report
/// rate after re-enumeration, verified with 125/500/1000 Hz on the device.
pub fn usb_reset(found: &Found) -> std::io::Result<()> {
    let p = &found.usb;
    let id = |f: &str| std::fs::read_to_string(p.join(f)).map(|s| s.trim().to_string()).unwrap_or_default();
    let (bus, dev): (u32, u32) = (id("busnum").parse().unwrap_or(0), id("devnum").parse().unwrap_or(0));
    let f = OpenOptions::new().read(true).write(true).open(format!("/dev/bus/usb/{bus:03}/{dev:03}"))?;
    const USBDEVFS_RESET: u64 = 0x5514;
    let r = unsafe { libc::ioctl(f.as_raw_fd(), USBDEVFS_RESET as _, 0) };
    if r < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

// ------------------------------------------------------------------ lighting
//
// Per profile at 0x449 + 8p: [R][G][B][mode][b4][b5][brightness 1-3]
// (verified visually on the device). b4/b5 are kept as the device holds them.

const LED: u16 = 0x449;
/// (mode, variant) the original driver writes for each UI tab: breathing,
/// neon, static, wave, trail, reactive, flash, off (hid.exe FUN_0040f7f0;
/// RDCfg FUN_00456d90 writes the same for its lighting variant 3).
const LED_MODE: [(u8, u8); 8] = [(1, 4), (1, 8), (1, 2), (2, 0), (6, 0), (7, 0), (1, 0x10), (0, 0)];
/// RDCfg FUN_00456d90, lighting variants 0-2 (no trail / reactive).
const LED_MODE_SIX: [(u8, u8); 8] = [(2, 1), (3, 1), (1, 1), (5, 1), (0xff, 0xff), (0xff, 0xff), (4, 1), (0, 0)];
/// Speed byte for low / mid / high (smaller = faster, verified on breathing).
const LED_SPEED: [u8; 3] = [8, 5, 2];

fn led_modes() -> &'static [(u8, u8); 8] {
    if models::current().led == 3 { &LED_MODE } else { &LED_MODE_SIX }
}

fn led_decode(led: &[u8; 7]) -> Option<(usize, u32, u8, u8)> {
    let six = models::current().led != 3;
    let mode = led_modes().iter().position(|&(m, v)| m == led[3] && (if six { m == 0 || v == led[5] } else { m != 1 || v == led[5] }))?;
    let color = (led[0] as u32) << 16 | (led[1] as u32) << 8 | led[2] as u32;
    let speed = match led[4] { 7.. => 0, 4..=6 => 1, _ => 2 };
    Some((mode, color, led[6].clamp(1, 3) - 1, speed))
}

// ------------------------------------------------------------------ macros
//
// 0x478 + 200 * slot: 3-byte events [type][code][hi], zero terminated.
// type 0x84/0x04 key down/up (HID usage), 0x81/0x01 mouse button down/up,
// 0x06 delay in 10 ms units (16-bit LE). Bound with `91 SLOT|FLAGS LOOPS(u16)`:
// FLAGS 0x80 = repeat while held, 0x40 = repeat until pressed again (both with
// LOOPS 0xffff), otherwise play LOOPS times (hid.exe FUN_00413990; verified).

pub const MACRO_BASE: u16 = 0x478;
/// Bytes of a profile block the bulk models write at once (RDCfg's 0xaf).
pub const PROFILE_IMAGE: usize = 0xaf;
pub const MACRO_SLOT: usize = 200;
pub const MACRO_SLOTS: usize = 15;

fn mouse_code(name: &str) -> Option<u8> {
    match name {
        "Mouse Left" => Some(0x01),
        "Mouse Right" => Some(0x02),
        "Mouse Middle" => Some(0x04),
        _ => None,
    }
}

pub fn encode_macro(m: &Macro) -> Option<[u8; MACRO_SLOT]> {
    let mut out = [0u8; MACRO_SLOT];
    let mut n = 0;
    for st in &m.steps {
        let ev = match st.action {
            Action::Delay => {
                let u = ((st.ms + 5) / 10).clamp(1, 65535) as u16;
                [0x06, u as u8, (u >> 8) as u8]
            }
            Action::Down | Action::Up => {
                let down = if st.action == Action::Down { 0x80 } else { 0 };
                match mouse_code(&st.key) {
                    Some(b) => [0x01 | down, b, 0],
                    None => [0x04 | down, key_usage(&st.key)?, 0],
                }
            }
        };
        if n + 3 > MACRO_SLOT {
            return None;
        }
        out[n..n + 3].copy_from_slice(&ev);
        n += 3;
    }
    Some(out)
}

pub fn decode_macro(b: &[u8]) -> Vec<MacroStep> {
    let mut v = Vec::new();
    for e in b.chunks_exact(3) {
        let step = match e[0] {
            0x06 => MacroStep { action: Action::Delay, key: String::new(), ms: (e[1] as u32 | (e[2] as u32) << 8) * 10 },
            0x84 | 0x04 => MacroStep {
                action: if e[0] & 0x80 != 0 { Action::Down } else { Action::Up },
                key: usage_key(e[1]),
                ms: 0,
            },
            0x81 | 0x01 => MacroStep {
                action: if e[0] & 0x80 != 0 { Action::Down } else { Action::Up },
                key: match e[1] { 0x02 => "Mouse Right", 0x04 => "Mouse Middle", _ => "Mouse Left" }.into(),
                ms: 0,
            },
            _ => break,
        };
        v.push(step);
    }
    v
}

/// What the device holds for one profile (raw where the meaning is unknown).
#[derive(Clone, Debug, PartialEq)]
pub struct DeviceProfile {
    pub stage: u8,
    pub stages: [[u8; 5]; 5],
    pub polling_ms: u8,
    /// one per model slot (models.rs), wheel up/down last
    pub buttons: Vec<[u8; 4]>,
    pub led: [u8; 7],
    /// bulk models: the whole profile block as read (`PROFILE_IMAGE` bytes from its
    /// base), rewritten in one piece; empty for the others
    pub image: Vec<u8>,
}

impl DeviceProfile {
    /// `image` with this profile's stage, DPI stages and buttons put in place.
    fn bulk_image(&self, model: &Model) -> Vec<u8> {
        let mut img = self.image.clone();
        img.resize(PROFILE_IMAGE, 0);
        img[2] = self.stage;
        for i in 0..5 {
            img[4 + 6 * i..9 + 6 * i].copy_from_slice(&self.stages[i]);
        }
        for (i, b) in self.buttons.iter().enumerate() {
            let o = model.slot(i).off as usize;
            if o + 4 <= PROFILE_IMAGE {
                img[o..o + 4].copy_from_slice(b);
            }
        }
        img
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DeviceConfig {
    pub active: u8,
    pub profiles: Vec<DeviceProfile>,
    pub macros: Vec<[u8; MACRO_SLOT]>,
}

pub fn read_config(d: &Device) -> std::io::Result<DeviceConfig> {
    let model = d.found.model;
    if model.is_sino() {
        return crate::sino::read_config(d);
    }
    if model.is_compx() {
        return crate::compx::read_config(d);
    }
    if model.is_du() {
        return crate::du::read_config(d);
    }
    if model.is_xy() {
        return crate::xy::read_config(d);
    }
    if model.is_bd() {
        return crate::bd::read_config(d);
    }
    if model.is_mo() {
        return crate::mo::read_config(d);
    }
    if model.is_dc() {
        return crate::dc::read_config(d);
    }
    if model.is_px() {
        return crate::pxi::read_config(d);
    }
    d.session(true)?;
    let r = (|| {
        let active = d.read(ACTIVE_PROFILE, 1)?[0];
        let mut profiles = Vec::new();
        for (p, &base) in PROFILE_BASE.iter().enumerate() {
            let stage = d.read(base + 2, 1)?[0];
            let raw = d.read(base + 4, 30)?;
            let mut stages = [[0u8; 5]; 5];
            for i in 0..5 {
                stages[i].copy_from_slice(&raw[i * 6..i * 6 + 5]);
            }
            let polling_ms = d.read(POLLING + 2 * p as u16, 1)?[0];
            let mut buttons = Vec::new();
            for i in 0..model.slot_count() {
                let mut b = [0u8; 4];
                b.copy_from_slice(&d.read(base + model.slot(i).off, 4)?);
                buttons.push(b);
            }
            let mut led = [0u8; 7];
            led.copy_from_slice(&d.read(LED + 8 * p as u16, 7)?);
            let image = if model.is_bulk() { d.read(base, PROFILE_IMAGE)? } else { Vec::new() };
            profiles.push(DeviceProfile { stage, stages, polling_ms, buttons, led, image });
        }
        let mut macros = Vec::new();
        for s in 0..MACRO_SLOTS {
            let mut m = [0u8; MACRO_SLOT];
            m.copy_from_slice(&d.read(MACRO_BASE + (MACRO_SLOT * s) as u16, MACRO_SLOT)?);
            macros.push(m);
        }
        Ok(DeviceConfig { active, profiles, macros })
    })();
    d.session(false)?;
    r
}

/// Updates the UI profile from what the device holds. Macro buttons resolve
/// to the name of the macro stored in that slot (see `macros_from_device`).
pub fn apply_to_profile_with(dp: &DeviceProfile, p: &mut Profile, macros: &[Macro]) {
    apply_to_profile(dp, p);
    for (i, b) in dp.buttons.iter().enumerate() {
        if b[0] == 0x91 {
            let slot = b[1] & 0x3f;
            let name = macros.iter().find(|m| m.slot == Some(slot)).map(|m| m.name.clone());
            if let Some(b) = p.buttons.get_mut(i) {
                *b = Func::Macro(name.unwrap_or_else(|| format!("M801 #{}", slot as u32 + 1)));
            }
        }
    }
    if let Some((mode, color, bright, speed)) = led_decode(&dp.led) {
        p.light_mode = mode;
        let l = &mut p.lights[mode];
        l.color = color;
        l.brightness = bright;
        l.speed = speed;
    }
}

/// Macros found on the mouse, merged with the app's list (names/options kept
/// for slots the app already knows).
pub fn macros_from_device(cfg: &DeviceConfig, known: &[Macro]) -> Vec<Macro> {
    let mut out: Vec<Macro> = known.iter().filter(|m| m.slot.is_none()).cloned().collect();
    for (s, raw) in cfg.macros.iter().enumerate() {
        let steps = decode_macro(raw);
        if steps.is_empty() {
            continue;
        }
        let mut m = known.iter().find(|m| m.slot == Some(s as u8)).cloned()
            .unwrap_or_else(|| Macro::new(&format!("M801 #{}", s + 1)));
        m.slot = Some(s as u8);
        m.steps = steps;
        // play mode/loops live in the button binding that points at the slot
        if let Some(b) = cfg.profiles.iter().flat_map(|p| p.buttons.iter()).find(|b| b[0] == 0x91 && b[1] & 0x3f == s as u8) {
            let loops = b[2] as u32 | (b[3] as u32) << 8;
            m.mode = if b[1] & 0x80 != 0 { PlayMode::WhileHeld } else if b[1] & 0x40 != 0 { PlayMode::Toggle } else { PlayMode::Loop };
            if m.mode == PlayMode::Loop {
                m.loops = loops.clamp(1, 255);
            }
        }
        out.push(m);
    }
    out.sort_by_key(|m| m.slot.unwrap_or(u8::MAX));
    out
}

/// Updates the UI profile from what the device holds.
pub fn apply_to_profile(dp: &DeviceProfile, p: &mut Profile) {
    let m = models::current();
    p.dpi_stage = (dp.stage as usize).min(4);
    for i in 0..5 {
        let s = dp.stages[i];
        p.dpi_enabled[i] = s[0] != 0;
        p.dpi[i] = m.snap_dpi(m.dpi_decode(s[1] as u16 | (s[2] as u16) << 8) as i64);
    }
    p.polling = polling_decode(dp.polling_ms);
    p.buttons.resize(dp.buttons.len(), Func::Disabled);
    for (i, b) in dp.buttons.iter().enumerate() {
        p.buttons[i] = decode_func(*b).unwrap_or(Func::Raw(*b));
    }
}

/// Like `profile_to_device`, also encoding macro buttons and lighting.
pub fn profile_to_device_with(p: &Profile, current: &DeviceProfile, macros: &[Macro]) -> DeviceProfile {
    let mut d = profile_to_device(p, current);
    // Sinowealth / CompX macros are not supported here
    let n = if models::current().has_wheel_rows() { d.buttons.len().min(p.buttons.len()) } else { 0 };
    for i in 0..n {
        if let Func::Macro(name) = &p.buttons[i] {
            if let Some(m) = macros.iter().find(|m| &m.name == name) {
                if let Some(slot) = m.slot {
                    d.buttons[i] = match m.mode {
                        PlayMode::WhileHeld => [0x91, slot | 0x80, 0xff, 0xff],
                        PlayMode::Toggle => [0x91, slot | 0x40, 0xff, 0xff],
                        PlayMode::Loop => {
                            let n = m.loops.clamp(1, 255) as u16;
                            [0x91, slot, n as u8, (n >> 8) as u8]
                        }
                    };
                }
            }
        }
    }
    let mode = p.light_mode.min(7);
    let l = &p.lights[mode];
    let want = (mode, l.color, l.brightness.min(2), l.speed.min(2));
    let supported = models::current().light_modes().is_some_and(|ms| ms.contains(&mode));
    // keep the device bytes when the UI shows exactly what they mean
    if supported && led_decode(&current.led) != Some(want) {
        let (m, v) = led_modes()[mode];
        d.led = [(l.color >> 16) as u8, (l.color >> 8) as u8, l.color as u8, m, LED_SPEED[want.3 as usize], v, want.2 + 1];
    }
    d
}

/// Assigns free slots to macros that have none and returns the slot images.
pub fn macros_to_device(macros: &mut [Macro], current: &[[u8; MACRO_SLOT]]) -> Vec<[u8; MACRO_SLOT]> {
    let mut out = current.to_vec();
    let mut used: Vec<u8> = macros.iter().filter_map(|m| m.slot).collect();
    for m in macros.iter_mut() {
        if m.slot.is_none() {
            if let Some(free) = (0..MACRO_SLOTS as u8).find(|s| !used.contains(s) && decode_macro(&current[*s as usize]).is_empty()) {
                m.slot = Some(free);
                used.push(free);
            }
        }
        if let Some(s) = m.slot {
            // unchanged macros keep their exact bytes (incl. anything past the terminator)
            if decode_macro(&current[s as usize]) == m.steps {
                continue;
            }
            if let Some(img) = encode_macro(m) {
                out[s as usize] = img;
            }
        }
    }
    out
}

/// Device bytes for a UI profile; functions the firmware encoding is unknown for
/// keep the bytes currently on the device.
pub fn profile_to_device(p: &Profile, current: &DeviceProfile) -> DeviceProfile {
    let model = models::current();
    let mut d = current.clone();
    d.stage = p.dpi_stage as u8;
    for i in 0..5 {
        let v = model.dpi_encode(p.dpi[i]);
        d.stages[i] = [p.dpi_enabled[i] as u8, v as u8, (v >> 8) as u8, v as u8, (v >> 8) as u8];
    }
    d.polling_ms = polling_encode(p.polling);
    for i in 0..d.buttons.len().min(p.buttons.len()) {
        if let Some(b) = encode_func(&p.buttons[i]) {
            d.buttons[i] = b;
        }
    }
    d
}

/// Writes only what differs from `current`, then commits and reads back.
pub fn write_config(d: &Device, new: &DeviceConfig, current: &DeviceConfig) -> std::io::Result<DeviceConfig> {
    let model = d.found.model;
    if model.is_sino() {
        return crate::sino::write_config(d, new, current);
    }
    if model.is_compx() {
        return crate::compx::write_config(d, new, current);
    }
    if model.is_du() {
        return crate::du::write_config(d, new, current);
    }
    if model.is_xy() {
        return crate::xy::write_config(d, new, current);
    }
    if model.is_bd() {
        return crate::bd::write_config(d, new, current);
    }
    if model.is_mo() {
        return crate::mo::write_config(d, new, current);
    }
    if model.is_dc() {
        return crate::dc::write_config(d, new, current);
    }
    if model.is_px() {
        return crate::pxi::write_config(d, new, current);
    }
    let mut new = new.clone();
    if model.is_bulk() {
        for n in new.profiles.iter_mut() {
            n.image = n.bulk_image(model);
        }
    }
    let new = &new;
    d.session(true)?;
    let r = (|| -> std::io::Result<()> {
        let mut mask = 0u8;
        for (p, &base) in PROFILE_BASE.iter().enumerate() {
            let (n, c) = (&new.profiles[p], &current.profiles[p]);
            if model.is_bulk() {
                // these firmwares take a profile block only whole, as RDCfg sends it
                if n.image != c.image {
                    d.write_bulk(base, &n.image)?;
                    mask |= 0x10 | 0x04;
                }
            } else if n.stage != c.stage {
                d.write(base + 2, &[n.stage])?;
                mask |= 0x10;
            }
            for i in 0..5 {
                if !model.is_bulk() && n.stages[i] != c.stages[i] {
                    d.write(base + 4 + 6 * i as u16, &n.stages[i])?;
                    mask |= 0x10;
                }
            }
            if n.polling_ms != c.polling_ms {
                d.write(POLLING + 2 * p as u16, &[n.polling_ms])?;
                mask |= 0x02;
            }
            for i in 0..n.buttons.len().min(c.buttons.len()) {
                if !model.is_bulk() && n.buttons[i] != c.buttons[i] {
                    d.write(base + model.slot(i).off, &n.buttons[i])?;
                    mask |= 0x04;
                }
            }
            if n.led != c.led {
                d.write(LED + 8 * p as u16, &n.led)?;
                mask |= 0x04 | 0x02;
            }
        }
        for (s, img) in new.macros.iter().enumerate() {
            if img != &current.macros[s] {
                if model.is_bulk() {
                    d.write_bulk(MACRO_BASE + (MACRO_SLOT * s) as u16, img)?;
                } else {
                    d.write(MACRO_BASE + (MACRO_SLOT * s) as u16, img)?;
                }
                mask |= 0x08;
            }
        }
        if new.active != current.active {
            d.write(ACTIVE_PROFILE, &[new.active, 0])?;
            mask |= 0x01;
        }
        for bit in [0x02, 0x10, 0x01, 0x04, 0x08] {
            if mask & bit != 0 {
                d.commit(bit)?;
            }
        }
        Ok(())
    })();
    d.session(false)?;
    r?;
    let back = read_config(d)?;
    if &back != new {
        return Err(std::io::Error::other("read-back differs from what was written"));
    }
    Ok(back)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dpi_registers_match_device_captures() {
        assert_eq!(dpi_encode(500), 0x0b);
        assert_eq!(dpi_encode(800), 0x12);
        assert_eq!(dpi_encode(1000), 0x16);
        assert_eq!(dpi_encode(1300), 0x1d);
        assert_eq!(dpi_encode(2000), 0x2d);
        assert_eq!(dpi_encode(3000), 0x43);
        assert_eq!(dpi_encode(6200), 0x8c);
        assert_eq!(dpi_encode(8000), 0x15a);
        assert_eq!(dpi_encode(11800), 0x185);
        for cpi in [500, 800, 1000, 1300, 2000, 3000, 6200, 8000, 11800, 12400] {
            assert_eq!(dpi_decode(dpi_encode(cpi)), cpi, "{cpi}");
        }
    }

    #[test]
    fn button_codes_round_trip() {
        for f in [Func::Click, Func::Menu, Func::Middle, Func::Back, Func::Forward, Func::Key("J".into()),
            Func::Key("7".into()), Func::Media(5), Func::Dpi(1), Func::Dpi(2), Func::Dpi(3), Func::ProfileSwitch,
            Func::ScrollUp, Func::ScrollDown, Func::Disabled] {
            assert_eq!(decode_func(encode_func(&f).unwrap()).unwrap(), f);
        }
        // values read from the user's mouse
        assert_eq!(decode_func([0x90, 0, 0x24, 0]), Some(Func::Key("7".into())));
        assert_eq!(decode_func([0x90, 0, 0x0d, 0]), Some(Func::Key("J".into())));
        assert_eq!(decode_func([0x90, 0, 0xe2, 0]), Some(Func::Key("LAlt".into())));
        assert_eq!(decode_func([0x91, 0, 1, 0]), None); // macro: resolved by apply_to_profile_with
    }

    #[test]
    fn original_table_codes() {
        assert_eq!(encode_func(&Func::Combo("CTRL+SHIFT+Escape".into())), Some([0x8f, 0x03, 0x29, 0]));
        assert_eq!(encode_func(&Func::Basic(2)), Some([0x8f, 0x01, 0x06, 0]));
        assert_eq!(encode_func(&Func::Advanced(1)), Some([0x8f, 0x04, 0x2b, 0]));
        assert_eq!(encode_func(&Func::Advanced(7)), Some([0x8e, 0x01, 0x96, 0x01]));
        assert_eq!(encode_func(&Func::Advanced(14)), Some([0x8e, 0x01, 0x8a, 0x01]));
        // the original driver's software-only codes still read back
        assert_eq!(decode_func([0x8e, 0x01, 0xff, 0x12]), Some(Func::Basic(2)));
        assert_eq!(decode_func([0x8e, 0x01, 0xff, 0x1f]), Some(Func::Advanced(7)));
        assert_eq!(encode_func(&Func::Media(1)), Some([0x8e, 0x01, 0xcd, 0]));
        assert_eq!(encode_func(&Func::Rate(2)), Some([0x98, 0, 0, 0]));
        assert_eq!(encode_func(&Func::LightSwitch), Some([0x9b, 0x04, 0, 0]));
        let mid = FireKey { target: 2, key: String::new(), times: 3, delay_ms: 10 };
        assert_eq!(encode_func(&Func::FireKey(mid)), Some([0x99, 0x84, 3, 1]));
        for f in [Func::Combo("CTRL+SHIFT+C".into()), Func::Basic(8), Func::Advanced(7), Func::Media(7), Func::Rate(1), Func::LightSwitch] {
            assert_eq!(decode_func(encode_func(&f).unwrap()), Some(f));
        }
        // the user's profile 4 button 4 (`99 01 90 00`) stays raw
        assert_eq!(decode_func([0x99, 0x01, 0x90, 0]), None);
    }

    #[test]
    fn lighting_and_macro_flags() {
        let mut p = Profile::default();
        p.light_mode = 2; // static
        p.lights[2].color = 0xff0000;
        p.lights[2].brightness = 2;
        let cur = DeviceProfile { stage: 0, stages: [[0; 5]; 5], polling_ms: 1, buttons: vec![[0; 4]; 11], led: [0, 0xff, 0xff, 2, 8, 0, 3], image: Vec::new() };
        assert_eq!(profile_to_device_with(&p, &cur, &[]).led, [0xff, 0, 0, 1, 8, 2, 3]);
        let mut m = Macro::new("m");
        m.slot = Some(5);
        m.mode = PlayMode::WhileHeld;
        p.buttons[8] = Func::Macro("m".into());
        assert_eq!(profile_to_device_with(&p, &cur, &[m.clone()]).buttons[8], [0x91, 0x85, 0xff, 0xff]);
        m.mode = PlayMode::Toggle;
        assert_eq!(profile_to_device_with(&p, &cur, &[m.clone()]).buttons[8], [0x91, 0x45, 0xff, 0xff]);
        m.mode = PlayMode::Loop;
        m.loops = 3;
        assert_eq!(profile_to_device_with(&p, &cur, &[m]).buttons[8], [0x91, 0x05, 3, 0]);
    }

    #[test]
    fn bulk_profile_image_matches_rdcfg_packet() {
        // M718-AIR build (9b650769) FUN_00450550, run in EmuCall: stage 2 selected,
        // stage 0 = [7, 0x11, 0, 0x22, 0], buttons from +0x42, wheel at +0x9a
        let m = models::by_key("m718-air").unwrap();
        assert!(m.is_bulk());
        let mut img = vec![0u8; PROFILE_IMAGE];
        img[0] = 5;
        let p = DeviceProfile {
            stage: 2,
            stages: [[7, 0x11, 0, 0x22, 0], [1, 0x33, 0, 0x44, 0], [1, 0x55, 0, 0x66, 0], [1, 0x77, 0, 0x48, 1], [1, 0x59, 1, 0x6a, 1]],
            polling_ms: 1,
            buttons: (0..m.slot_count() as u8).map(|i| [1, i, 0, 0]).collect(),
            led: [0; 7],
            image: img,
        };
        let out = p.bulk_image(m);
        assert_eq!(&out[..34], &[5, 0, 2, 0, 7, 0x11, 0, 0x22, 0, 0, 1, 0x33, 0, 0x44, 0, 0, 1, 0x55, 0, 0x66, 0, 0,
            1, 0x77, 0, 0x48, 1, 0, 1, 0x59, 1, 0x6a, 1, 0]);
        assert_eq!(&out[0x42..0x4a], &[1, 0, 0, 0, 1, 1, 0, 0]);
        assert_eq!(&out[0x9a..0xa2], &[1, 7, 0, 0, 1, 8, 0, 0]);
    }

    #[test]
    fn macro_slot_round_trip_matches_device_bytes() {
        // the user's macro in slot 1 (B down, 10 ms, B up, ...), as read from the mouse
        let raw = [0x84, 0x05, 0x00, 0x06, 0x01, 0x00, 0x04, 0x05, 0x00, 0x06, 0x01, 0x00, 0x84, 0x29, 0x00];
        let steps = decode_macro(&raw);
        assert_eq!(steps.len(), 5);
        assert_eq!(steps[0], MacroStep { action: Action::Down, key: "B".into(), ms: 0 });
        assert_eq!(steps[1].ms, 10);
        assert_eq!(steps[4].key, "Escape");
        let mut m = Macro::new("x");
        m.steps = steps;
        let enc = encode_macro(&m).unwrap();
        assert_eq!(&enc[..raw.len()], &raw[..]);
        assert!(enc[raw.len()..].iter().all(|b| *b == 0));
        // F13 + 500 ms, verified on the device (held 512 ms)
        m.steps = vec![
            MacroStep { action: Action::Down, key: "F13".into(), ms: 0 },
            MacroStep { action: Action::Delay, key: String::new(), ms: 500 },
            MacroStep { action: Action::Up, key: "F13".into(), ms: 0 },
        ];
        assert_eq!(&encode_macro(&m).unwrap()[..9], &[0x84, 0x68, 0, 0x06, 50, 0, 0x04, 0x68, 0]);
    }
}
