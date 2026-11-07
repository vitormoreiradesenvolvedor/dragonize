//! CompX mice (USB 3554:xxxx; wired, 2.4 GHz dongle or both) configured by
//! Redragon's "Mouse Drive Beta.exe" and the Qt drivers. Both talk through the
//! vendor's HIDUsb.dll (decompiled; tools/re/fleet/NOTES.md "CompX"):
//!   17-byte output / input report 8 on interface 1:
//!     08 CMD 00 ADDR_HI ADDR_LO LEN DATA[10] CHK      CHK = 0x55 - sum of the 16 bytes before
//!   CMD 08 = read LEN bytes of the settings flash at ADDR (the reply carries them),
//!   CMD 07 = write them (UsbServer_ReadFalshData / FUN_10005460).
//! Flash map (CS_ProtocolDataParser / MouseConfigParser); every field ends with its
//! own check byte 0x55 - sum(field):
//!   00 report rate code (1/2/4/8 = 1000/500/250/125 Hz, 0x10/0x20 = 2k/4k)
//!   02 number of DPI stages            04 current stage
//!   0c + 4i  stage i: x, y, DPIex      2c + 4i  stage i colour
//!   60 + 4k  button k: type, param1, param2 (KEY_CLASS: 0 off, 1 mouse button,
//!            2 DPI, 5 shortcut -> keys at 100 + 20k: count, count x (state|type, lo, hi))
//! The old Sinowealth OemDrv builds (Lowerdev.dll, e.g. build 670711b3 FUN_00440da0 /
//! FUN_0043f7f0) send the same packets as feature report 8 and get the answer as
//! input report 9: 09 CMD STATUS(0 = ok) .. DATA at 6; same flash map and key codes.
//! Lighting, macros and the sensor extras are left as the mouse holds them.

use crate::device::{Device, DeviceConfig, DeviceProfile, MACRO_SLOT, MACRO_SLOTS};
use crate::model::Func;
use crate::models::Model;

/// Bytes of the flash the driver's "read all" fetches (settings, DPI, buttons, LED).
pub const FLASH: usize = 0xc0;
const SHORTCUT: u16 = 0x100;
const STAGES: usize = 5;

fn chk(b: &[u8]) -> u8 {
    0x55u8.wrapping_sub(b.iter().fold(0u8, |a, &x| a.wrapping_add(x)))
}

fn packet(cmd: u8, addr: u16, data: &[u8], len: u8) -> [u8; 17] {
    let mut p = [0u8; 17];
    p[..6].copy_from_slice(&[8, cmd, 0, (addr >> 8) as u8, addr as u8, len]);
    p[6..6 + data.len()].copy_from_slice(data);
    p[16] = chk(&p[..16]);
    p
}

/// Sends one command and waits for the mouse's answer to it (the driver retries
/// up to 5 times with 1 s apart).
fn xfer(d: &Device, cmd: u8, addr: u16, data: &[u8], len: u8) -> std::io::Result<[u8; 10]> {
    xfer_via(d, crate::models::current().proto == crate::models::Proto::CompxFeature, cmd, addr, data, len)
}

/// `feature`: the OemDrv builds: SetFeature, answer = input report 9 [9 cmd status ..] (no check byte)
fn xfer_via(d: &Device, feature: bool, cmd: u8, addr: u16, data: &[u8], len: u8) -> std::io::Result<[u8; 10]> {
    let p = packet(cmd, addr, data, len);
    for _ in 0..3 {
        if feature {
            d.set_feature(&p, p.len())?;
        } else {
            d.write_report(&p)?;
        }
        let t = std::time::Instant::now();
        while t.elapsed() < std::time::Duration::from_millis(1000) {
            let Some(r) = d.read_report(200)?.filter(|r| r.len() >= 17) else { continue };
            let ok = if feature {
                r[0] == 9 && r[1] == cmd && r[2] == 0
            } else {
                r[0] == 8 && r[1] == cmd && r[3..5] == p[3..5] && r[16] == chk(&r[..16])
            };
            if ok {
                let mut out = [0u8; 10];
                out.copy_from_slice(&r[6..16]);
                return Ok(out);
            }
        }
    }
    Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "the mouse did not answer (wireless mouse asleep or off?)"))
}

/// OemDrv CheckPsd (build 43d56b1f FUN_0044ff10): command 1 with a 4-byte challenge,
/// len 8; the answer carries the mouse's chip ids CID, MID at data 4, 5.
pub fn psd(d: &Device) -> Option<(u8, u8)> {
    let r = xfer_via(d, true, 1, 0, &[0x11, 0x22, 0x33, 0x44], 8).ok()?;
    (r[4] != 0 || r[5] != 0).then_some((r[4], r[5]))
}

fn read_flash(d: &Device, addr: u16, n: usize) -> std::io::Result<Vec<u8>> {
    let mut out = Vec::with_capacity(n);
    while out.len() < n {
        let k = (n - out.len()).min(10);
        let r = xfer(d, 8, addr + out.len() as u16, &[], k as u8)?;
        out.extend_from_slice(&r[..k]);
    }
    Ok(out)
}

fn write_flash(d: &Device, addr: u16, data: &[u8]) -> std::io::Result<()> {
    for (i, c) in data.chunks(10).enumerate() {
        xfer(d, 7, addr + 10 * i as u16, c, c.len() as u8)?;
    }
    Ok(())
}

fn rate_to_ms(code: u8) -> u8 {
    match code {
        2 | 4 | 8 => code,
        _ => 1,
    }
}

/// A shortcut slot holding one key (with modifiers) -> (modifier mask, usage).
fn shortcut_key(s: &[u8]) -> Option<(u8, u8)> {
    let n = *s.first()? as usize;
    if !(2..=6).contains(&n) || s.len() < 3 * n + 2 || s[3 * n + 1] != chk(&s[..3 * n + 1]) {
        return None;
    }
    let (mut mods, mut key) = (0, None);
    for e in s[1..1 + 3 * n].chunks(3) {
        match (e[0] & 0xc0, e[0] & 0xf) {
            (0x80, 0) => mods |= e[1],
            (0x80, 1) if key.is_none() => key = Some(e[1]),
            (0x80, _) => return None,
            _ => {}
        }
    }
    Some((mods, key?))
}

/// mdb ValueConvert.App_AddShortCutKey: every key pressed, then released in reverse.
fn shortcut_bytes(mods: u8, usage: u8) -> Vec<u8> {
    let mut keys: Vec<(u8, u8)> = (0..8).filter(|b| mods & (1 << b) != 0).map(|b| (0, 1 << b)).collect();
    keys.push((1, usage));
    let mut v = vec![2 * keys.len() as u8];
    for &(t, c) in &keys {
        v.extend_from_slice(&[0x80 | t, c, 0]);
    }
    for &(t, c) in keys.iter().rev() {
        v.extend_from_slice(&[0x40 | t, c, 0]);
    }
    v.push(chk(&v));
    v
}

fn decode(model: &Model, flash: &[u8], shortcuts: &[Vec<u8>]) -> DeviceProfile {
    let n = (flash[2] as usize).clamp(1, 8);
    let mut stages = [[0u8; 5]; 5];
    for (i, s) in stages.iter_mut().enumerate() {
        let o = 0xc + 4 * i;
        *s = [(i < n) as u8, flash[o], flash[o + 2], flash[o + 1], flash[o + 2]];
    }
    let buttons = model.buttons.iter().map(|b| {
        let o = 0x60 + 4 * b.off as usize;
        let f = [flash[o], flash[o + 1], flash[o + 2], 0];
        match shortcuts.get(b.off as usize).and_then(|s| shortcut_key(s)) {
            Some((m, u)) if f[0] == 5 => [5, m, u, 1],
            _ => f,
        }
    }).collect();
    DeviceProfile {
        stage: (flash[4] as usize).min(n - 1).min(STAGES - 1) as u8,
        stages,
        polling_ms: rate_to_ms(flash[0]),
        buttons,
        led: [0; 7],
        image: flash.to_vec(),
    }
}

/// The flash image with the profile's settings put in, and the shortcut slots it needs.
pub fn encode(model: &Model, p: &DeviceProfile) -> (Vec<u8>, Vec<(u16, Vec<u8>)>) {
    let mut f = p.image.clone();
    f.resize(FLASH, 0);
    let field = |f: &mut Vec<u8>, at: usize, v: &[u8]| {
        f[at..at + v.len()].copy_from_slice(v);
        f[at + v.len()] = chk(v);
    };
    // the polling code only changes when the UI's value differs (2k/4k stay)
    if rate_to_ms(f[0]) != p.polling_ms {
        field(&mut f, 0, &[p.polling_ms]);
    }
    // the firmware cycles through the first N stages: enabled ones go first
    let on: Vec<usize> = (0..STAGES).filter(|&i| p.stages[i][0] != 0).collect();
    let on = if on.is_empty() { vec![p.stage as usize] } else { on };
    if f[2] as usize != on.len() || f[3] != chk(&[f[2]]) {
        field(&mut f, 2, &[on.len() as u8]);
    }
    let cur = on.iter().position(|&i| i == p.stage as usize).unwrap_or(0) as u8;
    if f[4] != cur || f[5] != chk(&[f[4]]) {
        field(&mut f, 4, &[cur]);
    }
    for (j, &i) in on.iter().enumerate() {
        let s = p.stages[i];
        let o = 0xc + 4 * j;
        if f[o..o + 3] != [s[1], s[3], s[2]] {
            field(&mut f, o, &[s[1], s[3], s[2]]);
        }
    }
    let mut shortcuts = Vec::new();
    for (b, c) in model.buttons.iter().zip(&p.buttons) {
        let k = b.off as usize;
        let o = 0x60 + 4 * k;
        if c[0] == 5 && c[3] == 1 {
            shortcuts.push((SHORTCUT + 0x20 * k as u16, shortcut_bytes(c[1], c[2])));
            field(&mut f, o, &[5, 0, 0]);
        } else if f[o..o + 3] != c[..3] {
            field(&mut f, o, &c[..3]);
        }
    }
    (f, shortcuts)
}

fn read_shortcuts(d: &Device, model: &Model, flash: &[u8]) -> std::io::Result<Vec<Vec<u8>>> {
    let mut out = vec![Vec::new(); 16];
    for b in model.buttons {
        let k = b.off as usize;
        if k < 16 && flash[0x60 + 4 * k] == 5 {
            let head = read_flash(d, SHORTCUT + 0x20 * k as u16, 1)?[0] as usize;
            if (2..=6).contains(&head) {
                out[k] = read_flash(d, SHORTCUT + 0x20 * k as u16, 3 * head + 2)?;
            }
        }
    }
    Ok(out)
}

pub fn read_config(d: &Device) -> std::io::Result<DeviceConfig> {
    let model = d.found.model;
    let flash = read_flash(d, 0, FLASH)?;
    let shortcuts = read_shortcuts(d, model, &flash)?;
    let p = decode(model, &flash, &shortcuts);
    // one configuration on the mouse; the UI's other four profiles stay local
    Ok(DeviceConfig { active: 0, profiles: vec![p; 5], macros: vec![[0u8; MACRO_SLOT]; MACRO_SLOTS] })
}

pub fn write_config(d: &Device, new: &DeviceConfig, current: &DeviceConfig) -> std::io::Result<DeviceConfig> {
    let model = d.found.model;
    let (n, c) = (&new.profiles[0], &current.profiles[0]);
    let (f, shortcuts) = encode(model, n);
    let (cf, cshort) = encode(model, c);
    for (addr, bytes) in &shortcuts {
        if !cshort.contains(&(*addr, bytes.clone())) {
            write_flash(d, *addr, bytes)?;
        }
    }
    // changed fields only, each with its check byte
    let mut i = 0;
    while i < FLASH {
        if f[i] != cf[i] {
            let start = i;
            while i < FLASH && (f[i] != cf[i] || i - start < 2) {
                i += 1;
            }
            write_flash(d, start as u16, &f[start..i])?;
        } else {
            i += 1;
        }
    }
    let back = read_config(d)?;
    let (a, b) = (&back.profiles[0], &encode_back(model, n));
    if (a.stage, a.polling_ms, &a.buttons) != (b.stage, b.polling_ms, &b.buttons) || a.stages != b.stages {
        return Err(std::io::Error::other("read-back differs from what was written"));
    }
    Ok(back)
}

/// What reading the profile back should give (enabled stages move to the front).
fn encode_back(model: &Model, p: &DeviceProfile) -> DeviceProfile {
    let (f, shortcuts) = encode(model, p);
    let mut s = vec![Vec::new(); 16];
    for (addr, b) in shortcuts {
        s[((addr - SHORTCUT) / 0x20) as usize] = b;
    }
    decode(model, &f, &s)
}

// ------------------------------------------------------------------ buttons
//
// [type, param1, param2, 1 = shortcut decoded], KEY_CLASS (DriverLib, mdb):
//   1 mouse button (01 left 02 right 04 middle 08 back 10 forward)
//   2 DPI (1 cycle, 2 +, 3 -)        5 shortcut        0 disabled

pub fn encode_func(f: &Func) -> Option<[u8; 4]> {
    Some(match f {
        Func::Click => [1, 0x01, 0, 0],
        Func::Menu => [1, 0x02, 0, 0],
        Func::Middle => [1, 0x04, 0, 0],
        Func::Back => [1, 0x08, 0, 0],
        Func::Forward => [1, 0x10, 0, 0],
        Func::Dpi(1) => [2, 1, 0, 0],
        Func::Dpi(2) => [2, 2, 0, 0],
        Func::Dpi(3) => [2, 3, 0, 0],
        Func::Key(k) => [5, 0, crate::device::key_usage(k)?, 1],
        Func::Combo(spec) => {
            let parts: Vec<&str> = spec.split('+').collect();
            let (key, mods) = parts.split_last()?;
            let mut mask = 0;
            for m in mods {
                mask |= crate::device::combo_mask(m)?;
            }
            [5, mask, crate::device::key_usage(key)?, 1]
        }
        Func::Disabled => [0, 0, 0, 0],
        Func::Raw(b) => *b,
        _ => return None,
    })
}

pub fn decode_func(b: [u8; 4]) -> Option<Func> {
    Some(match b {
        [1, 0x01, ..] => Func::Click,
        [1, 0x02, ..] => Func::Menu,
        [1, 0x04, ..] => Func::Middle,
        [1, 0x08, ..] => Func::Back,
        [1, 0x10, ..] => Func::Forward,
        [2, 1, ..] => Func::Dpi(1),
        [2, 2, ..] => Func::Dpi(2),
        [2, 3, ..] => Func::Dpi(3),
        [5, 0, k, 1] => Func::Key(crate::device::usage_key(k)),
        [5, mask, k, 1] => {
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
        [0, 0, 0, 0] => Func::Disabled,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> &'static Model {
        crate::models::MODELS.iter().find(|m| m.proto == crate::models::Proto::Compx).unwrap()
    }

    #[test]
    fn packets_match_hidusb() {
        // UsbServer_ReadFalshData(0, 10): 08 08 00 00 00 0a .. chk
        let p = packet(8, 0, &[], 10);
        assert_eq!(&p[..6], &[8, 8, 0, 0, 0, 10]);
        assert_eq!(p[16], 0x55u8.wrapping_sub(8 + 8 + 10));
        assert_eq!(chk(&[1]), 0x54);
    }

    #[test]
    fn flash_round_trips() {
        let m = model();
        let mut f = vec![0u8; FLASH];
        let mut put = |at: usize, v: &[u8]| {
            f[at..at + v.len()].copy_from_slice(v);
            f[at + v.len()] = chk(v);
        };
        put(0, &[2]);
        put(2, &[4]);
        put(4, &[1]);
        for i in 0..4 {
            put(0xc + 4 * i, &[15 + 16 * i as u8, 15 + 16 * i as u8, 0]);
        }
        put(0x60, &[1, 1, 0]);
        put(0x64, &[5, 0, 0]);
        let sc = shortcut_bytes(1, 0x04); // Ctrl+A
        let mut shortcuts = vec![Vec::new(); 16];
        shortcuts[1] = sc.clone();
        let p = decode(m, &f, &shortcuts);
        assert_eq!((p.stage, p.polling_ms), (1, 2));
        assert_eq!(p.stages[3], [1, 63, 0, 63, 0]);
        assert_eq!(p.stages[4][0], 0);
        assert_eq!(decode_func(p.buttons[0]), Some(Func::Click));
        assert_eq!(decode_func(p.buttons[1]), Some(Func::Combo("CTRL+A".into())));
        assert_eq!(sc, [4, 0x80, 1, 0, 0x81, 4, 0, 0x41, 4, 0, 0x40, 1, 0, chk(&sc[..13])]);
        // unchanged profile -> identical flash; disabling stage 1 moves the rest up
        assert_eq!(encode(m, &p).0, f);
        let mut q = p.clone();
        q.stages[0][0] = 0;
        let (g, _) = encode(m, &q);
        assert_eq!((g[2], g[4], g[0xc]), (3, 0, 31));
    }
}
