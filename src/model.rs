//! Profile and macro data, plus INI persistence laid out like the original
//! `RSProfile%d.pfd` / `MacroDB\MacroSet.MSDB` / `%s.MSMACRO` files.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// DPI index encoding taken from the original profiles' `DPIStageValue`:
/// index 4 = 500, 9 = 1000, 0x13 = 2000, 0x1D = 3000, 0x3D = 6200 CPI.
/// Steps are 100 CPI up to 6400 (index 63) and 200 CPI above, up to 12400.
pub const DPI_MIN: u32 = 200;
pub const DPI_MAX: u32 = 12400;

pub const fn dpi_to_index(dpi: u32) -> u32 {
    if dpi <= 6400 { dpi / 100 - 1 } else { 63 + (dpi - 6400) / 200 }
}

pub const fn index_to_dpi(i: u32) -> u32 {
    if i <= 63 { (i + 1) * 100 } else { 6400 + (i - 63) * 200 }
}

/// Snap any value to the nearest CPI the current mouse's sensor can express.
pub fn snap_dpi(v: i64) -> u32 {
    crate::models::current().snap_dpi(v)
}

/// DPI as stored in the profile file: the M801 keeps the original pfd index,
/// other models store their slider position.
fn dpi_file_index(cpi: u32) -> u32 {
    let m = crate::models::current();
    if m.is_m801() { dpi_to_index(cpi) } else { m.dpi_pos(cpi) as u32 }
}

fn dpi_from_file_index(i: u32) -> u32 {
    let m = crate::models::current();
    if m.is_m801() {
        index_to_dpi(i).clamp(DPI_MIN, DPI_MAX)
    } else {
        let v = m.dpi_values();
        v[(i as usize).min(v.len() - 1)]
    }
}

pub const POLLING_HZ_LIST: [u32; 4] = [125, 250, 500, 1000];
pub const MAX_MACROS: usize = 15;
pub const LIGHT_MODES: usize = 8;

/// Fixed palette of the lighting page swatches (images/background/*.png).
pub const SWATCHES: [u32; 7] = [0xff0000, 0xff6600, 0xffff00, 0x00ff00, 0x00ffff, 0x0000ff, 0xac00cb];

#[derive(Clone, Debug, PartialEq)]
pub enum Func {
    Click,
    Menu,
    Middle,
    Forward,
    Back,
    Key(String),
    Combo(String),
    Basic(u8),
    Advanced(u8),
    Media(u8),
    Macro(String),
    FireKey(FireKey),
    Dpi(u8),
    ProfileSwitch,
    Rate(u8),
    LightSwitch,
    ScrollUp,
    ScrollDown,
    Disabled,
    /// Device button code the app cannot name; written back unchanged.
    Raw([u8; 4]),
}

#[derive(Clone, Debug, PartialEq)]
pub struct FireKey {
    /// 0 = left, 1 = right, 2 = middle, 3 = keyboard key
    pub target: u8,
    pub key: String,
    pub times: u32,
    pub delay_ms: u32,
}

impl Default for FireKey {
    fn default() -> Self {
        FireKey { target: 0, key: String::new(), times: 3, delay_ms: 10 }
    }
}

impl Func {
    fn encode(&self) -> String {
        match self {
            Func::Click => "Click".into(),
            Func::Menu => "Menu".into(),
            Func::Middle => "Middle".into(),
            Func::Forward => "Forward".into(),
            Func::Back => "Back".into(),
            Func::Key(k) => format!("Key:{k}"),
            Func::Combo(k) => format!("Combo:{k}"),
            Func::Basic(i) => format!("Basic:{i}"),
            Func::Advanced(i) => format!("Advanced:{i}"),
            Func::Media(i) => format!("Media:{i}"),
            Func::Macro(n) => format!("Macro:{n}"),
            Func::FireKey(f) => format!("FireKey:{}:{}:{}:{}", f.target, f.times, f.delay_ms, f.key),
            Func::Dpi(i) => format!("Dpi:{i}"),
            Func::ProfileSwitch => "ProfileSwitch".into(),
            Func::Rate(i) => format!("Rate:{i}"),
            Func::LightSwitch => "LightSwitch".into(),
            Func::ScrollUp => "ScrollUp".into(),
            Func::ScrollDown => "ScrollDown".into(),
            Func::Disabled => "Disabled".into(),
            Func::Raw(b) => format!("Raw:{:02x}{:02x}{:02x}{:02x}", b[0], b[1], b[2], b[3]),
        }
    }

    fn decode(s: &str) -> Option<Func> {
        let (head, rest) = s.split_once(':').unwrap_or((s, ""));
        let num = || rest.parse::<u8>().ok();
        Some(match head {
            "Click" => Func::Click,
            "Menu" => Func::Menu,
            "Middle" => Func::Middle,
            "Forward" => Func::Forward,
            "Back" => Func::Back,
            "Key" => Func::Key(rest.into()),
            "Combo" => Func::Combo(rest.into()),
            "Basic" => Func::Basic(num()?),
            "Advanced" => Func::Advanced(num()?),
            "Media" => Func::Media(num()?),
            "Macro" => Func::Macro(rest.into()),
            "FireKey" => {
                let mut it = rest.splitn(4, ':');
                Func::FireKey(FireKey {
                    target: it.next()?.parse().ok()?,
                    times: it.next()?.parse().ok()?,
                    delay_ms: it.next()?.parse().ok()?,
                    key: it.next().unwrap_or("").into(),
                })
            }
            "Dpi" => Func::Dpi(num()?),
            "ProfileSwitch" => Func::ProfileSwitch,
            "Rate" => Func::Rate(num()?),
            "LightSwitch" => Func::LightSwitch,
            "ScrollUp" => Func::ScrollUp,
            "ScrollDown" => Func::ScrollDown,
            "Disabled" => Func::Disabled,
            "Raw" if rest.len() == 8 => {
                let mut b = [0u8; 4];
                for i in 0..4 {
                    b[i] = u8::from_str_radix(&rest[i * 2..i * 2 + 2], 16).ok()?;
                }
                Func::Raw(b)
            }
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LightMode {
    pub color: u32,
    /// 0 = low, 1 = mid, 2 = high
    pub brightness: u8,
    pub speed: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    pub dpi: [u32; 5],
    pub dpi_enabled: [bool; 5],
    pub dpi_stage: usize,
    pub polling: usize,
    pub acceleration: u32,
    pub pointer_speed: u32,
    pub scroll_speed: u32,
    pub double_click: u32,
    pub buttons: Vec<Func>,
    pub light_mode: usize,
    pub lights: Vec<LightMode>,
}

/// Slider ranges (min, max) of the general page. Defaults come from the
/// shipped RSProfile1.pfd: Acceleration=1, MouseSensitivity=10,
/// WheelScrollLines=1, DoubleClickSpeed=5.
pub const ACCEL_RANGE: (u32, u32) = (1, 4);
pub const POINTER_RANGE: (u32, u32) = (1, 20);
pub const SCROLL_RANGE: (u32, u32) = (1, 20);
pub const DOUBLE_CLICK_RANGE: (u32, u32) = (1, 10);

/// Button functions after "Restore": the M801 driver's defaults; other models
/// get the usual mouse layout (their vendor defaults are not in RDCfg's data).
fn default_buttons() -> Vec<Func> {
    let m = crate::models::current();
    if m.is_m801() {
        return vec![
            Func::Click,
            Func::Menu,
            Func::Middle,
            Func::FireKey(FireKey::default()),
            Func::Key(String::new()),
            Func::Key(String::new()),
            Func::Key(String::new()),
            Func::Dpi(2),
            Func::Dpi(3),
            Func::ScrollUp,
            Func::ScrollDown,
        ];
    }
    if !m.has_wheel_rows() {
        // the driver's defaults (OemDrv Cfg.ini K lines, CompX KeyParam); no wheel rows
        return m.defaults.iter().map(|&c| crate::device::decode_func(c.to_le_bytes()).unwrap_or(Func::Raw(c.to_le_bytes()))).collect();
    }
    let common = [Func::Click, Func::Menu, Func::Middle, Func::Back, Func::Forward, Func::Dpi(2), Func::Dpi(3)];
    let mut v: Vec<Func> = (0..m.buttons.len()).map(|i| common.get(i).cloned().unwrap_or(Func::Disabled)).collect();
    v.push(Func::ScrollUp);
    v.push(Func::ScrollDown);
    v
}

impl Default for Profile {
    fn default() -> Self {
        let m = crate::models::current();
        let dpi = if m.is_m801() { [500, 1000, 2000, 3000, 6200] } else { [400, 800, 1600, 3200, 6400].map(|d| m.snap_dpi(d)) };
        Profile {
            dpi,
            dpi_enabled: [true; 5],
            dpi_stage: 0,
            polling: 2,
            acceleration: 1,
            pointer_speed: 10,
            scroll_speed: 1,
            double_click: 5,
            buttons: default_buttons(),
            light_mode: 0,
            lights: (0..LIGHT_MODES).map(|_| LightMode { color: 0xff0000, brightness: 0, speed: 0 }).collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Down,
    Up,
    Delay,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MacroStep {
    pub action: Action,
    /// Key name for Down/Up, empty for Delay.
    pub key: String,
    /// Delay in ms (only meaningful for Delay rows).
    pub ms: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayMode {
    /// Run the sequence `loops` times.
    Loop,
    /// Repeat while the button is held.
    WhileHeld,
    /// Repeat until the button is pressed again.
    Toggle,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Macro {
    pub name: String,
    pub steps: Vec<MacroStep>,
    pub record_delay: bool,
    pub use_default_delay: bool,
    pub default_delay: u32,
    pub mode: PlayMode,
    pub loops: u32,
    /// Slot in the mouse's macro memory, once stored there.
    pub slot: Option<u8>,
}

impl Macro {
    pub fn new(name: &str) -> Macro {
        Macro {
            name: name.into(),
            steps: Vec::new(),
            record_delay: true,
            use_default_delay: true,
            default_delay: 10,
            mode: PlayMode::Loop,
            loops: 1,
            slot: None,
        }
    }
    pub fn total_delay(&self) -> u64 {
        self.steps.iter().filter(|s| s.action == Action::Delay).map(|s| s.ms as u64).sum()
    }
    pub fn action_count(&self) -> usize {
        self.steps.iter().filter(|s| s.action != Action::Delay).count()
    }
}

// ---------------------------------------------------------------- INI

pub type Ini = BTreeMap<String, BTreeMap<String, String>>;

pub fn parse_ini(text: &str) -> Ini {
    let mut out = Ini::new();
    let mut sec = String::new();
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        let t = line.trim();
        if t.starts_with('[') && t.ends_with(']') {
            sec = t[1..t.len() - 1].to_string();
            out.entry(sec.clone()).or_default();
        } else if let Some((k, v)) = line.split_once('=') {
            out.entry(sec.clone()).or_default().insert(k.trim().to_string(), v.to_string());
        }
    }
    out
}

fn write_ini(path: &Path, sections: &[(String, Vec<(String, String)>)]) -> std::io::Result<()> {
    let mut s = String::new();
    for (name, kv) in sections {
        s.push_str(&format!("[{name}]\r\n"));
        for (k, v) in kv {
            s.push_str(&format!("{k}={v}\r\n"));
        }
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, s)
}

pub fn config_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config"));
    base.join("m801-rgb").join("M801-RGB Gaming Mouse")
}

impl Profile {
    pub fn save(&self, path: &Path, group: usize) -> std::io::Result<()> {
        let mut g = vec![
            ("GroupID".into(), group.to_string()),
            ("PollingRate".into(), self.polling.to_string()),
            ("Acceleration".into(), self.acceleration.to_string()),
            ("MouseSensitivity".into(), self.pointer_speed.to_string()),
            ("WheelScrollLines".into(), self.scroll_speed.to_string()),
            ("DoubleClickSpeed".into(), self.double_click.to_string()),
            ("DPI".into(), self.dpi_stage.to_string()),
            ("DPIStageNum".into(), "5".into()),
            ("LedMode".into(), self.light_mode.to_string()),
        ];
        // same layout as the original: per stage enable, x index, y index, 1
        let mut hex = String::new();
        for i in 0..5 {
            let idx = dpi_file_index(self.dpi[i]);
            for v in [self.dpi_enabled[i] as u32, idx, idx, 1] {
                hex.push_str(&format!("{:08X}", v.swap_bytes()));
            }
        }
        g.push(("DPIStageValue".into(), hex));
        for (i, l) in self.lights.iter().enumerate() {
            g.push((format!("LedColor{}", i + 1), l.color.to_string()));
            g.push((format!("LedBrightness{}", i + 1), l.brightness.to_string()));
            g.push((format!("LedSpeed{}", i + 1), l.speed.to_string()));
        }
        let mut secs = vec![(format!("GROUP{group}"), g)];
        for (i, f) in self.buttons.iter().enumerate() {
            secs.push((format!("ButtonAssigned{group}_{}", i + 1), vec![("Func".into(), f.encode())]));
        }
        write_ini(path, &secs)
    }

    pub fn load(path: &Path) -> Option<Profile> {
        let ini = parse_ini(&std::fs::read_to_string(path).ok()?);
        let (gname, g) = ini.iter().find(|(k, _)| k.starts_with("GROUP"))?;
        let group = &gname[5..];
        let num = |k: &str, d: u32| g.get(k).and_then(|v| v.trim().parse().ok()).unwrap_or(d);
        let mut p = Profile::default();
        p.polling = (num("PollingRate", 2) as usize).min(3);
        p.acceleration = num("Acceleration", 1).clamp(ACCEL_RANGE.0, ACCEL_RANGE.1);
        p.pointer_speed = num("MouseSensitivity", 10).clamp(POINTER_RANGE.0, POINTER_RANGE.1);
        p.scroll_speed = num("WheelScrollLines", 1).clamp(SCROLL_RANGE.0, SCROLL_RANGE.1);
        p.double_click = num("DoubleClickSpeed", 5).clamp(DOUBLE_CLICK_RANGE.0, DOUBLE_CLICK_RANGE.1);
        p.dpi_stage = (num("DPI", 0) as usize).min(4);
        p.light_mode = (num("LedMode", 0) as usize).min(LIGHT_MODES - 1);
        if let Some(hex) = g.get("DPIStageValue") {
            let words: Vec<u32> = (0..hex.len() / 8)
                .filter_map(|i| u32::from_str_radix(&hex[i * 8..i * 8 + 8], 16).ok().map(u32::swap_bytes))
                .collect();
            for i in 0..5 {
                if let (Some(&en), Some(&idx)) = (words.get(i * 4), words.get(i * 4 + 1)) {
                    p.dpi_enabled[i] = en != 0;
                    p.dpi[i] = dpi_from_file_index(idx);
                }
            }
        }
        for (i, l) in p.lights.iter_mut().enumerate() {
            l.color = num(&format!("LedColor{}", i + 1), 0xff0000);
            l.brightness = num(&format!("LedBrightness{}", i + 1), 0).min(2) as u8;
            l.speed = num(&format!("LedSpeed{}", i + 1), 0).min(2) as u8;
        }
        for i in 0..p.buttons.len() {
            let sec = ini.get(&format!("ButtonAssigned{group}_{}", i + 1));
            if let Some(f) = sec.and_then(|s| s.get("Func")).and_then(|v| Func::decode(v)) {
                p.buttons[i] = f;
            }
        }
        Some(p)
    }
}

impl Macro {
    fn file_for(dir: &Path, name: &str) -> PathBuf {
        let safe: String = name.chars().map(|c| if "/\\:*?\"<>|".contains(c) { '_' } else { c }).collect();
        dir.join(format!("{safe}.MSMACRO"))
    }

    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        let mut kv = vec![
            ("MacroName".into(), self.name.clone()),
            ("RecordDelay".into(), (self.record_delay as u8).to_string()),
            ("UseDefaultDelay".into(), (self.use_default_delay as u8).to_string()),
            ("DefaultDelay".into(), self.default_delay.to_string()),
            ("Mode".into(), (match self.mode { PlayMode::Loop => 0, PlayMode::WhileHeld => 1, PlayMode::Toggle => 2 }).to_string()),
            ("LoopNumber".into(), self.loops.to_string()),
            ("Slot".into(), self.slot.map(|s| s.to_string()).unwrap_or_default()),
            ("Count".into(), self.steps.len().to_string()),
        ];
        for (i, s) in self.steps.iter().enumerate() {
            let v = match s.action {
                Action::Down => format!("D,{}", s.key),
                Action::Up => format!("U,{}", s.key),
                Action::Delay => format!("T,{}", s.ms),
            };
            kv.push((format!("Step{i}"), v));
        }
        write_ini(path, &[("MACRO".into(), kv)])
    }

    pub fn load_from(path: &Path) -> Option<Macro> {
        let ini = parse_ini(&std::fs::read_to_string(path).ok()?);
        let m = ini.get("MACRO")?;
        let num = |k: &str, d: u32| m.get(k).and_then(|v| v.trim().parse().ok()).unwrap_or(d);
        let mut mac = Macro::new(m.get("MacroName")?.as_str());
        mac.record_delay = num("RecordDelay", 1) != 0;
        mac.use_default_delay = num("UseDefaultDelay", 1) != 0;
        mac.default_delay = num("DefaultDelay", 10).min(65535);
        mac.mode = match num("Mode", 0) { 1 => PlayMode::WhileHeld, 2 => PlayMode::Toggle, _ => PlayMode::Loop };
        mac.loops = num("LoopNumber", 1).clamp(1, 255);
        mac.slot = m.get("Slot").and_then(|v| v.trim().parse().ok());
        for i in 0..num("Count", 0) {
            let Some(v) = m.get(&format!("Step{i}")) else { continue };
            let (t, rest) = v.split_once(',').unwrap_or((v.as_str(), ""));
            let step = match t {
                "D" => MacroStep { action: Action::Down, key: rest.into(), ms: 0 },
                "U" => MacroStep { action: Action::Up, key: rest.into(), ms: 0 },
                "T" => MacroStep { action: Action::Delay, key: String::new(), ms: rest.trim().parse().unwrap_or(0) },
                _ => continue,
            };
            mac.steps.push(step);
        }
        Some(mac)
    }
}

pub fn load_macros(dir: &Path) -> Vec<Macro> {
    let db = dir.join("MacroDB");
    let Some(ini) = std::fs::read_to_string(db.join("MacroSet.MSDB")).ok().map(|t| parse_ini(&t)) else {
        return Vec::new();
    };
    let Some(list) = ini.get("MACRO_LIST") else { return Vec::new() };
    (0..MAX_MACROS)
        .filter_map(|i| list.get(&i.to_string()))
        .filter(|n| !n.is_empty())
        .filter_map(|n| Macro::load_from(&Macro::file_for(&db, n)))
        .collect()
}

pub fn save_macros(dir: &Path, macros: &[Macro]) -> std::io::Result<()> {
    let db = dir.join("MacroDB");
    let mut list = Vec::new();
    for i in 0..MAX_MACROS {
        let name = macros.get(i).map(|m| m.name.clone()).unwrap_or_default();
        list.push((i.to_string(), name));
    }
    write_ini(&db.join("MacroSet.MSDB"), &[("MACRO_LIST".into(), list)])?;
    for m in macros {
        m.save_to(&Macro::file_for(&db, &m.name))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dpi_table_matches_original_profile() {
        assert_eq!(index_to_dpi(4), 500);
        assert_eq!(index_to_dpi(9), 1000);
        assert_eq!(index_to_dpi(0x13), 2000);
        assert_eq!(index_to_dpi(0x1D), 3000);
        assert_eq!(index_to_dpi(0x3D), 6200);
        assert_eq!(index_to_dpi(dpi_to_index(12400)), 12400);
        assert_eq!(snap_dpi(6450), 6400);
        assert_eq!(snap_dpi(6500), 6600);
        assert_eq!(snap_dpi(99999), 12400);
    }

    #[test]
    fn profile_round_trip() {
        let dir = std::env::temp_dir().join(format!("m801-test-{}", std::process::id()));
        let mut p = Profile::default();
        p.dpi[2] = 7800;
        p.buttons[4] = Func::Macro("Combo A".into());
        p.buttons[3] = Func::FireKey(FireKey { target: 3, key: "F".into(), times: 7, delay_ms: 33 });
        let f = dir.join("p.pfd");
        p.save(&f, 0).unwrap();
        assert_eq!(Profile::load(&f).unwrap(), p);
        let mut m = Macro::new("x");
        m.steps.push(MacroStep { action: Action::Down, key: "A".into(), ms: 0 });
        m.steps.push(MacroStep { action: Action::Delay, key: String::new(), ms: 15 });
        save_macros(&dir, &[m.clone()]).unwrap();
        assert_eq!(load_macros(&dir), vec![m]);
        let _ = std::fs::remove_dir_all(dir);
    }
}
