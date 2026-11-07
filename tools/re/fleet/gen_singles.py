#!/usr/bin/env python3
"""Registry entries for single-model RDCfg builds (M724, M995, M812-RGB, M711-FPS-1 V2, ...).

These builds take the product from <app>/Default.bin (QSettings INI, [PRODUCT]):
  Name                  retail name
  Mouse1 ^ Name1        VID;  Mouse2 = PID;  Mouse3 ^ Name1 .. Mouse4 ^ Name1 = bcdDevice range
  Name1 ^ Name2         sensor id (RDCfg's FUN_004314d0 values: 0xcff = 3327, 0xd43 = 3395, ...)
  Light                 lighting variant (as RDCfg FUN_00431a30)
  Count, M<i>           buttons (M<i> = default function id), M<i>L/M<i>R marker top-left,
  MUL/MUR, MDL/MDR      wheel up/down marker, MB1 marker size, Wheel = ui id of wheel up
(all read by the build's own code, e.g. FUN_00448c40 / FUN_00423b30 of build dcff7ffe).
Per-build behaviour still comes from the binary, run in Ghidra's emulator:
  EmuSlots   button writer -> profile offset of each slot
  EmuModels  DPI slider range (FUN_00436d40 with the sensor)
  EmuDpi     index -> CPI / register for the sensor
usage: gen_singles.py BUILDS_TSV OUT.json      (BUILDS_TSV: sha8, package, config.ini, skin dir)
"""
import configparser
import json
import os
import pathlib
import re
import subprocess
import sys

R = pathlib.Path('~/.cache/redragon-all').expanduser()
G = pathlib.Path('~/.local/share/ghidra/ghidra_11.3.2_PUBLIC').expanduser()
EMU = pathlib.Path(__file__).resolve().parent.parent / 'rdcfg'
# builds whose writers use the classic 16-byte f3 packets
CLASSIC = {'cb8d6b2f', 'dcff7ffe', 'ddfab882', 'dfd3de91'}
# builds that write a whole profile (0xaf bytes from its base) in one 256-byte feature
# report 4 (e.g. FUN_00450550 of 9b650769); same memory map. Their Default.bin keeps the
# sensor as the hex digits of Name2 read back as decimal (FUN_00422b10: "3311" = PAW3311)
BULK = {'9b650769', '9d3c8b4b', 'c278b9e3', 'c8986af2', 'eeba115f', 'f86486fa'}


def default_bin(pkg):
    for p in (R / 'x' / pkg).rglob('Default.bin'):
        cp = configparser.ConfigParser(interpolation=None, strict=False)
        cp.optionxform = str
        cp.read_string(p.read_bytes().decode('utf-8', 'replace'))
        return dict(cp['PRODUCT']), p.parent
    return None, None


def getters(build):
    """Default.bin getters of a build -> {key: function name}, from its decompiled C"""
    out = {}
    for m in re.finditer(r'// ===== (FUN_\w+) @ \w+\n(.*?)(?=// ===== |\Z)', open(R / 'ghidra/out' / f'holtek-rdcfg-{build}.c',
                         errors='replace').read(), re.S):
        keys = re.findall(r'"PRODUCT/(\w+)"', m.group(2))
        if keys == ['Name1', 'Name2']:
            out['sensor'] = m.group(1)
        elif keys == ['Light']:
            out['led'] = m.group(1)
    return out


def ghidra(build, out, *script, stub=''):
    env = dict(os.environ, EMU_MAP=str(R / 'ghidra/out' / f'{build}.map'), EMU_STUB=stub)
    subprocess.run([str(G / 'support/analyzeHeadless'), str(R / 'ghidra/proj' / f'holtek-rdcfg-{build}'), 'p',
                    '-process', f'holtek-rdcfg-{build}.exe', '-noanalysis', '-readOnly', '-scriptPath', str(EMU),
                    '-postScript', *script], env=env, stdout=open(out.with_suffix('.log'), 'w'),
                   stderr=subprocess.STDOUT, check=True)


def rows(path):
    return [l.rstrip('\n').split('\t') for l in open(path)]


def main():
    builds = [l.rstrip('\n').split('\t') for l in open(sys.argv[1], errors='surrogateescape')]
    out = {'models': [], 'sensors': {}}
    done = set()
    for sha, pkg, cfg, skin in builds:
        if sha not in CLASSIC | BULK:
            continue
        bulk = sha in BULK
        d, app = default_bin(pkg)
        if not d or d['Name'] in done:
            continue
        done.add(d['Name'])
        n1 = int(d['Name1'])
        if bulk:
            sensor = int(re.match(r'\d*', f"{int(d['Name2']):x}").group(0) or 0)
        else:
            sensor = n1 ^ int(d['Name2'])
        vid, pid = int(d['Mouse1']) ^ n1, int(d['Mouse2'])
        bcd = (int(d['Mouse3']) ^ n1, int(d['Mouse4']) ^ n1)
        name = re.sub(r'^REDRAGON ', '', d['Name'])
        model = re.search(r'PRODUCT_NAME_0=(.*)', open(cfg, errors='replace').read()).group(1).strip()
        w = R / 'emu' / sha
        w.mkdir(parents=True, exist_ok=True)
        count = int(d['Count'])
        wheel_ui = int(d.get('Wheel', 22))
        slots = [str(i) for i in range(count)] + [f'40:{wheel_ui}', f'41:{wheel_ui + 1}']
        g = getters(sha)
        stub = f"{g['sensor']}={sensor:x},{g['led']}={int(d.get('Light', 0)):x}"
        if not bulk:
            ghidra(sha, w / 'emu_slots.tsv', 'EmuSlots.java', str(w / 'emu_slots.tsv'), model, ','.join(slots), stub=stub)
        ghidra(sha, w / 'emu_dpi.tsv', 'EmuModels.java', str(w / 'emu_dpi.tsv'), '436d40', model, f'{model}={sensor:x}',
               stub=stub)
        addr, cur = {}, None
        if bulk:
            # the product name matches none of the encoder's special cases (FUN_0044f3a0),
            # so slot i lands at index i of the 4-byte button array at +0x42, the wheel at
            # index Wheel / Wheel + 1 (EmuCall run of the encoder, slot 2 -> index 2)
            addr = {i: 0x42 + 4 * i for i in range(count)}
            addr[40], addr[41] = 0x42 + 4 * wheel_ui, 0x46 + 4 * wheel_ui
        for f in ([] if bulk else rows(w / 'emu_slots.tsv')):
            if f[1] == 'SLOT':
                cur = int(f[2])
            elif 'hid_send_feature_report' in f[2] and cur is not None and cur not in addr:
                b = f[-1].split()[1:]
                addr[cur] = (int(b[2], 16) | int(b[3], 16) << 8) - 0x40
        rng = next(((int(f[5].split('=')[0], 16), int(f[4].split('=')[0], 16)) for f in rows(w / 'emu_dpi.tsv')
                    if f[2] == 'FUN_00435e40'), None)
        if sensor not in out['sensors']:
            ghidra(sha, w / 'emu_dpi_tab.tsv', 'EmuDpi.java', str(w / 'emu_dpi_tab.tsv'), f'{model}={sensor:x}', '780',
                   stub=stub)
            tab, rets, got, idx = {}, 0, False, None
            for f in rows(w / 'emu_dpi_tab.tsv'):
                if f[1] == 'IDX':
                    idx, rets, got = int(f[2]), 0, False
                elif f[1] == 'RET':
                    rets += 1
                    if rets == 1:
                        tab[idx] = [int(f[2], 16), None]
                elif rets == 1 and 'hid_send_feature_report' in f[2] and not got:
                    got = True
                    # [on][X lo][X flags][Y lo][Y flags] of stage 0
                    r = [int(x, 16) for x in f[-1].split()[1:][12:17] if bulk] or \
                        [int(x, 16) for x in f[-1].split()[1:][8:13]]
                    tab[idx][1] = r[1] | r[2] << 8
            out['sensors'][sensor] = [tab[i] for i in sorted(tab)]
        size = int(d.get('MB1', 20))
        marks = [(int(d[f'M{i + 1}L']) + size / 2, int(d[f'M{i + 1}R']) + size / 2) if f'M{i + 1}L' in d else None
                 for i in range(count)]
        wheel_marks = [(int(d['MUL']) + size / 2, int(d['MUR']) + size / 2) if 'MUL' in d else None,
                       (int(d['MDL']) + size / 2, int(d['MDR']) + size / 2) if 'MDL' in d else None]
        photo = next((str(p) for p in [pathlib.Path(skin) / 'mouse.png'] if p.exists()), None)
        out['models'].append(dict(name=name, rdcfg=model, build=sha, package=pkg, vid=vid, pid=pid, bcd=bcd, bulk=bulk,
                                  sensor=sensor, led=int(d.get('Light', 0)), dpi=rng,
                                  buttons=[addr.get(i) for i in range(count)], wheel=[addr.get(40), addr.get(41)],
                                  marks=marks, wheel_marks=wheel_marks, photo=photo))
        print(name, f'{vid:04x}:{pid:04x}', bcd, f'sensor {sensor:#x}', 'dpi', rng, 'buttons', out['models'][-1]['buttons'],
              out['models'][-1]['wheel'])
    out['sensors'] = {f'{k:#x}': v for k, v in out['sensors'].items()}
    json.dump(out, open(sys.argv[2], 'w'), indent=1)


main()
