#!/usr/bin/env python3
"""MosArt keyboards K598-KNS / K599 (062a, C++Builder + JVCL drivers) in gen_kbd.py's format (proto "mk"),
for gen_models.py --kbd. Usage: gen_mk.py OUT.json

Lighting only: the drivers' skins comment out the macro buttons, so the original app has no key remap.
Effects = the DFM list behind text ids 0x10a.. (index = group * 3 + variant of area 0 [8] / [9 + group]);
flags per effect from DAT_006b68ec (0x80 speed, 0x40 colour). See NOTES.md "K598-KNS / K599".
"""
import json
import os
import sys

ROOT = os.path.expanduser('~/.cache/redragon-all/x/Teclados')
MODELS = [('K598-KNS', 'Knight K598-KNS', 'K598', 0x8566), ('K599', 'Deimos K599', 'K599', 0x8556)]
EFFECTS = ['Trial of light', 'Breathe', 'Constantly bright', 'Drip', 'No trace of snow', 'Running away',
           'Dapeng wings', 'Thick and thin hair', 'Digital age', 'Flow by direction', 'Colorful clouds',
           'Peak circuit', 'Hundreds of flowers', 'Starry', 'Meteor', 'Right and left', 'Zhongzhichengcheng',
           'Speed passion']
FLAGS = [0xc0, 0xc0, 0x40, 0xc0, 0x80, 0xe0, 0xe0, 0xe0, 0xe0, 0xc0, 0xc0, 0xa0, 0x80, 0xc0, 0xc0, 0xc0, 0xc0, 0xc0]


def main():
    leds = [dict(n=i + 1, hw=i + 1, effect=0, light=True, speed=bool(f & 0x80), color=bool(f & 0x40),
                 random=bool(f & 0x40), rgb=False, direct=False, name=nm)
            for i, (nm, f) in enumerate(zip(EFFECTS, FLAGS))]
    res = []
    for name, pkg, skin, pid in MODELS:
        photo = f'{ROOT}/{pkg}/app/skins/{skin}/keyboard_led_normal.bmp'
        # general block: the standard lighting part, then area 0 (0x18) and area 1 (0x6a) as read
        res.append(dict(name=name, psd=[0] * 6, family='mosart', build='', vid=0x062a, pid=pid, alt_pids=[],
                        glen=8 + 0x16 + 2 * len(leds) + 0x18 + 0x6a, pro=0, key=0, rgb=0, rgb_read=False,
                        led_off=4, keys=[], leds=leds, light=[0, 1, 2, 3, 4], speed=[0, 1, 2, 3, 4],
                        rgbi=[0, 1, 2], random=8, photo=photo, exe='', proto='mk', package=pkg, drawn_keys=True))
        print(f'{name:10} 062a:{pid:04x}  {photo}')
    json.dump(res, open(sys.argv[1], 'w'), indent=1)


if __name__ == '__main__':
    main()
