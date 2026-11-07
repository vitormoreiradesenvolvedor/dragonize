#!/usr/bin/env python3
"""duilib mice (DuiLib + osConfLib drivers, Evision firmware 320f:xxxx) -> mouse entries in
gen_compx.py's format (proto "Duilib"), for gen_models.py --compx. Usage: gen_duilib.py OUT.json

Per package (app/ next to the exe): the exe's device table (FUN_00402810 in the M916-PLUS build:
rows vid, pid, .., class at +8 (2 = wired mouse, 6 / 0xa = 2.4 GHz), usage page, usage), the
skin's Skin/ms/mouse.png photo and its visible key_<n> buttons (n = the mouse's key index; the
skin's tooltips are swapped, the images are not), each key's default code from
DefaultData/Mouse.json KeyList[n].Assignment (type << 16 | p1 << 8 | p2). The sensor is what the
mouse reports (du.rs, command 3); the table here is only the guess the UI starts with, from the
stage bytes the package's Mouse.json carries. Protocol: du.rs, NOTES.md "duilib mice".
"""
import json
import pathlib
import re
import subprocess
import sys

R = pathlib.Path.home() / '.cache/redragon-all/x'
SKIP = re.compile(r'unins|update|audiorecord|upgrade', re.I)


def rows(exe):
    """Device table rows: 11 words each, written by mov reg, imm / mov WORD PTR ds:a, reg."""
    out = subprocess.run(['objdump', '-d', '-M', 'intel', '--no-show-raw-insn', str(exe)],
                         capture_output=True, text=True, errors='replace').stdout
    reg, mem = {}, {}
    for line in out.splitlines():
        ins = line.split('\t')[-1].strip()
        m = re.match(r'mov\s+e?([a-d]x|[sd]i),0x([0-9a-f]+)$', ins)
        if m:
            reg[m.group(1)] = int(m.group(2), 16)
            continue
        m = re.match(r'xor\s+e([a-d]x),e\1$', ins)
        if m:
            reg[m.group(1)] = 0
            continue
        m = re.match(r'mov\s+(?:WORD PTR )?ds:0x([0-9a-f]+),([a-d]x)$', ins)
        if m and m.group(2) in reg:
            mem[int(m.group(1), 16)] = reg[m.group(2)] & 0xffff
            continue
        m = re.match(r'mov\s+WORD PTR ds:0x([0-9a-f]+),0x([0-9a-f]+)$', ins)
        if m:
            mem[int(m.group(1), 16)] = int(m.group(2), 16)
    return [[mem.get(a + 2 * k, 0) for k in range(11)] for a, v in sorted(mem.items())
            if v == 0x320f and mem.get(a + 14) in (0xff1c, 0x000c)]


def model_name(pkg):
    m = re.search(r'[MS]\d{3}[A-Z0-9]*(?:-[A-Z0-9]+)*', pkg.upper())
    return m.group(0) if m else pkg


def keys(skin):
    t = re.sub(r'<!--.*?-->', '', (skin / 'XML/DeviceXml/mousedevice.xml').read_text('utf-8', 'replace'), flags=re.S)
    out = {}
    for m in re.finditer(r'<DeviceKeyButton name="key_(\d+)"([^>]*)>', t):
        a = dict(re.findall(r'(\w+)="([^"]*)"', m.group(2)))
        if a.get('visible') == 'false' or not a.get('pos'):
            continue
        x, y = (int(v) for v in a['pos'].split(',')[:2])
        out[int(m.group(1))] = (x, y, int(a.get('width', 30)), int(a.get('height', 30)))
    return out


def guess_sensor(stages):
    """Stage bytes 2-3 of the package's default stages -> the sensor they are written for."""
    xs = [s['StageDPIX'] for s in stages]
    if any(x > 0xff and x >> 8 == x & 0xff for x in xs):
        return 0x8925  # one byte, written twice
    if 390 in xs and 790 in xs:
        return 0x0070  # cpi - 10
    return 0x3395


def table(sensor):
    """(cpi, register) of the slider (du.rs `cpi`)."""
    if sensor == 0x8925:
        return [(r * 50 + 50 if r <= 99 else (r - 50) * 100 + 100, r) for r in range(150)]
    if sensor == 0x0070:
        return [(c, c - 10) for c in range(50, 45001, 10)]
    return [(r * 50 + 50, r) for r in range(520)]


def main():
    res, seen = [], {}
    for mj in sorted(R.rglob('DefaultData/Mouse.json')):
        app = mj.parent.parent
        pkg = str(app.relative_to(R))
        if pkg.startswith('Teclados'):
            continue
        exe = next((e for e in sorted(app.glob('*.exe')) if not SKIP.search(e.name) and b'DuiLib' in e.read_bytes()), None)
        if not exe or not (app / 'Skin/ms/mouse.png').exists():
            print('skip', pkg, file=sys.stderr)
            continue
        combo = pkg.startswith('Combos')
        pids = []
        # the wired id first: du.rs asks the dongle (0xaa) when the mouse is on another id
        for r in sorted(rows(exe), key=lambda r: r[4] != 2):
            if r[7] == 0xff1c and r[4] in ((2,) if combo else (2, 6, 0xa)) and r[1] and r[1] not in pids:
                pids.append(r[1])
        dev = next(d for d in json.loads(mj.read_text('utf-8', 'replace'))['Device'] if d['DeviceBasicInfo']['DeviceType'] == 'mouse')
        codes = [k['DefaultAssignment'] for k in dev['KeyList']]
        ks = keys(app / 'Skin')
        sensor = guess_sensor(dev['DPIStages']['StageDPIs'])
        top = pkg.split('/')[1]
        name = model_name(top)
        sig = (tuple(pids), tuple(sorted(ks.items())), tuple(codes[:11]))
        if sig in seen:  # same mouse, another package (M725-LIT / M725W-LIT)
            e = seen[sig]
            if e['name'].count(' / ') < 1 and name not in e['name']:
                e['name'] += ' / ' + name
            e['package'] += ', ' + top
            continue
        e = dict(name=name, package=top, family='duilib', vid=0x320f, pids=pids, sensor=f'{sensor:x}',
                 table=table(sensor), proto='Duilib', photo=str(app / 'Skin/ms/mouse.png'), stages=5,
                 keys=[dict(index=n, x=x, y=y, w=w, h=h, type=codes[n] >> 16 & 0xff, p1=codes[n] >> 8 & 0xff,
                            p2=codes[n] & 0xff) for n, (x, y, w, h) in sorted(ks.items())])
        seen[sig] = e
        res.append(e)
    for e in res:
        print(f"{e['name']:28} {' '.join('%04x' % p for p in e['pids']):15} {e['sensor']:>5} {len(e['keys'])} keys  {e['package']}")
    json.dump(res, open(sys.argv[1], 'w'), indent=1)
    print(len(res), 'duilib mice', file=sys.stderr)


main()
