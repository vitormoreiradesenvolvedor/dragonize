#!/usr/bin/env python3
"""The 2.4 GHz / Bluetooth usb id of each keyboard, which is not the wired one and is the only
way to recognise the keyboard when it is not on the cable:

  Dev/kb/KB.ini [OPT]   VID=0x258a PID=0x010c                 the cable
                        VID_Wireless=0x3554 PID_Wireless=0xfa09   the dongle
  Cfg.ini [OPT]         the same two pairs on the builds that keep the keyboard there

On that id the driver speaks another protocol: `OnNewDongle` / `OnNewCombo` build a CDev3632
(or a CDevCombo), whose own FindHIDDevice opens nothing, and everything goes through the
20-byte reports of `src/dg.rs`. The wired entry of the registry is reused for the product, so
all this table carries is where else to look for it.

Like gen_compx_ids.py this needs no unpacked archive: it reads the keyboard entries out of
src/models_data.rs, finds each product in Redragon's own download index and fetches just those
packages.

usage: gen_wl_ids.py src/models_wl.rs [WORKDIR]      (default ~/.cache/redragon-all/wl)
"""
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
    """(key, name, vid, pid) of every keyboard entry of the registry."""
    s = (ROOT / 'src/models_data.rs').read_text()
    out = []
    for b in re.split(r'\n    Model \{ ', s)[1:]:
        b = 'Model { ' + b
        if re.search(r'proto: (\w+)', b).group(1) != 'SinoKbd':
            continue
        out.append((re.search(r'key: "([^"]+)"', b).group(1), re.search(r'name: "([^"]+)"', b).group(1),
                    int(re.search(r'vid: (0x[0-9a-f]+)', b).group(1), 16),
                    int(re.search(r', pid: (0x[0-9a-f]+)', b).group(1), 16)))
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


def inis(dest):
    return [p for p in dest.rglob('*') if p.name.lower() in ('kb.ini', 'cfg.ini') and 'Sys32' not in p.parts]


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
            if inis(dest):
                return
            nxt += [p for p in sorted((dest / f'u{n}').rglob('*'))
                    if p.suffix.lower() in ARCHIVE and p.is_file() and p.stat().st_size > 1 << 14]
        pending = nxt
        if not pending:
            return


def num(s):
    try:
        return int(s, 0)
    except (TypeError, ValueError):
        return None


def main():
    out_rs = pathlib.Path(sys.argv[1])
    work = pathlib.Path(sys.argv[2] if len(sys.argv) > 2 else '~/.cache/redragon-all/wl').expanduser()
    (work / 'dl').mkdir(parents=True, exist_ok=True)
    lst = [e for e in index(work) if e.get('driverUrl')]
    by_code = {}
    for e in lst:
        for tok in e['name'].split():
            if len(code(tok)) >= 4:
                by_code.setdefault(code(tok), []).append(e)

    rows, missing = [], []
    for key, name, vid, pid in registry():
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
            if not inis(dest):
                unpack(pkg, dest)
            for p in sorted(inis(dest)):
                o = ini(p).get('OPT', {})
                # the package must be the product's own: its wired id is the registry's
                if (num(o.get('VID')), num(o.get('PID'))) != (vid, pid):
                    continue
                wv, wp = num(o.get('VID_Wireless')), num(o.get('PID_Wireless'))
                if wv is None or wp is None or (wv, wp) == (0, 0) or (wv, wp) == (vid, pid):
                    continue
                found = (wv, wp, e['name'])
                break
            if found:
                break
        if found:
            rows.append((key, name, *found))
        else:
            missing.append((key, name, 'no wireless id in the package'))

    body = ''.join(f'    ("{k}", {wv:#06x}, {wp:#06x}), // {name}, {pkg}\n'
                   for k, name, wv, wp, pkg in sorted(rows))
    out_rs.write_text(
        "// @generated by tools/re/fleet/gen_wl_ids.py from the Dev/kb/KB.ini (or Cfg.ini) [OPT]\n"
        "// VID_Wireless / PID_Wireless of each product's Windows package. Do not edit by hand.\n\n"
        '/// (registry key, vendor, product) of the usb id a keyboard shows on its 2.4 GHz dongle or\n'
        '/// over Bluetooth, which is not its wired one. The product is the same registry entry; what\n'
        '/// changes is the protocol, the 20-byte reports of `dg.rs` (OnNewDongle / CDev3632) instead\n'
        '/// of the feature reports of the cable.\n'
        'pub static WIRELESS_IDS: &[(&str, u16, u16)] = &[\n' + body + '];\n')

    for key, name, why in sorted(missing):
        print(f'skip {key:24} {name:22} {why}', file=sys.stderr)
    print(f'{len(rows)} of {len(registry())} keyboard entries have a wireless id -> {out_rs}')
    if shutil.which('rustfmt'):
        subprocess.run(['rustfmt', out_rs], check=False)


main()
