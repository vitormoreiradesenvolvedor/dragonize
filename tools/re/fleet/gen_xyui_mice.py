#!/usr/bin/env python3
"""xyui mice (DeviceDriver.exe + XYUI.dll, 1a2c:xxxx) -> mouse entries in gen_compx.py's format
(proto "Xyui"), for gen_models.py --compx. Usage: gen_xyui_mice.py OUT.json

Per package (app/): config.xml's <mouse> products (type 601.. wired, 701.. the 2.4 GHz dongle
1a2c:8fff), layouts/<layout_file>.xml's <mouse_key> keys (key_value = the key's id on the wire,
func_value = its default function; rect = "x1#x2#y" of the callout line, the end on the mouse
being x2 for direction 0 and x1 for direction 1, in dialog coordinates: the photo sits at
PHOTO_AT), the photo skins/theme1/mouse/<mouse_image>. DPI: 100..12800 in steps of 100
(register = cpi / 100 - 1, DAT_005671c0). Protocol: xy.rs, NOTES.md "xyui family".
"""
import json
import pathlib
import re
import sys

R = pathlib.Path.home() / '.cache/redragon-all/x'
PHOTO_AT = (65, 5)
# the layouts give the DPI keys func_value 5 (forward): their default is told by the label
# (func_desc, language/1033.lan; 833 is "DPI+" in some builds, "DPI cycle" in others)
DPI_FUNC = {'DPI+': 0xd5, 'DPI-': 0xd6, 'DPI cycle': 0xd8, 'DPI': 0xd8}


def attrs(s):
    return dict(re.findall(r'(\w+)\s*=\s*"([^"]*)"', s))


def model_name(name):
    m = re.search(r'\b([MS]\d{3}[A-Z0-9]*(?:-[A-Z0-9]+)*|BM-\d+)', name.upper())
    return m.group(1) if m else name.strip()


def main():
    res, seen = [], {}
    for cfg in sorted(R.rglob('config.xml')):
        app = cfg.parent
        t = re.sub(r'<!--.*?-->', '', cfg.read_text('utf-8', 'replace'), flags=re.S)
        mice = [attrs(m) for m in re.findall(r'<mouse\s([^>]*)/>', t)]
        if not mice:
            continue
        lay = app / 'layouts' / (mice[0]['layout_file'] + '.xml')
        if not lay.exists():
            print('skip (no layout)', app, file=sys.stderr)
            continue
        lt = re.sub(r'<!--.*?-->', '', lay.read_text('utf-8', 'replace'), flags=re.S)
        img = attrs(re.search(r'<mouse_image\s([^>]*)/>', lt).group(1))['file_name']
        photo = app / 'skins/theme1/mouse' / img
        lan = app / 'language/1033.lan'
        lan = dict(re.findall(r'^(\d+)=(.*?)\s*$', lan.read_bytes().decode('utf-16', 'replace'), re.M)) if lan.exists() else {}
        keys = []
        for k in re.findall(r'<key\s([^>]*)/>', lt.split('<mouse_key>')[1].split('</mouse_key>')[0]):
            a = attrs(k)
            x1, x2, y = (int(v) for v in a['rect'].split('#'))
            x = (x2 if a['direction'] == '0' else x1) - PHOTO_AT[0]
            func = DPI_FUNC.get(lan.get(a.get('func_desc'), ''), int(a['func_value']))
            keys.append(dict(index=int(a['key_value']), x=x - 15, y=y - PHOTO_AT[1] - 15, w=30, h=30, type=func, p1=0, p2=0))
        wired = [int(m['pid'], 16) for m in mice if int(m['type']) < 700]
        pids = wired + [p for p in (int(m['pid'], 16) for m in mice if int(m['type']) >= 700) if p not in wired]
        name = model_name(mice[0]['name'])
        pkg = str(app.relative_to(R)).split('/')[1]
        sig = (tuple(pids), tuple((k['index'], k['x'], k['y'], k['type']) for k in keys))
        if sig in seen:  # same mouse, another package / combo
            e = seen[sig]
            if name not in e['name'].split(' / ') and e['name'].count(' / ') < 3:
                e['name'] += ' / ' + name
            if pkg not in e['package']:
                e['package'] += ', ' + pkg
            continue
        e = dict(name=name, package=pkg, family='xyui', vid=int(mice[0]['vid'], 16), pids=pids, sensor='xy',
                 table=[(c, c // 100 - 1) for c in range(100, 12801, 100)], proto='Xyui', photo=str(photo), stages=5, keys=keys)
        seen[sig] = e
        res.append(e)
    for e in res:
        print(f"{e['name']:44} {' '.join('%04x' % p for p in e['pids']):10} {len(e['keys'])} keys  {e['package']}")
    json.dump(res, open(sys.argv[1], 'w'), indent=1)
    print(len(res), 'xyui mice', file=sys.stderr)


main()
