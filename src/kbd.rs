//! Sinowealth keyboards configured by Redragon's KeyboardDrv.exe and the keyboard
//! builds of OemDrv.exe (258a:xxxx), from the decompiled drivers (KeyboardDrv 801262a0
//! FUN_004110e0 ApplySetting, OemDrv 4f3eab8b FUN_0048a6d0 CDev916KB::ApplySetting;
//! tools/re/fleet/NOTES.md "Sinowealth keyboards"):
//!   command   feature report 5 `05 CMD ADDR 00 00 00 ..` (its size from the descriptor)
//!   data      feature report 6, 1032 bytes `06 CMD ADDR 00 40 00 00 00 data`; a read is the
//!             command, then GET feature report 6 with the data at byte 8
//!   83 / 03 ProAddr     general block (0x88 bytes KeyboardDrv, 0xfe OemDrv)
//!   84 / 04 KeyAddr     key matrix: 0x100 little-endian u32, one firmware code per matrix
//!                       index (keyinfo_to_hardware_code FUN_00489960, see `KFunc`)
//!   08 RgbTabAddr       the 7-colour table of each lighting mode (written only)
//!   05 81 ..            password: GET feature 5 = `05 01 p0..p5`, the model's Cfg PSD
//! One id (258a:0049 / 010c / 019d) serves dozens of keyboards: the password tells them
//! apart (FUN_004454d0 / FUN_00489100 compare it with Cfg.ini PSD).
//! General block (FUN_00410f70 / FUN_0048a5b0), profile part from byte 8, `o` =
//! LedModeSelectOffset (4): [o] 1 in the custom ("Gaoshou") mode, [o+1] hardware code of
//! the lighting mode, [o+2] custom colour group; from [0x16] two bytes per lighting mode
//! (LedOpt order): colour index 0-6 in the low nibble (RandomVal = colourful), then
//! SpeedHW[speed] << 4 | LightHW[brightness]. Everything else is kept as read.
//! Colour table: 21 zero bytes, then 21 bytes per mode (7 colours, components in
//! RGBIndex order); written whole, as the drivers do, unless the custom mode is active.
//!
//! xyui keyboards (DeviceDriver.exe + XYUI.dll, 1a2c:xxxx; NOTES.md "xyui keyboards"): no
//! settings memory to read, the driver keeps them in its own database. Wired, feature report
//! 7, 8 bytes: lighting `07 f7 MODE-1 COLOUR DIR SPEED BRIGHT 0` (FUN_0040ecc0; mode 4, one RGB
//! colour: `.. R G BRIGHT B`), key slots
//! `07 fa 01 SLOT KEY 01 VALUE 01` (FUN_0040e470, slots 1-10, KEY 0 / VALUE 0 = empty). The
//! app keeps the same KbConfig for them (matrix indexed by HID usage, general block laid out as
//! above) and writes all of it on every Apply.
//!
//! QMK/VIA keyboards (Redragon ships only the VIA definition; tools/re/fleet/gen_via.py): the
//! VIA raw HID protocol, 32-byte output / input reports on the usage page 0xff60 interface.
//! 04 L R C -> keycode (big endian) of layer L row R col C, 05 L R C hi lo sets it; 07 CH ID v.. /
//! 08 CH ID -> v.. a lighting value (channel and ids from the definition: brightness, effect,
//! effect speed 0-255, colour = hue, saturation), 09 CH saves the channel. Matrix index =
//! row * cols + col, codes = QMK keycodes (layer 0). The general block holds the lighting as for
//! the other keyboards (levels 0-4 scaled to 0-255).
//!
//! HidServ keyboards (InstallShield "<model>.exe" + HidServ.dll, Sonix 0c45:5004 / 5204 / 5304;
//! tools/re/fleet/gen_hidserv.py, NOTES.md "HidServ keyboards"): 64-byte output / input reports on
//! the usage page 0xff1c usage 0x92 interface, `04 SUM_lo SUM_hi CMD LEN OFF_lo OFF_hi 00 data`
//! (SUM = bytes 3-63 added up), each answered with the same layout, status in byte 7 (ff / fe =
//! refused), data from byte 8. 01 / 02 open / close a write, 03 reads the 0x2c-byte config
//! (active profile at 0x0a), 05 / 06 read / write the lighting block of 0x2a bytes per profile
//! (mode, brightness 0-5, speed 5-0, direction, 1 = colourful, R G B), 07 / 08 read / write the key
//! table, 0x7e entries of 3 bytes at profile * stride, 0x38 bytes per packet (`hs_code`). The
//! general block holds the lighting as for the other keyboards; its bytes 0-4 keep what the
//! keyboard reported (profile, R G B, colourful).
//!
//! Jieli / Witmod keyboards (Qt app + witmodSdk.dll, 4a4c:e987 / e989; tools/re/fleet/gen_jieli.py,
//! NOTES.md "Jieli keyboards"): 64-byte output reports with id 1 on USB interface 2. One id for
//! every keyboard: it answers `01 0d` with its firmware name (byte 1 = 0d, text from byte 6, up to
//! the first comma; CWitmodHid_GetHidAndFirmwareInfoStu), which names the model. The SDK maps the
//! app's command n to n + 1 on the wire (CWitmodHid_HidWriteBuff). Lighting only, write-only:
//! `01 07 00 00 00 0e TYPE BRIGHT-1 SPEED-1 R G B DIR MODETYPE ON UPDATE COLOURFUL ..` (FUN_004e3af0),
//! milliseconds & 0xffff in bytes 62-63.

//!
//! MosArt keyboards K598-KNS (062a:8566) / K599 (062a:8556), C++Builder + JVCL drivers (decompile
//! ghidra/out/k598kns.c, NOTES.md "K598-KNS / K599"): 64-byte output report 8 `08 CMD ADDR_lo ADDR_hi
//! LEN data` (FUN_004371a8), each answered by input report 8 echoing CMD (FUN_00440cac). 1 sync, 2 begin
//! write, 3 read (<= 0x3b bytes), 5 write, 7 end. Memory: area 0 at 0x000 (0x18 bytes, also feature 0x14),
//! area 1 at 0x100 (0x6a, feature 0x15: per-key flags of the custom mode, 8 x 13), area 2 at 0x180
//! (4 x 0x100 function blocks). Each area ends with a u16 checksum 0x8032 + sum of the bytes (FUN_00437d9c).
//! Area 0: [1] G-mode bits, [2] brightness, [5] colour (0-7 fixed colours, 8 colourful), [6] speed,
//! [8] effect group 0-5, [9 + group] its variant 0-2 (effect = group * 3 + variant, table DAT_006b68ec),
//! [0xf] custom mode. The app edits effect, colour, brightness and speed; the rest is kept as read. The
//! general block holds the lighting as for the other keyboards, then the two areas as read.
//!
//! Magnetic K729RGB-M (19f5:af08 wired / af28 receiver), MFC "Redragon Driver.exe" + mui.dll (decompile
//! ghidra/out/k729m.c, NOTES.md "K729RGB-M"): interface usage page 0xff00 usage 1, 65-byte output report
//! `00 55 CMD 00 SUM LEN OFF_lo OFF_hi 00 DATA[0x38]`, SUM = the low byte of the sum of bytes 5.. (FUN_00468680
//! / FUN_00468890); reads are answered `aa CMD .. LEN OFF_lo OFF_hi 00 DATA`. 1 begin, 2 end, 4 active profile
//! (answer byte 8), 5 / 6 read / write the profile's 0x20-byte settings at profile << 6 (FUN_0042cd50):
//! [4] & 0xf report rate, [8] light mode, [9] brightness 0..100, [10] 4 - speed, [11] direction,
//! [12] colourful, [13] colour index, [14..17] R G B. Modes (FUN_00437c70, names 661..684 of 1033.lan): 0x80
//! music, 0 per-key, 1..22 effects, 0xff off; config_func bits 1 brightness, 2 speed, 0x10 colourful, 0x20
//! colour. The general block holds the lighting as for the other keyboards, then the 0x20 bytes as read and
//! the profile number.
use crate::device::{Device, Found};

pub const MATRIX: usize = 0x100;
const BYK_MATRIX: usize = 0x7e;
const DATA_LEN: usize = 0x408;
const PROFILE: usize = 8;
const PAIRS: usize = 0x16;
pub const COLORS: usize = 7;

/// KeyboardDrv's default colour table of a mode (profile init, 0x0043f410)
pub const DEFAULT_PALETTE: [u32; COLORS] = [0xff0000, 0x0000ff, 0x00ff00, 0x8000ff, 0xfac603, 0xfa036a, 0xfa00fa];

#[derive(Debug)]
pub struct Key {
    /// rectangle on the keyboard photo
    pub x: i16,
    pub y: i16,
    pub w: i16,
    pub h: i16,
    pub matrix: u8,
    /// firmware code of the driver's default function (u32::MAX = not expressible)
    pub default: u32,
}

#[derive(Debug)]
pub struct LedOpt {
    pub hw: u8,
    pub name: &'static str,
    pub speed: bool,
    pub light: bool,
    pub color: bool,
    pub random: bool,
    /// xyui: the mode takes one RGB colour (attribute 0x10 / 0x80), sent in the light packet
    pub rgb: bool,
}

/// HidServ key table layout (the driver's per-profile stride, 0x17a or 0x200).
#[derive(Clone, Copy, Debug)]
pub struct Hs {
    pub stride: u16,
}

pub const HS_ENTRIES: usize = 0x7e;

/// Where a VIA keyboard keeps its settings (from its definition).
#[derive(Clone, Copy, Debug)]
pub struct Via {
    pub cols: u8,
    /// lighting channel and value ids (0 = absent)
    pub ch: u8,
    pub bright: u8,
    pub effect: u8,
    pub speed: u8,
    pub color: u8,
}

#[derive(Debug)]
pub struct Kbd {
    /// general block length
    pub glen: u16,
    pub pro: u8,
    pub key: u8,
    pub rgb: u8,
    pub led_off: u8,
    /// "BYK901" OemDrv builds: key matrix through feature report 9, their own key codes
    /// (`byk_code`), lighting not decoded (`leds` empty)
    pub byk: bool,
    /// xyui keyboard (`xy_code`, `write_config`): write-only, keys by HID usage
    pub xy: bool,
    /// QMK/VIA keyboard (`via_code`, `read_config`, `write_config`)
    pub via: Option<Via>,
    /// HidServ keyboard (`hs_code`, `read_config`, `write_config`)
    pub hs: Option<Hs>,
    /// Jieli keyboard: the firmware names it answers to `01 0d` (`jl_name`); lighting only
    pub jl: Option<&'static [&'static str]>,
    /// duilib keyboard (Evision firmware, du.rs transport; `du_code`, `du_read`, `du_packets`)
    pub du: bool,
    /// Shinetek (SOUI) build of the xyui protocol (`xy`): colour 1-based with the driver's
    /// per-mode reorder (`st_colour`), direction 1, media keys not mapped
    pub st: bool,
    /// MosArt keyboard (K598-KNS / K599; `mk_read`, `mk_write`): lighting only
    pub mk: bool,
    /// magnetic keyboard of the MFC "Redragon Driver.exe" (K729RGB-M; `mg_read`, `mg_write`): lighting only
    pub mg: bool,
    /// password the keyboard answers to 05 81 (Cfg.ini PSD)
    pub psd: [u8; 6],
    pub keys: &'static [Key],
    pub leds: &'static [LedOpt],
    pub light: &'static [u8],
    pub speed: &'static [u8],
    pub rgbi: [u8; 3],
    pub random: u8,
    /// photo size
    pub photo_w: u16,
    pub photo_h: u16,
}

impl Kbd {
    /// Firmware code of `f` for this keyboard.
    pub fn code(&self, f: &KFunc) -> u32 {
        if self.byk {
            byk_code(f)
        } else if self.xy {
            if self.st && matches!(f, KFunc::Media(_)) { u32::MAX } else { xy_code(f) }
        } else if self.via.is_some() {
            via_code(f)
        } else if self.hs.is_some() {
            hs_code(f)
        } else if self.du {
            du_code(f)
        } else {
            f.encode()
        }
    }

    pub fn func(&self, c: u32) -> KFunc {
        if self.byk {
            byk_func(c)
        } else if self.xy {
            xy_func(c)
        } else if self.via.is_some() {
            via_func(c)
        } else if self.hs.is_some() {
            hs_func(c)
        } else if self.du {
            du_func(c)
        } else {
            KFunc::decode(c)
        }
    }

    /// Key combinations can be assigned.
    pub fn combos(&self) -> bool {
        !self.xy && self.hs.is_none()
    }

    /// Mouse buttons can be assigned (VIA: their keycodes changed between protocol versions;
    /// HidServ: left / right / middle only).
    pub fn mouse_keys(&self) -> bool {
        !self.xy && self.via.is_none() && self.hs.is_none()
    }

    /// Nothing to read back: the app keeps the settings and sends them (xyui, Jieli).
    pub fn write_only(&self) -> bool {
        self.xy || self.jl.is_some()
    }

    /// Key entries the app may leave as the keyboard has them (u32::MAX in the matrix).
    fn keeps_unset(&self) -> bool {
        self.via.is_some() || self.hs.is_some() || self.du
    }

    /// The colours of mode `l` can be changed (xyui: fixed colours, except one RGB colour).
    pub fn colour_picker(&self, l: &LedOpt) -> bool {
        !self.mk && !self.mg && (!self.xy || (l.hw == 4 && l.rgb))
    }

    pub fn default_palette(&self) -> [u32; COLORS] {
        if self.xy {
            XY_PALETTE
        } else if self.mk {
            MK_PALETTE
        } else {
            DEFAULT_PALETTE
        }
    }

    /// Hardware code of the custom (per-key colour) mode, when the keyboard has one.
    pub fn custom_hw(&self) -> Option<u8> {
        self.leds.iter().any(|l| l.hw == 21).then_some(21)
    }

    pub fn custom_index(&self) -> Option<usize> {
        self.leds.iter().position(|l| Some(l.hw) == self.custom_hw())
    }
}

/// What a key does, as the firmware codes it (wire bytes [type, 0, b2, b3]).
#[derive(Clone, Debug, PartialEq)]
pub enum KFunc {
    /// keyboard usage (0xe0-0xe7 = modifier) with modifier mask
    Key { usage: u8, mods: u8 },
    /// 0 left, 1 right, 2 middle, 3 back, 4 forward
    Mouse(u8),
    /// consumer page usage
    Media(u16),
    Fn,
    Disabled,
    Raw(u32),
}

/// Media keys offered in the menu (usage, name).
pub const MEDIA: [(u16, &str); 11] = [
    (0xcd, "Play/Pause"), (0xb7, "Stop"), (0xb6, "Previous"), (0xb5, "Next"), (0xe9, "Volume +"),
    (0xea, "Volume -"), (0xe2, "Mute"), (0x183, "Media player"), (0x18a, "Mail"), (0x192, "Calculator"),
    (0x223, "Browser"),
];

impl KFunc {
    pub fn decode(c: u32) -> KFunc {
        let b = c.to_le_bytes();
        match b[0] {
            _ if c == 0 => KFunc::Disabled,
            0x20 if c == 0x20 => KFunc::Fn,
            0 if b[3] != 0 => KFunc::Key { usage: b[3], mods: b[2] },
            6 if (0xe0..=0xe7).contains(&b[3]) && b[1] == 0 && b[2] == 0 => KFunc::Key { usage: b[3], mods: 0 },
            7 if (0xf0..=0xf4).contains(&b[3]) && b[1] == 0 && b[2] == 0 => KFunc::Mouse(b[3] - 0xf0),
            4 if b[1] == 0 => KFunc::Media((b[2] as u16) << 8 | b[3] as u16),
            _ => KFunc::Raw(c),
        }
    }

    pub fn encode(&self) -> u32 {
        match *self {
            KFunc::Key { usage, mods } if (0xe0..=0xe7).contains(&usage) && mods == 0 => (usage as u32) << 24 | 6,
            KFunc::Key { usage, mods } => (usage as u32) << 24 | (mods as u32) << 16,
            KFunc::Mouse(n) => (0xf0 + n.min(4) as u32) << 24 | 7,
            KFunc::Media(u) => ((u & 0xff) as u32) << 24 | ((u >> 8) as u32) << 16 | 4,
            KFunc::Fn => 0x20,
            KFunc::Disabled => 0,
            KFunc::Raw(c) => c,
        }
    }
}

/// Key codes of the "BYK901" builds (OemDrv 6fccdd79 / c755de9a keyinfo_to_hardware_code
/// FUN_00402b30): key [0, mods, usage, 2nd usage], Fn 0x00b00000, mouse [0x10, f0+n, 0, 0]
/// (its f3/f4 swapped against KeyboardDrv's for the same menu entry), consumer
/// [1, 0, usage hi, usage lo].
fn byk_code(f: &KFunc) -> u32 {
    match *f {
        KFunc::Key { usage, mods } => (usage as u32) << 16 | (mods as u32) << 8,
        KFunc::Mouse(n) => (0xf0 + [0, 1, 2, 4, 3][n.min(4) as usize]) << 8 | 0x10,
        KFunc::Media(u) => ((u & 0xff) as u32) << 24 | ((u >> 8) as u32) << 16 | 1,
        KFunc::Fn => 0xb00000,
        KFunc::Disabled => 0,
        KFunc::Raw(c) => c,
    }
}

fn byk_func(c: u32) -> KFunc {
    let b = c.to_le_bytes();
    match b[0] {
        _ if c == 0 => KFunc::Disabled,
        _ if c == 0xb00000 => KFunc::Fn,
        0 if b[2] != 0 && b[3] == 0 => KFunc::Key { usage: b[2], mods: b[1] },
        0x10 if (0xf0..=0xf4).contains(&b[1]) && b[2] == 0 && b[3] == 0 => KFunc::Mouse([0, 1, 2, 4, 3][(b[1] - 0xf0) as usize]),
        1 if b[1] == 0 => KFunc::Media((b[2] as u16) << 8 | b[3] as u16),
        _ => KFunc::Raw(c),
    }
}

/// xyui key values: HID usage, 1 = disabled, e8-f9 consumer functions (XYUI.dll
/// KeyFuncControl::InitItemInfo, type 2), here with their consumer usage.
const XY_MEDIA: [(u8, u16); 18] = [
    (0xe8, 0x183), (0xe9, 0xea), (0xea, 0xe9), (0xeb, 0xe2), (0xec, 0xb7), (0xed, 0xb6), (0xee, 0xcd),
    (0xef, 0xb5), (0xf0, 0x18a), (0xf1, 0x223), (0xf2, 0x192), (0xf3, 0x224), (0xf4, 0x225), (0xf5, 0x226),
    (0xf6, 0x227), (0xf7, 0x221), (0xf8, 0x22a), (0xf9, 0x194),
];

/// The xyui colour buttons in COLOUR order (colourful = 7).
pub const XY_PALETTE: [u32; COLORS] = [0xff0000, 0x00ff00, 0x0000ff, 0xffff00, 0xff00ff, 0x00ffff, 0xffffff];
/// MosArt keyboards: the first 7 fixed colours (DAT_006b6854; the 8th is white)
pub const MK_PALETTE: [u32; COLORS] = [0xff0000, 0xff5500, 0xffff00, 0x00ff00, 0x00ffff, 0x0000ff, 0xff00ff];

fn xy_code(f: &KFunc) -> u32 {
    match *f {
        KFunc::Key { usage, .. } => usage as u32,
        KFunc::Media(u) => XY_MEDIA.iter().find(|m| m.1 == u).map_or(u32::MAX, |m| m.0 as u32),
        KFunc::Disabled => 1,
        KFunc::Raw(c) => c,
        KFunc::Mouse(_) | KFunc::Fn => u32::MAX,
    }
}

fn xy_func(c: u32) -> KFunc {
    match c {
        1 => KFunc::Disabled,
        _ if c > 0xff => KFunc::Raw(c),
        _ => XY_MEDIA.iter().find(|m| m.0 as u32 == c).map_or(KFunc::Key { usage: c as u8, mods: 0 }, |m| KFunc::Media(m.1)),
    }
}

/// QMK media keycodes (KC_AUDIO_MUTE ..) with their consumer usage.
const VIA_MEDIA: [(u16, u16); 18] = [
    (0xa8, 0xe2), (0xa9, 0xe9), (0xaa, 0xea), (0xab, 0xb5), (0xac, 0xb6), (0xad, 0xb7), (0xae, 0xcd),
    (0xaf, 0x183), (0xb1, 0x18a), (0xb2, 0x192), (0xb3, 0x194), (0xb4, 0x221), (0xb5, 0x223), (0xb6, 0x224),
    (0xb7, 0x225), (0xb8, 0x226), (0xb9, 0x227), (0xba, 0x22a),
];

/// QMK keycode: basic keycode (HID usage) with modifier bits 8-11 (ctrl shift alt gui),
/// bit 12 = right-hand ones; KFunc mods: low nibble left, high nibble right.
fn via_code(f: &KFunc) -> u32 {
    match *f {
        KFunc::Key { usage, mods } if mods & 0xf0 != 0 => 0x1000 | ((mods >> 4) as u32) << 8 | usage as u32,
        KFunc::Key { usage, mods } => ((mods & 0xf) as u32) << 8 | usage as u32,
        KFunc::Media(u) => VIA_MEDIA.iter().find(|m| m.1 == u).map_or(u32::MAX, |m| m.0 as u32),
        KFunc::Disabled => 0,
        KFunc::Raw(c) => c,
        KFunc::Mouse(_) | KFunc::Fn => u32::MAX,
    }
}

fn via_func(c: u32) -> KFunc {
    let (usage, m) = ((c & 0xff) as u8, (c >> 8) & 0xf);
    match c {
        0 => KFunc::Disabled,
        _ if c > 0x1fff => KFunc::Raw(c),
        _ if c & 0x1f00 == 0 => VIA_MEDIA.iter().find(|v| v.0 as u32 == c).map_or(KFunc::Key { usage, mods: 0 }, |v| KFunc::Media(v.1)),
        _ if c & 0x1000 != 0 => KFunc::Key { usage, mods: (m as u8) << 4 },
        _ => KFunc::Key { usage, mods: m as u8 },
    }
}

/// HidServ key entry [b0, b1, b2] as b0 | b1 << 8 | b2 << 16 (FUN_004378a0): key [02 02 usage],
/// modifier [02 01 mask], mouse button [01 01 mask], wheel [01 05 01/ff], consumer
/// [03 usage lo, usage hi], macro [05 01 n], disabled 0.
fn hs_code(f: &KFunc) -> u32 {
    match *f {
        KFunc::Key { usage, mods: 0 } if (0xe0..=0xe7).contains(&usage) => 0x0102 | 1 << (16 + usage - 0xe0),
        KFunc::Key { usage, mods: 0 } => 0x0202 | (usage as u32) << 16,
        KFunc::Mouse(n) if n < 3 => 0x0101 | 1 << (16 + n),
        KFunc::Media(u) => 3 | (u as u32 & 0xffff) << 8,
        KFunc::Disabled => 0,
        KFunc::Raw(c) => c,
        KFunc::Key { .. } | KFunc::Mouse(_) | KFunc::Fn => u32::MAX,
    }
}

fn hs_func(c: u32) -> KFunc {
    let [a, b, v, _] = c.to_le_bytes();
    match (a, b) {
        _ if c == 0 => KFunc::Disabled,
        _ if c > 0xffffff => KFunc::Raw(c),
        (2, 2) => KFunc::Key { usage: v, mods: 0 },
        (2, 1) if v.count_ones() == 1 => KFunc::Key { usage: 0xe0 + v.trailing_zeros() as u8, mods: 0 },
        (1, 1) if matches!(v, 1 | 2 | 4) => KFunc::Mouse(v.trailing_zeros() as u8),
        (3, _) => KFunc::Media((c >> 8) as u16),
        _ => KFunc::Raw(c),
    }
}

/// duilib key entry [type, p1, p2] as type | p1 << 8 | p2 << 16 (du.rs, Keyboard.json
/// KeyList): key [20 modifier mask, usage] (a modifier alone: [20 mask 00]), mouse button
/// [10 mask], consumer [30 usage lo, usage hi], Fn [a0 01], macro keys [d0 a2 n], disabled 0.
fn du_code(f: &KFunc) -> u32 {
    match *f {
        KFunc::Key { usage, mods: 0 } if (0xe0..=0xe7).contains(&usage) => 0x20 | 1 << (8 + usage - 0xe0),
        KFunc::Key { usage, mods } => 0x20 | (mods as u32) << 8 | (usage as u32) << 16,
        KFunc::Mouse(n) if n < 5 => 0x10 | 1 << (8 + n),
        KFunc::Media(u) => 0x30 | (u as u32 & 0xffff) << 8,
        KFunc::Fn => 0x01a0,
        KFunc::Disabled => 0,
        KFunc::Raw(c) => c,
        KFunc::Mouse(_) => u32::MAX,
    }
}

fn du_func(c: u32) -> KFunc {
    let [a, b, v, _] = c.to_le_bytes();
    match a {
        _ if c == 0 => KFunc::Disabled,
        _ if c > 0xffffff => KFunc::Raw(c),
        0x20 if v == 0 && b.count_ones() == 1 => KFunc::Key { usage: 0xe0 + b.trailing_zeros() as u8, mods: 0 },
        0x20 if v != 0 => KFunc::Key { usage: v, mods: b },
        0x10 if b.count_ones() == 1 && b < 0x20 => KFunc::Mouse(b.trailing_zeros() as u8),
        0x30 => KFunc::Media((c >> 8) as u16),
        0xa0 if c == 0x01a0 => KFunc::Fn,
        _ => KFunc::Raw(c),
    }
}

/// Lighting settings of one mode.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Light {
    /// colour index 0-6, or `COLORS` = colourful
    pub color: u8,
    pub speed: u8,
    pub bright: u8,
}

/// What the keyboard holds (blocks as read, edited in place).
#[derive(Clone, Debug, PartialEq)]
pub struct KbConfig {
    pub general: Vec<u8>,
    pub matrix: Vec<u32>,
}

impl KbConfig {
    pub fn blank(kb: &Kbd) -> KbConfig {
        // VIA: no default keymap known, HidServ: Fn / macro entries; u32::MAX = keep what the
        // keyboard has
        let mut matrix = vec![if kb.keeps_unset() { u32::MAX } else { 0u32 }; MATRIX];
        for k in kb.keys {
            if k.default != u32::MAX {
                matrix[k.matrix as usize] = k.default;
            }
        }
        let mut c = KbConfig { general: vec![0; kb.glen as usize], matrix };
        // never "everything off": first ordinary mode, full brightness, mid speed
        for i in 0..kb.leds.len() {
            let speed = (kb.speed.len() / 2) as u8;
            c.set_light(kb, i, Light { color: 0, speed, bright: kb.light.len().saturating_sub(1) as u8 });
        }
        if let Some(i) = kb.leds.iter().position(|l| l.hw != 0 && Some(l.hw) != kb.custom_hw()) {
            c.set_mode(kb, i, None);
        }
        c
    }

    fn p(&self, i: usize) -> u8 {
        self.general.get(PROFILE + i).copied().unwrap_or(0)
    }

    fn set_p(&mut self, i: usize, v: u8) {
        if let Some(b) = self.general.get_mut(PROFILE + i) {
            *b = v;
        }
    }

    /// Index of the active lighting mode in `kb.leds`.
    pub fn mode(&self, kb: &Kbd) -> usize {
        let hw = self.p(kb.led_off as usize + 1);
        kb.leds.iter().position(|l| l.hw == hw).unwrap_or(0)
    }

    pub fn set_mode(&mut self, kb: &Kbd, i: usize, custom: Option<usize>) {
        let Some(l) = kb.leds.get(i) else { return };
        self.set_p(kb.led_off as usize + 1, l.hw);
        self.set_p(kb.led_off as usize, (custom == Some(i)) as u8);
    }

    pub fn light(&self, kb: &Kbd, i: usize) -> Light {
        let (a, b) = (self.p(PAIRS + 2 * i), self.p(PAIRS + 2 * i + 1));
        let color = if a & 0xf == kb.random { COLORS as u8 } else { (a & 0xf).min(COLORS as u8 - 1) };
        let find = |t: &[u8], v: u8| t.iter().position(|&x| x == v).unwrap_or(0) as u8;
        Light { color, speed: find(kb.speed, b >> 4), bright: find(kb.light, b & 0xf) }
    }

    pub fn set_light(&mut self, kb: &Kbd, i: usize, l: Light) {
        let a = self.p(PAIRS + 2 * i);
        let c = if l.color as usize >= COLORS { kb.random } else { l.color };
        self.set_p(PAIRS + 2 * i, (a & 0xf0) | (c & 0xf));
        let s = kb.speed.get(l.speed as usize).copied().unwrap_or(0);
        let b = kb.light.get(l.bright as usize).copied().unwrap_or(0);
        self.set_p(PAIRS + 2 * i + 1, s << 4 | (b & 0xf));
    }
}

/// `current` (as read) with the settings the app edits taken from `want`: the key
/// matrix, the lighting mode and each mode's colour / speed / brightness. Every other
/// byte stays as the keyboard has it.
pub fn merged(kb: &Kbd, current: &KbConfig, want: &KbConfig) -> KbConfig {
    let mut new = current.clone();
    for (n, &w) in new.matrix.iter_mut().zip(&want.matrix) {
        if w != u32::MAX || !kb.keeps_unset() {
            *n = w;
        }
    }
    new.set_mode(kb, want.mode(kb), kb.custom_index());
    for i in 0..kb.leds.len() {
        new.set_light(kb, i, want.light(kb, i));
    }
    new
}

/// True when `a` and `b` agree on everything `merged` sets.
/// (VIA / HidServ: only the active mode's speed and brightness, which is all the keyboard
/// reports besides the keymap; VIA's colour comes back as hue / saturation, HidServ's as RGB,
/// of which only "colourful" is compared)
pub fn same_settings(kb: &Kbd, a: &KbConfig, b: &KbConfig) -> bool {
    if kb.keeps_unset() {
        let (la, lb) = (a.light(kb, a.mode(kb)), b.light(kb, b.mode(kb)));
        let colourful = |l: Light| (kb.hs.is_some() || kb.du) && l.color as usize >= COLORS;
        return a.matrix == b.matrix
            && a.mode(kb) == b.mode(kb)
            && (la.speed, la.bright, colourful(la)) == (lb.speed, lb.bright, colourful(lb));
    }
    a.matrix == b.matrix && a.mode(kb) == b.mode(kb) && (0..kb.leds.len()).all(|i| a.light(kb, i) == b.light(kb, i))
}

/// What the app keeps per keyboard (models/<key>/Keyboard.ini): the blocks, plus the
/// colour palettes, which the firmware never gives back.
#[derive(Clone, Debug, PartialEq)]
pub struct KbProfile {
    pub cfg: KbConfig,
    pub palettes: Vec<[u32; COLORS]>,
}

impl KbProfile {
    pub fn blank(kb: &Kbd) -> KbProfile {
        KbProfile { cfg: KbConfig::blank(kb), palettes: vec![kb.default_palette(); kb.leds.len()] }
    }

    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        use std::fmt::Write;
        let mut o = String::from("[Keyboard]\n");
        let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
        let _ = writeln!(o, "General={}", hex(&self.cfg.general));
        let m: Vec<u8> = self.cfg.matrix.iter().flat_map(|c| c.to_le_bytes()).collect();
        let _ = writeln!(o, "Matrix={}", hex(&m));
        for (i, p) in self.palettes.iter().enumerate() {
            let _ = writeln!(o, "Palette{i}={}", p.iter().map(|c| format!("{c:06x}")).collect::<Vec<_>>().join(","));
        }
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d)?;
        }
        std::fs::write(path, o)
    }

    /// `None` when the file is missing or was saved for another block size.
    pub fn load(path: &std::path::Path, kb: &Kbd) -> Option<KbProfile> {
        let ini = crate::model::parse_ini(&std::fs::read_to_string(path).ok()?);
        let sec = ini.get("Keyboard")?;
        let unhex = |s: &str| -> Option<Vec<u8>> {
            (0..s.len() / 2).map(|i| u8::from_str_radix(s.get(2 * i..2 * i + 2)?, 16).ok()).collect()
        };
        let general = unhex(sec.get("General")?)?;
        let m = unhex(sec.get("Matrix")?)?;
        if general.len() != kb.glen as usize || m.len() != MATRIX * 4 {
            return None;
        }
        let matrix = m.chunks(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
        let palettes = (0..kb.leds.len())
            .map(|i| {
                let mut p = kb.default_palette();
                if let Some(v) = sec.get(&format!("Palette{i}")) {
                    for (j, c) in v.split(',').take(COLORS).enumerate() {
                        p[j] = u32::from_str_radix(c.trim(), 16).unwrap_or(p[j]) & 0xffffff;
                    }
                }
                p
            })
            .collect();
        Some(KbProfile { cfg: KbConfig { general, matrix }, palettes })
    }
}

/// The colour table as the drivers send it (FUN_00445c20 / FUN_00489600).
pub fn rgb_table(kb: &Kbd, palettes: &[[u32; COLORS]]) -> Vec<u8> {
    let mut t = vec![0u8; 21 * (kb.leds.len() + 1)];
    for (m, pal) in palettes.iter().enumerate().take(kb.leds.len()) {
        for (j, c) in pal.iter().enumerate() {
            let rgb = [(c >> 16) as u8, (c >> 8) as u8, *c as u8];
            for k in 0..3 {
                t[21 * (m + 1) + 3 * j + kb.rgbi[k] as usize % 3] = rgb[k];
            }
        }
    }
    t.truncate(0x3ec);
    t
}

/// Size in bytes (with the id) of feature report `id`, from the report descriptor.
pub fn feature_size(found: &Found, id: u8) -> Option<usize> {
    let rd = std::fs::read(found.hidraw_sys().join("device/report_descriptor")).ok()?;
    let (mut i, mut size, mut count, mut cur, mut bits) = (0usize, 0u32, 0u32, 0u8, 0u32);
    while i < rd.len() {
        let p = rd[i];
        let n = match p & 3 { 3 => 4, x => x as usize };
        let mut v = 0u32;
        for k in 0..n {
            v |= (*rd.get(i + 1 + k)? as u32) << (8 * k);
        }
        match p & 0xfc {
            0x74 => size = v,
            0x94 => count = v,
            0x84 => cur = v as u8,
            0xb0 if cur == id => bits += size * count,
            _ => {}
        }
        i += 1 + n;
    }
    (bits > 0).then(|| 1 + (bits as usize).div_ceil(8))
}

fn cmd(d: &Device, c: u8, addr: u8) -> std::io::Result<()> {
    let n = feature_size(&d.found, 5).unwrap_or(7).max(3);
    d.set_feature(&[5, c, addr], n)?;
    std::thread::sleep(std::time::Duration::from_millis(30));
    Ok(())
}

fn read(d: &Device, c: u8, addr: u8, n: usize) -> std::io::Result<Vec<u8>> {
    cmd(d, c, addr)?;
    let r = d.get_feature(6, DATA_LEN)?;
    if r[1] != c && r[1] != 0 {
        return Err(std::io::Error::other(format!("keyboard answered {:02x} to {c:02x}", r[1])));
    }
    Ok(r[8..8 + n].to_vec())
}

fn write(d: &Device, c: u8, addr: u8, len: u8, data: &[u8]) -> std::io::Result<()> {
    let mut pkt = vec![6, c, addr, 0, len, 0, 0, 0];
    pkt.extend_from_slice(data);
    d.set_feature(&pkt, DATA_LEN)?;
    std::thread::sleep(std::time::Duration::from_millis(50));
    Ok(())
}

/// The 6-byte password the keyboard answers to 05 81.
pub fn psd(d: &Device) -> Option<[u8; 6]> {
    cmd(d, 0x81, 0).ok()?;
    let n = feature_size(&d.found, 5).unwrap_or(8).max(8);
    let r = d.get_feature(5, n).ok()?;
    (r[1] == 1).then(|| [r[2], r[3], r[4], r[5], r[6], r[7]])
}

/// One VIA command and its answer (the keyboard echoes the command byte).
fn via_cmd(d: &Device, cmd: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut pkt = vec![0u8; 33];
    pkt[1..1 + cmd.len()].copy_from_slice(cmd);
    while d.read_report(0)?.is_some() {}
    d.write_report(&pkt)?;
    for _ in 0..10 {
        match d.read_report(200)? {
            Some(r) if r.first() == Some(&cmd[0]) => return Ok(r),
            Some(_) => continue,
            None => break,
        }
    }
    Err(std::io::Error::other(format!("no answer to VIA command {:02x}", cmd[0])))
}

/// VIA level (0-255) <-> app level index 0-4.
fn via_level(i: u8) -> u8 {
    (i.min(4) as u16 * 255 / 4) as u8
}

fn via_index(v: u8) -> u8 {
    ((v as u16 * 4 + 127) / 255) as u8
}

fn via_read(d: &Device, kb: &Kbd, v: &Via) -> std::io::Result<KbConfig> {
    let mut matrix = vec![u32::MAX; MATRIX];
    for k in kb.keys {
        let (r, c) = (k.matrix / v.cols, k.matrix % v.cols);
        let a = via_cmd(d, &[0x04, 0, r, c])?;
        matrix[k.matrix as usize] = (a[4] as u32) << 8 | a[5] as u32;
    }
    let mut cfg = KbConfig { general: vec![0; kb.glen as usize], matrix };
    let get = |id: u8| -> std::io::Result<Vec<u8>> { if id == 0 { Ok(vec![0; 8]) } else { via_cmd(d, &[0x08, v.ch, id]) } };
    let (b, e, s) = (get(v.bright)?[3], get(v.effect)?[3], get(v.speed)?[3]);
    if let Some(i) = kb.leds.iter().position(|l| l.hw == e) {
        cfg.set_mode(kb, i, None);
        for j in 0..kb.leds.len() {
            cfg.set_light(kb, j, Light { color: 0, speed: via_index(s), bright: via_index(b) });
        }
    }
    Ok(cfg)
}

fn via_write(d: &Device, kb: &Kbd, v: &Via, new: &KbConfig, current: &KbConfig, palettes: &[[u32; COLORS]]) -> std::io::Result<()> {
    for k in kb.keys {
        let (i, c) = (k.matrix as usize, new.matrix[k.matrix as usize]);
        if c != current.matrix[i] && c <= 0xffff {
            via_cmd(d, &[0x05, 0, k.matrix / v.cols, k.matrix % v.cols, (c >> 8) as u8, c as u8])?;
        }
    }
    let i = new.mode(kb);
    let Some(m) = kb.leds.get(i) else { return Ok(()) };
    let l = new.light(kb, i);
    let set = |id: u8, val: &[u8]| -> std::io::Result<()> {
        if id != 0 {
            let mut c = vec![0x07, v.ch, id];
            c.extend_from_slice(val);
            via_cmd(d, &c)?;
        }
        Ok(())
    };
    set(v.effect, &[m.hw])?;
    set(v.bright, &[via_level(l.bright)])?;
    set(v.speed, &[via_level(l.speed)])?;
    if m.color && (l.color as usize) < COLORS {
        let rgb = palettes.get(i).map_or(DEFAULT_PALETTE, |p| *p)[l.color as usize];
        set(v.color, &hue_sat(rgb))?;
    }
    if v.ch != 0 {
        via_cmd(d, &[0x09, v.ch])?;
    }
    Ok(())
}

/// RGB -> QMK hue, saturation (0-255).
fn hue_sat(rgb: u32) -> [u8; 2] {
    let (r, g, b) = (((rgb >> 16) & 255) as f32, ((rgb >> 8) & 255) as f32, (rgb & 255) as f32);
    let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
    let c = mx - mn;
    let h = if c == 0.0 {
        0.0
    } else if mx == r {
        ((g - b) / c).rem_euclid(6.0)
    } else if mx == g {
        (b - r) / c + 2.0
    } else {
        (r - g) / c + 4.0
    };
    let s = if mx == 0.0 { 0.0 } else { c / mx };
    [(h / 6.0 * 255.0).round() as u8, (s * 255.0).round() as u8]
}

/// One HidServ command and its answer (FUN_00440b60: up to 4 tries).
fn hs_cmd(d: &Device, cmd: u8, off: u16, len: u8, data: &[u8]) -> std::io::Result<Vec<u8>> {
    let pkt = hs_packet(cmd, off, len, data);
    for _ in 0..4 {
        while d.read_report(0)?.is_some() {}
        d.write_report(&pkt)?;
        for _ in 0..4 {
            match d.read_report(300)? {
                Some(r) if r.len() >= 8 && r[0] == 4 && r[3] == cmd => {
                    if r[7] == 0xff || r[7] == 0xfe {
                        return Err(std::io::Error::other(format!("keyboard refused command {cmd:02x} ({:02x})", r[7])));
                    }
                    return Ok(r);
                }
                Some(_) => continue,
                None => break,
            }
        }
    }
    Err(std::io::Error::other(format!("no answer to command {cmd:02x}")))
}

pub fn hs_packet(cmd: u8, off: u16, len: u8, data: &[u8]) -> [u8; 64] {
    let mut p = [0u8; 64];
    p[0] = 4;
    p[3] = cmd;
    p[4] = len;
    p[5..7].copy_from_slice(&off.to_le_bytes());
    let n = data.len().min(56);
    p[8..8 + n].copy_from_slice(&data[..n]);
    let sum: u32 = p[3..].iter().map(|&b| b as u32).sum();
    p[1..3].copy_from_slice(&(sum as u16).to_le_bytes());
    p
}

/// Light block offsets (FUN_004190b0, FUN_0041e010, FUN_00420290).
const HS_LIGHT: u16 = 0x2a;

fn hs_read(d: &Device, kb: &Kbd, h: &Hs) -> std::io::Result<KbConfig> {
    let cfg = hs_cmd(d, 0x03, 0, 0x2c, &[])?;
    let profile = cfg.get(8 + 0x0a).copied().unwrap_or(0).min(2);
    let l = hs_cmd(d, 0x05, profile as u16 * HS_LIGHT, HS_LIGHT as u8, &[])?;
    let size = HS_ENTRIES * 3;
    let mut t = Vec::with_capacity(size);
    while t.len() < size {
        let n = (size - t.len()).min(0x38);
        let r = hs_cmd(d, 0x07, profile as u16 * h.stride + t.len() as u16, n as u8, &[])?;
        t.extend_from_slice(r.get(8..8 + n).ok_or_else(|| std::io::Error::other("short answer"))?);
    }
    let mut matrix = vec![0u32; MATRIX];
    for (i, e) in t.chunks(3).enumerate() {
        matrix[i] = e[0] as u32 | (e[1] as u32) << 8 | (e[2] as u32) << 16;
    }
    let mut c = KbConfig { general: vec![0; kb.glen as usize], matrix };
    let b = &l[8..];
    c.general[..5].copy_from_slice(&[profile, b[5], b[6], b[7], b[4]]);
    if let Some(i) = kb.leds.iter().position(|m| m.hw == b[0]) {
        c.set_mode(kb, i, None);
        let colour = if b[4] == 1 && kb.leds[i].random { COLORS as u8 } else { 0 };
        let find = |t: &[u8], v: u8| t.iter().position(|&x| x == v).unwrap_or(0) as u8;
        c.set_light(kb, i, Light { color: colour, speed: find(kb.speed, b[2]), bright: find(kb.light, b[1]) });
    }
    Ok(c)
}

/// The writes of an Apply, inside 01 / 02 as the driver does: changed key table chunks (08),
/// then the active mode's lighting bytes (06).
pub fn hs_packets(kb: &Kbd, h: &Hs, new: &KbConfig, current: &KbConfig, palettes: &[[u32; COLORS]]) -> Vec<[u8; 64]> {
    let profile = current.general.first().copied().unwrap_or(0).min(2) as u16;
    let mut out = vec![hs_packet(0x01, 0, 0, &[])];
    let entry = |c: &KbConfig, i: usize| -> [u8; 3] {
        let v = if c.matrix[i] == u32::MAX { current.matrix[i] } else { c.matrix[i] };
        [v as u8, (v >> 8) as u8, (v >> 16) as u8]
    };
    let t: Vec<u8> = (0..HS_ENTRIES).flat_map(|i| entry(new, i)).collect();
    let old: Vec<u8> = (0..HS_ENTRIES).flat_map(|i| entry(current, i)).collect();
    for off in (0..t.len()).step_by(0x38) {
        let end = (off + 0x38).min(t.len());
        if t[off..end] != old[off..end] {
            out.push(hs_packet(0x08, profile * h.stride + off as u16, (end - off) as u8, &t[off..end]));
        }
    }
    let i = new.mode(kb);
    if let Some(m) = kb.leds.get(i) {
        let l = new.light(kb, i);
        let base = profile * HS_LIGHT;
        let speed = kb.speed.get(l.speed as usize).copied().unwrap_or(0);
        let bright = kb.light.get(l.bright as usize).copied().unwrap_or(0);
        out.push(hs_packet(0x06, base, 3, &[m.hw, bright, speed]));
        if m.color {
            if l.color as usize >= COLORS && m.random {
                out.push(hs_packet(0x06, base + 4, 1, &[1]));
            } else {
                let rgb = palettes.get(i).map_or(DEFAULT_PALETTE, |p| *p)[(l.color as usize).min(COLORS - 1)];
                out.push(hs_packet(0x06, base + 4, 4, &[0, (rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8]));
            }
        }
    }
    out.push(hs_packet(0x02, 0, 0, &[]));
    out
}

/// duilib light block: 0x2d bytes at profile * 0x19 (CEevisionKeyboardDevice FUN_0049cb30 /
/// FUN_0049e770): 01 mode, 02 brightness 0-5, 03 4 - speed, 04 direction, 05 colourful, 06-08 RGB.
const DU_LIGHT: usize = 0x2d;

fn du_read(d: &Device, kb: &Kbd) -> std::io::Result<KbConfig> {
    let (_, n) = crate::du::info(d)?;
    let profile = crate::du::read(d, 5, 0, 1)?[0];
    let l = crate::du::read(d, 5, profile as u16 * 0x19, DU_LIGHT)?;
    let t = crate::du::read(d, 7, 0, 3 * n.min(MATRIX))?;
    let mut matrix = vec![0u32; MATRIX];
    for (i, e) in t.chunks(3).enumerate() {
        matrix[i] = e[0] as u32 | (e[1] as u32) << 8 | (e[2] as u32) << 16;
    }
    let mut c = KbConfig { general: vec![0; kb.glen as usize], matrix };
    c.general[..5].copy_from_slice(&[profile, l[6], l[7], l[8], l[5]]);
    let raw = kb.glen as usize - DU_LIGHT;
    c.general[raw..].copy_from_slice(&l);
    if let Some(i) = kb.leds.iter().position(|m| m.hw == l[1]) {
        c.set_mode(kb, i, None);
        let colour = if l[5] != 0 && kb.leds[i].random { COLORS as u8 } else { 0 };
        let find = |t: &[u8], v: u8| t.iter().position(|&x| x == v).unwrap_or(0) as u8;
        c.set_light(kb, i, Light { color: colour, speed: find(kb.speed, 4u8.wrapping_sub(l[3])), bright: find(kb.light, l[2]) });
    }
    Ok(c)
}

/// The writes of an Apply as (command, offset, data), each inside 01 / 02 like the driver:
/// the key table (09) when it changed, then the light block (06) with the active mode.
pub fn du_packets(kb: &Kbd, new: &KbConfig, current: &KbConfig, palettes: &[[u32; COLORS]], keys: usize) -> Vec<(u8, u16, Vec<u8>)> {
    let entry = |c: &KbConfig, i: usize| -> [u8; 3] {
        let v = if c.matrix[i] == u32::MAX { current.matrix[i] } else { c.matrix[i] };
        [v as u8, (v >> 8) as u8, (v >> 16) as u8]
    };
    let mut out = Vec::new();
    let t: Vec<u8> = (0..keys).flat_map(|i| entry(new, i)).collect();
    if t != (0..keys).flat_map(|i| entry(current, i)).collect::<Vec<u8>>() {
        out.push((9, 0, t));
    }
    let raw = kb.glen as usize - DU_LIGHT;
    let mut l = current.general[raw..].to_vec();
    let i = new.mode(kb);
    let (ci, lt) = (current.mode(kb), new.light(kb, i));
    let cl = current.light(kb, ci);
    // only what the UI changed: the block keeps the keyboard's own colour otherwise
    if let Some(m) = kb.leds.get(i).filter(|_| i != ci || lt != cl) {
        l[1] = m.hw;
        l[2] = kb.light.get(lt.bright as usize).copied().unwrap_or(0);
        l[3] = 4u8.wrapping_sub(kb.speed.get(lt.speed as usize).copied().unwrap_or(0));
        if m.color && (i != ci || lt.color != cl.color) {
            if lt.color as usize >= COLORS && m.random {
                l[5] = 1;
            } else {
                let rgb = palettes.get(i).map_or(DEFAULT_PALETTE, |p| *p)[(lt.color as usize).min(COLORS - 1)];
                l[5] = 0;
                l[6..9].copy_from_slice(&[(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8]);
            }
        }
    }
    if l != current.general[raw..] {
        let profile = current.general[0] as u16;
        out.push((6, profile * 0x19, l));
    }
    out
}

pub fn read_config(d: &Device, kb: &Kbd) -> std::io::Result<KbConfig> {
    if let Some(v) = &kb.via {
        return via_read(d, kb, v);
    }
    if kb.du {
        return du_read(d, kb);
    }
    if let Some(h) = &kb.hs {
        return hs_read(d, kb, h);
    }
    if kb.mk {
        return mk_read(d, kb);
    }
    if kb.mg {
        return mg_read(d, kb);
    }
    if kb.write_only() {
        return Err(std::io::Error::other("this keyboard does not report its settings"));
    }
    if kb.byk {
        // FUN_00403210 "SyncKB": GET feature report 9, 0x7e codes
        let r = d.get_feature(9, 1 + 4 * BYK_MATRIX)?;
        let mut matrix: Vec<u32> = r[1..].chunks(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
        matrix.resize(MATRIX, 0);
        return Ok(KbConfig { general: Vec::new(), matrix });
    }
    let general = read(d, 0x83, kb.pro, kb.glen as usize)?;
    let m = read(d, 0x84, kb.key, MATRIX * 4)?;
    let matrix = m.chunks(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
    Ok(KbConfig { general, matrix })
}

/// Writes what changed (matrix, colour table, general block, in the drivers' order).
pub fn write_config(d: &Device, kb: &Kbd, new: &KbConfig, current: &KbConfig, palettes: &[[u32; COLORS]]) -> std::io::Result<()> {
    if let Some(v) = &kb.via {
        return via_write(d, kb, v, new, current, palettes);
    }
    if let Some(h) = &kb.hs {
        for p in hs_packets(kb, h, new, current, palettes) {
            hs_cmd(d, p[3], u16::from_le_bytes([p[5], p[6]]), p[4], &p[8..])?;
        }
        return Ok(());
    }
    if kb.du {
        let (_, keys) = crate::du::info(d)?;
        for (c, off, data) in du_packets(kb, new, current, palettes, keys.min(MATRIX)) {
            crate::du::cmd(d, 1, 0, 0, &[])?;
            crate::du::write(d, c, off, &data)?;
            crate::du::cmd(d, 2, 0, 0, &[])?;
        }
        return Ok(());
    }
    if kb.mk {
        return mk_write(d, kb, new, current);
    }
    if kb.mg {
        return mg_write(d, kb, new, current);
    }
    if kb.jl.is_some() {
        let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u16);
        if let Some(p) = jl_light(kb, new, palettes, ms) {
            d.write_report(&p)?;
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
        return Ok(());
    }
    if kb.xy {
        for p in xy_packets(kb, new, palettes) {
            d.set_feature(&p, p.len())?;
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        return Ok(());
    }
    if kb.byk {
        if new.matrix[..BYK_MATRIX] != current.matrix[..BYK_MATRIX] {
            let mut pkt = vec![9u8];
            pkt.extend(new.matrix[..BYK_MATRIX].iter().flat_map(|c| c.to_le_bytes()));
            d.set_feature(&pkt, pkt.len())?;
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        return Ok(());
    }
    if new.matrix != current.matrix {
        let m: Vec<u8> = new.matrix.iter().flat_map(|c| c.to_le_bytes()).collect();
        write(d, 0x04, kb.key, 0x40, &m)?;
    }
    let custom = new.p(kb.led_off as usize) != 0;
    if !custom {
        write(d, 0x08, kb.rgb, 0x40, &rgb_table(kb, palettes))?;
    }
    if new.general != current.general {
        write(d, 0x03, kb.pro, 0, &new.general)?;
    }
    Ok(())
}

// ------------------------------------------------------------------ MosArt (K598-KNS / K599)

const MK_A0: usize = 0x18;
const MK_A1: usize = 0x6a;

/// Offset of the two areas in the general block (after the lighting pairs of every mode).
fn mk_base(kb: &Kbd) -> usize {
    PROFILE + PAIRS + 2 * kb.leds.len()
}

/// u16 checksum of an area (FUN_00437d9c), stored in its last two bytes.
pub fn mk_sum(a: &mut [u8]) {
    let n = a.len() - 2;
    let s = a[..n].iter().fold(0x8032u16, |s, &b| s.wrapping_add(b as u16));
    a[n..].copy_from_slice(&s.to_le_bytes());
}

/// Output report 8 with command `c`; waits for the echo (FUN_004371a8). Read replies carry the data
/// from byte 5.
fn mk_cmd(d: &Device, c: u8, addr: u16, data: &[u8], len: u8) -> std::io::Result<Vec<u8>> {
    let mut p = [0u8; 64];
    p[..5].copy_from_slice(&[8, c, addr as u8, (addr >> 8) as u8, len]);
    p[5..5 + data.len()].copy_from_slice(data);
    while d.read_report(0)?.is_some() {}
    d.write_report(&p)?;
    let t = std::time::Instant::now();
    while t.elapsed() < std::time::Duration::from_millis(1500) {
        if let Some(r) = d.read_report(300)? {
            if r.len() > 5 && r[0] == 8 && r[1] == c {
                return Ok(r);
            }
        }
    }
    Err(std::io::Error::other(format!("no answer to command {c}")))
}

fn mk_area(d: &Device, id: u8, addr: u16, n: usize) -> std::io::Result<Vec<u8>> {
    // FUN_00438ea0: the feature report, else command 3 in pieces of up to 0x3b bytes
    if let Ok(r) = d.get_feature(id, n + 1) {
        if r[0] == id && r[1..].iter().any(|&b| b != 0) {
            return Ok(r[1..].to_vec());
        }
    }
    let mut out = Vec::new();
    while out.len() < n {
        let k = (n - out.len()).min(0x3b);
        let r = mk_cmd(d, 3, addr + out.len() as u16, &[], k as u8)?;
        out.extend_from_slice(&r[5..5 + k]);
    }
    Ok(out)
}

/// Area 0 -> the general block's lighting (one setting for every mode).
pub fn mk_general(kb: &Kbd, a0: &[u8], a1: &[u8]) -> Vec<u8> {
    let mut c = KbConfig { general: vec![0; kb.glen as usize], matrix: vec![0; MATRIX] };
    let g = (a0[8] as usize).min(5);
    let i = (g * 3 + (a0[9 + g] as usize).min(2)).min(kb.leds.len().saturating_sub(1));
    let find = |t: &[u8], v: u8| t.iter().position(|&x| x == v).unwrap_or(t.len().saturating_sub(1)) as u8;
    let color = if a0[5] >= kb.random { COLORS as u8 } else { a0[5].min(COLORS as u8 - 1) };
    let l = Light { color, speed: find(kb.speed, a0[6]), bright: find(kb.light, a0[2].min(*kb.light.last().unwrap_or(&0))) };
    for m in 0..kb.leds.len() {
        c.set_light(kb, m, l);
    }
    c.set_mode(kb, i, None);
    let b = mk_base(kb);
    c.general[b..b + MK_A0].copy_from_slice(a0);
    c.general[b + MK_A0..b + MK_A0 + MK_A1].copy_from_slice(a1);
    c.general
}

/// The area 0 the keyboard is sent for `c`.
pub fn mk_area0(kb: &Kbd, c: &KbConfig) -> Vec<u8> {
    let b = mk_base(kb);
    let mut a = c.general[b..b + MK_A0].to_vec();
    let i = c.mode(kb);
    let l = c.light(kb, i);
    let (g, v) = (i / 3, i % 3);
    let old = (a[8] as usize).min(5) * 3 + (a[9 + (a[8] as usize).min(5)] as usize).min(2);
    if old != i {
        a[0xf] = 0; // a lighting effect replaces the custom mode
    }
    a[8] = g as u8;
    a[9 + g] = v as u8;
    // keep a fixed colour the menu cannot show (white)
    let shown = if a[5] >= kb.random { COLORS as u8 } else { a[5].min(COLORS as u8 - 1) };
    if l.color != shown {
        a[5] = if l.color as usize >= COLORS { kb.random } else { l.color };
    }
    a[6] = kb.speed.get(l.speed as usize).copied().unwrap_or(a[6]);
    let shown = kb.light.iter().position(|&x| x == a[2].min(*kb.light.last().unwrap_or(&0))).unwrap_or(0) as u8;
    if l.bright != shown {
        a[2] = kb.light.get(l.bright as usize).copied().unwrap_or(a[2]);
    }
    a[4] = 1;
    mk_sum(&mut a);
    a
}

fn mk_read(d: &Device, kb: &Kbd) -> std::io::Result<KbConfig> {
    mk_cmd(d, 1, 0, &[], 0)?;
    let a0 = mk_area(d, 0x14, 0, MK_A0)?;
    let a1 = mk_area(d, 0x15, 0x100, MK_A1)?;
    Ok(KbConfig { general: mk_general(kb, &a0, &a1), matrix: vec![0; MATRIX] })
}

/// FUN_0043a9e4: 1, 2, area 0 with command 5, 7, 1.
fn mk_write(d: &Device, kb: &Kbd, new: &KbConfig, current: &KbConfig) -> std::io::Result<()> {
    let a = mk_area0(kb, new);
    let b = mk_base(kb);
    if a[..MK_A0 - 2] == current.general[b..b + MK_A0 - 2] {
        return Ok(());
    }
    mk_cmd(d, 1, 0, &[], 0)?;
    mk_cmd(d, 2, 0, &[], 0)?;
    mk_cmd(d, 5, 0, &a, MK_A0 as u8)?;
    mk_cmd(d, 7, 0, &[], 0)?;
    mk_cmd(d, 1, 0, &[], 0)?;
    Ok(())
}

// ------------------------------------------------------------------ magnetic (K729RGB-M)

const MG: usize = 0x20;
/// Brightness byte of each UI level (the driver's slider: 0..100).
const MG_BRIGHT: [u8; 5] = [20, 40, 60, 80, 100];

fn mg_base(kb: &Kbd) -> usize {
    PROFILE + PAIRS + 2 * kb.leds.len()
}

/// The 65-byte report for command `c`, with its checksum.
pub fn mg_packet(c: u8, off: u16, data: &[u8], len: u8) -> [u8; 65] {
    let mut p = [0u8; 65];
    p[1] = 0x55;
    p[2] = c;
    p[5] = len;
    p[6..8].copy_from_slice(&off.to_le_bytes());
    p[9..9 + data.len()].copy_from_slice(data);
    p[4] = p[5..].iter().fold(0u8, |s, &b| s.wrapping_add(b));
    p
}

/// Sends `c`; with `answer`, waits for the keyboard's echo (FUN_00468890) and returns its data.
fn mg_cmd(d: &Device, c: u8, off: u16, data: &[u8], len: u8, answer: bool) -> std::io::Result<Vec<u8>> {
    while d.read_report(0)?.is_some() {}
    d.write_report(&mg_packet(c, off, data, len))?;
    if !answer {
        std::thread::sleep(std::time::Duration::from_millis(2));
        return Ok(Vec::new());
    }
    let t = std::time::Instant::now();
    while t.elapsed() < std::time::Duration::from_millis(1000) {
        if let Some(r) = d.read_report(50)? {
            if r.len() >= 8 + len as usize && r[0] == 0xaa && r[1] == c && r[4] == len && r[5..7] == off.to_le_bytes() {
                return Ok(r[8..8 + len as usize].to_vec());
            }
        }
    }
    Err(std::io::Error::other(format!("no answer to command {c}")))
}

fn mg_light(kb: &Kbd, a: &[u8]) -> (Option<usize>, Light) {
    let near = |t: &[u8], v: u8| (0..t.len()).min_by_key(|&i| (t[i] as i32 - v as i32).abs()).unwrap_or(0) as u8;
    let rgb = (a[14] as u32) << 16 | (a[15] as u32) << 8 | a[16] as u32;
    let color = if a[12] != 0 { COLORS as u8 } else { kb.default_palette().iter().position(|&c| c == rgb).unwrap_or(0) as u8 };
    let i = kb.leds.iter().position(|l| l.hw == a[8]);
    (i, Light { color, speed: near(kb.speed, a[10]), bright: near(&MG_BRIGHT, a[9]) })
}

/// The keyboard's settings block -> the general block (one setting for every mode).
pub fn mg_general(kb: &Kbd, a: &[u8], profile: u8) -> Vec<u8> {
    let mut c = KbConfig { general: vec![0; kb.glen as usize], matrix: vec![0; MATRIX] };
    let (i, l) = mg_light(kb, a);
    for m in 0..kb.leds.len() {
        c.set_light(kb, m, l);
    }
    c.set_mode(kb, i.unwrap_or(0), None);
    let b = mg_base(kb);
    c.general[b..b + MG].copy_from_slice(a);
    c.general[b + MG] = profile;
    c.general
}

/// The settings block the keyboard is sent for `c` (what the UI did not change keeps its bytes).
pub fn mg_block(kb: &Kbd, c: &KbConfig) -> Vec<u8> {
    let b = mg_base(kb);
    let mut a = c.general[b..b + MG].to_vec();
    let (old_i, old) = mg_light(kb, &a);
    let i = c.mode(kb);
    let l = c.light(kb, i);
    if old_i != Some(i) {
        a[8] = kb.leds[i].hw;
    }
    if l.color != old.color || old_i != Some(i) {
        a[12] = (l.color as usize >= COLORS) as u8;
        if (l.color as usize) < COLORS {
            let rgb = kb.default_palette()[l.color as usize];
            a[14..17].copy_from_slice(&[(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8]);
        }
    }
    if l.speed != old.speed {
        a[10] = kb.speed[l.speed as usize];
    }
    if l.bright != old.bright {
        a[9] = MG_BRIGHT[(l.bright as usize).min(MG_BRIGHT.len() - 1)];
    }
    a
}

fn mg_read(d: &Device, kb: &Kbd) -> std::io::Result<KbConfig> {
    mg_cmd(d, 1, 0, &[], 0, false)?;
    let r = (|| {
        let p = mg_cmd(d, 4, 0, &[], 0x10, true)?[0].min(3);
        let a = mg_cmd(d, 5, (p as u16) << 6, &[], MG as u8, true)?;
        Ok(KbConfig { general: mg_general(kb, &a, p), matrix: vec![0; MATRIX] })
    })();
    mg_cmd(d, 2, 0, &[], 0, false)?;
    r
}

/// FUN_0042cd50: 1, read the block (5), write it (6), 400 ms, 2.
fn mg_write(d: &Device, kb: &Kbd, new: &KbConfig, current: &KbConfig) -> std::io::Result<()> {
    let b = mg_base(kb);
    let a = mg_block(kb, new);
    if a == current.general[b..b + MG] {
        return Ok(());
    }
    let p = current.general[b + MG] as u16;
    mg_cmd(d, 1, 0, &[], 0, false)?;
    let r = (|| {
        let mut cur = mg_cmd(d, 5, p << 6, &[], MG as u8, true)?;
        cur[8..17].copy_from_slice(&a[8..17]);
        mg_cmd(d, 6, p << 6, &cur, MG as u8, false)?;
        std::thread::sleep(std::time::Duration::from_millis(400));
        Ok(())
    })();
    mg_cmd(d, 2, 0, &[], 0, false)?;
    r
}

/// The firmware name a Jieli keyboard answers to `01 0d` (None: no answer).
pub fn jl_name(d: &Device) -> Option<String> {
    let mut pkt = [0u8; 64];
    (pkt[0], pkt[1]) = (1, 0x0d);
    for _ in 0..3 {
        while d.read_report(0).ok()?.is_some() {}
        d.write_report(&pkt).ok()?;
        for _ in 0..4 {
            match d.read_report(300).ok()? {
                Some(r) if r.len() > 6 && r[1] == 0x0d => {
                    let t: Vec<u8> = r[6..].iter().copied().take_while(|&b| b != 0).collect();
                    let t = String::from_utf8_lossy(&t);
                    let name = t.split(',').next().unwrap_or("").trim().to_string();
                    if !name.is_empty() && !name.contains(':') {
                        return Some(name);
                    }
                }
                Some(_) => continue,
                None => break,
            }
        }
    }
    None
}

/// The keyboard lighting packet of the active mode (FUN_004e3af0, wire command 7). Brightness
/// and speed levels 1-5 are sent 0-4; colourful only for the animated modes (Static sends 0).
pub fn jl_light(kb: &Kbd, c: &KbConfig, palettes: &[[u32; COLORS]], ms: u16) -> Option<[u8; 64]> {
    let i = c.mode(kb);
    let m = kb.leds.get(i)?;
    let l = c.light(kb, i);
    let mut p = [0u8; 64];
    p[..6].copy_from_slice(&[1, 7, 0, 0, 0, 0x0e]);
    let rgb = palettes.get(i).map_or(DEFAULT_PALETTE, |p| *p)[(l.color as usize).min(COLORS - 1)];
    let colourful = (l.color as usize >= COLORS && m.random) as u8;
    p[6..17].copy_from_slice(&[
        // LedOpt.hw = ledType + 1 (gen_jieli.py)
        m.hw.saturating_sub(1),
        kb.light.get(l.bright as usize).copied().unwrap_or(0),
        kb.speed.get(l.speed as usize).copied().unwrap_or(0),
        (rgb >> 16) as u8,
        (rgb >> 8) as u8,
        rgb as u8,
        0,
        0,
        1,
        0,
        colourful,
    ]);
    p[62..].copy_from_slice(&ms.to_be_bytes());
    Some(p)
}

/// Everything an xyui keyboard is sent on Apply: the lighting of the active mode, then the
/// 10 key slots (changed keys first, the rest emptied).
/// Shinetek: the colour radio (1-based) as FUN_00420520 sends it in modes 0 and 4.
fn st_colour(mode: u8, c: u8) -> u8 {
    let map: &[(u8, u8)] = match mode {
        0 => &[(1, 7), (3, 4)],
        4 => &[(1, 7), (2, 4), (3, 2), (4, 1), (6, 3), (7, 6)],
        _ => &[],
    };
    map.iter().find(|m| m.0 == c).map_or(c, |m| m.1)
}

pub fn xy_packets(kb: &Kbd, c: &KbConfig, palettes: &[[u32; COLORS]]) -> Vec<[u8; 8]> {
    let mut out = Vec::new();
    let i = c.mode(kb);
    if let Some(m) = kb.leds.get(i) {
        let l = c.light(kb, i);
        let colour = if l.color as usize >= COLORS { kb.random } else { l.color };
        let speed = kb.speed.get(l.speed as usize).copied().unwrap_or(0);
        let bright = kb.light.get(l.bright as usize).copied().unwrap_or(0);
        if kb.st {
            // FUN_00420520: `07 f7 MODE COLOUR DIRECTION SPEED BRIGHT 0` (struct at 0x47c + 5 * mode)
            out.push([7, 0xf7, m.hw.saturating_sub(1), st_colour(m.hw.saturating_sub(1), (l.color as usize).min(COLORS - 1) as u8 + 1), 1, speed, bright, 0]);
        } else {
        if m.hw == 8 {
            out.push([7, 0xf7, 0, 0, 0, 0, 0, 0]);
        }
        let mut p = [7, 0xf7, m.hw.saturating_sub(1), colour, 0, speed, bright, 0];
        if m.hw == 4 && m.rgb {
            // color_value, low byte (red) first, into [4] [5] [7]; brightness stays in [6]
            let rgb = palettes.get(i).map_or(XY_PALETTE, |p| *p)[(l.color as usize).min(COLORS - 1)];
            (p[4], p[5], p[7]) = ((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8);
        }
        if m.hw == 9 {
            // the driver sends this mode's speed as stored (1-based)
            p[5] = speed + 1;
        }
        out.push(p);
        }
    }
    let changed: Vec<(u8, u8)> = kb
        .keys
        .iter()
        .filter_map(|k| {
            let v = c.matrix[k.matrix as usize];
            (v != k.default && v <= 0xff).then_some((k.matrix, v as u8))
        })
        .take(10)
        .collect();
    for slot in 0..10 {
        let (key, value) = changed.get(slot).copied().unwrap_or((0, 0));
        out.push([7, 0xfa, 1, slot as u8 + 1, key, 1, value, 1]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_codes_match_the_driver_encoder() {
        // FUN_00489960: left button, Ctrl, consumer usages, plain key with modifiers
        assert_eq!(KFunc::Mouse(0).encode(), 0xf0000007);
        assert_eq!(KFunc::Mouse(2).encode(), 0xf2000007);
        assert_eq!(KFunc::Key { usage: 0xe0, mods: 0 }.encode(), 0xe0000006);
        assert_eq!(KFunc::Media(0x183).encode(), 0x83010004);
        assert_eq!(KFunc::Media(0xcd).encode(), 0xcd000004);
        // Ctrl+Z as the driver's "undo" (0x43 -> 0x1d010000)
        assert_eq!(KFunc::Key { usage: 0x1d, mods: 1 }.encode(), 0x1d010000);
        for c in [0xf0000007u32, 0xe0000006, 0x83010004, 0x1d010000, 0, 0x14000007, 0x10010] {
            assert_eq!(KFunc::decode(c).encode(), c, "{c:#x}");
        }
    }

    #[test]
    fn colour_table_layout() {
        static LEDS: [LedOpt; 2] = [
            LedOpt { hw: 1, name: "a", speed: true, light: true, color: true, random: true, rgb: false },
            LedOpt { hw: 2, name: "b", speed: true, light: true, color: true, random: true, rgb: false },
        ];
        let kb = Kbd { glen: 0x88, pro: 0xb6, key: 0xd4, rgb: 0xb8, led_off: 4, psd: [0; 6], keys: &[], leds: &LEDS,
            light: &[0, 1, 2, 3, 4], speed: &[0, 1, 2, 3, 4], rgbi: [2, 1, 0], random: 7, photo_w: 0, photo_h: 0, byk: false, xy: false, via: None, hs: None, jl: None, du: false, st: false, mk: false, mg: false };
        let t = rgb_table(&kb, &[DEFAULT_PALETTE; 2]);
        assert_eq!(t.len(), 63);
        assert!(t[..21].iter().all(|&b| b == 0));
        // RGBIndex 2,1,0: red's R byte goes to offset 2 of its triple
        assert_eq!(&t[21..24], &[0, 0, 0xff]);
        let mut c = KbConfig { general: vec![0; 0x88], matrix: vec![0; MATRIX] };
        c.set_mode(&kb, 1, None);
        c.set_light(&kb, 1, Light { color: COLORS as u8, speed: 2, bright: 4 });
        assert_eq!(c.mode(&kb), 1);
        assert_eq!(c.general[8 + 5], 2);
        assert_eq!(&c.general[8 + 0x18..8 + 0x1a], &[7, 0x24]);
        assert_eq!(c.light(&kb, 1), Light { color: COLORS as u8, speed: 2, bright: 4 });
    }

    #[test]
    fn xy_packets_follow_the_driver() {
        static LEDS: [LedOpt; 2] = [
            LedOpt { hw: 1, name: "a", speed: true, light: true, color: true, random: true, rgb: false },
            LedOpt { hw: 4, name: "b", speed: false, light: true, color: true, random: false, rgb: true },
        ];
        static KEYS: [Key; 2] = [Key { x: 0, y: 0, w: 1, h: 1, matrix: 4, default: 4 }, Key { x: 0, y: 0, w: 1, h: 1, matrix: 5, default: 5 }];
        let kb = Kbd { glen: 8 + 0x16 + 4, pro: 0, key: 0, rgb: 0, led_off: 4, psd: [0; 6], keys: &KEYS, leds: &LEDS,
            light: &[0, 1, 2, 3], speed: &[0, 1, 2, 3], rgbi: [0, 1, 2], random: 7, photo_w: 0, photo_h: 0, byk: false, xy: true, via: None, hs: None, jl: None, du: false, st: false, mk: false, mg: false };
        let mut c = KbConfig::blank(&kb);
        assert_eq!(c.mode(&kb), 0);
        c.set_light(&kb, 0, Light { color: COLORS as u8, speed: 2, bright: 3 });
        c.matrix[5] = kb.code(&KFunc::Media(0xcd));
        assert_eq!(kb.func(c.matrix[5]), KFunc::Media(0xcd));
        assert_eq!(kb.func(kb.code(&KFunc::Disabled)), KFunc::Disabled);
        let p = xy_packets(&kb, &c, &[XY_PALETTE; 2]);
        assert_eq!(p.len(), 11);
        assert_eq!(p[0], [7, 0xf7, 0, 7, 0, 2, 3, 0]);
        assert_eq!(p[1], [7, 0xfa, 1, 1, 5, 1, 0xee, 1]);
        assert_eq!(p[10], [7, 0xfa, 1, 10, 0, 1, 0, 1]);
        c.set_mode(&kb, 1, None);
        c.set_light(&kb, 1, Light { color: 2, speed: 0, bright: 1 });
        assert_eq!(xy_packets(&kb, &c, &[XY_PALETTE; 2])[0], [7, 0xf7, 3, 2, 0, 0, 1, 0xff]);
    }

    #[test]
    fn st_light_follows_the_driver() {
        static LEDS: [LedOpt; 2] = [
            LedOpt { hw: 1, name: "Ambilight", speed: true, light: true, color: true, random: false, rgb: false },
            LedOpt { hw: 5, name: "Breathing", speed: true, light: true, color: true, random: false, rgb: false },
        ];
        let kb = Kbd { glen: 8 + 0x16 + 4, pro: 0, key: 0, rgb: 0, led_off: 4, psd: [0; 6], keys: &[], leds: &LEDS,
            light: &[0, 1, 2, 3], speed: &[0, 1, 2, 3], rgbi: [0, 1, 2], random: 7, photo_w: 0, photo_h: 0, byk: false, xy: true, via: None, hs: None, jl: None, du: false, st: true, mk: false, mg: false };
        let mut c = KbConfig::blank(&kb);
        // FUN_00420520: mode 0 sends colour radio 1 as 7; mode 4 sends 3 as 2
        c.set_light(&kb, 0, Light { color: 0, speed: 3, bright: 2 });
        assert_eq!(xy_packets(&kb, &c, &[XY_PALETTE; 2])[0], [7, 0xf7, 0, 7, 1, 3, 2, 0]);
        c.set_mode(&kb, 1, None);
        c.set_light(&kb, 1, Light { color: 2, speed: 1, bright: 3 });
        assert_eq!(xy_packets(&kb, &c, &[XY_PALETTE; 2])[0], [7, 0xf7, 4, 2, 1, 1, 3, 0]);
        assert_eq!(kb.code(&KFunc::Media(0xcd)), u32::MAX);
    }

    #[test]
    fn via_keycodes_and_colour() {
        let kb = Kbd { glen: 8 + 0x16 + 2, pro: 0, key: 0, rgb: 0, led_off: 4, psd: [0; 6], keys: &[], leds: &[],
            light: &[0, 1, 2, 3, 4], speed: &[0, 1, 2, 3, 4], rgbi: [0, 1, 2], random: 15, photo_w: 0, photo_h: 0, byk: false, xy: false,
            via: Some(Via { cols: 16, ch: 3, bright: 1, effect: 2, speed: 3, color: 4 }), hs: None, jl: None, du: false, st: false, mk: false, mg: false };
        // QMK: LCTL(KC_C) = 0x0106, RSFT(KC_A) = 0x1204, KC_MUTE = 0xa8, KC_NO = 0
        assert_eq!(kb.code(&KFunc::Key { usage: 6, mods: 1 }), 0x0106);
        assert_eq!(kb.code(&KFunc::Key { usage: 4, mods: 0x20 }), 0x1204);
        assert_eq!(kb.code(&KFunc::Media(0xe2)), 0xa8);
        assert_eq!(kb.code(&KFunc::Disabled), 0);
        for c in [0x0106u32, 0x1204, 0xa8, 0, 0x29, 0x7e00] {
            assert_eq!(kb.code(&kb.func(c)), c, "{c:#x}");
        }
        assert_eq!(hue_sat(0xff0000), [0, 255]);
        assert_eq!(hue_sat(0x0000ff), [170, 255]);
        assert_eq!((via_level(4), via_index(255), via_index(via_level(2))), (255, 4, 2));
    }

    #[test]
    fn hs_packets_follow_the_driver() {
        static LEDS: [LedOpt; 2] = [
            LedOpt { hw: 5, name: "Breathing", speed: true, light: true, color: true, random: true, rgb: false },
            LedOpt { hw: 6, name: "Normally on", speed: false, light: true, color: true, random: false, rgb: false },
        ];
        static KEYS: [Key; 1] = [Key { x: 0, y: 0, w: 1, h: 1, matrix: 8, default: 0x290202 }];
        let kb = Kbd { glen: 8 + 0x16 + 4, pro: 0, key: 0, rgb: 0, led_off: 4, psd: [0; 6], keys: &KEYS, leds: &LEDS,
            light: &[0, 1, 2, 3, 4, 5], speed: &[5, 4, 3, 2, 1, 0], rgbi: [0, 1, 2], random: 7, photo_w: 0, photo_h: 0,
            byk: false, xy: false, via: None, hs: Some(Hs { stride: 0x17a }), jl: None, du: false, st: false, mk: false, mg: false };
        // entries: Esc, LShift, middle button, Play/Pause (K580's default table bytes)
        for (f, c) in [(KFunc::Key { usage: 0x29, mods: 0 }, 0x290202u32), (KFunc::Key { usage: 0xe1, mods: 0 }, 0x020102),
            (KFunc::Mouse(2), 0x040101), (KFunc::Media(0xcd), 0x00cd03)] {
            assert_eq!(kb.code(&f), c);
            assert_eq!(kb.func(c), f);
        }
        assert_eq!(kb.code(&KFunc::Key { usage: 6, mods: 1 }), u32::MAX);
        // begin: 04 sum 01 ..
        assert_eq!(hs_packet(1, 0, 0, &[])[..8], [4, 1, 0, 1, 0, 0, 0, 0]);
        let mut current = KbConfig { general: vec![0; kb.glen as usize], matrix: vec![0; MATRIX] };
        current.general[0] = 1; // profile 2
        current.matrix[8] = 0x290202;
        let mut want = KbConfig::blank(&kb);
        assert_eq!(want.matrix[0], u32::MAX);
        want.matrix[8] = kb.code(&KFunc::Disabled);
        want.set_mode(&kb, 1, None);
        want.set_light(&kb, 1, Light { color: 2, speed: 0, bright: 5 });
        let new = merged(&kb, &current, &want);
        let p = hs_packets(&kb, kb.hs.as_ref().unwrap(), &new, &current, &[DEFAULT_PALETTE; 2]);
        assert_eq!(p.len(), 5);
        // key table chunk 0 of profile 2 at 0x17a, Esc (entry 8) disabled
        assert_eq!((p[1][3], p[1][4], u16::from_le_bytes([p[1][5], p[1][6]])), (8, 0x38, 0x17a));
        assert_eq!(p[1][8 + 24..8 + 27], [0, 0, 0]);
        // mode 6, brightness 5, speed byte 5, then colour 0 (single) + RGB at 0x2a + 4
        assert_eq!(p[2][3..11], [6, 3, 0x2a, 0, 0, 6, 5, 5]);
        assert_eq!(p[3][3..13], [6, 4, 0x2e, 0, 0, 0, 0, 0xff, 0, 0]);
        assert_eq!(p[4][3], 2);
        let sum: u32 = p[2][3..].iter().map(|&b| b as u32).sum();
        assert_eq!(u16::from_le_bytes([p[2][1], p[2][2]]), sum as u16);
    }

    #[test]
    fn jl_light_follows_the_driver() {
        static LEDS: [LedOpt; 2] = [
            LedOpt { hw: 1, name: "Static", speed: false, light: true, color: true, random: false, rgb: false },
            LedOpt { hw: 3, name: "Wave", speed: true, light: true, color: true, random: true, rgb: false },
        ];
        let kb = Kbd { glen: 8 + 0x16 + 4, pro: 0, key: 0, rgb: 0, led_off: 4, psd: [0; 6], keys: &[], leds: &LEDS,
            light: &[0, 1, 2, 3, 4], speed: &[0, 1, 2, 3, 4], rgbi: [0, 1, 2], random: 7, photo_w: 0, photo_h: 0,
            byk: false, xy: false, via: None, hs: None, jl: Some(&["7153USHEXYXCPARGB"]), du: false, st: false, mk: false, mg: false };
        assert!(kb.write_only());
        let mut c = KbConfig::blank(&kb);
        // the driver's default: Wave, brightness 3, speed 3 (sent 2, 2), red
        c.set_mode(&kb, 1, None);
        c.set_light(&kb, 1, Light { color: 0, speed: 2, bright: 2 });
        let p = jl_light(&kb, &c, &[DEFAULT_PALETTE; 2], 0x1234).unwrap();
        assert_eq!(p[..17], [1, 7, 0, 0, 0, 0x0e, 2, 2, 2, 0xff, 0, 0, 0, 0, 1, 0, 0]);
        assert_eq!(p[62..], [0x12, 0x34]);
        c.set_light(&kb, 1, Light { color: COLORS as u8, speed: 4, bright: 4 });
        assert_eq!(jl_light(&kb, &c, &[DEFAULT_PALETTE; 2], 0).unwrap()[16], 1);
    }

    #[test]
    fn byk_codes_match_the_driver_encoder() {
        let kb = Kbd { glen: 0, pro: 0, key: 0, rgb: 0, led_off: 0, psd: [0; 6], keys: &[], leds: &[], light: &[], speed: &[],
            rgbi: [0, 1, 2], random: 7, photo_w: 0, photo_h: 0, byk: true, xy: false, via: None, hs: None, jl: None, du: false, st: false, mk: false, mg: false };
        // FUN_00402b30: mouse 0x11 -> f010, 0x14 -> f410; media 0x21 -> 0x83010001; Fn
        assert_eq!(kb.code(&KFunc::Mouse(0)), 0xf010);
        assert_eq!(kb.code(&KFunc::Mouse(3)), 0xf410);
        assert_eq!(kb.code(&KFunc::Media(0x183)), 0x83010001);
        assert_eq!(kb.code(&KFunc::Fn), 0xb00000);
        assert_eq!(kb.code(&KFunc::Key { usage: 0x29, mods: 0 }), 0x290000);
        for c in [0xf010u32, 0xf310, 0x83010001, 0xb00000, 0x290000, 0x1d0100, 0, 0x130] {
            assert_eq!(kb.code(&kb.func(c)), c, "{c:#x}");
        }
    }

    #[test]
    fn merge_keeps_what_the_app_does_not_edit() {
        static LEDS: [LedOpt; 2] = [
            LedOpt { hw: 1, name: "a", speed: true, light: true, color: true, random: true, rgb: false },
            LedOpt { hw: 21, name: "custom", speed: false, light: false, color: false, random: false, rgb: false },
        ];
        let kb = Kbd { glen: 0x88, pro: 0xb6, key: 0xd4, rgb: 0xb8, led_off: 4, psd: [0; 6], keys: &[], leds: &LEDS,
            light: &[0, 1, 2, 3, 4], speed: &[0, 1, 2, 3, 4], rgbi: [0, 1, 2], random: 7, photo_w: 0, photo_h: 0, byk: false, xy: false, via: None, hs: None, jl: None, du: false, st: false, mk: false, mg: false };
        let current = KbConfig { general: (0..0x88).map(|i| i as u8).collect(), matrix: vec![7; MATRIX] };
        let mut want = KbConfig::blank(&kb);
        want.matrix[3] = 0x1d010000;
        want.set_mode(&kb, 1, kb.custom_index());
        let m = merged(&kb, &current, &want);
        assert_eq!(m.matrix[3], 0x1d010000);
        // custom mode: flag and code set, colour group (o+2) and the header kept
        assert_eq!(&m.general[8 + 4..8 + 7], &[1, 21, 14]);
        assert_eq!(&m.general[..8], &current.general[..8]);
        // high nibble of each colour byte kept
        assert_eq!(m.general[8 + 0x16] & 0xf0, current.general[8 + 0x16] & 0xf0);
        assert!(same_settings(&kb, &m, &merged(&kb, &m, &want)));
    }

    #[test]
    fn du_codes_and_packets_follow_the_driver() {
        // Keyboard.json defaults: Esc 20 00 29, Ctrl 20 01 00, Fn a0 01 00, left button 10 01
        assert_eq!(du_code(&KFunc::Key { usage: 0x29, mods: 0 }), 0x290020);
        assert_eq!(du_code(&KFunc::Key { usage: 0xe0, mods: 0 }), 0x0120);
        assert_eq!(du_code(&KFunc::Mouse(0)), 0x0110);
        for c in [0x290020u32, 0x0120, 0x01a0, 0x0110, 0x00e930, 0x1d0120, 0] {
            assert_eq!(du_code(&du_func(c)), c, "{c:#x}");
        }
        let kb = crate::models::MODELS.iter().filter_map(|m| m.kb).find(|k| k.du).unwrap();
        let mut cur = KbConfig::blank(kb);
        let raw = kb.glen as usize - DU_LIGHT;
        cur.general[raw + 1] = kb.leds[0].hw;
        let mut new = cur.clone();
        assert!(du_packets(kb, &new, &cur, &[], 4).is_empty());
        new.matrix[1] = 0x290020;
        let i = kb.leds.iter().position(|l| l.random).unwrap();
        new.set_mode(kb, i, None);
        new.set_light(kb, i, Light { color: COLORS as u8, speed: 1, bright: 5 });
        let p = du_packets(kb, &new, &cur, &[], 4);
        assert_eq!(p[0].0, 9);
        assert_eq!(&p[0].2[3..6], &[0x20, 0, 0x29]);
        assert_eq!((p[1].0, p[1].1, p[1].2.len()), (6, 0, DU_LIGHT));
        assert_eq!(&p[1].2[1..6], &[kb.leds[i].hw, 5, 3, 0, 1]);
    }

    #[test]
    fn mk_area0_follows_the_driver() {
        // FUN_00437d9c on the driver's default area 0 (effect group 3 variant 0, colourful)
        let mut a0 = vec![0, 0, 5, 0, 1, 8, 2, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 7, 1, 2, 1, 1, 0, 0];
        mk_sum(&mut a0);
        assert_eq!(&a0[0x16..], &(0x8032u16 + 0x1f).to_le_bytes());
        let kb = crate::models::MODELS.iter().filter_map(|m| m.kb).find(|k| k.mk).unwrap();
        let cur = KbConfig { general: mk_general(kb, &a0, &[0; 0x6a]), matrix: vec![0; MATRIX] };
        assert_eq!(cur.mode(kb), 9);
        assert_eq!(cur.light(kb, 9).color, COLORS as u8);
        let mut same = a0.clone();
        same[4] = 1;
        mk_sum(&mut same);
        assert_eq!(mk_area0(kb, &cur), same);
        let mut new = cur.clone();
        new.set_mode(kb, 13, None); // group 4, variant 1
        new.set_light(kb, 13, Light { color: 2, speed: 4, bright: 1 });
        let a = mk_area0(kb, &new);
        assert_eq!((a[8], a[9 + 4], a[5], a[6], a[2]), (4, 1, 2, 4, 1));
        assert_eq!(a[9 + 3], 0); // the other groups keep their variant
        let mut b = a.clone();
        mk_sum(&mut b);
        assert_eq!(a, b);
    }

    #[test]
    fn mg_block_follows_the_driver() {
        // FUN_00468680: 00 55 02 00 SUM .. (begin / end carry no data)
        let p = mg_packet(5, 0x40, &[], 0x20);
        assert_eq!(&p[..9], &[0, 0x55, 5, 0, 0x60, 0x20, 0x40, 0, 0]);
        let kb = crate::models::MODELS.iter().filter_map(|m| m.kb).find(|k| k.mg).unwrap();
        let mut a = vec![0u8; MG];
        a[4] = 1;
        a[8] = 4; // breathing
        a[9] = 100;
        a[10] = 0; // speed 4
        a[12] = 1;
        let cur = KbConfig { general: mg_general(kb, &a, 1), matrix: vec![0; MATRIX] };
        assert_eq!(kb.leds[cur.mode(kb)].hw, 4);
        assert_eq!(cur.light(kb, cur.mode(kb)).color, COLORS as u8);
        assert_eq!(mg_block(kb, &cur), a);
        let mut new = cur.clone();
        let i = kb.leds.iter().position(|l| l.hw == 3).unwrap(); // static
        new.set_mode(kb, i, None);
        new.set_light(kb, i, Light { color: 0, speed: 0, bright: 0 });
        let n = mg_block(kb, &new);
        let rgb = kb.default_palette()[0];
        assert_eq!((n[8], n[9], n[10], n[12]), (3, MG_BRIGHT[0], kb.speed[0], 0));
        assert_eq!(&n[14..17], &[(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8]);
        assert_eq!(n[4], 1);
    }
}
