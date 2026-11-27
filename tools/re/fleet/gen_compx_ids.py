#!/usr/bin/env python3
"""CID / MID of the CompX mice, the only thing that tells apart the dozens of products sharing
3554:f55d / f55e / f5d5 (and the other CompX ids). The "Mouse Drive Beta" driver asks the mouse
with command 1 and matches the answer against its own package:

  FormHomePage::GetDeviceInfo (ILSpy):  deviceInfo.CID == driveParam.CID
                                        && deviceInfo.MID == driveParam.DeviceParams[j].MID
  DriveConfig.cs                        CID  = base64 of "<DriveName><digits>", digits as
                                               DECIMAL (int.Parse), e.g.
                                               "Redragon M916-PRO-1K23" -> 23
  Config.ini [Device<n>]                MID  = that device's id (decimal)
  DataParser.GetDeviceInfo              the answer's CID / MID / DeviceType = bytes 10, 11, 12
                                        of the 17-byte report 8 (compx.rs `psd_via`)

Unlike the other generators this one needs no unpacked archive: it reads the registry's CompX
entries out of src/models_data.rs, finds each product in Redragon's own download index and
fetches just those packages.

usage: gen_compx_ids.py src/models_cid.rs [WORKDIR]      (default ~/.cache/redragon-all/compx)
"""
import base64
import json
import pathlib
import re
import shutil
import subprocess
import sys
import urllib.request

SITE = 'https://www.redragonzone.com'
ROOT = pathlib.Path(__file__).resolve().parents[3]


def index(work):
    """Redragon's download list (the JSON behind software.html), cached."""
    cache = work / 'list.json'
    if cache.exists():
        return json.loads(cache.read_text())
    out, page = [], 1
    while True:
        with urllib.request.urlopen(f'{SITE}/download/list?page={page}&categoryId=&keyword=', timeout=120) as r:
            d = json.load(r)['data']
        out += d['list']
        if page >= d['pageCount']:
            break
        page += 1
    cache.write_text(json.dumps(out, indent=1))
    return out


def registry():
    """(key, name, vid, pids) of every Proto::Compx entry of the registry."""
    s = (ROOT / 'src/models_data.rs').read_text()
    out = []
    for b in re.split(r'\n    Model \{ ', s)[1:]:
        b = 'Model { ' + b
        if re.search(r'proto: (\w+)', b).group(1) != 'Compx':
            continue
        pids = [int(re.search(r', pid: (0x[0-9a-f]+)', b).group(1), 16)]
        pids += [int(x, 16) for x in re.findall(r'0x[0-9a-f]+', re.search(r'alt_pids: &\[([^\]]*)\]', b).group(1))]
        out.append((re.search(r'key: "([^"]+)"', b).group(1), re.search(r'name: "([^"]+)"', b).group(1),
                    int(re.search(r'vid: (0x[0-9a-f]+)', b).group(1), 16), pids))
    return out


def code(s):
    return re.sub(r'[^A-Z0-9]', '', s.upper())


def ini(path):
    raw = path.read_bytes()
    txt = raw.decode('utf-16') if raw[:2] in (b'\xff\xfe', b'\xfe\xff') else raw.decode('utf-8', 'replace')
    out, cur = {}, None
    for line in txt.splitlines():
        m = re.match(r'\s*\[(\w+)\]', line)
        if m:
            cur = out.setdefault(m.group(1), {})
        elif '=' in line and cur is not None and not line.lstrip().startswith(';'):
            k, v = line.split('=', 1)
            cur.setdefault(k.strip(), v.strip())
    return out


def b64(s):
    try:
        return base64.b64decode(s).decode('latin1')
    except Exception:
        return ''


ARCHIVE = ('.exe', '.zip', '.rar', '.7z')


def extract_one(arc, dest):
    """Inno Setup, the NSIS "niuniu" wrapper, or a plain zip / rar / 7z."""
    dest.mkdir(parents=True, exist_ok=True)
    if arc.suffix.lower() == '.exe':
        if subprocess.run(['innoextract', '-s', '-q', '-d', dest, arc], capture_output=True).returncode == 0:
            return
        subprocess.run(['7z', 'x', '-y', f'-o{dest}', arc], capture_output=True)
        return
    # bsdtar reads rar, 7z does not here
    if subprocess.run(['bsdtar', '-xf', arc, '-C', dest], capture_output=True).returncode != 0:
        subprocess.run(['7z', 'x', '-y', f'-o{dest}', arc], capture_output=True)


def unpack(pkg, dest):
    """Several of these packages are an archive holding the installer, sometimes two deep."""
    pending, done, n = [pkg], set(), 0
    for _ in range(4):
        nxt = []
        for arc in pending:
            if arc in done:
                continue
            done.add(arc)
            n += 1
            extract_one(arc, dest / f'u{n}')
            if any(dest.rglob('Config.ini')):
                return
            nxt += [p for p in sorted((dest / f'u{n}').rglob('*'))
                    if p.suffix.lower() in ARCHIVE and p.is_file() and p.stat().st_size > 1 << 14]
        pending = nxt
        if not pending:
            return


def main():
    out_rs = pathlib.Path(sys.argv[1])
    work = pathlib.Path(sys.argv[2] if len(sys.argv) > 2 else '~/.cache/redragon-all/compx').expanduser()
    (work / 'dl').mkdir(parents=True, exist_ok=True)
    lst = [e for e in index(work) if e.get('driverUrl')]
    by_code = {}
    for e in lst:
        for tok in e['name'].split():
            if len(code(tok)) >= 4:
                by_code.setdefault(code(tok), []).append(e)

    rows, missing = [], []
    for key, name, vid, pids in registry():
        cands = by_code.get(code(name)) or [e for e in lst if code(e['name']).endswith(code(name))]
        if not cands:
            missing.append((key, name, 'no download entry'))
            continue
        found = None
        for e in cands:
            url = e['driverUrl']
            pkg = work / 'dl' / url.rsplit('/', 1)[-1]
            if not pkg.exists() or not pkg.stat().st_size:
                try:
                    with urllib.request.urlopen(SITE + url, timeout=600) as r:
                        pkg.write_bytes(r.read())
                except Exception as err:
                    missing.append((key, name, f'download failed: {err}'))
                    continue
            dest = work / 'x' / re.sub(r'\W', '_', url)
            if not any(dest.rglob('Config.ini')):
                unpack(pkg, dest)
            for cfgp in sorted(p for p in dest.rglob('Config.ini') if 'Sys32' not in p.parts):
                c = ini(cfgp)
                o = c.get('Option', {})
                # the ids are base64 of Company + text, the CID base64 of DriveName + digits
                co = o.get('Company', 'Redragon')
                if 'VID' not in o or b64(o['VID']).replace(co, '').strip() != f'{vid:04X}':
                    continue
                digits = b64(o.get('CID', '')).replace(o.get('DriveName', ''), '').strip()
                if not digits.isdigit():
                    continue
                for d in range(1, int(o.get('DeviceTotal', 1) or 1) + 1):
                    dev = c.get(f'Device{d}') or c.get(f'Drive{d}') or {}
                    if not (dev.get('MID') or '').isdigit():
                        continue
                    cid, mid = int(digits), int(dev['MID'])
                    if cid > 0xff or mid > 0xff:
                        continue
                    found = (cid, mid, e['name'], url)
                    break
                if found:
                    break
            if found:
                break
        if found:
            rows.append((key, name, *found))
        else:
            missing.append((key, name, 'no CID / MID in the package'))

    body = ''.join(f'    ("{k}", {cid:#04x}, {mid:#04x}), // {name}, {pkg}\n'
                   for k, name, cid, mid, pkg, _ in sorted(rows))
    out_rs.write_text(
        '// @generated by tools/re/fleet/gen_compx_ids.py from the Config.ini of each product\'s\n'
        '// Windows package (CID decimal, MID of its [Device<n>]); see that script for the driver\n'
        '// side. Do not edit by hand.\n\n'
        '/// (registry key, CID, MID) of the CompX mice whose package has been read. One usb id\n'
        '/// serves dozens of products and this pair is what the firmware answers to command 1\n'
        '/// (compx.rs `psd_via`), so it is what names one of them. Products that share a pair\n'
        '/// (the M933 shells, M913-WL and M913W-WL) narrow the selection dialog to themselves;\n'
        '/// an entry missing here leaves the whole id in it.\n'
        'pub static COMPX_IDS: &[(&str, u8, u8)] = &[\n' + body + '];\n')

    dupes = {}
    for key, name, cid, mid, *_ in rows:
        dupes.setdefault((cid, mid), []).append(key)
    for key, name, why in sorted(missing):
        print(f'skip {key:24} {name:22} {why}', file=sys.stderr)
    for (cid, mid), keys in sorted(dupes.items()):
        if len(keys) > 1:
            print(f'CID {cid:#04x} MID {mid:#04x} is shared by {" ".join(keys)}', file=sys.stderr)
    print(f'{len(rows)} of {len(registry())} CompX entries have a CID / MID -> {out_rs}')
    if shutil.which('rustfmt'):
        subprocess.run(['rustfmt', out_rs], check=False)


main()
