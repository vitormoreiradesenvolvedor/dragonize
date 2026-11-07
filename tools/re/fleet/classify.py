#!/usr/bin/env python3
"""Per-package inventory: protocol family, app directory, USB ids.
Reads fp.json (fingerprint.py), writes ~/.cache/redragon-all/inventory.tsv."""
import json, pathlib, collections, sys

R = pathlib.Path("~/.cache/redragon-all").expanduser()
fp = json.load(open(R / "fp.json"))

def family(v):
    d = set(v["dlls"]); exe = pathlib.PurePath(v["exes"][0][0]).name if v["exes"] else ""
    if "XYUI.dll" in d: return "xyui"
    if "jl_firmware_upgrade_x86.dll" in d: return "jieli"
    if "HIDUsb.dll" in d: return "hidusb"
    if "opencv_world320.dll" in d: return "duilib-kb"
    if "DuiLib.dll" in d: return "duilib"
    if "uihook.dll" in d: return "holtek-mmcfg"
    if "CheckPidVid.dll" in d: return "holtek-ldcfg"
    if "HIDApi.dll" in d and "HidDevice.dll" in d: return "holtek-rdcfg"
    if exe in ("OemDrv.exe",): return "sino-oem"
    if exe in ("KeyboardDrv.exe",): return "sino-kbd"
    if exe == "Mouse Drive Beta.exe": return "mdb"
    if "SunplusIT_6651.dll" in d: return "sunplus"
    if "GamingMouseWriter.dll" in d: return "gamingmouse"
    return "other:" + exe

rows = []
for k, v in fp.items():
    f = family(v)
    main = v["exes"][0][0] if v["exes"] else ""
    ids = " ".join(f"{a}:{b}" for a, b in v["ids"])
    rows.append((k, f, main, ids))
with open(R / "inventory.tsv", "w", errors="replace") as o:
    for r in sorted(rows, key=lambda r: (r[1], r[0])):
        print("\t".join(r), file=o)
c = collections.Counter(r[1] for r in rows)
for f, n in c.most_common():
    print(f"{n:4} {f}")
