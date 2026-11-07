#!/usr/bin/env python3
"""duilib keyboards (DuiLib drivers, Evision firmware 320f:xxxx) -> keyboard entries in
gen_kbd.py's format (proto "du"), for gen_models.py --kbd. Usage: gen_duilib_kb.py OUT.json

Per package (app/): the exe's device table (gen_duilib.py `rows`; class 1 = wired keyboard,
9 / 0xa = 2.4 GHz), the photo Skin/Kb/KbKey_normal.png with the visible key_<n> buttons of
Skin/XML/DeviceXml/keyboarddevice.xml (n = index in the key table), each key's default code
from DefaultData/Keyboard.json KeyList[n].DefaultAssignment, the lighting modes of
default_light.json group 0 (value = the mode byte; per-key custom, music and screen-colour
modes left out). Protocol: du.rs, kbd.rs `du_*`, NOTES.md "duilib".
"""
import json
import pathlib
import re
import sys

R = pathlib.Path.home() / '.cache/redragon-all/x'
SKIP = re.compile(r'unins|update|audiorecord|upgrade', re.I)
NOT_MODES = {'custom', 'audiorecord', 'capturecolor', 'off'}


def rows(exe):
    src = pathlib.Path(__file__).with_name('gen_duilib.py').read_text()
    ns = {}
    exec(src.split('\ndef model_name')[0], ns)  # imports + rows(), without running main()
    return ns['rows'](exe)


def model_name(pkg):
    m = re.search(r'K\d{3}[A-Z0-9]*(?:-[A-Z0-9]+)*', pkg.upper())
    return m.group(0) if m else pkg


def keys(skin):
    t = re.sub(r'<!--.*?-->', '', (skin / 'XML/DeviceXml/keyboarddevice.xml').read_text('utf-8', 'replace'), flags=re.S)
    # some skins repeat the layout per legend set (ru / kr / sp photos): the first one only
    t = t.split('keyboard_device_key_light_1')[0]
    out = []
    for m in re.finditer(r'<DeviceKeyButton name="key_(\d+)"([^>]*)>', t):
        a = dict(re.findall(r'(\w+)="([^"]*)"', m.group(2)))
        if a.get('visible') == 'false' or not a.get('pos'):
            continue
        x, y = (int(v) for v in a['pos'].split(',')[:2])
        w, h = int(a.get('width', 0)), int(a.get('height', 0))
        if w > 2 and h > 2:
            out.append((int(m.group(1)), x, y, w, h))
    return sorted(out)


def main():
    res, seen = [], {}
    for kj in sorted(R.rglob('DefaultData/Keyboard.json')):
        app = kj.parent.parent
        pkg = str(app.relative_to(R))
        if not pkg.startswith('Teclados') or not (app / 'Skin/Kb/KbKey_normal.png').exists():
            continue
        exe = next((e for e in sorted(app.glob('*.exe')) if not SKIP.search(e.name) and b'DuiLib' in e.read_bytes()), None)
        if not exe:
            print('skip', pkg, file=sys.stderr)
            continue
        pids = []
        for r in sorted(rows(exe), key=lambda r: r[4] != 1):
            if r[7] == 0xff1c and r[4] in (1, 9, 0xa) and r[1] and r[1] not in pids:
                pids.append(r[1])
        if not pids:
            print('skip (no id)', pkg, file=sys.stderr)
            continue
        dev = next(d for d in json.loads(kj.read_text('utf-8', 'replace'))['Device'] if d['DeviceBasicInfo']['DeviceType'] == 'keyboard')
        codes = [k['DefaultAssignment'] for k in dev['KeyList']]
        code = lambda n: (codes[n] >> 16 & 0xff | (codes[n] >> 8 & 0xff) << 8 | (codes[n] & 0xff) << 16) if n < len(codes) else None
        ks = [dict(x=x, y=y, w=w, h=h, matrix=n, led=None, code=code(n), kind=0, vk=0) for n, x, y, w, h in keys(app / 'Skin')]
        groups = json.loads((app / 'DefaultData/default_light.json').read_text('utf-8', 'replace'))['DefaultLightName']
        modes = [(e['value'], e['LightName'][len('light_mode_'):-len('_text')]) for e in groups[0] if e['visible']]
        modes = [(v, n) for v, n in modes if n not in NOT_MODES]
        leds = [dict(n=i + 1, hw=v, effect=0, light=True, speed=n != 'static', color=True, random=n != 'static',
                     rgb=False, direct=False, name=n.replace('_', ' ').title()) for i, (v, n) in enumerate(modes)]
        top = pkg.split('/')[1]
        name = model_name(top)
        sig = (tuple(pids), tuple((k['matrix'], k['x'], k['y']) for k in ks), tuple(modes))
        if sig in seen:  # same keyboard, another package / colour
            e = seen[sig]
            if e['name'].count(' / ') < 1 and name not in e['name']:
                e['name'] += ' / ' + name
            e['package'] += ', ' + top
            continue
        # general block: lighting pairs as the other keyboards, then the light block as read
        e = dict(name=name, psd=[0] * 6, family='duilib-kb', build='', vid=0x320f, pid=pids[0], alt_pids=pids[1:],
                 glen=8 + 0x16 + 2 * len(leds) + 0x2d, pro=0, key=0, rgb=0, rgb_read=False, led_off=4,
                 keys=ks, leds=leds, light=[0, 1, 2, 3, 4, 5], speed=[0, 1, 2, 3, 4], rgbi=[0, 1, 2], random=7,
                 photo=str(app / 'Skin/Kb/KbKey_normal.png'), exe='', proto='du', package=top)
        seen[sig] = e
        res.append(e)
    for e in res:
        print(f"{e['name']:32} {e['pid']:04x} {' '.join('%04x' % p for p in e['alt_pids']):10} {len(e['keys']):3} keys {len(e['leds']):2} modes  {e['package']}")
    json.dump(res, open(sys.argv[1], 'w'), indent=1)
    print(len(res), 'duilib keyboards', file=sys.stderr)


main()
