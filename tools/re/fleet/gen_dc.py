#!/usr/bin/env python3
"""libdc mice (Qt + CEF app qcef.exe, HTML UI in view/, device access libdc.dll; M805, M906,
04d9:b0d2) -> mouse entries in gen_compx.py's format (proto "Libdc"), for gen_models.py --compx.
Usage: gen_dc.py OUT.json

Per package: view/xml/driver.xml (property ids and function codes, identical in both),
view/xml/configuration.xml mode 0 = the default buttons (contextId buttonN = property
pButtonN), the DPI slider range of view/js/js.js (M906 250..4000, M805 100..14400), photo
view/img/mouse.png (cropped to its content, 320 px high). Marks: picked by hand on that photo
(the UI's callouts .m1.msbg>.nI follow the order of the mode's <key> list: L R wheel, side
buttons, the buttons behind the wheel). DPI register = ceil(cpi / 50) (getDpiValue). Protocol: dc.rs.
"""
import json
import pathlib
import re
import sys

from PIL import Image

R = pathlib.Path.home() / '.cache/redragon-all'
H = 320
PKGS = [
    dict(name='M906', package='M906', dir='x/Mouses/M906', lo=250, hi=4000, step=50,
         marks={0: (95, 90), 1: (215, 90), 2: (165, 125), 3: (165, 190), 4: (165, 240),
                5: (290, 215), 6: (290, 285), 7: (40, 215), 8: (40, 285)}),
    dict(name='M805', package='Hydra M805', dir='x/Mouses/Hydra M805/Hydra M805/setup.exe.x', lo=100, hi=14400, step=100,
         marks={0: (85, 130), 1: (225, 120), 2: (150, 110), 3: (40, 160), 4: (148, 215), 5: (148, 252),
                6: (148, 290), 7: (25, 260), 8: (25, 310), 9: (25, 360)}),
]
# marks above are on the photo scaled to 520 px high
MARK_H = 520


def attrs(s):
    return dict(re.findall(r'(\w+)="([^"]*)"', s))


def main():
    res = []
    for p in PKGS:
        view = R / p['dir'] / 'view'
        drv = re.sub(r'<!--.*?-->', '', (view / 'xml/driver.xml').read_text('utf-8'), flags=re.S)
        codes = {a['task']: a['code'] for a in map(attrs, re.findall(r'<key\s([^>]*)>', drv)) if 'code' in a and 'task' in a}
        conf = (view / 'xml/configuration.xml').read_text('utf-8')
        mode0 = re.search(r'<mode index="0".*?</mode>', conf, re.S).group(0)
        img = Image.open(view / 'img/mouse.png').convert('RGBA')
        img = img.crop(img.getbbox())
        w = round(img.width * H / img.height)
        photo = R / 'emu' / f"dc-{p['name'].lower()}.png"
        img.resize((w, H), Image.LANCZOS).save(photo)
        s = H / MARK_H
        keys = []
        for k in map(attrs, re.findall(r'<key\s([^>]*)/>', mode0)):
            slot = int(k['contextId'].removeprefix('button'))
            if k['taskType'] == 'hotKeys':
                mods = {'Ctrl': 1, 'Shift': 2, 'Alt': 4, 'Win': 8}[k['task']]
                b = [2, mods, 0, 0]
            elif k['task'] in codes and k['task'].count('|') == 0:
                b = [int(x, 16) for x in codes[k['task']].split(',')]
            else:  # multiKey = a macro of the mouse's own: kept as raw bytes
                b = [4, 0, 0, 0]
            x, y = p['marks'][slot]
            keys.append(dict(index=slot, x=round(x * s) - 12, y=round(y * s) - 12, w=24, h=24, type=b[0], p1=b[1], p2=b[2], code=b[3] << 24))
        keys.sort(key=lambda k: k['index'])
        table = [(c, -(-c // 50)) for c in range(p['lo'], p['hi'] + 1, p['step'])]
        res.append(dict(name=p['name'], package=p['package'], family='libdc', vid=0x04d9, pids=[0xb0d2], sensor='dc',
                        table=table, proto='Libdc', photo=str(photo), stages=5, keys=keys))
        print(f"{p['name']:10} 04d9:b0d2 {len(keys)} keys {table[0][0]}-{table[-1][0]}", file=sys.stderr)
    json.dump(res, open(sys.argv[1], 'w'), indent=1)


main()
