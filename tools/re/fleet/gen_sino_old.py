#!/usr/bin/env python3
"""Registry entries for the Sinowealth mice of the old OemDrv builds (Lowerdev.dll, v3.1
2020-2023): the CompX packets and flash map over feature report 8 (compx.rs, Proto::CompxFeature).

Builds whose protocol functions match build 670711b3 (match_funcs.py: SendCommand 440da0,
ReadEEPROM 4407f0, WriteEEPROM 4404d0, reader thread 43f7f0, checksum 43ff10, code -> keyinfo
4094c0); their built-in DPI source arrays are byte-identical (checked in the exes).
Per product, from the package's Cfg.ini:
  [OPT]  VID, PID, PID2, Sensor (selects the [SENSOR_n] section with that Sensor), Title
  [SENSOR_n]  DM (stages), Mark<i> = x, y, w, h on skins/mouse_nr.png, K<i>_1 = kind, code, param, matrix index (flash key = index - 1)
DPI = FUN_004369a0's built-in tables (emu/sino_old_builtin.json, EmuCall memory dump) and
FUN_00437260: 0x3330 x = cpi/50 - 1, above 7200 cpi/100 - 1 with flag 1; 0x3325 / 0x4090 flag
above 5000 (the table holds the hw byte of cpi/2); register = x | flag << 8.
K line -> flash code = inverse of FUN_004094c0 (major kind << 24 | code -> type, p1, p2).
usage: gen_sino_old.py OUT.json
"""
import hashlib
import json
import pathlib
import re
import sys

R = pathlib.Path('~/.cache/redragon-all').expanduser()
BUILDS = {'670711b3', 'de32d483', '6051954c', 'dbab1e04', '28bab71d', 'd483a086', '5710f22a'}
# The DEV_n builds (2.4 GHz dongles shared by several mice): same packets (SendCommand 451420,
# ReadEEPROM 450b50, GetData 450130 of build 43d56b1f; bede2dce matches all, 2bb2937b the read
# side), [DEV_n] sections told apart by the CID / MID the mouse answers to command 1 (CheckPsd
# 44ff10), photo skins/<CID><MID>/mouse_nr.png. Stage = x, y, ex, chk with (FUN_00450de0)
# ex = flag | flag << 4, and for the 0x3395 x >> 8 in bits 2-3 / 6-7 (FUN_0040c890: 0x3395
# x = cpi/50 - 1, from 26100 cpi/100 - 1 with flag; 0x3104 table lookup, flag above 4000).
DEV_BUILDS = {'43d56b1f', 'bede2dce', '2bb2937b'}
# FUN_0040c890's 0x3104 table (DAT_005de858 / DAT_005de8e8 of 43d56b1f; same bytes in the 3 builds)
T3104 = list(zip(range(600, 4001, 100), [0x10, 0x12, 0x15, 0x17, 0x1a, 0x1d, 0x1f, 0x22, 0x25, 0x27, 0x2a, 0x2d,
                                         0x2f, 0x32, 0x34, 0x37, 0x3a, 0x3c, 0x3f, 0x42, 0x44, 0x47, 0x4a, 0x4c,
                                         0x4f, 0x52, 0x54, 0x57, 0x59, 0x5c, 0x5f, 0x61, 0x64, 0x67, 0x69]))

# FUN_004094c0, inverted: (kind, code) -> flash type | p1 << 8 | p2 << 16
MOUSE = {0x11: 0x101, 0x13: 0x201, 0x12: 0x401, 0x15: 0x801, 0x14: 0x1001, 0x17: 0x33204}
SPECIAL = {0xa2: 0, 0xaa: 0x102, 0xa8: 0x202, 0xa9: 0x302, 0xac: 7, 0xa7: 8, 0xb0: 9}
# Windows virtual key -> HID usage (the keys default K lines use)
VK = {**{0x30 + i: 0x27 if i == 0 else 0x1d + i for i in range(10)},
      **{0x41 + i: 4 + i for i in range(26)}, **{0x70 + i: 0x3a + i for i in range(12)},
      0x6b: 0x57, 0x6d: 0x56, 0xbb: 0x2e, 0xbd: 0x2d, 0x20: 0x2c, 0x0d: 0x28, 0x1b: 0x29}


def ini(path):
    out, cur = {}, None
    raw = path.read_bytes()
    for line in raw.decode('utf-16' if raw[:2] == b'\xff\xfe' else 'gbk', 'replace').splitlines():
        line = '' if line.lstrip().startswith(';') else line.split(';', 1)[0].strip()
        m = re.match(r'\[(\w+)\]', line)
        if m:
            cur = out.setdefault(m.group(1), {})
        elif '=' in line and cur is not None:
            k, v = line.split('=', 1)
            cur.setdefault(k.strip(), v.strip())
    return out


def nums(s):
    return [int(x, 0) for x in re.findall(r'0x[0-9a-fA-F]+|\d+', s or '')]


def table(sensor, builtin):
    if sensor == 0x3330:
        return [(c, c // 50 - 1) for c in range(50, 7201, 50)] + [(c, c // 100 - 1 | 0x100) for c in range(7300, 12001, 100)]
    t = builtin.get(sensor)
    if t is None:
        return None
    flag = sensor in (0x3325, 0x4090)
    return [(c, h | (0x100 if flag and c > 5000 else 0)) for c, h in t if h <= 0xff]


def dev_table(sensor, sec, opt):
    """[(cpi, register)] for a DEV_n build: Cfg DPISET (+ DPIHW, else 1..n) or the built-in table"""
    dset = nums(sec.get('DPISET'))
    if dset:
        t = list(zip(dset, nums(sec.get('DPIHW')) or range(1, len(dset) + 1)))
    elif sensor == 0x3395:
        top = int(opt.get('Sensor3395Max', '26000'))
        t = [(c, c // 50 - 1 if c < 26100 else c // 100 - 1) for c in range(50, top + 1, 50)]
    elif sensor == 0x3104:
        t = T3104
    else:
        return None
    out = []
    for c, x in t:
        f = int(c >= 26100) if sensor == 0x3395 else int(c > 4000) if sensor == 0x3104 else 0
        ex = f | f << 4
        if sensor == 0x3395:
            ex |= (x >> 8) << 2 | (x >> 8) << 6
        out.append((c, (x & 0xff) | ex << 8))
    return out


def keys_of(sec):
    keys = []
    for i in range(1, 33):
        v = nums(sec.get(f'K{i}_1'))  # _1 in every section
        if len(v) != 4:
            continue
        mk = nums(sec.get(f'Mark{i}'))
        x = y = None
        if len(mk) == 4 and mk[2] > 0:
            x, y = mk[0], mk[1]
        hw = code(*v[:3])
        keys.append(dict(index=v[3] - 1, x=x, y=y, w=mk[2] if x is not None else 0, h=mk[3] if x is not None else 0,
                         code=hw, type=None if hw is None else hw & 0xff,
                         p1=None if hw is None else hw >> 8 & 0xff, p2=None if hw is None else hw >> 16 & 0xff))
    return keys


def code(kind, c, p):
    if kind == 1:
        return MOUSE.get(c)
    if kind == 8:
        if c == 0xa4:  # fire: param = p1 << 8 | p2
            return 4 | (p >> 8 & 0xff) << 8 | (p & 0xff) << 16
        return SPECIAL.get(c)
    if kind == 2 and c in VK:
        return 5 | VK[c] << 16 | 1 << 24
    return None


def main():
    builtin = {int(k, 16): [tuple(x) for x in v] for k, v in json.load(open(R / 'emu/sino_old_builtin.json')).items()}
    out, seen = [], set()
    for drv in sorted((R / 'x').rglob('OemDrv.exe')):
        build = hashlib.sha256(drv.read_bytes()).hexdigest()[:8]
        cfgp = drv.parent / 'Cfg.ini'
        if build not in BUILDS | DEV_BUILDS or not cfgp.exists():
            continue
        c = ini(cfgp)
        opt = c.get('OPT', {})
        pkg = drv.relative_to(R / 'x').parts[1]
        title = re.sub(r'^Redragon\s+', '', opt.get('Title', pkg), flags=re.I).strip()
        if not re.search(r'M\d', title):  # "SNIPER PRO": the model code of the folder name
            title = re.search(r'M\d[\w-]*', pkg).group(0)
        pids = [int(opt[k], 0) for k in ('PID', 'PID2') if k in opt]
        if build in BUILDS:
            sensor = int(opt.get('Sensor', '0'), 0)
            sec = next((v for k, v in sorted(c.items()) if k.startswith('SENSOR_') and int(v.get('Sensor', '0'), 0) == sensor),
                       c.get('SENSOR_1'))
            devs = [(title, sec, sensor, table(sensor, builtin), None, drv.parent / 'skins' / 'mouse_nr.png')]
        else:
            devs = []
            for k, sec in sorted(c.items()):
                if not k.startswith('DEV_'):
                    continue
                sensor = int(sec.get('Sensor', '0'), 0)
                cid, mid = int(sec.get('CID', '0'), 0), int(sec.get('MID', '0'), 0)
                devs.append((title if k == 'DEV_1' else f'{title} #{k[4:]}', sec, sensor, dev_table(sensor, sec, opt),
                             cid << 8 | mid, drv.parent / 'skins' / f'{cid:02x}{mid:02x}' / 'mouse_nr.png'))
        for name, sec, sensor, t, psd, photo in devs:
            if name in seen:
                continue
            seen.add(name)
            if t is None:
                print('skip (sensor table unknown):', name, hex(sensor), file=sys.stderr)
                continue
            keys = keys_of(sec)
            e = dict(name=name, package=pkg, family='sino-old', build=build, vid=int(opt['VID'], 0), pids=pids,
                     sensor=f'{sensor:#x}', table=t, keys=keys, psd=psd,
                     stages=int(sec.get('DM', 5)), photo=str(photo) if photo.exists() else None, cfg=str(cfgp),
                     proto='CompxFeature')
            out.append(e)
            print(f"{name:22} {build} {e['vid']:04x}:{','.join(f'{p:04x}' for p in pids)} psd {psd or 0:04x} sensor "
                  f"{sensor:#x} {len(t)} dpi, keys", ' '.join(f"{k['index']}={'?' if k['code'] is None else hex(k['code'])}" for k in keys))
    json.dump(out, open(sys.argv[1], 'w'), indent=1)


main()
