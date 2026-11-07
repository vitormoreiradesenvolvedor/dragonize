#!/usr/bin/env python3
"""Registry entries for the CompX (3554:xxxx) mice: "Mouse Drive Beta.exe" (.NET, family mdb)
and the Qt drivers with HIDUsb.dll (family hidusb). Both drive the mouse through the same
HIDUsb.dll (mdb embeds it with Costura); see compx.rs for the protocol.

Per product, from the package's Config.ini (ids are base64 of "Redragon" + text):
  [Option]  VID, M_PID / D_PID (mdb) or USB_PID / Wireless*_PID (hidusb), DriveName
  [Device1] / [Drive1]  KeyNumber (mdb: buttons*10 + 1 if the wheel rows show; hidusb: buttons),
            KeyParam<i> (mdb: x, y, list index, type, param1, param2 = position on dev1.png
            and default function), DPIRange = min,max,step[,min,max,step..],flags, Sensor
DPI encoding = mdb DPIControl.ValueToDPI / UpdateValue (decompiled with ILSpy), with the
"discrete" sensors' tables from driver_sensor.h (SensorDPIFile.GetSensorDPI).
usage: gen_compx.py OUT.json
"""
import base64
import json
import pathlib
import re
import sys

R = pathlib.Path('~/.cache/redragon-all').expanduser()


def ini(path):
    out, cur = {}, None
    for line in path.read_bytes().decode('utf-8', 'replace').splitlines():
        m = re.match(r'\s*\[(\w+)\]', line)
        if m:
            cur = out.setdefault(m.group(1), {})
        elif '=' in line and cur is not None:
            k, v = line.split('=', 1)
            cur.setdefault(k.strip(), v.strip())
    return out


def b64(s):
    t = base64.b64decode(s).decode('latin1')
    return t[8:] if t.startswith('Redragon') else t


def num(s):
    return int(s.strip(), 0)


def discrete(header, sensor, lo, step):
    """SensorDPIFile.GetSensorDPI: SENSOR_<type>_DPI_<cpi> lines from lo in steps of step"""
    vals, cpi = [], lo
    lines = header.read_text(errors='replace').splitlines() if header.exists() else []
    for line in lines:
        if f'SENSOR_{sensor}_DPI_{cpi}' in line:
            m = re.search(r'(0x[0-9a-fA-F]+|\d+)\s*$', line.split(f'_DPI_{cpi}', 1)[1])
            if m:
                vals.append((cpi, int(m.group(1), 0)))
                cpi += step
    return vals


def table(sensor, rng, header):
    """every CPI the driver can set -> (x byte, DPIex byte), as ValueToDPI computes it"""
    p = [num(x) for x in rng.split(',')]
    flags, seg = p[-1], [p[i:i + 3] for i in range(0, len(p) - 1 - (len(p) - 1) % 3, 3)]
    disc = dict(discrete(header, sensor, seg[0][0], seg[0][2]))
    out = []
    for si, (lo, hi, step) in enumerate(seg):
        for v in range(lo, hi + 1, step):
            ex, mul = 0, 1
            if flags == 0x33:
                s = max(i for i, sg in enumerate(seg) if v >= sg[0])
                ex, mul = {3: (0x33, 4), 2: (0x11, 2), 1: (0x22, 2)}.get(s, (0, 1))
            elif flags in (0x22, 0x11) and v > seg[0][1]:
                ex, mul = flags, 2
            if disc:
                x = disc.get(v // mul)
                if x is None:
                    continue
            else:
                x = v // (50 * mul) - 1
                if sensor in ('3395', '3950'):
                    ex |= (x >> 8) << 2 | (x >> 8) << 6
                    x &= 0xff
            if 0 <= x <= 0xff:
                out.append((v, x | ex << 8))
    return out


def main():
    inv = [l.rstrip('\n').split('\t') for l in open(R / 'inventory.tsv', errors='surrogateescape')]
    out, seen = [], set()
    for r in inv:
        if r[1] not in ('mdb', 'hidusb'):
            continue
        pkg = r[0]  # the folder keeps the " (+n)" of packages shared by n more products
        cfgs = sorted(p for p in (R / 'x' / pkg).rglob('Config.ini') if 'Sys32' not in p.parts)
        if not cfgs:
            print('skip (no Config.ini):', pkg, file=sys.stderr)
        for cfgp in cfgs:
            c = ini(cfgp)
            o = c.get('Option', {})
            if 'VID' not in o:
                continue
            mdb = r[1] == 'mdb'
            vid = int(b64(o['VID']), 16)
            pids = []
            for k in ('M_PID', 'D_PID', 'USB_PID', 'Wireless_PID', 'Wireless2_PID', 'Wireless4_PID', 'Wireless8_PID'):
                if o.get(k):
                    try:
                        pids += [int(x, 16) for x in b64(o[k]).split(',') if re.fullmatch(r'[0-9a-fA-F]{4}', x)]
                    except Exception:
                        pass
            app = cfgp.parent if mdb else cfgp.parent.parent
            for d in range(1, int(o.get('DeviceTotal', 1) or 1) + 1):
                dev = c.get(f'Device{d}') or c.get(f'Drive{d}')
                if not dev or 'DPIRange' not in dev:
                    continue
                if mdb:
                    title = o.get('DriveName', pkg.split('/')[-1])
                else:
                    # the Qt packages carry no product name: the model code of the folder name
                    title = re.sub(r' \(\+\d+\)$', '', pkg).split('/')[-1].split()[-1]
                name = re.sub(r'^Redragon\s+|\s+(3-Mode|Tri-Mode|Wireless|Wired)?\s*(Gaming\s+)?Mouse$', '', title,
                              flags=re.I).strip()
                if d > 1:
                    name += f' #{d}'
                if name in seen:
                    continue
                seen.add(name)
                sensor = b64(dev.get('Sensor', ''))
                kn = int(dev.get('KeyNumber', '0'))
                nkeys = kn // 10 if mdb else kn
                keys = []
                for i in range(1, nkeys + 1):
                    v = [x.strip() for x in dev.get(f'KeyParam{i}', '').split(',')]
                    if mdb and len(v) >= 6:
                        keys.append(dict(x=int(v[0]), y=int(v[1]), type=num(v[3]), p1=num(v[4]), p2=num(v[5])))
                    else:
                        keys.append(dict(x=None, y=None, type=None, p1=None, p2=None))
                photo = app / 'res' / '2Button' / f'dev{d}.png' if mdb else app / 'Picture' / 'Buttoninterface' / 'mouse0.png'
                t = table(sensor, dev['DPIRange'], app / 'driver_sensor.h')
                e = dict(name=name, package=pkg, family=r[1], vid=vid, pids=sorted(set(pids)), sensor=sensor,
                         dpirange=dev['DPIRange'], table=t, keys=keys, stages=int(dev.get('DPIMaxGrade', 5)),
                         photo=str(photo) if photo.exists() else None, cfg=str(cfgp))
                out.append(e)
                print(f"{name:26} {vid:04x}:{','.join(f'{p:04x}' for p in e['pids']):15} {sensor:5} "
                      f"{len(t):4} dpi {t[0][0] if t else '-'}-{t[-1][0] if t else '-'} {len(keys)} keys")
    json.dump(out, open(sys.argv[1], 'w'), indent=1)


main()
