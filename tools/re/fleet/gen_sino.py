#!/usr/bin/env python3
"""Registry entries for the Sinowealth mice driven by OemDrv.exe ("BY" OEM driver).

Everything per product comes from the package's Cfg.ini, read the way OemDrv reads it
(build 7875e77b = Ghidra project sino-oem-ms):
  [MS] or [OPT]   VID, PID, Title                                  (FUN_00476890)
  [SENSOR_n]      Sensor, DM (DPI stages), DPI (defaults), DEFLEVEL,
                  DPIRANGE = min,max,step,hw0[,hwstep,min2,max2,step2] or DPISET + DPIHW
                  (else the sensor's built-in table, see builtin_tables)   (FUN_00475b00)
                  K<i>_<n> = type,code,param,matrix index  (button i of sensor section n)
                                                                    (FUN_00475830)
Button codes = FUN_004140d0 (keyinfo -> 4-byte hardware code, little endian, first byte = kind).
usage: gen_sino.py OUT.json
"""
import hashlib
import json
import pathlib
import re
import sys

R = pathlib.Path('~/.cache/redragon-all').expanduser()


# OemDrv builds whose protocol functions (FUN_00413290/413720/413800/4138b0/415260/4140d0 of
# the decoded build 86e99000) all match (FuncFeatures.java + match_funcs.py); the other 11
# builds found have no 0x7b general-block writer = older protocol, not supported yet
VERIFIED = {'86e99000', 'a41b18bc', '62ac35b8', '7a61779c', '1e0b64f3', 'b7b42187', '64858b09', 'f4072cc7'}
# "CDrvBY24G" builds (sino.rs Proto::SinoBy24g; c1111b9b = 987426b5's code shifted, match_funcs
# maps all its protocol functions, button codes = 86e99000's FUN_004140d0); built-in DPI tables =
# FUN_0040c500 run in EmuCall (emu/by24g_builtin.json), 16-bit values; [OPT] Sensor = section
BY24G = {'987426b5', 'c1111b9b', '030c3779', 'ff62fb10'}
# "CDrv8832" build (M712, MsFw=10, sino.rs Proto::Sino8832): DPI codes = the 86e99000 built-in
# tables (cpi/100-1 for 0x3325), key codes = FUN_004140d0's
S8832 = {'ac3d8a3f'}
# "CDrv8948" build (S101-4-DE combo mouse, MsFw=3; Apply_M_STD_8948_1 FUN_00416130): report 5 command
# (FUN_00412cb0 = 413290), report 4 read / write (FUN_00413140 / FUN_00413200 = 413720 / 413800), the
# matrix writer FUN_004132b0 and the general encoder FUN_00414c00 differ in code only: same
# 0x12 + 0x50 / 0x11 + 0x7b blocks, same general layout (rate codes 0x125/0x250/0x500/0x1000 -> 1-4,
# [3] enabled | position << 4, [4] disabled mask, [5..] DPI codes, [0x15..] colours); DPI codes =
# DPISET index + 1 without DPIHW (FUN_0046a7d0) as in 86e99000
VERIFIED |= {'66995128'}
# Build 7a7c5b20 (Lavawolf M701, appdir BY8801, sino.rs Proto::Sino8801): FindDev FUN_00432a00 opens
# VID_258A&PID_1007 itself (the Cfg has no VID / PID); built-in DPI tables FUN_0042b550 (code = index + 1);
# key codes FUN_00408430
B8801 = {'7a7c5b20'}
T8801 = {
    0x3050: [500, 750, 1000, 1250, 1500, 1750, 2000, 2500, 3000, 3500, 4000],
    0x5050: [500, 750, 1000, 1250, 1375, 1500, 1750, 2000, 2500, 2750],
    0x9800: [500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 3750, 4000, 4500, 5000,
             6000, 7000, 8200, 16400],
    0x3212: [500, 750, 1000, 1200, 1600, 2000, 2400, 3000, 3200, 3500, 4000, 4500, 5000, 5500, 6000, 7200],
    0x3320: [500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 4000, 4500, 5000, 6000, 7000],
    0x3330: [500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 4000, 4500, 5000, 6000, 7200,
             14400],
    0x3360: [500, 800, 1000, 1200, 1600, 2000, 2500, 2800, 3000, 3200, 3500, 4000, 4500, 5000, 6000, 7000, 8000, 9000,
             10000, 12000, 24000],
    0x3610: [400, 800, 1000, 1200, 1600, 2000, 2400, 2800, 3000, 3200, 4000, 4800, 5600, 6400],
}
MOUSE8801 = {0x11: 0xf010, 0x12: 0xf210, 0x13: 0xf110, 0x14: 0xf410, 0x15: 0xf310, 0x16: 0x232f020, 0x17: 0x130,
             0x18: 0x232f120}
SPECIAL8801 = {0xa2: 0x150, 0xa7: 0x250, 0xa8: 0x2040, 0xa9: 0x4040, 0xaa: 0x40, 0xab: 0x650}


def code8801(k):
    """FUN_00408430 for the kinds a default Cfg.ini uses (key number nibble left 0)"""
    kind, c, p = k[0], k[1], k[2]
    if kind == 1:
        return MOUSE8801.get(c)
    if kind == 8 and c == 0xa6:  # DPI lock: the stage's code (index + 1) << 8
        return (p & 0xf) << 8 | 0x8040
    if kind == 8:
        return SPECIAL8801.get(c)
    if kind == 2:
        u = vk_usage(c)
        return None if u is None else u << 16 | (p & 0xff) << 8 | 0x60
    return None


def ini(path):
    t = path.read_bytes().decode('gbk', 'replace')
    out, cur = {}, None
    for line in t.splitlines():
        line = line.split(';', 1)[0].strip() if not line.lstrip().startswith(';') else ''
        m = re.match(r'\[(\w+)\]', line)
        if m:
            cur = out.setdefault(m.group(1), {})
        elif '=' in line and cur is not None:
            k, v = line.split('=', 1)
            cur.setdefault(k.strip(), v.strip())
    return out


def nums(s):
    return [int(x, 0) for x in re.findall(r'0x[0-9a-fA-F]+|\d+', s or '')]


def builtin_tables():
    """sensor -> [(cpi, hw)]: OemDrv's built-in tables. FUN_0046f420 fills DAT_0061c098 at
    start-up (records of 0x968 bytes: [sensor u32][count u32][cpi u16 x 600][hw u16 x 600]);
    emu/sino_builtin.json = that memory after an EmuCall run of the function."""
    return {int(k, 16): [tuple(x) for x in v] for k, v in json.load(open(R / 'emu/sino_builtin.json')).items()}


def dpi_table(sec, builtin):
    rng = nums(sec.get('DPIRANGE'))
    if rng:
        t, hw = [], rng[3]
        for c in range(rng[0], rng[1] + 1, rng[2]):
            t.append((c, hw))
            hw += 1
        if len(rng) > 5:
            for c in range(rng[5], rng[6] + 1, rng[7]):
                t.append((c, hw))
                hw += rng[4]
        return t
    s = nums(sec.get('DPISET'))
    if s:
        h = nums(sec.get('DPIHW')) or list(range(1, len(s) + 1))
        return list(zip(s, h))
    return builtin.get(int(sec.get('Sensor', '0'), 0))


MOUSE = {0x11: 0x111, 0x12: 0x411, 0x13: 0x211, 0x14: 0x1011, 0x15: 0x811, 0x16: 0x2320131,
         0x17: 0x3320131, 0x18: 0x112, 0x19: 0xff12, 0x1a: 0xff13, 0x1b: 0x113}
SPECIAL = {0xa2: 0x150, 0xa7: 0x250, 0xa8: 0x141, 0xa9: 0x241, 0xaa: 0x41, 0xab: 0x650, 0xac: 0x450,
           0xae: 0x750, 0xaf: 0x950, 0xc2: 0x850, 0xd0: 0x4150, 0xd1: 0x4250, 0xd2: 0x143, 0xd3: 0x243}


def code(k):
    """FUN_004140d0 for the kinds a default Cfg.ini uses"""
    kind, c, p = k[0], k[1], k[2]
    if kind == 1:
        return MOUSE.get(c)
    if kind == 8:
        return SPECIAL.get(c)
    if kind in (9, 0xb):
        return p
    if kind == 2:  # key: VK -> HID usage (FUN_00483b10), modifier mask from the param
        u = vk_usage(c)
        return None if u is None else u << 16 | (p & 0xff) << 8 | 0x21
    return None


def vk_usage(vk):
    if 0x41 <= vk <= 0x5a:
        return vk - 0x41 + 4
    if 0x31 <= vk <= 0x39:
        return vk - 0x31 + 0x1e
    if vk == 0x30:
        return 0x27
    if 0x70 <= vk <= 0x7b:
        return vk - 0x70 + 0x3a
    return None


def main():
    inv = [l.rstrip('\n').split('\t') for l in open(R / 'inventory.tsv', errors='surrogateescape')]
    out = []
    seen = set()
    builtin = builtin_tables()
    by24g = {int(k, 16): [tuple(x) for x in v] for k, v in json.load(open(R / 'emu/by24g_builtin.json')).items()}
    for r in inv:
        if r[1] != 'sino-oem' or not r[0].startswith(('Mouses', 'Combos')):
            continue
        pkg = r[0]  # the folder keeps the " (+n)" of packages shared by n more products
        for cfgp in sorted((R / 'x' / pkg).rglob('Cfg.ini')):
            c = ini(cfgp)
            ms = c.get('MS') or c.get('OPT', {})
            drv = cfgp.parent / 'OemDrv.exe'
            build = hashlib.sha256(drv.read_bytes()).hexdigest()[:8] if drv.exists() else '-'
            if build in B8801:
                ms = dict(ms, VID='0x258a', PID='0x1007')
            if 'VID' not in ms or not any(k.startswith('SENSOR_') for k in c):
                continue
            if build not in VERIFIED | BY24G | S8832 | B8801:
                print('skip (OemDrv build', build, 'protocol not verified):', pkg, file=sys.stderr)
                continue
            opt = c.get('OPT', {})
            sec = c['SENSOR_1']
            if build in BY24G:
                want = int(opt.get('Sensor', '0'), 0)
                sec = next((v for k, v in sorted(c.items()) if k.startswith('SENSOR_') and int(v.get('Sensor', '0'), 0) == want), sec)
            if build in B8801:  # "sensor=" in lower case
                sec = dict(sec, Sensor=sec.get('Sensor', sec.get('sensor', '0')))
                table = dpi_table(sec, {k: [(c, i + 1) for i, c in enumerate(v)] for k, v in T8801.items()})
            else:
                table = dpi_table(sec, by24g if build in BY24G else builtin)
            keys = []
            for i in range(1, 21):
                v = nums(sec.get(f'K{i}_1'))
                if len(v) == 4:
                    keys.append(v)
            title = re.sub(r'^Redragon\s+', '', opt.get('Title', pkg.split('/')[-1]), flags=re.I).strip()
            if build in BY24G:  # M602-KS's Cfg.ini says "M607-KS": the folder's model code
                title = re.search(r'M\d[\w-]*', pkg.split('/')[-1]).group(0).rstrip('-')
            vid, pid = int(ms['VID'], 0), int(ms['PID'], 0)
            if (vid, pid, title) in seen:
                continue
            seen.add((vid, pid, title))
            # BY24G: the same mouse in another package (the W variant): one entry, both names
            twin = build in BY24G and next((e for e in out if (e['vid'], e['pid'], e['cfg_sig']) == (vid, pid, (repr(table), repr(keys)))), None)
            if twin:
                if title not in twin['name'].split(' / '):
                    twin['name'] += ' / ' + title
                continue
            photo = cfgp.parent / 'skins' / 'mouse' / 'mouse_nr.png'
            if not photo.exists() and (cfgp.parent / 'skins' / 'key_nr.png').exists():
                # BY8801: the Keys page's background (912 x 500, framed), cropped to the mouse
                from PIL import Image
                im = Image.open(cfgp.parent / 'skins' / 'key_nr.png').convert('RGB')
                im = im.crop((4, 4, im.width - 4, im.height - 4))
                box = im.convert('L').point(lambda v: 255 if v > 40 else 0).getbbox()
                photo = R / 'emu' / f"by8801-{hashlib.sha256(str(cfgp).encode()).hexdigest()[:8]}.png"
                im.crop((box[0] - 8, box[1] - 8, box[2] + 8, box[3] + 8)).save(photo)
            e = dict(name=title, package=pkg, build=build, vid=vid, pid=pid, pid2=int(ms['PID2'], 0) if 'PID2' in ms else None,
                     sensor=int(sec.get('Sensor', '0'), 0), stages=int(sec.get('DM', 5)),
                     # FUN_00475b00: MDNUM 3 / 6 / 9 -> 1 / 2 / 3 modes (profiles)
                     modes={3: 1, 6: 2}.get(int(sec.get('MDNUM', 9)), 3),
                     dpi_defaults=nums(sec.get('DPI')), deflevel=int(sec.get('DEFLEVEL', '0') or 0),
                     table=table, buttons=[dict(kind=k[0], code=k[1], param=k[2], index=k[3], hw=(code8801 if build in B8801 else code)(k)) for k in keys],
                     photo=str(photo) if photo.exists() else None, cfg=str(cfgp), cfg_sig=(repr(table), repr(keys)),
                     proto='SinoBy24g' if build in BY24G else 'Sino8832' if build in S8832 else 'Sino8801' if build in B8801 else 'Sino')
            out.append(e)
            print(f"{title:28} {vid:04x}:{pid:04x} sensor {e['sensor']:#x} {len(table or [])} dpi steps"
                  f" {len(keys)} buttons", ['?' if b['hw'] is None else f"{b['hw']:x}" for b in e['buttons']])
    json.dump(out, open(sys.argv[1], 'w'), indent=1)


main()
