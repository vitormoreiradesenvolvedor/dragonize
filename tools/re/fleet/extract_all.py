#!/usr/bin/env python3
"""Unpack every Redragon download (zip/rar/7z/exe installers) into
~/.cache/redragon-all/x/<category>/<name>/, recursively unpacking installers.
Idempotent: skips entries already marked done."""
import os, subprocess, sys, pathlib, shutil

SRC = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else "~/Downloads/Regradon_Softwares").expanduser()
INNO = os.environ.get("INNO", "innoextract")
DST = pathlib.Path("~/.cache/redragon-all/x").expanduser()

def run(cmd, cwd=None):
    return subprocess.run(cmd, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, errors="replace")

def is_inno(p):
    r = run([INNO, "-i", str(p)])
    return r.returncode == 0 and "Inno Setup" in r.stdout or (r.returncode == 0 and "Files:" in r.stdout)

def unpack_archive(p, out):
    out.mkdir(parents=True, exist_ok=True)
    ext = p.suffix.lower()
    if ext == ".exe":
        r = run([INNO, "-q", "-s", "-d", str(out), str(p)])
        if r.returncode == 0 and any(out.iterdir()):
            return "inno"
        shutil.rmtree(out); out.mkdir()
        r = run(["7z", "x", "-y", "-o" + str(out), str(p)])
        if r.returncode == 0 and any(out.iterdir()):
            return "7z-exe"
        shutil.rmtree(out); out.mkdir()
        return "raw"
    if ext == ".msi":
        r = run(["7z", "x", "-y", "-o" + str(out), str(p)])
        return "msi" if r.returncode == 0 else "fail"
    r = run(["7z", "x", "-y", "-o" + str(out), str(p)])
    if r.returncode != 0 or not any(out.iterdir()):
        r = run(["unar", "-q", "-f", "-o", str(out), str(p)])
    return "arch" if any(out.iterdir()) else "fail"

def nested(out, depth):
    """Unpack installers / archives found inside an unpacked tree."""
    if depth > 3:
        return
    for f in sorted(out.rglob("*")):
        if not f.is_file():
            continue
        ext = f.suffix.lower()
        if ext in (".zip", ".rar", ".7z", ".msi") or (ext == ".exe" and f.stat().st_size > 1_000_000 and looks_installer(f)):
            sub = f.with_name(f.name + ".x")
            if sub.exists():
                continue
            kind = unpack_archive(f, sub)
            if kind in ("raw", "fail"):
                shutil.rmtree(sub, ignore_errors=True)
            else:
                nested(sub, depth + 1)

def looks_installer(f):
    with open(f, "rb") as h:
        data = h.read(4 << 20)
    return any(s in data for s in (b"Inno Setup", b"Nullsoft", b"NullsoftInst", b"7z\xbc\xaf\x27\x1c", b"InstallShield", b"WinRAR SFX"))

def main():
    log = open(DST.parent / "extract.log", "a")
    for cat in sorted(p for p in SRC.iterdir() if p.is_dir()):
        for p in sorted(cat.iterdir()):
            out = DST / cat.name / p.stem
            mark = out.with_name(out.name + ".done")
            if mark.exists():
                continue
            if out.exists():
                shutil.rmtree(out)
            kind = unpack_archive(p, out)
            if kind == "raw":
                out.mkdir(parents=True, exist_ok=True)
                shutil.copy(p, out / p.name)
            nested(out, 0)
            mark.write_text(kind)
            print(f"{kind:7} {cat.name}/{p.name}", file=log, flush=True)
            print(f"{kind:7} {cat.name}/{p.name}", flush=True)

main()
