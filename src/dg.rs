//! The 2.4 GHz dongle of the Sinowealth keyboards (OemDrv.exe `CDev3632` / `CDevCombo`,
//! decompile of the K686AK-RGB-PRO build de7646c7; tools/re/fleet/NOTES.md "the dongle").
//!
//! On the cable the keyboard answers the feature reports of `kbd.rs`. On its dongle it is
//! another usb id (`KB.ini [OPT] VID_Wireless / PID_Wireless`, `models_wl.rs`) and another
//! protocol: `CDev3632::FindHIDDevice` (FUN_004853e0) opens no interface of its own and
//! everything goes through 20-byte interrupt reports on the dongle's own interface.
//!
//! Which interface (FUN_0040c140, the table FUN_00415b90 fills from HIDP_CAPS): usage page
//! **0xff02**, input and output reports of **20 bytes**, and the report id says which dongle
//! it is: **0x13** = the single-device dongle (CDev3632), **0x15** = the combo dongle that
//! carries a keyboard and a mouse (CDevCombo, tag 0x5588). Usage 2 and a 0x208-byte feature
//! report is the third kind (tag 0x8805), which is not this protocol.
//!
//! Every packet, in both directions, is
//!
//! ```text
//!   0    report id (0x13 or 0x15)
//!   1    command
//!   2    number of packets, bit 7 = the device refused this one (checksum)
//!   3    index of this packet, 0 based
//!   4    high nibble: the command's argument (the layer, for instance)
//!        low nibble:  bytes of payload in this packet, at most 14
//!   5..18 payload
//!   19   checksum: the low byte of the sum of bytes 0..18
//! ```
//!
//! A read (`SendCMD` FUN_004856e0) sends one packet with no payload and collects the answers
//! until `index + 1 == packets`. Commands (`Cmd`): a read is its write plus 0x40. The combo
//! dongle answers its own small set on report 0x15: 5 and 0x15 are the password of the keyboard
//! and of the mouse (`OnNewCombo` FUN_0040c5c0, the same six bytes `kbd::psd` reads on the
//! cable, so the same registry entry names the product), 7 is which of the two is online.
//!
//! Writing is `AccessData` (FUN_00485440): the same packets with the payload 14 bytes at a time,
//! each answered by the device echoing the command, except for the commands with bit 7 set
//! (0x88, 0x8e), which are live effects and are not answered. It is not here, because what a
//! block holds is **not** decoded: the dongle serialises the settings the way
//! `CDevG5KB::ApplySetting` (FUN_0048cb20) does, a 0x80-byte configuration block, a key matrix
//! per layer and a colour table, which is not the layout `kbd.rs` reads on the cable. `dump`
//! reads them as they are, so they can be decoded against a keyboard whose settings are known,
//! and until then a device on its dongle is named but not configured.
use crate::device::Device;

/// Report id of the dongle's packets, which is also what kind of dongle it is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dongle {
    /// CDev3632: the keyboard has the dongle to itself
    One = 0x13,
    /// CDevCombo (tag 0x5588): a keyboard and a mouse share it
    Combo = 0x15,
}

/// The commands of the dongle. A read is the write plus 0x40, which is how the driver names
/// them (CDev3632::SetMatrix 1 / GetMatrix 0x41, and so on).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cmd {
    /// key matrix of one layer, 4 bytes per key (the argument is the layer)
    Matrix = 1,
    /// the "game" block
    Game = 2,
    Macro = 3,
    /// the 0x80-byte configuration block, which is what CDevG5KB calls Cfg
    Cfg = 4,
    /// the colour table, 3 bytes per entry
    RgbTab = 9,
    /// whether the keyboard keeps its settings without the driver
    OnBoard = 0x0a,
}

/// Is this interface a dongle of this protocol, and which (FUN_0040c140: usage page 0xff02,
/// input and output reports of 20 bytes, the report id telling the two dongles apart)?
pub fn kind(d: &crate::hid::Desc) -> Option<Dongle> {
    if !d.page(0xff02) {
        return None;
    }
    [Dongle::One, Dongle::Combo].into_iter().find(|k| {
        d.len_of(*k as u8, crate::hid::Kind::Out) == Some(PACKET) && d.len_of(*k as u8, crate::hid::Kind::In) == Some(PACKET)
    })
}

pub const PACKET: usize = 20;
const PAYLOAD: usize = 14;

/// The byte the device checks: the low byte of the sum of the other nineteen
/// (FUN_00485d10, and the same sum inlined in AccessData).
fn sum(p: &[u8]) -> u8 {
    p[..PACKET - 1].iter().fold(0u8, |a, &b| a.wrapping_add(b))
}

fn packet(kind: Dongle, cmd: u8, packets: u8, index: u8, arg: u8, data: &[u8]) -> [u8; PACKET] {
    let mut p = [0u8; PACKET];
    p[0] = kind as u8;
    p[1] = cmd;
    p[2] = packets;
    p[3] = index;
    p[4] = arg << 4 | data.len() as u8;
    p[5..5 + data.len()].copy_from_slice(data);
    p[PACKET - 1] = sum(&p);
    p
}

/// The next packet of this command, skipping whatever else the dongle is sending
/// (FUN_00488c50: at most 20 reports, and the device says so when it refused ours).
fn answer(d: &Device, cmd: u8, ms: i32) -> std::io::Result<[u8; PACKET]> {
    for _ in 0..20 {
        let Some(r) = d.read_report(ms)? else {
            return Err(std::io::Error::other(format!("the dongle did not answer command {cmd:#04x}")));
        };
        if r.len() < PACKET || r[1] != cmd {
            continue;
        }
        if r[2] & 0x80 != 0 {
            return Err(std::io::Error::other(format!("the dongle refused command {cmd:#04x} (checksum)")));
        }
        let mut p = [0u8; PACKET];
        p.copy_from_slice(&r[..PACKET]);
        return Ok(p);
    }
    Err(std::io::Error::other(format!("no answer to command {cmd:#04x} in twenty reports")))
}

/// Reads `n` bytes of a block (SendCMD FUN_004856e0): one empty packet out, then its answers
/// until the device says the last one is in.
pub fn read(d: &Device, kind: Dongle, cmd: u8, arg: u8, n: usize) -> std::io::Result<Vec<u8>> {
    while d.read_report(0)?.is_some() {}
    d.write_report(&packet(kind, cmd, 1, 0, arg, &[]))?;
    let mut out = Vec::with_capacity(n);
    loop {
        let r = answer(d, cmd, 500)?;
        let len = (r[4] & 0xf) as usize;
        out.extend_from_slice(&r[5..5 + len.min(PAYLOAD)]);
        if out.len() >= n || r[3] as usize + 1 >= (r[2] & 0x7f) as usize {
            break;
        }
    }
    out.truncate(n);
    if out.len() < n {
        return Err(std::io::Error::other(format!("the dongle sent {} of the {n} bytes of command {cmd:#04x}", out.len())));
    }
    Ok(out)
}

pub fn read_block(d: &Device, kind: Dongle, c: Cmd, arg: u8, n: usize) -> std::io::Result<Vec<u8>> {
    read(d, kind, c as u8 | 0x40, arg, n)
}

/// Which of the two devices of a combo dongle are online: keyboard, mouse (command 7, the
/// `bOnline[2]` of FUN_0040c1f0). The single-device dongle does not answer it.
pub fn online(d: &Device) -> Option<(bool, bool)> {
    let r = read(d, Dongle::Combo, 7, 0, 2).ok()?;
    Some((r[0] != 0, r[1] != 0))
}

/// The six-byte password of the keyboard (command 5) or of the mouse (command 0x15) the dongle
/// carries, which is the one `kbd::psd` reads on the cable, so the same registry entry names
/// the product (OnNewCombo FUN_0040c5c0).
pub fn psd(d: &Device, kind: Dongle, mouse: bool) -> Option<[u8; 6]> {
    let r = read(d, kind, if mouse { 0x15 } else { 5 }, 0, 6).ok()?;
    let mut p = [0u8; 6];
    p.copy_from_slice(&r);
    (p != [0; 6]).then_some(p)
}

/// Every block the dongle will part with, as it sends them. The layout inside is not decoded,
/// so this is what `--dongle-dump` prints for a keyboard whose settings are known.
pub fn dump(d: &Device, kind: Dongle) -> String {
    use std::fmt::Write;
    let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" ");
    let mut o = String::new();
    let _ = writeln!(o, "dongle: report id {:#04x}", kind as u8);
    if let Some((kb, ms)) = online(d) {
        let _ = writeln!(o, "online: keyboard {kb}, mouse {ms}");
    }
    for (name, mouse) in [("keyboard", false), ("mouse", true)] {
        match psd(d, kind, mouse) {
            Some(p) => {
                let _ = writeln!(o, "password of the {name}: {}", hex(&p));
            }
            None => {
                let _ = writeln!(o, "password of the {name}: no answer");
            }
        }
    }
    for (c, arg, n) in [
        (Cmd::Cfg, 0, 0x80),
        (Cmd::OnBoard, 0, 1),
        (Cmd::Game, 0, 0x40),
        (Cmd::RgbTab, 0, 0x40),
        (Cmd::Macro, 0, 0x40),
        (Cmd::Matrix, 0, 0x80),
        (Cmd::Matrix, 1, 0x80),
    ] {
        let _ = match read_block(d, kind, c, arg, n) {
            Ok(b) => writeln!(o, "{c:?}[{arg}] {n} bytes: {}", hex(&b)),
            Err(e) => writeln!(o, "{c:?}[{arg}]: {e}"),
        };
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The packet the driver sends to read the configuration block: report id, command, one
    /// packet, index 0, no payload, and the sum of the other nineteen bytes last.
    #[test]
    fn a_read_packet_carries_its_checksum() {
        let p = packet(Dongle::One, Cmd::Cfg as u8 | 0x40, 1, 0, 0, &[]);
        assert_eq!(p[..5], [0x13, 0x44, 1, 0, 0]);
        assert_eq!(p[19], 0x13u8.wrapping_add(0x44).wrapping_add(1));
        assert_eq!(p[19], sum(&p));
    }

    /// The interface the driver keeps: usage page 0xff02 with 20-byte reports, the report id
    /// telling the single-device dongle from the combo one.
    #[test]
    fn the_dongle_interface_is_recognised() {
        let desc = |id: u8| {
            let rd = [
                0x06, 0x02, 0xff, 0x09, 0x02, 0xa1, 0x01, // usage page 0xff02, usage 2, collection
                0x85, id, 0x75, 0x08, 0x95, 0x13, 0x09, 0x01, 0x81, 0x02, // input, 19 bytes
                0x95, 0x13, 0x09, 0x01, 0x91, 0x02, // output, 19 bytes
                0xc0,
            ];
            crate::hid::Desc::of(&rd)
        };
        assert_eq!(kind(&desc(0x13)), Some(Dongle::One));
        assert_eq!(kind(&desc(0x15)), Some(Dongle::Combo));
        // another report id on that page is one of the dongles this protocol does not drive
        assert_eq!(kind(&desc(0x08)), None);
        // the keyboard's own interface on the cable is not a dongle
        let wired = crate::hid::Desc::of(&[0x06, 0x00, 0xff, 0x09, 0x01, 0xa1, 0x01, 0x85, 0x05, 0x75, 0x08, 0x95, 0x05, 0x09, 0x01, 0xb1, 0x02, 0xc0]);
        assert_eq!(kind(&wired), None);
    }

    /// A packet carries the argument in the high nibble of byte 4 and the length in the low one,
    /// and holds at most fourteen bytes.
    #[test]
    fn a_packet_holds_fourteen_bytes() {
        let data: Vec<u8> = (0..14).collect();
        let p = packet(Dongle::Combo, Cmd::Matrix as u8, 2, 1, 3, &data);
        assert_eq!(p[..5], [0x15, 1, 2, 1, 0x3e]);
        assert_eq!(p[5..19], data[..]);
        assert_eq!(p[19], sum(&p));
        assert_eq!(30usize.div_ceil(PAYLOAD), 3);
    }
}
