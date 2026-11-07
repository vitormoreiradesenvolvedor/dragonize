#!/usr/bin/env python3
"""Jieli / Witmod keyboards (Qt app + witmodSdk.dll, 4a4c:e987 / e989) -> keyboard entries in
gen_kbd.py's format (proto "jl"), for gen_models.py --kbd. Usage: gen_jieli.py OUT.json

Per package (app/): images/hidImages/Keyboard_<firmware name>.png = the keyboards the build
serves (the name the keyboard answers to `01 0d`, CWitmodHid_GetHidAndFirmwareInfoStu), the
photo; the layout is the qrc resource images/configDir/KeyInfo_<name>.config (extracted by
~/.cache/redragon-all/tools/jl/qrcnames.py + qrctree.py into emu/jl/<exe sha1[:10]>/). Lighting
only (keys: drawn, not remappable: the remap path was not decoded). Protocol: kbd.rs `jl_*`,
NOTES.md "Jieli keyboards".
"""
import hashlib
import json
import pathlib
import re
import sys

R = pathlib.Path.home() / '.cache/redragon-all'

# ledType codes and names of the lighting page (FUN_004e5880 "LightsConfig->NextItem"), UI order
MODES = [(0, 'Static'), (3, 'Neon'), (1, 'Breath'), (2, 'Wave'), (9, 'Twinkle'), (5, 'Snake'), (4, 'Radar'),
         (0xe, 'Auto Ripple'), (0xf, 'Striation'), (7, 'Aurora'), (8, 'Ripple'), (6, 'Reactive'), (0xb, 'Cross'),
         (0x10, 'Fireworks'), (0xc, 'Speed Respond')]


def keyinfo(path):
    """KeyInfo_*.config (JSON-like, numbers with leading zeros) -> [(x, y, w, h, usage)]"""
    out = []
    t = path.read_text('utf-8', 'replace')
    for m in re.finditer(r'\{\s*"PosX"\s*:\s*(-?\d+)\s*,\s*"PosY"\s*:\s*(-?\d+)\s*,\s*"Width"\s*:\s*(\d+)\s*,\s*"Height"\s*:\s*(\d+)\s*,\s*"Dec"\s*:\s*(\d+)', t):
        x, y, w, h, dec = (int(v) for v in m.groups())
        if x >= 0 and y >= 0 and w > 0 and h > 0:
            out.append((x, y, w, h, dec))
    return out


def model_name(pkg):
    m = re.search(r'K\d{3}[A-Z0-9]*(?:-[A-Z0-9]+)*', pkg.upper())
    return m.group(0) if m else pkg


def main():
    groups = {}
    for img in sorted((R / 'x').rglob('hidImages/Keyboard_*.png')):
        app = img.parent.parent.parent
        exe = next((e for e in sorted(app.glob('*.exe')) if 'upgrade' not in e.name.lower()), None)
        if not exe:
            continue
        qrc = R / 'emu/jl' / hashlib.sha1(exe.read_bytes()).hexdigest()[:10] / 'images/configDir'
        name = img.stem[len('Keyboard_'):]
        pkg = img.relative_to(R / 'x').parts[1]
        g = groups.setdefault(pkg, dict(names=[], qrc=qrc, imgs=[]))
        if name not in g['names']:
            g['names'].append(name)
            g['imgs'].append(img)
    res, seen = [], {}
    for pkg, g in groups.items():
        layout = next(((n, i, g['qrc'] / f'KeyInfo_{n}.config') for n, i in zip(g['names'], g['imgs'])
                       if (g['qrc'] / f'KeyInfo_{n}.config').exists()), None)
        if not layout:
            print('skip', pkg, 'no layout for', g['names'], file=sys.stderr)
            continue
        n0, photo, cfg = layout
        sig = tuple(g['names'])
        if sig in seen:  # same keyboards, another package name (K701 / K701W / K701WB)
            if seen[sig]['name'].count(' / ') < 1:  # the header has room for two names
                seen[sig]['name'] += ' / ' + model_name(pkg)
            seen[sig]['package'] += ', ' + pkg
            continue
        keys = [dict(x=x, y=y, w=w, h=h, matrix=min(u, 0xff), led=None, code=None, kind=0, vk=0)
                for x, y, w, h, u in keyinfo(cfg)]
        # hw = ledType + 1 (hw 0 is "Off" in the app; kbd.rs jl_light sends hw - 1)
        leds = [dict(n=i + 1, hw=c + 1, effect=0, light=True, speed=c != 0, color=True, random=c != 0,
                     rgb=False, direct=False, name=nm) for i, (c, nm) in enumerate(MODES)]
        e = dict(name=model_name(pkg), psd=[0] * 6, family='jieli', build='', vid=0x4a4c, pid=0xe987, alt_pids=[0xe989],
                 glen=8 + 0x16 + 2 * len(leds), pro=0, key=0, rgb=0, rgb_read=False, led_off=4,
                 keys=keys, leds=leds, light=[0, 1, 2, 3, 4], speed=[0, 1, 2, 3, 4], rgbi=[0, 1, 2], random=7,
                 photo=str(photo), exe='', proto='jl', jl=g['names'], package=pkg, drawn_keys=True)
        seen[sig] = e
        res.append(e)
    for e in res:
        print(f"{e['name']:34} {len(e['keys']):3} keys  {', '.join(e['jl'])}")
    json.dump(res, open(sys.argv[1], 'w'), indent=1)
    print(len(res), 'Jieli keyboards', file=sys.stderr)


main()
