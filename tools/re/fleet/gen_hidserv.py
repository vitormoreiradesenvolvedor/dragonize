#!/usr/bin/env python3
"""HidServ keyboards (InstallShield packages with "<model>.exe" + HidServ.dll, Sonix
0c45:5004 / 5204 / 5304) -> keyboard entries in gen_kbd.py's format (proto "hs"), for
gen_models.py --kbd. Usage: gen_hidserv.py OUT.json PHOTO_DIR

Per package (isx.x/cab): DefaultComponent/<model>.exe (id table `vid pid 00 00 ff ff`, default
key table = 0x7e entries of 3 bytes, recognised by Esc `02 02 29` 8 entries in, preceded by
size / per-profile stride / count), DefaultComponent/Lang0409.ini [MODE] (lighting mode code =
name), Files1/Main.ini (UTF-16; [BUTTONINFO] KEY_<set-1 scan code, 0x80 = extended> = x, y, w, h
on Files1/main_normal.jpg). A table entry is matched to its key through the scan code the
driver derives from it (FUN_00443400). Protocol: kbd.rs `hs_*`, NOTES.md "HidServ keyboards".
"""
import hashlib
import json
import pathlib
import re
import struct
import sys

from PIL import Image

R = pathlib.Path.home() / '.cache/redragon-all/x'
ENTRIES = 0x7e

# set-1 scan code (0x100 = E0 prefix) -> HID usage
SET1 = {
    0x01: 0x29, 0x02: 0x1e, 0x03: 0x1f, 0x04: 0x20, 0x05: 0x21, 0x06: 0x22, 0x07: 0x23, 0x08: 0x24, 0x09: 0x25,
    0x0a: 0x26, 0x0b: 0x27, 0x0c: 0x2d, 0x0d: 0x2e, 0x0e: 0x2a, 0x0f: 0x2b, 0x10: 0x14, 0x11: 0x1a, 0x12: 0x08,
    0x13: 0x15, 0x14: 0x17, 0x15: 0x1c, 0x16: 0x18, 0x17: 0x0c, 0x18: 0x12, 0x19: 0x13, 0x1a: 0x2f, 0x1b: 0x30,
    0x1c: 0x28, 0x1d: 0xe0, 0x1e: 0x04, 0x1f: 0x16, 0x20: 0x07, 0x21: 0x09, 0x22: 0x0a, 0x23: 0x0b, 0x24: 0x0d,
    0x25: 0x0e, 0x26: 0x0f, 0x27: 0x33, 0x28: 0x34, 0x29: 0x35, 0x2a: 0xe1, 0x2b: 0x31, 0x2c: 0x1d, 0x2d: 0x1b,
    0x2e: 0x06, 0x2f: 0x19, 0x30: 0x05, 0x31: 0x11, 0x32: 0x10, 0x33: 0x36, 0x34: 0x37, 0x35: 0x38, 0x36: 0xe5,
    0x37: 0x55, 0x38: 0xe2, 0x39: 0x2c, 0x3a: 0x39, 0x3b: 0x3a, 0x3c: 0x3b, 0x3d: 0x3c, 0x3e: 0x3d, 0x3f: 0x3e,
    0x40: 0x3f, 0x41: 0x40, 0x42: 0x41, 0x43: 0x42, 0x44: 0x43, 0x45: 0x53, 0x46: 0x47, 0x47: 0x5f, 0x48: 0x60,
    0x49: 0x61, 0x4a: 0x56, 0x4b: 0x5c, 0x4c: 0x5d, 0x4d: 0x5e, 0x4e: 0x57, 0x4f: 0x59, 0x50: 0x5a, 0x51: 0x5b,
    0x52: 0x62, 0x53: 0x63, 0x56: 0x64, 0x57: 0x44, 0x58: 0x45, 0x70: 0x88, 0x73: 0x87, 0x79: 0x8a, 0x7b: 0x8b,
    0x7d: 0x89, 0x7e: 0x85,
    0x11c: 0x58, 0x11d: 0xe4, 0x135: 0x54, 0x137: 0x46, 0x138: 0xe6, 0x145: 0x48, 0x147: 0x4a, 0x148: 0x52,
    0x149: 0x4b, 0x14b: 0x50, 0x14d: 0x4f, 0x14f: 0x4d, 0x150: 0x51, 0x151: 0x4e, 0x152: 0x49, 0x153: 0x4c,
    0x15b: 0xe3, 0x15c: 0xe7, 0x15d: 0x65,
}
USAGE_SC = {u: s for s, u in SET1.items()}
# Pause/Break is E1 1D 45 (the driver's 0x45 "Pause"), Num Lock 0x145
USAGE_SC[0x48] = 0x45
USAGE_SC[0x53] = 0x145
MOD_SC = {1: 0x1d, 2: 0x2a, 4: 0x38, 8: 0x15b, 0x10: 0x11d, 0x20: 0x36, 0x40: 0x138, 0x80: 0x15c}

# lighting mode code -> controls the driver shows (K580 FUN_0041e010): brightness everywhere,
# speed, colour buttons, "colourful" (+4 = 1)
SPEED = {1, 2, 3, 4, 5, 7, 8, 9, 10, 11, 12, 13, 14, 15}
COLOUR = {1, 2, 3, 5, 6, 7, 8, 9, 10, 13, 14, 15, 16}
COLOURFUL = {1, 2, 3, 5, 7, 8, 9, 13, 14, 15}
SKIP_MODES = {20}  # per-key colour mode (own colour table, not decoded)


def ini(path):
    raw = path.read_bytes()
    return raw.decode('utf-16' if raw[:2] in (b'\xff\xfe', b'\xfe\xff') else 'gbk', 'replace')


def section(text, name):
    m = re.search(rf'^\[{name}\]\s*$(.*?)(?=^\[|\Z)', text, re.M | re.S)
    return m.group(1) if m else ''


def pe_va(b, off):
    pe = struct.unpack_from('<I', b, 0x3c)[0]
    n, opt = struct.unpack_from('<H', b, pe + 6)[0], struct.unpack_from('<H', b, pe + 20)[0]
    base = struct.unpack_from('<I', b, pe + 52)[0]
    o = pe + 24 + opt
    for i in range(n):
        va, rs, raw = struct.unpack_from('<III', b, o + 40 * i + 12)
        if raw <= off < raw + rs:
            return base + va + off - raw
    return None


def ids(b):
    out = []
    for m in re.finditer(rb'(?=(..)\x00\x00(..)\x00\x00\x00\x00\xff\xff)', b, re.S):
        v, p = int.from_bytes(m.group(1), 'little'), int.from_bytes(m.group(2), 'little')
        if v in (0x0c45, 0x04b4) and (v, p) not in out:
            out.append((v, p))
    return out


def table(b):
    """-> (entries [(b0, b1, b2)], per-profile stride). Most builds keep only the first 52
    entries statically and take the rest from the keyboard (cmd 07, FUN_0043fb10); every full
    table found agrees with K580's, so the gaps are filled from the fullest one (`FULL`)."""
    hits = [m.start() for m in re.finditer(rb'\x02\x02\x29\x02\x02', b)]
    if not hits:
        return None, None
    t = hits[-1] - 24
    tab = [tuple(b[t + 3 * i:t + 3 * i + 3]) for i in range(ENTRIES)]
    if FULL:
        tab = [e if any(e) else f for e, f in zip(tab, FULL)]
    size, stride, count = struct.unpack_from('<III', b, t - 16)
    # static header (K580 7a01 7a01 7e; K550 7a01 0002 7e); zeros = set in code (0x17a)
    return tab, stride if (size, count) == (0x17a, 0x7e) and stride in (0x17a, 0x200) else 0x17a


def entry_sc(e):
    a, c, v = e
    if a == 2 and c == 2:
        return USAGE_SC.get(v)
    if a == 2 and c == 1:
        return MOD_SC.get(v)
    return None


def code(e):
    return e[0] | e[1] << 8 | e[2] << 16


def package(d):
    exe = next((e for e in sorted((d / 'DefaultComponent').glob('*.exe')) if 'unins' not in e.name.lower()), None)
    main, lang = d / 'Files1' / 'Main.ini', d / 'DefaultComponent' / 'Lang0409.ini'
    photo = d / 'Files1' / 'main_normal.jpg'
    if not exe or not main.exists() or not photo.exists():
        return None, 'missing files'
    b = exe.read_bytes()
    idl = ids(b)
    tab, stride = table(b)
    if not idl or not tab:
        return None, 'no id / key table'
    by_sc = {}
    for i, e in enumerate(tab):
        sc = entry_sc(e)
        if sc is not None and sc not in by_sc:
            by_sc[sc] = i
    keys = []
    for m in re.finditer(r'^\s*KEY_([0-9A-Fa-f]{3})\s*=\s*(-?\d+)\s*,\s*(-?\d+)\s*,\s*(\d+)\s*,\s*(\d+)', section(ini(main), 'BUTTONINFO'), re.M):
        k = int(m.group(1), 16)
        sc = (k & 0x7f) | (0x100 if k & 0x80 else 0)
        i = by_sc.get(sc)
        if i is None:
            continue
        x, y, w, h = (int(m.group(j)) for j in range(2, 6))
        keys.append(dict(x=x, y=y, w=w, h=h, matrix=i, led=None, code=code(tab[i]), kind=0, vk=0))
    if not keys:
        return None, 'no keys matched'
    modes = []
    # [MODE] of Lang0409.ini, or (newer builds, "Mechanical Gaming Keyboard.exe") [Mode] of Main.ini
    sec = section(ini(lang), 'MODE') if lang.exists() else ''
    for m in re.finditer(r'^\s*(\d+)\s*=(.*)$', sec or section(ini(main), 'Mode'), re.M):
            n = int(m.group(1))
            if 0 < n < 0x20 and n not in SKIP_MODES:
                modes.append((n, m.group(2).replace('　', ' ').strip()))
    leds = [dict(n=i + 1, hw=n, effect=0, light=True, speed=n in SPEED, color=n in COLOUR, random=n in COLOURFUL,
                 rgb=False, direct=False, name=nm) for i, (n, nm) in enumerate(modes)]
    return dict(exe=exe, ids=idl, stride=stride, keys=keys, leds=leds, photo=photo), None


def crop(photo, keys, out):
    """Keyboard area of the skin (bounding box of the keys + margin); keys shifted into it."""
    im = Image.open(photo).convert('RGB')
    m = 14
    x0 = max(0, min(k['x'] for k in keys) - m)
    y0 = max(0, min(k['y'] for k in keys) - m)
    x1 = min(im.width, max(k['x'] + k['w'] for k in keys) + m)
    y1 = min(im.height, max(k['y'] + k['h'] for k in keys) + m)
    im.crop((x0, y0, x1, y1)).save(out)
    for k in keys:
        k['x'] -= x0
        k['y'] -= y0


def model_name(pkg):
    m = re.search(r'K\d{3}[A-Z0-9]*(?:[- ](?:RGB|PRO|SE|KR|RK|N|TKL|BLUE|1|2)\b)*', pkg.upper())
    return re.sub(r'\s+', '-', m.group(0)) if m else pkg


FULL = None


def main():
    global FULL
    full = [(sum(map(any, t)), t) for t in (table(d.read_bytes())[0] for d in R.rglob('*.exe') if (d.parent / 'HidServ.dll').exists())
            if t]
    FULL = max(full)[1]
    out, photos = sys.argv[1], pathlib.Path(sys.argv[2])
    photos.mkdir(parents=True, exist_ok=True)
    groups = {}
    for dll in sorted(R.rglob('HidServ.dll')):
        d = dll.parent.parent
        pkg = dll.relative_to(R).parts[1]
        e, why = package(d)
        if e is None:
            print('skip', pkg, why, file=sys.stderr)
            continue
        sig = hashlib.sha1(e['exe'].read_bytes() + (d / 'Files1' / 'Main.ini').read_bytes()
                           + (d / 'Files1' / 'main_normal.jpg').read_bytes()).hexdigest()
        groups.setdefault(sig, (e, d, []))[2].append(pkg)
    res = []
    for sig, (e, d, pkgs) in groups.items():
        names = list(dict.fromkeys(model_name(p) for p in pkgs))[:2]  # the header has room for two
        name = ' / '.join(names)
        photo = photos / f'hs-{sig[:10]}.png'
        crop(e['photo'], e['keys'], photo)
        (vid, pid), alt = e['ids'][0], [p for _, p in e['ids'][1:]]
        res.append(dict(name=name, psd=[0] * 6, family='hidserv', build='', vid=vid, pid=pid, alt_pids=alt,
                        glen=8 + 0x16 + 2 * len(e['leds']), pro=0, key=0, rgb=0, rgb_read=False, led_off=4,
                        keys=e['keys'], leds=e['leds'], light=[0, 1, 2, 3, 4, 5], speed=[5, 4, 3, 2, 1, 0],
                        rgbi=[0, 1, 2], random=7, photo=str(photo), exe=str(e['exe']), proto='hs',
                        hs=dict(stride=e['stride']), package=', '.join(pkgs)))
        print(f"{name:34} {vid:04x}:{pid:04x} {alt} stride {e['stride']:#x} {len(e['keys'])} keys {len(e['leds'])} modes")
    json.dump(res, open(out, 'w'), indent=1)
    print(len(res), 'HidServ keyboards', file=sys.stderr)


main()
