#!/bin/sh
# Runs the RDCfg emulation (tools/re/rdcfg/README.md) on another RDCfg build.
# usage: emu_rdcfg_build.sh BUILD CONFIG_INI OUTDIR
#   BUILD = holtek-rdcfg-<sha8> (Ghidra project in ~/.cache/redragon-all/ghidra/proj/BUILD,
#           map in ~/.cache/redragon-all/ghidra/out/<sha8>.map from resolve_rdcfg.py)
set -e
B=$1; CFG=$2; O=$3
W=~/.cache/redragon-all/ghidra
G=~/.local/share/ghidra/ghidra_11.3.2_PUBLIC
S="$(cd "$(dirname "$0")/../rdcfg" && pwd)"
export EMU_MAP=$W/out/${B#holtek-rdcfg-}.map
mkdir -p "$O"
M=$(grep -a '^PRODUCT_NAME_' "$CFG" | cut -d= -f2- | tr -d '\r' | paste -sd'|')
run() { $G/support/analyzeHeadless $W/proj/$B p -process $B.exe -noanalysis -readOnly \
        -scriptPath "$S" -postScript "$@" > "$O/emu.log" 2>&1; }
run EmuModels.java "$O/emu_all.tsv" 431a30,4314d0,406780 "$M"
run EmuModels.java "$O/emu_btn.tsv" 45d5c0 "$M"
A=$(awk -F'\t' '$2=="BEGIN"{f=$3} $2=="RET" && f=="4314d0"{print $1"="$3}' "$O/emu_all.tsv" | paste -sd'|')
run EmuModels.java "$O/emu_dpi.tsv" 436d40 "$M" "$A"
run EmuDpi.java "$O/emu_dpi_tab.tsv" "$A" 780
echo "$M" | tr '|' '\n' | wc -l
