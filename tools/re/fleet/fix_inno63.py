#!/usr/bin/env python3
"""Second pass: installers that 7z only split into PE sections (Inno Setup
>= 6.3, unsupported by innoextract 1.9) are unpacked again with a newer
innoextract ($INNO), then their nested installers too."""
import os, pathlib, shutil, sys
sys.argv = sys.argv[:1]
here = pathlib.Path(__file__).parent
ns = {"__name__": "lib"}
src = (here / "extract_all.py").read_text().replace("\nmain()\n", "\n")
exec(compile(src, "extract_all.py", "exec"), ns)
X = ns["DST"]
SRC = ns["SRC"]
done = 0
for sec in sorted(X.rglob("[[]0]")):
    d = sec.parent
    if not (d / ".text").exists():
        continue
    # the installer: d is "<file>.x" (nested) or the package dir (top level)
    if d.name.endswith(".x"):
        inst = d.with_name(d.name[:-2])
    else:
        rel = d.relative_to(X)
        cands = [p for p in (SRC / rel.parts[0]).iterdir() if p.stem == rel.parts[1]]
        inst = cands[0] if cands else None
    if not inst or not inst.exists():
        print("?", d); continue
    tmp = d.with_name(d.name + ".tmp")
    shutil.rmtree(tmp, ignore_errors=True)
    kind = ns["unpack_archive"](inst, tmp) if inst.suffix.lower() == ".exe" else "skip"
    if kind == "inno":
        shutil.rmtree(d); tmp.rename(d); ns["nested"](d, 1); done += 1
        print("inno ", d)
    else:
        shutil.rmtree(tmp, ignore_errors=True); print("FAIL ", kind, d)
print("fixed", done)
