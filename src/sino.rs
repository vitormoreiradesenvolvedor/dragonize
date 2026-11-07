//! Sinowealth mice configured by Redragon's OemDrv.exe ("BY" OEM driver), from
//! the decompiled driver (build 7875e77b, tools/re/fleet/NOTES.md):
//!   command   feature report 5, 6 bytes `05 cmd 00 00 00 00`        (FUN_00413290)
//!   read      command 0x11 / 0x21 / 0x31 (mode 1-3), then GET feature report 4,
//!             520 bytes; the block starts at byte 8                    (FUN_00413720)
//!   write     SET feature report 4, 520 bytes `04 cmd 00 len 00 00 00 00 data`:
//!             0x11.. + len 0x7b = general block (0x8c bytes)          (FUN_00413800)
//!             0x12.. + len 0x50 = key matrix, 20 x 4-byte codes       (FUN_004138b0)
//! General block (FUN_00415260): [2] polling code 1-4 (low nibble), bit 7 = X/Y
//! separate; [3] enabled stages | 1-based position of the active one among
//! them << 4; [4] disabled-stage mask; [5..] one hardware DPI value per stage
//! (two, X then Y, when bit 7 of [2] is set); [0x15..] stage colours. OemDrv
//! always reads the general block first and rewrites it changed, so does this.
//! It never reads the key matrix back: here a reply that does not echo the
//! command is ignored and the matrix starts from the Cfg.ini defaults.
//! Lighting ([0x2d..], per-firmware variants) is kept as the mouse holds it.
//!
//! The "CDrvBY24G" builds (987426b5 / c1111b9b, Proto::SinoBy24g; FUN_00409f30,
//! FUN_0040a1c0, FUN_0040a480, FUN_0040a630, FUN_0040b700): command = feature report 5 of
//! 8 bytes; read = command 0x11 (wired) / 0x21 (2.4 GHz dongle) then GET feature report 8
//! (520 bytes, block at byte 8); write = feature report 8 `08 cmd 00 len 0000 0000 data`,
//! general block len 0x92, matrix 0x50 + 0xa5 after it. General block: [0] 100,
//! [2] rate | 0x80 X/Y separate, [3] enabled stages | 1-based active << 4, then the
//! enabled stages only, compacted: 16-bit DPI values from [5] (Y from [0x15]), colours
//! 3 bytes each from [0x25]. The mouse holds one profile per connection.
//!
//! The "CDrv8832" build ac3d8a3f (M712, Cfg [OPT] MsFw=10; Apply_M_CJ_8832_1 FUN_0040f310,
//! FindDev FUN_0040bc90, SetMode FUN_0040c4f0; Proto::Sino8832): no command report, three
//! feature reports of the usage page 0xff00 interface: 4 = general block (0x50 bytes after the
//! id; [1] low nibble = enabled stage count, [2] low nibble = X/Y separate, [4..12] one DPI
//! code per stage (8), | 0x80 = disabled), 8 = mode ([1] current mode + 1, [2..5] report rate
//! codes 1-4 = 125-1000 Hz per mode, [5] current DPI stage + 1), 6 = key matrix (3 modes x 10
//! 4-byte codes as above, then the macro buffers). The driver reads 4 and 8 back, never 6.

use crate::device::{Device, DeviceConfig, DeviceProfile, MACRO_SLOT, MACRO_SLOTS};
use crate::model::{FireKey, Func};
use crate::models::Model;

pub const GENERAL: usize = 0x8c;
const GENERAL24: usize = 0x92;
const MATRIX: usize = 0x50;
const STAGES: usize = 5;

/// polling code (RATE1-4 = 125/250/500/1000 Hz) <-> report interval in ms
fn rate_to_ms(code: u8) -> u8 {
    match code & 0xf {
        1 => 8,
        2 => 4,
        3 => 2,
        _ => 1,
    }
}

fn ms_to_rate(ms: u8) -> u8 {
    match ms {
        8 => 1,
        4 => 2,
        2 => 3,
        _ => 4,
    }
}

fn by24g(model: &Model) -> bool {
    model.proto == crate::models::Proto::SinoBy24g
}

fn s8832(model: &Model) -> bool {
    model.proto == crate::models::Proto::Sino8832
}

fn s8801(model: &Model) -> bool {
    model.proto == crate::models::Proto::Sino8801
}

fn general_len(model: &Model) -> usize {
    if by24g(model) { GENERAL24 } else { GENERAL }
}

fn cmd(d: &Device, c: u8) -> std::io::Result<()> {
    if by24g(d.found.model) {
        return d.set_feature(&[5, c, 0, 0, 0, 0, 0, 0], 8);
    }
    d.set_feature(&[5, c, 0, 0, 0, 0], 6)
}

fn read_block(d: &Device, c: u8, n: usize) -> std::io::Result<Option<Vec<u8>>> {
    cmd(d, c)?;
    std::thread::sleep(std::time::Duration::from_millis(40));
    if by24g(d.found.model) {
        // no echo of the command: a general block starts with 100 (the driver checks it)
        let r = d.get_feature(8, 520)?;
        return Ok((r[8] == 100).then(|| r[8..8 + n].to_vec()));
    }
    let r = d.get_feature(4, 520)?;
    Ok(if r[1] == c { Some(r[8..8 + n].to_vec()) } else { None })
}

fn write_block(d: &Device, c: u8, len: u8, data: &[u8]) -> std::io::Result<()> {
    let id = if by24g(d.found.model) { 8 } else { 4 };
    let mut pkt = vec![id, c, 0, len, 0, 0, 0, 0];
    pkt.extend_from_slice(data);
    if id == 8 && c & 0xf == 2 {
        pkt.push(0xa5); // FUN_0040a630: after the matrix
    }
    d.set_feature(&pkt, 520)?;
    std::thread::sleep(std::time::Duration::from_millis(40));
    Ok(())
}

fn defaults(model: &Model) -> Vec<u8> {
    let mut m = vec![0u8; MATRIX];
    for i in 0..20 {
        m[4 * i..4 * i + 4].copy_from_slice(&0x150u32.to_le_bytes());
    }
    for (b, &c) in model.buttons.iter().zip(model.defaults) {
        let o = b.off as usize * 4;
        if o + 4 <= MATRIX {
            m[o..o + 4].copy_from_slice(&c.to_le_bytes());
        }
    }
    m
}

fn buttons_of(model: &Model, matrix: &[u8]) -> Vec<[u8; 4]> {
    model.buttons.iter().map(|b| {
        let o = b.off as usize * 4;
        let mut c = [0u8; 4];
        if o + 4 <= matrix.len() {
            c.copy_from_slice(&matrix[o..o + 4]);
        }
        c
    }).collect()
}

/// BY24G general block (only the enabled stages, 16-bit values) -> device profile.
fn decode24(model: &Model, g: &[u8], matrix: &[u8]) -> DeviceProfile {
    let xy = g[2] & 0x80 != 0;
    let n = ((g[3] & 0xf) as usize).clamp(1, STAGES);
    let first = model.dpi_encode(model.dpi_min());
    let mut stages = [[0, first as u8, (first >> 8) as u8, first as u8, (first >> 8) as u8]; STAGES];
    for (i, s) in stages.iter_mut().enumerate().take(n) {
        let (x, y) = (5 + 2 * i, if xy { 0x15 + 2 * i } else { 5 + 2 * i });
        *s = [1, g[x], g[x + 1], g[y], g[y + 1]];
    }
    let stage = ((g[3] >> 4) as usize).clamp(1, n) - 1;
    let mut image = g.to_vec();
    image.extend_from_slice(matrix);
    DeviceProfile { stage: stage as u8, stages, polling_ms: rate_to_ms(g[2]), buttons: buttons_of(model, matrix), led: [0; 7], image }
}

/// FUN_0040b700: the enabled stages compacted from [5], count and active position in [3].
fn encode24(p: &DeviceProfile, mut g: Vec<u8>) -> Vec<u8> {
    let xy = g[2] & 0x80 != 0;
    g[0] = 100;
    g[2] = (g[2] & 0xf0) | ms_to_rate(p.polling_ms);
    g[5..0x25].fill(0);
    let (mut k, mut pos) = (0, 0);
    for (i, s) in p.stages.iter().enumerate() {
        if s[0] == 0 {
            continue;
        }
        g[5 + 2 * k..7 + 2 * k].copy_from_slice(&s[1..3]);
        if xy {
            g[0x15 + 2 * k..0x17 + 2 * k].copy_from_slice(&s[3..5]);
        }
        k += 1;
        if i == p.stage as usize {
            pos = k;
        }
    }
    g[3] = k as u8 | (pos.max(1) as u8) << 4;
    g
}

/// General block + matrix of one mode -> the app's device profile.
fn decode(model: &Model, general: &[u8], matrix: &[u8]) -> DeviceProfile {
    if by24g(model) {
        return decode24(model, general, matrix);
    }
    let xy = general[2] & 0x80 != 0;
    let enabled = |i: usize| general[4] & (1 << i) == 0;
    let mut stages = [[0u8; 5]; 5];
    for (i, s) in stages.iter_mut().enumerate() {
        let (x, y) = if xy { (general[5 + 2 * i], general[6 + 2 * i]) } else { (general[5 + i], general[5 + i]) };
        *s = [enabled(i) as u8, x, 0, y, 0];
    }
    // the active stage is stored as its position among the enabled ones
    let pos = (general[3] >> 4) as usize;
    let mut stage = 0;
    let mut k = 0;
    for i in 0..8 {
        if enabled(i) {
            k += 1;
            if k == pos {
                stage = i;
                break;
            }
        }
    }
    let buttons = buttons_of(model, matrix);
    let mut image = general.to_vec();
    image.extend_from_slice(matrix);
    DeviceProfile { stage: stage.min(STAGES - 1) as u8, stages, polling_ms: rate_to_ms(general[2]), buttons, led: [0; 7], image }
}

/// The device profile put back into its general block and matrix.
pub fn encode(model: &Model, p: &DeviceProfile) -> (Vec<u8>, Vec<u8>) {
    let gl = general_len(model);
    let mut g = p.image.get(..gl).map(<[u8]>::to_vec).unwrap_or_else(|| vec![0; gl]);
    let mut m = p.image.get(gl..gl + MATRIX).map(<[u8]>::to_vec).unwrap_or_else(|| defaults(model));
    for (b, c) in model.buttons.iter().zip(&p.buttons) {
        let o = b.off as usize * 4;
        if o + 4 <= MATRIX {
            m[o..o + 4].copy_from_slice(c);
        }
    }
    if by24g(model) {
        return (encode24(p, g), m);
    }
    let xy = g[2] & 0x80 != 0;
    g[0] = 100; // FUN_00415260 always sets it
    g[2] = (g[2] & 0xf0) | ms_to_rate(p.polling_ms);
    let mut mask = g[4] & !0x1f;
    for i in 0..STAGES {
        let s = &p.stages[i];
        if s[0] == 0 {
            mask |= 1 << i;
        }
        if xy {
            g[5 + 2 * i] = s[1];
            g[6 + 2 * i] = s[3];
        } else {
            g[5 + i] = s[1];
        }
    }
    g[4] = mask;
    let on = (0..8).filter(|&i| mask & (1 << i) == 0).count() as u8;
    let pos = (0..=p.stage as usize).filter(|&i| mask & (1 << i) == 0).count() as u8;
    g[3] = (on & 0xf) | (pos.max(1) << 4);
    (g, m)
}

fn mode_cmd(mode: usize, block: u8) -> u8 {
    ((mode as u8 + 1) << 4) | block
}

/// BY24G: the mode is the connection (Apply_24G: nProIndex 1 = PID, 2 = PID2).
fn mode_of(d: &Device, mode: usize) -> usize {
    if !by24g(d.found.model) {
        return mode;
    }
    let pid = std::fs::read_to_string(d.found.usb.join("idProduct")).ok().and_then(|s| u16::from_str_radix(s.trim(), 16).ok());
    (pid != Some(d.found.model.pid)) as usize
}

pub fn read_config(d: &Device) -> std::io::Result<DeviceConfig> {
    let model = d.found.model;
    if s8832(model) {
        return s8832_read(d);
    }
    if s8801(model) {
        return s8801_read(d);
    }
    let mut profiles = Vec::new();
    for p in 0..5 {
        // the UI has five profiles; the ones past the mouse's modes stay local
        let mode = mode_of(d, p.min(model.modes.max(1) as usize - 1));
        let general = read_block(d, mode_cmd(mode, 1), general_len(model))?
            .ok_or_else(|| std::io::Error::other(format!("no reply reading mode {}", mode + 1)))?;
        // (BY24G drivers never read the matrix back)
        let matrix = if by24g(model) { None } else { read_block(d, mode_cmd(mode, 2), MATRIX)? }.unwrap_or_else(|| defaults(model));
        profiles.push(decode(model, &general, &matrix));
    }
    Ok(DeviceConfig { active: 0, profiles, macros: vec![[0u8; MACRO_SLOT]; MACRO_SLOTS] })
}

pub fn write_config(d: &Device, new: &DeviceConfig, current: &DeviceConfig) -> std::io::Result<DeviceConfig> {
    let model = d.found.model;
    if s8832(model) {
        return s8832_write(d, new);
    }
    if s8801(model) {
        return s8801_write(d, new);
    }
    for mode in 0..(model.modes.max(1) as usize).min(new.profiles.len()) {
        let (n, c) = (&new.profiles[mode], &current.profiles[mode]);
        let (g, m) = encode(model, n);
        let (cg, cm) = encode(model, c);
        let dm = mode_of(d, mode);
        if g != cg {
            write_block(d, mode_cmd(dm, 1), if by24g(model) { GENERAL24 as u8 } else { 0x7b }, &g)?;
        }
        if m != cm {
            write_block(d, mode_cmd(dm, 2), 0x50, &m)?;
        }
    }
    let back = read_config(d)?;
    for mode in 0..(model.modes.max(1) as usize).min(new.profiles.len()) {
        let (a, b) = (&back.profiles[mode], &new.profiles[mode]);
        if (a.stage, a.stages, a.polling_ms) != (b.stage, b.stages, b.polling_ms) {
            return Err(std::io::Error::other("read-back differs from what was written"));
        }
    }
    Ok(back)
}

// ------------------------------------------------------------------ CDrv8832 (M712)

const G8832: usize = 0x51;
const M8832: usize = 0x479;
const KEYS8832: usize = 10;

/// Size of feature report `id` in the interface's report descriptor (with the id byte).
fn feature_len(d: &Device, id: u8) -> Option<usize> {
    feature_size(&std::fs::read(d.found.hidraw_sys().join("device/report_descriptor")).ok()?, id)
}

/// Size of feature report `id` in report descriptor `rd` (with the id byte).
pub fn feature_size(rd: &[u8], id: u8) -> Option<usize> {
    let (mut i, mut cur, mut size, mut count, mut bits) = (0, 0u32, 0u32, 0u32, 0u32);
    while i < rd.len() {
        let p = rd[i];
        let n = [0, 1, 2, 4][(p & 3) as usize];
        let v = rd.get(i + 1..i + 1 + n).map(|b| b.iter().rev().fold(0u32, |a, &x| a << 8 | x as u32)).unwrap_or(0);
        match p & 0xfc {
            0x84 => cur = v,
            0x74 => size = v,
            0x94 => count = v,
            0xb0 if cur == id as u32 => bits += size * count,
            _ => {}
        }
        i += 1 + n;
    }
    (bits > 0).then(|| 1 + (bits as usize).div_ceil(8))
}

fn s8832_decode(model: &Model, g: &[u8], mode: &[u8], m: usize, matrix: &[u8]) -> DeviceProfile {
    let xy = g[2] & 0xf != 0;
    let mut stages = [[0u8; 5]; 5];
    for (i, s) in stages.iter_mut().enumerate() {
        let (x, y) = if xy { (g[4 + 2 * i], g[5 + 2 * i]) } else { (g[4 + i], g[4 + i]) };
        *s = [(x & 0x80 == 0) as u8, x & 0x7f, 0, y & 0x7f, 0];
    }
    let mut image = g.to_vec();
    image.extend_from_slice(mode);
    DeviceProfile {
        stage: (mode.get(5).copied().unwrap_or(1).max(1) - 1).min(STAGES as u8 - 1),
        stages,
        polling_ms: rate_to_ms(mode.get(2 + m).copied().unwrap_or(4)),
        buttons: buttons_of(model, &matrix[m * KEYS8832 * 4..]),
        led: [0; 7],
        image,
    }
}

fn s8832_read(d: &Device) -> std::io::Result<DeviceConfig> {
    let model = d.found.model;
    let g = d.get_feature(4, feature_len(d, 4).unwrap_or(G8832))?;
    let mode = d.get_feature(8, feature_len(d, 8).unwrap_or(9))?;
    // the key matrix is never read back: the Cfg.ini defaults for every mode
    let one = defaults(model);
    let matrix: Vec<u8> = (0..3).flat_map(|_| one[..KEYS8832 * 4].to_vec()).collect();
    let modes = model.modes.max(1) as usize;
    let profiles = (0..5).map(|p| s8832_decode(model, &g, &mode, p.min(modes - 1).min(2), &matrix)).collect();
    let active = (mode[1].max(1) - 1).min(modes as u8 - 1);
    Ok(DeviceConfig { active, profiles, macros: vec![[0u8; MACRO_SLOT]; MACRO_SLOTS] })
}

fn s8832_write(d: &Device, new: &DeviceConfig) -> std::io::Result<DeviceConfig> {
    let model = d.found.model;
    let modes = (model.modes.max(1) as usize).min(3);
    let a = (new.active as usize).min(modes - 1);
    let p = &new.profiles[a];
    // general block: the DPI stages of the active profile (one set on the mouse)
    let gl = feature_len(d, 4).unwrap_or(G8832);
    let mut g = d.get_feature(4, gl)?;
    let xy = g[2] & 0xf != 0;
    let mut on = 0;
    for (i, s) in p.stages.iter().enumerate() {
        let off = if s[0] == 0 { 0x80 } else { on += 1; 0 };
        if xy {
            g[4 + 2 * i] = s[1] & 0x7f | off;
            g[5 + 2 * i] = s[3] & 0x7f | off;
        } else {
            g[4 + i] = s[1] & 0x7f | off;
        }
    }
    for i in STAGES..8 {
        // the driver's three spare stages: disabled
        if xy {
            g[4 + 2 * i] |= 0x80;
            g[5 + 2 * i] |= 0x80;
        } else {
            g[4 + i] |= 0x80;
        }
    }
    g[1] = g[1] & 0xf0 | on.max(1);
    d.set_feature(&g, gl)?;
    std::thread::sleep(std::time::Duration::from_millis(20));
    // mode: current mode, its report rate, the current stage
    let ml = feature_len(d, 8).unwrap_or(9);
    let mut mode = d.get_feature(8, ml)?;
    mode[1] = a as u8 + 1;
    for (m, q) in new.profiles.iter().take(modes).enumerate() {
        mode[2 + m] = ms_to_rate(q.polling_ms);
    }
    if mode.len() > 5 {
        mode[5] = p.stage + 1;
    }
    d.set_feature(&mode, ml)?;
    std::thread::sleep(std::time::Duration::from_millis(20));
    // key matrix: 3 modes x 10 keys, no macros
    let mut m = vec![0u8; feature_len(d, 6).unwrap_or(M8832)];
    m[0] = 6;
    let one = defaults(model);
    for k in 0..3 {
        let q = &new.profiles[k.min(modes - 1)];
        let o = 1 + k * KEYS8832 * 4;
        m[o..o + KEYS8832 * 4].copy_from_slice(&one[..KEYS8832 * 4]);
        for (b, c) in model.buttons.iter().zip(&q.buttons) {
            let i = b.off as usize;
            if i < KEYS8832 {
                m[o + 4 * i..o + 4 * i + 4].copy_from_slice(c);
            }
        }
    }
    let n = m.len();
    d.set_feature(&m, n)?;
    std::thread::sleep(std::time::Duration::from_millis(20));
    let mut back = s8832_read(d)?;
    for (k, q) in new.profiles.iter().enumerate() {
        back.profiles[k].buttons = q.buttons.clone();
    }
    back.active = new.active;
    Ok(back)
}

// ------------------------------------------------------------------ build 7a7c5b20 (Lavawolf M701)
//
// OemDrv 7a7c5b20 (Cfg appdir BY8801; decompile ghidra/out/sino-ms-7a7c5b20.c, NOTES.md "BY8801").
// FindDev FUN_00432a00: VID_258A&PID_1007, usage page 0xff00 usage 1, m_hDevice (feature report 6)
// and m_hGeneral (feature report 4). Get / set FUN_00432f50 / FUN_00432ec0: id, then the data.
// General block (0x99 bytes after the id, Apply FUN_00408e50): [7] >> 4 = report rate code 1-4,
// [0x46] low nibble = enabled stage count, [0x47] >> 4 = sensor index (FUN_0042bd50, 7 = 0x3610), bit 0
// = X/Y separate (0x9800 / 0x3360 only), [0x49..0x59] 16 DPI codes (X/Y pairs when separate) & 0x7f,
// | 0x80 = disabled, [0x5c..0x60] lighting, [0x63..] stage colours, [0x93..0x99] the mouse's password.
// Matrix report 6 (0x47f bytes after the id): [0..0x400] macro buffers, [0x400] current mode + 1,
// [0x401..0x479] 3 modes x 10 dwords (FUN_00408a10: low nibble of the first byte = key number 1-10),
// [0x479..] the password again. The driver never reads the matrix back; the current DPI stage is
// not in either block (the driver keeps what the mouse has).

const G8801: usize = 0x9a;
const M8801: usize = 0x480;

/// FUN_00408430 (keyinfo -> hardware code), little endian, key number nibble cleared.
const MOUSE8801: [(Func, u8); 5] = [(Func::Click, 0xf0), (Func::Menu, 0xf1), (Func::Middle, 0xf2), (Func::Forward, 0xf3), (Func::Back, 0xf4)];

fn encode8801(f: &Func) -> Option<[u8; 4]> {
    if let Some(m) = MOUSE8801.iter().find(|m| &m.0 == f) {
        return Some([0x10, m.1, 0, 0]);
    }
    Some(match f {
        // kind 2: 60 mods usage usage2
        Func::Key(k) => [0x60, 0, crate::device::key_usage(k)?, 0],
        Func::Combo(spec) => {
            let parts: Vec<&str> = spec.split('+').collect();
            let (key, mods) = parts.split_last()?;
            let mut mask = 0;
            for m in mods {
                mask |= crate::device::combo_mask(m)?;
            }
            [0x60, mask, crate::device::key_usage(key)?, 0]
        }
        Func::Dpi(1) => [0x40, 0, 0, 0],
        Func::Dpi(2) => [0x40, 0x20, 0, 0],
        Func::Dpi(3) => [0x40, 0x40, 0, 0],
        Func::Disabled => [0x50, 0x01, 0, 0],
        Func::Raw(b) => [b[0] & 0xf0, b[1], b[2], b[3]],
        _ => return None,
    })
}

fn decode8801(b: [u8; 4]) -> Option<Func> {
    let b = [b[0] & 0xf0, b[1], b[2], b[3]];
    if b[0] == 0x10 {
        if let Some(m) = MOUSE8801.iter().find(|m| m.1 == b[1]) {
            return Some(m.0.clone());
        }
    }
    decode_func_like(b, 0x60).or(match b {
        [0x40, 0, ..] => Some(Func::Dpi(1)),
        [0x40, 0x20, ..] => Some(Func::Dpi(2)),
        [0x40, 0x40, ..] => Some(Func::Dpi(3)),
        [0x50, 0x01, ..] => Some(Func::Disabled),
        _ => None,
    })
}

/// A key / combination code with kind byte `kind` (`kind mods usage 00`).
fn decode_func_like(b: [u8; 4], kind: u8) -> Option<Func> {
    if b[0] != kind || b[3] != 0 || b[2] == 0 {
        return None;
    }
    decode_std([0x21, b[1], b[2], 0])
}

fn s8801_matrix(model: &Model, p: &DeviceProfile) -> Vec<u8> {
    let mut m = vec![0u8; KEYS8832 * 4];
    for i in 0..KEYS8832 {
        m[4 * i..4 * i + 4].copy_from_slice(&0x150u32.to_le_bytes());
    }
    for (b, &c) in model.buttons.iter().zip(model.defaults) {
        let o = b.off as usize * 4;
        if o + 4 <= m.len() {
            m[o..o + 4].copy_from_slice(&c.to_le_bytes());
        }
    }
    for (b, c) in model.buttons.iter().zip(&p.buttons) {
        let o = b.off as usize * 4;
        if o + 4 <= m.len() {
            m[o..o + 4].copy_from_slice(c);
        }
    }
    for i in 0..KEYS8832 {
        m[4 * i] = m[4 * i] & 0xf0 | (i as u8 + 1);
    }
    m
}

fn s8801_decode(g: &[u8], buttons: Vec<[u8; 4]>) -> DeviceProfile {
    let d = &g[1..];
    let xy = d[0x47] & 1 != 0;
    let mut stages = [[0u8; 5]; 5];
    for (i, s) in stages.iter_mut().enumerate() {
        let (x, y) = if xy { (d[0x49 + 2 * i], d[0x4a + 2 * i]) } else { (d[0x49 + i], d[0x49 + i]) };
        *s = [(x & 0x80 == 0) as u8, x & 0x7f, 0, y & 0x7f, 0];
    }
    let first = stages.iter().position(|s| s[0] != 0).unwrap_or(0);
    DeviceProfile { stage: first as u8, stages, polling_ms: rate_to_ms(d[7] >> 4), buttons, led: [0; 7], image: g.to_vec() }
}

fn s8801_read(d: &Device) -> std::io::Result<DeviceConfig> {
    let model = d.found.model;
    let g = d.get_feature(4, feature_len(d, 4).unwrap_or(G8801))?;
    if g.len() < G8801 {
        return Err(std::io::Error::other("general block too short"));
    }
    let profiles = (0..5).map(|_| {
        let b = buttons_of(model, &defaults(model));
        s8801_decode(&g, b)
    }).collect();
    Ok(DeviceConfig { active: 0, profiles, macros: vec![[0u8; MACRO_SLOT]; MACRO_SLOTS] })
}

fn s8801_write(d: &Device, new: &DeviceConfig) -> std::io::Result<DeviceConfig> {
    let model = d.found.model;
    let modes = (model.modes.max(1) as usize).min(3);
    let a = (new.active as usize).min(modes - 1);
    let p = &new.profiles[a];
    // general block, read-modify-write as Apply does: rate, stage count, DPI codes
    let gl = feature_len(d, 4).unwrap_or(G8801);
    let mut g = d.get_feature(4, gl)?;
    if g.len() < G8801 {
        return Err(std::io::Error::other("general block too short"));
    }
    {
        let x = &mut g[1..];
        x[7] = x[7] & 0xf | ms_to_rate(p.polling_ms) << 4;
        let xy = x[0x47] & 1 != 0;
        x[0x49..0x59].fill(0x80);
        let mut on = 0;
        for (i, s) in p.stages.iter().enumerate() {
            let off = if s[0] == 0 { 0x80 } else { on += 1; 0 };
            if xy {
                x[0x49 + 2 * i] = s[1] & 0x7f | off;
                x[0x4a + 2 * i] = s[3] & 0x7f | off;
            } else {
                x[0x49 + i] = s[1] & 0x7f | off;
            }
        }
        x[0x46] = x[0x46] & 0xf0 | (on.max(1) & 0xf);
    }
    d.set_feature(&g, gl)?;
    std::thread::sleep(std::time::Duration::from_millis(100));
    // matrix: no macros, current mode, 3 modes x 10 keys, the password
    let mut m = vec![0u8; feature_len(d, 6).unwrap_or(M8801).max(M8801)];
    m[0] = 6;
    m[1 + 0x400] = a as u8 + 1;
    for k in 0..3 {
        let o = 1 + 0x401 + k * KEYS8832 * 4;
        m[o..o + KEYS8832 * 4].copy_from_slice(&s8801_matrix(model, &new.profiles[k.min(modes - 1)]));
    }
    m[1 + 0x479..1 + 0x47f].copy_from_slice(&g[1 + 0x93..1 + 0x99]);
    let n = m.len();
    d.set_feature(&m, n)?;
    std::thread::sleep(std::time::Duration::from_millis(20));
    let mut back = s8801_read(d)?;
    for (k, q) in new.profiles.iter().enumerate() {
        back.profiles[k].buttons = q.buttons.clone();
        back.profiles[k].stage = q.stage;
    }
    back.active = new.active;
    Ok(back)
}

// ------------------------------------------------------------------ buttons
//
// OemDrv FUN_004140d0 (keyinfo -> hardware code), little endian:
//   11 m        mouse button, m = 01 left 02 right 04 middle 08 forward 10 back
//   12 01 / ff  wheel up / down          21 mods key 00   key / combination
//   31 m t n    m clicked n times every t ms (double click = 31 01 32 02)
//   41 00/01/02 DPI cycle / + / -        50 01            disabled

pub fn encode_func(f: &Func) -> Option<[u8; 4]> {
    if s8801(crate::models::current()) {
        return encode8801(f);
    }
    Some(match f {
        Func::Click => [0x11, 0x01, 0, 0],
        Func::Menu => [0x11, 0x02, 0, 0],
        Func::Middle => [0x11, 0x04, 0, 0],
        Func::Forward => [0x11, 0x08, 0, 0],
        Func::Back => [0x11, 0x10, 0, 0],
        Func::ScrollUp => [0x12, 0x01, 0, 0],
        Func::ScrollDown => [0x12, 0xff, 0, 0],
        Func::Key(k) => [0x21, 0, crate::device::key_usage(k)?, 0],
        Func::Combo(spec) => {
            let parts: Vec<&str> = spec.split('+').collect();
            let (key, mods) = parts.split_last()?;
            let mut mask = 0;
            for m in mods {
                mask |= crate::device::combo_mask(m)?;
            }
            [0x21, mask, crate::device::key_usage(key)?, 0]
        }
        Func::FireKey(fk) if fk.target < 3 => {
            [0x31, [1, 2, 4][fk.target as usize], fk.delay_ms.clamp(1, 255) as u8, fk.times.clamp(1, 255) as u8]
        }
        Func::Dpi(1) => [0x41, 0, 0, 0],
        Func::Dpi(2) => [0x41, 1, 0, 0],
        Func::Dpi(3) => [0x41, 2, 0, 0],
        Func::Disabled => [0x50, 0x01, 0, 0],
        Func::Raw(b) => *b,
        _ => return None,
    })
}

pub fn decode_func(b: [u8; 4]) -> Option<Func> {
    if s8801(crate::models::current()) {
        return decode8801(b);
    }
    decode_std(b)
}

fn decode_std(b: [u8; 4]) -> Option<Func> {
    Some(match b {
        [0x11, 0x01, ..] => Func::Click,
        [0x11, 0x02, ..] => Func::Menu,
        [0x11, 0x04, ..] => Func::Middle,
        [0x11, 0x08, ..] => Func::Forward,
        [0x11, 0x10, ..] => Func::Back,
        [0x12, 0x01, ..] => Func::ScrollUp,
        [0x12, 0xff, ..] => Func::ScrollDown,
        [0x21, 0, k, 0] => Func::Key(crate::device::usage_key(k)),
        [0x21, mask, k, 0] => {
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
        [0x31, m @ (1 | 2 | 4), t, n] => Func::FireKey(FireKey {
            target: match m { 1 => 0, 2 => 1, _ => 2 },
            key: String::new(),
            times: n as u32,
            delay_ms: t as u32,
        }),
        [0x41, 0, ..] => Func::Dpi(1),
        [0x41, 1, ..] => Func::Dpi(2),
        [0x41, 2, ..] => Func::Dpi(3),
        [0x50, 0x01, ..] => Func::Disabled,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s8832_general_decodes() {
        let m = crate::models::by_key("m712").unwrap();
        let mut g = vec![0u8; G8832];
        g[0] = 4;
        g[1] = 4;
        g[4..12].copy_from_slice(&[4, 9, 0x13, 0x1d, 0x80 | 0x27, 0x80, 0x80, 0x80]);
        let mode = [8, 1, 3, 3, 3, 2, 3, 1, 0];
        let p = s8832_decode(m, &g, &mode, 0, &vec![0u8; 3 * KEYS8832 * 4]);
        assert_eq!((p.stage, p.polling_ms, p.stages[1], p.stages[4][0]), (1, 2, [1, 9, 0, 9, 0], 0));
    }

    #[test]
    fn s8801_codes_and_blocks() {
        let m = crate::models::by_key("lavawolf-m701").unwrap();
        assert_eq!(encode8801(&Func::Back), Some([0x10, 0xf4, 0, 0]));
        assert_eq!(decode8801([0x13, 0xf1, 0, 0]), Some(Func::Menu));
        assert_eq!(decode8801([0x67, 0, 0x04, 0]), Some(Func::Key(crate::device::usage_key(4))));
        assert_eq!(decode8801([0x48, 0x40, 0, 0]), Some(Func::Dpi(3)));
        // general: 500 Hz, stages 1..5 = codes 3/5/7/10/14, the 4th disabled
        let mut g = vec![0u8; G8801];
        g[0] = 4;
        g[1 + 7] = 3 << 4;
        g[1 + 0x49..1 + 0x4e].copy_from_slice(&[3, 5, 7, 0x80 | 10, 14]);
        let p = s8801_decode(&g, Vec::new());
        assert_eq!((p.polling_ms, p.stages[1], p.stages[3][0]), (2, [1, 5, 0, 5, 0], 0));
        // matrix: defaults from the Cfg, key number in the low nibble
        let mx = s8801_matrix(m, &DeviceProfile { buttons: vec![[0x50, 1, 0, 0]], ..p });
        assert_eq!(&mx[..8], &[0x51, 1, 0, 0, 0x12, 0xf1, 0, 0]);
    }

    #[test]
    fn general_block_round_trips() {
        let m = crate::models::by_key("m719-rgb-lit").unwrap();
        // 5 stages, stage 3 (index 2) active, hw values 4/8/16/24/40, 500 Hz
        let mut g = vec![0u8; GENERAL];
        g[0] = 100;
        g[2] = 3;
        g[3] = 5 | 3 << 4;
        g[4] = 0xe0;
        g[5..10].copy_from_slice(&[4, 8, 16, 24, 40]);
        let p = decode(m, &g, &defaults(m));
        assert_eq!((p.stage, p.polling_ms), (2, 2));
        assert_eq!(p.stages[4], [1, 40, 0, 40, 0]);
        assert_eq!(decode_func(p.buttons[0]), Some(Func::Click));
        let mut q = p.clone();
        q.stages[1][0] = 0; // disable stage 2: stage 3 becomes the 2nd enabled one
        let (g2, _) = encode(m, &q);
        assert_eq!((g2[3], g2[4]), (4 | 2 << 4, 0xe2));
        let (g3, m3) = encode(m, &p);
        assert_eq!(g3, g);
        assert_eq!(decode(m, &g3, &m3), p);
    }

    #[test]
    fn by24g_block_compacts_enabled_stages() {
        let m = crate::models::by_key("m602-ks").unwrap();
        // FUN_0040b700 layout: 3 enabled stages, the 2nd active, 1000 Hz, 16-bit values
        let mut g = vec![0u8; GENERAL24];
        g[0] = 100;
        g[2] = 4;
        g[3] = 3 | 2 << 4;
        g[5..11].copy_from_slice(&[4, 0, 8, 0, 0x10, 1]);
        let p = decode(m, &g, &defaults(m));
        assert_eq!((p.stage, p.polling_ms), (1, 1));
        assert_eq!(p.stages[2], [1, 0x10, 1, 0x10, 1]);
        assert_eq!(p.stages[3][0], 0);
        assert_eq!(encode(m, &p).0, g);
        let mut q = p.clone();
        q.stages[0][0] = 0; // the active stage moves to the 1st position
        let (g2, _) = encode(m, &q);
        assert_eq!((g2[3], &g2[5..9]), (2 | 1 << 4, &[8, 0, 0x10, 1][..]));
    }

    #[test]
    fn button_codes_match_oemdrv() {
        assert_eq!(u32::from_le_bytes(encode_func(&Func::Middle).unwrap()), 0x411);
        assert_eq!(u32::from_le_bytes(encode_func(&Func::Back).unwrap()), 0x1011);
        assert_eq!(u32::from_le_bytes(encode_func(&Func::Dpi(2)).unwrap()), 0x141);
        assert_eq!(u32::from_le_bytes(encode_func(&Func::Disabled).unwrap()), 0x150);
        // Cfg.ini double click (K 0x01,0x16) = 0x2320131
        assert!(matches!(decode_func(0x2320131u32.to_le_bytes()), Some(Func::FireKey(FireKey { target: 0, times: 2, delay_ms: 50, .. }))));
    }
}
