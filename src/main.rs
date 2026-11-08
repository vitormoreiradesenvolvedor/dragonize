mod assets;
mod compx;
mod device;
mod du;
mod gfx;
mod kbd;
mod model;
mod models;
mod sino;
mod ui;
mod bd;
mod dc;
mod pxi;
mod mo;
mod xy;
mod daemon;
mod pointer;

use eframe::egui;
use std::path::PathBuf;

use gfx::{Canvas, Fonts};
use ui::{Input, KeyEv, Lang, Request, Res, State, WIN_H, WIN_W};

const APP_NAME: &str = "Dragonize";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

const TAHOMA: &[u8] = include_bytes!("../assets/fonts/tahoma.ttf");
const TAHOMA_BOLD: &[u8] = include_bytes!("../assets/fonts/tahomabd.ttf");
const CJK: &[u8] = include_bytes!("../assets/fonts/NotoSansCJKtc-Regular-subset.otf");
const CJK_BOLD: &[u8] = include_bytes!("../assets/fonts/NotoSansCJKtc-Bold-subset.otf");

/// The original UI resolved CJK text to Microsoft JhengHei through font
/// linking. When that face is installed it is used for an exact match;
/// otherwise the bundled Noto Sans CJK TC subset keeps the AppImage self-contained.
fn find_font(names: &[&str]) -> Option<&'static [u8]> {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let dirs = [
        PathBuf::from("/usr/share/fonts"),
        PathBuf::from("/usr/local/share/fonts"),
        home.join(".local/share/fonts"),
        home.join(".fonts"),
    ];
    for d in dirs {
        let mut stack = vec![d];
        while let Some(dir) = stack.pop() {
            let rd = match std::fs::read_dir(&dir) {
                Ok(rd) => rd,
                Err(e) => {
                    if std::env::var_os("M801_DEBUG_FONTS").is_some() {
                        eprintln!("font dir {}: {e}", dir.display());
                    }
                    continue;
                }
            };
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if let Some(n) = p.file_name().and_then(|n| n.to_str()) {
                    if names.iter().any(|w| n.eq_ignore_ascii_case(w)) {
                        if std::env::var_os("M801_DEBUG_FONTS").is_some() {
                            eprintln!("font: {}", p.display());
                        }
                        if let Ok(b) = std::fs::read(&p) {
                            return Some(Box::leak(b.into_boxed_slice()));
                        }
                    }
                }
            }
        }
    }
    None
}

fn load_fonts(bundled_only: bool) -> Fonts {
    let sys = if bundled_only { None } else { find_font(&["msjh.ttc"]) };
    match sys {
        // msjh.ttc: index 0 = Microsoft JhengHei; there is no separate bold
        // file needed, GDI emboldened the linked face too.
        Some(b) => Fonts::new(TAHOMA, TAHOMA_BOLD, (b, 0), find_font(&["msjhbd.ttc"]).map(|b| (b, 0))),
        None => Fonts::new(TAHOMA, TAHOMA_BOLD, (CJK, 0), Some((CJK_BOLD, 0))),
    }
}

fn lang_file() -> PathBuf {
    model::config_dir().join("language")
}

fn initial_lang(cli: Option<&String>) -> Lang {
    if let Some(l) = cli.and_then(|c| Lang::from_code(c)) {
        return l;
    }
    if let Some(l) = std::env::var("M801_LANG").ok().and_then(|c| Lang::from_code(&c)) {
        return l;
    }
    if let Some(l) = std::fs::read_to_string(lang_file()).ok().and_then(|c| Lang::from_code(c.trim())) {
        return l;
    }
    // English by default; no locale guessing so every machine starts the same
    Lang::En
}

struct App {
    res: Res,
    state: State,
    canvas: Canvas,
    tex: Option<egui::TextureHandle>,
    last_click: Option<(std::time::Instant, (i32, i32))>,
    scale: f32,
    /// modifiers held after the last frame (Ctrl, Shift, Alt, Super): egui sends no key events for them
    mods: [bool; 4],
}

/// Key names of the modifiers in `App::mods` order (device.rs `key_usage`).
const MOD_KEYS: [&str; 4] = ["Ctrl", "Shift", "Alt", "Win"];

fn mods_of(m: &egui::Modifiers) -> [bool; 4] {
    // mac_cmd = the Super key on every platform (vendor/egui-winit)
    [m.ctrl, m.shift, m.alt, m.mac_cmd]
}

/// Key events for the modifiers that changed from `held` to `now`: presses first, releases after.
fn mod_events(held: &mut [bool; 4], now: [bool; 4], out: &mut Vec<KeyEv>) {
    for i in 0..4 {
        if now[i] != held[i] {
            out.push(KeyEv { name: MOD_KEYS[i].to_string(), pressed: now[i], repeat: false });
            held[i] = now[i];
        }
    }
}

impl App {
    fn gather_input(&mut self, ctx: &egui::Context) -> Input {
        let mut inp = Input::default();
        let scale = self.scale;
        ctx.input(|i| {
            if let Some(p) = i.pointer.latest_pos() {
                inp.mouse = ((p.x / scale).floor() as i32, (p.y / scale).floor() as i32);
            } else {
                inp.mouse = (-1, -1);
            }
            inp.down = i.pointer.primary_down();
            inp.pressed = i.pointer.primary_pressed();
            inp.released = i.pointer.primary_released();
            inp.right_pressed = i.pointer.secondary_pressed();
            inp.shift = i.modifiers.shift;
            inp.ctrl = i.modifiers.ctrl;
            inp.alt = i.modifiers.alt;
            inp.logo = i.modifiers.mac_cmd;
            for e in &i.events {
                match e {
                    egui::Event::Key { key, pressed, repeat, modifiers, .. } => {
                        // each key event carries the modifiers held with it: their presses / releases go first
                        mod_events(&mut self.mods, mods_of(modifiers), &mut inp.keys);
                        inp.keys.push(KeyEv { name: key.name().to_string(), pressed: *pressed, repeat: *repeat });
                    }
                    egui::Event::Text(t) => inp.text.push_str(t),
                    egui::Event::MouseWheel { delta, unit, .. } => {
                        let notches = match unit {
                            egui::MouseWheelUnit::Line => delta.y,
                            egui::MouseWheelUnit::Page => delta.y * 3.0,
                            egui::MouseWheelUnit::Point => delta.y / 50.0,
                        };
                        inp.wheel += notches.round() as i32;
                    }
                    _ => {}
                }
            }
            mod_events(&mut self.mods, mods_of(&i.modifiers), &mut inp.keys);
        });
        if inp.pressed {
            let now = std::time::Instant::now();
            if let Some((t, p)) = self.last_click {
                let near = (p.0 - inp.mouse.0).abs() < 4 && (p.1 - inp.mouse.1).abs() < 4;
                inp.double = near && now.duration_since(t).as_millis() < 450;
            }
            self.last_click = Some((now, inp.mouse));
        }
        inp
    }
}

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let inp = self.gather_input(ctx);
        self.state.frame(&self.res, &mut self.canvas, inp);
        if self.state.lang_changed {
            self.state.lang_changed = false;
            self.res.strs = ui::Strings::load(self.state.lang);
            let _ = std::fs::create_dir_all(model::config_dir());
            let _ = std::fs::write(lang_file(), self.state.lang.code());
        }
        for r in std::mem::take(&mut self.state.requests) {
            match r {
                Request::Minimize => ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true)),
                Request::Close => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                Request::Drag => ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag),
            }
        }
        let img = egui::ColorImage::from_rgba_unmultiplied([WIN_W as usize, WIN_H as usize], &self.canvas.to_rgba_bytes());
        // nearest filtering keeps pixels crisp on HiDPI (integer scale)
        let opts = egui::TextureOptions::NEAREST;
        match &mut self.tex {
            Some(t) => t.set(img, opts),
            None => self.tex = Some(ctx.load_texture("canvas", img, opts)),
        }
        let tex = self.tex.as_ref().unwrap();
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| {
            let size = egui::vec2(WIN_W as f32 * self.scale, WIN_H as f32 * self.scale);
            ui.painter().image(
                tex.id(),
                egui::Rect::from_min_size(egui::pos2(0.0, 0.0), size),
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        });
        ctx.request_repaint_after(std::time::Duration::from_millis(33));
    }
}

fn main() {
    let args = clap::Command::new(APP_NAME)
        .version(APP_VERSION)
        .about("Dragonize: unofficial configuration tool for the M801-RGB and other Redragon gaming mice and keyboards (see --list-models)")
        .arg(clap::Arg::new("screenshot").long("screenshot").value_name("FILE.PNG").help("Render a scene to a PNG and exit (no window)"))
        .arg(clap::Arg::new("scene").long("scene").value_name("NAME").default_value("main").help("Scene for --screenshot: main dpi light info led0..led7 btnmenu sub_basic firekey assignkey combokey macro macro1 macro_rec newmacro perm select nodevice; keyboards: main light info kled0.. keymenu keymedia keyset assignkey"))
        .arg(clap::Arg::new("lang").long("lang").value_name("tc|sc|pt|en|ru").help("UI language (default: saved choice, then M801_LANG, then Traditional Chinese like the original)"))
        .arg(clap::Arg::new("bundled-fonts").long("bundled-fonts").action(clap::ArgAction::SetTrue).help("Ignore system Microsoft JhengHei and use the bundled fonts"))
        .arg(clap::Arg::new("device-dump").long("device-dump").action(clap::ArgAction::SetTrue).help("Print the configuration stored on the connected mouse and exit"))
        .arg(clap::Arg::new("device-load").long("device-load").value_names(["FILE.pfd", "PROFILE"]).num_args(2).help("Write a saved profile to on-device profile 1-5 (same path as Apply) and exit"))
        .arg(clap::Arg::new("device-roundtrip").long("device-roundtrip").action(clap::ArgAction::SetTrue).help("Check that reading the mouse into the app and encoding it back changes no byte (no write)"))
        .arg(clap::Arg::new("device-apply").long("device-apply").action(clap::ArgAction::SetTrue).help("Run the Apply button's device path (write, reconnect, reopen) with the saved profiles and exit"))
        .arg(clap::Arg::new("diagnose").long("diagnose").action(clap::ArgAction::SetTrue).help("Report why the mouse can or cannot be used on this machine"))
        .arg(clap::Arg::new("install-udev-rule").long("install-udev-rule").action(clap::ArgAction::SetTrue).help("Grant the logged-in user access to the mouse (asks for the admin password once)"))
        .arg(clap::Arg::new("no-device").long("no-device").action(clap::ArgAction::SetTrue).help("Do not talk to the mouse (edit saved profiles only)"))
        .arg(clap::Arg::new("config-dir").long("config-dir").value_name("DIR").help("Profile directory (default: ~/.config/m801-rgb)"))
        .arg(clap::Arg::new("model").long("model").value_name("KEY").help("Model to show without a mouse (screenshots, --no-device); see --list-models"))
        .arg(clap::Arg::new("usb").long("usb").value_name("PORT").help("With several mice plugged, the one on this USB port (as --diagnose prints it)"))
        .arg(clap::Arg::new("daemon").long("daemon").action(clap::ArgAction::SetTrue).help("Background mode: follow the mouse's profile button and apply each profile's pointer settings (started by the app and at login)"))
        .arg(clap::Arg::new("autostart").long("autostart").value_name("on|off").value_parser(["on", "off"]).help("Start the background mode at login (default on)"))
        .arg(clap::Arg::new("list-models").long("list-models").action(clap::ArgAction::SetTrue).help("List the supported models and exit"))
        .get_matches();

    if args.get_flag("list-models") {
        for m in models::MODELS {
            let rev = if m.bcd == (0, 0) { "any      ".to_string() } else { format!("{:04x}-{:04x}", m.bcd.0, m.bcd.1) };
            if let Some(kb) = m.kb {
                println!("{:<13} {:04x}:{:04x} rev {}  {:<22} keyboard, {} keys, {} lighting modes", m.key, m.vid, m.pid, rev, m.name, kb.keys.len(), kb.leds.len());
                continue;
            }
            println!("{:<13} {:04x}:{:04x} rev {}  {:<22} {} buttons, {}-{} CPI{}", m.key, m.vid, m.pid, rev, m.name,
                m.buttons.len(), m.dpi_min(), m.dpi_max(), if m.light_modes().is_none() { ", lighting not supported" } else { "" });
        }
        return;
    }
    if let Some(k) = args.get_one::<String>("model") {
        match models::by_key(k) {
            Some(m) => models::set_current(m),
            None => {
                eprintln!("unknown model {k:?}; see --list-models");
                std::process::exit(1);
            }
        }
    }
    // CLI device commands: the chosen (or only) mouse, its model made current
    let open_cli = || -> std::io::Result<device::Device> {
        let d = device::Device::open_first(args.get_one::<String>("usb").map(|s| s.as_str()))?;
        models::set_current(d.found.model);
        Ok(d)
    };

    if args.get_flag("daemon") {
        daemon::run();
        return;
    }
    if let Some(v) = args.get_one::<String>("autostart") {
        match daemon::set_autostart(v == "on") {
            Ok(()) => println!("background mode at login: {v}"),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return;
    }
    if args.get_flag("diagnose") {
        print!("{}", device::diagnose());
        println!("{}", daemon::status());
        return;
    }
    if args.get_flag("install-udev-rule") {
        match device::install_udev_rule() {
            Ok(()) => println!("access granted: {}", device::UDEV_PATH),
            Err(e) => {
                eprintln!("{e}\nmanual install:\n  sudo tee {} <<'EOF'\n{}EOF\n  sudo udevadm control --reload-rules && sudo udevadm trigger", device::UDEV_PATH, device::udev_rule());
                std::process::exit(1);
            }
        }
        return;
    }
    if args.get_flag("device-dump") {
        match open_cli().and_then(|d| device::read_config(&d).map(|c| (d, c))) {
            Ok((d, c)) => {
                println!("{} ({}, rev {:04x})  device {}  active profile {}", d.found.model.name, d.found.model.key, d.found.bcd, d.path.display(), c.active + 1);
                let macros = device::macros_from_device(&c, &[]);
                for m in &macros {
                    let steps: Vec<String> = m.steps.iter().map(|s| match s.action {
                        model::Action::Down => format!("+{}", s.key),
                        model::Action::Up => format!("-{}", s.key),
                        model::Action::Delay => format!("{}ms", s.ms),
                    }).collect();
                    println!("macro slot {:>2}: {}", m.slot.unwrap_or(0) + 1, steps.join(" "));
                }
                for (i, p) in c.profiles.iter().enumerate() {
                    let mut pr = model::Profile::default();
                    device::apply_to_profile_with(p, &mut pr, &macros);
                    let l = &pr.lights[pr.light_mode];
                    println!("profile {} lighting: mode {} color #{:06x} brightness {}", i + 1, pr.light_mode, l.color, l.brightness + 1);
                    println!("profile {}: stage {} polling {} Hz", i + 1, pr.dpi_stage + 1, model::POLLING_HZ_LIST[pr.polling]);
                    for s in 0..5 {
                        println!("  DPI{} {:>5} {}", s + 1, pr.dpi[s], if pr.dpi_enabled[s] { "on" } else { "off" });
                    }
                    for (b, raw) in p.buttons.iter().enumerate() {
                        let f = device::decode_func(*raw).map(|f| format!("{f:?}")).unwrap_or_else(|| "(raw)".into());
                        println!("  button {:>2}: {:02x} {:02x} {:02x} {:02x}  {f}", b + 1, raw[0], raw[1], raw[2], raw[3]);
                    }
                }
            }
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return;
    }
    if args.get_flag("device-roundtrip") {
        let r = (|| -> Result<usize, String> {
            let d = open_cli().map_err(|e| e.to_string())?;
            let cur = device::read_config(&d).map_err(|e| e.to_string())?;
            let mut macros = device::macros_from_device(&cur, &[]);
            let mut profiles = vec![model::Profile::default(); 5];
            for (i, p) in cur.profiles.iter().enumerate() {
                device::apply_to_profile_with(p, &mut profiles[i], &macros);
            }
            let new = device::DeviceConfig {
                active: cur.active,
                macros: device::macros_to_device(&mut macros, &cur.macros),
                profiles: (0..5).map(|i| device::profile_to_device_with(&profiles[i], &cur.profiles[i], &macros)).collect(),
            };
            let mut diffs = 0;
            for (i, (a, b)) in cur.profiles.iter().zip(&new.profiles).enumerate() {
                if a != b {
                    diffs += 1;
                    println!("profile {}:\n  device {:?}\n  app    {:?}", i + 1, a, b);
                }
            }
            for (s, (a, b)) in cur.macros.iter().zip(&new.macros).enumerate() {
                if a != b {
                    diffs += 1;
                    println!("macro slot {} differs", s + 1);
                }
            }
            Ok(diffs)
        })();
        match r {
            Ok(0) => println!("round trip identical: Apply without changes writes nothing"),
            Ok(n) => {
                println!("{n} blocks differ");
                std::process::exit(2);
            }
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return;
    }
    if let Some(mut v) = args.get_many::<String>("device-load") {
        let (file, n) = (v.next().unwrap(), v.next().unwrap().parse::<usize>().unwrap_or(1).clamp(1, 5) - 1);
        let r = (|| -> Result<(), String> {
            let d = open_cli().map_err(|e| e.to_string())?;
            let pr = model::Profile::load(std::path::Path::new(file)).ok_or("cannot read profile file")?;
            let cur = device::read_config(&d).map_err(|e| e.to_string())?;
            let mut new = cur.clone();
            let mut macros = device::macros_from_device(&cur, &[]);
            new.macros = device::macros_to_device(&mut macros, &cur.macros);
            new.profiles[n] = device::profile_to_device_with(&pr, &cur.profiles[n], &macros);
            device::write_config(&d, &new, &cur).map_err(|e| e.to_string())?;
            println!("profile {} written and verified by read-back", n + 1);
            if new.profiles[n].polling_ms != cur.profiles[n].polling_ms {
                let found = d.found.clone();
                drop(d);
                device::usb_reset(&found).map_err(|e| format!("polling needs a reconnect: {e}"))?;
                println!("mouse re-enumerated to apply the polling rate");
            }
            Ok(())
        })();
        if let Err(e) = r {
            eprintln!("{e}");
            std::process::exit(1);
        }
        return;
    }
    let lang = initial_lang(args.get_one::<String>("lang"));
    let fonts = load_fonts(args.get_flag("bundled-fonts"));
    let res = Res::new(fonts, lang);
    let dir = args.get_one::<String>("config-dir").map(PathBuf::from).unwrap_or_else(model::config_dir);

    if let Some(out) = args.get_one::<String>("screenshot") {
        // scenes start from factory defaults so they compare against the original's captures
        let tmp = std::env::temp_dir().join(format!("m801-scene-{}", std::process::id()));
        let mut state = State::new(tmp.clone(), lang);
        state.scene(args.get_one::<String>("scene").unwrap(), &res.strs);
        let mut canvas = Canvas::new(WIN_W, WIN_H);
        let inp = Input { mouse: (-1, -1), ..Default::default() };
        state.frame(&res, &mut canvas, inp);
        let img = image::RgbaImage::from_raw(WIN_W as u32, WIN_H as u32, canvas.to_rgba_bytes()).unwrap();
        img.save(out).expect("write screenshot");
        let _ = std::fs::remove_dir_all(tmp);
        return;
    }

    if args.get_flag("device-apply") {
        let d = open_cli().map_err(|e| e.to_string());
        let mut state = State::new(dir, lang);
        if let Ok(d) = d {
            state.choose(&d.found);
        }
        state.attach_device();
        match state.apply_to_device() {
            Ok(()) => println!("applied, reconnected and reopened: {}", state.device_path().unwrap_or_default()),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return;
    }
    let mut state = State::new(dir, lang);
    if !args.get_flag("no-device") {
        if let Some(f) = device::scan().iter().find(|f| Some(&f.usb_name()) == args.get_one::<String>("usb")) {
            state.choose(f);
        }
        state.enable_hotplug();
        state.attach_device();
        daemon::sync_autostart();
        daemon::ensure_running();
    }
    // one canvas pixel per physical pixel, or an integer multiple on HiDPI
    let icon = image::load_from_memory(include_bytes!("../assets/brand/icon.png")).ok().map(|i| {
        let i = i.to_rgba8();
        let (w, h) = i.dimensions();
        std::sync::Arc::new(egui::IconData { rgba: i.into_raw(), width: w, height: h })
    });
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([WIN_W as f32, WIN_H as f32])
        .with_resizable(false)
        .with_decorations(false)
        .with_transparent(true)
        .with_title("DRAGONIZE GAMING MOUSE")
        .with_app_id("dragonize");
    if let Some(icon) = icon {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    let r = eframe::run_native(
        "DRAGONIZE GAMING MOUSE",
        options,
        Box::new(move |cc| {
            let ppp = cc.egui_ctx.native_pixels_per_point().unwrap_or(1.0);
            let scale = ppp.round().max(1.0);
            // logical size such that the canvas maps to `scale` physical pixels per pixel
            cc.egui_ctx.set_pixels_per_point(ppp);
            let logical = scale / ppp;
            cc.egui_ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(WIN_W as f32 * logical, WIN_H as f32 * logical)));
            Ok(Box::new(App { res, state, canvas: Canvas::new(WIN_W, WIN_H), tex: None, last_click: None, scale: logical, mods: [false; 4] }))
        }),
    );
    if let Err(e) = r {
        eprintln!("{APP_NAME}: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifier_keys_become_key_events() {
        // Ctrl down, C down / up, Ctrl up, then Super alone: what a macro records
        let mut held = [false; 4];
        let mut out = Vec::new();
        mod_events(&mut held, [true, false, false, false], &mut out);
        mod_events(&mut held, [true, false, false, false], &mut out);
        mod_events(&mut held, [false, false, false, true], &mut out);
        let got: Vec<(&str, bool)> = out.iter().map(|k| (k.name.as_str(), k.pressed)).collect();
        assert_eq!(got, [("Ctrl", true), ("Ctrl", false), ("Win", true)]);
        for n in MOD_KEYS {
            assert!(device::key_usage(n).is_some(), "{n}");
        }
    }
}
