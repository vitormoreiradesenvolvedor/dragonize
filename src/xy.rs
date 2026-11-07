//! xyui mice (DeviceDriver.exe + XYUI.dll, USB 1a2c:xxxx wired / 1a2c:8fff dongle; M821-AIR
//! build decompiled, tools/re/fleet/NOTES.md "xyui family"). The driver keeps the settings in
//! its own database and only sends them: nothing is read back. Interface usage page 0xff00,
//! usage 1; 64-byte output reports without a report id (FUN_0044d380):
//!   wired (config.xml type 601..700)  08 CMD ..
//!   2.4 GHz (type 701..)              bb aa 99 88 09 CMD ..
//! Commands: f9 DPI group `index, colour, cpi / 100 - 1` (FUN_004251a0, table DAT_005671c0 =
//! 100..12800), f8 report rate 0..3 = 125 / 250 / 500 / 1000 Hz (FUN_0042f9e0, radios
//! 0x20d..0x210), f6 key `profile, key_value, func_value` (FUN_00426fa0; the function type is
//! not sent), f7 profile, fc lighting, fb reset, f0 macro blocks.
//! Function values (FUN_00426730): d0 left, d1 right, d2 middle, d3 forward, d4 back, d5 DPI+,
//! d6 DPI-, d8 DPI cycle; e8..f9 consumer functions (the xyui keyboards' table, kbd.rs);
//! other values are HID key usages. The layouts' defaults use 1 left, 2 middle, 3 right,
//! 5 forward, 6 back (type 1). Profile = the driver's database row id, 1 for its first profile
//! (assumed: the mouse's own profile numbering is not in the driver). Lighting not sent.

use crate::device::{Device, DeviceConfig, DeviceProfile, MACRO_SLOT, MACRO_SLOTS};
use crate::model::Func;

/// The 2.4 GHz dongle every xyui mouse of the fleet pairs with.
pub const DONGLE: u16 = 0x8fff;
const PROFILE: u8 = 1;

/// Consumer usage of the app's media functions (du.rs order) -> xyui function value.
const MEDIA: [(u16, u8); 7] = [(0xcd, 0xee), (0xb7, 0xec), (0xb6, 0xed), (0xb5, 0xef), (0xe9, 0xea), (0xea, 0xe9), (0xe2, 0xeb)];

/// The 65 bytes written for `payload` (CMD ..): no report id, then the wired / dongle prefix.
pub fn packet(dongle: bool, payload: &[u8]) -> Vec<u8> {
    let mut p = vec![0u8; 65];
    let head: &[u8] = if dongle { &[0xbb, 0xaa, 0x99, 0x88, 0x09] } else { &[0x08] };
    p[1..1 + head.len()].copy_from_slice(head);
    p[1 + head.len()..1 + head.len() + payload.len()].copy_from_slice(payload);
    p
}

pub fn encode_func(f: &Func) -> Option<[u8; 4]> {
    let v = match f {
        Func::Click => 0xd0,
        Func::Menu => 0xd1,
        Func::Middle => 0xd2,
        Func::Forward => 0xd3,
        Func::Back => 0xd4,
        Func::Dpi(1) => 0xd8,
        Func::Dpi(2) => 0xd5,
        Func::Dpi(3) => 0xd6,
        Func::Key(k) => crate::device::key_usage(k)?,
        Func::Media(i) => {
            let u = *crate::du::MEDIA.get((*i as usize).checked_sub(1)?)?;
            MEDIA.iter().find(|m| m.0 == u)?.1
        }
        Func::Raw(b) => return Some(*b),
        _ => return None,
    };
    Some([v, 0, 0, 0])
}

pub fn decode_func(b: [u8; 4]) -> Option<Func> {
    Some(match b[0] {
        0xd0 | 1 => Func::Click,
        0xd1 | 3 => Func::Menu,
        0xd2 | 2 => Func::Middle,
        0xd3 | 5 => Func::Forward,
        0xd4 | 6 => Func::Back,
        0xd8 => Func::Dpi(1),
        0xd5 => Func::Dpi(2),
        0xd6 => Func::Dpi(3),
        v if v >= 0xe8 => {
            let u = MEDIA.iter().find(|m| m.1 == v)?.0;
            Func::Media(crate::du::MEDIA.iter().position(|&m| m == u)? as u8 + 1)
        }
        v if (4..0xe8).contains(&v) => Func::Key(crate::device::usage_key(v)),
        _ => return None,
    })
}

/// What the app assumes the mouse holds: the layout's default keys; DPI and rate unknown
/// (always sent).
fn blank(model: &crate::models::Model) -> DeviceConfig {
    let buttons = model.defaults.iter().map(|&c| c.to_le_bytes()).collect();
    let p = DeviceProfile { stage: 0xff, stages: [[0; 5]; 5], polling_ms: 0, buttons, led: [0; 7], image: Vec::new() };
    DeviceConfig { active: 0, profiles: vec![p; 5], macros: vec![[0u8; MACRO_SLOT]; MACRO_SLOTS] }
}

pub fn read_config(d: &Device) -> std::io::Result<DeviceConfig> {
    Ok(blank(d.found.model))
}

/// The packets that turn `c` into `n` (one profile: the UI's current one).
pub fn packets(model: &crate::models::Model, n: &DeviceProfile, c: &DeviceProfile) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    for (i, (a, b)) in n.buttons.iter().zip(&c.buttons).enumerate() {
        if a != b {
            out.push(vec![0xf6, PROFILE, model.slot(i).off as u8, a[0]]);
        }
    }
    if n.polling_ms != c.polling_ms {
        out.push(vec![0xf8, [8, 4, 2, 1].iter().position(|&m| m == n.polling_ms).unwrap_or(3) as u8]);
    }
    if n.stages != c.stages || n.stage != c.stage {
        // each f9 also selects its group: the current stage goes last
        let cur = (n.stage as usize).min(4);
        for i in (0..5).filter(|&i| i != cur).chain([cur]) {
            out.push(vec![0xf9, i as u8, i as u8, n.stages[i][1]]);
        }
    }
    out
}

pub fn write_config(d: &Device, new: &DeviceConfig, current: &DeviceConfig) -> std::io::Result<DeviceConfig> {
    let pid = std::fs::read_to_string(d.found.usb.join("idProduct")).ok().and_then(|s| u16::from_str_radix(s.trim(), 16).ok());
    let dongle = pid == Some(DONGLE);
    let (n, c) = (&new.profiles[new.active as usize % 5], &current.profiles[new.active as usize % 5]);
    for p in packets(d.found.model, n, c) {
        d.write_report(&packet(dongle, &p))?;
        // the driver waits 100 ms after every command
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let mut back = new.clone();
    for p in back.profiles.iter_mut() {
        *p = n.clone();
    }
    Ok(back)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packets_follow_the_driver() {
        assert_eq!(&packet(false, &[0xf8, 3])[..4], &[0, 0x08, 0xf8, 3]);
        assert_eq!(&packet(true, &[0xf9, 1, 1, 15])[..10], &[0, 0xbb, 0xaa, 0x99, 0x88, 0x09, 0xf9, 1, 1, 15]);
        assert_eq!(encode_func(&Func::Back), Some([0xd4, 0, 0, 0]));
        assert_eq!(decode_func([6, 0, 0, 0]), Some(Func::Back));
        assert_eq!(encode_func(&Func::Dpi(1)), Some([0xd8, 0, 0, 0]));
        // mute: consumer 0xe2 = xyui 0xeb
        let m = Func::Media(7);
        assert_eq!(encode_func(&m), Some([0xeb, 0, 0, 0]));
        assert_eq!(decode_func([0xeb, 0, 0, 0]), Some(m));
        assert_eq!(decode_func([0x04, 0, 0, 0]), Some(Func::Key(crate::device::usage_key(4))));
    }
}
