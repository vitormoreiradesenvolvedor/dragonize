#!/usr/bin/env python3
"""BYDUI mice (DeviceDriver.exe + BYDUI.dll, with the driver's .pdb) -> mouse entries in
gen_compx.py's format (proto "Bydui"), for gen_models.py --compx. Usage: gen_bd.py OUT.json

Per package (inno.x/app/): config.xml <mouse> with its <mode> ids (value 0 USB, 1 the 2.4 GHz
receiver), device/<device_attr>.xml <mouse_key> keys (key_value = the key's slot, func_value =
its default; rect = the callout line in dialog coordinates), the photo skins/theme1/<image_file>.
Default key bytes = SetMouseProfile's built-in table (5 buttons + the DPI button under the
wheel, slot 5); marks = the buttons on the photo (picked by hand: the callouts point at the
dialog, whose photo offset is not in the files). DPI: 50..26000 in steps of 50, register =
cpi / 50 - 1 (SetMouseDPIData; 30000 on the ic_type 2 chip). Protocol: bd.rs.
"""
import json
import pathlib
import re
import sys

R = pathlib.Path.home() / '.cache/redragon-all/x'
DEFAULTS = [[1, 1, 1, 0], [1, 2, 1, 0], [1, 3, 1, 0], [1, 5, 1, 0], [1, 4, 1, 0], [5, 4, 0, 0]]
# slot -> centre on the photo, per photo file
MARKS = {'mouse_M998.png': {0: (60, 45), 1: (178, 45), 2: (126, 62), 3: (18, 158), 4: (18, 128), 5: (126, 138)}}


def attrs(s):
    return dict(re.findall(r'(\w+)\s*=\s*"([^"]*)"', s))


def main():
    res = []
    for exe in sorted(R.rglob('DeviceDriver.exe')):
        app = exe.parent
        if not (app / 'BYDUI.dll').exists():
            continue
        cfg = re.sub(r'<!--.*?-->', '', (app / 'config.xml').read_text('utf-8-sig', 'replace'), flags=re.S)
        m = re.search(r'<mouse\s([^>]*)>(.*?)</mouse>', cfg, re.S)
        mouse, modes = attrs(m.group(1)), [attrs(x) for x in re.findall(r'<mode\s([^>]*)/>', m.group(2))]
        dev = re.sub(r'<!--.*?-->', '', (app / 'device' / (mouse['device_attr'] + '.xml')).read_text('utf-8-sig', 'replace'), flags=re.S)
        img = attrs(re.search(r'<image_file\s([^>]*)/>', dev).group(1))['img'].replace('\\', '/')
        slots = sorted({int(attrs(k)['key_value']) for k in re.findall(r'<key\s([^>]*)/>', dev)} | {5})
        marks = MARKS[pathlib.Path(img).name]
        keys = []
        for s in slots:
            t, p1, p2, p3 = DEFAULTS[s]
            x, y = marks[s]
            keys.append(dict(index=s, x=x - 15, y=y - 15, w=30, h=30, type=t, p1=p1, p2=p2, code=p3 << 24))
        name = re.sub(r'\s+', '-', mouse['name'].strip().upper())
        pids = [int(x['pid'], 16) for x in sorted(modes, key=lambda x: int(x['value']))]
        res.append(dict(name=name, package=app.relative_to(R).parts[1], family='bydui', vid=int(modes[0]['vid'], 16), pids=pids,
                        sensor='bd', table=[(c, c // 50 - 1) for c in range(50, 26001, 50)], proto='Bydui',
                        photo=str(app / 'skins/theme1' / img), stages=5, keys=keys))
        print(f"{name:12} {res[-1]['vid']:04x}:{' '.join('%04x' % p for p in pids)} {len(keys)} keys  {res[-1]['package']}", file=sys.stderr)
    json.dump(res, open(sys.argv[1], 'w'), indent=1)


main()
