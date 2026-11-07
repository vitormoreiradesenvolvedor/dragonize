#!/usr/bin/env python3
"""Magnetic K729RGB-M (MFC "Redragon Driver.exe" + mui.dll, 19f5:af08 wired / af28 receiver) in
gen_kbd.py's format (proto "mg"), for gen_models.py --kbd. Usage: gen_mg.py OUT.json

Lighting only (the actuation / rapid-trigger / key tables are not ported). Modes = the driver's
default t_light_data rows (FUN_00437c70): mode id, config_func (1 brightness, 2 speed, 0x10
colourful, 0x20 colour), names = language/1033.lan 663..684. Brightness 0..100 (kbd.rs MG_BRIGHT per UI level), speed byte = 4 - speed.
Photo: skins/theme1/keyboard/keybord_bg.png. Protocol: kbd.rs (mg_*), NOTES.md "K729RGB-M".
"""
import json
import pathlib
import re
import sys

APP = pathlib.Path.home() / '.cache/redragon-all/x/Teclados/OTIIM MAGNETIC K729RGB-M/inno.x/app'
# (mode, config_func); 0x80 music and 0 per-key are left out
MODES = [(1, 3), (2, 0x31), (3, 0x31), (4, 0x33), (5, 3), (6, 0x37), (7, 0x3b), (8, 0x73), (9, 0x33), (10, 0xb3)] + \
        [(m, 0x33) for m in range(11, 23)]


def main():
    lan = (APP / 'language/1033.lan').read_text('utf-16')
    names = dict(re.findall(r'^(\d+)=(.*?)\s*$', lan, re.M))
    leds = [dict(n=i + 1, hw=m, effect=0, light=bool(f & 1), speed=bool(f & 2), color=bool(f & 0x20),
                 random=bool(f & 0x10), rgb=False, direct=False, name=names[str(662 + m)])
            for i, (m, f) in enumerate(MODES)]
    e = dict(name='K729RGB-M', psd=[0] * 6, family='magnetic', build='', vid=0x19f5, pid=0xaf08, alt_pids=[0xaf28],
             glen=8 + 0x16 + 2 * len(leds) + 0x20 + 1, pro=0, key=0, rgb=0, rgb_read=False, led_off=4,
             keys=[], leds=leds, light=[0, 1, 2, 3, 4], speed=[4, 3, 2, 1, 0], rgbi=[0, 1, 2], random=7,
             photo=str(APP / 'skins/theme1/keyboard/keybord_bg.png'), exe='', proto='mg',
             package='OTIIM MAGNETIC K729RGB-M', drawn_keys=True)
    print(f"{e['name']} 19f5:af08/af28 {len(leds)} modes: {', '.join(l['name'] for l in leds)}")
    json.dump([e], open(sys.argv[1], 'w'), indent=1)


main()
