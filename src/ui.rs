//! The configuration UI, drawn into a [`Canvas`] at the exact pixel
//! coordinates of the original ldcfg.exe (measured on captures of it running).
//! Immediate mode: every frame redraws the stack of layers (main window,
//! popup menus, dialogs, macro manager) and the topmost layer gets the input.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use crate::gfx::*;
use crate::model::*;

// ------------------------------------------------------------------ input

#[derive(Default, Clone)]
pub struct KeyEv {
    pub name: String,
    pub pressed: bool,
    pub repeat: bool,
}

#[derive(Default, Clone)]
pub struct Input {
    pub mouse: (i32, i32),
    pub down: bool,
    pub pressed: bool,
    pub released: bool,
    pub right_pressed: bool,
    pub double: bool,
    /// Wheel notches, positive = up.
    pub wheel: i32,
    pub keys: Vec<KeyEv>,
    pub text: String,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub logo: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Request {
    Minimize,
    Close,
    Drag,
}

// ------------------------------------------------------------------ strings

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    Tc,
    Sc,
    Pt,
    En,
    Ru,
}

impl Lang {
    pub const ALL: [Lang; 5] = [Lang::Tc, Lang::Sc, Lang::Pt, Lang::En, Lang::Ru];
    pub fn code(self) -> &'static str {
        match self {
            Lang::Tc => "tc",
            Lang::Sc => "sc",
            Lang::Pt => "pt",
            Lang::En => "en",
            Lang::Ru => "ru",
        }
    }
    pub fn from_code(s: &str) -> Option<Lang> {
        let s = s.to_ascii_lowercase();
        Some(match s.as_str() {
            "tc" | "zh_tw" | "zh-tw" | "zh_hk" => Lang::Tc,
            "sc" | "zh_cn" | "zh-cn" | "zh" => Lang::Sc,
            "pt" | "pt_br" | "pt-br" => Lang::Pt,
            "en" | "en_us" | "en-us" => Lang::En,
            "ru" | "ru_ru" | "ru-ru" => Lang::Ru,
            _ => return None,
        })
    }
    /// Self-name shown in the language row of the info page.
    pub fn native(self) -> &'static str {
        match self {
            Lang::Tc => "繁體中文",
            Lang::Sc => "简体中文",
            Lang::Pt => "Português",
            Lang::En => "English",
            Lang::Ru => "Русский",
        }
    }
    fn files(self) -> (&'static str, &'static str) {
        match self {
            Lang::Tc => (include_str!("../assets/strings/tc/general.ini"), include_str!("../assets/strings/tc/skins.ini")),
            Lang::Sc => (include_str!("../assets/strings/sc/general.ini"), include_str!("../assets/strings/sc/skins.ini")),
            Lang::Pt => (include_str!("../assets/strings/pt/general.ini"), include_str!("../assets/strings/pt/skins.ini")),
            Lang::En => (include_str!("../assets/strings/en/general.ini"), include_str!("../assets/strings/en/skins.ini")),
            Lang::Ru => (include_str!("../assets/strings/ru/general.ini"), include_str!("../assets/strings/ru/skins.ini")),
        }
    }
}

pub struct Strings {
    general: Ini,
    skins: Ini,
}

impl Strings {
    pub fn load(lang: Lang) -> Strings {
        let (g, s) = lang.files();
        Strings { general: parse_ini(g), skins: parse_ini(s) }
    }
    /// general.ini lookup
    pub fn g(&self, sec: &str, key: u32) -> String {
        self.general.get(sec).and_then(|m| m.get(&key.to_string())).cloned().unwrap_or_default()
    }
    /// skins.ini lookup
    pub fn s(&self, sec: &str, key: u32) -> String {
        self.skins.get(sec).and_then(|m| m.get(&key.to_string())).cloned().unwrap_or_default()
    }
}

// ------------------------------------------------------------------ text styles

const WHITE: Rgba = rgb(0xffffff);
const RED: Rgba = rgb(0xe60012);
const T_TAB: TextStyle = TextStyle::new(13.0, Weight::Bold, WHITE).cjk(18.0);
const T_FUNC: TextStyle = TextStyle::new(12.0, Weight::Bold, WHITE).cjk(15.8).dy(1);
const T_TITLE: TextStyle = TextStyle::new(14.0, Weight::Bold, WHITE);
const T_POLL: TextStyle = TextStyle::new(12.0, Weight::Regular, WHITE);
const T_PROFILE: TextStyle = TextStyle::new(13.0, Weight::Regular, WHITE);
const T_BOTTOM: TextStyle = TextStyle::new(13.0, Weight::Regular, WHITE).cjk(14.0).dy(-1);
const T_STAGE: TextStyle = TextStyle::new(14.0, Weight::Bold, WHITE);
const T_DPIVAL: TextStyle = TextStyle::new(12.0, Weight::Bold, WHITE);
const T_LTAB: TextStyle = TextStyle::new(16.0, Weight::Bold, WHITE);
const T_LLABEL: TextStyle = TextStyle::new(13.0, Weight::Bold, WHITE);
const T_LROW: TextStyle = TextStyle::new(12.0, Weight::Regular, WHITE);
const T_INFO: TextStyle = TextStyle::new(14.2, Weight::Regular, WHITE).cjk(12.5).dy(-1);
const T_MENU: TextStyle = TextStyle::new(13.0, Weight::Regular, WHITE).cjk(14.5);
const T_DLG_TITLE: TextStyle = TextStyle::new(19.0, Weight::Bold, WHITE).cjk(22.3).italic();
const T_DLG: TextStyle = TextStyle::new(14.0, Weight::Regular, WHITE);
const T_FK: TextStyle = TextStyle::new(12.0, Weight::Regular, WHITE).cjk(13.5);
const T_STAGE_TXT: TextStyle = TextStyle::new(12.0, Weight::Bold, WHITE).cjk(15.0);
const T_MAC_TITLE: TextStyle = TextStyle::new(20.0, Weight::Bold, WHITE).cjk(21.2).italic();
const T_MAC_HEAD: TextStyle = TextStyle::new(15.0, Weight::Bold, WHITE);
const T_MAC: TextStyle = TextStyle::new(12.0, Weight::Regular, WHITE).cjk(12.8);
const T_MAC_OPT: TextStyle = TextStyle::new(13.0, Weight::Regular, WHITE).cjk(13.5);
const T_MAC_BTN: TextStyle = TextStyle::new(14.0, Weight::Regular, WHITE);
const T_BUBBLE: TextStyle = TextStyle::new(11.0, Weight::Bold, WHITE);

// ------------------------------------------------------------------ layout (measured)

pub const WIN_W: i32 = 795;
pub const WIN_H: i32 = 625;

const TAB_X: [i32; 4] = [73, 238, 403, 567];
fn tab_rect(i: usize) -> Rect {
    Rect::new(TAB_X[i], 163, 160, 18)
}
/// Button rows that fit on the general page.
const ROWS: usize = 11;
fn func_rect(i: usize) -> Rect {
    Rect::new(336, 202 + 28 * i as i32, 141, 18)
}
fn profile_rect(i: usize) -> Rect {
    Rect::new(110 + 120 * i as i32, 525, 83, 19)
}
const BOTTOM_X: [i32; 7] = [59, 159, 257, 357, 455, 553, 651];
const BOTTOM_W: [i32; 7] = [89, 89, 89, 89, 90, 89, 89];
fn bottom_rect(i: usize) -> Rect {
    Rect::new(BOTTOM_X[i], 562, BOTTOM_W[i], 24)
}
/// General page sliders: (title y, track y) for acceleration, pointer, scroll, double-click.
const SLIDERS: [(i32, i32); 4] = [(199, 217), (246, 266), (293, 312), (346, 372)];
const TRACK_X: i32 = 511;
const TRACK_SPAN: i32 = 195; // thumb travel in px (706 - 511)
const POLL_X: [i32; 4] = [511, 566, 621, 676];
const DPI_CX: [i32; 5] = [178, 303, 428, 548, 668];
const DPI_LABEL_X: [i32; 5] = [146, 271, 396, 516, 636];
/// Thumb top for a slider position (0..n-1 over the model's DPI values).
/// The M801 track has 93 positions, index 1..93 of its pfd table; the slope
/// was measured by sweeping the original slider. Other models spread their
/// values over the same travel.
fn dpi_thumb_y(pos: usize, n: usize) -> i32 {
    let idx = 1.0 + pos as f32 * 92.0 / (n.max(2) - 1) as f32;
    (425.298 - 1.8246 * idx).round() as i32
}
fn dpi_pos_at(y: i32, n: usize) -> usize {
    let idx = (9.0 + (422.0 - y as f32) / 1.8246).clamp(1.0, 93.0);
    ((idx - 1.0) * (n.max(2) - 1) as f32 / 92.0).round() as usize
}
const SWATCH_X0: i32 = 107;
const SWATCH_Y: i32 = 288;
const PICKER: Rect = Rect::new(329, 293, 130, 122);
const MENU_W: i32 = 148;
const MENU_ITEM_H: i32 = 20;
const DLG: Rect = Rect::new(158, 166, 478, 292);
const MAC: Rect = Rect::new(19, 21, 757, 582);
const MAC_ROWS_Y: i32 = 164;
const MAC_ROW_H: i32 = 23;
const MAC_VISIBLE: usize = 11;
const MAC_LIST_Y: i32 = 142;

/// Lighting page widgets present for each mode (as captured from the original):
/// breathing, neon, static, wave, trail, reactive, flash, off.
#[derive(Clone, Copy)]
struct LightLayout {
    color: bool,
    speed: bool,
    off: bool,
    /// x of the brightness column and the speed column labels
    bright_x: i32,
    speed_x: i32,
    speed_y: i32,
}
const LIGHT_LAYOUT: [LightLayout; 8] = [
    LightLayout { color: true, speed: true, off: false, bright_x: 509, speed_x: 509, speed_y: 380 },
    LightLayout { color: false, speed: true, off: false, bright_x: 109, speed_x: 249, speed_y: 260 },
    LightLayout { color: true, speed: false, off: false, bright_x: 509, speed_x: 0, speed_y: 0 },
    LightLayout { color: false, speed: true, off: false, bright_x: 109, speed_x: 249, speed_y: 260 },
    LightLayout { color: false, speed: false, off: false, bright_x: 109, speed_x: 0, speed_y: 0 },
    LightLayout { color: false, speed: false, off: false, bright_x: 109, speed_x: 0, speed_y: 0 },
    LightLayout { color: true, speed: true, off: false, bright_x: 509, speed_x: 509, speed_y: 380 },
    LightLayout { color: false, speed: false, off: true, bright_x: 0, speed_x: 0, speed_y: 0 },
];
/// skins.ini keys of the 8 mode tabs, top row then bottom row.
const LIGHT_KEYS: [u32; 8] = [80, 81, 82, 83, 84, 85, 86, 87];

// ------------------------------------------------------------------ resources

pub struct Res {
    imgs: HashMap<&'static str, Image>,
    pub fonts: Fonts,
    pub strs: Strings,
}

impl Res {
    pub fn new(fonts: Fonts, lang: Lang) -> Res {
        let mut imgs: HashMap<&'static str, Image> = HashMap::new();
        for (name, bytes) in crate::assets::FILES {
            imgs.insert(name, Image::decode(bytes));
        }
        // scaled pieces: the original stretched these at runtime
        let mouse = imgs["mouse-layout-front"].resize_nearest(189, 270);
        imgs.insert("mouse", mouse);
        // selection window thumbnails (photos are 189x270 boxes)
        let thumbs: Vec<(&'static str, Image)> = imgs
            .iter()
            .filter(|(k, _)| k.starts_with("mice/") || k.starts_with("kbd/") || **k == "mouse")
            .map(|(k, im)| {
                // keyboards are wide: 36 px across, centred in the row
                let t = if k.starts_with("kbd/") { im.resize_nearest(36, (im.h * 36 / im.w.max(1)).max(1)) } else { im.resize_nearest(30, 43) };
                (&*Box::leak(format!("thumb/{k}").into_boxed_str()), t)
            })
            .collect();
        imgs.extend(thumbs);
        Res { imgs, fonts, strs: Strings::load(lang) }
    }
    fn img(&self, n: &str) -> &Image {
        self.imgs.get(n).unwrap_or_else(|| panic!("missing image {n}"))
    }
}

struct P<'a> {
    r: &'a Res,
    cv: &'a mut Canvas,
}

impl P<'_> {
    fn img(&mut self, n: &str, x: i32, y: i32) {
        let im = self.r.img(n);
        self.cv.blit(im, x, y);
    }
    /// Draw an alpha-mask skin piece in a solid colour.
    fn tint(&mut self, n: &str, x: i32, y: i32, c: Rgba) {
        let im = self.r.img(n);
        for j in 0..im.h {
            for i in 0..im.w {
                let a = im.get(i, j).3;
                if a > 0 {
                    self.cv.blend(x + i, y + j, c, a as f32 / 255.0);
                }
            }
        }
    }

    /// Draw a skin piece widened to `w` by repeating its middle column.
    fn img_w(&mut self, n: &str, x: i32, y: i32, w: i32) {
        let im = self.r.img(n);
        let half = im.w / 2;
        self.cv.blit_part(im, Rect::new(0, 0, half, im.h), x, y);
        for k in 0..(w - im.w).max(0) {
            self.cv.blit_part(im, Rect::new(half, 0, 1, im.h), x + half + k, y);
        }
        let rest = im.w - half;
        self.cv.blit_part(im, Rect::new(half, 0, rest, im.h), x + w - rest, y);
    }

    fn text(&mut self, x: i32, y: i32, s: &str, st: &TextStyle) {
        self.cv.text(&self.r.fonts, x, y, s, st);
    }
    fn text_in(&mut self, r: Rect, s: &str, st: &TextStyle) {
        self.cv.text_in(&self.r.fonts, r, s, st, Align::Center, 0);
    }
    fn text_al(&mut self, r: Rect, s: &str, st: &TextStyle, a: Align) {
        self.cv.text_in(&self.r.fonts, r, s, st, a, 0);
    }
    fn fill(&mut self, r: Rect, c: Rgba) {
        self.cv.fill(r, c);
    }
    fn width(&self, s: &str, st: &TextStyle) -> i32 {
        self.r.fonts.measure(s, st)
    }
    /// `s` shortened with "..." so it is at most `max` px wide.
    fn ellipsize(&self, s: &str, st: &TextStyle, max: i32) -> String {
        if self.width(s, st) <= max {
            return s.to_string();
        }
        let chars: Vec<char> = s.chars().collect();
        for n in (0..chars.len()).rev() {
            let t: String = chars[..n].iter().collect::<String>().trim_end().to_string() + "...";
            if self.width(&t, st) <= max {
                return t;
            }
        }
        "...".into()
    }

    /// Centred label that never extends past `right` (exclusive): if centring
    /// would cross it, the label is centred in [r.x + pad, right) and shortened
    /// with "..." when it still does not fit.
    fn label_fit(&mut self, r: Rect, s: &str, st: &TextStyle, right: i32) {
        let pad = 4;
        let w = self.width(s, st);
        if r.x + (r.w + w) / 2 <= right {
            self.text_in(r, s, st);
            return;
        }
        let area = Rect::new(r.x + pad, r.y, right - r.x - pad, r.h);
        // first try up to 2px smaller, then cut with "..."
        for d in [1.0, 2.0] {
            let mut sm = *st;
            sm.size -= d;
            if sm.cjk > 0.0 {
                sm.cjk -= d;
            }
            if self.width(s, &sm) <= area.w {
                self.text_in(area, s, &sm);
                return;
            }
        }
        let t = self.ellipsize(s, st, area.w);
        self.text_in(area, &t, st);
    }

    /// Word/char wrap inside `w` px; returns lines.
    fn wrap(&self, s: &str, st: &TextStyle, w: i32) -> Vec<String> {
        let mut lines = vec![String::new()];
        for c in s.chars() {
            let cur = lines.last_mut().unwrap();
            let mut t = cur.clone();
            t.push(c);
            if self.width(&t, st) > w && !cur.is_empty() {
                lines.push(c.to_string());
            } else {
                *cur = t;
            }
        }
        lines
    }
    /// Like `wrap`, but breaks between words when the text has spaces.
    fn wrap_words(&self, s: &str, st: &TextStyle, w: i32) -> Vec<String> {
        if !s.contains(' ') {
            return self.wrap(s, st, w);
        }
        let mut lines: Vec<String> = Vec::new();
        for word in s.split(' ') {
            match lines.last_mut() {
                Some(cur) if self.width(&format!("{cur} {word}"), st) <= w => {
                    cur.push(' ');
                    cur.push_str(word);
                }
                _ => lines.push(word.to_string()),
            }
        }
        lines
    }
}

// ------------------------------------------------------------------ menus

#[derive(Clone, Debug)]
enum MenuAct {
    Set(Func),
    AssignKey,
    Combo,
    FireKey,
    MacroManager,
    // macro manager "more..." menu
    InsertMouse(u8),
    InsertDelay,
    /// keyboards: firmware code for the key
    KSet(u32),
}

#[derive(Clone, Debug)]
struct MenuItem {
    label: String,
    act: Option<MenuAct>,
    sub: Vec<MenuItem>,
    arrow: bool,
}

impl MenuItem {
    fn leaf(label: String, act: MenuAct) -> Self {
        MenuItem { label, act: Some(act), sub: Vec::new(), arrow: false }
    }
    fn sub(label: String, sub: Vec<MenuItem>) -> Self {
        MenuItem { label, act: None, sub, arrow: true }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum MenuOwner {
    Button(usize),
    MacroMore,
    /// keyboards: index in `Kbd::keys`
    Key(usize),
}

struct MenuState {
    owner: MenuOwner,
    items: Vec<MenuItem>,
    x: i32,
    y: i32,
    open: Option<usize>,
    /// false until the release of the click that opened the menu has passed
    armed: bool,
}

// ------------------------------------------------------------------ dialogs

#[derive(Clone, Copy, Debug, PartialEq)]
enum Confirm {
    RestoreOne,
    RestoreAll,
    DeleteMacro,
    LeaveMacro,
}

#[derive(Clone, Debug)]
enum Dialog {
    AssignKey { button: usize, key: String },
    Combo { button: usize, key: String },
    FireKey { button: usize, fk: FireKey, focus: u8, times: String, delay: String },
    NewMacro { name: String },
    Confirm(Confirm),
    Notice(String),
    /// The mouse is there but its device node is root-only: offer the udev rule.
    Permission,
    /// Several supported mice are plugged: pick the one to configure.
    SelectDevice { list: Vec<crate::device::Found>, sel: usize, top: usize },
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum MacField {
    DefaultDelay,
    Loops,
    Step(usize),
}

struct MacroEd {
    button: Option<usize>,
    macros: Vec<Macro>,
    sel: Option<usize>,
    row: Option<usize>,
    scroll: usize,
    recording: bool,
    /// recording started from "Record key" (insert at the row) rather than the record button
    rec_inserting: bool,
    rec_insert: usize,
    last_event: Option<Instant>,
    held: Vec<String>,
    clipboard: Vec<MacroStep>,
    edit: Option<(MacField, String)>,
    dirty: bool,
}

enum Layer {
    Menu(MenuState),
    Dialog(Dialog),
    Macro(MacroEd),
}

/// Deferred state changes produced while drawing.
enum Cmd {
    Push(Layer),
    Pop,
    PopMenus,
    SetFunc(usize, Func),
    MenuAct(MenuOwner, MenuAct),
}

// ------------------------------------------------------------------ state

pub struct State {
    pub tab: usize,
    pub profiles: Vec<Profile>,
    saved: Vec<Profile>,
    pub cur: usize,
    pub macros: Vec<Macro>,
    pub lang: Lang,
    layers: Vec<Layer>,
    cmds: Vec<Cmd>,
    press_at: Option<(i32, i32)>,
    drag: Option<(u8, usize)>,
    dpi_edit: Option<(usize, String)>,
    pub requests: Vec<Request>,
    pub lang_changed: bool,
    /// Configuration root; `dir` is the active model's profile directory in it.
    base: PathBuf,
    dir: PathBuf,
    /// USB port of the mouse being configured, once chosen.
    target: Option<String>,
    pub status: Option<(String, Instant)>,
    inp: Input,
    btn_hover: Option<usize>,
    /// First button row shown when the model has more buttons than rows.
    btn_top: usize,
    /// The connected mouse and what it held when last read/written.
    device: Option<(crate::device::Device, crate::device::DeviceConfig)>,
    /// Apply running in the background (write, reconnect, reopen).
    busy: Option<Busy>,
    /// Keyboards (models with `kb`): what is being edited, its saved copy, the
    /// connected keyboard with what it held.
    kprof: Option<crate::kbd::KbProfile>,
    ksaved: Option<crate::kbd::KbProfile>,
    kdev: Option<(crate::device::Device, crate::kbd::KbConfig)>,
    /// Live window: follows devices being plugged in and out (None for screenshots and --no-device).
    hotplug: Option<Hotplug>,
}

struct Hotplug {
    last: Instant,
    /// (USB port, hidraw node) of every supported device at the last scan
    seen: Vec<(String, PathBuf)>,
    /// No supported device plugged: the fallback screen is shown...
    none: bool,
    /// ...unless the user chose to edit the profiles without a device.
    offline: bool,
    /// Last profile write of the background mode (daemon.rs).
    applied: Option<std::time::SystemTime>,
}

/// Progress messages of a background Apply.
pub enum ApplyMsg {
    Phase(u8),
    Done(Result<(crate::device::Device, crate::device::DeviceConfig, Vec<Macro>), String>),
    KbDone(Result<(crate::device::Device, crate::kbd::KbConfig), String>),
}

struct Busy {
    rx: std::sync::mpsc::Receiver<ApplyMsg>,
    phase: u8,
    since: Instant,
    close_after: bool,
}

const DRAG_SLIDER: u8 = 1;
const DRAG_DPI: u8 = 2;
const DRAG_PICKER: u8 = 3;

impl State {
    pub fn new(base: PathBuf, lang: Lang) -> State {
        let dir = crate::models::current().profile_dir(&base);
        let (profiles, macros) = Self::load_dir(&dir);
        let mut st = State {
            tab: 0,
            saved: profiles.clone(),
            profiles,
            cur: 0,
            macros,
            lang,
            layers: Vec::new(),
            cmds: Vec::new(),
            press_at: None,
            drag: None,
            dpi_edit: None,
            requests: Vec::new(),
            lang_changed: false,
            base,
            dir,
            target: None,
            status: None,
            inp: Input::default(),
            btn_hover: None,
            btn_top: 0,
            device: None,
            busy: None,
            kprof: None,
            ksaved: None,
            kdev: None,
            hotplug: None,
        };
        st.load_kbd();
        st
    }

    /// The keyboard profile of the current model (models/<key>/Keyboard.ini).
    fn load_kbd(&mut self) {
        let m = crate::models::current();
        self.kprof = m.kb.map(|kb| crate::kbd::KbProfile::load(&self.dir.join("Keyboard.ini"), kb).unwrap_or_else(|| crate::kbd::KbProfile::blank(kb)));
        self.ksaved = self.kprof.clone();
        self.kdev = None;
        if m.is_kbd() && self.tab > 2 {
            self.tab = 2;
        }
    }

    fn load_dir(dir: &std::path::Path) -> (Vec<Profile>, Vec<Macro>) {
        let profiles = (0..5).map(|i| Profile::load(&dir.join(format!("RSProfile{}.pfd", i + 1))).unwrap_or_default()).collect();
        (profiles, load_macros(dir))
    }

    /// Makes `m` the model being edited: its profiles, layout and encoders.
    pub fn set_model(&mut self, m: &'static crate::models::Model) {
        if std::ptr::eq(crate::models::current(), m) && !self.profiles.is_empty() {
            return;
        }
        crate::models::set_current(m);
        self.dir = m.profile_dir(&self.base);
        let (profiles, macros) = Self::load_dir(&self.dir);
        self.saved = profiles.clone();
        self.profiles = profiles;
        self.macros = macros;
        self.dpi_edit = None;
        self.device = None;
        self.load_kbd();
    }

    /// Finds the plugged mice: one is opened directly, several open the
    /// selection window (the chosen one is remembered for this session).
    pub fn attach_device(&mut self) {
        use crate::device::*;
        if let Access::NoPermission(_) = access() {
            if !self.layers.iter().any(|l| matches!(l, Layer::Dialog(Dialog::Permission))) {
                self.layers.push(Layer::Dialog(Dialog::Permission));
            }
            return;
        }
        let found = scan();
        // one USB device may be listed once per candidate model (HidServ keyboards): the
        // current model's entry first
        let cur = crate::models::current();
        let pick = self
            .target
            .as_ref()
            .and_then(|t| {
                let mut on = found.iter().filter(|f| &f.usb_name() == t);
                on.clone().find(|f| std::ptr::eq(f.model, cur)).or_else(|| on.next())
            })
            .or_else(|| if found.len() == 1 { found.first() } else { None });
        let Some(f) = pick.cloned() else {
            if found.len() > 1 {
                self.open_selector(found);
            } else if self.hotplug.is_none() {
                self.flash(format!("{}: not connected", crate::models::current().name));
            }
            return;
        };
        self.target = Some(f.usb_name());
        self.set_model(f.model);
        let _excl = exclusive();
        if let Some(kb) = f.model.kb {
            let held = self.kprof.as_ref().map(|p| p.cfg.clone()).filter(|_| kb.write_only());
            let read = |d: &Device| held.clone().map_or_else(|| crate::kbd::read_config(d, kb), Ok);
            match Device::open(&f).and_then(|d| read(&d).map(|c| (d, c))) {
                Ok((d, c)) => {
                    if let Some(p) = &mut self.kprof {
                        p.cfg = c.clone();
                    }
                    self.ksaved = self.kprof.clone();
                    self.flash(format!("{}: {}", f.model.name, d.path.display()));
                    self.kdev = Some((d, c));
                }
                Err(e) => self.flash(format!("{}: {e}", f.model.name)),
            }
            return;
        }
        match Device::open(&f).and_then(|d| read_config(&d).map(|c| (d, c))) {
            Ok((d, cfg)) => {
                // xyui mice report nothing: the UI's profiles stay what Apply sends
                if !f.model.write_only() {
                    self.macros = macros_from_device(&cfg, &self.macros);
                    for (i, dp) in cfg.profiles.iter().enumerate() {
                        // block 1 holds a copy of the selected profile: profile 1 stays as saved
                        if i == 0 && cfg.active != 0 && f.model.single_live_profile() {
                            continue;
                        }
                        apply_to_profile_with(dp, &mut self.profiles[i], &self.macros);
                    }
                    self.cur = (cfg.active as usize).min(4);
                }
                self.saved = self.profiles.clone();
                self.flash(format!("{}: {}", f.model.name, d.path.display()));
                self.device = Some((d, cfg));
            }
            Err(e) => self.flash(format!("{}: {e}", f.model.name)),
        }
    }

    /// Starts following plugged and unplugged devices (live window only).
    pub fn enable_hotplug(&mut self) {
        let found = crate::device::scan();
        let applied = crate::daemon::applied_at();
        self.hotplug = Some(Hotplug { last: Instant::now(), seen: plug_sig(&found), none: found.is_empty(), offline: false, applied });
    }

    /// Every second: a newly plugged device becomes the one being configured, an
    /// unplugged one hands over to what is left (or the fallback screen), and a
    /// re-enumerated one (Apply from the background mode) is read again.
    fn poll_hotplug(&mut self) {
        use crate::device::*;
        let Some(h) = &mut self.hotplug else { return };
        if self.busy.is_some() || h.last.elapsed() < std::time::Duration::from_secs(1) {
            return;
        }
        h.last = Instant::now();
        let found = scan();
        let sig = plug_sig(&found);
        let applied = crate::daemon::applied_at();
        // an open dialog or editor may refer to the current model: wait until it closes
        if (sig == h.seen && applied == h.applied) || self.layers.iter().any(|l| !matches!(l, Layer::Dialog(Dialog::SelectDevice { .. }))) {
            return;
        }
        let reread = applied != h.applied;
        h.applied = applied;
        let old = std::mem::replace(&mut h.seen, sig);
        h.none = found.is_empty();
        if h.none {
            h.offline = false;
        }
        self.layers.clear();
        if found.is_empty() {
            self.device = None;
            self.kdev = None;
            return;
        }
        let open = self.device.as_ref().map(|(d, _)| d.path.clone()).or_else(|| self.kdev.as_ref().map(|(d, _)| d.path.clone()));
        if let Some(f) = found.iter().find(|f| !old.iter().any(|(u, _)| *u == f.usb_name())) {
            self.target = Some(f.usb_name());
        } else if let Some(t) = &self.target {
            if !found.iter().any(|f| &f.usb_name() == t) {
                self.target = None;
            } else if !reread && open.is_some_and(|p| found.iter().any(|f| f.hidraw == p)) {
                return;
            }
        }
        self.device = None;
        self.kdev = None;
        self.attach_device();
    }

    /// Hot-plug mode with nothing plugged: the fallback screen replaces the editor.
    fn no_device(&self) -> bool {
        self.hotplug.as_ref().is_some_and(|h| h.none && !h.offline)
    }

    /// Picks the mouse to configure (before `attach_device`).
    pub fn choose(&mut self, f: &crate::device::Found) {
        self.target = Some(f.usb_name());
        self.set_model(f.model);
    }

    fn open_selector(&mut self, list: Vec<crate::device::Found>) {
        if self.layers.iter().any(|l| matches!(l, Layer::Dialog(Dialog::SelectDevice { .. }))) {
            return;
        }
        let cur = crate::models::current();
        let sel = self
            .target
            .as_ref()
            .and_then(|t| {
                list.iter().position(|f| &f.usb_name() == t && std::ptr::eq(f.model, cur)).or_else(|| list.iter().position(|f| &f.usb_name() == t))
            })
            .unwrap_or(0);
        self.layers.push(Layer::Dialog(Dialog::SelectDevice { list, sel, top: 0 }));
    }

    /// Sends the UI profiles to the mouse (only changed fields), verified by read-back.
    /// Apply button's device step, also used by `--device-apply` (blocking).
    pub fn apply_to_device(&mut self) -> Result<(), String> {
        let rx = self.spawn_apply().ok_or("mouse not connected")?;
        loop {
            match rx.recv().map_err(|e| e.to_string())? {
                ApplyMsg::Phase(_) => {}
                ApplyMsg::Done(r) => {
                    let (d, c, m) = r?;
                    self.device = Some((d, c));
                    self.macros = m;
                    return Ok(());
                }
                ApplyMsg::KbDone(r) => {
                    let (d, c) = r?;
                    if let Some(p) = &mut self.kprof {
                        p.cfg = c.clone();
                    }
                    self.kdev = Some((d, c));
                    return Ok(());
                }
            }
        }
    }

    /// Starts a background Apply; false when no mouse is connected.
    fn start_apply(&mut self, close_after: bool) -> bool {
        match self.spawn_apply() {
            Some(rx) => {
                self.busy = Some(Busy { rx, phase: 0, since: Instant::now(), close_after });
                true
            }
            None => false,
        }
    }

    /// Hands the device to a worker thread: (re)open + read, write only the
    /// changed fields (verified), re-enumerate the mouse, reopen it.
    fn spawn_apply(&mut self) -> Option<std::sync::mpsc::Receiver<ApplyMsg>> {
        use crate::device::*;
        if crate::models::current().is_kbd() {
            return self.spawn_kbd_apply();
        }
        if self.device.is_none() && self.target.is_none() && scan().len() != 1 {
            return None;
        }
        if self.device.is_none() {
            if let Access::NoPermission(_) = access() {
                self.layers.push(Layer::Dialog(Dialog::Permission));
                return None;
            }
        }
        let usb = self.device.as_ref().map(|(d, _)| d.found.usb_name()).or_else(|| self.target.clone());
        let held = self.device.take();
        let profiles = self.profiles.clone();
        let mut macros = self.macros.clone();
        let active = self.cur as u8;
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            // fresh read: survives unplug/replug and a mouse attached after start
            drop(held);
            let phase = |p| {
                let _ = tx.send(ApplyMsg::Phase(p));
            };
            let r = apply_profiles(usb.as_deref(), &profiles, &mut macros, active, false, &phase).map(|(d, c)| {
                crate::pointer::apply(&d.found, &profiles[active as usize]);
                (d, c, macros)
            });
            let _ = tx.send(ApplyMsg::Done(r));
        });
        Some(rx)
    }

    /// Keyboard Apply: read what the keyboard holds, write the changed blocks (with
    /// the colour table, as the drivers do), read back. No re-enumeration needed.
    fn spawn_kbd_apply(&mut self) -> Option<std::sync::mpsc::Receiver<ApplyMsg>> {
        use crate::device::*;
        let kb = crate::models::current().kb?;
        let usb = self.kdev.as_ref().map(|(d, _)| d.found.usb_name()).or_else(|| self.target.clone());
        if usb.is_none() && scan().iter().filter(|f| f.model.is_kbd()).count() != 1 {
            return None;
        }
        if self.kdev.is_none() {
            if let Access::NoPermission(_) = access() {
                self.layers.push(Layer::Dialog(Dialog::Permission));
                return None;
            }
        }
        let held = self.kdev.take();
        let want = self.kprof.clone()?;
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let r = (|| -> Result<(Device, crate::kbd::KbConfig), String> {
                let _ = tx.send(ApplyMsg::Phase(0));
                drop(held);
                let here: Vec<Found> =
                    scan().into_iter().filter(|f| f.model.is_kbd() && usb.as_deref().is_none_or(|u| f.usb_name() == u)).collect();
                let f = here
                    .iter()
                    .find(|f| f.model.kb.is_some_and(|k| std::ptr::eq(k, kb)))
                    .or(here.first())
                    .cloned()
                    .ok_or("keyboard not connected")?;
                // another keyboard on that port: its key matrix is laid out differently
                if !f.model.kb.is_some_and(|k| std::ptr::eq(k, kb)) {
                    return Err(format!("the connected keyboard is a {}", f.model.name));
                }
                let d = Device::open(&f).map_err(|e| e.to_string())?;
                // xyui / Jieli: nothing to read, everything is sent
                if kb.write_only() {
                    let _ = tx.send(ApplyMsg::Phase(1));
                    crate::kbd::write_config(&d, kb, &want.cfg, &want.cfg, &want.palettes).map_err(|e| e.to_string())?;
                    return Ok((d, want.cfg));
                }
                let current = crate::kbd::read_config(&d, kb).map_err(|e| e.to_string())?;
                let _ = tx.send(ApplyMsg::Phase(1));
                let new = crate::kbd::merged(kb, &current, &want.cfg);
                crate::kbd::write_config(&d, kb, &new, &current, &want.palettes).map_err(|e| e.to_string())?;
                let back = crate::kbd::read_config(&d, kb).map_err(|e| e.to_string())?;
                if !crate::kbd::same_settings(kb, &back, &new) {
                    return Err("written, but the keyboard reads back different settings".into());
                }
                Ok((d, back))
            })();
            let _ = tx.send(ApplyMsg::KbDone(r));
        });
        Some(rx)
    }

    /// Polls a running Apply; returns true while it is still busy.
    fn poll_apply(&mut self) -> bool {
        let Some(b) = &mut self.busy else { return false };
        let mut done = None;
        let mut kdone = None;
        while let Ok(m) = b.rx.try_recv() {
            match m {
                ApplyMsg::Phase(p) => b.phase = p,
                ApplyMsg::Done(r) => done = Some(r),
                ApplyMsg::KbDone(r) => kdone = Some(r),
            }
        }
        let close = b.close_after;
        if let Some(r) = kdone {
            self.busy = None;
            match r {
                Ok((d, c)) => {
                    if let Some(p) = &mut self.kprof {
                        p.cfg = c.clone();
                        let _ = p.save(&self.dir.join("Keyboard.ini"));
                    }
                    self.ksaved = self.kprof.clone();
                    self.kdev = Some((d, c));
                    if close {
                        self.requests.push(Request::Close);
                    } else {
                        self.flash("OK".into());
                    }
                }
                Err(e) => self.flash(e),
            }
            return false;
        }
        let Some(r) = done else { return true };
        self.busy = None;
        match r {
            Ok((d, c, m)) => {
                self.device = Some((d, c));
                self.macros = m;
                let _ = save_macros(&self.dir, &self.macros);
                if close {
                    self.requests.push(Request::Close);
                } else {
                    self.flash("OK".into());
                }
            }
            Err(e) => self.flash(e),
        }
        false
    }

    /// The "saving / reconnecting" screen shown while Apply runs.
    fn draw_busy(&mut self, p: &mut P) {
        let Some(b) = &self.busy else { return };
        let s = &p.r.strs;
        p.fill(Rect::new(0, 0, WIN_W, WIN_H), Rgba(0, 0, 0, 120));
        p.img("dlg_small", DLG.x, DLG.y);
        p.text_in(Rect::new(DLG.x, 195, DLG.w, 26), &s.s("General", 93), &T_DLG_TITLE);
        let msg = if crate::models::current().is_kbd() { s.s("KBD", 10 + b.phase.min(1) as u32) } else { s.s("General", 94 + b.phase.min(2) as u32) };
        p.text_in(Rect::new(DLG.x, 262, DLG.w, 18), &msg, &T_DLG);
        // indeterminate bar in the skin's red, inside a dark slot
        let track = Rect::new(DLG.x + 70, 305, DLG.w - 140, 14);
        p.fill(track, rgb(0x1a1a1a));
        p.cv.frame(track, 1, rgb(0x660000));
        let w = 90;
        let t = b.since.elapsed().as_millis() as i32;
        let span = track.w - 2 - w;
        let x = (t / 6) % (2 * span);
        let x = if x > span { 2 * span - x } else { x };
        p.fill(Rect::new(track.x + 1 + x, track.y + 1, w, track.h - 2), rgb(0xe60012));
        let secs = format!("{:.1} s", b.since.elapsed().as_secs_f32());
        p.text_in(Rect::new(DLG.x, 330, DLG.w, 16), &secs, &T_POLL);
    }

    pub fn device_path(&self) -> Option<String> {
        self.device.as_ref().map(|(d, _)| d.path.display().to_string()).or_else(|| self.kdev.as_ref().map(|(d, _)| d.path.display().to_string()))
    }

    pub fn save_all(&mut self) -> std::io::Result<()> {
        if let Some(p) = &self.kprof {
            p.save(&self.dir.join("Keyboard.ini"))?;
            self.ksaved = self.kprof.clone();
            return Ok(());
        }
        for (i, p) in self.profiles.iter().enumerate() {
            p.save(&self.dir.join(format!("RSProfile{}.pfd", i + 1)), i)?;
        }
        save_macros(&self.dir, &self.macros)?;
        self.saved = self.profiles.clone();
        Ok(())
    }

    fn p(&mut self) -> &mut Profile {
        &mut self.profiles[self.cur]
    }

    // ---------------------------------------------------------- interaction helpers

    /// Index of the layer that receives input (0 = main window).
    fn top(&self) -> usize {
        self.layers.len()
    }
    fn hover(&self, layer: usize, r: Rect) -> bool {
        layer == self.top() && r.contains(self.inp.mouse)
    }
    /// Press and release both inside `r` on the active layer.
    fn clicked(&self, layer: usize, r: Rect) -> bool {
        self.hover(layer, r) && self.inp.released && self.press_at.is_some_and(|p| r.contains(p))
    }
    fn pressed(&self, layer: usize, r: Rect) -> bool {
        self.hover(layer, r) && self.inp.pressed
    }
    fn wheel(&self, layer: usize, r: Rect) -> i32 {
        if self.hover(layer, r) { self.inp.wheel } else { 0 }
    }
    /// Held down: pressed inside `r` and the mouse still over it.
    fn held(&self, layer: usize, r: Rect) -> bool {
        self.hover(layer, r) && self.inp.down && self.press_at.is_some_and(|p| r.contains(p))
    }
    /// Feedback drawn over a button already painted at `r`: dimmed when it can not be used, a red
    /// outline while `active` (pulsing), darker while held down, lighter under the mouse.
    fn btn_fx(&self, p: &mut P, layer: usize, r: Rect, enabled: bool, active: bool) {
        if !enabled {
            p.fill(r, Rgba(0, 0, 0, 120));
            return;
        }
        if active {
            let a = if blink() { 255 } else { 150 };
            p.cv.frame(r.shrink(-3), 2, Rgba(255, 40, 40, a));
        }
        if self.held(layer, r) {
            p.fill(r, Rgba(0, 0, 0, 90));
            p.cv.frame(r, 1, Rgba(255, 90, 90, 255));
        } else if self.hover(layer, r) {
            p.fill(r, Rgba(255, 255, 255, 36));
        }
    }

    fn flash(&mut self, s: String) {
        self.status = Some((s, Instant::now()));
    }

    // ---------------------------------------------------------- frame

    pub fn frame(&mut self, r: &Res, cv: &mut Canvas, inp: Input) {
        // while Apply runs the UI is shown but takes no input
        let inp = if self.poll_apply() { Input { mouse: (-1, -1), ..Default::default() } } else { inp };
        if inp.pressed {
            self.press_at = Some(inp.mouse);
        }
        self.inp = inp;
        self.poll_hotplug();
        cv.clear();
        let mut p = P { r, cv };
        if self.no_device() {
            self.draw_no_device(&mut p);
        } else {
            self.draw_main(&mut p);
        }
        let n = self.layers.len();
        for i in 0..n {
            // temporarily take the layer out so drawing can borrow self mutably
            let mut layer = std::mem::replace(&mut self.layers[i], Layer::Dialog(Dialog::Notice(String::new())));
            match &mut layer {
                Layer::Menu(m) => self.draw_menu(&mut p, i + 1, m),
                Layer::Dialog(d) => self.draw_dialog(&mut p, i + 1, d),
                Layer::Macro(m) => self.draw_macro(&mut p, i + 1, m),
            }
            self.layers[i] = layer;
        }
        self.draw_status(&mut p);
        self.draw_busy(&mut p);
        // click outside an open popup menu closes it
        if self.inp.pressed {
            if let Some(Layer::Menu(_)) = self.layers.last() {
                let inside = self.menu_rects().iter().any(|r| r.contains(self.inp.mouse));
                if !inside {
                    self.cmds.push(Cmd::PopMenus);
                }
            }
        }
        if self.inp.released {
            self.press_at = None;
            self.drag = None;
        }
        self.apply_cmds();
    }

    fn menu_rects(&self) -> Vec<Rect> {
        let mut v = Vec::new();
        if let Some(Layer::Menu(m)) = self.layers.last() {
            v.push(Rect::new(m.x, m.y, MENU_W, MENU_ITEM_H * m.items.len() as i32));
            if let Some(o) = m.open {
                let (sx, sy) = sub_pos(m, o);
                v.push(Rect::new(sx, sy, MENU_W, MENU_ITEM_H * m.items[o].sub.len().max(1) as i32));
            }
        }
        v
    }

    fn apply_cmds(&mut self) {
        let cmds = std::mem::take(&mut self.cmds);
        for c in cmds {
            match c {
                Cmd::Push(l) => self.layers.push(l),
                Cmd::Pop => {
                    self.layers.pop();
                }
                Cmd::PopMenus => {
                    while let Some(Layer::Menu(_)) = self.layers.last() {
                        self.layers.pop();
                    }
                }
                Cmd::SetFunc(b, f) => {
                    self.set_func(b, f);
                }
                Cmd::MenuAct(owner, act) => self.menu_act(owner, act),
            }
        }
    }

    fn set_func(&mut self, b: usize, f: Func) {
        let p = &mut self.profiles[self.cur];
        let old = std::mem::replace(&mut p.buttons[b], f);
        // original rule: at least one button must keep the "click" function
        if !p.buttons.iter().any(|f| *f == Func::Click) {
            p.buttons[b] = old;
            let msg = self.strs_cache_warning();
            self.layers.push(Layer::Dialog(Dialog::Notice(msg)));
        }
    }

    fn strs_cache_warning(&self) -> String {
        Strings::load(self.lang).s("WARNING", 2)
    }

    fn menu_act(&mut self, owner: MenuOwner, act: MenuAct) {
        while let Some(Layer::Menu(_)) = self.layers.last() {
            self.layers.pop();
        }
        match (owner, act) {
            (MenuOwner::Button(b), MenuAct::Set(f)) => self.set_func(b, f),
            (MenuOwner::Button(b), MenuAct::AssignKey) => {
                let key = match &self.profiles[self.cur].buttons[b] { Func::Key(k) => k.clone(), _ => String::new() };
                self.layers.push(Layer::Dialog(Dialog::AssignKey { button: b, key }));
            }
            (MenuOwner::Button(b), MenuAct::Combo) => {
                let key = match &self.profiles[self.cur].buttons[b] { Func::Combo(k) => k.clone(), _ => String::new() };
                self.layers.push(Layer::Dialog(Dialog::Combo { button: b, key }));
            }
            (MenuOwner::Button(b), MenuAct::FireKey) => {
                let fk = match &self.profiles[self.cur].buttons[b] { Func::FireKey(f) => f.clone(), _ => FireKey::default() };
                let (times, delay) = (fk.times.to_string(), fk.delay_ms.to_string());
                self.layers.push(Layer::Dialog(Dialog::FireKey { button: b, fk, focus: 0, times, delay }));
            }
            (MenuOwner::Button(b), MenuAct::MacroManager) => self.open_macro_manager(Some(b)),
            (MenuOwner::Key(k), MenuAct::KSet(c)) => self.kset(k, c),
            (MenuOwner::Key(k), MenuAct::AssignKey) => {
                let kb = crate::models::current().kb.expect("keyboard");
                let key = match kb.func(self.kcode(k)) {
                    crate::kbd::KFunc::Key { usage, mods: 0 } => crate::device::usage_key(usage),
                    _ => String::new(),
                };
                self.layers.push(Layer::Dialog(Dialog::AssignKey { button: k, key }));
            }
            (MenuOwner::Key(k), MenuAct::Combo) => self.layers.push(Layer::Dialog(Dialog::Combo { button: k, key: String::new() })),
            (MenuOwner::MacroMore, a) => {
                if let Some(Layer::Macro(ed)) = self.layers.last_mut() {
                    ed_insert(ed, a);
                }
            }
            _ => {}
        }
    }

    pub fn open_macro_manager(&mut self, button: Option<usize>) {
        let macros = self.macros.clone();
        let sel = if macros.is_empty() { None } else { Some(0) };
        self.layers.push(Layer::Macro(MacroEd {
            button,
            macros,
            sel,
            row: None,
            scroll: 0,
            recording: false,
            rec_inserting: false,
            rec_insert: 0,
            last_event: None,
            held: Vec::new(),
            clipboard: Vec::new(),
            edit: None,
            dirty: false,
        }));
    }

    // ---------------------------------------------------------- main window

    fn draw_main(&mut self, p: &mut P) {
        let s = &p.r.strs;
        let kbd = crate::models::current().is_kbd();
        // keyboards: keys, lighting, info
        let tabs = if kbd {
            vec![s.s("KBD", 1), s.s("General", 51), s.s("General", 52)]
        } else {
            vec![s.s("General", 49), s.s("General", 50), s.s("General", 51), s.s("General", 52)]
        };
        let profile_names: Vec<String> = (0..5).map(|i| s.s("General", 53 + i)).collect();
        let bottom: Vec<String> = (42..=48).map(|k| s.s("General", k)).collect();

        let model = crate::models::current();
        if model.is_m801() {
            p.img("cfgMainback", 0, 0);
        } else {
            // other models: their name where the M801 skin says "RGB"
            p.img(if model.is_kbd() { "cfgMainback_kbd" } else { "cfgMainback_plain" }, 0, 0);
            let mut size = 34.0;
            while size > 14.0 && p.width(model.name, &TextStyle::new(size, Weight::Bold, RED).italic()) > 156 {
                size -= 1.0;
            }
            let st = TextStyle::new(size, Weight::Bold, RED).italic();
            let w = p.width(model.name, &st);
            p.text(733 - w, 96 - size as i32 - 4, model.name, &st);
        }
        // the product name switches between mice when several are plugged
        let name_r = Rect::new(575, 44, 160, 70);
        if self.top() == 0 && self.clicked(0, name_r) {
            let found = crate::device::scan();
            if found.len() > 1 {
                self.open_selector(found);
            }
        }
        // window drag on the skin's non-interactive areas
        if self.top() == 0 && self.inp.pressed {
            let m = self.inp.mouse;
            let content = Rect::ltrb(62, 142, 737, 551);
            let interactive = content.contains(m) || bottom_rect(0).offset(0, 0).y <= m.1 && m.1 < 590 && m.0 > 50 && m.0 < 745;
            let in_buttons = Rect::new(722, 34, 35, 14).contains(m);
            if !interactive && !in_buttons && m.1 < 620 {
                self.requests.push(Request::Drag);
            }
        }
        // minimise / close (images min-1..3 / close-1..3: normal, hover, pressed)
        for (i, name) in [(0, "min"), (1, "close")] {
            let r = Rect::new(722 + 20 * i, 34, 15, 14);
            let state = if self.hover(0, r) { if self.inp.down { 3 } else { 2 } } else { 1 };
            p.img(&format!("{name}-{state}"), r.x, r.y);
            if self.clicked(0, r) {
                self.requests.push(if i == 0 { Request::Minimize } else { Request::Close });
            }
        }
        for (i, t) in tabs.iter().enumerate() {
            let r = tab_rect(i);
            p.img(if self.tab == i { "tab_on" } else { "tab_off" }, r.x - 2, r.y - 2);
            p.text_in(r, t, &T_TAB);
            if self.pressed(0, r) {
                self.tab = i;
                self.dpi_edit = None;
            }
        }
        match (kbd, self.tab) {
            (true, 0) => self.draw_keys(p),
            (true, 1) => self.draw_kbd_light(p),
            (true, _) => self.draw_info(p),
            (false, 0) => self.draw_general(p),
            (false, 1) => self.draw_dpi(p),
            (false, 2) => self.draw_light(p),
            _ => self.draw_info(p),
        }
        if self.tab != 3 && !kbd {
            for i in 0..5 {
                let r = profile_rect(i);
                p.img(if self.cur == i { "profile_on" } else { "profile_off" }, r.x - 2, r.y - 2);
                p.text_in(r, &profile_names[i], &T_PROFILE);
                if self.pressed(0, r) {
                    self.cur = i;
                    self.dpi_edit = None;
                }
            }
        }
        for i in 0..7 {
            let r = bottom_rect(i);
            p.img_w("bottom_btn", r.x - 2, r.y - 2, r.w + 4);
            p.label_fit(r, &bottom[i], &T_BOTTOM, r.x + r.w - 5);
            if self.clicked(0, r) {
                self.bottom_action(i);
            }
        }
    }

    fn bottom_action(&mut self, i: usize) {
        if let (Some(kb), 0 | 1 | 5) = (crate::models::current().kb, i) {
            match i {
                0 => {
                    if let Some(path) = rfd::FileDialog::new().add_filter("ini", &["ini"]).set_file_name("Keyboard.ini").save_file() {
                        if let Some(pr) = &self.kprof {
                            match pr.save(&path) {
                                Ok(()) => self.flash(path.display().to_string()),
                                Err(e) => self.flash(e.to_string()),
                            }
                        }
                    }
                }
                1 => {
                    if let Some(path) = rfd::FileDialog::new().add_filter("ini", &["ini"]).pick_file() {
                        match crate::kbd::KbProfile::load(&path, kb) {
                            Some(pr) => self.kprof = Some(pr),
                            None => self.flash(format!("? {}", path.display())),
                        }
                    }
                }
                _ => {
                    self.kprof = self.ksaved.clone();
                    self.requests.push(Request::Close);
                }
            }
            return;
        }
        match i {
            0 => {
                // save as
                if let Some(path) = rfd::FileDialog::new().add_filter("pfd", &["pfd"]).set_file_name("Profile.pfd").save_file() {
                    match self.profiles[self.cur].save(&path, self.cur) {
                        Ok(()) => self.flash(path.display().to_string()),
                        Err(e) => self.flash(e.to_string()),
                    }
                }
            }
            1 => {
                if let Some(path) = rfd::FileDialog::new().add_filter("pfd", &["pfd"]).pick_file() {
                    match Profile::load(&path) {
                        Some(pr) => self.profiles[self.cur] = pr,
                        None => self.flash(format!("? {}", path.display())),
                    }
                }
            }
            2 => self.layers.push(Layer::Dialog(Dialog::Confirm(Confirm::RestoreOne))),
            3 => self.layers.push(Layer::Dialog(Dialog::Confirm(Confirm::RestoreAll))),
            4 => match self.save_all() {
                Err(e) => self.flash(e.to_string()),
                Ok(()) => {
                    if !self.start_apply(true) {
                        self.requests.push(Request::Close);
                    }
                }
            },
            5 => {
                self.profiles = self.saved.clone();
                self.requests.push(Request::Close);
            }
            _ => match self.save_all() {
                Err(e) => self.flash(e.to_string()),
                Ok(()) => {
                    if !self.start_apply(false) {
                        self.flash("OK".into());
                    }
                }
            },
        }
    }

    fn func_label(&self, s: &Strings, f: &Func) -> String {
        match f {
            Func::Click => s.g("FUNCTION_STRING", 1),
            Func::Menu => s.g("FUNCTION_STRING", 2),
            Func::Middle => s.g("FUNCTION_STRING", 3),
            Func::Forward => s.g("FUNCTION_STRING", 4),
            Func::Back => s.g("FUNCTION_STRING", 5),
            Func::Key(k) => if k.is_empty() { "key".into() } else { k.clone() },
            Func::Combo(k) => if k.is_empty() { s.g("FUNCTION_STRING", 7) } else { k.clone() },
            Func::Basic(i) => s.g("BASIC", *i as u32),
            Func::Advanced(i) => s.g("ADVANCED", *i as u32),
            Func::Media(i) => s.g("MEDIA", *i as u32),
            Func::Macro(n) => n.clone(),
            Func::FireKey(_) => s.g("FUNCTION_STRING", 12),
            Func::Dpi(i) => s.g("DPI", *i as u32),
            Func::ProfileSwitch => s.g("FUNCTION_STRING", 14),
            Func::Rate(i) => s.g("RATE", *i as u32),
            Func::LightSwitch => s.g("SCROLL", 3),
            Func::ScrollUp => s.g("SCROLL", 1),
            Func::ScrollDown => s.g("SCROLL", 2),
            Func::Disabled => s.g("FUNCTION_STRING", 16),
            Func::Raw(b) => format!("{:02X} {:02X} {:02X} {:02X}", b[0], b[1], b[2], b[3]),
        }
    }

    fn button_menu(&self, s: &Strings, b: usize) -> Vec<MenuItem> {
        let fs = |k| s.g("FUNCTION_STRING", k);
        let range = |sec: &str, n: u8, mk: fn(u8) -> Func| -> Vec<MenuItem> {
            (1..=n).map(|i| MenuItem::leaf(s.g(sec, i as u32), MenuAct::Set(mk(i)))).collect()
        };
        let mut mac = vec![MenuItem { label: s.g("MACRO", 1), act: Some(MenuAct::MacroManager), sub: Vec::new(), arrow: true }];
        for m in &self.macros {
            mac.push(MenuItem::leaf(m.name.clone(), MenuAct::Set(Func::Macro(m.name.clone()))));
        }
        let mut v = vec![
            MenuItem::leaf(fs(1), MenuAct::Set(Func::Click)),
            MenuItem::leaf(fs(2), MenuAct::Set(Func::Menu)),
            MenuItem::leaf(fs(3), MenuAct::Set(Func::Middle)),
            MenuItem::leaf(fs(4), MenuAct::Set(Func::Forward)),
            MenuItem::leaf(fs(5), MenuAct::Set(Func::Back)),
            MenuItem::leaf(fs(6), MenuAct::AssignKey),
            MenuItem::leaf(fs(7), MenuAct::Combo),
            MenuItem::sub(fs(8), range("BASIC", 8, Func::Basic)),
            MenuItem::sub(fs(9), range("ADVANCED", 14, Func::Advanced)),
            MenuItem::sub(fs(10), range("MEDIA", 8, Func::Media)),
            MenuItem::sub(fs(100), mac),
            MenuItem::leaf(fs(12), MenuAct::FireKey),
            MenuItem::sub(fs(13), range("DPI", 3, Func::Dpi)),
            MenuItem::leaf(fs(14), MenuAct::Set(Func::ProfileSwitch)),
            MenuItem::sub(fs(15), range("RATE", 2, Func::Rate)),
            MenuItem::leaf(s.g("SCROLL", 3), MenuAct::Set(Func::LightSwitch)),
        ];
        if crate::models::current().is_wheel(b) {
            v.push(MenuItem::leaf(s.g("SCROLL", 1), MenuAct::Set(Func::ScrollUp)));
            v.push(MenuItem::leaf(s.g("SCROLL", 2), MenuAct::Set(Func::ScrollDown)));
        }
        v.push(MenuItem::leaf(fs(16), MenuAct::Set(Func::Disabled)));
        v
    }

    fn draw_general(&mut self, p: &mut P) {
        let s = &p.r.strs;
        let model = crate::models::current();
        let num = |i: usize| -> String {
            if model.is_wheel(i) {
                (if i == model.buttons.len() { "num_up" } else { "num_down" }).to_string()
            } else {
                format!("num_{}", i + 1)
            }
        };
        match model.photo.filter(|_| !model.is_m801()) {
            Some(photo) => {
                p.img(photo, 102, 201);
                for i in 0..model.slot_count() {
                    if let Some((x, y)) = model.slot(i).mark {
                        p.img(&num(i), 102 + x as i32 - 11, 201 + y as i32 - 11);
                    }
                }
            }
            None => p.img("mouse", 102, 201),
        }
        let menu_open = match self.layers.last() {
            Some(Layer::Menu(m)) => match m.owner { MenuOwner::Button(b) => Some(b), _ => None },
            _ => None,
        };
        // more buttons than rows (side keypads): the list scrolls with the wheel
        let n = model.slot_count();
        let rows = n.min(ROWS);
        if n > ROWS {
            let list = Rect::new(310, 200, 170, 28 * ROWS as i32);
            let w = self.wheel(0, list);
            self.btn_top = (self.btn_top as i32 - w).clamp(0, (n - ROWS) as i32) as usize;
            let (h, y0) = (28 * ROWS as i32, 200);
            let bar_h = h * ROWS as i32 / n as i32;
            let bar_y = y0 + (h - bar_h) * self.btn_top as i32 / (n - ROWS) as i32;
            p.fill(Rect::new(482, y0, 3, h), rgb(0x2a2a2a));
            p.fill(Rect::new(482, bar_y, 3, bar_h), rgb(0xe60012));
        } else {
            self.btn_top = 0;
        }
        for row in 0..rows {
            let i = self.btn_top + row;
            let r = func_rect(row);
            p.img(&num(i), 312, r.y - 2);
            let hot = self.hover(0, r) || menu_open == Some(i);
            p.img(if hot { "btn_func_hover" } else { "btn_func" }, r.x - 2, r.y - 2);
            let label = self.func_label(s, &self.profiles[self.cur].buttons[i]);
            p.label_fit(r, &label, &T_FUNC, r.x + r.w - 3);
            if self.pressed(0, r) {
                let items = self.button_menu(s, i);
                let h = MENU_ITEM_H * items.len() as i32;
                // the original menu may hang below the window; ours must stay inside it
                let y = (r.y + r.h + 2).min(WIN_H - 4 - h);
                self.cmds.push(Cmd::Push(Layer::Menu(MenuState { owner: MenuOwner::Button(i), items, x: 334, y, open: None, armed: false })));
            }
        }
        self.btn_hover = None;
        let titles = [s.s("General", 1), s.s("General", 2), s.s("General", 3), s.s("General", 4)];
        let ranges = [ACCEL_RANGE, POINTER_RANGE, SCROLL_RANGE, DOUBLE_CLICK_RANGE];
        for k in 0..4 {
            let (ty, y) = SLIDERS[k];
            p.text(TRACK_X, ty, &titles[k], &T_TITLE);
            p.img("track", TRACK_X - 2, y);
            let (lo, hi) = ranges[k];
            let pr = self.profiles[self.cur].clone();
            let val = [pr.acceleration, pr.pointer_speed, pr.scroll_speed, pr.double_click][k];
            let hit = Rect::new(TRACK_X - 2, y - 2, TRACK_SPAN + 19, 22);
            let mut v = val as i32;
            if self.pressed(0, hit) {
                self.drag = Some((DRAG_SLIDER, k));
            }
            if self.drag == Some((DRAG_SLIDER, k)) && self.inp.down && self.top() == 0 {
                let t = (self.inp.mouse.0 - TRACK_X - 7) as f32 / TRACK_SPAN as f32;
                v = lo as i32 + (t * (hi - lo) as f32).round() as i32;
            }
            // finer control than the original: one step per wheel notch
            v += self.wheel(0, hit);
            let v = v.clamp(lo as i32, hi as i32) as u32;
            let pr = self.p();
            match k {
                0 => pr.acceleration = v,
                1 => pr.pointer_speed = v,
                2 => pr.scroll_speed = v,
                _ => pr.double_click = v,
            }
            let x = TRACK_X + ((v - lo) as f32 / (hi - lo) as f32 * TRACK_SPAN as f32).round() as i32;
            p.img("y", x, y + 2);
            if self.hover(0, hit) || self.drag == Some((DRAG_SLIDER, k)) {
                bubble(p, x + 7, y - 1, &format!("{v}"));
            }
        }
        p.text(TRACK_X, 418, &s.s("General", 5), &T_TITLE);
        for i in 0..4 {
            let x = POLL_X[i];
            let on = self.profiles[self.cur].polling == i;
            if on { p.img("poll_on", x - 2, 440) } else { p.img("poll_off", x - 1, 440) }
            p.text(x + 16, 442, &s.s("General", 6 + i as u32), &T_POLL);
            let w = 16 + p.width(&s.s("General", 6 + i as u32), &T_POLL);
            if self.pressed(0, Rect::new(x, 440, w, 17)) {
                self.p().polling = i;
            }
        }
    }

    fn draw_dpi(&mut self, p: &mut P) {
        let s = &p.r.strs;
        let pr = self.profiles[self.cur].clone();
        p.text(73, 184, &format!("{}：{}", s.s("General", 91), pr.dpi_stage + 1), &T_STAGE_TXT);
        for i in 0..5 {
            let lr = Rect::new(DPI_LABEL_X[i], 205, 61, 21);
            // the original's check mark is gone (user request); right-click
            // the title toggles the stage instead, disabled stages are dimmed
            p.img("dpi_label", lr.x - 2, lr.y - 2);
            let name = s.s("General", 15 + i as u32);
            if pr.dpi_stage == i {
                // current stage: "DPIn" + two spaces + the original's check mark, centred
                let tw = p.width(&name, &T_STAGE);
                let gap = p.width("  ", &T_STAGE);
                let ck = 17;
                let x0 = lr.x + (lr.w - (tw + gap + ck)) / 2;
                p.text_al(Rect::new(x0, lr.y, tw + 1, lr.h), &name, &T_STAGE, Align::Left);
                p.tint("check_mask", x0 + tw + gap, lr.y + 3, WHITE);
            } else {
                p.text_in(lr, &name, &T_STAGE);
            }
            if !pr.dpi_enabled[i] {
                p.fill(lr.shrink(1), Rgba(0, 0, 0, 120));
            }
            if self.hover(0, lr) && self.inp.right_pressed {
                let p2 = self.p();
                // at least one stage stays enabled
                if p2.dpi_enabled.iter().filter(|e| **e).count() > 1 || !p2.dpi_enabled[i] {
                    p2.dpi_enabled[i] = !p2.dpi_enabled[i];
                }
            } else if self.pressed(0, lr) {
                self.p().dpi_stage = i;
            }
            let cx = DPI_CX[i];
            p.img("dpi_track", cx - 18, 250);
            p.text_in(Rect::new(cx - 10, 236, 20, 14), "+", &T_DPIVAL);
            p.text_in(Rect::new(cx - 10, 445, 20, 14), "-", &T_DPIVAL);
            let hit = Rect::new(cx - 26, 252, 52, 192);
            let model = crate::models::current();
            let vals = model.dpi_values();
            let n = vals.len();
            let mut idx = model.dpi_pos(pr.dpi[i]) as i64;
            if self.pressed(0, hit) {
                self.drag = Some((DRAG_DPI, i));
                self.dpi_edit = None;
            }
            if self.drag == Some((DRAG_DPI, i)) && self.inp.down {
                idx = dpi_pos_at(self.inp.mouse.1, n) as i64;
            }
            // exact sensitivity: wheel steps one table entry (100/200 CPI), Shift = 10 entries
            let wl = self.wheel(0, hit) + self.wheel(0, Rect::new(cx - 30, 458, 60, 18));
            idx += wl as i64 * if self.inp.shift { 10 } else { 1 };
            if self.pressed(0, Rect::new(cx - 2, 236, 6, 14)) {
                idx += 1;
            }
            let idx = idx.clamp(0, n as i64 - 1) as usize;
            let new_dpi = vals[idx];
            if new_dpi != self.profiles[self.cur].dpi[i] {
                // editing a stage makes it the active one, so the change is felt after Apply
                self.p().dpi_stage = i;
            }
            self.p().dpi[i] = new_dpi;
            p.img("x2", cx - 26, dpi_thumb_y(idx, n));
            let vr = Rect::new(cx - 30, 458, 60, 18);
            if self.clicked(0, vr) && self.dpi_edit.is_none() {
                self.dpi_edit = Some((i, String::new()));
            }
            match &mut self.dpi_edit {
                Some((e, buf)) if *e == i => {
                    for c in self.inp.text.chars().filter(|c| c.is_ascii_digit()) {
                        if buf.len() < 5 {
                            buf.push(c);
                        }
                    }
                    let mut commit = false;
                    let mut cancel = false;
                    for k in &self.inp.keys {
                        if !k.pressed {
                            continue;
                        }
                        match k.name.as_str() {
                            "Backspace" => {
                                buf.pop();
                            }
                            "Enter" => commit = true,
                            "Escape" => cancel = true,
                            _ => {}
                        }
                    }
                    p.fill(Rect::new(cx - 27, 459, 54, 16), rgb(0x5a5a5a));
                    p.cv.frame(Rect::new(cx - 27, 459, 54, 16), 1, rgb(0xe60012));
                    let shown = format!("{buf}|");
                    p.text_in(Rect::new(cx - 27, 460, 54, 14), &shown, &T_DPIVAL);
                    if commit || (self.inp.pressed && !vr.contains(self.inp.mouse)) {
                        if let Ok(v) = buf.parse::<i64>() {
                            self.profiles[self.cur].dpi[i] = snap_dpi(v);
                            self.profiles[self.cur].dpi_stage = i;
                        }
                        self.dpi_edit = None;
                    } else if cancel {
                        self.dpi_edit = None;
                    }
                }
                _ => {
                    p.text_in(Rect::new(cx - 34, 461, 60, 14), &self.profiles[self.cur].dpi[i].to_string(), &T_DPIVAL);
                }
            }
            if self.hover(0, hit) || self.drag == Some((DRAG_DPI, i)) {
                bubble(p, cx, dpi_thumb_y(idx, n) - 3, &format!("{} CPI", vals[idx]));
            }
        }
    }

    fn draw_light(&mut self, p: &mut P) {
        let s = &p.r.strs;
        let Some(modes) = crate::models::current().light_modes() else {
            // zone lighting (RDCfg variant 4 / the 2850): not decoded, the device keeps its setting
            p.fill(Rect::ltrb(100, 236, 699, 479), rgb(0x272727));
            let mut y = 330;
            for line in p.wrap(&s.s("General", 99), &T_INFO, 520) {
                p.text_in(Rect::new(100, y, 599, 16), &line, &T_INFO);
                y += 20;
            }
            return;
        };
        if !modes.contains(&self.profiles[self.cur].light_mode) {
            self.p().light_mode = modes[0];
        }
        let mode = self.profiles[self.cur].light_mode;
        p.fill(Rect::ltrb(100, 236, 699, 479), rgb(0x272727));
        for i in 0..8 {
            let top = i < 4;
            let x = 100 + 150 * (i % 4) as i32;
            let (y, h, ty) = if top { (209, 28, 211) } else { (478, 30, 480) };
            let name = match (top, mode == i) {
                (true, true) => "ltab_top_on",
                (true, false) => "ltab_top_off",
                (false, true) => "ltab_bot_on",
                (false, false) => "ltab_bot_off",
            };
            p.img(name, x, y);
            let r = Rect::new(x, ty, 150, 24);
            p.text_in(r, &s.s("General", LIGHT_KEYS[i]), &T_LTAB);
            if !modes.contains(&i) {
                // this firmware has no such effect
                p.fill(Rect::new(x, y, 150, h), Rgba(0, 0, 0, 170));
                continue;
            }
            if self.pressed(0, Rect::new(x, y, 150, h)) {
                self.p().light_mode = i;
            }
        }
        let lay = LIGHT_LAYOUT[mode];
        if lay.off {
            p.text(109, 260, &s.s("General", 88), &T_LLABEL);
            return;
        }
        if lay.color {
            p.text(110, 259, &s.s("General", 23), &T_LLABEL);
            p.text(310, 259, &s.s("General", 24), &T_LLABEL);
            for (k, c) in SWATCHES.iter().enumerate() {
                let r = Rect::new(SWATCH_X0 + 26 * k as i32, SWATCH_Y, 24, 24);
                p.tint("swatch_mask", r.x, r.y, rgb(*c));
                // mark the preset in use with the original's check mark
                if self.profiles[self.cur].lights[mode].color == *c {
                    let luma = ((c >> 16) & 255) * 3 + ((c >> 8) & 255) * 6 + (c & 255);
                    let ink = if luma > 1500 { rgb(0x000000) } else { WHITE };
                    p.tint("check_mask", r.x + 4, r.y + 4, ink);
                }
                if self.pressed(0, r) {
                    self.p().lights[mode].color = *c;
                }
            }
            p.img("picker", PICKER.x, PICKER.y);
            if self.pressed(0, PICKER) {
                self.drag = Some((DRAG_PICKER, 0));
            }
            if self.drag == Some((DRAG_PICKER, 0)) && self.inp.down {
                let u = ((self.inp.mouse.0 - PICKER.x) as f32 / (PICKER.w - 1) as f32).clamp(0.0, 1.0);
                let v = ((self.inp.mouse.1 - PICKER.y) as f32 / (PICKER.h - 1) as f32).clamp(0.0, 1.0);
                let base = self.profiles[self.cur].lights[mode].color;
                let (r0, g0, b0) = hue_of(base);
                let mix = |c: f32| ((255.0 + (c - 255.0) * u) * (1.0 - v)).round() as u32;
                self.p().lights[mode].color = (mix(r0) << 16) | (mix(g0) << 8) | mix(b0);
            }
            // current colour, drawn as a small chip under the swatches
            let c = self.profiles[self.cur].lights[mode].color;
            if !SWATCHES.contains(&c) {
                p.fill(Rect::new(SWATCH_X0, SWATCH_Y + 30, 23, 8), rgb(c));
            }
        }
        let cols: Vec<(i32, i32, bool)> = {
            let mut v = vec![(lay.bright_x, 260, false)];
            if lay.speed {
                v.push((lay.speed_x, lay.speed_y, true));
            }
            v
        };
        for (x, y, is_speed) in cols {
            p.text(x + 1, y - 1, &s.s("General", if is_speed { 29 } else { 25 }), &T_LLABEL);
            for k in 0..3u8 {
                let ry = y + 30 + 24 * k as i32;
                let cur = if is_speed { self.profiles[self.cur].lights[mode].speed } else { self.profiles[self.cur].lights[mode].brightness };
                p.img(if cur == k { "lrow_on" } else { "lrow_off" }, x + 17, ry - 1);
                let base = if is_speed { 30 } else { 26 };
                p.text_in(Rect::new(x + 37, ry + 1, 70, 14), &s.s("General", base + k as u32), &T_LROW);
                if self.pressed(0, Rect::new(x + 17, ry - 1, 91, 18)) {
                    let l = &mut self.p().lights[mode];
                    if is_speed { l.speed = k } else { l.brightness = k }
                }
            }
        }
    }

    // ---------------------------------------------------------- keyboards

    fn kcode(&self, k: usize) -> u32 {
        match (crate::models::current().kb, &self.kprof) {
            (Some(kb), Some(pr)) => kb.keys.get(k).map(|key| pr.cfg.matrix[key.matrix as usize]).unwrap_or(0),
            _ => 0,
        }
    }

    fn kset(&mut self, k: usize, code: u32) {
        if let (Some(kb), Some(pr)) = (crate::models::current().kb, &mut self.kprof) {
            if let Some(key) = kb.keys.get(k) {
                pr.cfg.matrix[key.matrix as usize] = code;
            }
        }
    }

    fn key_menu(&self, s: &Strings, k: usize) -> Vec<MenuItem> {
        use crate::kbd::{KFunc, MEDIA};
        let fs = |n| s.g("FUNCTION_STRING", n);
        let mut v = Vec::new();
        let Some(kb) = crate::models::current().kb else { return Vec::new() };
        let def = kb.keys.get(k).map_or(u32::MAX, |key| key.default);
        if def != u32::MAX {
            v.push(MenuItem::leaf(format!("{} ({})", s.s("KBD", 2), kfunc_label(s, kb, def)), MenuAct::KSet(def)));
        }
        v.push(MenuItem::leaf(fs(6), MenuAct::AssignKey));
        if kb.combos() {
            v.push(MenuItem::leaf(fs(7), MenuAct::Combo));
        }
        if kb.mouse_keys() {
            // KFunc::Mouse order: left, right, middle, back, forward
            let mouse = [1, 2, 3, 5, 4].iter().enumerate().map(|(i, &n)| MenuItem::leaf(fs(n), MenuAct::KSet(kb.code(&KFunc::Mouse(i as u8))))).collect();
            v.push(MenuItem::sub(s.s("KBD", 3), mouse));
        }
        let media = MEDIA.iter().map(|&(u, _)| MenuItem::leaf(media_label(s, u), MenuAct::KSet(kb.code(&KFunc::Media(u))))).collect();
        v.push(MenuItem::sub(fs(10), media));
        v.push(MenuItem::leaf(fs(16), MenuAct::KSet(kb.code(&KFunc::Disabled))));
        v
    }

    /// Keys tab: the keyboard's photo with its keys; a click opens the function menu.
    fn draw_keys(&mut self, p: &mut P) {
        let s = &p.r.strs;
        let model = crate::models::current();
        let (Some(kb), Some(photo)) = (model.kb, model.photo) else { return };
        let (pw, ph) = (kb.photo_w as i32, kb.photo_h as i32);
        let (ox, oy) = (400 - pw / 2, 200 + (300 - ph) / 2);
        p.img(photo, ox, oy);
        let menu_key = match self.layers.last() {
            Some(Layer::Menu(m)) => match m.owner { MenuOwner::Key(k) => Some(k), _ => None },
            _ => None,
        };
        let mut hot = None;
        for (i, k) in kb.keys.iter().enumerate() {
            let r = Rect::new(ox + k.x as i32, oy + k.y as i32, k.w as i32, k.h as i32);
            let code = self.kcode(i);
            if k.default != u32::MAX && code != k.default {
                p.fill(r, Rgba(230, 0, 18, 110));
            }
            if self.hover(0, r) || menu_key == Some(i) {
                p.cv.frame(r, 1, WHITE);
                hot = Some(i);
            }
            if self.pressed(0, r) {
                let items = self.key_menu(s, i);
                let h = MENU_ITEM_H * items.len() as i32;
                let x = (r.x + r.w / 2).min(WIN_W - MENU_W - 4);
                let y = (r.y + r.h + 2).min(WIN_H - 4 - h);
                self.cmds.push(Cmd::Push(Layer::Menu(MenuState { owner: MenuOwner::Key(i), items, x, y, open: None, armed: false })));
            }
        }
        let line = match hot {
            Some(i) => {
                let k = &kb.keys[i];
                let now = kfunc_label(s, kb, self.kcode(i));
                if k.default == u32::MAX { now } else { format!("{}  →  {now}", kfunc_label(s, kb, k.default)) }
            }
            // Jieli keyboards: lighting only, no key to click
            None if kb.keys.is_empty() => s.s("KBD", 18),
            None => s.s("KBD", 9),
        };
        p.text_in(Rect::new(62, 512, 675, 16), &line, &T_INFO);
    }

    /// Lighting tab: the modes of this keyboard, then the current mode's palette,
    /// brightness and speed.
    fn draw_kbd_light(&mut self, p: &mut P) {
        use crate::kbd::COLORS;
        let s = &p.r.strs;
        let Some(kb) = crate::models::current().kb else { return };
        let Some(pr) = self.kprof.clone() else { return };
        if kb.leds.is_empty() {
            // BYK901 builds: lighting not decoded, the keyboard keeps its own
            p.fill(Rect::ltrb(100, 236, 699, 479), rgb(0x272727));
            let mut y = 330;
            for line in p.wrap_words(&s.s("KBD", 17), &T_INFO, 520) {
                p.text_in(Rect::new(100, y, 599, 16), &line, &T_INFO);
                y += 20;
            }
            return;
        }
        let mut mode = pr.cfg.mode(kb);
        // many modes (VIA keyboards: up to 46): a denser grid
        let (cols, cw, rh) = if kb.leds.len() > 24 { (6, 99, 20) } else { (4, 148, 26) };
        for (i, l) in kb.leds.iter().enumerate() {
            let r = Rect::new(105 + cw * (i % cols) as i32, 195 + rh * (i / cols) as i32, cw - 4, rh - 3);
            let on = mode == i;
            p.fill(r, if on { RED } else if self.hover(0, r) { rgb(0x3a3a3a) } else { rgb(0x262626) });
            p.cv.frame(r, 1, if on { rgb(0xff4d5a) } else { rgb(0x444444) });
            let name = p.ellipsize(&kled_name(s, kb, l), &T_LROW, r.w - 6);
            p.text_in(r, &name, &T_LROW);
            if self.pressed(0, r) {
                mode = i;
            }
        }
        let y0 = 195 + rh * kb.leds.len().div_ceil(cols) as i32 + 8;
        let panel = Rect::ltrb(100, y0, 699, (y0 + 150).min(508));
        p.fill(panel, rgb(0x272727));
        let l = &kb.leds[mode];
        let light = pr.cfg.light(kb, mode);
        let mut nl = light;
        let mut pal = pr.palettes.get(mode).copied().unwrap_or(kb.default_palette());
        if l.hw == 0 {
            p.text(110, y0 + 10, &s.s("General", 88), &T_LLABEL);
        } else if Some(l.hw) == kb.custom_hw() {
            p.text(110, y0 + 10, &s.s("KBD", 15), &T_LLABEL);
        }
        if l.color {
            p.text(110, y0 + 8, &s.s("General", 23), &T_LLABEL);
            let n = if l.random { COLORS + 1 } else { COLORS };
            for j in 0..n {
                let r = Rect::new(110 + 26 * j as i32, y0 + 30, 24, 24);
                if j < COLORS {
                    p.tint("swatch_mask", r.x, r.y, rgb(pal[j]));
                } else {
                    // colourful: the driver's random colour value
                    for (k, c) in [0xff0000, 0xffa000, 0xfff000, 0x00e000, 0x0080ff, 0x8000ff].iter().enumerate() {
                        p.fill(Rect::new(r.x + 4 * k as i32, r.y, 4, r.h), rgb(*c));
                    }
                }
                if nl.color as usize == j {
                    let c = if j < COLORS { pal[j] } else { 0 };
                    let luma = ((c >> 16) & 255) * 3 + ((c >> 8) & 255) * 6 + (c & 255);
                    p.tint("check_mask", r.x + 4, r.y + 4, if luma > 1500 { rgb(0x000000) } else { WHITE });
                }
                if self.pressed(0, r) {
                    nl.color = j as u8;
                }
            }
            if kb.colour_picker(l) {
                let mut y = y0 + 64;
                for line in p.wrap_words(&s.s("KBD", 16), &T_LROW, 205) {
                    p.text(110, y, &line, &T_LROW);
                    y += 16;
                }
            }
            if (nl.color as usize) < COLORS && kb.colour_picker(l) {
                let slot = nl.color as usize;
                let pk = Rect::new(330, y0 + 14, PICKER.w, PICKER.h);
                p.img("picker", pk.x, pk.y);
                if self.pressed(0, pk) {
                    self.drag = Some((DRAG_PICKER, 1));
                }
                if self.drag == Some((DRAG_PICKER, 1)) && self.inp.down {
                    let u = ((self.inp.mouse.0 - pk.x) as f32 / (pk.w - 1) as f32).clamp(0.0, 1.0);
                    let v = ((self.inp.mouse.1 - pk.y) as f32 / (pk.h - 1) as f32).clamp(0.0, 1.0);
                    let (r0, g0, b0) = hue_of(pal[slot]);
                    let mix = |c: f32| ((255.0 + (c - 255.0) * u) * (1.0 - v)).round() as u32;
                    pal[slot] = (mix(r0) << 16) | (mix(g0) << 8) | mix(b0);
                }
            }
        }
        let cols = [(l.light, 485, 25, kb.light.len()), (l.speed, 595, 29, kb.speed.len())];
        for (k, &(shown, x, title, levels)) in cols.iter().enumerate() {
            if !shown {
                continue;
            }
            p.text(x, y0 + 8, &s.s("General", title), &T_LLABEL);
            for lv in 0..levels {
                let ry = y0 + 30 + 20 * lv as i32;
                let cur = if k == 0 { nl.bright } else { nl.speed };
                p.img(if cur as usize == lv { "lrow_on" } else { "lrow_off" }, x, ry - 1);
                p.text_in(Rect::new(x + 20, ry + 1, 70, 14), &(lv + 1).to_string(), &T_LROW);
                if self.pressed(0, Rect::new(x, ry - 1, 91, 18)) {
                    if k == 0 { nl.bright = lv as u8 } else { nl.speed = lv as u8 }
                }
            }
        }
        if let Some(pr) = &mut self.kprof {
            if mode != pr.cfg.mode(kb) {
                pr.cfg.set_mode(kb, mode, kb.custom_index());
            }
            if nl != light {
                pr.cfg.set_light(kb, mode, nl);
            }
            if let Some(slot) = pr.palettes.get_mut(mode) {
                *slot = pal;
            }
        }
    }

    fn draw_info(&mut self, p: &mut P) {
        let s = &p.r.strs;
        let c = Rect::new(72, 0, 676, 0);
        for (k, y) in [(37, 227), (38, 250), (39, 274), (40, 358), (41, 384)] {
            let m = crate::models::current();
            let t = if m.is_m801() { s.s("General", k) } else { s.s("General", k).replace("M801-RGB", m.name) };
            p.text_in(Rect::new(c.x, y, c.w, 16), &t, &T_INFO);
        }
        // tribute to the author of the Linux port (addition)
        p.img("vortron", c.x + c.w / 2 - 32, 412);
        p.text_in(Rect::new(c.x, 482, c.w, 16), &s.s("General", 92), &T_INFO);
        // language row (addition: the original only followed the system locale)
        for (i, l) in Lang::ALL.iter().enumerate() {
            let r = profile_rect(i);
            p.img(if self.lang == *l { "profile_on" } else { "profile_off" }, r.x - 2, r.y - 2);
            p.text_in(r, l.native(), &T_PROFILE);
            if self.pressed(0, r) && self.lang != *l {
                self.lang = *l;
                self.lang_changed = true;
            }
        }
    }

    /// Fallback screen: no supported device is plugged. Plugging one in opens it
    /// (poll_hotplug); the profiles can still be edited offline.
    fn draw_no_device(&mut self, p: &mut P) {
        let s = &p.r.strs;
        let (title, l1, l2, wait, offline) = (s.s("NODEV", 1), s.s("NODEV", 2), s.s("NODEV", 3), s.s("NODEV", 4), s.s("NODEV", 5));
        p.img("cfgMainback_plain", 0, 0);
        for (i, name) in [(0, "min"), (1, "close")] {
            let r = Rect::new(722 + 20 * i, 34, 15, 14);
            let state = if self.hover(0, r) { if self.inp.down { 3 } else { 2 } } else { 1 };
            p.img(&format!("{name}-{state}"), r.x, r.y);
            if self.clicked(0, r) {
                self.requests.push(if i == 0 { Request::Minimize } else { Request::Close });
            }
        }
        let c = Rect::ltrb(62, 142, 737, 551);
        p.text_in(Rect::new(c.x, 250, c.w, 30), &title, &TextStyle::new(24.0, Weight::Bold, WHITE).cjk(24.0).italic());
        p.fill(Rect::new(c.x + c.w / 2 - 60, 290, 120, 2), RED);
        p.text_in(Rect::new(c.x, 312, c.w, 16), &l1, &T_INFO);
        p.text_in(Rect::new(c.x, 336, c.w, 16), &l2, &T_INFO);
        let st = TextStyle::new(12.0, Weight::Regular, rgb(0x9a9a9a));
        let ww = p.width(&wait, &st);
        let x = c.x + (c.w - ww) / 2;
        p.text(x, 380, &wait, &st);
        p.fill(Rect::new(x - 14, 386, 6, 6), if blink() { RED } else { rgb(0x5a0008) });
        let w = (p.width(&offline, &T_BOTTOM) + 40).max(150);
        let b = Rect::new(c.x + (c.w - w) / 2, 440, w, 24);
        p.img_w("bottom_btn", b.x - 2, b.y - 2, b.w + 4);
        p.text_in(b, &offline, &T_BOTTOM);
        self.btn_fx(p, 0, b, true, false);
        if self.clicked(0, b) {
            if let Some(h) = &mut self.hotplug {
                h.offline = true;
            }
        }
        if self.top() == 0 && self.inp.pressed && !b.contains(self.inp.mouse) && !Rect::new(722, 34, 35, 14).contains(self.inp.mouse) {
            self.requests.push(Request::Drag);
        }
    }

    // ---------------------------------------------------------- popup menu

    fn draw_menu(&mut self, p: &mut P, layer: usize, m: &mut MenuState) {
        let active = layer == self.top();
        let mouse = self.inp.mouse;
        draw_menu_panel(p, m.x, m.y, &m.items, None);
        // hover selection
        let mut hovered = None;
        for (i, _) in m.items.iter().enumerate() {
            let r = Rect::new(m.x, m.y + MENU_ITEM_H * i as i32, MENU_W, MENU_ITEM_H);
            if active && r.contains(mouse) {
                hovered = Some(i);
            }
        }
        if let Some(h) = hovered {
            m.open = if m.items[h].sub.is_empty() { None } else { Some(h) };
        }
        let mut sub_hover = None;
        if let Some(o) = m.open {
            let (sx, sy) = sub_pos(m, o);
            for (j, _) in m.items[o].sub.iter().enumerate() {
                let r = Rect::new(sx, sy + MENU_ITEM_H * j as i32, MENU_W, MENU_ITEM_H);
                if active && r.contains(mouse) {
                    sub_hover = Some(j);
                }
            }
        }
        draw_menu_panel(p, m.x, m.y, &m.items, hovered.or(m.open));
        if let Some(o) = m.open {
            let (sx, sy) = sub_pos(m, o);
            draw_menu_panel(p, sx, sy, &m.items[o].sub, sub_hover);
        }
        if active && self.inp.released {
            let pick = match (hovered, sub_hover, m.open) {
                (Some(h), _, _) => m.items[h].act.clone(),
                (None, Some(j), Some(o)) => m.items[o].sub[j].act.clone(),
                _ => None,
            };
            // the release of the click that opened the menu only arms it
            if !m.armed {
                m.armed = true;
            } else if let Some(a) = pick {
                self.cmds.push(Cmd::MenuAct(m.owner, a));
            }
        }
        for k in &self.inp.keys {
            if k.pressed && k.name == "Escape" && active {
                self.cmds.push(Cmd::Pop);
            }
        }
    }

    // ---------------------------------------------------------- dialogs

    fn draw_dialog(&mut self, p: &mut P, layer: usize, d: &mut Dialog) {
        let active = layer == self.top();
        let s = &p.r.strs;
        p.img("dlg_small", DLG.x, DLG.y);
        let title_r = Rect::new(DLG.x, 195, DLG.w, 26);
        let ok_r = Rect::new(295, 387, 89, 24);
        let cancel_r = Rect::new(394, 387, 89, 24);
        let (title, show_cancel) = match d {
            Dialog::AssignKey { .. } => (s.s("SINGLEKEY", 1), true),
            Dialog::Combo { .. } => (s.s("SINGLEKEY", 3), true),
            Dialog::FireKey { .. } => (s.s("SINGLEKEY", 7), true),
            Dialog::NewMacro { .. } => (s.s("SINGLEKEY", 5), true),
            Dialog::Confirm(c) => (
                match c {
                    Confirm::RestoreOne => s.s("WARNING", 3),
                    Confirm::RestoreAll => s.s("WARNING", 6),
                    Confirm::DeleteMacro => s.s("WARNING", 9),
                    Confirm::LeaveMacro => s.s("WARNING", 1),
                },
                true,
            ),
            Dialog::Notice(_) => (s.s("WARNING", 1), false),
            Dialog::Permission => (s.s("WARNING", 1), true),
            Dialog::SelectDevice { .. } => (s.s("General", 97), true),
        };
        p.text_in(title_r, &title, &T_DLG_TITLE);
        let keys: Vec<KeyEv> = if active { self.inp.keys.clone() } else { Vec::new() };
        let mut ok = false;
        let mut cancel = false;
        for k in &keys {
            if k.pressed && k.name == "Escape" {
                cancel = true;
            }
        }
        let combo = matches!(d, Dialog::Combo { .. });
        match d {
            Dialog::AssignKey { key, .. } | Dialog::Combo { key, .. } => {
                let msg = if combo { s.s("SINGLEKEY", 4) } else { s.s("SINGLEKEY", 1) };
                for (i, line) in p.wrap(&msg, &T_DLG, 380).iter().enumerate() {
                    p.text(204, 251 + 17 * i as i32, line, &T_DLG);
                }
                p.img("dlg_edit_red", 238, 310);
                // any key press is captured (Escape too: it is a valid assignment)
                cancel = false;
                for k in &keys {
                    if !k.pressed || is_modifier(&k.name) {
                        continue;
                    }
                    *key = if combo { combo_name(&self.inp, &k.name) } else { k.name.clone() };
                }
                p.text_in(Rect::new(242, 314, 296, 22), key, &T_DLG);
            }
            Dialog::FireKey { fk, focus, times, delay, .. } => {
                let names = [s.s("FireKey", 1), s.s("FireKey", 2), s.s("FireKey", 3)];
                let boxes = [(193, 276), (246, 276), (316, 276), (193, 300)];
                for (i, (bx, by)) in boxes.iter().enumerate() {
                    let on = fk.target as usize == i;
                    p.img(if on { "dlg_check_on" } else { "dlg_check_off" }, *bx, *by);
                    if i < 3 {
                        p.text(bx + 30, 279, &names[i], &T_FK);
                    } else {
                        p.text(243, 304, &s.s("FireKey", 4), &T_FK);
                    }
                    if active && self.pressed(layer, Rect::new(*bx, *by, 19, 20)) {
                        fk.target = i as u8;
                    }
                }
                let kr = Rect::new(217, 329, 102, 30);
                p.img("dlg_edit_red_small", kr.x, kr.y);
                if self.pressed(layer, kr) {
                    *focus = 1;
                    fk.target = 3;
                }
                p.text(438, 296, &s.s("FireKey", 7), &T_FK);
                p.text(438, 338, &s.s("FireKey", 8), &T_FK);
                let tr = Rect::new(539, 294, 65, 23);
                let dr = Rect::new(539, 335, 65, 23);
                p.img("dlg_edit_gray", tr.x, tr.y);
                p.img("dlg_edit_gray", dr.x, dr.y);
                if self.pressed(layer, tr) {
                    *focus = 2;
                }
                if self.pressed(layer, dr) {
                    *focus = 3;
                }
                match focus {
                    1 => {
                        for k in &keys {
                            if k.pressed && !is_modifier(&k.name) {
                                fk.key = k.name.clone();
                            }
                        }
                    }
                    2 | 3 => {
                        let buf = if *focus == 2 { &mut *times } else { &mut *delay };
                        edit_digits(buf, &self.inp, &keys, 5);
                        // granular: wheel adjusts the focused number by 1 (Shift: 10)
                    }
                    _ => {}
                }
                for (r, buf, lo, hi) in [(tr, &mut *times, 1i64, 255i64), (dr, &mut *delay, 1, 65535)] {
                    let w = self.wheel(layer, r);
                    if w != 0 {
                        let v = buf.parse::<i64>().unwrap_or(lo) + w as i64 * if self.inp.shift { 10 } else { 1 };
                        *buf = v.clamp(lo, hi).to_string();
                    }
                }
                let caret = |f: u8, want: u8| if f == want && blink() { "|" } else { "" };
                p.text(kr.x + 6, kr.y + 7, &format!("{}{}", fk.key, caret(*focus, 1)), &T_DLG);
                p.text(tr.x + 2, tr.y + 3, &format!("{}{}", times, caret(*focus, 2)), &T_FK);
                p.text(dr.x + 2, dr.y + 3, &format!("{}{}", delay, caret(*focus, 3)), &T_FK);
                fk.times = times.parse::<u32>().unwrap_or(1).clamp(1, 255);
                fk.delay_ms = delay.parse::<u32>().unwrap_or(1).clamp(1, 65535);
            }
            Dialog::NewMacro { name } => {
                p.text(204, 251, &s.s("SINGLEKEY", 6), &T_DLG);
                p.img("dlg_edit_gray_wide", 238, 310);
                if active {
                    for c in self.inp.text.chars() {
                        if name.chars().count() < 24 && !"/\\:*?\"<>|".contains(c) {
                            name.push(c);
                        }
                    }
                    for k in &keys {
                        if k.pressed && k.name == "Backspace" {
                            name.pop();
                        }
                        if k.pressed && k.name == "Enter" {
                            ok = true;
                        }
                    }
                }
                p.text(246, 318, &format!("{name}{}", if blink() { "|" } else { "" }), &T_DLG);
            }
            Dialog::Confirm(c) => {
                let msg = match c {
                    Confirm::RestoreOne => format!("{}\n{}", s.s("WARNING", 4), s.s("WARNING", 5)),
                    Confirm::RestoreAll => format!("{}\n{}", s.s("WARNING", 7), s.s("WARNING", 8)),
                    Confirm::DeleteMacro => s.s("WARNING", 10),
                    Confirm::LeaveMacro => s.s("WARNING", 11),
                };
                let mut y = 260;
                for para in msg.split('\n') {
                    for line in p.wrap(para, &T_DLG, 380) {
                        p.text_in(Rect::new(DLG.x, y, DLG.w, 17), &line, &T_DLG);
                        y += 20;
                    }
                }
                for k in &keys {
                    if k.pressed && k.name == "Enter" {
                        ok = true;
                    }
                }
            }
            Dialog::Permission => {
                let mut y = 250;
                for line in p.wrap(&s.s("WARNING", 15), &T_DLG, 400) {
                    p.text_in(Rect::new(DLG.x, y, DLG.w, 17), &line, &T_DLG);
                    y += 20;
                }
            }
            Dialog::SelectDevice { list, sel, top } => {
                // one row per plugged mouse: photo, name, USB port; double-click = OK
                const VIS: usize = 3;
                let area = Rect::new(DLG.x + 40, 232, DLG.w - 80, 48 * VIS as i32);
                let w = if active { self.wheel(layer, area) } else { 0 };
                *top = (*top as i32 - w).clamp(0, list.len().saturating_sub(VIS) as i32) as usize;
                for row in 0..VIS.min(list.len()) {
                    let i = *top + row;
                    let f = &list[i];
                    let r = Rect::new(area.x, area.y + 48 * row as i32, area.w, 46);
                    let on = *sel == i;
                    p.fill(r, if on { rgb(0x3a0006) } else { rgb(0x1c1c1c) });
                    p.cv.frame(r, 1, if on { RED } else { rgb(0x444444) });
                    let thumb = f.model.photo.filter(|_| !f.model.is_m801()).map(|ph| format!("thumb/{ph}")).unwrap_or("thumb/mouse".into());
                    let th = p.r.img(&thumb).h;
                    p.img(&thumb, r.x + 6, if th < 43 { r.y + (46 - th) / 2 } else { r.y + 2 });
                    p.text(r.x + 46, r.y + 7, f.model.name, &T_DLG);
                    p.text(r.x + 46, r.y + 26, &format!("USB {}  ·  {}", f.usb_name(), f.hidraw.display()), &T_POLL);
                    if active && self.pressed(layer, r) {
                        if *sel == i && self.inp.double {
                            ok = true;
                        }
                        *sel = i;
                    }
                }
                for k in &keys {
                    if k.pressed && k.name == "Enter" {
                        ok = true;
                    }
                    if k.pressed && k.name == "ArrowDown" {
                        *sel = (*sel + 1).min(list.len() - 1);
                    }
                    if k.pressed && k.name == "ArrowUp" {
                        *sel = sel.saturating_sub(1);
                    }
                }
                if *sel < *top {
                    *top = *sel;
                } else if *sel >= *top + VIS {
                    *top = *sel + 1 - VIS;
                }
            }
            Dialog::Notice(msg) => {
                let mut y = 270;
                for line in p.wrap(msg, &T_DLG, 380) {
                    p.text_in(Rect::new(DLG.x, y, DLG.w, 17), &line, &T_DLG);
                    y += 20;
                }
                for k in &keys {
                    if k.pressed && k.name == "Enter" {
                        ok = true;
                    }
                }
            }
        }
        let ok_r = if show_cancel { ok_r } else { Rect::new(345, 387, 89, 24) };
        p.img("dlg_btn", ok_r.x - 2, ok_r.y - 2);
        p.text_in(ok_r, &s.s("FireKey", 9), &T_DLG);
        if show_cancel {
            p.img("dlg_btn", cancel_r.x - 2, cancel_r.y - 2);
            p.text_in(cancel_r, &s.s("FireKey", 10), &T_DLG);
        }
        ok |= self.clicked(layer, ok_r);
        cancel |= show_cancel && self.clicked(layer, cancel_r);
        if !active {
            return;
        }
        if ok {
            self.dialog_ok(d);
        } else if cancel {
            self.cmds.push(Cmd::Pop);
        }
    }

    fn dialog_ok(&mut self, d: &Dialog) {
        match d {
            Dialog::AssignKey { button, key } | Dialog::Combo { button, key } if crate::models::current().is_kbd() => {
                self.cmds.push(Cmd::Pop);
                if key.is_empty() {
                    return;
                }
                // "CTRL+SHIFT+A": modifier mask in byte 2, as KeyboardDrv codes its combos
                let mut parts: Vec<&str> = key.split('+').collect();
                let last = parts.pop().unwrap_or_default();
                let mods = parts.iter().filter_map(|m| crate::device::combo_mask(m)).fold(0, |a, b| a | b);
                match crate::device::key_usage(last) {
                    Some(usage) => {
                        let kb = crate::models::current().kb.expect("keyboard");
                        self.kset(*button, kb.code(&crate::kbd::KFunc::Key { usage, mods }));
                    }
                    None => self.flash(format!("? {key}")),
                }
            }
            Dialog::AssignKey { button, key } => {
                self.cmds.push(Cmd::Pop);
                if !key.is_empty() {
                    self.cmds.push(Cmd::SetFunc(*button, Func::Key(key.clone())));
                }
            }
            Dialog::Combo { button, key } => {
                self.cmds.push(Cmd::Pop);
                if !key.is_empty() {
                    self.cmds.push(Cmd::SetFunc(*button, Func::Combo(key.clone())));
                }
            }
            Dialog::FireKey { button, fk, .. } => {
                self.cmds.push(Cmd::Pop);
                self.cmds.push(Cmd::SetFunc(*button, Func::FireKey(fk.clone())));
            }
            Dialog::NewMacro { name } => {
                let name = name.trim().to_string();
                if name.is_empty() {
                    return;
                }
                self.cmds.push(Cmd::Pop);
                // the macro manager is the layer below this dialog (layers[n-1] is the dialog)
                let n = self.layers.len();
                if let Some(Layer::Macro(ed)) = self.layers.get_mut(n.wrapping_sub(2)) {
                    if ed.macros.len() >= MAX_MACROS {
                        self.cmds.push(Cmd::Push(Layer::Dialog(Dialog::Notice(Strings::load(self.lang).s("WARNING", 12)))));
                        return;
                    }
                    let mut unique = name.clone();
                    let mut k = 2;
                    while ed.macros.iter().any(|m| m.name == unique) {
                        unique = format!("{name} ({k})");
                        k += 1;
                    }
                    ed.macros.push(Macro::new(&unique));
                    ed.sel = Some(ed.macros.len() - 1);
                    ed.row = None;
                    ed.scroll = 0;
                    ed.dirty = true;
                }
            }
            Dialog::Confirm(c) => {
                self.cmds.push(Cmd::Pop);
                match c {
                    Confirm::RestoreOne | Confirm::RestoreAll if crate::models::current().is_kbd() => {
                        let kb = crate::models::current().kb.expect("keyboard");
                        let blank = crate::kbd::KbProfile::blank(kb);
                        if let Some(pr) = &mut self.kprof {
                            // one: the keys back to the driver's defaults; all: lighting too
                            if *c == Confirm::RestoreOne {
                                pr.cfg.matrix = blank.cfg.matrix;
                            } else {
                                *pr = blank;
                            }
                        }
                    }
                    Confirm::RestoreOne => self.profiles[self.cur] = Profile::default(),
                    Confirm::RestoreAll => self.profiles = vec![Profile::default(); 5],
                    Confirm::DeleteMacro => {
                        let n = self.layers.len();
                        if let Some(Layer::Macro(ed)) = self.layers.get_mut(n.wrapping_sub(2)) {
                            if let Some(i) = ed.sel {
                                ed.macros.remove(i);
                                ed.sel = if ed.macros.is_empty() { None } else { Some(i.min(ed.macros.len() - 1)) };
                                ed.row = None;
                                ed.dirty = true;
                            }
                        }
                    }
                    Confirm::LeaveMacro => self.cmds.push(Cmd::Pop),
                }
            }
            Dialog::Notice(_) => self.cmds.push(Cmd::Pop),
            Dialog::SelectDevice { list, sel, .. } => {
                self.cmds.push(Cmd::Pop);
                if let Some(f) = list.get(*sel) {
                    self.target = Some(f.usb_name());
                    self.set_model(f.model);
                    self.attach_device();
                }
            }
            Dialog::Permission => {
                self.cmds.push(Cmd::Pop);
                match crate::device::install_udev_rule() {
                    Ok(()) => self.attach_device(),
                    Err(e) => {
                        let msg = format!("{}  ({e})", Strings::load(self.lang).s("WARNING", 16));
                        self.cmds.push(Cmd::Push(Layer::Dialog(Dialog::Notice(msg))));
                    }
                }
            }
        }
    }

    // ---------------------------------------------------------- macro manager

    fn draw_macro(&mut self, p: &mut P, layer: usize, ed: &mut MacroEd) {
        let active = layer == self.top();
        let s = &p.r.strs;
        let o = (MAC.x, MAC.y);
        let _ = o;
        p.img("dlg_big", MAC.x, MAC.y);
        p.text_in(Rect::new(MAC.x - 20, 45, MAC.w, 28), &s.s("MACRO", 1), &T_MAC_TITLE);
        let blue = rgb(0x013c64);
        p.cv.frame(Rect::ltrb(45, 98, 255, 528), 2, blue);
        p.cv.frame(Rect::ltrb(271, 100, 591, 530), 2, blue);
        p.cv.frame(Rect::ltrb(590, 100, 746, 530), 2, blue);
        p.text(52, 111, &s.s("MACRO", 2), &T_MAC);

        // ---- macro list
        for i in 0..MAX_MACROS {
            let r = Rect::new(59, MAC_LIST_Y + 23 * i as i32, 171, 21);
            let sel = ed.sel == Some(i) && i < ed.macros.len();
            p.img("mac_item", r.x - 2, r.y - 2);
            if sel {
                p.fill(r, rgb(0xc80000));
            }
            if let Some(m) = ed.macros.get(i) {
                let name = p.ellipsize(&m.name, &T_MAC, r.w - 12);
                p.text_al(r.offset(6, 0), &name, &T_MAC, Align::Left);
                if self.pressed(layer, r) && !ed.recording {
                    ed.sel = Some(i);
                    ed.row = None;
                    ed.scroll = 0;
                    ed.edit = None;
                }
            }
        }
        let new_r = Rect::new(50, 500, 97, 16);
        let del_r = Rect::new(153, 500, 97, 16);
        p.img("mac_btn_small", 48, 498);
        p.img("mac_btn_small", 151, 498);
        p.text_in(new_r, &s.s("MACRO", 18), &T_MAC);
        p.text_in(del_r, &s.s("MACRO", 19), &T_MAC);
        self.btn_fx(p, layer, new_r, !ed.recording, false);
        self.btn_fx(p, layer, del_r, ed.sel.is_some() && !ed.recording, false);
        if self.clicked(layer, new_r) && !ed.recording {
            self.cmds.push(Cmd::Push(Layer::Dialog(Dialog::NewMacro { name: String::new() })));
        }
        if self.clicked(layer, del_r) && ed.sel.is_some() && !ed.recording {
            self.cmds.push(Cmd::Push(Layer::Dialog(Dialog::Confirm(Confirm::DeleteMacro))));
        }

        // ---- editor list
        p.text(297, 110, &s.s("MACRO", 3), &T_MAC_HEAD);
        p.text(304, 142, &s.s("MACRO", 4), &T_MAC);
        p.text(382, 142, &s.s("MACRO", 5), &T_MAC);
        p.text(467, 142, &s.s("MACRO", 6), &T_MAC);
        let list = Rect::ltrb(294, 164, 566, 430);
        p.fill(list, rgb(0x060606));
        let mut opt_changed = false;
        if let Some(mi) = ed.sel {
            let n = ed.macros[mi].steps.len();
            let mut wheel_used = false;
            p.cv.set_clip(Some(list));
            for vis in 0..=MAC_VISIBLE {
                let idx = ed.scroll + vis;
                let Some(step) = ed.macros[mi].steps.get(idx).cloned() else { break };
                let r = Rect::new(294, MAC_ROWS_Y + MAC_ROW_H * vis as i32, 273, MAC_ROW_H);
                if ed.row == Some(idx) {
                    p.fill(r, rgb(0xc80000));
                }
                match step.action {
                    Action::Down | Action::Up => {
                        p.img(if step.action == Action::Down { "423" } else { "424" }, 299, r.y + 5);
                        p.text_in(Rect::new(340, r.y + 3, 75, 16), &step.key, &T_MAC);
                    }
                    Action::Delay => {
                        let editing = matches!(ed.edit, Some((MacField::Step(e), _)) if e == idx);
                        if editing {
                            let buf = &ed.edit.as_ref().unwrap().1;
                            p.fill(Rect::new(424, r.y + 3, 48, 17), rgb(0x5a5a5a));
                            p.text_in(Rect::new(424, r.y + 3, 48, 16), &format!("{buf}|"), &T_MAC);
                        } else {
                            p.text_in(Rect::new(420, r.y + 3, 55, 16), &step.ms.to_string(), &T_MAC);
                        }
                        // exact delays: wheel +-1 ms (Shift 10, Ctrl 100) over a delay row
                        let w = self.wheel(layer, r);
                        if w != 0 {
                            let mul = if self.inp.ctrl { 100 } else if self.inp.shift { 10 } else { 1 };
                            let st = &mut ed.macros[mi].steps[idx];
                            st.ms = (st.ms as i64 + w as i64 * mul).clamp(0, 65535) as u32;
                            ed.dirty = true;
                            wheel_used = true;
                        }
                        if self.inp.double && self.hover(layer, r) && !ed.recording {
                            ed.edit = Some((MacField::Step(idx), String::new()));
                        }
                    }
                }
                if self.pressed(layer, r) && !ed.recording {
                    ed.row = Some(idx);
                }
            }
            p.cv.set_clip(None);
            // wheel over a delay row edits it; anywhere else in the list it scrolls
            let wl = self.wheel(layer, list);
            if wl != 0 && !wheel_used {
                ed.scroll = (ed.scroll as i64 - wl as i64).clamp(0, n.saturating_sub(MAC_VISIBLE) as i64) as usize;
            }
            if n > MAC_VISIBLE {
                // scroll position marker (the original list grew a scrollbar here)
                let track = Rect::new(560, 165, 5, 264);
                p.fill(track, rgb(0x1a1a1a));
                let h = (track.h as usize * MAC_VISIBLE / n).max(12) as i32;
                let y = track.y + ((track.h - h) as usize * ed.scroll / (n - MAC_VISIBLE).max(1)) as i32;
                p.fill(Rect::new(track.x, y, track.w, h), rgb(0x660000));
            }
            let m = &ed.macros[mi];
            p.text_al(Rect::new(480, 440, 60, 16), &m.total_delay().to_string(), &T_MAC, Align::Right);
            p.text_al(Rect::new(450, 464, 62, 16), &m.action_count().to_string(), &T_MAC, Align::Right);
        }
        p.text(375, 440, &s.s("MACRO", 7), &T_MAC);
        p.text(566, 440, &s.s("MACRO", 8), &T_MAC);
        p.text(375, 466, &s.s("MACRO", 9), &T_MAC);
        p.text(554, 466, &s.s("MACRO", 10), &T_MAC);
        let rec_r = Rect::new(378, 501, 31, 24);
        let stop_r = Rect::new(471, 501, 31, 24);
        p.img(if ed.recording && blink() { "iconstart2" } else { "iconstart1" }, rec_r.x, rec_r.y);
        p.img(if self.hover(layer, stop_r) { "iconstop2" } else { "iconstop1" }, stop_r.x, stop_r.y);
        self.btn_fx(p, layer, rec_r, ed.sel.is_some() && !ed.recording || ed.recording && !ed.rec_inserting, ed.recording && !ed.rec_inserting);
        self.btn_fx(p, layer, stop_r, ed.recording, false);
        if self.clicked(layer, rec_r) && ed.sel.is_some() && !ed.recording {
            start_recording(ed, false);
        }
        if self.clicked(layer, stop_r) && ed.recording {
            stop_recording(ed);
        }

        // ---- options
        p.text(636, 110, &s.s("MACRO", 11), &T_MAC_OPT);
        {
            let mi = ed.sel.unwrap_or(usize::MAX);
            let m = ed.macros.get(mi).cloned().unwrap_or_else(|| {
                let mut m = Macro::new("");
                m.record_delay = false;
                m.use_default_delay = false;
                m.mode = PlayMode::WhileHeld;
                m
            });
            let none = ed.sel.is_none();
            let checks: [(i32, bool, String, i32); 5] = [
                (148, m.record_delay, s.s("MACRO", 12), 627),
                (179, m.use_default_delay, s.s("MACRO", 13), 649),
                (220, !none && m.mode == PlayMode::Loop, s.s("MACRO", 15), 627),
                (259, !none && m.mode == PlayMode::WhileHeld, s.s("MACRO", 16), 627),
                (299, m.mode == PlayMode::Toggle, s.s("MACRO", 28), 627),
            ];
            for (k, (y, on, label, lx)) in checks.iter().enumerate() {
                p.img(if *on { "mac_check_on" } else { "mac_check_off" }, 606, *y);
                p.text(*lx, y - 3, label, &T_MAC);
                let lw = p.width(label, &T_MAC);
                if !none && self.pressed(layer, Rect::new(604, y - 3, lx - 604 + lw, 18)) && !ed.recording {
                    let mm = &mut ed.macros[mi];
                    match k {
                        0 => mm.record_delay = !mm.record_delay,
                        1 => mm.use_default_delay = !mm.use_default_delay,
                        2 => mm.mode = PlayMode::Loop,
                        3 => mm.mode = PlayMode::WhileHeld,
                        _ => mm.mode = PlayMode::Toggle,
                    }
                    opt_changed = true;
                }
            }
            for (field, y, val, lo, hi) in [
                (MacField::DefaultDelay, 196, m.default_delay, 1u32, 65535u32),
                (MacField::Loops, 236, m.loops, 1, 255),
            ] {
                let r = Rect::new(626, y, 35, 17);
                p.img("mac_edit", r.x, r.y);
                let editing = matches!(&ed.edit, Some((f, _)) if *f == field);
                let shown = if editing { format!("{}|", ed.edit.as_ref().unwrap().1) } else { val.to_string() };
                p.text(r.x + 3, r.y + 1, &shown, &T_MAC);
                if !none && self.pressed(layer, r) && !ed.recording {
                    ed.edit = Some((field, String::new()));
                }
                let w = self.wheel(layer, r);
                if !none && w != 0 && !ed.recording {
                    let mul = if self.inp.shift { 10 } else { 1 };
                    let v = (val as i64 + w as i64 * mul).clamp(lo as i64, hi as i64) as u32;
                    let mm = &mut ed.macros[mi];
                    if field == MacField::Loops { mm.loops = v } else { mm.default_delay = v }
                    opt_changed = true;
                }
            }
            p.text(667, 199, &s.s("MACRO", 14), &T_MAC);
        }
        if opt_changed {
            ed.dirty = true;
        }

        // ---- action editing
        p.text(597, 323, &s.s("MACRO", 17), &T_MAC_HEAD);
        let labels = [20u32, 21, 22, 23, 24, 25, 26, 27];
        for (k, key) in labels.iter().enumerate() {
            let r = Rect::new(613, 344 + 22 * k as i32, 121, 17);
            p.img("mac_btn", r.x - 2, r.y - 2);
            p.text_in(r, &s.s("MACRO", *key), &T_MAC_BTN);
            let rec_here = k == 0 && ed.recording && ed.rec_inserting;
            self.btn_fx(p, layer, r, ed.sel.is_some() && !ed.recording || rec_here, rec_here);
            if self.clicked(layer, r) && ed.sel.is_some() && !ed.recording {
                match k {
                    0 => start_recording(ed, true),
                    1 => ed_delete(ed),
                    2 => ed_move(ed, -1),
                    3 => ed_move(ed, 1),
                    4 => ed_copy(ed, false),
                    5 => ed_copy(ed, true),
                    6 => ed_paste(ed),
                    _ => {
                        let items = vec![
                            MenuItem::leaf(s.g("FUNCTION_STRING", 1), MenuAct::InsertMouse(0)),
                            MenuItem::leaf(s.g("FUNCTION_STRING", 2), MenuAct::InsertMouse(1)),
                            MenuItem::leaf(s.g("FUNCTION_STRING", 3), MenuAct::InsertMouse(2)),
                            MenuItem::leaf(s.s("MACRO", 32), MenuAct::InsertDelay),
                        ];
                        let y = (r.y + r.h + 1).min(WIN_H - 4 - MENU_ITEM_H * items.len() as i32);
                        self.cmds.push(Cmd::Push(Layer::Menu(MenuState { owner: MenuOwner::MacroMore, items, x: 611, y, open: None, armed: true })));
                    }
                }
            }
        }

        // ---- bottom buttons
        let load_r = Rect::new(59, 539, 85, 23);
        let save_r = Rect::new(153, 539, 85, 23);
        let ok_r = Rect::new(539, 539, 85, 23);
        let cancel_r = Rect::new(638, 539, 85, 23);
        for (r, label) in [(load_r, s.s("MACRO", 29)), (save_r, s.s("MACRO", 30)), (ok_r, s.s("General", 46)), (cancel_r, s.s("General", 47))] {
            p.img("mac_gray_btn", r.x - 2, r.y - 2);
            p.text_in(r, &label, &T_MAC_BTN);
            let needs_sel = r == load_r || r == save_r;
            self.btn_fx(p, layer, r, !ed.recording && (!needs_sel || ed.sel.is_some()), false);
        }
        if !active {
            return;
        }

        // ---- keyboard: recording, inline edits, shortcuts
        let keys = self.inp.keys.clone();
        if ed.recording {
            record_keys(ed, &keys);
        } else if let Some((field, buf)) = &mut ed.edit {
            let field = *field;
            edit_digits(buf, &self.inp, &keys, 5);
            let commit = keys.iter().any(|k| k.pressed && k.name == "Enter") || (self.inp.pressed && !self.hover(layer, Rect::ltrb(424, 164, 566, 430)) && !matches!(field, MacField::DefaultDelay | MacField::Loops));
            let cancel = keys.iter().any(|k| k.pressed && k.name == "Escape");
            if commit || cancel {
                if commit {
                    if let (Some(mi), Ok(v)) = (ed.sel, buf.parse::<u32>()) {
                        let m = &mut ed.macros[mi];
                        match field {
                            MacField::DefaultDelay => m.default_delay = v.clamp(1, 65535),
                            MacField::Loops => m.loops = v.clamp(1, 255),
                            MacField::Step(i) => {
                                if let Some(st) = m.steps.get_mut(i) {
                                    st.ms = v.min(65535);
                                }
                            }
                        }
                        ed.dirty = true;
                    }
                }
                ed.edit = None;
            }
        } else {
            for k in &keys {
                if !k.pressed {
                    continue;
                }
                match (k.name.as_str(), self.inp.ctrl) {
                    ("Delete", _) => ed_delete(ed),
                    ("C", true) => ed_copy(ed, false),
                    ("X", true) => ed_copy(ed, true),
                    ("V", true) => ed_paste(ed),
                    ("ArrowUp", true) => ed_move(ed, -1),
                    ("ArrowDown", true) => ed_move(ed, 1),
                    ("ArrowUp", false) => ed.row = ed.row.map(|r| r.saturating_sub(1)),
                    ("ArrowDown", false) => {
                        if let Some(mi) = ed.sel {
                            let n = ed.macros[mi].steps.len();
                            ed.row = ed.row.map(|r| (r + 1).min(n.saturating_sub(1)));
                        }
                    }
                    _ => {}
                }
            }
        }
        if self.clicked(layer, load_r) && ed.sel.is_some() && !ed.recording {
            if let Some(path) = rfd::FileDialog::new().add_filter(&s.s("MACRO", 31), &["MSMACRO"]).pick_file() {
                if let Some(m) = Macro::load_from(&path) {
                    let mi = ed.sel.unwrap();
                    let name = ed.macros[mi].name.clone();
                    ed.macros[mi] = Macro { name, ..m };
                    ed.dirty = true;
                }
            }
        }
        if self.clicked(layer, save_r) && ed.sel.is_some() && !ed.recording {
            let m = &ed.macros[ed.sel.unwrap()];
            if let Some(path) = rfd::FileDialog::new().add_filter(&s.s("MACRO", 31), &["MSMACRO"]).set_file_name(format!("{}.MSMACRO", m.name)).save_file() {
                if let Err(e) = m.save_to(&path) {
                    self.flash(e.to_string());
                }
            }
        }
        if self.clicked(layer, ok_r) && !ed.recording {
            if !ed.held.is_empty() {
                self.cmds.push(Cmd::Push(Layer::Dialog(Dialog::Notice(s.s("WARNING", 13)))));
            } else {
                self.macros = ed.macros.clone();
                // drop references to deleted macros
                for pr in &mut self.profiles {
                    for f in &mut pr.buttons {
                        if let Func::Macro(n) = f {
                            if !self.macros.iter().any(|m| &m.name == n) {
                                *f = Func::Disabled;
                            }
                        }
                    }
                }
                if let (Some(b), Some(mi)) = (ed.button, ed.sel) {
                    self.cmds.push(Cmd::SetFunc(b, Func::Macro(ed.macros[mi].name.clone())));
                }
                if let Err(e) = save_macros(&self.dir, &self.macros) {
                    self.flash(e.to_string());
                }
                self.cmds.push(Cmd::Pop);
            }
        }
        if self.clicked(layer, cancel_r) && !ed.recording {
            if ed.dirty {
                self.cmds.push(Cmd::Push(Layer::Dialog(Dialog::Confirm(Confirm::LeaveMacro))));
            } else {
                self.cmds.push(Cmd::Pop);
            }
        }
    }

    fn draw_status(&mut self, p: &mut P) {
        if let Some((msg, at)) = &self.status {
            if at.elapsed().as_secs() < 4 {
                let w = p.width(msg, &T_BUBBLE) + 12;
                let r = Rect::new((WIN_W - w) / 2, 604, w, 16);
                p.fill(r, Rgba(0, 0, 0, 220));
                p.text_in(r, msg, &T_BUBBLE);
            } else {
                self.status = None;
            }
        }
    }

    // ---------------------------------------------------------- scripted scenes (for diffing)

    pub fn scene(&mut self, name: &str, s: &Strings) {
        if crate::models::current().is_kbd() {
            match name {
                "main" => self.tab = 0,
                "light" => self.tab = 1,
                "info" => self.tab = 2,
                "keymenu" | "keymedia" => {
                    let items = self.key_menu(s, 20);
                    let open = (name == "keymedia").then(|| items.len() - 2);
                    self.layers.push(Layer::Menu(MenuState { owner: MenuOwner::Key(20), items, x: 300, y: 260, open, armed: open.is_some() }));
                }
                "keyset" => {
                    // a remapped key and the lighting of the third mode
                    let kb = crate::models::current().kb.expect("keyboard");
                    self.kset(20, kb.code(&crate::kbd::KFunc::Mouse(0)));
                    self.kset(30, kb.code(&crate::kbd::KFunc::Media(0xe9)));
                }
                "assignkey" => self.layers.push(Layer::Dialog(Dialog::AssignKey { button: 20, key: String::new() })),
                n if n.starts_with("kled") => {
                    self.tab = 1;
                    let kb = crate::models::current().kb.expect("keyboard");
                    if let Some(pr) = &mut self.kprof {
                        pr.cfg.set_mode(kb, n[4..].parse().unwrap_or(0), kb.custom_index());
                    }
                }
                _ => {}
            }
            return;
        }
        match name {
            "main" => self.tab = 0,
            "dpi" => self.tab = 1,
            "light" => self.tab = 2,
            "info" => self.tab = 3,
            n if n.starts_with("led") => {
                self.tab = 2;
                self.profiles[self.cur].light_mode = n[3..].parse().unwrap_or(0);
            }
            "btnmenu" => {
                let items = self.button_menu(s, 4);
                self.layers.push(Layer::Menu(MenuState { owner: MenuOwner::Button(4), items, x: 334, y: 333, open: None, armed: false }));
            }
            "sub_adv" => {
                let items = self.button_menu(s, 4);
                self.layers.push(Layer::Menu(MenuState { owner: MenuOwner::Button(4), items, x: 334, y: 281, open: Some(8), armed: true }));
            }
            "perm" => self.layers.push(Layer::Dialog(Dialog::Permission)),
            "nodevice" => {
                // hot-plug window with nothing plugged (the scan is not repeated for a still)
                self.hotplug = Some(Hotplug { last: Instant::now(), seen: Vec::new(), none: true, offline: false, applied: None });
            }
            "select" => {
                // three plugged mice, as scan() reports them
                let fake = |key: &str, port: &str, n: u32| crate::device::Found {
                    hidraw: PathBuf::from(format!("/dev/hidraw{n}")),
                    usb: PathBuf::from(format!("/sys/bus/usb/devices/{port}")),
                    bcd: 0x0100,
                    model: crate::models::by_key(key).expect("model"),
                };
                let list = vec![fake(crate::models::M801, "1-4", 3), fake("m711", "3-2", 7), fake("legend", "3-1", 9)];
                self.layers.push(Layer::Dialog(Dialog::SelectDevice { list, sel: 1, top: 0 }));
            }
            "busy" => {
                let (_tx, rx) = std::sync::mpsc::channel();
                self.busy = Some(Busy { rx, phase: 2, since: Instant::now(), close_after: false });
            }
            "sub_basic" => {
                let items = self.button_menu(s, 4);
                self.layers.push(Layer::Menu(MenuState { owner: MenuOwner::Button(4), items, x: 334, y: 333, open: Some(7), armed: true }));
            }
            "firekey" => self.layers.push(Layer::Dialog(Dialog::FireKey { button: 3, fk: FireKey::default(), focus: 0, times: "3".into(), delay: "10".into() })),
            "assignkey" => self.layers.push(Layer::Dialog(Dialog::AssignKey { button: 5, key: String::new() })),
            "combokey" => self.layers.push(Layer::Dialog(Dialog::Combo { button: 5, key: String::new() })),
            "macro" | "macro1" | "macro_rec" | "macro_live" | "newmacro" => {
                self.open_macro_manager(None);
                if name != "macro" {
                    if let Some(Layer::Macro(ed)) = self.layers.last_mut() {
                        let mut m = Macro::new("Macro1");
                        if name == "macro_rec" {
                            for k in ["A", "B", "C"] {
                                m.steps.push(MacroStep { action: Action::Down, key: k.into(), ms: 0 });
                                m.steps.push(MacroStep { action: Action::Delay, key: String::new(), ms: 10 });
                                m.steps.push(MacroStep { action: Action::Up, key: k.into(), ms: 0 });
                                if k != "C" {
                                    m.steps.push(MacroStep { action: Action::Delay, key: String::new(), ms: 10 });
                                }
                            }
                            ed.row = Some(m.steps.len() - 1);
                        }
                        if name == "macro_live" {
                            // recording in progress: Ctrl+C, then Super held
                            for (a, k) in [(Action::Down, "Ctrl"), (Action::Down, "C"), (Action::Up, "C"), (Action::Up, "Ctrl"), (Action::Down, "Win")] {
                                m.steps.push(MacroStep { action: a, key: k.into(), ms: 0 });
                            }
                            ed.row = Some(m.steps.len() - 1);
                            ed.recording = true;
                            ed.held = vec!["Win".into()];
                        }
                        ed.macros = vec![m];
                        ed.sel = Some(0);
                    }
                }
                if name == "newmacro" {
                    self.layers.push(Layer::Dialog(Dialog::NewMacro { name: String::new() }));
                }
            }
            _ => {}
        }
    }
}

// ------------------------------------------------------------------ free helpers

/// Name of a keyboard key's firmware code, as the menus show it.
fn kfunc_label(s: &Strings, kb: &crate::kbd::Kbd, c: u32) -> String {
    use crate::kbd::KFunc;
    if c == u32::MAX {
        // VIA keyboards before the first read
        return "?".into();
    }
    match kb.func(c) {
        KFunc::Key { usage, mods } => {
            let m = mods | mods >> 4;
            let mut t = String::new();
            for (bit, n) in [(8, "WIN"), (1, "CTRL"), (2, "SHIFT"), (4, "ALT")] {
                if m & bit != 0 {
                    t.push_str(n);
                    t.push('+');
                }
            }
            t + &kbd_key_name(usage)
        }
        KFunc::Mouse(n) => s.g("FUNCTION_STRING", [1, 2, 3, 5, 4][n.min(4) as usize]),
        KFunc::Media(u) => media_label(s, u),
        KFunc::Disabled => s.g("FUNCTION_STRING", 16),
        KFunc::Fn => s.s("KBD", 5),
        KFunc::Raw(c) => format!("{c:08X}"),
    }
}

/// HID usage names beyond the mouse set (`device::usage_key`).
fn kbd_key_name(u: u8) -> String {
    const PAD: [&str; 16] = ["Num /", "Num *", "Num -", "Num +", "Num Enter", "Num 1", "Num 2", "Num 3", "Num 4", "Num 5", "Num 6", "Num 7", "Num 8", "Num 9", "Num 0", "Num ."];
    match u {
        0x32 => "#".into(),
        0x39 => "CapsLock".into(),
        0x46 => "PrintScreen".into(),
        0x47 => "ScrollLock".into(),
        0x48 => "Pause".into(),
        0x53 => "NumLock".into(),
        0x54..=0x63 => PAD[(u - 0x54) as usize].into(),
        0x64 => "\\".into(),
        0x65 => "App".into(),
        0xe7 => "RGui".into(),
        _ => crate::device::usage_key(u),
    }
}

/// Media key names: the mouse's MEDIA strings, then the keyboard's extras.
fn media_label(s: &Strings, u: u16) -> String {
    match crate::kbd::MEDIA.iter().position(|m| m.0 == u) {
        Some(i) if i < 8 => s.g("MEDIA", i as u32 + 1),
        Some(i) => s.s("KBD", 12 + (i as u32 - 8)),
        None => format!("0x{u:03X}"),
    }
}

/// Lighting mode name; the off and custom modes are translated.
fn kled_name(s: &Strings, kb: &crate::kbd::Kbd, l: &crate::kbd::LedOpt) -> String {
    if l.hw == 0 {
        s.s("KBD", 6)
    } else if Some(l.hw) == kb.custom_hw() && (l.name.starts_with("Mode") || l.name == "Self-define") {
        s.s("KBD", 7)
    } else {
        l.name.to_string()
    }
}

fn sub_pos(m: &MenuState, o: usize) -> (i32, i32) {
    let n = m.items[o].sub.len().max(1) as i32;
    let y = (m.y + MENU_ITEM_H * o as i32).min(WIN_H - 4 - MENU_ITEM_H * n);
    let x = if m.x + 2 * MENU_W > WIN_W { m.x - MENU_W } else { m.x + MENU_W };
    (x, y)
}

fn draw_menu_panel(p: &mut P, x: i32, y: i32, items: &[MenuItem], hot: Option<usize>) {
    for (i, it) in items.iter().enumerate() {
        let r = Rect::new(x, y + MENU_ITEM_H * i as i32, MENU_W, MENU_ITEM_H);
        p.img(if hot == Some(i) { "menu_item_hover" } else { "menu_item" }, r.x, r.y);
        // keep two spaces between the label and the submenu arrow
        let arrow_x = r.x + 132;
        let right = if it.arrow { arrow_x - p.width("  ", &T_MENU) } else { r.x + r.w - 4 };
        p.label_fit(r.offset(0, 1), &it.label, &T_MENU, right);
        if it.arrow {
            p.img("RightArrow", arrow_x, r.y + 6);
        }
    }
}

/// Small value read-out shown while hovering/dragging a slider (port addition).
/// What `poll_hotplug` compares: every plugged supported device by port and node.
fn plug_sig(found: &[crate::device::Found]) -> Vec<(String, PathBuf)> {
    let mut v: Vec<(String, PathBuf)> = found.iter().map(|f| (f.usb_name(), f.hidraw.clone())).collect();
    v.dedup();
    v
}

fn bubble(p: &mut P, cx: i32, bottom: i32, s: &str) {
    let w = p.width(s, &T_BUBBLE) + 8;
    let r = Rect::new(cx - w / 2, bottom - 14, w, 14);
    p.fill(r, Rgba(0, 0, 0, 230));
    p.cv.frame(r, 1, rgb(0xe60012));
    p.text_in(r.offset(0, 0), s, &T_BUBBLE);
}

fn blink() -> bool {
    use std::sync::OnceLock;
    static T0: OnceLock<Instant> = OnceLock::new();
    (T0.get_or_init(Instant::now).elapsed().as_millis() / 530) % 2 == 0
}

fn hue_of(c: u32) -> (f32, f32, f32) {
    let (r, g, b) = (((c >> 16) & 255) as f32, ((c >> 8) & 255) as f32, (c & 255) as f32);
    let mx = r.max(g).max(b).max(1.0);
    let mn = r.min(g).min(b);
    if mx - mn < 1.0 {
        return (255.0, 0.0, 0.0);
    }
    let f = |v: f32| (v - mn) / (mx - mn) * 255.0;
    (f(r), f(g), f(b))
}

fn is_modifier(n: &str) -> bool {
    matches!(n, "Shift" | "Ctrl" | "Alt" | "Win" | "Command")
}

fn combo_name(inp: &Input, key: &str) -> String {
    let mut s = String::new();
    for (on, n) in [(inp.logo, "WIN"), (inp.ctrl, "CTRL"), (inp.shift, "SHIFT"), (inp.alt, "ALT")] {
        if on {
            s.push_str(n);
            s.push('+');
        }
    }
    s.push_str(key);
    s
}

fn edit_digits(buf: &mut String, inp: &Input, keys: &[KeyEv], max: usize) {
    for c in inp.text.chars().filter(|c| c.is_ascii_digit()) {
        if buf.len() < max {
            buf.push(c);
        }
    }
    for k in keys {
        if k.pressed && k.name == "Backspace" {
            buf.pop();
        }
    }
}

fn start_recording(ed: &mut MacroEd, insert: bool) {
    let Some(mi) = ed.sel else { return };
    if !insert {
        ed.macros[mi].steps.clear();
        ed.row = None;
        ed.scroll = 0;
    }
    ed.rec_insert = if insert { ed.row.map(|r| r + 1).unwrap_or(ed.macros[mi].steps.len()) } else { 0 };
    ed.recording = true;
    ed.rec_inserting = insert;
    ed.last_event = None;
    ed.held.clear();
    ed.edit = None;
    ed.dirty = true;
}

fn stop_recording(ed: &mut MacroEd) {
    ed.recording = false;
    // release anything still held so the sequence is balanced
    if let Some(mi) = ed.sel {
        for k in std::mem::take(&mut ed.held) {
            let at = ed.rec_insert.min(ed.macros[mi].steps.len());
            ed.macros[mi].steps.insert(at, MacroStep { action: Action::Up, key: k, ms: 0 });
            ed.rec_insert = at + 1;
        }
    }
}

fn record_keys(ed: &mut MacroEd, keys: &[KeyEv]) {
    let Some(mi) = ed.sel else { return };
    for k in keys {
        if k.repeat {
            continue;
        }
        let m = &mut ed.macros[mi];
        let now = Instant::now();
        if let Some(prev) = ed.last_event {
            let ms = if m.record_delay {
                now.duration_since(prev).as_millis().min(65535) as u32
            } else if m.use_default_delay {
                m.default_delay
            } else {
                0
            };
            if m.record_delay || m.use_default_delay {
                let at = ed.rec_insert.min(m.steps.len());
                m.steps.insert(at, MacroStep { action: Action::Delay, key: String::new(), ms });
                ed.rec_insert = at + 1;
            }
        }
        let action = if k.pressed { Action::Down } else { Action::Up };
        if k.pressed {
            ed.held.push(k.name.clone());
        } else {
            ed.held.retain(|h| h != &k.name);
        }
        let at = ed.rec_insert.min(m.steps.len());
        m.steps.insert(at, MacroStep { action, key: k.name.clone(), ms: 0 });
        ed.rec_insert = at + 1;
        ed.row = Some(at);
        ed.last_event = Some(now);
        if at >= ed.scroll + MAC_VISIBLE {
            ed.scroll = at + 1 - MAC_VISIBLE;
        }
    }
}

fn ed_delete(ed: &mut MacroEd) {
    let (Some(mi), Some(r)) = (ed.sel, ed.row) else { return };
    let steps = &mut ed.macros[mi].steps;
    if r < steps.len() {
        steps.remove(r);
        ed.row = if steps.is_empty() { None } else { Some(r.min(steps.len() - 1)) };
        ed.dirty = true;
    }
}

fn ed_move(ed: &mut MacroEd, d: i64) {
    let (Some(mi), Some(r)) = (ed.sel, ed.row) else { return };
    let steps = &mut ed.macros[mi].steps;
    let t = r as i64 + d;
    if t >= 0 && (t as usize) < steps.len() {
        steps.swap(r, t as usize);
        ed.row = Some(t as usize);
        ed.dirty = true;
    }
}

fn ed_copy(ed: &mut MacroEd, cut: bool) {
    let (Some(mi), Some(r)) = (ed.sel, ed.row) else { return };
    if let Some(st) = ed.macros[mi].steps.get(r).cloned() {
        ed.clipboard = vec![st];
        if cut {
            ed_delete(ed);
        }
    }
}

fn ed_paste(ed: &mut MacroEd) {
    let Some(mi) = ed.sel else { return };
    let steps = &mut ed.macros[mi].steps;
    let mut at = ed.row.map(|r| r + 1).unwrap_or(steps.len()).min(steps.len());
    for st in ed.clipboard.clone() {
        steps.insert(at, st);
        at += 1;
    }
    if !ed.clipboard.is_empty() {
        ed.row = Some(at - 1);
        ed.dirty = true;
    }
}

fn ed_insert(ed: &mut MacroEd, a: MenuAct) {
    let Some(mi) = ed.sel else { return };
    let m = &mut ed.macros[mi];
    let mut at = ed.row.map(|r| r + 1).unwrap_or(m.steps.len()).min(m.steps.len());
    let new: Vec<MacroStep> = match a {
        MenuAct::InsertMouse(b) => {
            let key = ["Mouse Left", "Mouse Right", "Mouse Middle"][b as usize].to_string();
            vec![
                MacroStep { action: Action::Down, key: key.clone(), ms: 0 },
                MacroStep { action: Action::Delay, key: String::new(), ms: m.default_delay },
                MacroStep { action: Action::Up, key, ms: 0 },
            ]
        }
        MenuAct::InsertDelay => vec![MacroStep { action: Action::Delay, key: String::new(), ms: m.default_delay }],
        _ => return,
    };
    for st in new {
        m.steps.insert(at, st);
        at += 1;
    }
    ed.row = Some(at - 1);
    ed.dirty = true;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct H {
        res: Res,
        st: State,
        cv: Canvas,
        dir: PathBuf,
    }

    impl H {
        fn new(tag: &str) -> H {
            let fonts = Fonts::new(
                include_bytes!("../assets/fonts/tahoma.ttf"),
                include_bytes!("../assets/fonts/tahomabd.ttf"),
                (include_bytes!("../assets/fonts/NotoSansCJKtc-Regular-subset.otf"), 0),
                Some((include_bytes!("../assets/fonts/NotoSansCJKtc-Bold-subset.otf"), 0)),
            );
            let dir = std::env::temp_dir().join(format!("m801-ui-{tag}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            H { res: Res::new(fonts, Lang::Tc), st: State::new(dir.clone(), Lang::Tc), cv: Canvas::new(WIN_W, WIN_H), dir }
        }
        fn frame(&mut self, inp: Input) {
            self.st.frame(&self.res, &mut self.cv, inp);
        }
        fn at(&self, x: i32, y: i32) -> Input {
            Input { mouse: (x, y), ..Default::default() }
        }
        fn click(&mut self, x: i32, y: i32) {
            self.frame(self.at(x, y));
            self.frame(Input { pressed: true, down: true, ..self.at(x, y) });
            self.frame(Input { released: true, ..self.at(x, y) });
        }
        fn double(&mut self, x: i32, y: i32) {
            self.frame(Input { pressed: true, down: true, double: true, ..self.at(x, y) });
            self.frame(Input { released: true, ..self.at(x, y) });
        }
        fn wheel(&mut self, x: i32, y: i32, n: i32, shift: bool) {
            self.frame(Input { wheel: n, shift, ..self.at(x, y) });
        }
        fn key(&mut self, name: &str, pressed: bool) {
            let k = KeyEv { name: name.into(), pressed, repeat: false };
            self.frame(Input { keys: vec![k], ..self.at(-1, -1) });
        }
        fn tap(&mut self, name: &str) {
            self.key(name, true);
            self.key(name, false);
        }
        fn text(&mut self, s: &str) {
            self.frame(Input { text: s.into(), ..self.at(-1, -1) });
        }
    }

    impl Drop for H {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    #[test]
    fn device_selection_switches_model_and_layout() {
        let mut h = H::new("select");
        let strs = Strings::load(Lang::Tc);
        h.st.scene("select", &strs);
        // second row (M711 COBRA) is preselected; pick the third (LEGEND), then OK
        h.click(400, 232 + 48 * 2 + 20);
        h.tap("Enter");
        assert!(h.st.layers.is_empty());
        assert_eq!(crate::models::current().key, "legend");
        assert_eq!(h.st.target.as_deref(), Some("3-1"));
        assert_eq!(h.st.profiles[0].buttons.len(), 25);
        assert!(h.st.dir.ends_with("models/legend"));
        // 25 rows: the list scrolls; wheel down shows the last rows
        h.frame(h.at(-1, -1));
        h.wheel(400, 300, -30, false);
        assert_eq!(h.st.btn_top, 25 - ROWS);
        // the Cobra: 8 buttons + wheel, 100..10000 CPI, stage 5 wheel-edited to the top
        h.st.set_model(crate::models::by_key("m711").unwrap());
        assert_eq!(h.st.profiles[0].buttons.len(), 10);
        h.click(318, 172);
        h.wheel(668, 300, 500, false);
        assert_eq!(h.st.profiles[0].dpi[4], 10000);
    }

    #[test]
    fn tabs_and_profiles_switch() {
        let mut h = H::new("tabs");
        h.click(318, 172);
        assert_eq!(h.st.tab, 1);
        h.click(483, 172);
        assert_eq!(h.st.tab, 2);
        h.click(271, 534);
        assert_eq!(h.st.cur, 1);
    }

    #[test]
    fn button_menu_assigns_function() {
        let mut h = H::new("menu");
        h.click(406, 323); // button 5
        assert!(matches!(h.st.layers.last(), Some(Layer::Menu(_))));
        let (mx, my) = match h.st.layers.last() { Some(Layer::Menu(m)) => (m.x, m.y), _ => panic!() };
        h.click(mx + 40, my + 8); // first item: click
        assert!(h.st.layers.is_empty());
        assert_eq!(h.st.profiles[0].buttons[4], Func::Click);
        // submenu: open DPI switch (item 12) and pick "DPI -"
        h.click(406, 323);
        let (mx, my) = match h.st.layers.last() { Some(Layer::Menu(m)) => (m.x, m.y), _ => panic!() };
        h.frame(h.at(mx + 40, my + 12 * MENU_ITEM_H + 8));
        h.click(mx + MENU_W + 40, my + 12 * MENU_ITEM_H + 2 * MENU_ITEM_H + 8);
        assert_eq!(h.st.profiles[0].buttons[4], Func::Dpi(3));
    }

    #[test]
    fn last_click_button_is_protected() {
        let mut h = H::new("guard");
        h.st.profiles[0].buttons[1] = Func::Disabled;
        h.st.profiles[0].buttons[2] = Func::Disabled;
        h.click(406, 211); // button 1 (the only "click")
        let (mx, my) = match h.st.layers.last() { Some(Layer::Menu(m)) => (m.x, m.y), _ => panic!() };
        h.click(mx + 40, my + 16 * MENU_ITEM_H + 8); // last item: disabled
        assert_eq!(h.st.profiles[0].buttons[0], Func::Click);
        assert!(matches!(h.st.layers.last(), Some(Layer::Dialog(Dialog::Notice(_)))));
    }

    #[test]
    fn dpi_wheel_and_exact_entry() {
        let mut h = H::new("dpi");
        h.click(318, 172);
        h.wheel(178, 350, 1, false);
        assert_eq!(h.st.profiles[0].dpi[0], 600);
        h.wheel(178, 350, 1, true);
        assert_eq!(h.st.profiles[0].dpi[0], 1600);
        h.click(174, 467);
        h.text("7777");
        h.tap("Enter");
        assert_eq!(h.st.profiles[0].dpi[0], 7800); // snapped to the 200-step range
        h.wheel(668, 350, 50, false);
        assert_eq!(h.st.profiles[0].dpi[4], DPI_MAX);
    }

    #[test]
    fn dpi_stage_select_and_right_click_toggle() {
        let mut h = H::new("stage");
        h.click(318, 172);
        h.click(301, 215); // DPI2 title: select stage
        assert_eq!(h.st.profiles[0].dpi_stage, 1);
        h.frame(Input { right_pressed: true, ..h.at(301, 215) });
        assert!(!h.st.profiles[0].dpi_enabled[1]);
        h.frame(Input { right_pressed: true, ..h.at(301, 215) });
        assert!(h.st.profiles[0].dpi_enabled[1]);
    }

    #[test]
    fn slider_wheel_is_clamped() {
        let mut h = H::new("slider");
        h.wheel(600, 226, 10, false);
        assert_eq!(h.st.profiles[0].acceleration, ACCEL_RANGE.1);
        h.wheel(600, 275, -3, false);
        assert_eq!(h.st.profiles[0].pointer_speed, 7);
    }

    #[test]
    fn macro_create_record_edit_save() {
        let mut h = H::new("macro");
        h.st.open_macro_manager(Some(4));
        h.click(98, 508); // new
        h.text("Combo");
        h.tap("Enter");
        let Some(Layer::Macro(ed)) = h.st.layers.last() else { panic!("manager closed") };
        assert_eq!(ed.macros.len(), 1);
        h.click(393, 513); // record
        h.key("A", true);
        std::thread::sleep(std::time::Duration::from_millis(30));
        h.key("A", false);
        h.click(486, 513); // stop
        let Some(Layer::Macro(ed)) = h.st.layers.last() else { panic!() };
        let steps = &ed.macros[0].steps;
        assert_eq!(steps.len(), 3, "{steps:?}");
        assert_eq!(steps[0].action, Action::Down);
        assert_eq!(steps[1].action, Action::Delay);
        assert!(steps[1].ms >= 20);
        // exact delay: double-click the delay row and type it
        h.double(450, MAC_ROWS_Y + MAC_ROW_H + 10);
        h.text("25");
        h.tap("Enter");
        // wheel nudges it by 1 ms
        h.wheel(450, MAC_ROWS_Y + MAC_ROW_H + 10, 2, false);
        // more... -> middle click inserted after the selected row
        h.click(673, 506);
        let (mx, my) = match h.st.layers.last() { Some(Layer::Menu(m)) => (m.x, m.y), _ => panic!() };
        h.click(mx + 40, my + 2 * MENU_ITEM_H + 8);
        let kinds: Vec<&str> = h.st.layers.iter().map(|l| match l { Layer::Menu(_) => "menu", Layer::Dialog(d) => match d { Dialog::Notice(_) => "notice", Dialog::Confirm(_) => "confirm", _ => "dialog" }, Layer::Macro(_) => "macro" }).collect();
        let Some(Layer::Macro(ed)) = h.st.layers.last() else { panic!("{kinds:?}") };
        let m = &ed.macros[0];
        assert_eq!(m.steps[1].ms, 27);
        assert_eq!(m.action_count(), 4);
        // loop count: wheel on the edit box
        h.wheel(640, 244, 4, false);
        h.click(581, 550); // OK
        assert!(h.st.layers.is_empty());
        assert_eq!(h.st.macros[0].loops, 5);
        assert_eq!(h.st.profiles[0].buttons[4], Func::Macro("Combo".into()));
        assert_eq!(load_macros(&h.dir), h.st.macros);
    }

    #[test]
    fn fire_key_dialog() {
        let mut h = H::new("fire");
        h.st.scene("firekey", &h.res.strs);
        h.wheel(571, 305, 2, false);
        h.wheel(571, 346, 1, true);
        h.click(255, 285); // right button target
        h.click(339, 398); // OK
        match &h.st.profiles[0].buttons[3] {
            Func::FireKey(f) => {
                assert_eq!((f.target, f.times, f.delay_ms), (1, 5, 20));
            }
            f => panic!("{f:?}"),
        }
    }

    #[test]
    fn long_submenu_labels_keep_clear_of_the_arrow() {
        for lang in Lang::ALL {
            let h = H::new(&format!("ell-{}", lang.code()));
            let res = Res::new(
                Fonts::new(
                    include_bytes!("../assets/fonts/tahoma.ttf"),
                    include_bytes!("../assets/fonts/tahomabd.ttf"),
                    (include_bytes!("../assets/fonts/NotoSansCJKtc-Regular-subset.otf"), 0),
                    Some((include_bytes!("../assets/fonts/NotoSansCJKtc-Bold-subset.otf"), 0)),
                ),
                lang,
            );
            let mut cv = Canvas::new(WIN_W, WIN_H);
            let p = P { r: &res, cv: &mut cv };
            let gap = p.width("  ", &T_MENU);
            for it in h.st.button_menu(&res.strs, 4) {
                let limit = 132 - gap - 4;
                let shown = if it.arrow { p.ellipsize(&it.label, &T_MENU, limit) } else { it.label.clone() };
                if it.arrow {
                    assert!(p.width(&shown, &T_MENU) <= limit, "{lang:?} {shown}");
                }
            }
        }
    }

    #[test]
    fn language_switch_from_info_page() {
        let mut h = H::new("lang");
        h.click(647, 172);
        h.click(271 + 240, 534); // 4th = English
        assert_eq!(h.st.lang, Lang::En);
        assert!(h.st.lang_changed);
    }

    #[test]
    fn keyboard_keys_lighting_and_profile() {
        use crate::kbd::{KFunc, KbProfile};
        let mut h = H::new("kbd");
        h.st.set_model(crate::models::by_key("k605").expect("k605"));
        let kb = crate::models::current().kb.expect("keyboard");
        let (ox, oy) = (400 - kb.photo_w as i32 / 2, 200 + (300 - kb.photo_h as i32) / 2);
        let k = &kb.keys[20];
        let (cx, cy) = (ox + k.x as i32 + k.w as i32 / 2, oy + k.y as i32 + k.h as i32 / 2);
        // click a key: its menu; the last entry disables it
        h.click(cx, cy);
        let (mx, my, n) = match h.st.layers.last() {
            Some(Layer::Menu(m)) => (m.x, m.y, m.items.len()),
            _ => panic!("no key menu"),
        };
        h.click(mx + 40, my + MENU_ITEM_H * (n as i32 - 1) + 10);
        assert!(h.st.layers.is_empty());
        assert_eq!(h.st.kcode(20), 0);
        // a combo from the combo dialog, coded with its modifier mask
        let d = Dialog::Combo { button: 20, key: "CTRL+SHIFT+A".into() };
        h.st.layers.push(Layer::Dialog(d.clone()));
        h.st.dialog_ok(&d);
        h.st.apply_cmds();
        assert_eq!(h.st.kcode(20), kb.code(&KFunc::Key { usage: 4, mods: 3 }));
        // restore: the driver's default code comes back
        h.st.layers.push(Layer::Dialog(Dialog::Confirm(Confirm::RestoreOne)));
        h.st.dialog_ok(&Dialog::Confirm(Confirm::RestoreOne));
        h.st.apply_cmds();
        assert_eq!(h.st.kcode(20), k.default);
        // lighting: third mode, lowest brightness
        h.st.tab = 1;
        h.click(105 + 148 * 2 + 72, 195 + 11);
        let cfg = &h.st.kprof.as_ref().unwrap().cfg;
        assert_eq!(cfg.mode(kb), 2);
        let y0 = 195 + 26 * kb.leds.len().div_ceil(4) as i32 + 8;
        h.click(485 + 40, y0 + 30 + 8);
        assert_eq!(h.st.kprof.as_ref().unwrap().cfg.light(kb, 2).bright, 0);
        // the profile file keeps all of it
        h.st.save_all().unwrap();
        let back = KbProfile::load(&h.st.dir.join("Keyboard.ini"), kb).expect("saved profile");
        assert_eq!(Some(back), h.st.kprof);
    }
}
