//! BYDUI mice (DeviceDriver.exe + BYDUI.dll with its .pdb, USB 372e:1060 wired / 372e:105f
//! 2.4 GHz: M998-ULT; tools/re/fleet/NOTES.md "BYDUI family"). Interface usage page 0xff00,
//! usage 1; 64-byte feature report 3 both ways (DriverComm::SendFeatureReport / GetFeatureReport):
//!   03 CHK 50 LEN TAG CMD AT DATA..   CHK = sum of bytes 2..63, TAG = any byte the mouse echoes
//! The answer (GET feature) has the status in byte 2 (0 done, 1 failed, 2 busy) and, for reads,
//! the 10 bytes asked for from byte 6. Commands (DeviceMainDlg::Set* / Get*):
//!   4f  read 10 bytes: AT = page, keys 0..3, configuration 0x40 + n
//!   31 / 32  key block (10 + 10 + 4 bytes at AT 0, 10, 20; 4 bytes per key)
//!   36 / 37  configuration block (10 bytes at AT 0, 10, 20, 30; then byte 50 alone)
//! Configuration (m_config_data): 0 chip, 1 rate wired | 2.4 GHz << 4 (0..6 = 125..8000 Hz),
//! 2 (stage + 1) << 4 | stages, 3 lift-off, 4 debounce, 5 sensor flags, 8 sleep,
//! 11 + 2 i stage i (LE, cpi / 50 - 1, 8 stages), 27 + 3 i its colour.
//! Keys (GetMouseData): 01 b n 00 mouse button b (1 left, 2 right, 3 middle, 4 forward, 5 back,
//! 6 / 7 wheel; n = clicks), 05 v DPI (1 +, 2 -, 4 cycle), 00 mods k2 k1 keyboard (HID usages),
//! 02 00 hi lo consumer usage, 00 00 00 00 disabled.

use crate::device::{Device, DeviceConfig, DeviceProfile, MACRO_SLOT, MACRO_SLOTS};
use crate::model::Func;

const ID: u8 = 3;
const LEN: usize = 64;
const CFG: usize = 60;
/// Keys the driver writes (its default table: 5 buttons + the DPI button).
const KEYS: usize = 6;
const STAGES: usize = 5;

/// The report for `cmd` (TAG, CMD, AT, DATA), with its checksum.
pub fn packet(tag: u8, cmd: u8, at: u8, data: &[u8]) -> [u8; LEN] {
    let mut p = [0u8; LEN];
    p[0] = ID;
    p[2] = 0x50;
    p[3] = data.len() as u8;
    p[4] = tag;
    p[5] = cmd;
    p[6] = at;
    p[7..7 + data.len()].copy_from_slice(data);
    p[1] = p[2..].iter().fold(0u8, |s, &b| s.wrapping_add(b));
    p
}

/// Sends one command and waits for the mouse to answer it (the driver: 3 tries, 5 polls each).
fn exchange(d: &Device, p: &[u8; LEN]) -> std::io::Result<Vec<u8>> {
    for _ in 0..3 {
        d.set_feature(p, LEN)?;
        for _ in 0..5 {
            std::thread::sleep(std::time::Duration::from_millis(5));
            let r = d.get_feature(ID, LEN)?;
            if r[4] != p[4] {
                continue;
            }
            match r[2] {
                0 => return Ok(r),
                2 => continue,
                _ => break,
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    Err(std::io::Error::other(format!("no answer to command {:02x} {:02x}", p[5], p[6])))
}

fn read(d: &Device, tag: u8, page: u8) -> std::io::Result<[u8; 10]> {
    let r = exchange(d, &packet(tag, 0x4f, page, &[]))?;
    Ok(r[6..16].try_into().unwrap())
}

pub fn encode_func(f: &Func) -> Option<[u8; 4]> {
    Some(match f {
        Func::Click => [1, 1, 1, 0],
        Func::Menu => [1, 2, 1, 0],
        Func::Middle => [1, 3, 1, 0],
        Func::Forward => [1, 4, 1, 0],
        Func::Back => [1, 5, 1, 0],
        Func::ScrollUp => [1, 6, 1, 0],
        Func::ScrollDown => [1, 7, 1, 0],
        Func::Dpi(1) => [5, 4, 0, 0],
        Func::Dpi(2) => [5, 1, 0, 0],
        Func::Dpi(3) => [5, 2, 0, 0],
        Func::Key(k) => [0, 0, 0, crate::device::key_usage(k)?],
        Func::Combo(spec) => {
            let parts: Vec<&str> = spec.split('+').collect();
            let (key, mods) = parts.split_last()?;
            let mut mask = 0;
            for m in mods {
                mask |= crate::device::combo_mask(m)?;
            }
            [0, mask, 0, crate::device::key_usage(key)?]
        }
        Func::Media(i) => {
            let u = *crate::du::MEDIA.get((*i as usize).checked_sub(1)?)?;
            [2, 0, (u >> 8) as u8, u as u8]
        }
        Func::Disabled => [0, 0, 0, 0],
        Func::Raw(b) => *b,
        _ => return None,
    })
}

pub fn decode_func(b: [u8; 4]) -> Option<Func> {
    Some(match b {
        [1, 1, 1, _] => Func::Click,
        [1, 2, ..] => Func::Menu,
        [1, 3, ..] => Func::Middle,
        [1, 4, ..] => Func::Forward,
        [1, 5, ..] => Func::Back,
        [1, 6, ..] => Func::ScrollUp,
        [1, 7, ..] => Func::ScrollDown,
        [5, 4, ..] => Func::Dpi(1),
        [5, 1, ..] => Func::Dpi(2),
        [5, 2, ..] => Func::Dpi(3),
        [0, 0, 0, 0] => Func::Disabled,
        [0, 0, 0, k] => Func::Key(crate::device::usage_key(k)),
        [0, mask, 0, k] if mask & 0xf0 == 0 => {
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
        [2, 0, hi, lo] => Func::Media(crate::du::MEDIA.iter().position(|&m| m == (lo as u16 | (hi as u16) << 8))? as u8 + 1),
        _ => return None,
    })
}

fn rate_ms(r: u8) -> u8 {
    [8, 4, 2, 1].get(r as usize).copied().unwrap_or(1)
}

/// The UI profile of configuration `cfg` and key bytes `keys`.
pub fn decode(model: &crate::models::Model, cfg: &[u8], keys: &[u8]) -> DeviceProfile {
    let n = ((cfg[2] & 0xf) as usize).clamp(1, STAGES);
    let mut stages = [[0u8; 5]; 5];
    for (i, s) in stages.iter_mut().enumerate() {
        let (lo, hi) = (cfg[11 + 2 * i], cfg[12 + 2 * i]);
        *s = [(i < n) as u8, lo, hi, lo, hi];
    }
    let buttons = (0..model.slot_count())
        .map(|i| {
            let k = model.slot(i).off as usize * 4;
            keys.get(k..k + 4).map(|b| b.try_into().unwrap()).unwrap_or([0; 4])
        })
        .collect();
    DeviceProfile {
        stage: ((cfg[2] >> 4) as usize).saturating_sub(1).min(n - 1) as u8,
        stages,
        polling_ms: rate_ms(cfg[1] & 7),
        buttons,
        led: [0; 7],
        image: cfg.to_vec(),
    }
}

/// The configuration and key bytes for `p` (`p.image` = the configuration read).
pub fn encode(model: &crate::models::Model, p: &DeviceProfile, keys: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let mut c = p.image.clone();
    c.resize(CFG, 0);
    if rate_ms(c[1] & 7) != p.polling_ms {
        let r = [8, 4, 2, 1].iter().position(|&m| m == p.polling_ms).unwrap_or(3) as u8;
        c[1] = r << 4 | r;
    }
    // the mouse cycles through the first N stages: enabled ones go first
    let on: Vec<usize> = (0..STAGES).filter(|&i| p.stages[i][0] != 0).collect();
    let on = if on.is_empty() { vec![p.stage as usize] } else { on };
    let cur = on.iter().position(|&i| i == p.stage as usize).unwrap_or(0);
    c[2] = ((cur + 1) << 4 | on.len()) as u8;
    for (j, &i) in on.iter().enumerate() {
        c[11 + 2 * j] = p.stages[i][1];
        c[12 + 2 * j] = p.stages[i][2];
    }
    let mut k = keys.to_vec();
    k.resize(4 * KEYS, 0);
    for (i, b) in p.buttons.iter().enumerate() {
        let o = model.slot(i).off as usize * 4;
        if o + 4 <= k.len() {
            k[o..o + 4].copy_from_slice(b);
        }
    }
    (c, k)
}

fn read_all(d: &Device) -> std::io::Result<(Vec<u8>, Vec<u8>)> {
    let mut cfg = Vec::new();
    for i in 0..6 {
        cfg.extend(read(d, 10 + i, 0x40 + i)?);
    }
    let mut keys = Vec::new();
    for i in 0..4 {
        keys.extend(read(d, 5 + i, i)?);
    }
    Ok((cfg, keys))
}

pub fn read_config(d: &Device) -> std::io::Result<DeviceConfig> {
    let (cfg, keys) = read_all(d)?;
    let p = decode(d.found.model, &cfg, &keys);
    // one configuration on the mouse: the UI's other four profiles stay local
    Ok(DeviceConfig { active: 0, profiles: vec![p; 5], macros: vec![[0u8; MACRO_SLOT]; MACRO_SLOTS] })
}

/// The commands that turn the mouse's (`cfg`, `keys`) into (`nc`, `nk`), in the driver's order
/// (SetMouseProfile).
pub fn packets(cfg: &[u8], keys: &[u8], nc: &[u8], nk: &[u8]) -> Vec<[u8; LEN]> {
    let mut out = Vec::new();
    if nk[..4 * KEYS] != keys[..4 * KEYS] {
        out.push(packet(0x80, 0x31, 0, &nk[0..10]));
        out.push(packet(0x81, 0x31, 10, &nk[10..20]));
        out.push(packet(0x82, 0x32, 20, &nk[20..24]));
    }
    if nc[..40] != cfg[..40] || nc[50] != cfg[50] {
        for i in 0..4u8 {
            let o = 10 * i as usize;
            out.push(packet(0x90 + i, 0x36, o as u8, &nc[o..o + 10]));
        }
        out.push(packet(0x96, 0x37, 50, &nc[50..51]));
    }
    out
}

pub fn write_config(d: &Device, new: &DeviceConfig, _current: &DeviceConfig) -> std::io::Result<DeviceConfig> {
    let model = d.found.model;
    let (cfg, keys) = read_all(d)?;
    let mut n = new.profiles[0].clone();
    n.image = cfg.clone();
    let (nc, nk) = encode(model, &n, &keys);
    for p in packets(&cfg, &keys, &nc, &nk) {
        exchange(d, &p)?;
    }
    let back = read_config(d)?;
    let (a, want) = (&back.profiles[0], decode(model, &nc, &nk));
    if (a.stage, a.polling_ms, &a.buttons, a.stages) != (want.stage, want.polling_ms, &want.buttons, want.stages) {
        return Err(std::io::Error::other("read-back differs from what was written"));
    }
    Ok(back)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packets_follow_the_driver() {
        // SetMouseReporteRateData: 03 CHK 50 01 31 35 01 rate
        let p = packet(0x31, 0x35, 1, &[0x33]);
        assert_eq!(&p[..8], &[3, 0xeb, 0x50, 1, 0x31, 0x35, 1, 0x33]);
        assert_eq!(encode_func(&Func::Back), Some([1, 5, 1, 0]));
        assert_eq!(decode_func([5, 4, 0, 0]), Some(Func::Dpi(1)));
        let m = Func::Media(7);
        assert_eq!(encode_func(&m), Some([2, 0, 0, 0xe2]));
        assert_eq!(decode_func([2, 0, 0, 0xe2]), Some(m));
        let c = Func::Combo("CTRL+A".into());
        assert_eq!(decode_func(encode_func(&c).unwrap()), Some(c));
    }

    #[test]
    fn config_round_trips() {
        let model = crate::models::MODELS.iter().find(|m| m.proto == crate::models::Proto::Bydui).unwrap();
        let mut cfg = vec![0u8; CFG];
        cfg[1] = 0x33;
        cfg[2] = 0x24; // stage 2 of 4
        for i in 0..8 {
            cfg[11 + 2 * i] = (16 * (i + 1) - 1) as u8;
        }
        let mut keys = vec![0u8; 40];
        keys[..8].copy_from_slice(&[1, 1, 1, 0, 1, 2, 1, 0]);
        let p = decode(model, &cfg, &keys);
        assert_eq!((p.stage, p.polling_ms, p.stages[3][0], p.stages[4][0]), (1, 1, 1, 0));
        let (nc, nk) = encode(model, &p, &keys);
        assert_eq!((&nc[..], &nk[..]), (&cfg[..], &keys[..4 * KEYS]));
        assert!(packets(&cfg, &keys, &nc, &nk).is_empty());
        let mut q = p.clone();
        q.stages[0][0] = 0;
        q.polling_ms = 8;
        let (nc, nk) = encode(model, &q, &keys);
        assert_eq!((nc[1], nc[2], nc[11]), (0, 0x13, 31));
        let ps = packets(&cfg, &keys, &nc, &nk);
        assert_eq!(ps.len(), 5);
        assert_eq!(&ps[4][..8], &[3, ps[4][1], 0x50, 1, 0x96, 0x37, 50, 0]);
    }
}
