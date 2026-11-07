#!/usr/bin/env python3
"""PixArt mice of the Qt drivers with PXICtrl3169.dll (M709-1, 093a:2534) -> mouse entries in
gen_compx.py's format (proto "Pixart"), for gen_models.py --compx. Usage: gen_pxi.py OUT.json

Per package: res/setting.ini [PixArt_USBHID_Setting] USB_VID / USB_PID, [Buttons] ButtonCount;
res/config.json Buttons.Pos_Scale_X / _Y = each button's place on res/image/mouse/Device.png
(cropped to its content, 320 px high). Default buttons: res/json/Profile_Defaut.json (Group,
Field) through the driver's maps (pxi.rs). DPI 100..12800 in steps of 100, register = cpi / 100 - 1.
Protocol: pxi.rs.
"""
import configparser
import json
import pathlib
import sys

from PIL import Image

R = pathlib.Path.home() / '.cache/redragon-all'
H = 320
# (Group, Field) -> 13-bit code (DAT_100cc0d0 .. e8; buttons 0-2 are fixed: 0xff, n)
CODES = {(1, 4): 0x1800, (1, 5): 0x1801, (4, 0x33): 0x1809, (4, 0x34): 0x180a, (4, 0x35): 0x1808, (0, 0): 0}


def main():
    res = []
    for dll in sorted((R / 'x').rglob('PXICtrl3169.dll')):
        app = dll.parent
        ini = configparser.ConfigParser()
        ini.read(app / 'res/setting.ini', encoding='utf-8-sig')
        vid, pid = (int(ini['PixArt_USBHID_Setting'][k], 16) for k in ('USB_VID', 'USB_PID'))
        n = int(ini['Buttons']['ButtonCount'])
        pos = json.load(open(app / 'res/config.json'))['Buttons']
        prof = json.load(open(app / 'res/json/Profile_Defaut.json'))['Profiles'][0]['Buttons']
        img = Image.open(app / 'res/image/mouse/Device.png').convert('RGBA')
        x0, y0, x1, y1 = img.getbbox()
        s = H / (y1 - y0)
        name = app.name.split()[-1]
        photo = R / 'emu' / f'pxi-{name.lower()}.png'
        img.crop((x0, y0, x1, y1)).resize((round((x1 - x0) * s), H), Image.LANCZOS).save(photo)
        keys = []
        for i in range(n):
            x = (pos['Pos_Scale_X'][i] * img.width - x0) * s
            y = (pos['Pos_Scale_Y'][i] * img.height - y0) * s
            c = (0xff | i << 8) if i < 3 else CODES[(prof[i]['Group'], prof[i]['Field'])]
            keys.append(dict(index=i, x=round(x) - 12, y=round(y) - 12, w=24, h=24, type=c & 0xff, p1=c >> 8, p2=0, code=0))
        table = [(c, c // 100 - 1) for c in range(100, 12801, 100)]
        res.append(dict(name=name, package=app.name, family='pixart', vid=vid, pids=[pid], sensor='pxi',
                        table=table, proto='Pixart', photo=str(photo), stages=5, keys=keys))
        print(f'{name:10} {vid:04x}:{pid:04x} {n} keys  {app.name}', file=sys.stderr)
    json.dump(res, open(sys.argv[1], 'w'), indent=1)


main()
