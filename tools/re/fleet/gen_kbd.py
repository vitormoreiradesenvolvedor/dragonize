#!/usr/bin/env python3
"""Registry entries for the Sinowealth keyboards of the KeyboardDrv.exe / OemDrv.exe packages
(258a:xxxx); protocol in src/kbd.rs.

Both drivers talk to the keyboard the same way (KeyboardDrv 801262a0 FUN_004110e0 ApplySetting,
OemDrv 4f3eab8b FUN_0048a6d0 CDev916KB::ApplySetting): command = feature report 5, data = feature
report 6 (1032 bytes, `06 CMD ADDR 00 40 00 00 00 data`):
  83 / 03 ProAddr     general block (KeyboardDrv 0x88 bytes, OemDrv 0xfe; profile part at +8)
  84 / 04 KeyAddr     key matrix, 0x100 u32 = firmware code per matrix index
  08 RgbTabAddr       7 RGB colours per lighting mode (0x15 bytes each; OemDrv reads with 88)
The address bytes default per build (push imm of the GetPrivateProfileInt calls or the literal
of the read function) and Cfg.ini [OPT] ProAddr / KeyAddr / RgbTabAddr override them.
Per product:
  KeyboardDrv: app/Cfg.ini [OPT] VID, PID, Title, LedOpt<i>, Light(UI/HW), Speed(UI/HW),
    RGBIndex; [KEY] K<i> = x1, y1, x2, y2, kind, vk, param, matrix index, led index on
    skins/keyimg.png
  OemDrv: app/Dev/kb[/<dev>]/KB.ini [OPT] the same keys (Name), [KEY] K<i> = rect, kind, vk,
    param, matrix index on keyimg.png in that folder
Default function -> firmware code: OemDrv keyinfo_to_hardware_code (FUN_00489960).
Mode names: Text/en/text.xml tc_led_mode<LedOpt number>.
usage: gen_kbd.py OUT.json
"""
import collections
import hashlib
import json
import pathlib
import re
import struct
import sys

R = pathlib.Path('~/.cache/redragon-all').expanduser()

# Windows virtual key -> HID usage (keyboard page)
VK = {0x08: 0x2a, 0x09: 0x2b, 0x0d: 0x28, 0x13: 0x48, 0x14: 0x39, 0x1b: 0x29, 0x20: 0x2c, 0x21: 0x4b,
      0x22: 0x4e, 0x23: 0x4d, 0x24: 0x4a, 0x25: 0x50, 0x26: 0x52, 0x27: 0x4f, 0x28: 0x51, 0x2c: 0x46,
      0x2d: 0x49, 0x2e: 0x4c, 0x5d: 0x65, 0x6a: 0x55, 0x6b: 0x57, 0x6c: 0x85, 0x6d: 0x56, 0x6e: 0x63,
      0x6f: 0x54, 0x90: 0x53, 0x91: 0x47, 0xba: 0x33, 0xbb: 0x2e, 0xbc: 0x36, 0xbd: 0x2d, 0xbe: 0x37,
      0xbf: 0x38, 0xc0: 0x35, 0xdb: 0x2f, 0xdc: 0x31, 0xdd: 0x30, 0xde: 0x34, 0xe2: 0x64, 0xc1: 0x87,
      0x15: 0x90, 0x19: 0x91, 0x1c: 0x8a, 0x1d: 0x8b, 0xdf: 0x32}
VK.update({0x30 + i: 0x27 if i == 0 else 0x1d + i for i in range(10)})
VK.update({0x41 + i: 4 + i for i in range(26)})
VK.update({0x70 + i: 0x3a + i for i in range(12)})
VK.update({0x7c + i: 0x68 + i for i in range(12)})
VK.update({0x60: 0x62, **{0x61 + i: 0x59 + i for i in range(9)}})
MODS = {0xa2: 0xe0, 0xa0: 0xe1, 0xa4: 0xe2, 0x5b: 0xe3, 0xa3: 0xe4, 0xa1: 0xe5, 0xa5: 0xe6, 0x5c: 0xe7,
        0x11: 0xe0, 0x10: 0xe1, 0x12: 0xe2}
MOUSE = {0x11: 0xf0000007, 0x12: 0xf2000007, 0x13: 0xf1000007, 0x14: 0xf3000007, 0x15: 0xf4000007}


def ini(path):
    out, cur = {}, None
    raw = path.read_bytes()
    for line in raw.decode('utf-16' if raw[:2] in (b'\xff\xfe', b'\xfe\xff') else 'gbk', 'replace').splitlines():
        line = '' if line.lstrip().startswith((';', '//')) else line.split(';', 1)[0].strip()
        m = re.match(r'\[(\w+)\]', line)
        if m:
            cur = out.setdefault(m.group(1), {})
        elif '=' in line and cur is not None:
            k, v = line.split('=', 1)
            cur.setdefault(k.strip(), v.strip())
    return out


def nums(s, base=0):
    out = []
    for x in re.findall(r'0x[0-9a-fA-F]+|[0-9a-fA-F]+', s or ''):
        try:
            out.append(int(x, 16 if base == 16 and not x.startswith('0x') else 0))
        except ValueError:
            out.append(int(x, 16))
    return out


def hw_code(kind, vk, p):
    """keyinfo_to_hardware_code (OemDrv FUN_00489960) for the K lines' defaults"""
    if kind == 1:
        return MOUSE.get(vk)
    if kind == 2:
        if vk in MODS:
            return MODS[vk] << 24 | 6
        if vk == 0xfa:  # Fn
            return 0x20
        u = VK.get(vk)
        return None if u is None else u << 24
    if kind == 9:
        return p & 0xffffffff
    if kind == 8 and vk == 0xa2:
        return 0
    return None


class Pe:
    def __init__(self, b):
        self.b = b
        pe = struct.unpack_from('<I', b, 0x3c)[0]
        ns, osz = struct.unpack_from('<H', b, pe + 6)[0], struct.unpack_from('<H', b, pe + 20)[0]
        self.ib = struct.unpack_from('<I', b, pe + 52)[0]
        self.secs = [struct.unpack_from('<8sIIII', b, pe + 24 + osz + 40 * i) for i in range(ns)]

    def va(self, off):
        for _, _, v, rs, ro in self.secs:
            if ro <= off < ro + rs:
                return self.ib + v + off - ro

    def wstr_va(self, s):
        w = (s + '\0').encode('utf-16le')
        for m in re.finditer(re.escape(w), self.b):
            if m.start() % 2 == 0 and self.b[m.start() - 2:m.start()] == b'\0\0':
                return self.va(m.start())


def build_params(exe):
    """general block length and default ProAddr / KeyAddr / RgbTabAddr of one driver build"""
    b = exe.read_bytes()
    pe = Pe(b)
    # read of the general block: cmp byte [esp+x], 0x83 ... mov ecx, n (dwords) [movsw]
    lens = {m.group(1)[0] * 4 + (2 if m.group(2) else 0)
            for m in re.finditer(rb'\x80\x7c\x24.\x83.{2,16}?\xb9(.)\x00\x00\x00.{0,8}?\xf3\xa5(\x66\xa5)?', b, re.S)}
    lens &= {0x88, 0xfe}
    glen = lens.pop() if len(lens) == 1 else None
    addr = {}
    for key, cmd in (('ProAddr', 0x83), ('KeyAddr', 0x84), ('RgbTabAddr', 0x08)):
        va = pe.wstr_va(key)
        if va:
            mm = re.search(rb'\x68(.)\x00\x00\x00\x68' + re.escape(struct.pack('<I', va)), b, re.S)
            if mm:
                addr[key] = mm.group(1)[0]
                continue
        # mov byte [esp+d], cmd ... mov byte [esp+d+1], ADDR (within a few instructions)
        lits = set()
        for x in re.finditer(rb'\xc6\x44\x24(.)' + re.escape(bytes([cmd])), b, re.S):
            d = x.group(1)[0]
            y = re.search(rb'\xc6\x44\x24' + re.escape(bytes([(d + 1) & 0xff])) + rb'(.)', b[x.end():x.end() + 40], re.S)
            if y and y.group(1)[0] >= 0x80:
                lits.add(y.group(1)[0])
        if len(lits) == 1:
            addr[key] = lits.pop()
    # OemDrv reads the colour table back (88 RgbTabAddr)
    rgb_read = bool(re.search(rb'\xc6(\x44\x24|\x45).\x88\xc6(\x44\x24|\x45).' + bytes([addr.get('RgbTabAddr', 0xb8)]), b))
    return glen, addr, rgb_read


def mode_names(app):
    for p in [app / 'Text' / 'en' / 'text.xml', app / 'Text' / 'en' / 'Text.xml']:
        if p.exists():
            raw = p.read_bytes()
            t = raw.decode('utf-16' if raw[:2] in (b'\xff\xfe', b'\xfe\xff') else 'utf-8', 'replace')
            # KeyboardDrv: tc_led_mode<n>; OemDrv: tc_kb_led<n> (tc_ms_led = the combo's mouse)
            for tag in ('tc_led_mode', 'tc_kb_led'):
                m = {int(n): v.strip() for n, v in re.findall(rf'<{tag}(\d+)>([^<]*)</{tag}\d+>', t)}
                if m:
                    return m
    return {}


# "BYK901" OemDrv builds (045be641 6fccdd79 c755de9a 288e15ea, decompiled ghidra/out/kb-*.c):
# key matrix = feature report 9, 0x7e LE u32 (FUN_00403210), keyinfo_to_hardware_code
# FUN_00402b30: key [0, mods, usage(vk), usage(vk2)], Fn 0x00b00000, mouse [10 f0+n ..],
# consumer [01 00 hi lo]. Lighting (report 10, 0x29 bytes, firmware colour groups) not decoded.
BYK_MOUSE = {0x11: 0xf010, 0x12: 0xf210, 0x13: 0xf110, 0x14: 0xf410, 0x15: 0xf310}


# FUN_0042e3a0's table: 0x7d (usage, vk) pairs, starting with Esc, F1 (29 1b 3a 70)
BYK_TABLE = bytes([0x29, 0x1b, 0x3a, 0x70, 0x3b, 0x71])


def is_byk(exe):
    b = exe.read_bytes()
    return b'Set matrix, err'.decode().encode('utf-16-le') in b and b.find(BYK_TABLE) > 0


def byk_table(exe):
    b = exe.read_bytes()
    i = b.find(BYK_TABLE)
    return {b[i + 2 * k + 1]: b[i + 2 * k] for k in range(0x7d) if b[i + 2 * k]}


def byk_code(kind, vk, p, table):
    if kind == 1:
        return BYK_MOUSE.get(vk)
    if kind == 2:
        if vk == 0xfa and p == 0:
            return 0xb00000
        u = table.get(vk)
        return None if u is None else u << 16 | (p & 0xff) << 8
    return None


def byk_entry(opt, keysec, photo, exe, build, family):
    table = byk_table(exe)
    keys = []
    for i in range(1, 200):
        v = nums(keysec.get(f'K{i}'))
        if len(v) < 8:
            continue
        x1, y1, x2, y2, kind, vk, p, mx = v[:8]
        if mx >= 0x7e:
            continue
        keys.append(dict(x=x1, y=y1, w=x2 - x1, h=y2 - y1, matrix=mx, led=None, code=byk_code(kind, vk, p, table), kind=kind, vk=vk))
    try:
        vid, pid = int(opt['VID'], 0), int(opt['PID'], 0)
    except (KeyError, ValueError):
        return None, 'no VID/PID'
    title = opt.get('Name') or opt.get('Title') or ''
    title = re.sub(r'^Redragon\s+|\s+(Mechanical\s+)?(Gaming\s+)?Keyboard$', '', title, flags=re.I).strip()
    return dict(name=title, psd=[0] * 6, family=family, build=build, vid=vid, pid=pid, glen=0, pro=0, key=0, rgb=0,
                rgb_read=False, led_off=0, keys=keys, leds=[], light=[], speed=[], rgbi=[0, 1, 2], random=7,
                photo=str(photo) if photo.exists() else None, exe=str(exe), proto='byk'), None


def entry(opt, keysec, photo, app, exe, build, params, family):
    if is_byk(exe):
        return byk_entry(opt, keysec, photo, exe, build, family)
    glen, addr, rgb_read = params
    for k in ('ProAddr', 'KeyAddr', 'RgbTabAddr'):
        if k in opt:
            addr[k] = int(opt[k], 0)
    if glen is None or 'ProAddr' not in addr or 'KeyAddr' not in addr:
        return None, 'protocol constants not found'
    keys = []
    for i in range(1, 200):
        v = nums(keysec.get(f'K{i}'))
        if len(v) < 8:
            continue
        x1, y1, x2, y2, kind, vk, p, mx = v[:8]
        if mx > 0x83:
            continue
        keys.append(dict(x=x1, y=y1, w=x2 - x1, h=y2 - y1, matrix=mx, led=v[8] if len(v) > 8 else None,
                         code=hw_code(kind, vk, p), kind=kind, vk=vk))
    names = mode_names(app)
    leds = []
    for i in range(1, 40):
        v = nums(opt.get(f'LedOpt{i}'))
        if len(v) >= 7:
            leds.append(dict(n=i, hw=v[0], effect=v[1], speed=bool(v[2]), light=bool(v[3]), direct=bool(v[4]),
                             random=bool(v[5]), color=bool(v[6]), name=names.get(i)))
    light = nums(opt.get('LightHW')) or list(range(5))
    speed = nums(opt.get('SpeedHW')) or list(range(5))
    rgbi = nums(opt.get('RGBIndex')) or [0, 1, 2]
    try:
        vid, pid = int(opt['VID'], 0), int(opt['PID'], 0)
    except (KeyError, ValueError):
        return None, 'no VID/PID'
    psd = next((opt[k] for k in opt if k.lower() == 'psd'), '')
    psd = [int(x, 16) for x in re.findall(r'[0-9a-fA-F]+', psd)][:6]
    title = opt.get('Name') or opt.get('Title') or ''
    title = re.sub(r'^Redragon\s+|\s+(Mechanical\s+)?(Gaming\s+)?Keyboard$', '', title, flags=re.I).strip()
    return dict(name=title, psd=psd if len(psd) == 6 else None, family=family, build=build, vid=vid, pid=pid, glen=glen, pro=addr['ProAddr'],
                key=addr['KeyAddr'], rgb=addr.get('RgbTabAddr', 0xb8), rgb_read=rgb_read,
                led_off=int(opt.get('LedModeSelectOffset', '4'), 0), keys=keys, leds=leds, light=light,
                speed=speed, rgbi=rgbi[:3], random=int(opt.get('RandomVal', '7'), 0),
                photo=str(photo) if photo.exists() else None, exe=str(exe)), None


def main():
    out, seen, skipped = [], set(), []
    cache = {}
    exes = sorted(list((R / 'x').rglob('KeyboardDrv.exe')) + list((R / 'x').rglob('OemDrv.exe')))
    for exe in exes:
        app = exe.parent
        pkg = exe.relative_to(R / 'x').parts[1]
        build = hashlib.sha256(exe.read_bytes()).hexdigest()[:8]
        cands = []
        cfg = app / 'Cfg.ini'
        if cfg.exists():
            c = ini(cfg)
            if 'KEY' in c:
                cands.append((c.get('OPT', {}), c['KEY'], app / 'skins' / 'keyimg.png', 'sino-kbd'))
        for kb in sorted((app / 'Dev').rglob('KB.ini')) if (app / 'Dev').exists() else []:
            c = ini(kb)
            if 'KEY' in c:
                cands.append((c.get('OPT', {}), c['KEY'], kb.parent / 'keyimg.png', 'sino-oem-kb'))
        if not cands:
            continue
        if build not in cache:
            cache[build] = build_params(exe)
        for opt, keysec, photo, fam in cands:
            e, why = entry(dict(opt), keysec, photo, app, exe, build, (cache[build][0], dict(cache[build][1]), cache[build][2]), fam)
            if e is None:
                skipped.append((pkg, build, why))
                continue
            if not e['name'] or not re.search(r'K\d', e['name']):
                m = re.search(r'K\d[\w-]*', pkg)
                e['name'] = m.group(0) if m else pkg
            e['package'] = pkg
            sig = (e['name'], e['vid'], e['pid'])
            if sig in seen:
                continue
            seen.add(sig)
            out.append(e)
            print(f"{e['name']:24} {fam:11} {build} {e['vid']:04x}:{e['pid']:04x} gen {e['glen']:#x} pro {e['pro']:#x} "
                  f"key {e['key']:#x} rgb {e['rgb']:#x}{'r' if e['rgb_read'] else ''} {len(e['keys'])} keys "
                  f"{len(e['leds'])} modes {'photo' if e['photo'] else 'NO PHOTO'}")
    # builds without Text/ (names drawn in their skins): the name the other 79 builds
    # give the same hardware code (consistent in all but one, see NOTES.md)
    vote = collections.defaultdict(collections.Counter)
    for e in out:
        for l in e['leds']:
            if l['name']:
                vote[l['hw']][l['name']] += 1
    for e in out:
        for l in e['leds']:
            if not l['name']:
                l['name'] = vote[l['hw']].most_common(1)[0][0] if vote[l['hw']] else f"Mode {l['n']}"
    for s in skipped:
        print('skip', *s, file=sys.stderr)
    json.dump(out, open(sys.argv[1], 'w'), indent=1)
    print(len(out), 'keyboards', file=sys.stderr)


main()
