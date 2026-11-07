//! PixArt mice of the Qt drivers with PXICtrl3169.dll (M709-1, 093a:2534; decompile
//! ghidra/out/pxi3169.c, tools/re/fleet/NOTES.md "PixArt"). The mouse runs from a 0x2000-byte
//! EEPROM that holds register writes as (register, value) byte pairs: the driver reads all of
//! it, rebuilds it from its JSON (FUN_1001ae70) and writes it back. This app reads the header
//! and the three profile blocks and rewrites only the 8-byte rows whose values changed.
//! Access: 12-byte feature report 5 `05 OP BANK_lo BANK_hi REG(4, LE) VALUE(4, LE)`, OP 1 write,
//! 0 read (then GET feature 5, value from byte 8; then `05 01 00..` releases the bus).
//! EEPROM (FUN_10015940 / FUN_10015a50 around the transfer, bank 2 registers):
//!   open: 2e = 0, 28 &= 0x83, 34 &= ~1, 16 = 0x1a      close: 16 = 0x1e, 34 |= 1
//!   read 8 at A (FUN_100155f0): 36 = A lo, 37 = A hi, 33 = 0x10, 33 = 0x18, then
//!     `05 00 06 00 40 ..` and GET: the 8 bytes from byte 4
//!   write 8 at A (FUN_100157b0): 36, 37 as above, `05 01 06 D7 D0 D1 D2 D3 D4 D5 D6 00`,
//!     33 = 0x10, 33 = 0x14
//! [4..8] = XOR of the u32 words 0x100..0x2000. Header: [0xb] & 3 = active profile. Profile p
//! at 0x300 + 0x100 p, values at odd addresses (FUN_1001bce0 / FUN_1001ae70):
//!   DPI i = (hi << 4 | lo) * 100 + 100, lo = the low nibble of 0x73 + 2 i, hi 3 bits (see `HI`),
//!   its colour index in the high nibble; [0x7f] >> 6 = DPI count - 3; [0xa3] & 7 default
//!   DPI (also at 0xc3); [0xad] bits 3-4 = report rate ^ 3 (also at 0xcd).
//!   Buttons 3..10: 13-bit codes, LSB first, in the values of 0x33, 0x35 .. 0x45, 0x4b, 0x4d,
//!   0x4f; button 11 in 0x47 and 0x49 (buttons 0-2 are fixed). Code (the driver's maps
//!   DAT_100cc0d0..e8): mods << 8 | HID usage, 0x1000 | consumer usage, 0x18xx mouse functions;
//!   0 = a macro when a macro's button mask (0x51..0x6d) is 1 << button, else disabled.
//! Device bytes per button here: [code lo, code hi, macro + 1 or 0, 0]; buttons 0-2 [0xff, n, 0, 0].

use crate::device::{Device, DeviceConfig, DeviceProfile, MACRO_SLOT, MACRO_SLOTS};
use crate::model::Func;

const ID: u8 = 5;
const LEN: usize = 12;
/// EEPROM bytes this app reads: the header and the three profile blocks.
pub const SIZE: usize = 0x600;
const PROFILES: usize = 3;
const STAGES: usize = 5;
const LEVELS: usize = 6;

/// Value bytes of buttons 3..10 (13 bits each, LSB first).
const STREAM: [usize; 13] = [0x33, 0x35, 0x37, 0x39, 0x3b, 0x3d, 0x3f, 0x41, 0x43, 0x45, 0x4b, 0x4d, 0x4f];
/// DPI level i: high 3 bits as (byte, shift) pairs, low bit first.
const HI: [[(usize, u8); 3]; LEVELS] = [
    [(0x7f, 3), (0x7f, 4), (0x7f, 5)],
    [(0x7f, 0), (0x7f, 1), (0x7f, 2)],
    [(0x89, 1), (0x89, 2), (0x89, 3)],
    [(0x8b, 2), (0x8b, 3), (0x8b, 4)],
    [(0x8b, 0), (0x8b, 1), (0x8d, 7)],
    [(0x8f, 2), (0x8f, 3), (0x8f, 4)],
];
/// Register bytes FUN_1001ae70 writes in every profile block: checked before a write.
const REGS: [(usize, u8); 20] = [
    (0x32, 0x1c), (0x34, 0x1d), (0x36, 0x1e), (0x38, 0x1f), (0x3a, 0x21), (0x3c, 0x22), (0x3e, 0x31),
    (0x40, 0x4a), (0x42, 0x51), (0x44, 0x56), (0x46, 0x7a), (0x48, 0x7b), (0x4a, 0x7c), (0x4c, 0x7d),
    (0x4e, 0x7e), (0x72, 0x11), (0x7e, 0x17), (0xa2, 0x3c), (0xac, 0x5e), (0xcc, 0x5e),
];
/// The driver's mouse functions (group 1 and 4 maps).
const MOUSE: [(Func, u16); 9] = [
    (Func::Back, 0x1800),
    (Func::Forward, 0x1801),
    (Func::Dpi(1), 0x1808),
    (Func::Dpi(2), 0x1809),
    (Func::Dpi(3), 0x180a),
    (Func::ProfileSwitch, 0x181d),
    (Func::Rate(1), 0x1814),
    (Func::LightSwitch, 0x1807),
    (Func::Disabled, 0),
];

fn block(p: usize) -> usize {
    0x300 + 0x100 * p
}

fn bits(e: &[u8], at: &[usize], from: usize, n: usize) -> u32 {
    (0..n).fold(0, |v, i| v | ((e[at[(from + i) / 8]] >> ((from + i) % 8)) as u32 & 1) << i)
}

fn set_bits(e: &mut [u8], at: &[usize], from: usize, n: usize, v: u32) {
    for i in 0..n {
        let (o, b) = (at[(from + i) / 8], (from + i) % 8);
        e[o] = e[o] & !(1 << b) | (((v >> i) & 1) as u8) << b;
    }
}

/// Code of button `n` (3..11) of the profile block at `b`.
pub fn code(e: &[u8], b: usize, n: usize) -> u16 {
    let at: Vec<usize> = if n == 11 { vec![b + 0x47, b + 0x49] } else { STREAM.iter().map(|o| b + o).collect() };
    let from = if n == 11 { 0 } else { (n - 3) * 13 };
    bits(e, &at, from, 13) as u16
}

pub fn set_code(e: &mut [u8], b: usize, n: usize, v: u16) {
    let at: Vec<usize> = if n == 11 { vec![b + 0x47, b + 0x49] } else { STREAM.iter().map(|o| b + o).collect() };
    let from = if n == 11 { 0 } else { (n - 3) * 13 };
    set_bits(e, &at, from, 13, v as u32);
}

/// Button mask of macro `j` (FUN_1001e020).
fn macro_mask(e: &[u8], b: usize, j: usize) -> u16 {
    let (lo, hi) = (b + 0x51 + 6 * (j / 2) + 2 * (j % 2), b + 0x55 + 6 * (j / 2));
    let h = if j % 2 == 0 { e[hi] >> 4 } else { e[hi] & 0xf };
    (h as u16) << 8 | e[lo] as u16
}

fn clear_macro(e: &mut [u8], b: usize, j: usize) {
    let (lo, hi) = (b + 0x51 + 6 * (j / 2) + 2 * (j % 2), b + 0x55 + 6 * (j / 2));
    e[lo] = 0;
    e[hi] &= if j % 2 == 0 { 0x0f } else { 0xf0 };
}

/// DPI register (cpi / 100 - 1) of level `i`.
pub fn level(e: &[u8], b: usize, i: usize) -> u8 {
    let hi = HI[i].iter().enumerate().fold(0, |v, (k, &(o, s))| v | (e[b + o] >> s & 1) << k);
    hi << 4 | e[b + 0x73 + 2 * i] & 0xf
}

pub fn set_level(e: &mut [u8], b: usize, i: usize, v: u8) {
    let o = b + 0x73 + 2 * i;
    e[o] = e[o] & 0xf0 | v & 0xf;
    for (k, &(o, s)) in HI[i].iter().enumerate() {
        e[b + o] = e[b + o] & !(1 << s) | (v >> (4 + k) & 1) << s;
    }
}

#[cfg(test)]
pub fn checksum(e: &[u8]) -> u32 {
    e[0x100..].chunks(4).fold(0, |s, w| s ^ u32::from_le_bytes(w.try_into().unwrap()))
}

pub fn encode_func(f: &Func) -> Option<[u8; 4]> {
    let c = match f {
        Func::Key(k) => crate::device::key_usage(k)? as u16,
        Func::Combo(spec) => {
            let parts: Vec<&str> = spec.split('+').collect();
            let (key, mods) = parts.split_last()?;
            let mut mask = 0u16;
            for m in mods {
                mask |= crate::device::combo_mask(m)? as u16;
            }
            mask << 8 | crate::device::key_usage(key)? as u16
        }
        Func::Media(i) => 0x1000 | *crate::du::MEDIA.get((*i as usize).checked_sub(1)?)?,
        Func::Raw(b) => return Some(*b),
        f => MOUSE.iter().find(|(m, _)| m == f)?.1,
    };
    Some([c as u8, (c >> 8) as u8, 0, 0])
}

pub fn decode_func(b: [u8; 4]) -> Option<Func> {
    if b[0] == 0xff {
        return [Func::Click, Func::Menu, Func::Middle].get(b[1] as usize).cloned();
    }
    if b[2] != 0 {
        return None; // a macro of the mouse's own
    }
    let c = b[0] as u16 | (b[1] as u16) << 8;
    if let Some((f, _)) = MOUSE.iter().find(|(_, m)| *m == c) {
        return Some(f.clone());
    }
    Some(match c >> 8 {
        0 => Func::Key(crate::device::usage_key(c as u8)),
        0x10..=0x1f if c < 0x1800 => Func::Media(crate::du::MEDIA.iter().position(|&m| m == c & 0xfff)? as u8 + 1),
        m @ 1..=0xf => {
            let mut spec = String::new();
            for (bit, name) in [(1, "CTRL"), (2, "SHIFT"), (4, "ALT"), (8, "WIN")] {
                if m & bit != 0 {
                    spec.push_str(name);
                    spec.push('+');
                }
            }
            spec.push_str(&crate::device::usage_key(c as u8));
            Func::Combo(spec)
        }
        _ => return None,
    })
}

fn rate_ms(r: u8) -> u8 {
    [8, 4, 2, 1][r as usize & 3]
}

/// The UI profile of profile `p` of the EEPROM image `e`.
pub fn decode(model: &crate::models::Model, e: &[u8], p: usize) -> DeviceProfile {
    let b = block(p);
    let n = ((e[b + 0x7f] >> 6) as usize + 3).min(STAGES);
    let mut stages = [[0u8; 5]; 5];
    for (i, s) in stages.iter_mut().enumerate() {
        let v = level(e, b, i);
        *s = [(i < n) as u8, v, 0, v, 0];
    }
    let buttons = (0..model.slot_count())
        .map(|i| {
            let k = model.slot(i).off as usize;
            if k < 3 {
                return [0xff, k as u8, 0, 0];
            }
            let c = code(e, b, k);
            let m = if c == 0 { (0..10).find(|&j| macro_mask(e, b, j) == 1 << k).map_or(0, |j| j as u8 + 1) } else { 0 };
            [c as u8, (c >> 8) as u8, m, 0]
        })
        .collect();
    DeviceProfile {
        stage: (e[b + 0xa3] & 7).min(n as u8 - 1),
        stages,
        polling_ms: rate_ms(!(e[b + 0xad] >> 3) & 3),
        buttons,
        led: [0; 7],
        image: Vec::new(),
    }
}

/// `e` with profile `p` set to `q` (fields the UI did not change keep their bytes).
pub fn encode(model: &crate::models::Model, e: &[u8], p: usize, q: &DeviceProfile) -> Vec<u8> {
    let mut e = e.to_vec();
    let b = block(p);
    let cur = decode(model, &e, p);
    if q.polling_ms != cur.polling_ms {
        let r = [8, 4, 2, 1].iter().position(|&m| m == q.polling_ms).unwrap_or(3) as u8;
        for o in [0xad, 0xcd] {
            e[b + o] = e[b + o] & 0xe7 | (r ^ 3) << 3;
        }
    }
    if q.stages != cur.stages || q.stage != cur.stage {
        // the mouse cycles through the first N levels (3..6): enabled ones go first
        let on: Vec<usize> = (0..STAGES).filter(|&i| q.stages[i][0] != 0).collect();
        let on = if on.is_empty() { vec![q.stage as usize] } else { on };
        let cur_i = on.iter().position(|&i| i == q.stage as usize).unwrap_or(0);
        let mut vals: Vec<u8> = on.iter().map(|&i| q.stages[i][1].min(0x7f)).collect();
        for i in 0..STAGES {
            if !on.contains(&i) {
                vals.push(q.stages[i][1].min(0x7f));
            }
        }
        for (i, &v) in vals.iter().enumerate() {
            set_level(&mut e, b, i, v);
        }
        let n = on.len().clamp(3, LEVELS) as u8;
        e[b + 0x7f] = e[b + 0x7f] & 0x3f | (n - 3) << 6;
        for o in [0xa3, 0xc3] {
            e[b + o] = e[b + o] & 0xf8 | cur_i as u8;
        }
    }
    for (i, (nb, ob)) in q.buttons.iter().zip(&cur.buttons).enumerate() {
        let k = model.slot(i).off as usize;
        if k < 3 || nb == ob || nb[0] == 0xff || nb[2] != 0 {
            continue;
        }
        let c = nb[0] as u16 | (nb[1] as u16) << 8;
        set_code(&mut e, b, k, c & 0x1fff);
        for j in 0..10 {
            if macro_mask(&e, b, j) == 1 << k {
                clear_macro(&mut e, b, j);
            }
        }
    }
    e
}

/// Register write (FUN_100155b0).
fn put(d: &Device, reg: u32, v: u32) -> std::io::Result<()> {
    let mut p = [0u8; LEN];
    p[0] = ID;
    p[1] = 1;
    p[2] = 2;
    p[4..8].copy_from_slice(&reg.to_le_bytes());
    p[8..12].copy_from_slice(&v.to_le_bytes());
    d.set_feature(&p, LEN)
}

fn release(d: &Device) -> std::io::Result<()> {
    let mut p = [0u8; LEN];
    p[0] = ID;
    p[1] = 1;
    d.set_feature(&p, LEN)
}

/// Register read (FUN_10015520).
fn get(d: &Device, reg: u32) -> std::io::Result<u32> {
    let mut p = [0u8; LEN];
    p[0] = ID;
    p[2] = 2;
    p[4..8].copy_from_slice(&reg.to_le_bytes());
    d.set_feature(&p, LEN)?;
    let r = d.get_feature(ID, LEN)?;
    release(d)?;
    Ok(u32::from_le_bytes(r[8..12].try_into().unwrap()))
}

fn open(d: &Device) -> std::io::Result<()> {
    put(d, 0x2e, 0)?;
    let v = get(d, 0x28)?;
    put(d, 0x28, v & 0x83)?;
    let v = get(d, 0x34)?;
    put(d, 0x34, v & !1)?;
    put(d, 0x16, 0x1a)
}

fn close(d: &Device) -> std::io::Result<()> {
    put(d, 0x16, 0x1e)?;
    let v = get(d, 0x34)?;
    put(d, 0x34, v | 1)
}

fn read_row(d: &Device, a: usize) -> std::io::Result<[u8; 8]> {
    put(d, 0x36, a as u32 & 0xff)?;
    put(d, 0x37, (a >> 8) as u32 & 0xff)?;
    put(d, 0x33, 0x10)?;
    put(d, 0x33, 0x18)?;
    let mut p = [0u8; LEN];
    p[0] = ID;
    p[2] = 6;
    p[4] = 0x40;
    d.set_feature(&p, LEN)?;
    let r = d.get_feature(ID, LEN)?;
    release(d)?;
    Ok(r[4..12].try_into().unwrap())
}

fn write_row(d: &Device, a: usize, v: &[u8]) -> std::io::Result<()> {
    put(d, 0x36, a as u32 & 0xff)?;
    put(d, 0x37, (a >> 8) as u32 & 0xff)?;
    let mut p = [0u8; LEN];
    p[0] = ID;
    p[1] = 1;
    p[2] = 6;
    p[3] = v[7];
    p[4..11].copy_from_slice(&v[..7]);
    d.set_feature(&p, LEN)?;
    put(d, 0x33, 0x10)?;
    put(d, 0x33, 0x14)
}

fn read_image(d: &Device) -> std::io::Result<Vec<u8>> {
    open(d)?;
    let mut e = Vec::with_capacity(SIZE);
    let r = (0..SIZE).step_by(8).try_for_each(|a| read_row(d, a).map(|v| e.extend(v)));
    close(d)?;
    r.map(|_| e)
}

/// Is the EEPROM laid out as FUN_1001ae70 writes it?
pub fn known_layout(e: &[u8]) -> bool {
    (0..PROFILES).all(|p| REGS.iter().all(|&(o, v)| e[block(p) + o] == v))
}

pub fn read_config(d: &Device) -> std::io::Result<DeviceConfig> {
    let e = read_image(d)?;
    let model = d.found.model;
    let mut profiles: Vec<DeviceProfile> = (0..PROFILES).map(|p| decode(model, &e, p)).collect();
    profiles.resize(5, profiles[0].clone());
    profiles[0].image = e.clone();
    Ok(DeviceConfig { active: (e[0xb] & 3).min(PROFILES as u8 - 1), profiles, macros: vec![[0u8; MACRO_SLOT]; MACRO_SLOTS] })
}

/// The rows of `e` that differ from `old`, with the checksum row updated.
pub fn rows(old: &[u8], e: &mut [u8]) -> Vec<usize> {
    let mut sum = u32::from_le_bytes(old[4..8].try_into().unwrap());
    for w in (0x100..SIZE).step_by(4) {
        sum ^= u32::from_le_bytes(old[w..w + 4].try_into().unwrap()) ^ u32::from_le_bytes(e[w..w + 4].try_into().unwrap());
    }
    e[4..8].copy_from_slice(&sum.to_le_bytes());
    (0..SIZE).step_by(8).filter(|&a| old[a..a + 8] != e[a..a + 8]).collect()
}

pub fn write_config(d: &Device, new: &DeviceConfig, _current: &DeviceConfig) -> std::io::Result<DeviceConfig> {
    let model = d.found.model;
    let old = read_image(d)?;
    if !known_layout(&old) {
        return Err(std::io::Error::other("unknown EEPROM layout: nothing written"));
    }
    let mut e = old.clone();
    for p in 0..PROFILES {
        e = encode(model, &e, p, &new.profiles[p]);
    }
    if new.active as usize != (old[0xb] & 3) as usize && (new.active as usize) < PROFILES {
        e[0xb] = e[0xb] & !3 | new.active;
    }
    let todo = rows(&old, &mut e);
    if !todo.is_empty() {
        open(d)?;
        let r = todo.iter().try_for_each(|&a| write_row(d, a, &e[a..a + 8]));
        close(d)?;
        r?;
    }
    let back = read_config(d)?;
    if back.profiles[0].image != e {
        return Err(std::io::Error::other("read-back differs from what was written"));
    }
    Ok(back)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A profile block the way FUN_1001ae70 leaves it.
    fn image() -> Vec<u8> {
        let mut e = vec![0xffu8; SIZE];
        for p in 0..PROFILES {
            for &(o, v) in &REGS {
                e[block(p) + o] = v;
            }
        }
        e
    }

    #[test]
    fn fields_follow_the_driver() {
        let mut e = image();
        let b = block(0);
        // FUN_1001d6a0 for level 4 = 6400 cpi (register 63): low nibble 0xf, hi bits 0b011
        set_level(&mut e, b, 4, 63);
        assert_eq!((e[b + 0x7b] & 0xf, e[b + 0x8b] & 3, e[b + 0x8d] >> 7), (0xf, 3, 0));
        assert_eq!(level(&e, b, 4), 63);
        for i in 0..LEVELS {
            set_level(&mut e, b, i, 7 * i as u8 + 0x41);
        }
        assert!((0..LEVELS).all(|i| level(&e, b, i) == 7 * i as u8 + 0x41));
        // FUN_1001d230 for button 4: [0x35] top 3 bits = code, [0x37] = code >> 3, [0x39] & 3 = code >> 11
        set_code(&mut e, b, 4, 0x1808);
        assert_eq!((e[b + 0x35] >> 5, e[b + 0x37], e[b + 0x39] & 3), (0, 0x01, 3));
        for n in 3..12 {
            set_code(&mut e, b, n, 0x1000 + n as u16);
        }
        assert!((3..12).all(|n| code(&e, b, n) == 0x1000 + n as u16));
        assert_eq!(e[b + 0x46], 0x7a);
    }

    #[test]
    fn functions_round_trip() {
        assert_eq!(encode_func(&Func::Dpi(1)), Some([0x08, 0x18, 0, 0]));
        assert_eq!(encode_func(&Func::Combo("WIN+D".into())), Some([0x07, 0x08, 0, 0]));
        assert_eq!(encode_func(&Func::Media(1)), Some([0xcd, 0x10, 0, 0]));
        for f in [Func::Back, Func::Forward, Func::Disabled, Func::Media(7), Func::Combo("CTRL+C".into()), Func::Key("A".into())] {
            assert_eq!(decode_func(encode_func(&f).unwrap()), Some(f));
        }
        assert_eq!(decode_func([0xff, 2, 0, 0]), Some(Func::Middle));
    }

    #[test]
    fn profile_round_trips() {
        let model = crate::models::MODELS.iter().find(|m| m.proto == crate::models::Proto::Pixart).unwrap();
        let mut e = image();
        let b = block(1);
        for (i, v) in [3u8, 7, 15, 23, 31, 31].iter().enumerate() {
            set_level(&mut e, b, i, *v);
        }
        e[b + 0x7f] = e[b + 0x7f] & 0x3f | 2 << 6; // 5 levels
        e[b + 0xa3] = 1;
        e[b + 0xad] = 0; // rate 3 (1000 Hz)
        e[b + 0xcd] = 0;
        set_code(&mut e, b, 3, 0x1801);
        set_code(&mut e, b, 4, 0x1800);
        set_code(&mut e, b, 5, 0x1808);
        let p = decode(model, &e, 1);
        assert_eq!((p.stage, p.polling_ms, p.stages[4], p.stages[0][1]), (1, 1, [1, 31, 0, 31, 0], 3));
        assert_eq!(encode(model, &e, 1, &p), e);
        let mut q = p.clone();
        q.stages[1][0] = 0;
        q.polling_ms = 8;
        let i = (0..model.slot_count()).find(|&i| model.slot(i).off == 5).unwrap();
        q.buttons[i] = encode_func(&Func::Dpi(2)).unwrap();
        let mut n = encode(model, &e, 1, &q);
        let r = decode(model, &n, 1);
        assert_eq!((r.polling_ms, r.stages[1][1], r.stages[3][0], r.stages[4][0]), (8, 15, 1, 0));
        assert_eq!(code(&n, b, 5), 0x1809);
        let todo = rows(&e, &mut n);
        let sum = |x: &[u8]| u32::from_le_bytes(x[4..8].try_into().unwrap());
        assert_eq!(sum(&n) ^ checksum(&n), sum(&e) ^ checksum(&e));
        assert!(todo.iter().all(|&a| (0x400..0x500).contains(&a) || a == 0));
    }
}
