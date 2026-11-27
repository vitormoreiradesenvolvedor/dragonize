//! What a HID report descriptor declares, which is how the drivers pick the interface to talk
//! to. They all enumerate the HID devices of the usb id and keep the one whose `HIDP_CAPS`
//! match: the usage page and usage of its top-level collection and the length of one of its
//! reports (OemDrv FUN_00415b90 fills the table, CDev916KB::FindHIDDevice FUN_00486600 reads
//! it: usage page 0xff00 usage 1, feature report of 0x408 bytes = the data interface, 6 = the
//! command interface, 0x17e = the "music" one).
//!
//! A product exposes several of those collections, sometimes on one usb interface and
//! sometimes on more than one, and Linux gives each interface its own hidraw node carrying
//! every report of that interface. Looking for the byte pair `85 05` in the raw descriptor,
//! as this did before, answers neither question: the pair also occurs inside the data of
//! other items, so a plain keyboard interface can pass for a command interface.

/// Which direction a report is declared in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    In,
    Out,
    Feat,
}

#[derive(Debug, Default)]
pub struct Desc {
    /// usage page and usage of every top-level collection
    pub tops: Vec<(u16, u16)>,
    /// every usage page and every usage the descriptor mentions, at any depth
    pub pages: Vec<u16>,
    pub usages: Vec<u16>,
    /// every report declared, with its length in bytes including the report id
    pub reports: Vec<(u8, Kind, usize)>,
}

/// Reads the items of a report descriptor (HID 1.11 6.2.2).
pub fn parse(rd: &[u8]) -> Desc {
    let mut d = Desc::default();
    let mut bits: Vec<(u8, Kind, u32)> = Vec::new();
    let (mut page, mut size, mut count, mut id) = (0u16, 0u32, 0u32, 0u8);
    let mut saved: Vec<(u16, u32, u32, u8)> = Vec::new();
    // the first usage of the item group, which is the collection's own when one opens
    let mut usage = 0u16;
    let mut depth = 0usize;
    let mut i = 0usize;
    while i < rd.len() {
        let p = rd[i];
        if p == 0xfe {
            // long item: one size byte, one tag byte, then the data
            i += 3 + *rd.get(i + 1).unwrap_or(&0) as usize;
            continue;
        }
        let n = match p & 3 {
            3 => 4,
            x => x as usize,
        };
        if i + 1 + n > rd.len() {
            break;
        }
        let v = rd[i + 1..i + 1 + n].iter().rev().fold(0u32, |a, &b| a << 8 | b as u32);
        match p & 0xfc {
            0x04 => {
                page = v as u16;
                if !d.pages.contains(&page) {
                    d.pages.push(page);
                }
            }
            0x08 => {
                usage = v as u16;
                if !d.usages.contains(&usage) {
                    d.usages.push(usage);
                }
            }
            0x74 => size = v,
            0x84 => id = v as u8,
            0x94 => count = v,
            0xa4 => saved.push((page, size, count, id)),
            0xb4 => {
                if let Some(s) = saved.pop() {
                    (page, size, count, id) = s;
                }
            }
            0xa0 => {
                if depth == 0 {
                    d.tops.push((page, usage));
                }
                depth += 1;
                usage = 0;
            }
            0xc0 => {
                depth = depth.saturating_sub(1);
                usage = 0;
            }
            k @ (0x80 | 0x90 | 0xb0) => {
                let kind = match k {
                    0x80 => Kind::In,
                    0x90 => Kind::Out,
                    _ => Kind::Feat,
                };
                match bits.iter_mut().find(|e| e.0 == id && e.1 == kind) {
                    Some(e) => e.2 += size * count,
                    None => bits.push((id, kind, size * count)),
                }
                usage = 0;
            }
            _ => {}
        }
        i += 1 + n;
    }
    // the id byte counts towards the length the drivers compare, as HIDP_CAPS does
    d.reports = bits.into_iter().map(|(id, k, b)| (id, k, 1 + (b as usize).div_ceil(8))).collect();
    d
}

impl Desc {
    pub fn of(rd: &[u8]) -> Desc {
        parse(rd)
    }

    /// Length of that report in bytes, the report id included.
    pub fn len_of(&self, id: u8, k: Kind) -> Option<usize> {
        self.reports.iter().find(|r| r.0 == id && r.1 == k).map(|r| r.2)
    }

    /// Is that report declared, in any direction?
    pub fn has(&self, id: u8) -> bool {
        self.reports.iter().any(|r| r.0 == id)
    }

    pub fn has_kind(&self, id: u8, k: Kind) -> bool {
        self.len_of(id, k).is_some()
    }

    /// Is this usage page named anywhere in the descriptor, at any depth? The drivers compare
    /// the top-level collection's page, but which products put the configuration reports in a
    /// top-level collection of their own has not been checked device by device, so the loose
    /// question is the one asked of the interfaces whose choice already works.
    pub fn page(&self, page: u16) -> bool {
        self.pages.contains(&page)
    }

    /// This usage page and this usage, each named anywhere in the descriptor.
    pub fn decl(&self, page: u16, usage: u16) -> bool {
        self.page(page) && self.usages.contains(&usage)
    }

    /// One line for `--diagnose`: what this interface declares.
    pub fn summary(&self) -> String {
        let tops: Vec<String> = self.tops.iter().map(|(p, u)| format!("{p:04x}/{u:02x}")).collect();
        let mut r: Vec<String> = self
            .reports
            .iter()
            .map(|(id, k, n)| {
                let k = match k {
                    Kind::In => "in",
                    Kind::Out => "out",
                    Kind::Feat => "feature",
                };
                format!("{k} {id} = {n}")
            })
            .collect();
        r.sort();
        format!("usage {}; reports {}", tops.join(" "), if r.is_empty() { "none".into() } else { r.join(", ") })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A boot keyboard holds the byte pair `85 05` (report count 133, then usage page 1) without
    /// declaring a report 5, which is what the old byte search tripped over.
    #[test]
    fn a_byte_pair_is_not_a_report_id() {
        let rd = [
            0x05, 0x01, 0x09, 0x06, 0xa1, 0x01, // usage page 1, keyboard, collection
            0x85, 0x01, // report id 1
            0x75, 0x01, 0x95, 0x85, 0x05, 0x01, 0x81, 0x02, // report size 1, count 133, page 1, input
            0xc0,
        ];
        let d = parse(&rd);
        assert!(!d.has(5));
        assert!(d.has(1));
        assert_eq!(d.len_of(1, Kind::In), Some(1 + 133 / 8 + 1));
        assert_eq!(d.tops, vec![(0x0001, 0x0006)]);
    }

    /// The command and data collections of a Sinowealth keyboard, as CDev916KB looks for them:
    /// usage page 0xff00 usage 1, feature report 5 of 6 bytes and feature report 6 of 0x408.
    #[test]
    fn the_keyboard_command_interface_is_recognised() {
        let rd = [
            0x06, 0x00, 0xff, 0x09, 0x01, 0xa1, 0x01, // usage page 0xff00, usage 1, collection
            0x85, 0x05, 0x75, 0x08, 0x95, 0x05, 0x09, 0x01, 0xb1, 0x02, // feature 5, 5 bytes
            0xc0, //
            0x06, 0x00, 0xff, 0x09, 0x01, 0xa1, 0x01, //
            0x85, 0x06, 0x75, 0x08, 0x96, 0x07, 0x04, 0x09, 0x01, 0xb1, 0x02, // feature 6, 0x407
            0xc0,
        ];
        let d = parse(&rd);
        assert_eq!(d.len_of(5, Kind::Feat), Some(6));
        assert_eq!(d.len_of(6, Kind::Feat), Some(0x408));
        assert!(d.tops.contains(&(0xff00, 1)));
        assert!(!d.tops.contains(&(0xff00, 2)));
        assert!(d.page(0xff00));
        assert!(d.decl(0xff00, 1));
        assert!(!d.decl(0xff01, 1));
        assert_eq!(d.tops.len(), 2);
    }

    /// Push and Pop restore the report id, so what follows must not land on the wrong report.
    #[test]
    fn push_and_pop_restore_the_globals() {
        let rd = [
            0x06, 0xa0, 0xff, 0x09, 0x01, 0xa1, 0x01, //
            0x85, 0x04, 0x75, 0x08, 0x95, 0x01, 0xb1, 0x02, // feature 4, 1 byte
            0xa4, // push
            0x85, 0x09, 0x75, 0x08, 0x95, 0x02, 0xb1, 0x02, // feature 9, 2 bytes
            0xb4, // pop
            0x95, 0x03, 0xb1, 0x02, // 3 more bytes of feature 4
            0xc0,
        ];
        let d = parse(&rd);
        assert_eq!(d.len_of(4, Kind::Feat), Some(5));
        assert_eq!(d.len_of(9, Kind::Feat), Some(3));
    }
}
