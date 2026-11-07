#!/usr/bin/env python3
"""Builds src/models_data.rs and assets/vendor/mice/*.png from the RDCfg.exe
("REDRAGON Gaming Mouse.exe") analysis.

Inputs (produced by README.md steps, outside the repo):
  WORK/x/userappdata/REDRAGON Gaming Mouse/Config.ini   product list (VID/PID/bcd/name)
  WORK/out/emu_all.tsv       EmuModels: 431a30 (LED type), 4314d0 (sensor), 406780 (layout)
  WORK/out/emu_btn.tsv       EmuModels: 45d5c0 (button slot -> address)
  WORK/out/emu_dpi.tsv       EmuModels: 436d40 (DPI slider range)
  WORK/out/emu_dpi_tab.tsv   EmuDpi: index -> (CPI, register) per sensor
  WORK/x/app/skin/mouse/*.png
usage: gen_models.py WORK
       gen_models.py --cfg CONFIG_INI --emu EMU_DIR --skin SKIN_MOUSE_DIR --out FILE.rs --img DIR [--extra singles.json] [--sino sino.json]
           [--compx compx.json]
         (another RDCfg build, tools/re/fleet/emu_rdcfg_build.sh)
"""
import json
import os
import re
import sys
from collections import defaultdict

from PIL import Image

APP = os.path.join(os.path.dirname(__file__), '..', '..', '..')
if sys.argv[1].startswith('--'):
    opt = dict(zip(sys.argv[1::2], sys.argv[2::2]))
    CFG, EMU, SKIN, OUT_RS, OUT_IMG = opt['--cfg'], opt['--emu'], opt['--skin'], opt['--out'], opt['--img']
    EXTRA = opt.get('--extra')
    SINO = opt.get('--sino')
    COMPX = opt.get('--compx')
    KBD = opt.get('--kbd')
else:
    WORK = sys.argv[1]
    CFG = f'{WORK}/x/userappdata/REDRAGON Gaming Mouse/Config.ini'
    EMU = f'{WORK}/out'
    SKIN = f'{WORK}/x/app/skin/mouse'
    OUT_RS = os.path.join(APP, 'src', 'models_data.rs')
    OUT_IMG = os.path.join(APP, 'assets', 'vendor', 'mice')
    EXTRA = None
    SINO = None
    COMPX = None
    KBD = None
BOX = (189, 270)  # mouse picture box of the general page (same as the M801 photo)
# RDCfg shows the photo scaled to fit 300x300, centred in a 661x262 label at (287, 56)
LABEL = (287, 56, 661, 262)

cfg = open(CFG, encoding='utf-8').read()
kv = dict(re.findall(r'^(\w+)=(.*?)\r?$', cfg, re.M))
products = []
for i in range(int(kv['Count'])):
    products.append(dict(name=kv[f'PRODUCT_NAME_{i}'], vid=int(kv[f'VID_{i}']), pid=int(kv[f'PID_{i}']),
                         bcd=(int(kv[f'VER_MIN_{i}']), int(kv[f'VER_MAX_{i}']))))


def sections(path):
    """model -> function -> list of rows (split on tabs)"""
    out = defaultdict(lambda: defaultdict(list))
    cur = None
    for line in open(path):
        f = line.rstrip('\n').split('\t')
        if f[1] == 'BEGIN':
            cur = f[2]
            continue
        out[f[0]][cur].append(f)
    return out


def hexarg(s):
    return int(s.split('=')[0], 16)


emu = sections(f'{EMU}/emu_all.tsv')
led, sensor, markers = {}, {}, {}
for m, fns in emu.items():
    for f in fns['431a30']:
        if f[1] == 'RET':
            led[m] = int(f[2], 16)
    for f in fns['4314d0']:
        if f[1] == 'RET':
            sensor[m] = int(f[2], 16)
    mk = {}
    for f in fns['406780']:
        if f[2] == 'FUN_00490870':
            a = [hexarg(x) for x in f[4:12]]
            wid = a[0] - 0x20000000
            if 0x204 <= wid < 0x2ac:
                mk[wid] = tuple(a[3:7])
    markers[m] = mk

# button slot (config index) -> address relative to the profile base
buttons = defaultdict(list)
pend = {}
for line in open(f'{EMU}/emu_btn.tsv'):
    f = line.rstrip('\n').split('\t')
    if f[1] in ('BEGIN', 'END', 'RET'):
        continue
    if f[2] == 'FUN_0045c570':
        pend[f[0]] = [int(x, 16) for x in f[4:7]]
    elif 'hid_send_feature_report' in f[2] and f[0] in pend:
        b = f[-1].split()[1:]
        p, slot, ui = pend.pop(f[0])
        if p == 0:
            buttons[f[0]].append((slot, (int(b[2], 16) | int(b[3], 16) << 8) - 0x40))

dpi_range = {}
for m, fns in sections(f'{EMU}/emu_dpi.tsv').items():
    for f in fns['436d40']:
        if f[2] == 'FUN_00435e40':
            dpi_range[m] = (hexarg(f[5]), hexarg(f[4]))  # (min, max)
            break

def dpi_tables():
    """EmuDpi output -> {model: {index: {cpi, reg}}}; reg = the 5 data bytes of
    the first stage write: [on][X lo][X flags][Y lo][Y flags]."""
    t = defaultdict(dict)
    rets, got = 0, False
    for line in open(f'{EMU}/emu_dpi_tab.tsv'):
        f = line.rstrip('\n').split('\t')
        if f[1] == 'IDX':
            m, idx, rets, got = f[0], int(f[2]), 0, False
        elif f[1] == 'RET':
            rets += 1
            if rets == 1:  # FUN_00432150: CPI
                t[m][str(idx)] = {'cpi': int(f[2], 16)}
        elif rets == 1 and 'hid_send_feature_report' in line and not got:  # FUN_00457b30
            got = True
            t[m][str(idx)]['reg'] = [int(x, 16) for x in f[-1].split()[1:][8:13]]
    return t


tables = dpi_tables()
sensor_of_table = {}
for m, t in tables.items():
    sensor_of_table[sensor[m]] = t


def key_of(name):
    return re.sub(r'[^a-z0-9]+', '-', name.lower()).strip('-')


# Display names: RDCfg's own, plus the retail names the user's mice carry.
DISPLAY = {'M711': 'M711 COBRA', 'M711-FPS-1': 'M711-FPS COBRA FPS', 'M711-FPS-2': 'M711-FPS COBRA FPS',
           'M912-RGB': 'M801-RGB / M912-RGB'}

os.makedirs(OUT_IMG, exist_ok=True)
rs = ['// @generated by tools/re/rdcfg/gen_models.py from RDCfg.exe (REDRAGON Gaming Mouse.exe).',
      '// Do not edit by hand; see MECHANICS.md "Other models".', '',
      'use crate::models::{Btn, Model, Proto::*};', '']
# sensor -> [(cpi, 16-bit register)] by slider index
reg_tables = {s: [(t[str(i)]['cpi'], t[str(i)]['reg'][1] | t[str(i)]['reg'][2] << 8) for i in range(len(t))]
              for s, t in sensor_of_table.items()}
top_index = defaultdict(int)
entries = []


def fit_photo(src, key, rects, raw=False):
    """Fits a skin photo into the general page box; rects = {id: (x, y, w, h)} of the
    button markers in RDCfg window coordinates -> {id: (cx, cy)} in box coordinates."""
    im = Image.open(src).convert('RGBA')
    W, H = im.size
    k300 = min(300 / W, 300 / H)
    sw, sh = round(W * k300), round(H * k300)
    ox, oy = LABEL[0] + (LABEL[2] - sw) // 2, LABEL[1] + (LABEL[3] - sh) // 2
    if raw:
        k300, ox, oy = 1, 0, 0
    # drop the transparent margin some photos carry, so every mouse fills the box
    l, t, r, btm = im.getchannel('A').point(lambda a: 255 if a > 8 else 0).getbbox() or (0, 0, W, H)
    l, t, r, btm = max(0, l - 4), max(0, t - 4), min(W, r + 4), min(H, btm + 4)
    crop = im.crop((l, t, r, btm))
    kb = min(BOX[0] / crop.width, BOX[1] / crop.height)
    bw, bh = round(crop.width * kb), round(crop.height * kb)
    bx, by = (BOX[0] - bw) // 2, (BOX[1] - bh) // 2
    canvas = Image.new('RGBA', BOX, (0, 0, 0, 0))
    canvas.alpha_composite(crop.resize((bw, bh), Image.LANCZOS), (bx, by))
    canvas.save(f'{OUT_IMG}/{key}.png', optimize=True)
    marks = {}
    for wid, (x, y, w, h) in rects.items():
        cx = ((x + w / 2 - ox) / k300 - l) * kb + bx
        cy = ((y + h / 2 - oy) / k300 - t) * kb + by
        marks[wid] = (round(cx), round(cy))
    return marks


def entry(key, name, pid, bcd, sens, led_v, lo, hi, photo, btns, wheel, vid=0x04d9, proto='Holtek', defaults=(),
          modes=5, alt=(), kb='None'):
    if kb == 'None':  # keyboards have no sensor table
        top_index[sens] = max(top_index[sens], hi)
    entries.append(
        f'    Model {{ key: "{key}", name: "{name}", vid: {vid:#06x}, pid: {pid:#06x}, '
        f'alt_pids: &[{", ".join(f"{a:#06x}" for a in alt)}], bcd: ({bcd[0]}, {bcd[1]}), '
        f'sensor: {sens:#x}, led: {led_v}, dpi: ({lo}, {hi}), photo: {photo},\n'
        f'        buttons: &[{", ".join(btns)}],\n'
        f'        wheel: [{", ".join(wheel)}], proto: {proto}, '
        f'defaults: &[{", ".join(f"{d:#x}" for d in defaults)}], modes: {modes}, kb: {kb} }},')


def btn_rs(off, mk):
    mk = f'Some(({mk[0]}, {mk[1]}))' if mk else 'None'
    return f'Btn {{ off: {off:#x}, mark: {mk} }}'


for pr in products:
    m = pr['name']
    if m not in sensor or m not in buttons:
        print('skip (no data):', m, file=sys.stderr)
        continue
    key = key_of(m)
    lo, hi = dpi_range.get(m, (1, 0))
    if hi <= lo:
        print('skip (no dpi range):', m, file=sys.stderr)
        continue
    src = f'{SKIN}/{m}.png'
    photo, marks = 'None', {}
    if os.path.exists(src):
        marks = fit_photo(src, key, markers[m])
        photo = f'Some("mice/{key}")'
    phys = sorted(s for s, _ in buttons[m] if s < 40)
    addr = dict(buttons[m])
    btns = [btn_rs(addr[s], marks.get(0x204 + 4 * s if s < 20 else -1)) for s in phys]
    wheel = [btn_rs(addr[40], marks.get(0x2a4)), btn_rs(addr[41], marks.get(0x2a8))]
    entry(key, DISPLAY.get(m, m), pr['pid'], pr['bcd'], sensor[m], led.get(m, 0), lo, hi, photo, btns, wheel)

# single-model builds (tools/re/fleet/gen_singles.py)
if EXTRA:
    ex = json.load(open(EXTRA))
    for s, t in ex['sensors'].items():
        s = int(s, 16)
        t = [tuple(x) for x in t]
        # entries past a sensor's slider range are table overreads; compare the used part
        used = max(e['dpi'][1] for e in ex['models'] if e['sensor'] == s and e['dpi']) + 1
        if s in reg_tables:
            n = min(used, len(reg_tables[s]))
            assert t[:n] == reg_tables[s][:n], f'sensor {s:#x} table differs between builds'
            if len(reg_tables[s]) < used:
                reg_tables[s] = t
        else:
            reg_tables[s] = t
    for e in ex['models']:
        key = key_of(e['name'])
        if not e['dpi'] or None in e['buttons'] + e['wheel']:
            print('skip (incomplete):', e['name'], file=sys.stderr)
            continue
        size = 20
        rects = {i: (x - size / 2, y - size / 2, size, size) for i, mk in enumerate(e['marks'] + e['wheel_marks']) if mk
                 for x, y in [mk]}
        marks = fit_photo(e['photo'], key, rects) if e['photo'] else {}
        n = len(e['buttons'])
        btns = [btn_rs(off, marks.get(i)) for i, off in enumerate(e['buttons'])]
        wheel = [btn_rs(e['wheel'][0], marks.get(n)), btn_rs(e['wheel'][1], marks.get(n + 1))]
        lo, hi = e['dpi']
        entry(key, e['name'], e['pid'], e['bcd'], e['sensor'], e['led'], lo, hi,
              f'Some("mice/{key}")' if e['photo'] else 'None', btns, wheel, e['vid'],
              'HoltekBulk' if e.get('bulk') else 'Holtek')

# Sinowealth mice (tools/re/fleet/gen_sino.py): each gets its own DPI table (Cfg.ini decides it)
if SINO:
    keys = {re.search(r'key: "([^"]+)"', x).group(1) for x in entries}
    for n, e in enumerate(json.load(open(SINO))):
        # one byte per stage on the wire (BY24G: two)
        table = [(c, h) for c, h in (e['table'] or []) if h <= (0xffff if e.get('proto') == 'SinoBy24g' else 0xff)]
        if not table or not e['buttons']:
            print('skip (incomplete):', e['name'], file=sys.stderr)
            continue
        name = re.sub(r'\s+(Wired\s+)?Gaming Mouse$', '', e['name'], flags=re.I).strip()
        key = key_of(name)
        while key in keys:
            key += '-b'
        keys.add(key)
        sid = 0xa000 + n
        reg_tables[sid] = table
        marks = fit_photo(e['photo'], key, {}) if e['photo'] else {}
        # rows from the left button on, as numbered on the vendor photo (the K lines' index): the
        # M811's Cfg.ini lists its 8 side keys first
        if e['buttons'][0]['index'] != 1:
            e['buttons'] = sorted(e['buttons'], key=lambda b: b['index'])
        btns = [btn_rs(b['index'] - 1, None) for b in e['buttons']]
        none = 'Btn { off: 0xffff, mark: None }'
        entry(key, name, e['pid'], (0, 0), sid, 0xff, 0, len(table) - 1,
              f'Some("mice/{key}")' if e['photo'] else 'None', btns, [none, none], e['vid'], e.get('proto', 'Sino'),
              [b['hw'] if b['hw'] is not None else 0x150 for b in e['buttons']],
              # BY24G: one profile per connection, the dongle id is the same mouse
              1 if e.get('proto') == 'SinoBy24g' else e.get('modes', 3),
              [e['pid2']] if e.get('proto') == 'SinoBy24g' and e.get('pid2') else ())

# CompX mice (tools/re/fleet/gen_compx.py): register = x | DPIex << 8, buttons = flash key index
if COMPX:
    keys = {re.search(r'key: "([^"]+)"', x).group(1) for x in entries}
    common = [0x101, 0x201, 0x401, 0x801, 0x1001, 0x102, 0x202, 0x302]
    # --compx a.json,b.json: gen_compx.py and gen_sino_old.py (the same flash over feature report 8)
    for n, e in enumerate(e for f in COMPX.split(',') for e in json.load(open(f))):
        if not e['table'] or not e['keys'] or not e['pids']:
            print('skip (incomplete):', e['name'], file=sys.stderr)
            continue
        key = key_of(e['name'])
        while key in keys:
            key += '-b'
        keys.add(key)
        sid = 0xb000 + n
        reg_tables[sid] = [tuple(x) for x in e['table']]
        # markers: 46x46 buttons at KeyParam (x, y) on the 432x356 device picture
        # (old Sino: Mark<i> rects on the skin; flash key index = K line's matrix index - 1)
        rects = {i: (k['x'], k['y'], k.get('w', 46), k.get('h', 46)) for i, k in enumerate(e['keys']) if k['x'] is not None}
        marks = fit_photo(e['photo'], key, rects, raw=True) if e['photo'] else {}
        idx = [k.get('index', i) for i, k in enumerate(e['keys'])]
        btns = [btn_rs(idx[i], marks.get(i)) for i in range(len(e['keys']))]
        defaults = [k['type'] | k['p1'] << 8 | k['p2'] << 16 | k.get('code', 0) >> 24 << 24 if k['type'] is not None else
                    (common[idx[i]] if idx[i] < len(common) else 0) for i, k in enumerate(e['keys'])]
        none = 'Btn { off: 0xffff, mark: None }'
        pids = e['pids']
        psd = e.get('psd') or 0  # DEV_n mice: bcd = (CID << 8 | MID) twice, see models.rs
        entry(key, e['name'], pids[0], (psd, psd), sid, 0xff, 0, len(e['table']) - 1,
              f'Some("mice/{key}")' if e['photo'] else 'None', btns, [none, none], e['vid'], e.get('proto', 'Compx'),
              defaults, 1, pids[1:])

# Sinowealth keyboards (tools/re/fleet/gen_kbd.py): one Kbd static per entry, photo in vendor/kbd
def via(e):
    v = e.get('via')
    return 'None' if not v else ('Some(Via { ' + ', '.join(f'{k}: {v[k]}' for k in ('cols', 'ch', 'bright', 'effect', 'speed', 'color')) + ' })')


def jl(e):
    names = e.get('jl')
    return 'None' if not names else 'Some(&[' + ', '.join(f'"{n}"' for n in names) + '])'


def hs(e):
    h = e.get('hs')
    return 'None' if not h else f'Some(Hs {{ stride: {h["stride"]:#x} }})'


kbds = []
KB_MAX = (650, 300)  # keyboard photo box of the keys page
if KBD:
    keys = {re.search(r'key: "([^"]+)"', x).group(1) for x in entries}
    out_kb = os.path.join(os.path.dirname(OUT_IMG.rstrip('/')), 'kbd')
    os.makedirs(out_kb, exist_ok=True)
    for n, e in enumerate(e for f in KBD.split(',') for e in json.load(open(f))):
        if e.get('proto') in ('jl', 'mk', 'mg'):
            e['keys'] = []  # lighting only: the photo, no remappable key
        elif not e['keys'] or not e['photo'] or not e['psd']:
            print('skip (incomplete):', e['name'], file=sys.stderr)
            continue
        key = key_of(e['name'])
        while key in keys:
            key += '-b'
        keys.add(key)
        im = Image.open(e['photo']).convert('RGBA')
        k = min(1.0, KB_MAX[0] / im.width, KB_MAX[1] / im.height)
        im = im.resize((round(im.width * k), round(im.height * k)), Image.LANCZOS) if k < 1 else im
        im.quantize(256, method=Image.Quantize.FASTOCTREE).save(f'{out_kb}/{key}.png', optimize=True)
        ks = ', '.join(f'Key {{ x: {round(q["x"] * k)}, y: {round(q["y"] * k)}, w: {round(q["w"] * k)}, h: {round(q["h"] * k)}, '
                       f'matrix: {q["matrix"]}, default: {q["code"] if q["code"] is not None else 0xffffffff:#x} }}'
                       for q in e['keys'])
        nm = lambda t: t.replace('_', ' ').replace('"', "'").strip()
        ls = ', '.join(f'LedOpt {{ hw: {l["hw"]}, name: "{nm(l["name"])}", speed: {str(l["speed"]).lower()}, '
                       f'light: {str(l["light"]).lower()}, color: {str(l["color"]).lower()}, random: {str(l["random"]).lower()}, rgb: {str(l.get("rgb", False)).lower()} }}'
                       for l in e['leds'])
        kbds.append(f'static KB{n}: Kbd = Kbd {{ glen: {e["glen"]:#x}, pro: {e["pro"]:#x}, key: {e["key"]:#x}, rgb: {e["rgb"]:#x}, '
                    f'led_off: {e["led_off"]}, byk: {str(e.get("proto") == "byk").lower()}, xy: {str(e.get("proto") == "xy").lower()}, via: {via(e)}, hs: {hs(e)}, jl: {jl(e)}, du: {str(e.get("proto") == "du").lower()}, st: {str(bool(e.get("st"))).lower()}, mk: {str(e.get("proto") == "mk").lower()}, mg: {str(e.get("proto") == "mg").lower()}, psd: [{", ".join(f"{b:#04x}" for b in e["psd"])}], '
                    f'light: &[{", ".join(map(str, e["light"]))}], speed: &[{", ".join(map(str, e["speed"]))}], '
                    f'rgbi: [{", ".join(map(str, e["rgbi"]))}], random: {e["random"]}, photo_w: {im.width}, photo_h: {im.height},\n'
                    f'    keys: &[{ks}],\n    leds: &[{ls}] }};')
        none = 'Btn { off: 0xffff, mark: None }'
        entry(key, e['name'], e['pid'], (0, 0), 0, 0xff, 0, 0, f'Some("kbd/{key}")', [], [none, none], e['vid'], 'SinoKbd',
              (), 1, e.get('alt_pids', ()), f'Some(&KB{n})')

rs.append('pub static MODELS: &[Model] = &[')
rs += entries
rs.append('];')
rs.append('')
rs.append('/// Index -> (CPI, 16-bit register) per sensor, from RDCfg FUN_00432150 / FUN_00457b30.')
rs.append('pub static SENSORS: &[(u16, &[(u32, u16)])] = &[')
for s in sorted(top_index):
    vals = [f'({c}, {r:#x})' for c, r in reg_tables[s][:top_index[s] + 1]]
    rs.append(f'    ({s:#x}, &[{", ".join(vals)}]),')
rs.append('];')
rs.append('')
rs += kbds
if kbds:
    rs[rs.index('use crate::models::{Btn, Model, Proto::*};')] = 'use crate::kbd::{Hs, Kbd, Key, LedOpt, Via};\nuse crate::models::{Btn, Model, Proto::*};'
open(OUT_RS, 'w').write('\n'.join(rs) + '\n')
print(len(entries), 'models,', len(top_index), 'sensors ->', OUT_RS)
