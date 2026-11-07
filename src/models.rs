//! The mice this app configures. Mostly the Holtek-based family (USB 04d9:xxxx)
//! that Redragon's universal driver RDCfg.exe ("REDRAGON Gaming Mouse.exe")
//! supports, plus the Sinowealth (sino.rs) and CompX (compx.rs) mice of other
//! Redragon drivers (`Proto`). The Holtek ones share the M801 protocol and memory
//! map (device.rs); per model
//! only these differ, each extracted from RDCfg.exe by running its own code in
//! Ghidra's emulator (tools/re/rdcfg, MECHANICS.md "Other models"):
//!   - which bcdDevice revisions of a product id it is (Config.ini Mouse_List)
//!   - button slots and their address in the profile block (FUN_0045d5c0)
//!   - button markers on the product photo (FUN_00406780)
//!   - sensor, i.e. how a DPI stage is encoded (FUN_004314d0/00432150/00457b30)
//!   - DPI slider range (FUN_00436d40) and lighting variant (FUN_00431a30)
//!
//! One model is active at a time (the mouse being configured), kept globally
//! because the encoders in device.rs and model.rs are called from the UI and
//! from the background Apply thread alike.

use std::sync::atomic::{AtomicUsize, Ordering};

#[path = "models_data.rs"]
mod data;
pub use data::MODELS;

/// A button slot: its 4-byte function at `profile base + off`, and where its
/// number is drawn on the product photo (general page box coordinates).
#[derive(Debug)]
pub struct Btn {
    pub off: u16,
    pub mark: Option<(i16, i16)>,
}

#[derive(Debug)]
pub struct Model {
    pub key: &'static str,
    pub name: &'static str,
    pub vid: u16,
    pub pid: u16,
    /// other product ids of the same mouse (CompX: wired / 2.4 GHz dongle)
    pub alt_pids: &'static [u16],
    /// bcdDevice range; (0, 0) = any revision. CompxFeature DEV_n mice sharing a dongle
    /// id: (CID << 8 | MID, same) of the mouse's answer to command 1 (compx.rs `psd`)
    pub bcd: (u16, u16),
    pub sensor: u16,
    /// RDCfg lighting variant: 3 = M801 modes, 0-2 = six-mode firmware, 4 = zones
    pub led: u8,
    /// DPI slider range as sensor table indices
    pub dpi: (u16, u16),
    pub photo: Option<&'static str>,
    pub buttons: &'static [Btn],
    /// wheel up / wheel down
    pub wheel: [Btn; 2],
    pub proto: Proto,
    /// Sino: each button's default 4-byte code (OemDrv Cfg.ini K lines); empty otherwise
    pub defaults: &'static [u32],
    /// Profiles the mouse holds (Sino: OemDrv "modes"; the UI always shows 5)
    pub modes: u8,
    /// Keyboards (Proto::SinoKbd): keys, lighting modes, protocol constants (kbd.rs)
    pub kb: Option<&'static crate::kbd::Kbd>,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Proto {
    /// RDCfg / M801 protocol (device.rs)
    Holtek,
    /// Later single-model RDCfg builds: same, but a whole profile block (and a
    /// macro slot) goes in one 256-byte feature report 4 (device.rs `write_bulk`)
    HoltekBulk,
    /// Sinowealth firmware driven by OemDrv.exe (sino.rs); `Btn::off` = key matrix
    /// index, no wheel rows, `sensor` = this model's own table in SENSORS
    Sino,
    /// OemDrv "CDrvBY24G" builds (sino.rs): command = 8-byte feature report 5, data =
    /// feature report 8, a 0x92-byte general block with 16-bit DPI values; one profile
    /// per connection (wired = mode 1, dongle = mode 2)
    SinoBy24g,
    /// OemDrv "CDrv8832" build (M712, MsFw=10; sino.rs `s8832`): general block, mode and key
    /// matrix in feature reports 4 / 8 / 6 of one interface
    Sino8832,
    /// OemDrv build 7a7c5b20 (Lavawolf M701, appdir BY8801; sino.rs `s8801`): general block in
    /// feature report 4 (0x99 bytes), key matrix + current mode in feature report 6, own key codes
    Sino8801,
    /// CompX firmware (Mouse Drive Beta / HIDUsb.dll, compx.rs); `Btn::off` = key index,
    /// no wheel rows, `sensor` = this model's table (register = x | DPIex << 8)
    Compx,
    /// Old Sinowealth OemDrv builds (Lowerdev.dll): the CompX packets and flash map,
    /// sent as feature report 8 and answered with input report 9 (compx.rs)
    CompxFeature,
    /// Sinowealth keyboards of KeyboardDrv.exe / OemDrv.exe (kbd.rs); `kb` holds the
    /// layout, one id serves many keyboards told apart by their password (`Kbd::psd`)
    SinoKbd,
    /// Evision firmware of the DuiLib drivers (du.rs); `Btn::off` = key index, no wheel rows,
    /// `sensor` = the generator's guess until the mouse reports its own (du.rs `Info`)
    Duilib,
    /// xyui mice (xy.rs): write-only, `Btn::off` = the layout's key_value, no wheel rows,
    /// `defaults` = the layout's function values
    Xyui,
    /// BYDUI mice (bd.rs)
    Bydui,
    /// MosArt mice of the C++Builder drivers (mo.rs)
    Mosart,
    /// libdc mice (dc.rs): write-only property writes
    Libdc,
    /// PixArt mice of PXICtrl3169.dll (pxi.rs): EEPROM rows; `Btn::off` = button number
    Pixart,
}

/// The M801-RGB: RDCfg lists 04d9:fc58 rev 2.00-2.FF as "M912-RGB"; the M801
/// driver's buttons, DPI table and lighting are exactly this entry's.
pub const M801: &str = "m912-rgb";

static CURRENT: AtomicUsize = AtomicUsize::new(usize::MAX);
// tests run in parallel threads: each gets its own current model
#[cfg(test)]
thread_local!(static CURRENT_TEST: std::cell::Cell<usize> = const { std::cell::Cell::new(usize::MAX) });

fn current_index() -> usize {
    #[cfg(test)]
    return CURRENT_TEST.with(|c| c.get());
    #[cfg(not(test))]
    CURRENT.load(Ordering::Relaxed)
}

pub fn by_key(key: &str) -> Option<&'static Model> {
    MODELS.iter().find(|m| m.key == key)
}

pub fn current() -> &'static Model {
    MODELS.get(current_index()).unwrap_or_else(|| by_key(M801).expect("M801 entry"))
}

pub fn set_current(m: &'static Model) {
    let i = MODELS.iter().position(|x| std::ptr::eq(x, m)).expect("registered model");
    #[cfg(test)]
    CURRENT_TEST.with(|c| c.set(i));
    CURRENT.store(i, Ordering::Relaxed);
}

/// The model of a plugged mouse. A revision RDCfg does not list falls back to
/// the first entry of that product id (the M801 for fc58).
pub fn identify(vid: u16, pid: u16, bcd: u16) -> Option<&'static Model> {
    let same_pid = || MODELS.iter().filter(move |m| m.vid == vid && (m.pid == pid || m.alt_pids.contains(&pid)));
    same_pid()
        .find(|m| m.bcd == (0, 0) || (m.proto != Proto::CompxFeature && (m.bcd.0..=m.bcd.1).contains(&bcd)))
        .or_else(|| same_pid().find(|m| m.key == M801))
        .or_else(|| same_pid().next())
}

/// The CompxFeature model of this id answering CID / MID, when several share the id.
pub fn identify_psd(vid: u16, pid: u16, cid: u8, mid: u8) -> Option<&'static Model> {
    let psd = (cid as u16) << 8 | mid as u16;
    MODELS.iter().find(|m| m.vid == vid && (m.pid == pid || m.alt_pids.contains(&pid)) && m.bcd == (psd, psd))
}

/// Whether more than one model answers to this id with a CID / MID (compx.rs `psd` decides).
pub fn needs_psd(vid: u16, pid: u16) -> bool {
    MODELS.iter().filter(|m| m.vid == vid && (m.pid == pid || m.alt_pids.contains(&pid)) && m.bcd != (0, 0)
        && m.proto == Proto::CompxFeature).count() > 1
}

/// The keyboard of this id answering password `psd`, when several share the id.
pub fn identify_kbd(vid: u16, pid: u16, psd: [u8; 6]) -> Option<&'static Model> {
    MODELS.iter().find(|m| m.vid == vid && m.pid == pid && m.kb.is_some_and(|k| k.psd == psd))
}

/// (VID, PID) of every model.
pub fn product_ids() -> Vec<(u16, u16)> {
    let mut v: Vec<(u16, u16)> =
        MODELS.iter().flat_map(|m| std::iter::once(m.pid).chain(m.alt_pids.iter().copied()).map(|p| (m.vid, p))).collect();
    v.sort();
    v.dedup();
    v
}

impl Model {
    pub fn is_m801(&self) -> bool {
        self.key == M801
    }

    pub fn is_bulk(&self) -> bool {
        self.proto == Proto::HoltekBulk
    }

    pub fn is_sino(&self) -> bool {
        matches!(self.proto, Proto::Sino | Proto::SinoBy24g | Proto::Sino8832 | Proto::Sino8801)
    }

    pub fn is_kbd(&self) -> bool {
        self.kb.is_some()
    }

    pub fn is_du(&self) -> bool {
        self.proto == Proto::Duilib
    }

    /// Nothing to read back (xyui): the UI keeps its profiles and sends them.
    pub fn is_xy(&self) -> bool {
        self.proto == Proto::Xyui
    }

    pub fn is_bd(&self) -> bool {
        self.proto == Proto::Bydui
    }

    pub fn is_mo(&self) -> bool {
        self.proto == Proto::Mosart
    }

    pub fn is_dc(&self) -> bool {
        self.proto == Proto::Libdc
    }

    pub fn is_px(&self) -> bool {
        self.proto == Proto::Pixart
    }

    /// Mice whose driver only sends: the app keeps the UI's profiles, nothing is read back.
    pub fn write_only(&self) -> bool {
        self.is_xy() || self.is_mo() || self.is_dc()
    }

    pub fn is_compx(&self) -> bool {
        matches!(self.proto, Proto::Compx | Proto::CompxFeature)
    }

    /// Holtek firmware: wheel up/down are remappable rows after the buttons.
    pub fn has_wheel_rows(&self) -> bool {
        matches!(self.proto, Proto::Holtek | Proto::HoltekBulk)
    }

    /// Number of button rows: the buttons, then wheel up and wheel down
    /// (Sinowealth / CompX mice: the wheel is not remappable).
    pub fn slot_count(&self) -> usize {
        self.buttons.len() + if self.has_wheel_rows() { 2 } else { 0 }
    }

    pub fn slot(&self, i: usize) -> &Btn {
        self.buttons.get(i).unwrap_or_else(|| &self.wheel[(i - self.buttons.len()).min(1)])
    }

    pub fn is_wheel(&self, i: usize) -> bool {
        self.has_wheel_rows() && i >= self.buttons.len()
    }

    fn table(&self) -> &'static [(u32, u16)] {
        if self.is_du() {
            if let Some(t) = crate::du::runtime_table() {
                return t;
            }
        }
        data::SENSORS.iter().find(|(s, _)| *s == self.sensor).map(|(_, t)| *t).unwrap_or(&[])
    }

    /// Every CPI value the DPI slider can take, ascending.
    pub fn dpi_values(&self) -> Vec<u32> {
        if self.is_m801() {
            // the M801 driver's own steps (its pfd index), kept so the slider
            // matches the original pixel for pixel
            let (lo, hi) = (crate::model::dpi_to_index(200), crate::model::dpi_to_index(12400));
            return (lo..=hi).map(crate::model::index_to_dpi).collect();
        }
        // RDCfg's table sometimes gives two steps the same register (e.g. 15900
        // and 16000 on the 2805): only the first one is a distinct setting
        let t = self.table();
        let mut seen = std::collections::HashSet::new();
        let mut v: Vec<u32> = (self.dpi.0 as usize..=self.dpi.1 as usize)
            .filter_map(|i| t.get(i))
            .filter(|e| seen.insert(e.1))
            .map(|e| e.0)
            .collect();
        v.dedup();
        v
    }

    pub fn dpi_min(&self) -> u32 {
        self.dpi_values().first().copied().unwrap_or(200)
    }

    pub fn dpi_max(&self) -> u32 {
        self.dpi_values().last().copied().unwrap_or(12400)
    }

    /// Nearest value the sensor can do.
    pub fn snap_dpi(&self, v: i64) -> u32 {
        let vals = self.dpi_values();
        // ties go up, like the M801's original rounding (6500 -> 6600)
        *vals.iter().min_by_key(|&&c| ((c as i64 - v).abs(), std::cmp::Reverse(c))).unwrap_or(&200)
    }

    /// Position of a CPI value on the slider (0 = lowest).
    pub fn dpi_pos(&self, cpi: u32) -> usize {
        let vals = self.dpi_values();
        vals.iter().enumerate().min_by_key(|(_, c)| (**c as i64 - cpi as i64).abs()).map(|(i, _)| i).unwrap_or(0)
    }

    pub fn dpi_encode(&self, cpi: u32) -> u16 {
        if self.is_m801() {
            return crate::device::dpi_encode(cpi);
        }
        let t = self.table();
        let lo = self.dpi.0 as usize;
        let hi = (self.dpi.1 as usize).min(t.len().saturating_sub(1));
        (lo..=hi).min_by_key(|&i| (t[i].0 as i64 - cpi as i64).abs()).map(|i| t[i].1).unwrap_or(0)
    }

    pub fn dpi_decode(&self, reg: u16) -> u32 {
        if self.is_m801() {
            return crate::device::dpi_decode(reg);
        }
        let t = self.table();
        let hi = (self.dpi.1 as usize).min(t.len().saturating_sub(1));
        (self.dpi.0 as usize..=hi).find(|&i| t[i].1 == reg).map(|i| t[i].0).unwrap_or_else(|| self.dpi_min())
    }

    /// Lighting modes (UI tab indices) this firmware has, None = not supported here.
    pub fn light_modes(&self) -> Option<&'static [usize]> {
        match self.led {
            3 if self.key != "2850" => Some(&[0, 1, 2, 3, 4, 5, 6, 7]),
            0..=2 if self.key != "2850" => Some(&[0, 1, 2, 3, 6, 7]),
            _ => None,
        }
    }

    /// Where this model's profiles live (the M801 keeps the original location).
    pub fn profile_dir(&self, base: &std::path::Path) -> std::path::PathBuf {
        if self.is_m801() { base.to_path_buf() } else { base.join("models").join(self.key) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VID: u16 = 0x04d9; // Holtek

    #[test]
    fn registry_matches_rdcfg() {
        assert_eq!(MODELS.len(), 374);
        let cobra = identify(VID, 0xfc30, 0x0100).unwrap();
        assert_eq!(cobra.key, "m711");
        assert_eq!(cobra.slot_count(), 10);
        assert_eq!(cobra.slot(8).off, 0x6a);
        assert_eq!((cobra.dpi_min(), cobra.dpi_max()), (100, 10000));
        // fc58 rev 0x300 is RDCfg's "2805" (other sensor); unknown revisions are the M801
        assert_eq!(identify(VID, 0xfc58, 0x0300).unwrap().key, "2805");
        assert_eq!(identify(VID, 0xfc58, 0x0001).unwrap().key, M801);
        assert!(identify(VID, 0x1234, 0).is_none());
        // one 2.4 GHz dongle id, five mice: told apart by CID / MID only
        assert!(needs_psd(0x25a7, 0xfa7b));
        assert_eq!(identify_psd(0x25a7, 0xfa7c, 0x33, 0x02).unwrap().key, "m994");
        assert_eq!(identify_psd(0x25a7, 0xfa7b, 0x33, 0x01).unwrap().name, "M693-RGB");
        assert!(!needs_psd(0x25a7, 0xfa68));
    }

    #[test]
    fn m801_entry_is_the_verified_layout() {
        let m = by_key(M801).unwrap();
        let offs: Vec<u16> = (0..m.slot_count()).map(|i| m.slot(i).off).collect();
        assert_eq!(offs, [0x42, 0x46, 0x4a, 0x4e, 0x52, 0x56, 0x5a, 0x5e, 0x62, 0x9a, 0x9e]);
        // RDCfg's sensor table for this entry is the M801's hid.exe table
        let t = m.table();
        for (i, &(cpi, reg)) in t.iter().enumerate().skip(1).take(91) {
            assert_eq!(reg, crate::device::dpi_encode(cpi), "index {i} ({cpi} CPI)");
        }
    }

    #[test]
    fn sensor_codec_round_trips() {
        for m in MODELS.iter().filter(|m| !m.is_m801()) {
            for cpi in m.dpi_values() {
                assert_eq!(m.dpi_decode(m.dpi_encode(cpi)), cpi, "{} {cpi}", m.key);
            }
        }
    }
}
