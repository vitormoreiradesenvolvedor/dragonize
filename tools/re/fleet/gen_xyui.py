#!/usr/bin/env python3
"""xyui keyboards (DeviceDriver.exe + XYUI.dll, 1a2c:xxxx) -> keyboard entries in gen_kbd.py's
format (proto "xy"), for gen_models.py --kbd. Usage: gen_xyui.py OUT.json

Per package: config.xml (product name, device_info = layout, wired mode 0 vid/pid),
layouts/<layout>.xml (<light> modes, <key code rect>), skins/theme1/keyboard/<image>,
language/1033.lan (UTF-16 INI, first occurrence of an id wins, as GetPrivateProfileString).
Protocol and the meaning of the fields: NOTES.md "xyui keyboards", kbd.rs `xy_*`.
"""
import json
import pathlib
import re
import sys

R = pathlib.Path.home() / '.cache/redragon-all/x'


def lan(path):
    out = {}
    if not path.exists():
        return out
    raw = path.read_bytes()
    t = raw.decode('utf-16' if raw[:2] in (b'\xff\xfe', b'\xfe\xff') else 'utf-8', 'replace')
    for line in t.splitlines():
        m = re.match(r'\s*(\d+)\s*=(.*)', line)
        if m and int(m.group(1)) not in out:
            out[int(m.group(1))] = m.group(2).strip()
    return out


def attr(tag, name):
    m = re.search(rf'\b{name}\s*=\s*"([^"]*)"', tag)
    return m.group(1) if m else None


def strip_comments(t):
    return re.sub(r'<!--.*?-->', '', t, flags=re.S)


def entry(cfg):
    app = cfg.parent
    t = strip_comments(cfg.read_text('utf-8', 'replace'))
    kb = re.search(r'<keyboard\s[^>]*>', t)
    if not kb:
        return None, 'no keyboard'
    name = attr(kb.group(0), 'name') or ''
    name = re.sub(r'^Redragon\s+|\s+(GAMING\s+)?KEYBOARD$', '', re.sub(r'\s+', ' ', name).strip(), flags=re.I).strip()
    layout = attr(kb.group(0), 'device_info')
    modes = re.findall(r'<mode\s[^>]*>', t[kb.end():t.find('</keyboard>', kb.end())])
    wired = next((m for m in modes if attr(m, 'value') == '0'), None)
    if not wired or not layout:
        return None, 'no wired mode'
    vid, pid = int(attr(wired, 'vid'), 16), int(attr(wired, 'pid'), 16)
    lx = app / 'layouts' / f'{layout}.xml'
    if not lx.exists():
        return None, f'no layout {layout}'
    L = strip_comments(lx.read_text('utf-8', 'replace'))
    names = lan(app / 'language' / '1033.lan')
    light = re.search(r'<light\s[^>]*>', L)
    bmax = int(attr(light.group(0), 'brightness_max') or 4) if light else 4
    smax = int(attr(light.group(0), 'speed_max') or 4) if light else 4
    leds = []
    for info in re.findall(r'<info\s[^>]*>', L):
        mode, a = int(attr(info, 'mode')), int(attr(info, 'attribute') or 0)
        nm = names.get(int(attr(info, 'name') or 0), f'Mode {mode}')
        # attribute (FUN_0041dcf0 control visibility): 1 brightness, 2 speed, 4 colours +
        # colourful, 8 colours, 0x10 / 0x80 one RGB colour, 0x40 direction, 0x800 per-key, 0x1000 music
        leds.append(dict(n=len(leds) + 1, hw=mode, effect=a, light=bool(a & 1), speed=bool(a & 2),
                         color=bool(a & 0x9c), random=bool(a & 4), rgb=bool(a & 0x90), direct=False, name=nm))
    kbtag = re.search(r'<Keyboard\s[^>]*>', L)
    image = attr(kbtag.group(0), 'image') if kbtag else None
    photo = app / 'skins' / 'theme1' / 'keyboard' / image if image else None
    keys = []
    for k in re.findall(r'<key\s[^>]*>', L):
        code, rect = attr(k, 'code'), attr(k, 'rect')
        if not code or not rect:
            continue
        x, y, w, h = (int(v.strip()) for v in rect.split('#')[:4])
        u = int(code, 0)
        if not 0 < u < 0x100:
            continue
        keys.append(dict(x=x, y=y, w=w, h=h, matrix=u, led=None, code=u, kind=0, vk=0))
    if not keys:
        return None, 'no keys'
    return dict(name=name, psd=[0] * 6, family='xyui', build='', vid=vid, pid=pid,
                glen=8 + 0x16 + 2 * len(leds), pro=0, key=0, rgb=0, rgb_read=False, led_off=4,
                keys=keys, leds=leds, light=list(range(bmax)), speed=list(range(smax)), rgbi=[0, 1, 2],
                random=7, photo=str(photo) if photo and photo.exists() else None, exe=str(app / 'DeviceDriver.exe'),
                proto='xy', package=str(cfg.relative_to(R).parts[1])), None


def main():
    out, seen = [], set()
    for cfg in sorted(R.rglob('config.xml')):
        if not (cfg.parent / 'DeviceDriver.exe').exists():
            continue
        e, why = entry(cfg)
        if e is None:
            print('skip', cfg.relative_to(R).parts[1], why, file=sys.stderr)
            continue
        sig = (e['name'], e['vid'], e['pid'])
        if sig in seen:
            continue
        seen.add(sig)
        out.append(e)
        print(f"{e['name']:28} {e['vid']:04x}:{e['pid']:04x} {len(e['keys'])} keys {len(e['leds'])} modes "
              f"{'photo' if e['photo'] else 'NO PHOTO'}")
    json.dump(out, open(sys.argv[1], 'w'), indent=1)
    print(len(out), 'xyui keyboards', file=sys.stderr)


main()
