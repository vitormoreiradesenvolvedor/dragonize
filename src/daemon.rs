//! Background mode (`dragonize --daemon`, started by the app and at login).
//!
//! What hid.exe does on Windows while the mouse is plugged: the firmware sends
//! input report `08 01 PP` on the vendor interface when the profile button picks
//! profile PP (0-based). hid.exe then applies that profile's pointer settings
//! (pointer.rs). On firmware that always runs block 1 (`single_live_profile`)
//! the profile is also written there, the Apply path, so its DPI levels, polling
//! rate, buttons and lighting take effect. A mouse that is plugged in gets the
//! pointer settings of its active profile.
//!
//! Only profiles saved by the app are used: a model without profile files in the
//! configuration directory is left as it is.

use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use crate::device::{self, Found};
use crate::model::{self, Profile};

enum Event {
    /// Profile button: (USB port, 0-based profile)
    Profile(String, u8),
    /// The listener of this hidraw node stopped (unplugged or re-enumerated).
    Gone(PathBuf),
}

fn lock_path() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join("dragonize-daemon.lock")
}

/// Holds the single-instance lock while the returned file stays open.
fn try_lock() -> Option<File> {
    let f = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(lock_path()).ok()?;
    let r = unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    (r == 0).then_some(f)
}

/// The saved profiles of `found`'s model, when the app saved any.
fn saved_profiles(found: &Found) -> Option<(PathBuf, Vec<Profile>)> {
    let dir = found.model.profile_dir(&model::config_dir());
    let file = |i: usize| dir.join(format!("RSProfile{}.pfd", i + 1));
    if !(0..5).all(|i| file(i).exists()) {
        return None;
    }
    let profiles = (0..5).map(|i| Profile::load(&file(i)).unwrap_or_default()).collect();
    Some((dir, profiles))
}

fn log(msg: &str) {
    eprintln!("dragonize daemon: {msg}");
}

fn listen(found: Found, tx: mpsc::Sender<Event>) {
    std::thread::spawn(move || {
        let port = found.usb_name();
        if let Ok(mut f) = File::open(&found.hidraw) {
            let mut buf = [0u8; 64];
            while let Ok(n) = f.read(&mut buf) {
                if n >= 3 && buf[0] == 0x08 && buf[1] == 0x01 && buf[2] < 5 {
                    let _ = tx.send(Event::Profile(port.clone(), buf[2]));
                }
            }
        }
        let _ = tx.send(Event::Gone(found.hidraw));
    });
}

/// Pointer settings of the active profile of a mouse that just appeared.
fn connected(found: &Found) {
    crate::models::set_current(found.model);
    let Some((_, profiles)) = saved_profiles(found) else { return };
    let active = if found.model.write_only() {
        0
    } else {
        let _excl = device::exclusive();
        match device::Device::open(found).and_then(|d| device::read_config(&d)) {
            Ok(c) => c.active.min(4),
            Err(e) => {
                log(&format!("{}: {e}", found.model.name));
                return;
            }
        }
    };
    log(&format!("{} on {}: profile {}, {}", found.model.name, found.usb_name(), active + 1, crate::pointer::apply(found, &profiles[active as usize])));
}

fn profile_button(found: &Found, profile: u8) {
    crate::models::set_current(found.model);
    let Some((dir, profiles)) = saved_profiles(found) else {
        log(&format!("{}: profile {} chosen, no saved profiles", found.model.name, profile + 1));
        return;
    };
    if found.model.single_live_profile() {
        let mut macros = model::load_macros(&dir);
        match device::apply_profiles(Some(&found.usb_name()), &profiles, &mut macros, profile, true, &|_| {}) {
            Ok(_) => {
                let _ = std::fs::write(device::applied_mark(), found.usb_name());
                log(&format!("{}: profile {} written to the live block", found.model.name, profile + 1));
            }
            Err(e) => log(&format!("{}: profile {}: {e}", found.model.name, profile + 1)),
        }
    }
    log(&format!("{}: profile {}, {}", found.model.name, profile + 1, crate::pointer::apply(found, &profiles[profile as usize])));
}

/// Modification time of the mark the window watches.
pub fn applied_at() -> Option<std::time::SystemTime> {
    std::fs::metadata(device::applied_mark()).and_then(|m| m.modified()).ok()
}

pub fn run() {
    let Some(_lock) = try_lock() else {
        log("already running");
        return;
    };
    log("started");
    let (tx, rx) = mpsc::channel();
    // hidraw nodes already handled: every mouse once per (re)connection
    let mut seen: HashSet<PathBuf> = HashSet::new();
    loop {
        let all = device::scan();
        seen.retain(|p| all.iter().any(|f| &f.hidraw == p));
        let found: Vec<Found> = all.into_iter().filter(|f| !f.model.is_kbd() && !seen.contains(&f.hidraw)).collect();
        // without access (udev rule not installed yet) try again later
        if !found.is_empty() && !matches!(device::access(), device::Access::NoPermission(_)) {
            for f in found {
                seen.insert(f.hidraw.clone());
                connected(&f);
                if f.model.is_holtek() {
                    listen(f, tx.clone());
                }
            }
        }
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(Event::Gone(p)) => {
                seen.remove(&p);
            }
            Ok(Event::Profile(port, mut profile)) => {
                // several presses in a row: the last one counts
                while let Ok(e) = rx.recv_timeout(Duration::from_millis(400)) {
                    match e {
                        Event::Profile(p, n) if p == port => profile = n,
                        Event::Profile(..) => {}
                        Event::Gone(p) => {
                            seen.remove(&p);
                        }
                    }
                }
                if let Some(f) = device::scan().into_iter().find(|f| f.usb_name() == port && !f.model.is_kbd()) {
                    profile_button(&f, profile);
                }
            }
            Err(_) => {}
        }
    }
}

/// One line for `--diagnose`.
pub fn status() -> String {
    let running = if try_lock().is_none() { "running" } else { "not running" };
    let login = if autostart_file().exists() { "on" } else { "off" };
    format!("background mode: {running}, start at login: {login} (log: {})", log_path().display())
}

fn log_path() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join("dragonize-daemon.log")
}

/// Starts the background mode unless it runs already (detached: it outlives the window).
pub fn ensure_running() {
    if try_lock().is_none() {
        return;
    }
    // the lock was free; it is released here so the child can take it
    let exe = exe_path();
    let err = File::create(log_path()).map(std::process::Stdio::from).unwrap_or_else(|_| std::process::Stdio::null());
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("--daemon").stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(err);
    use std::os::unix::process::CommandExt;
    // own session: closing the window or its terminal does not stop it
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    let _ = cmd.spawn();
}

/// The program to start again: the AppImage file itself (its mount goes away with
/// the window), else this executable.
fn exe_path() -> PathBuf {
    std::env::var_os("APPIMAGE").map(PathBuf::from).or_else(|| std::env::current_exe().ok()).unwrap_or_else(|| PathBuf::from("dragonize"))
}

fn autostart_file() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config"));
    base.join("autostart").join("dragonize-daemon.desktop")
}

/// Marker in the configuration directory: the user turned the login start off.
fn autostart_off_marker() -> PathBuf {
    model::config_dir().join("autostart-off")
}

fn desktop_entry(exe: &Path) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName=Dragonize background mode\n\
         Comment=Applies the mouse profile chosen with the profile button\n\
         Exec=\"{}\" --daemon\nIcon=dragonize\nTerminal=false\nNoDisplay=true\nX-GNOME-Autostart-enabled=true\n",
        exe.display()
    )
}

/// Keeps the login entry pointing at this program (unless turned off).
pub fn sync_autostart() {
    if autostart_off_marker().exists() {
        return;
    }
    let _ = set_autostart(true);
}

pub fn set_autostart(on: bool) -> std::io::Result<()> {
    let file = autostart_file();
    if on {
        let _ = std::fs::remove_file(autostart_off_marker());
        let entry = desktop_entry(&exe_path());
        if std::fs::read_to_string(&file).ok().as_deref() != Some(entry.as_str()) {
            std::fs::create_dir_all(file.parent().unwrap())?;
            std::fs::write(&file, entry)?;
        }
        return Ok(());
    }
    std::fs::create_dir_all(model::config_dir())?;
    std::fs::write(autostart_off_marker(), "")?;
    match std::fs::remove_file(&file) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_entry_quotes_the_path() {
        let e = desktop_entry(Path::new("/home/u/My Apps/Dragonize-x86_64.AppImage"));
        assert!(e.contains("Exec=\"/home/u/My Apps/Dragonize-x86_64.AppImage\" --daemon\n"));
    }
}
