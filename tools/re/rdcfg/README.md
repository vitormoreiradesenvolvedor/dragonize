# Model registry from RDCfg.exe

`src/models_data.rs` and `assets/vendor/mice/` are generated from Redragon's
universal driver ("REDRAGON Gaming Mouse.exe", sha256 81fdc3d67113fe34…,
June 2022), whose `RDCfg.exe` configures 52 Holtek-based mice with the same
protocol as the M801. Nothing was transcribed by hand: the per-model
functions of RDCfg.exe were run in Ghidra's PCode emulator once per product
name (`EmuModels.java`): every call is intercepted, name comparisons answer
for the model being emulated, and the arguments of the interesting calls
(button writes, widget geometry, slider ranges) are logged.

```sh
W=~/.cache/cobra-re
innoextract -s -d $W/x "REDRAGON Gaming Mouse.exe"
G=~/.local/share/ghidra/ghidra_11.3.2_PUBLIC
$G/support/analyzeHeadless $W/proj cobra -import $W/x/app/RDCfg.exe
M=$(grep '^PRODUCT_NAME_' "$W/x/userappdata/REDRAGON Gaming Mouse/Config.ini" | cut -d= -f2- | tr -d '\r' | paste -sd'|')
run() { $G/support/analyzeHeadless $W/proj cobra -process RDCfg.exe -noanalysis -readOnly \
        -scriptPath "$PWD/tools/re/rdcfg" -postScript "$@"; }
run EmuModels.java $W/out/emu_all.tsv 431a30,4314d0,406780 "$M"   # LED variant, sensor, layout
run EmuModels.java $W/out/emu_btn.tsv 45d5c0 "$M"                 # button slot -> address
A=$(awk -F'\t' '$2=="BEGIN"{f=$3} $2=="RET" && f=="4314d0"{print $1"="$3}' $W/out/emu_all.tsv | paste -sd'|')
run EmuModels.java $W/out/emu_dpi.tsv 436d40 "$M" "$A"            # DPI slider range (arg = sensor)
run EmuDpi.java $W/out/emu_dpi_tab.tsv 'M711=cfd|M912-RGB=cff|LEGEND=d20|LEGEND2=d3d|GRIFFIN=c8c|M721-RGB2=c20|2805=d07|2805-P3333=d05|2890=d47|2903 3395=d43' 780
# then
python3 tools/re/rdcfg/gen_models.py $W
```

| RDCfg function | meaning |
|---|---|
| FUN_00423c30 | current product name (QString) |
| FUN_00431a30 | lighting variant: 3 = M801 modes, 0-2 = six modes, 4 = zones |
| FUN_004314d0 | sensor id (0xcfd 3325, 0xcff 3327, 0xd20, 0xd3d, ...) |
| FUN_00432150 | slider index -> CPI, per sensor |
| FUN_00457b30 | DPI stage write `base+4+6i`: [on][X lo][X flags][Y lo][Y flags] |
| FUN_0045c570 / FUN_0045d5c0 | button write: slot -> `base + 0x42 + ...` (per model) |
| FUN_00406780 | product photo + button markers (300x300 fit in a 661x262 label at 287,56) |
| FUN_00436d40 | DPI slider range per model |
| FUN_00456d90 | lighting write `0x449 + 8p` per variant |
| FUN_00459cf0 | macro slot write `0x478 + 200 * slot` |
| FUN_00455dc0 | enumeration: VID/PID/bcdDevice range + usage page 0xffa0 |
