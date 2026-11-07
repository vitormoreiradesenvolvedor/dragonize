//! duilib mice (Evision firmware, USB 320f:xxxx wired / 2.4 GHz dongle) configured by
//! Redragon's DuiLib drivers (M916-PLUS build decompiled; tools/re/fleet/NOTES.md
//! "duilib mice"). Same 64-byte packets as the HidServ keyboards (kbd.rs `hs_packet`) on the
//! usage page 0xff1c / usage 0x92 interface:
//!   04 SUM_LO SUM_HI CMD LEN OFF_LO OFF_HI 00 DATA      SUM = bytes 3..63
//!   the answer echoes CMD at [3]; [7] 0xff / 0xfe = refused; data from [8]
//! Commands (FUN_00484bf0 ..): 01 / 02 begin / end of a write, 03 device info (0x22 bytes),
//! 04 write it back (first run: [7] 0 -> 0x16), 05 / 06 read / write settings, 07 / 09 read /
//! write the key table (3 bytes per key), 0xaa does the dongle see the mouse ([8] = 0xff).
//! Info (FUN_004af040): [0] 0x55aa, [5] number of keys, [7] 0x16 = initialised, [9] sensor,
//! [0xb] DPI step, [0xc] DPI max, [0xe] DPI min.
//! Settings: [0] active profile; profile p at p * 0x80, 0x57 bytes (FUN_004b2830 / 004af610):
//!   01 light mode  02 brightness  03 4 - speed  04 direction  05 colourful  06 R  07 G  08 B
//!   0b report rate (0 125 .. 3 1000 Hz)  0c current stage
//!   0e + 9i  stage i (7): enabled, ?, DPI (2 bytes; sensor 8925: one byte written twice), ?, ?, R, G, B
//!   4d-50 sleep timers  51 debounce  52 lift-off  53 linear correction  54 ripple  56 light mode
//! Key codes [type, p1, p2] (FUN_00410db0 and its tables): 10 mouse button mask, 13 DPI
//! (1 +, 2 -, 3 cycle), 20 key (p1 modifier mask, p2 usage), 30 consumer usage (lo, hi),
//! 40 system, 14 fire (interval ms, count), 11 / 12 swing / wheel, 70 / 71 macro.
//! Lighting, macros and the sensor extras are left as the mouse holds them.

use std::sync::Mutex;

use crate::device::{Device, DeviceConfig, DeviceProfile, MACRO_SLOT, MACRO_SLOTS};
use crate::model::Func;

const BLOCK: usize = 0x57;
const STRIDE: u16 = 0x80;
const CHUNK: usize = 0x38;
const STAGE: usize = 0x0e;

/// What the mouse said about its sensor (command 3): the DPI codec depends on it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Info {
    pub sensor: u16,
    pub step: u8,
    pub max: u16,
    pub min: u16,
}

static INFO: Mutex<Option<(Info, &'static [(u32, u16)])>> = Mutex::new(None);

/// The CPI the driver shows for a register value (FUN_004b8470).
pub fn cpi(i: &Info, raw: u32) -> u32 {
    let (step, min) = (i.step.max(1) as u32, i.min as u32);
    match i.sensor {
        0x3325 if raw <= 0x30 => raw * step + min,
        0x3325 => step * 0x30 + (raw - 0x30) * 200 + min,
        0x3395 => raw * 50 + 50,
        0x8925 if raw <= 99 => raw * 50 + 50,
        0x8925 => (raw - 50) * 100 + 100,
        0x3104 => match raw {
            0 => 200,
            1 => 400,
            2..=0x34 => (raw - 2) * 50 + 400,
            _ => (raw.saturating_sub(0x36) * 100 + 3000).min(8000),
        },
        0x0070 if raw <= 30000 => raw + 10,
        0x0070 if raw > 0x7526 => (raw + 10 + (raw & 1)).min(45000),
        0x0070 => raw,
        _ => raw * step + min,
    }
}

/// (cpi, register) of the DPI slider: the driver's ranges (FUN_004af040), 0x70 in 10-CPI steps.
pub fn table(i: &Info) -> Vec<(u32, u16)> {
    let raws: Vec<u32> = match i.sensor {
        0x8925 => (0..=0x95).collect(),
        0x3395 => (0..=0x207).collect(),
        0x3104 => (0..=(i.max as u32).saturating_sub(10)).collect(),
        0x0070 => (40..=44990).step_by(10).collect(),
        _ => (0..=0xffff).take_while(|&r| cpi(i, r) <= (i.max as u32).max(i.min as u32 + 1)).collect(),
    };
    raws.into_iter().map(|r| (cpi(i, r), r as u16)).collect()
}

/// The table of the mouse last read, once its sensor is known.
pub fn runtime_table() -> Option<&'static [(u32, u16)]> {
    INFO.lock().ok()?.map(|(_, t)| t)
}

fn set_info(i: Info) {
    let Ok(mut g) = INFO.lock() else { return };
    if g.is_none_or(|(old, _)| old != i) {
        *g = Some((i, Box::leak(table(&i).into_boxed_slice())));
    }
}

fn one_byte_dpi() -> bool {
    // before any read: the generator's guess (gen_duilib.py), 8925 = a 150-step table
    INFO.lock().ok().and_then(|g| g.map(|(i, _)| i.sensor == 0x8925)).unwrap_or(crate::models::current().dpi_values().len() == 0x96)
}

pub(crate) fn cmd(d: &Device, c: u8, off: u16, len: u8, data: &[u8]) -> std::io::Result<Vec<u8>> {
    let pkt = crate::kbd::hs_packet(c, off, len, data);
    for _ in 0..4 {
        while d.read_report(0)?.is_some() {}
        d.write_report(&pkt)?;
        for _ in 0..4 {
            match d.read_report(300)? {
                Some(r) if r.len() >= 8 && r[0] == 4 && r[3] == c => {
                    if c != 0xaa && (r[7] == 0xff || r[7] == 0xfe) {
                        return Err(std::io::Error::other(format!("device refused command {c:02x} ({:02x})", r[7])));
                    }
                    return Ok(r);
                }
                Some(_) => continue,
                None => break,
            }
        }
    }
    Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "no answer (wireless device asleep or off?)"))
}

pub(crate) fn read(d: &Device, c: u8, off: u16, n: usize) -> std::io::Result<Vec<u8>> {
    let mut out = Vec::with_capacity(n);
    while out.len() < n {
        let k = (n - out.len()).min(CHUNK);
        let r = cmd(d, c, off + out.len() as u16, k as u8, &[])?;
        out.extend_from_slice(r.get(8..8 + k).ok_or_else(|| std::io::Error::other("short answer"))?);
    }
    Ok(out)
}

pub(crate) fn write(d: &Device, c: u8, off: u16, data: &[u8]) -> std::io::Result<()> {
    for (i, ch) in data.chunks(CHUNK).enumerate() {
        cmd(d, c, off + (i * CHUNK) as u16, ch.len() as u8, ch)?;
    }
    Ok(())
}

/// Info block; initialises a factory-fresh mouse the way the driver does.
pub(crate) fn info(d: &Device) -> std::io::Result<(Info, usize)> {
    let pid = std::fs::read_to_string(d.found.usb.join("idProduct")).ok().and_then(|s| u16::from_str_radix(s.trim(), 16).ok());
    if pid.is_some_and(|p| p != d.found.model.pid) {
        // the dongle answers for the mouse only while it is linked (Hid2_4GDeviceIsLink)
        let r = cmd(d, 0xaa, 0, 0, &[])?;
        if r.get(8) != Some(&0xff) || r[7] == 0xff || r[7] == 0xfe {
            return Err(std::io::Error::other("the dongle does not see the device (asleep or off?)"));
        }
    }
    let mut b = read(d, 3, 0, 0x22)?;
    let le = |o: usize| u16::from_le_bytes([b[o], b[o + 1]]);
    if b[7] != 0x16 {
        if b[7] != 0 || le(0) != 0x55aa {
            return Err(std::io::Error::other("the device reports no configuration (not initialised)"));
        }
        b[7] = 0x16;
        cmd(d, 1, 0, 0, &[])?;
        write(d, 4, 0, &b)?;
        cmd(d, 2, 0, 0, &[])?;
    }
    let le = |o: usize| u16::from_le_bytes([b[o], b[o + 1]]);
    Ok((Info { sensor: le(9), step: b[0xb], max: le(0xc), min: le(0xe) }, b[5] as usize))
}

fn rate_ms(code: u8) -> u8 {
    [8, 4, 2, 1].get(code as usize).copied().unwrap_or(1)
}

fn decode(model: &crate::models::Model, image: &[u8], one_byte: bool) -> DeviceProfile {
    let blk = &image[..BLOCK];
    let keys = &image[BLOCK..];
    let mut stages = [[0u8; 5]; 5];
    for (i, s) in stages.iter_mut().enumerate() {
        let o = STAGE + 9 * i;
        let reg = if one_byte { blk[o + 2] as u16 } else { u16::from_le_bytes([blk[o + 2], blk[o + 3]]) };
        *s = [(blk[o] != 0) as u8, reg as u8, (reg >> 8) as u8, reg as u8, (reg >> 8) as u8];
    }
    let buttons = model.buttons.iter().map(|b| {
        let o = 3 * b.off as usize;
        keys.get(o..o + 3).map_or([0; 4], |k| [k[0], k[1], k[2], 0])
    }).collect();
    DeviceProfile {
        stage: blk[0xc].min(4),
        stages,
        polling_ms: rate_ms(blk[0xb]),
        buttons,
        led: [0; 7],
        image: image.to_vec(),
    }
}

/// The profile block and key table with the profile's settings put in.
pub fn encode(model: &crate::models::Model, p: &DeviceProfile, one_byte: bool) -> Vec<u8> {
    let mut img = p.image.clone();
    if img.len() < BLOCK {
        img.resize(BLOCK, 0);
    }
    let blk = &mut img[..BLOCK];
    if rate_ms(blk[0xb]) != p.polling_ms {
        blk[0xb] = [8, 4, 2, 1].iter().position(|&m| m == p.polling_ms).unwrap_or(3) as u8;
    }
    blk[0xc] = p.stage;
    for (i, s) in p.stages.iter().enumerate() {
        let o = STAGE + 9 * i;
        blk[o] = s[0];
        if one_byte {
            blk[o + 2] = s[1];
            blk[o + 3] = s[1];
        } else {
            blk[o + 2] = s[1];
            blk[o + 3] = s[2];
        }
    }
    for (b, c) in model.buttons.iter().zip(&p.buttons) {
        let o = BLOCK + 3 * b.off as usize;
        if let Some(k) = img.get_mut(o..o + 3) {
            k.copy_from_slice(&c[..3]);
        }
    }
    img
}

pub fn read_config(d: &Device) -> std::io::Result<DeviceConfig> {
    let model = d.found.model;
    let (inf, nkeys) = info(d)?;
    set_info(inf);
    let active = read(d, 5, 0, 1)?[0];
    let mut image = read(d, 5, active as u16 * STRIDE, BLOCK)?;
    image[0] = active;
    image.extend(read(d, 7, 0, 3 * nkeys.min(0xaa))?);
    let p = decode(model, &image, inf.sensor == 0x8925);
    // the active profile only; the UI's other four stay local (like the CompX mice)
    Ok(DeviceConfig { active: 0, profiles: vec![p; 5], macros: vec![[0u8; MACRO_SLOT]; MACRO_SLOTS] })
}

pub fn write_config(d: &Device, new: &DeviceConfig, current: &DeviceConfig) -> std::io::Result<DeviceConfig> {
    let model = d.found.model;
    let one = one_byte_dpi();
    let (n, c) = (encode(model, &new.profiles[0], one), encode(model, &current.profiles[0], one));
    let active = n[0];
    if n[..BLOCK] != c[..BLOCK] {
        cmd(d, 1, 0, 0, &[])?;
        write(d, 6, active as u16 * STRIDE, &n[..BLOCK])?;
        cmd(d, 2, 0, 0, &[])?;
    }
    if n[BLOCK..] != c[BLOCK..] {
        cmd(d, 1, 0, 0, &[])?;
        write(d, 9, 0, &n[BLOCK..])?;
        cmd(d, 2, 0, 0, &[])?;
    }
    let back = read_config(d)?;
    let want = decode(model, &n, one);
    let a = &back.profiles[0];
    if (a.stage, a.polling_ms, &a.buttons, a.stages) != (want.stage, want.polling_ms, &want.buttons, want.stages) {
        return Err(std::io::Error::other("read-back differs from what was written"));
    }
    Ok(back)
}

// ------------------------------------------------------------------ buttons

pub(crate) const MEDIA: [u16; 7] = [0xcd, 0xb7, 0xb6, 0xb5, 0xe9, 0xea, 0xe2];

pub fn encode_func(f: &Func) -> Option<[u8; 4]> {
    Some(match f {
        Func::Click => [0x10, 0x01, 0, 0],
        Func::Menu => [0x10, 0x02, 0, 0],
        Func::Middle => [0x10, 0x04, 0, 0],
        Func::Back => [0x10, 0x08, 0, 0],
        Func::Forward => [0x10, 0x10, 0, 0],
        Func::Dpi(1) => [0x13, 3, 0, 0],
        Func::Dpi(2) => [0x13, 1, 0, 0],
        Func::Dpi(3) => [0x13, 2, 0, 0],
        Func::Key(k) => [0x20, 0, crate::device::key_usage(k)?, 0],
        Func::Combo(spec) => {
            let parts: Vec<&str> = spec.split('+').collect();
            let (key, mods) = parts.split_last()?;
            let mut mask = 0;
            for m in mods {
                mask |= crate::device::combo_mask(m)?;
            }
            [0x20, mask, crate::device::key_usage(key)?, 0]
        }
        Func::Media(i) => {
            let u = *MEDIA.get((*i as usize).checked_sub(1)?)?;
            [0x30, u as u8, (u >> 8) as u8, 0]
        }
        Func::FireKey(fk) if fk.target == 0 => [0x14, fk.delay_ms.clamp(10, 255) as u8, fk.times.clamp(1, 255) as u8, 0],
        Func::Disabled => [0, 0, 0, 0],
        Func::Raw(b) => *b,
        _ => return None,
    })
}

pub fn decode_func(b: [u8; 4]) -> Option<Func> {
    Some(match b {
        [0x10, 0x01, ..] => Func::Click,
        [0x10, 0x02, ..] => Func::Menu,
        [0x10, 0x04, ..] => Func::Middle,
        [0x10, 0x08, ..] => Func::Back,
        [0x10, 0x10, ..] => Func::Forward,
        [0x13, 3, ..] => Func::Dpi(1),
        [0x13, 1, ..] => Func::Dpi(2),
        [0x13, 2, ..] => Func::Dpi(3),
        [0x20, 0, k, _] => Func::Key(crate::device::usage_key(k)),
        [0x20, mask, k, _] if mask & 0xf0 == 0 => {
            let mut spec = String::new();
            for (bit, name) in [(1, "CTRL"), (2, "SHIFT"), (4, "ALT"), (8, "WIN")] {
                if mask & bit != 0 {
                    spec.push_str(name);
                    spec.push('+');
                }
            }
            spec.push_str(&crate::device::usage_key(k));
            Func::Combo(spec)
        }
        [0x30, lo, hi, _] => Func::Media(MEDIA.iter().position(|&m| m == (lo as u16 | (hi as u16) << 8))? as u8 + 1),
        [0x14, ms, n, _] => Func::FireKey(crate::model::FireKey { target: 0, key: String::new(), times: n as u32, delay_ms: ms as u32 }),
        [0, 0, 0, 0] => Func::Disabled,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> &'static crate::models::Model {
        crate::models::MODELS.iter().find(|m| m.proto == crate::models::Proto::Duilib).unwrap()
    }

    #[test]
    fn dpi_follows_the_driver() {
        let i = |sensor| Info { sensor, step: 50, max: 26000, min: 50 };
        assert_eq!(cpi(&i(0x3395), 0x207), 26000);
        assert_eq!((cpi(&i(0x8925), 7), cpi(&i(0x8925), 0x95)), (400, 10000));
        assert_eq!((cpi(&i(0x0070), 390), cpi(&i(0x0070), 44990)), (400, 45000));
        assert_eq!((cpi(&i(0x3104), 0), cpi(&i(0x3104), 0x36)), (200, 3000));
        let t = table(&i(0x8925));
        assert_eq!((t[0], t[t.len() - 1]), ((50, 0), (10000, 0x95)));
    }

    #[test]
    fn block_round_trips() {
        let m = model();
        let mut img = vec![0u8; BLOCK + 3 * 11];
        img[0xb] = 2;
        img[0xc] = 1;
        for i in 0..7 {
            img[STAGE + 9 * i] = (i < 4) as u8;
            img[STAGE + 9 * i + 2] = 7 + 8 * i as u8;
            img[STAGE + 9 * i + 3] = 7 + 8 * i as u8;
        }
        img[BLOCK..BLOCK + 6].copy_from_slice(&[0x10, 1, 0, 0x10, 2, 0]);
        let p = decode(m, &img, true);
        assert_eq!((p.stage, p.polling_ms), (1, 2));
        assert_eq!(p.stages[2], [1, 23, 0, 23, 0]);
        assert_eq!(p.stages[4][0], 0);
        assert_eq!(decode_func(p.buttons[0]), Some(Func::Click));
        assert_eq!(encode(m, &p, true), img);
        let mut q = p.clone();
        q.stages[0][1] = 9;
        q.polling_ms = 1;
        let g = encode(m, &q, true);
        assert_eq!((g[STAGE + 2], g[STAGE + 3], g[0xb]), (9, 9, 3));
        assert_eq!(encode_func(&Func::Dpi(2)), Some([0x13, 1, 0, 0]));
        assert_eq!(decode_func([0x30, 0xe9, 0, 0]), Some(Func::Media(5)));
    }
}
