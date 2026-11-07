#!/usr/bin/env python3
"""MosArt mice of the C++Builder / JVCL drivers -> mouse entries in gen_compx.py's format
(proto "Mosart"), for gen_models.py --compx. Usage: gen_mosart.py OUT.json

The drivers keep their tables in code, so each package's are read from its decompile
(ghidra/out/mosart-<model>.c, NOTES.md "MosArt"):
  M690 "2645" (Redragon Wireless Gaming Mouse.exe): levels DAT_0062b600 = 800 1200 1600 2400 3200
  4800 cpi, codes DAT_0062b618 = 05 04 00 01 02 03; 13 key records (default PTR_DAT_0062b660),
  the 8 the UI shows (skin_button_control.ini Btn1 2 3 12 11 10 5 4 = slots 0 1 2 11 10 9 4 3);
  photo = the mouse on skins/button_normal.jpg (60..222 x 20..340, without the callouts), marks = its callout dots.
  M688 "2613" (Monster Wireless Gaming Mouse.exe): levels DAT_0062e5b4 = 500 1000 2000 3000 4000 7200,
  same codes; 10 key records (PTR_DAT_0062e614), all shown (Btn1 2 3 6 7 8 10 5 4 9).
  M691 (Redragon M691 GAMING MOUSE.exe): the M690's tables (DAT_0062f614 / PTR_DAT_0062f674), 9 shown
  (Btn1 2 3 6 7 8 5 4 10), photo skins/button_normal.bmp.
  All three answer as 062a:415e, so the device dialog lists them as candidates.
Key bytes (mo.rs): [1, code] function code, [0, n] the key's own function (2 DPI+, 3 DPI-,
4 fire key, 5 ALT, 6 CTRL, 7 SHIFT, 8 HOME).
"""
import json
import pathlib
import sys

from PIL import Image

R = pathlib.Path.home() / '.cache/redragon-all'
PKGS = [
    dict(name='M690', package='MIRAGE M690', dir='x/Mouses/MIRAGE M690/Redragon Wireless Gaming Mouse 2645 Setup.exe.x/app',
         vid=0x062a, pids=[0x415e], crop=(60, 20, 222, 340),
         table=[(800, 5), (1200, 4), (1600, 0), (2400, 1), (3200, 2), (4800, 3)],
         # slot, mark on the photo, default key bytes
         keys=[(0, (45, 75), [1, 0xb5, 0, 0]), (1, (126, 75), [1, 0xb6, 0, 0]), (2, (88, 93), [1, 0xb7, 0, 0]),
               (11, (15, 137), [1, 0xb9, 0, 0]), (10, (25, 180), [1, 0xb8, 0, 0]), (9, (20, 104), [0, 4, 0, 0]),
               (4, (88, 127), [0, 2, 0, 0]), (3, (88, 147), [0, 3, 0, 0])]),
    dict(name='M688', package='SHARK M688', dir='x/Mouses/SHARK M688/Monster Wireless Gaming Mouse 2613 Setup-1.0.0.1.exe.x/app',
         vid=0x062a, pids=[0x415e], crop=(50, 55, 230, 305),
         table=[(500, 5), (1000, 4), (2000, 0), (3000, 1), (4000, 2), (7200, 3)],
         keys=[(0, (56, 63), [1, 0xb5, 0, 0]), (1, (126, 49), [1, 0xb6, 0, 0]), (2, (92, 34), [1, 0xb7, 0, 0]),
               (5, (29, 86), [0, 5, 0, 0]), (6, (32, 116), [0, 6, 0, 0]), (7, (39, 151), [0, 7, 0, 0]),
               (9, (32, 41), [0, 4, 0, 0]), (4, (95, 65), [0, 2, 0, 0]), (3, (95, 80), [0, 3, 0, 0]),
               (8, (95, 96), [0, 8, 0, 0])]),
    dict(name='M691', package='MIST M691', dir='x/Mouses/MIST M691/Redragon M691 GAMING MOUSE Setup 1.0.1.0.exe.x/app',
         skin='button_normal.bmp', vid=0x062a, pids=[0x415e], crop=(45, 75, 232, 345),
         table=[(800, 5), (1200, 4), (1600, 0), (2400, 1), (3200, 2), (4800, 3)],
         keys=[(0, (51, 36), [1, 0xb5, 0, 0]), (1, (131, 44), [1, 0xb6, 0, 0]), (2, (94, 62), [1, 0xb7, 0, 0]),
               (5, (25, 103), [0, 5, 0, 0]), (6, (26, 136), [0, 6, 0, 0]), (7, (31, 175), [0, 7, 0, 0]),
               (4, (94, 102), [0, 2, 0, 0]), (3, (94, 122), [0, 3, 0, 0]), (9, (30, 52), [0, 4, 0, 0])]),
]


def main():
    res = []
    for p in PKGS:
        photo = R / 'emu' / f"mosart-{p['name'].lower()}.png"
        Image.open(R / p['dir'] / 'skins' / p.get('skin', 'button_normal.jpg')).convert('RGB').crop(p['crop']).save(photo)
        keys = [dict(index=s, x=x - 12, y=y - 12, w=24, h=24, type=b[0], p1=b[1], p2=b[2], code=b[3] << 24) for s, (x, y), b in p['keys']]
        res.append(dict(name=p['name'], package=p['package'], family='mosart', vid=p['vid'], pids=p['pids'], sensor='mosart',
                        table=p['table'], proto='Mosart', photo=str(photo), stages=5, keys=keys))
        print(f"{p['name']:10} {p['vid']:04x}:{p['pids'][0]:04x} {len(keys)} keys", file=sys.stderr)
    json.dump(res, open(sys.argv[1], 'w'), indent=1)


main()
