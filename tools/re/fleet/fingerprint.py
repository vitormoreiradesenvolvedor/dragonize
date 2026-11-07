#!/usr/bin/env python3
"""Fingerprint every unpacked Redragon package: main executables (sha256),
USB ids found in config files and binaries, config file names.
Writes ~/.cache/redragon-all/fp.json and prints families (same main exe)."""
import hashlib, json, pathlib, re, sys, collections

X = pathlib.Path("~/.cache/redragon-all/x").expanduser()
SKIP = re.compile(r"(unins|vcredist|vc_redist|dotnet|setup|install|dxsetup|directx|7z|uninst|update|crash|QtWebEngine|repair)", re.I)
VIDPID = re.compile(rb"(?i)vid\s*[_&=:]?\s*(?:0x)?([0-9a-f]{4})(?![0-9a-z])[^0-9a-z]{0,8}pid\s*[_&=:]?\s*(?:0x)?([0-9a-f]{4})(?![0-9a-z])")
VIDPID_W = re.compile(rb"(?i)v\x00i\x00d\x00[_&=:]\x00(?:0\x00x\x00)?((?:[0-9a-f]\x00){4})[^v]{0,12}?p\x00i\x00d\x00[_&=:]\x00(?:0\x00x\x00)?((?:[0-9a-f]\x00){4})")

def sha(p):
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()[:16]

def ids_in(data):
    s = set()
    for m in VIDPID.finditer(data):
        s.add((m.group(1).decode().lower(), m.group(2).decode().lower()))
    for m in VIDPID_W.finditer(data):
        s.add((m.group(1)[::2].decode().lower(), m.group(2)[::2].decode().lower()))
    return s

def main():
    out = {}
    for cat in sorted(X.iterdir()):
        if not cat.is_dir():
            continue
        for pkg in sorted(p for p in cat.iterdir() if p.is_dir()):
            exes, dlls, cfgs, ids = [], [], [], set()
            for f in pkg.rglob("*"):
                if not f.is_file():
                    continue
                e = f.suffix.lower()
                rel = str(f.relative_to(pkg))
                if e in (".exe", ".dll", ".ini", ".xml", ".json", ".cfg", ".txt", ".dat") and f.stat().st_size < 80 << 20:
                    try:
                        data = f.read_bytes()
                    except OSError:
                        continue
                    ids |= ids_in(data)
                if e == ".exe" and not SKIP.search(f.name) and f.stat().st_size > 200_000 \
                        and not f.with_name(f.name + ".x").exists() and "/Update/" not in "/" + rel:
                    exes.append((rel, f.stat().st_size, sha(f)))
                elif e == ".dll" and not re.match(r"(?i)(qt|msvc|vcrun|api-ms|ucrt|lib|icu|d3d|opengl|concrt|mfc)", f.name):
                    dlls.append(f.name)
                elif e in (".ini", ".xml", ".json", ".cfg") and "/" not in rel.replace("\\", "/")[-60:] or e in (".ini",):
                    cfgs.append(rel)
            out[f"{cat.name}/{pkg.name}"] = {
                "exes": sorted(exes, key=lambda x: -x[1]),
                "dlls": sorted(set(dlls)),
                "cfgs": sorted(cfgs)[:40],
                "ids": sorted(ids),
            }
    json.dump(out, open(X.parent / "fp.json", "w"), indent=1)
    fam = collections.defaultdict(list)
    for k, v in out.items():
        main = v["exes"][0] if v["exes"] else ("<none>", 0, "")
        fam[(pathlib.PurePath(main[0]).name, main[2])].append(k)
    for (name, h), ks in sorted(fam.items(), key=lambda x: -len(x[1])):
        print(f"{len(ks):3} {name} {h}")
        for k in ks[:4]:
            print("      ", k)

main()
