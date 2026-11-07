//! MosArt wireless mice of the C++Builder / JVCL drivers (M690 "2645", M688 "2613", M691; all
//! 062a:415e; decompiles ghidra/out/mosart-m690.c / -m688.c / -m691.c, tools/re/fleet/NOTES.md "MosArt"). Write-only: the driver keeps
//! everything in its own files and never reads the mouse back. Output report 5, then the mouse
//! answers with an input report (FUN_004080b0: `05 CMD P1..P5`, rest zero):
//!   b2 i R G B L   stage i (0..5): colour 0..9 per channel, level code (FUN_004082cc)
//!   31 RATE LIGHT SPEED 0 0   report rate 1 / 2 / 4 = 125 / 250 / 500 Hz, lighting (FUN_00408358)
//!   53 k ff a5 C 0  key k: function code C (mouse buttons, media)
//!   53 k 0 0 0 0    key k: its own function (DPI keys, fire key)
//!   51 o d0..d3     key record bytes at o (4 at a time, from byte 8 through table 0x62ab09)
//!   52 80 0 0 0 0   end of a key record (FUN_00408398)
//! Apply (FUN_00408744): the 6 stages, 31, the 13 keys, then b2 of the current stage again.

use crate::device::{Device, DeviceConfig, DeviceProfile, MACRO_SLOT, MACRO_SLOTS};
use crate::model::Func;

const ID: u8 = 5;
const STAGES: usize = 6;
/// Default stage colours (0..9 per channel) and levels (the driver's default profile).
const COLOURS: [[u8; 3]; STAGES] = [[9, 0, 0], [9, 4, 0], [9, 9, 0], [0, 9, 0], [0, 0, 9], [0, 9, 9]];
/// Level index -> wire code (DAT_0062b618); the levels are 800 / 1200 / 1600 / 2400 / 3200 / 4800.
const LEVEL: [u8; STAGES] = [5, 4, 0, 1, 2, 3];
/// Lighting of the default profile: breathing, speed 2 (31 bytes 2-3).
const LIGHT: [u8; 2] = [0x04, 0x20];

/// Function codes after table 0x62ab09 (mouse buttons, then the app's media order).
const MOUSE: [(Func, u8); 5] = [(Func::Click, 0xb5), (Func::Menu, 0xb6), (Func::Middle, 0xb7), (Func::Back, 0xb8), (Func::Forward, 0xb9)];
/// Consumer usages of the app's media functions -> code (play, stop, prev, next, vol+, vol-, mute).
const MEDIA: [(u16, u8); 7] = [(0xcd, 0xa1), (0xb7, 0xa2), (0xb6, 0xa3), (0xb5, 0xa4), (0xe9, 0xa0), (0xea, 0x9f), (0xe2, 0x9e)];

/// Keyboard keys some keys have as their own function (M688 / M691: [0, 5..8, 0, 0]).
const NATIVE_KEYS: [u8; 4] = [0xe2, 0xe0, 0xe1, 0x4a];

/// Key bytes kept in the profile: [1, code, 0, 0] function code, [0, n, 0, 0] the key's own
/// function (n: 1 DPI cycle, 2 DPI +, 3 DPI -, 4 fire key, 5 ALT, 6 CTRL, 7 SHIFT, 8 HOME;
/// keyboard keys are not encodable, the key keeps its bytes), [2, 0, 0, 0] disabled.
pub fn encode_func(f: &Func) -> Option<[u8; 4]> {
    if let Some(c) = MOUSE.iter().find(|m| &m.0 == f).map(|m| m.1) {
        return Some([1, c, 0, 0]);
    }
    Some(match f {
        Func::Media(i) => {
            let u = *crate::du::MEDIA.get((*i as usize).checked_sub(1)?)?;
            [1, MEDIA.iter().find(|m| m.0 == u)?.1, 0, 0]
        }
        Func::Dpi(n @ 1..=3) => [0, *n, 0, 0],
        Func::FireKey(_) => [0, 4, 0, 0],
        Func::Disabled => [2, 0, 0, 0],
        Func::Raw(b) => *b,
        _ => return None,
    })
}

pub fn decode_func(b: [u8; 4]) -> Option<Func> {
    Some(match b {
        [1, c, ..] => {
            if let Some(m) = MOUSE.iter().find(|m| m.1 == c) {
                return Some(m.0.clone());
            }
            let u = MEDIA.iter().find(|m| m.1 == c)?.0;
            Func::Media(crate::du::MEDIA.iter().position(|&m| m == u)? as u8 + 1)
        }
        [0, n @ 1..=3, ..] => Func::Dpi(n),
        [0, 4, ..] => Func::FireKey(crate::model::FireKey::default()),
        [0, n @ 5..=8, ..] => Func::Key(crate::device::usage_key(NATIVE_KEYS[n as usize - 5])),
        [2, ..] => Func::Disabled,
        _ => return None,
    })
}

/// The commands for key `k` set to `b`.
fn key_packets(k: u8, b: [u8; 4]) -> Vec<[u8; 6]> {
    match b {
        [1, c, ..] => vec![[0x53, k, 0xff, 0xa5, c, 0]],
        // the "Disable" record (DAT_0062aa83 + 0x33): type 8, one data byte 0
        [2, ..] => {
            let rec = [k, 1, 1 ^ 0x5a, 0, 0, 0, 0, 0, 0];
            let mut out: Vec<[u8; 6]> = rec.chunks(4).enumerate().map(|(i, c)| {
                let mut p = [0x51, 4 * i as u8, 0, 0, 0, 0];
                p[2..2 + c.len()].copy_from_slice(c);
                p
            }).collect();
            out.push([0x52, 0x80, 0, 0, 0, 0]);
            out
        }
        _ => vec![[0x53, k, 0, 0, 0, 0]],
    }
}

fn blank(model: &crate::models::Model) -> DeviceConfig {
    let buttons = model.defaults.iter().map(|&c| c.to_le_bytes()).collect();
    let p = DeviceProfile { stage: 0xff, stages: [[0; 5]; 5], polling_ms: 0, buttons, led: [0; 7], image: Vec::new() };
    DeviceConfig { active: 0, profiles: vec![p; 5], macros: vec![[0u8; MACRO_SLOT]; MACRO_SLOTS] }
}

pub fn read_config(d: &Device) -> std::io::Result<DeviceConfig> {
    Ok(blank(d.found.model))
}

/// The commands that turn `c` into `n` (the UI's current profile), in the driver's order.
pub fn packets(model: &crate::models::Model, n: &DeviceProfile, c: &DeviceProfile) -> Vec<[u8; 6]> {
    let mut out = Vec::new();
    let stages_changed = n.stages != c.stages || n.stage != c.stage;
    if stages_changed {
        // stages 0..4 from the UI, the sixth keeps its default level
        for i in 0..STAGES {
            let code = if i < 5 { n.stages[i][1] } else { LEVEL[5] };
            let [r, g, b] = COLOURS[i];
            out.push([0xb2, i as u8, r, g, b, code]);
        }
    }
    if n.polling_ms != c.polling_ms {
        let rate = match n.polling_ms { 8 => 1, 4 => 2, 2 => 4, _ => 8 };
        out.push([0x31, rate, LIGHT[0], LIGHT[1], 0, 0]);
    }
    for (i, (a, b)) in n.buttons.iter().zip(&c.buttons).enumerate() {
        if a != b {
            out.extend(key_packets(model.slot(i).off as u8, *a));
        }
    }
    if stages_changed {
        let i = (n.stage as usize).min(4);
        let [r, g, b] = COLOURS[i];
        out.push([0xb2, i as u8, r, g, b, n.stages[i][1]]);
    }
    out
}

/// Size of output report 5 in the interface's report descriptor (8 when not found).
fn report_len(d: &Device) -> usize {
    let name = d.found.hidraw.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
    let rd = std::fs::read(format!("/sys/class/hidraw/{name}/device/report_descriptor")).unwrap_or_default();
    let (mut i, mut id, mut size, mut count, mut bits) = (0, 0u32, 0u32, 0u32, 0u32);
    while i < rd.len() {
        let p = rd[i];
        let n = [0, 1, 2, 4][(p & 3) as usize];
        let v = rd.get(i + 1..i + 1 + n).map(|b| b.iter().rev().fold(0u32, |a, &x| a << 8 | x as u32)).unwrap_or(0);
        match p & 0xfc {
            0x84 => id = v,
            0x74 => size = v,
            0x94 => count = v,
            0x90 if id == ID as u32 => bits += size * count,
            _ => {}
        }
        i += 1 + n;
    }
    if bits == 0 { 8 } else { 1 + (bits as usize).div_ceil(8) }
}

pub fn write_config(d: &Device, new: &DeviceConfig, current: &DeviceConfig) -> std::io::Result<DeviceConfig> {
    let len = report_len(d).max(7);
    let (n, c) = (&new.profiles[new.active as usize % 5], &current.profiles[new.active as usize % 5]);
    for p in packets(d.found.model, n, c) {
        let mut r = vec![0u8; len];
        r[0] = ID;
        r[1..7].copy_from_slice(&p);
        d.write_report(&r)?;
        // the driver waits for the mouse's answer (up to 3 s) before the next command
        let _ = d.read_report(300)?;
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
        assert_eq!(encode_func(&Func::Back), Some([1, 0xb8, 0, 0]));
        assert_eq!(decode_func([1, 0x9e, 0, 0]), Some(Func::Media(7)));
        assert_eq!(key_packets(10, [1, 0xb9, 0, 0]), vec![[0x53, 10, 0xff, 0xa5, 0xb9, 0]]);
        assert_eq!(key_packets(3, [0, 3, 0, 0]), vec![[0x53, 3, 0, 0, 0, 0]]);
        assert_eq!(decode_func([0, 8, 0, 0]), Some(Func::Key(crate::device::usage_key(0x4a))));
        let d = key_packets(9, [2, 0, 0, 0]);
        assert_eq!(d.len(), 4);
        assert_eq!(d[0], [0x51, 0, 9, 1, 0x5b, 0]);
        assert_eq!(d[3], [0x52, 0x80, 0, 0, 0, 0]);
    }
}
