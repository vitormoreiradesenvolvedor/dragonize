//! libdc mice (Qt + CEF app qcef.exe with an HTML UI, device access libdc.dll; M805 "FRC-ZYB",
//! M906 "FRC-ZMA", both 04d9:b0d2; decompile ghidra/out/libdc.c, tools/re/fleet/NOTES.md
//! "libdc"). Write-only: the UI keeps its profiles in view/xml/configuration.xml and only sends.
//! dcWriteProp (FUN_00500df8 -> FUN_00501584): feature report, id by size (2 = 16 bytes,
//! 3 = 64, 4 = 256, 5 = 1024, 6 = 2048, FUN_00501474 / FUN_005014c0):
//!   RID 06 bb aa ID_lo ID_hi LEN_lo LEN_hi DATA[LEN]
//! Property ids (view/xml/driver.xml): 4 current mode, 40 report rate (8 / 4 / 2 / 1 = 125 / 250
//! / 500 / 1000 Hz), 49 DPI count, 50 current DPI, 52 + i stage i (8 bytes: enabled, x lo, x hi,
//! y lo, y hi; value = ceil(cpi / 50)), 62 + n button n (8 bytes: 4 function bytes, play type,
//! count, delay lo, delay hi). The "p" properties act on the current mode.

use crate::device::{Device, DeviceConfig, DeviceProfile, MACRO_SLOT, MACRO_SLOTS};
use crate::model::Func;

const STAGES: usize = 5;
const MODE: u16 = 4;
const RATE: u16 = 40;
const DPI_COUNT: u16 = 49;
const DPI_CURRENT: u16 = 50;
const DPI: u16 = 52;
const BUTTON: u16 = 62;

/// driver.xml function codes.
const MOUSE: [(Func, [u8; 2]); 9] = [
    (Func::Click, [1, 0x01]),
    (Func::Menu, [1, 0x02]),
    (Func::Middle, [1, 0x04]),
    (Func::Back, [1, 0x08]),
    (Func::Forward, [1, 0x10]),
    (Func::ScrollUp, [1, 0x11]),
    (Func::ScrollDown, [1, 0x12]),
    (Func::ProfileSwitch, [1, 0x26]),
    (Func::LightSwitch, [1, 0xa0]),
];

/// Function bytes: [1, f, 0, 0] mouse function (0x20 DPI cycle, 0x21 DPI +, 0x22 DPI -),
/// [2, mods, usage, 0] key, [3, 0, usage lo, usage hi] consumer, [4, ..] macro, 0 disabled.
pub fn encode_func(f: &Func) -> Option<[u8; 4]> {
    if let Some(c) = MOUSE.iter().find(|m| &m.0 == f).map(|m| m.1) {
        return Some([c[0], c[1], 0, 0]);
    }
    Some(match f {
        Func::Dpi(n @ 1..=3) => [1, 0x1f + n, 0, 0],
        Func::Media(i) => {
            let u = *crate::du::MEDIA.get((*i as usize).checked_sub(1)?)?;
            [3, 0, u as u8, (u >> 8) as u8]
        }
        Func::Key(k) => match crate::device::key_usage(k)? {
            // a lone modifier = the hot key's modifier bits (CTRL 1, SHIFT 2, ALT 4, WIN 8)
            u @ 0xe0..=0xe3 => [2, 1 << (u - 0xe0), 0, 0],
            u => [2, 0, u, 0],
        },
        Func::Disabled => [0, 0, 0, 0],
        Func::Raw(b) => *b,
        _ => return None,
    })
}

pub fn decode_func(b: [u8; 4]) -> Option<Func> {
    if let Some(m) = MOUSE.iter().find(|m| m.1 == [b[0], b[1]]) {
        return Some(m.0.clone());
    }
    Some(match b {
        [1, n @ 0x20..=0x22, ..] => Func::Dpi(n - 0x1f),
        [3, 0, lo, hi] => {
            let u = lo as u16 | (hi as u16) << 8;
            Func::Media(crate::du::MEDIA.iter().position(|&m| m == u)? as u8 + 1)
        }
        [2, 0, k, 0] if k != 0 => Func::Key(crate::device::usage_key(k)),
        [2, m @ (1 | 2 | 4 | 8), 0, 0] => Func::Key(crate::device::usage_key(0xe0 + m.trailing_zeros() as u8)),
        [0, 0, 0, 0] => Func::Disabled,
        _ => return None,
    })
}

/// The feature report that sets property `id` to `data`.
fn prop(id: u16, data: &[u8]) -> Vec<u8> {
    let n = 8 + data.len();
    let (rid, size) = [(2, 0x10), (3, 0x40), (4, 0x100), (5, 0x400), (6, 0x800)].into_iter().find(|&(_, s)| n <= s).unwrap_or((6, 0x800));
    let mut r = vec![rid, 6, 0xbb, 0xaa, id as u8, (id >> 8) as u8, data.len() as u8, (data.len() >> 8) as u8];
    r.extend_from_slice(data);
    r.resize(size, 0);
    r
}

fn blank(model: &crate::models::Model) -> DeviceConfig {
    let buttons = model.defaults.iter().map(|&c| c.to_le_bytes()).collect();
    let p = DeviceProfile { stage: 0xff, stages: [[0; 5]; 5], polling_ms: 0, buttons, led: [0; 7], image: Vec::new() };
    DeviceConfig { active: 0, profiles: vec![p; 5], macros: vec![[0u8; MACRO_SLOT]; MACRO_SLOTS] }
}

pub fn read_config(d: &Device) -> std::io::Result<DeviceConfig> {
    Ok(blank(d.found.model))
}

/// The reports that turn `c` into `n` (profile `mode` of the UI = the mouse's mode).
pub fn packets(model: &crate::models::Model, mode: u8, n: &DeviceProfile, c: &DeviceProfile) -> Vec<Vec<u8>> {
    let mut out = vec![prop(MODE, &[mode])];
    if n.polling_ms != c.polling_ms {
        let rate = match n.polling_ms { 8 => 8, 4 => 4, 2 => 2, _ => 1 };
        out.push(prop(RATE, &[rate]));
    }
    if n.stages != c.stages {
        out.push(prop(DPI_COUNT, &[STAGES as u8]));
        for (i, s) in n.stages.iter().enumerate() {
            out.push(prop(DPI + i as u16, &[(s[0] != 0) as u8, s[1], s[2], s[1], s[2], 0, 0, 0]));
        }
    }
    if n.stages != c.stages || n.stage != c.stage {
        out.push(prop(DPI_CURRENT, &[(n.stage as usize).min(STAGES - 1) as u8]));
    }
    for (i, (a, b)) in n.buttons.iter().zip(&c.buttons).enumerate() {
        if a != b {
            // play type 0 (once), count 1, no delay
            out.push(prop(BUTTON + model.slot(i).off, &[a[0], a[1], a[2], a[3], 0, 1, 0, 0]));
        }
    }
    out
}

pub fn write_config(d: &Device, new: &DeviceConfig, current: &DeviceConfig) -> std::io::Result<DeviceConfig> {
    let a = new.active as usize % 5;
    let (n, c) = (&new.profiles[a], &current.profiles[a]);
    for r in packets(d.found.model, a as u8, n, c) {
        d.set_feature(&r, r.len())?;
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let mut back = new.clone();
    back.profiles[a] = n.clone();
    Ok(back)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packets_follow_the_driver() {
        assert_eq!(encode_func(&Func::Forward), Some([1, 0x10, 0, 0]));
        assert_eq!(encode_func(&Func::Dpi(2)), Some([1, 0x21, 0, 0]));
        assert_eq!(decode_func([2, 4, 0, 0]).and_then(|f| encode_func(&f)), Some([2, 4, 0, 0]));
        assert_eq!(decode_func([3, 0, 0xe9, 0]).and_then(|f| encode_func(&f)), Some([3, 0, 0xe9, 0]));
        let r = prop(BUTTON + 3, &[1, 0x21, 0, 0, 0, 1, 0, 0]);
        assert_eq!(r.len(), 16);
        assert_eq!(&r[..10], &[2, 6, 0xbb, 0xaa, 65, 0, 8, 0, 1, 0x21]);
        // 300 cpi -> ceil(300 / 50) = 6
        assert_eq!(&prop(DPI, &[1, 6, 0, 6, 0, 0, 0, 0])[4..13], &[52, 0, 8, 0, 1, 6, 0, 6, 0]);
    }
}
