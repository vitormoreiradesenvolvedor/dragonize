#!/usr/bin/env python3
"""Shinetek (SOUI) keyboards K513-RGB / K512-RGB / K515-RGB -> keyboard entries in gen_kbd.py's
format (proto "xy", st), for gen_models.py --kbd. Usage: gen_shinetek.py OUT.json

The K513 build (ghidra/out/shinetek-k513.c) speaks the xyui keyboards' feature report 7:
07 f7 light (FUN_00420520), 07 fa key slots (FUN_0041f6xx), 07 fb read slots, 07 f1 macros,
07 fc profile, 07 f0 aa reset; the K512 / K515 builds carry the same six commands.
Per package (advinst.x/cab/): config.xml <usb vid pid>, positions.xml <pos rect="x,y,w,h" id=HID
usage> on img_keboard_normal.png, dlg_main.xml cmb_led_mode items (data = mode + 1; Customize and
the per-key mode left out). Light struct per mode (FUN_0041fcc0): colour radio 1-7, direction 1/2,
speed 0-3, brightness 0-3.
"""
import json
import pathlib
import re
import sys

R = pathlib.Path.home() / '.cache/redragon-all/x/Teclados'
PKGS = ['K513-RGB', 'Shiva K512-RGB', 'SHIVA K515-RGB']
NOT_MODES = {'Customize'}


def main():
    res = []
    for pkg in PKGS:
        c = R / pkg / 'advinst.x/cab'
        cfg = (c / 'config.xml').read_text('utf-8-sig', 'replace')
        usb = dict(re.findall(r'(\w+)="([^"]*)"', re.search(r'<usb\s([^>]*)/>', cfg).group(1)))
        pos = re.sub(r'<!--.*?-->', '', (c / 'positions.xml').read_text('utf-8-sig', 'replace'), flags=re.S)
        keys = []
        for p in re.findall(r'<pos\s([^>]*)/>', pos):
            a = dict(re.findall(r'(\w+)\s*=\s*"([^"]*)"', p))
            x, y, w, h = (int(v) for v in a['rect'].replace(' ', '').split(','))
            u = int(a['id'])
            if w > 2 and h > 2 and u < 0xe8:
                keys.append(dict(x=x, y=y, w=w, h=h, matrix=u, led=None, code=u, kind=0, vk=0))
        dlg = re.sub(r'<!--.*?-->', '', (c / 'dlg_main.xml').read_text('utf-8-sig', 'replace'), flags=re.S)
        items = re.findall(r'<item text="([^"]*)" icon="\d+" data="(\d+)"', dlg.split('cmb_led_mode')[1].split('</items>')[0])
        leds = [dict(n=i + 1, hw=int(d), effect=0, light=True, speed=t != 'Often bright', color=True, random=False,
                     rgb=False, direct=False, name=t) for i, (t, d) in enumerate((t, d) for t, d in items if t not in NOT_MODES)]
        name = re.search(r'K\d{3}[A-Z0-9-]*', pkg.upper()).group(0)
        res.append(dict(name=name, psd=[0] * 6, family='shinetek', build='', vid=int(usb['vid'], 16), pid=int(usb['pid'], 16),
                        glen=8 + 0x16 + 2 * len(leds), pro=0, key=0, rgb=0, rgb_read=False, led_off=4, keys=keys, leds=leds,
                        light=[0, 1, 2, 3], speed=[0, 1, 2, 3], rgbi=[0, 1, 2], random=7,
                        photo=str(c / 'img_keboard_normal.png'), exe='', proto='xy', st=True, package=pkg))
        print(f"{name:12} {res[-1]['vid']:04x}:{res[-1]['pid']:04x} {len(keys)} keys {len(leds)} modes", file=sys.stderr)
    json.dump(res, open(sys.argv[1], 'w'), indent=1)


main()
