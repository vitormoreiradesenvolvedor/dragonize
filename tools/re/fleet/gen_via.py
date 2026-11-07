#!/usr/bin/env python3
"""QMK/VIA keyboards (Redragon ships only the VIA definition .json: K715 PRO, K717 PRO, K742,
K751, K754) -> keyboard entries in gen_kbd.py's format (proto "via"), for gen_models.py --kbd.
Usage: gen_via.py OUT.json PHOTO_DIR

The definition gives vendor/product id, matrix size, the KLE layout (each key labelled
"row,col", layout options in legend 3 = "group,choice": choice 0 is kept) and the lighting
menu (channel + value ids of brightness / effect / effect speed / colour, effect list). No
photo exists: one is drawn from the layout. No default keymap either (the keyboard reports it).
Protocol: kbd.rs `via_*`.
"""
import json
import pathlib
import sys

from PIL import Image, ImageDraw

R = pathlib.Path.home() / '.cache/redragon-all/x'
U = 40  # px per key unit in the drawn photo

# VIA's built-in "qmk_rgb_matrix" menu: effect names in id order (via/src/utils/... lighting presets)
QMK_RGB_MATRIX = [
    'All Off', 'Solid Color', 'Alphas Mods', 'Gradient Up Down', 'Gradient Left Right', 'Breathing',
    'Band Sat', 'Band Val', 'Band Pinwheel Sat', 'Band Pinwheel Val', 'Band Spiral Sat', 'Band Spiral Val',
    'Cycle All', 'Cycle Left Right', 'Cycle Up Down', 'Rainbow Moving Chevron', 'Cycle Out In',
    'Cycle Out In Dual', 'Cycle Pinwheel', 'Cycle Spiral', 'Dual Beacon', 'Rainbow Beacon',
    'Rainbow Pinwheels', 'Raindrops', 'Jellybean Raindrops', 'Hue Breathing', 'Hue Pendulum', 'Hue Wave',
    'Typing Heatmap', 'Digital Rain', 'Solid Reactive Simple', 'Solid Reactive', 'Solid Reactive Wide',
    'Solid Reactive Multi Wide', 'Solid Reactive Cross', 'Solid Reactive Multi Cross', 'Solid Reactive Nexus',
    'Solid Reactive Multi Nexus', 'Splash', 'Multi Splash', 'Solid Splash', 'Solid Multi Splash',
]


def kle(rows):
    """KLE rows -> [(x, y, w, h, row, col)] in key units, layout choice 0 only."""
    out, y = [], 0.0
    for row in rows:
        x, w, h = 0.0, 1.0, 1.0
        for it in row:
            if isinstance(it, dict):
                x += it.get('x', 0)
                y += it.get('y', 0)
                w, h = it.get('w', w), it.get('h', h)
                continue
            legends = it.split('\n')
            opt = legends[3] if len(legends) > 3 else ''
            if ',' in legends[0] and (not opt or opt.split(',')[1].strip() == '0'):
                r, c = (int(v) for v in legends[0].split(','))
                out.append((x, y, w, h, r, c))
            x += w
            w, h = 1.0, 1.0
        y += 1
    x0, y0 = min(k[0] for k in out), min(k[1] for k in out)
    return [(x - x0, y - y0, *k) for x, y, *k in out]


def lighting(menus):
    """-> (channel, {name: value id}, effects [(name, id)])"""
    if menus == ['qmk_rgb_matrix']:
        return 3, {'brightness': 1, 'effect': 2, 'speed': 3, 'color': 4}, list(zip(QMK_RGB_MATRIX, range(len(QMK_RGB_MATRIX))))
    ids, effects, ch = {}, [], 3

    def walk(n):
        nonlocal effects, ch
        if isinstance(n, dict):
            c = n.get('content')
            if isinstance(c, list) and c and isinstance(c[0], str) and c[0].startswith('id_'):
                name = c[0]
                ch = c[1]
                for k in ('brightness', 'effect_speed', 'effect', 'color'):
                    if name.endswith('_' + k):
                        ids['speed' if k == 'effect_speed' else k] = c[2]
                        if k == 'effect':
                            effects = [(o[0], o[1]) if isinstance(o, list) else (o, i) for i, o in enumerate(n.get('options', []))]
                        break
            for v in n.values():
                walk(v)
        elif isinstance(n, list):
            for v in n:
                walk(v)
    walk(menus)
    return ch, ids, effects


def draw(keys, path):
    W = max(x + w for x, y, w, h, *_ in keys) * U + 8
    H = max(y + h for x, y, w, h, *_ in keys) * U + 8
    im = Image.new('RGBA', (round(W), round(H)), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    d.rounded_rectangle((0, 0, im.width - 1, im.height - 1), 10, fill=(28, 28, 30, 255), outline=(70, 70, 74, 255))
    for x, y, w, h, *_ in keys:
        b = (4 + x * U + 2, 4 + y * U + 2, 4 + (x + w) * U - 3, 4 + (y + h) * U - 3)
        d.rounded_rectangle(b, 4, fill=(52, 52, 56, 255), outline=(96, 96, 100, 255))
        d.rounded_rectangle((b[0] + 3, b[1] + 2, b[2] - 3, b[3] - 5), 3, fill=(62, 62, 66, 255))
    im.save(path)


def main():
    out, photos = sys.argv[1], pathlib.Path(sys.argv[2])
    photos.mkdir(parents=True, exist_ok=True)
    seen, res = set(), []
    for f in sorted(R.rglob('*.json')):
        try:
            j = json.loads(f.read_text('utf-8-sig'))
        except (ValueError, UnicodeDecodeError):
            continue
        if not isinstance(j, dict) or 'vendorId' not in j or 'layouts' not in j:
            continue
        vid, pid = int(j['vendorId'], 16), int(j['productId'], 16)
        if (vid, pid) in seen:
            continue
        seen.add((vid, pid))
        name = f.stem.split('_')[-1].upper() if j['name'] in ('Gaming Keyboard',) or j['name'].islower() else j['name']
        name = {'K717 PRO': 'K717-PRO'}.get(f.stem.replace('ALCOR ', ''), name) if 'K717' in f.stem else name
        if 'K715' in f.stem:
            name = 'K715-PRO'
        rows, cols = j['matrix']['rows'], j['matrix']['cols']
        ks = kle(j['layouts']['keymap'])
        ch, ids, effects = lighting(j.get('menus', []))
        photo = photos / f'via-{pid:04x}.png'
        draw(ks, photo)
        keys = [dict(x=round(4 + x * U), y=round(4 + y * U), w=round(w * U), h=round(h * U), matrix=r * cols + c,
                     led=None, code=None, kind=0, vk=0) for x, y, w, h, r, c in ks]
        leds = [dict(n=i + 1, hw=e[1], effect=0, light='brightness' in ids, speed='speed' in ids,
                     color='color' in ids and e[1] != 0, random=False, rgb=False, direct=False, name=e[0])
                for i, e in enumerate(effects)]
        res.append(dict(name=name, psd=[0] * 6, family='via', build='', vid=vid, pid=pid,
                        glen=8 + 0x16 + 2 * len(leds), pro=0, key=0, rgb=0, rgb_read=False, led_off=4,
                        keys=keys, leds=leds, light=[0, 1, 2, 3, 4], speed=[0, 1, 2, 3, 4], rgbi=[0, 1, 2],
                        random=15, photo=str(photo), exe='', proto='via',
                        via=dict(cols=cols, ch=ch, bright=ids.get('brightness', 0), effect=ids.get('effect', 0),
                                 speed=ids.get('speed', 0), color=ids.get('color', 0)),
                        package=str(f.relative_to(R).parts[1])))
        print(f"{name:12} {vid:04x}:{pid:04x} {rows}x{cols} {len(keys)} keys {len(leds)} effects ch {ch} {ids}")
    json.dump(res, open(out, 'w'), indent=1)


main()
