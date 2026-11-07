# Redragon software fleet (2026-11-05)

Source: `~/Downloads/Regradon_Softwares/{Mouses,Teclados,Combos_Teclado_Mouse}` (494 downloads,
`INDICE.csv` = file -> retail products -> official URL).

Work dir (outside repo, regenerable): `~/.cache/redragon-all/`
- `x/<cat>/<pkg>/` unpacked (`extract_all.py`; Inno 6.3 via innoextract master,
  6.4 via dscho's fork `tools/inno-dscho`, both built in `tools/`); `fix_inno63.py` = 2nd pass
- `fp.json` (`fingerprint.py`), `inventory.tsv` (`classify.py`: pkg, family, main exe, usb ids)
- `ghidra/{bin,proj,out}`: one Ghidra project per binary; `out/*.feat.tsv` (FuncFeatures.java),
  `out/*.c` (Decomp.java)
- 2026-11-06 2nd pass, no Wine needed (`extract3.log`):
  * Advanced Installer SFX (K513-RGB, SHIVA K515-RGB, Shiva K512-RGB): `tools/advx.py` reads the
    file table before the trailing "ADVINSTSFX" (xor flag 2 = first 0x200 bytes ^ 0xff) ->
    `<pkg>/advinst.x/` (MainFeature.cab via 7z -> `cab/`). Driver = SOUI "Shinetek" app, 1a2c:841c /
    4b4d / 4c4d (config.xml `<usb vid pid interface subdevice>`).
  * InstallShield (37 "other" keyboards K530..K630, K552P-KR EU/ES): `tools/isoverlay.py` (PE overlay =
    `name\0 path\0 version\0 size\0 data` records) + `unshield` (brew) on Disk1/data1.cab ->
    `<pkg>/isx.x/cab/`. All share one HidServ.dll (d97af6af, generic HID wrapper); layout in
    Files1/Main.ini (UTF-16, `KEY_<scancode set 1, 0x80|ext> = x,y,w,h,"name"`), photo main_normal.jpg.
  * Inno 6.4 (M998-ULT, K729RGB-M): `tools/inno-dscho/build/innoextract` -> `<pkg>/inno.x/`.
- Not a driver package (VIA / web drivers): K754, K742, K715 PRO, K717WB, K751 (QMK VIA json or a PDF
  with a web-driver URL).

## Families (packages)

| family | pkgs | driver | usb | notes |
|---|---|---|---|---|
| sino-oem | 140 | OemDrv.exe (MFC, "BY" OEM) | 258a:xxxx (Sinowealth), 25a7 | Cfg.ini driven; kb + mice + combos |
| sino-kbd | 70 | KeyboardDrv.exe | 258a | Cfg.ini driven keyboards |
| xyui | 50 | 700N(0X3B4F)defineV9 / DeviceDriver.exe, XYUI.dll | 1a2c | keyboards |
| holtek-rdcfg | 39 | RDCfg.exe (13 builds) | 04d9 | already the app's base |
| holtek-ldcfg/mmcfg | 15 | ldcfg.exe+hid.exe / MMCfg.exe | 04d9 | all PIDs in RDCfg list except Mammoth M801 (fa56, MMCfg) |
| mdb | 36 | "Mouse Drive Beta.exe" | 3554 (CompX) etc | Config.ini (base64 ids "Redragon"+hex), driver_sensor.h |
| hidusb | 17 | Qt + HIDUsb.dll (3 with .pdb/.obj!) | 3554? | same vendor as mdb (driver_sensor.h) |
| duilib | 25 | DuiLib+osConfLib (NSIS niuniu) | ? | mice M816-LIT, M725-LIT, M995GB-LIT... |
| jieli | 22 | Qt + jl_firmware_upgrade (magnetic kb) | ? | K617RGB-MP, K686BG-RGB-M, K580-M... |
| duilib-kb | 13 | DuiLib + opencv | ? | K719, K776, K771BGP, K708, K724 |
| other | ~25 | singletons | | |

## Holtek (RDCfg) status
- New product names vs app registry: `2935 3395` (fc71 rev 2.xx), `2761-F` (fc49 rev 4.xx)
  in the 54-model build (sha 9e8c21b1, 19 pkgs); `G881-B` (fc71 rev 1.xx) = the single-model
  builds of M724, M695(CT), M619, M995, M602-STD, M607-STD, M812-RGB, M718-AIR, M925, M923, S165
  (all report the same id; per-build data differs -> variants the user picks);
  `RD-GMMS02-BK` (fc75 rev 1.xx, M711-FPS-1 V2 build; same id range as "2915").
- match_funcs.py maps ref (old RDCfg) functions to each build; chain functions
  (LED variant 431a30, sensor 4314d0) of multi-model builds: candidates 432550/431fd0 (9e8c21b1).
  Single builds: sensor getter is the call compared with 0xcfd in the DPI writer
  (dcff7ffe: FUN_00423b30 in FUN_0044a3e0; stage data at DAT_004d0d10 + p*0x2518).

- Single-model builds read the product from `<app>/Default.bin` (QSettings INI [PRODUCT]);
  `gen_singles.py` turns them into registry entries (`gen_models.py --extra`). The Default.bin
  getters are stubbed in the emulator (env `EMU_STUB=FUN_x=hex`, Emu*.java) since QSettings
  is not emulated; resolve_rdcfg's map swaps LED/sensor getters there (they have no constants).
  - classic (cb8d6b2f dcff7ffe ddfab882 dfd3de91): VID = Mouse1^Name1, sensor = Name1^Name2.
    M711-FPS-1 V2, M995, M723-RGB, M724, M812-RGB.
  - bulk (9b650769 9d3c8b4b c278b9e3 c8986af2 eeba115f f86486fa): **VID 0x36b7**
    (Mouse1^Name1, checked in FUN_00448000 asm), sensor = hex digits of Name2 read as decimal
    (FUN_00422b10; "3311", "208"). Same memory map as the M801; a profile block (0xaf bytes
    from its base) goes in one 256-byte feature report 4 `04 f3 lo hi af 00 00 00 data`
    (FUN_00450550; EmuCall run confirmed the layout: base+2 stage, base+4+6i stages,
    +0x40 count, +0x42+4i buttons, wheel at index `Wheel`=22 -> 0x9a), macros the same way
    with len 200 (FUN_0044bab0). LED: same bytes (0x449+8p, +0x80 at 0x448+8p), sent as one
    report 3 of 0x32 bytes from 0x446 (FUN_00448c40, variant 3). The builds never read the
    mouse (no hid_get_feature_report/hid_read import) -> Dragonize reads with the classic f2
    (unverified on these firmwares). Light variant 10 (M728-AIR) = other LED map, unsupported.
  - `EmuCall.java`: run any function with stack args / pokes / stubs, dumps buffers and
    out-params (`EMU_DUMP`); `~/.cache/redragon-all/tools/emucall.sh`. One Ghidra run per
    project at a time (project lock).

## Sinowealth OemDrv (sino-ms build 86e99000)
- report 5 (6 bytes) = command: `05 CMD ...`; report 4 (520 bytes) config; report 6 (1032 bytes) big data.
- read config profile n: cmd 0x11/0x21/0x31 (FUN_00413720) then GET report 4;
  write: report 4 `[cmd][0][0x7b] ... data @7` (FUN_00413800); 0x12/0x22/0x32 = 2nd block (FUN_004138b0).
- cmd `05 83` -> 136 bytes (FUN_0040fe70), `05 84 c8` -> 1024 bytes (FUN_00410170, key matrix),
  `06 04 c8 00 40 ...` write 1024 (FUN_00410050), password `05 01 PSD...` (FUN_0040fca0, Cfg PSD=).
- FUN_00411960 = ApplySetting (keyboard: LED matrix, key matrix, Fn matrices, macros 0xcc..0xd8).
- Builds: FuncFeatures of every OemDrv.exe in the mouse packages (`tools/sino_feat.sh` in the
  work dir) + match_funcs.py against 86e99000 for 413290/413720/413800/4138b0/415260/4140d0.
  All six match in a41b18bc 62ac35b8 7a61779c 1e0b64f3 b7b42187 64858b09 f4072cc7 (gen_sino.py
  VERIFIED). The others (030c3779 987426b5 670711b3 de32d483 c1111b9b 6051954c ff62fb10 ac3d8a3f
  dbab1e04 28bab71d d483a086 5710f22a 43d56b1f 2bb2937b bede2dce) have no 0x7b general-block
  writer = older/other protocol, handled below or still skipped.

## Old OemDrv builds = CompX flash over feature report 8 (Proto::CompxFeature)
M901P-KS (5710f22a) joined BUILDS 2026-11-06 (8 functions hash-identical to 670711b3, 0x3335 table identical).
25a7:fa08 has no CID/MID: M913 / M915RGB-WL / M808-KS / M901P-KS are all listed by scan.
- Lowerdev.dll builds 670711b3 (decompiled, ghidra/out/sino-ms-670711b3.c) de32d483 6051954c
  dbab1e04 28bab71d d483a086: SendCommand FUN_00440da0 = SetFeature report 8, 17 bytes, the CompX
  packet and checksum (FUN_0043ff10); reader thread FUN_0043f7f0 ReadFile 17 bytes = input report 9
  `09 CMD STATUS .. DATA@6`, no check byte; ReadEEPROM 8 / WriteEEPROM 7 both wait for the echo
  (FUN_0043f6e0). Flash map and key codes = CompX (GetProfile FUN_00409950, code->keyinfo
  FUN_004094c0 inverted in gen_sino_old.py). Stage = x, y, flag(only X's), chk (FUN_00440a60).
  DPI: FUN_004369a0 tables (EmuCall memory dump -> emu/sino_old_builtin.json) + FUN_00437260 rules;
  the source arrays are byte-identical in the 6 builds. Cfg.ini [OPT] Sensor picks the [SENSOR_n].
- DEV_n builds 43d56b1f (decompiled) bede2dce (all protocol functions match) 2bb2937b (read side
  matches): same packets (SendCommand FUN_00451420, GetData FUN_00450130 = input report 9). One
  dongle id 25a7:fa7b/fa7c serves M994/M995/M917/M693/M806: CheckPsd FUN_0044ff10 = command 1
  `01 00 00 00 08 rand4`, answer data[4..6] = CID, MID = the Cfg [DEV_n] CID/MID; Dragonize
  asks it in scan() (compx::psd) when several registry entries share the id (Model.bcd =
  CID<<8|MID). Stage writer FUN_00450de0: ex = flagX | flagY<<4; 0x3395 x>>8 in bits 2-3/6-7.
  FUN_0040c890: 0x3395 x = cpi/50-1 (< 26100), else cpi/100-1 flag; 0x3104 table (DAT_005de858),
  flag above 4000; DPISET/DPIHW of Cfg.ini override. Photo skins/<CID><MID>/mouse_nr.png.
- "CDrvBY24G" 987426b5 (M602-KS) c1111b9b (M910-KS, same code shifted): Proto::SinoBy24g in
  sino.rs. Command feature 5 (8 bytes `05 c b1 b2 b3 len 0 0`), data feature 8 (520 bytes, data
  @8), general block 0x92 with the enabled stages compacted as 16-bit DPI values (FUN_0040b700),
  matrix 0x50 + 0xa5; mode = connection (wired 0x11, dongle 0x21). DPI tables: FUN_0040c500 in
  EmuCall (EMU_DUMP=5d1928:8558) -> emu/by24g_builtin.json; Cfg DPISET overrides (gen_sino.py).
- Still unsupported: ac3d8a3f (M712, hMatrix/hGeneral/hNotify/hMode). 030c3779 / ff62fb10
  (M811-RGB-PRO, M690-PRO) are CDrvBY24G (Apply_24G FUN_0040bcb0 = 987426b5's FUN_0040bd20; GetSensor
  FUN_0040e810 adds 0x17 = 3395): in gen_sino.py BY24G. Kind 2 K lines (keys) = FUN_004140d0:
  `0x21, mods, usage(VK), usage2`.
- duilib family (20 mice, Sonix-style "DefaultData" JSON, exe decompiled duilib-m916plus.c):
  64-byte output report `04 sumLo sumHi CMD LEN OFFlo OFFhi 0 DATA` (sum of bytes 3..63), answer
  echoes CMD at [3], status [7] (0xff/0xfe = error); FUN_00483d20 = transfer. Chunked commands:
  5 read / 6 write (profile data, e.g. 0x2d bytes at profile*0x19), 7/0x1b/10 read, 9/0x15/0x17/
  0xb/0x12 write, 1/2/0x1c/4/0x1a control. Decoded: section "duilib mice and keyboards" below.
- Mouse block layout and button codes: see src/sino.rs header. Lighting not decoded.

## CompX (families mdb + hidusb; 3554:xxxx, wired + dongle)
- "Mouse Drive Beta.exe" is .NET: `ilspycmd -p` (podman mcr.microsoft.com/dotnet/sdk:8.0,
  `DOTNET_ROLL_FORWARD=Major`, ilspycmd 8.2.0.7535) -> `~/.cache/redragon-all/ilspy/mdb-m916/`.
  It embeds HIDUsb.dll with Costura (`costura32.hidusb32.dll`); the Qt packages ship it in
  Sys32/Sys64. Both decompiled (ghidra/out/hidusb-mdb.c, hidusb-m933.c): mouse paths identical
  (the 0x80 length flag is keyboard only, UsbServer_KeyboardStart).
- Transport, flash map, checksums: src/compx.rs header. ReadAllFlashData = flash 0..0xc0.
- DPI: mdb DPIControl.ValueToDPI/UpdateValue (x = cpi/(50*mul)-1, DPIex flags 0x11/0x22/0x33
  per DPIRange segment, sensors 3395/3950 put x>>8 in DPIex bits 2-3 and 6-7; "discrete"
  sensors use the SENSOR_<s>_DPI_<cpi> table of driver_sensor.h). Qt drivers (hidusb):
  Buttoninterface::HexToDpi (from the shipped .obj with symbols, M923-WL) = same table/formula;
  multi-segment ranges assumed as mdb (unverified there) and capped where x > 0xff.
- Buttons: KeyFunMap [type, p1, p2] at 0x60+4k; shortcut keys at 0x100+0x20k (ShortcutKeyToBuffer).
- Config.ini (base64 "Redragon"+text): gen_compx.py. Many products share 3554:f55e/f5d5; the
  firmware tells them apart by CID/MID (cmd 01 reply bytes 10-12) - not used yet: the first
  registry entry is picked, the user can switch model.

## Sinowealth keyboards (KeyboardDrv.exe + keyboard builds of OemDrv.exe; 258a:0049 / 010c / 019d)
- Protocol from the decompiles (KeyboardDrv 801262a0 FUN_004110e0 ApplySetting, OemDrv 4f3eab8b
  FUN_0048a6d0 CDev916KB::ApplySetting): `app/src/kbd.rs` header has the full map. Command = feature
  report 5 `05 CMD ADDR ..` (size from the report descriptor), data = feature report 6 (1032 bytes,
  data at byte 8). 83/03 ProAddr general block (0x88 KeyboardDrv, 0xfe OemDrv), 84/04 KeyAddr key matrix
  (256 LE u32), 08 RgbTabAddr colour table (write-only: OemDrv's reader has no callers), 05 81 password.
- Cfg [OPT] ProAddr/KeyAddr/RgbTabAddr override the build defaults (b6/d4/b8 or c6/d8/c8).
- Key codes (keyinfo_to_hardware_code FUN_00489960): type 0 key (b2 modifier mask, b3 usage), 6 modifier,
  7 mouse f0-f4, 4 consumer usage, 0x20 Fn, 0x10 macro, 0 disabled.
- One id = dozens of keyboards: scan() asks the password (6 bytes) and matches Cfg PSD.
- Generator: `fleet/gen_kbd.py OUT.json` (Cfg.ini [KEY] K lines or Dev/kb/KB.ini, keyimg.png, protocol
  constants found in the exe by byte patterns) -> `gen_models.py --kbd` (photos scaled to 650x300,
  256 colours, assets/vendor/kbd). 97 keyboards (3 entries without PSD skipped).
- "BYK901" OemDrv builds 045be641 6fccdd79 c755de9a 288e15ea (Cfg.ini LedOpt = mode,speed,light;
  decompiled ghidra/out/kb-c755de9a.c, kb-6fccdd79.c + its Lowerdev.dll): ApplySetting FUN_00403210.
  Key matrix = feature report 9 (0x7e LE u32, GET when SyncKB, SET whole); codes FUN_00402b30
  (kbd.rs `byk_code`); VK->usage table FUN_0042e3a0 (pairs from `29 1b 3a 70`). LED = report 10
  (0x29 bytes: [0] LedOpt mode, [1] ColorGroupHw[colour] (firmware colour groups 1..15), [2] speed
  via FUN_004031d0 (1->5 .. 5->1), [3] unclear), report 11 = per-key colour group (0x7e), report 12 =
  macro buffer (0x780). No password: 258a:013b / 002a shared by 4 / 3 keyboards, the first entry is
  picked. Only the key matrix is supported (Kbd.byk, Lighting tab shows a notice). +7 keyboards.
- Mode names: Text/en/text.xml tc_led_mode<n> (KeyboardDrv) or tc_kb_led<n> (OemDrv), n = LedOpt number.
  21 builds have no Text/ (names drawn in skins): name voted from the hardware code, which maps to the
  same name in 78 of 79 named builds (hw 0 OFF, 1 Fixed on, 2 Respire, ... 21 Self-define).
- App: Keys tab (photo, click a key: default / key / combo / mouse button / media / disabled; changed keys
  red), Lighting tab (modes, 7-colour palette per mode + colourful, brightness, speed), Info. Profile in
  models/<key>/Keyboard.ini (blocks + palettes). Apply merges only the edited fields into what the
  keyboard holds, writes matrix / colour table / general block, reads back and compares. Not written:
  per-key LED matrix (09) and macros (05 dc..). Unverified on hardware.

## xyui family (DeviceDriver.exe + XYUI.dll; 5 mice, ~31 keyboards, many combos)
- config.xml: product list (name, layout_file, type 601.. wired / 701.. 2.4G, vid 1A2C, pid);
  layouts/<layout>.xml: keys (key_value, func_type/func_value), light modes, dpi groups.
- Decompile of the M821-AIR build: ghidra/out/xyui-m821.c. Output report 5, 64 bytes (FUN_00440b30;
  FUN_0044e630 = 65-byte write, FUN_0044e710 read answer). Commands are immediate settings, no
  memory map: wired types (601-700) `05 08 f7 VAL ..` / `05 08 f6 KEY FUNC`, 2.4G (>700) prefix
  `05 bb aa 99 88 09 f7 ..` / `.. 09 f6 ..`, block transfers `.. 09 f0 ..` (FUN_00426cb0).
  Correction: FUN_0044d380 writes 65 bytes with byte 0 = 0 (no report id), so wired packets are
  `00 08 CMD ..`, 2.4G `00 bb aa 99 88 09 CMD ..`. Write-only (no settings read-back found):
  f9 DPI `group, colour, cpi/100-1` (table DAT_005671c0 = 100..12800 step 100, FUN_004251a0),
  f8 report rate = radio 0..3 (FUN_0042f9e0; Hz order of the radios not found), f7 profile
  (FUN_0042f910), f6 key `profile, key_value (0xd0 left..0xd6 DPI-), func_value` (FUN_00426fa0),
  fd sleep time, fc/fb lighting, f0 macro/keyboard blocks. All 5 mice use layout MOUSE_DM204
  (func_type 1: 1 left, 2 middle, 3 right, 5 forward, 6 back per the layout defaults).
  * Integrated (xy.rs, Proto::Xyui, gen_xyui_mice.py; 14 packages -> 5 entries). Interface usage
    page 0xff00 usage 1 (FUN_00440e20). f8 radios 0x20d..0x210 = 125 / 250 / 500 / 1000 Hz -> 0..3.
    f9 = `index, dpi_color<i>_value, table index`, one group per packet (sent for all 5, current last;
    colour sent = index, assumed). f6 profile = the driver's DB row id (1 for its first profile,
    assumed). Function values (FUN_00426730): d0 left d1 right d2 middle d3 forward d4 back d5 DPI+
    d6 DPI- d8 DPI cycle, e8..f9 media (kbd.rs XY_MEDIA), else HID usage; the layouts' defaults are
    type 1: 1 left 2 middle 3 right 5 forward 6 back (DPI keys carry 5: default taken from the label).
    Only keys that differ from the layout default are sent. Light fc `mode-1 bright speed colorful
    direction R G B` (t_light_data fields; layout <light> info mode/attribute) and fb = reset: not used.
    Photo marks: rect "x1#x2#y" (mouse end x2 for direction 0, x1 for 1) - (65, 5), fitted by eye.
- xyui keyboards (S155 build, ghidra/out/xyui-s155.c; XYUI.dll -> xyui-s155-ui.c): wired = feature
  report 7, 8 bytes, via HidD_SetFeature (DAT_0057bfe4); GET feature 7 (FUN_0043fed0, IOCTL 0xb0192).
  2.4G = 65-byte output report `00 bb aa 99 88 ..` (FUN_0043e830). Settings live in a local SQLite DB
  (t_light_data, t_key_macro_data, ... schemas in the exe strings).
  * Light FUN_0040ecc0: `07 f7 MODE-1 colorindex direction speed-1 bright-1 combo` (bright/speed
    stored 1-based, 0 kept 0); mode 4: color_value bytes 0,1,2 into [4],[5],[7] (bright kept); mode 9:
    [5] = speed unchanged; mode 8 first sends `07 f7 00`. Layout <light><info name=lan-id mode
    attribute=bitmask>; names in language/*.lan (UTF-16, ids 501..521, 100..104 music).
  * Keys: slots. `07 fb 01` + 4 GET feature 7 = 28 bytes: [1..12] key codes in the 12 macro
    slots (types 3 macro / 7 = f1 blocks, FUN_0040e580: 28 x `07 f1 ..6 bytes`), [14..23] key
    codes in the 10 simple slots (types 1, 2, 12): FUN_0040e470 `07 fa 01 SLOT KEY 01 VALUE 01`.
    "Disable key" = type 1 value 1 (DAT_0055e8e0). key = layout key `code` (HID usage).
    Function lists (type, value) per menu item: XYUI.dll KeyFuncControl::InitItemInfo.
  * Integrated (kbd.rs `xy_packets`, gen_xyui.py): matrix indexed by HID usage, value = usage,
    1 = disabled, e8-f9 = consumer functions (XY_MEDIA); speed/brightness hw = level index (0-based
    on the wire). Colour index assumed = order of the colour buttons (red green blue yellow magenta
    cyan white, 7 colourful): unverified. Mode 4 sends the RGB only when its attribute has 0x10/0x80
    (KUMARA ELITE's mode 4 is "Neon stream", attr 3: its build not decompiled). Combo byte (attr
    0x100) sent 0, direction 0. 2.4G ids not registered (wired pid only).

## Shinetek (SOUI) keyboards K513 / K512 / K515 (advinst.x/cab, `gen_shinetek.py`, Kbd.st)
- K513RGB.exe decompiled (ghidra/out/shinetek-k513.c, objdump in the scratchpad for the asm).
  HidD_SetFeature report 7, 8 bytes = the xyui keyboards' commands: f7 light (FUN_00420520),
  fa key slot `07 fa PROFILE+1 SLOT+1 KEY 01 VALUE 01`, f1 macro blocks, fb slot read-back (2 GETs),
  fc profile (FUN_0041cfb0), f0 aa factory reset. Output report 5 (FUN_0041b790) goes to the
  vhidflt virtual device (VHidev), not the keyboard; IOCTL 0x22a000 = the filter's key table.
- Light struct per mode at +0x47c + 5 * mode (FUN_0041fcc0): [0] colour radio 1-7 (mode 8 custom:
  RGB nibbles), [1] direction 1/2, [2] speed 0-3, [3] brightness 0-3. Sent as `07 f7 MODE [0..4]`,
  [0] remapped in mode 0 (1->7, 3->4) and mode 4 (1->7 2->4 3->2 4->1 6->3 7->6). Mode = combo
  index (dlg_main.xml cmb_led_mode data - 1). Colour order of the radios assumed = XY_PALETTE.
- Media keys: the driver's funkey codes are consumer usages (config.xml 0c00xx); their slot value
  was not traced, so Dragonize does not offer them on these keyboards.

## QMK/VIA keyboards (K717-PRO, K715-PRO, K742, K754, K751)
- Only the VIA definition .json ships. `fleet/gen_via.py` -> emu/via_kbd.json (+ drawn photo):
  KLE layout (layout option choice 0), matrix row*cols+col, lighting menu ids (or VIA's built-in
  qmk_rgb_matrix list). Kbd.via; protocol in kbd.rs header (raw HID 0xff60, 04/05 keycodes,
  07/08/09 lighting). Keymap entries not shown in the layout are kept (u32::MAX).

## HidServ keyboards (InstallShield "<model>.exe" + HidServ.dll; 37 packages -> 29 entries)
- Decompile: ghidra/out/hidserv-k580.c (Redragon K580RGB.exe). Device: Sonix 0c45:5004 (also
  0c45:5204 K550, 0c45:5304 K551-N, 04b4:5004 K567), interface usage page 0xff1c usage 0x92,
  64-byte output report `04 sum_lo sum_hi CMD LEN OFF_lo OFF_hi 00 data[56]`, sum = bytes 3..63
  (FUN_00441d10 etc.), answer read back (FUN_00440b60, 4 tries), byte 7 ff/fe = refused, data at 8.
  320f:5000 (Evision) is also accepted by most builds with a 0x80-entry table of unclear layout:
  not registered.
- Commands: 01 begin / 02 end around every write; 03 config 0x2c bytes (+0x0a active profile 0-2,
  +0x0c.. 32 mode codes the firmware offers = the mode combo, FUN_00425e30); 05 / 06 lighting block,
  0x2a bytes per profile: +0 mode, +1 brightness 0-5 (slider pos), +2 speed = 5 - slider pos,
  +3 direction ff/00, +4 1 = colourful, +5..7 R G B (COLORREF order), +0xf / +0x11 mode specific;
  07 / 08 key table, 0x7e entries x 3 bytes at profile*stride + i, 0x38 per packet (stride 0x17a,
  K550 0x200 per its static header); 09/0a/0f/11 macros / per-key colours (not used).
- Key entry (FUN_004378a0): [02 02 usage] key, [02 01 mask] modifier, [01 01 1/2/4] mouse L/R/M,
  [01 05 01/ff] wheel, [03 lo hi] consumer, [05 01 n] macro, [05 02 00] / [05 05 n] functions,
  [05 00 00] Fn entries, 0 = disabled. Table index = column*8 + row of the K580 table; most builds
  ship only its first 52 entries (the rest is read from the keyboard, FUN_0043fb10), every complete
  table equals K580's, so gen_hidserv.py fills the gaps from it. Keys placed by matching Main.ini
  KEY_<set-1 scan code, 0x80 = E0> to the scan code of the entry (FUN_00443400). Right numpad
  column (9 6 3 * + Enter) is outside the 0x7e entries: not remappable (driver neither).
- Per-mode controls (FUN_0041e010, assumed for all builds): speed 1-5, 7-15; colour 1-3, 5-10,
  13-16; colourful 1-3, 5, 7-9, 13-15; mode 20 (per-key colours) skipped. Mode names: Lang0409.ini
  [MODE] or (newer "Mechanical Gaming Keyboard.exe" builds) Main.ini [Mode].
- No model id on the wire: one USB device is listed once per candidate HidServ model of its id,
  the device selection dialog picks it (device.rs scan, ui.rs prefers the current model).
- Integrated: kbd.rs `hs_*`, Kbd.hs, gen_hidserv.py -> emu/hs_kbd.json. Apply = read config,
  lighting block, key table of the active profile; write changed table chunks + mode / brightness /
  speed (+ colour flag / RGB); read back. 311 models, 35 tests, M801 identical. Not on hardware.

## Jieli / Witmod keyboards (Qt app + witmodSdk.dll; 22 packages -> 16 entries)
- Decompiles: ghidra/out/jieli-k617.c (Redragon K617RGB-MP.exe, lossy: loop bodies dropped, use
  objdump on ghidra/bin/jieli-k617.exe, .text delta 0x400c00), ghidra/out/witmod-k617.c (SDK).
- Ids (SDK "supportHidList"): 4a4c:e987 JL Keyboard, e988 JL Dongle, e989 JL USB Keyboard (+ other
  vendors' boards). Interface = USB interface 2 ("mi_02"), 64-byte output reports, id 1 (6 on
  "col06" paths). The SDK rewrites byte 1 on every write: app command n -> wire n + 1 for 0..0x25,
  table in CWitmodHid_HidWriteBuff (0x64.. aliases), length byte 5 must be < 0x3b.
- Identification: `01 0d 00..` (written directly), answer byte 1 = 0d, text from byte 6, first
  comma-separated token = firmware name (CWitmodHid_GetHidAndFirmwareInfoStu). Each package ships
  app/images/hidImages/Keyboard_<name>.png for the keyboards it serves (photo); the qrc holds the
  generic Witmod catalogue (~250 KeyInfo_<name>.config layouts, PosX/PosY/Width/Height/Dec = HID
  usage, coordinates in a 728x250 box, not aligned to the photo). qrc extraction:
  fleet/qrcnames.py + qrctree.py (rcc v1, tree located through a KeyInfo_ name).
- Lighting (FUN_004e3af0, app command 6 = wire 7): `01 07 00 00 00 0e TYPE BRIGHT-1 SPEED-1 R G B
  DIR MODETYPE ON UPDATE COLOURFUL X ..`, bytes 62-63 = msecs & 0xffff (big endian as stored).
  Struct (XML ledKeyboardOrSide, ledType, ledBrightness, ledSpeed, ledModeR/G/B, ledDirection,
  ledModeType, ledOpenOrClose, ledUpdate, then two more ints); default item FUN_00498a80: Wave,
  3, 3, ff 00 00, 0 .., [11] = 1 (sent as COLOURFUL; Static forces it 0). Types (UI order):
  Static 0, Neon 3, Breath 1, Wave 2, Twinkle 9, Snake 5, Radar 4, Auto Ripple 0xe, Striation 0xf,
  Aurora 7, Ripple 8, Reactive 6, Cross 0xb, Fireworks 0x10, Speed Respond 0xc. Brightness/speed
  ranges guessed 1-5; ON sent 1, UPDATE 0 (meaning not found).
- Not decoded: key remap ("Write Six Key"/"Full Key" = app commands 3/4 are key-press reports,
  remap goes through the onboard/profile path 0x17), trigger / rapid-trigger / SnapTap / DKS (0x29,
  0x91, 0x92, 0x2a), side LEDs (7), custom per-key LEDs (8), macros.
- Integrated: kbd.rs `jl_name`, `jl_light`, Kbd.jl (firmware names), gen_jieli.py -> emu/jl_kbd.json
  (LedOpt.hw = ledType + 1). Write-only like xyui; scan asks the name and lists the models that
  serve it (all Jieli models when it does not answer). Keys tab: photo + notice (KBD 18). 327 models,
  36 tests, M801 identical. Not on hardware.

## duilib mice and keyboards (DuiLib drivers, Evision firmware 320f:xxxx; 22 + 17 packages -> 15 + 8 entries)
- Decompile ghidra/out/duilib-m916plus.c (Redragon M916-PLUS.exe; Format() args are lost, use
  objdump on ghidra/bin/duilib-m916plus.exe). Classes by RTTI (vtable slot 166 = the light writer):
  CEevisionMouseDevice, CEevisionKeyboardDevice, CEevisionMousePadDevice, CEevisionHS6533Device.
- Device table FUN_00402810: rows of 11 words (vid, pid, ?, ?, class, .., usage page, usage, ..);
  class 2 = wired mouse, 6 / 0xa = 2.4 GHz, 1 = wired keyboard, 9 / 0xa = keyboard dongle, 3 = pad,
  0c76:1630 headset. Generators parse it from objdump (`mov reg, imm; mov WORD PTR ds:a, reg`).
  The ids are shared widely (22a4/226f: six mice; 5000, 511b/511c: several keyboards): scan lists
  every candidate, the selection dialog names the device. Mouse.json VID/PID (0c45:1007) is a stub.
- Transport = HidServ packets (kbd.rs `hs_packet`) on usage page 0xff1c / usage 0x92, chunks of
  0x38. 01 / 02 bracket every write; 0xaa = dongle linked ([8] = 0xff, only off the wired id).
- Info (cmd 3, 0x22 bytes): [0] 55aa, [5] keys, [7] 0x16 (0 + 55aa = fresh: the driver writes it
  back with 0x16, cmd 4), [9] sensor, [0xb] step, [0xc] max, [0xe] min. DPI display FUN_004b8470
  (raw -> CPI): 3395 50r+50 (0..519), 8925 50r+50 / (r-50)*100+100 (0..149, one byte written to
  both stage bytes), 0x70 r+10 (0..45000), 3104 table-ish (0..max-10), 3325 / other r*step+min.
- Mouse settings (cmd 5/6): [0] active profile; profile block 0x57 at p*0x80 (FUN_004b2830 write,
  FUN_004af610 read): 01 mode 02 bright 03 4-speed 04 dir 05 colourful 06 R 07 G 08 B (the
  reader swaps G/B, the writer and the keyboard / pad code do not) 0b rate (0..3 = 125..1000 Hz)
  0c stage, 0e+9i seven stages [on, ?, dpi lo, dpi hi, ?, ?, R, G, B], 4d-50 sleep timers,
  51 debounce, 52 LOD, 53 linear, 54 ripple, 56 mode. Key table cmd 7 read / 9 write, 3 bytes per
  key, index = the skin's key_<n> = Mouse.json KeyList n (skin tooltips swap left/right).
- Key codes (FUN_00410db0 + tables 6d5d78 / 6d5d60 / 6d5cf0 / 6d5cd8 / 6d5938): 10 mask mouse
  (01 L 02 R 04 M 08 back 10 fwd), 13 1/2/3 DPI + / - / cycle, 20 mask usage key (Ctrl+O = 20 10
  12 in the office table), 30 lo hi consumer, 40 1/2/4 power/sleep/wake, 14 interval count fire,
  11 / 12 swing / wheel, 70 / 71 macro, keyboards: a0 01 Fn, d0 a2 n macro keys M1-M5.
- Keyboard light block (CEevisionKeyboardDevice FUN_0049cb30 / FUN_0049e770): 0x2d bytes at
  p*0x19, same field order as the mouse; modes = default_light.json group 0 "value" (group 1 =
  mouse). Key table as the mouse (126 entries).
- Integrated: du.rs (mice: DPI, stages, rate, buttons; sensor table from the mouse's own info at
  runtime, models.rs `table`), kbd.rs `du_*` (keys + lighting), gen_duilib.py -> emu/du_mice.json
  (--compx), gen_duilib_kb.py -> emu/du_kbd.json (--kbd). Mouse lighting not written. 350 models,
  39 tests, M801 identical. Not on hardware.

## BYDUI mice (DeviceDriver.exe + BYDUI.dll + DeviceDriver.pdb; M998-ULT 372e:1060 USB / 105f 2.4 GHz)
Decompile with symbols: ghidra/out/m998ult.c (DeviceMainDlg::Set* / Get*, DriverComm, ProtocolData).
Interface usage page 0xff00 usage 1 (0xff06 = the TFT screen). Feature report 3, 64 bytes:
`03 CHK 50 LEN TAG CMD AT DATA..`, CHK = sum of bytes 2..63; the driver then GETs report 3 until byte 2
(status 0 ok / 1 fail / 2 busy) is 0 and byte 4 echoes TAG (any value; the driver uses fixed ones).
- read `4f`: AT = page; keys 0..3 (TAG 5+n), configuration 0x40+n (TAG 10+n); 10 bytes from answer[6].
- keys: `31` AT 0 / 10 (10 bytes), `32` AT 20 (4 bytes); single key `30` AT 4*k (SetMouseSideKeyData, byte0 | 0x80).
- configuration: `35` single field (rate AT 1, sensor AT 3 len 3, sleep AT 8), block `36` AT 0/10/20/30
  (TAG 0x90+n) then `37` AT 50 len 1 (SetMouseProfile; bytes 40..49 are not sent), `3a` current DPI.
- config bytes: 0 ic_type (2 = 30000 max, raw cpi above 30000), 1 rate wired | 2.4G << 4 (0..6 =
  125..8000 Hz), 2 (stage+1) << 4 | count, 3 lift-off, 4 debounce, 5 sensor flag & 0x3f | e-sports << 6,
  8 sleep, 11+2i stage i LE16 cpi*2/100-1 (8 stages), 27+3i colour.
- key bytes (GetMouseData): 01 b n 00 mouse (1 L, 2 R, 3 M, 4 fwd, 5 back, 6/7 wheel; n=2 double click),
  05 v DPI (1 +, 2 -, 4 cycle), 00 mods k2 k1 keyboard (HID usages; modifiers as mask), 02 00 hi lo
  consumer, 03 01 01 n macro, 00 00 00 00 off. Default table: L, R, M, back(slot 3), fwd(slot 4), DPI cycle.
App: src/bd.rs reads both areas, writes only changed blocks in the driver's order, reads back.
Not done: macros (f0-like `SetMouseMacroData`), TFT images (SendDeviceTFTData), lighting (menu light=0).

## MosArt mice (C++Builder + JVCL TJvHidDevice; M690 "2645", M688 "2613", M691; 062a:415e)
Decompiles ghidra/out/mosart-m690.c / -m688.c / -m691.c (same send code, tables per exe; Ghidra loses
the Borland register arguments: read call sites with gs/Disasm.java / Callers.java). Output report 5,
`05 CMD P1..P5` rest 0, then wait for an input report (FUN_004080b0). Write-only.
- `b2 i R G B L` stage i (6), colour 0..9, L = level code (table 05 04 00 01 02 03 -> M690/M691 800 1200
  1600 2400 3200 4800, M688 500 1000 2000 3000 4000 7200). `31 RATE LIGHT SPEED 0 0` rate 1/2/4/8 =
  125/250/500/1000. `53 k ff a5 C 0` key k = function code C (after table T: L b5 R b6 M b7 back b8 fwd b9,
  media 9e..a5), `53 k 0 0 0 0` key's own function, `51 off d0..d3` + `52 80` = a key record (Disable).
- Apply: 6x b2, 31, keys, b2 of the current stage. Keys: M690/M691 13 records (UI shows 8 / 9), M688 10.
  Native functions kept as [0, n]: 2 DPI+, 3 DPI-, 4 fire, 5 ALT, 6 CTRL, 7 SHIFT, 8 HOME (not remappable
  to other keys: the record format of keyboard keys is not decoded).

## libdc mice (Qt + CEF qcef.exe, HTML UI view/, libdc.dll Delphi; M805 "FRC-ZYB", M906 "FRC-ZMA"; 04d9:b0d2)
Decompile ghidra/out/libdc.c. Exports dcWrite / dcWriteProp / dcReadProp -> FUN_00500c0c / 00500df8 /
00500ee8: header `CMD bb aa ADDR_lo ADDR_hi LEN_lo LEN_hi 0` (4 write by address, 6 write property,
7 read property), FUN_00501504 puts the report id in front (dropping the 0), FUN_00501474 picks id 2..6
for total 16/64/256/1024/2048 bytes, HidD_SetFeature (FUN_004fe74c). Reads: header in report 2, then
GetFeature of the size for LEN+8, data at 8. The UI (view/js) writes properties only: ids = driver.xml
`index` (4 mode, 40 rate 8/4/2/1, 49 DPI count, 50 current DPI, 52+i stage [en, x lo, x hi, y lo, y hi]
value ceil(cpi/50), 62+n button [4 function bytes, play type, count, delay lo, hi]); function codes =
driver.xml `code` (01 xx mouse, 02 mods usage key, 03 00 lo hi consumer, 04 macro). Values are hex
strings without 0x (getInt16). M906 DPI 250..4000, M805 100..14400 (js.js). Lighting (pLights, 10 bytes)
not done. Device check in the UI: Version prefix FRC-ZMA-V1.0.1 / FRC-ZYB-V1.7.0.00.

## CDrv8832 (OemDrv ac3d8a3f, M712 258a:1007, Cfg [OPT] MsFw=10) -> Proto::Sino8832
Decompile ghidra/out/sino-ms-ac3d8a3f.c. Apply by MsFw (FUN_0040d710): 3 FUN_0040e290, 5/7 FUN_0040ea70,
8 FUN_0040db90, 9 FUN_0040fe10, 10 FUN_0040f310 (Apply_M_CJ_8832_1). FindDev FUN_0040bc90 opens four
collections of usage page 0xff00 usage 1 told apart by feature length (LAB_0040bb40.. : matrix 0x479 /
0x429 for MsFw 2,3,8; general 0x51 / 0x3b; notify; mode). Reports (FUN_0040c380 get / FUN_0040c2b0 set,
id first): 4 general, 6 matrix, 8 mode. General (FUN_0040e980): [1] low nibble = enabled count, [2] low
nibble = X/Y, [4..] 8 DPI codes (X/Y pairs when separate) & 0x7f | 0x80 disabled, [0x14..] LED, [0x2b..]
DPI colour indexes. Mode (SetMode FUN_0040c4f0): [1] mode + 1, [2..4] rate codes (0x125/0x250/0x500/0x1000
-> 1..4), [5] current DPI + 1. Matrix (FUN_0040d990/FUN_0040d400): 3 modes x 10 dwords (sino codes,
0x150 = disabled), then macro buffers. DPI codes = 86e99000's built-in table (3325: cpi/100-1).
sino.rs s8832_*: reads 4 + 8, writes 4, 8, 6 (matrix from the UI, macros zero). Not on hardware.

## CDrv8948 (OemDrv 66995128, S101-4-DE combo mouse "M601RGB", 258a:1007, MsFw=3) -> Proto::Sino
- FindDev FUN_00412370 (PIDs 0x1007 / 0x28, m_hCmd / m_hData / m_hNotify on usage page 0xff00 usage 1),
  Apply_M_STD_8948_1 FUN_00416130. Same wire protocol as 86e99000: SendCmd FUN_00412cb0 = feature 5
  `05 cmd 0 0 0 0`, data FUN_00412e60 / FUN_00412f20 = feature 4 of 0x208 bytes, block at byte 8;
  general 0x11/0x21/0x31 len 0x7b (FUN_00413200), matrix 0x12/0x22/0x32 len 0x50 (FUN_004132b0).
- General encoder FUN_00414c00 = 86e99000's FUN_00415260 layout ([0] 100, [2] rate 1-4 | 0x80 X/Y,
  [3] enabled | position << 4, [4] disabled mask, [5..] 8 DPI codes, [0x15..] 8 colours); lighting
  per MsFw (FUN_004158d0 for low nibble 3) kept as the mouse holds it. DPI code FUN_00466c40 = table
  filled from Cfg DPISET / DPIHW (FUN_0046a7d0: without DPIHW code = index + 1). Gun data (FUN_004133f0,
  MsFw & 0xf0) unused by this Cfg. gen_sino.py VERIFIED.
- 258a:1007 is also the CDrv8832 M712: scan() picks Sino when the interface has feature report 5.

## BY8801 (OemDrv 7a7c5b20, Lavawolf M701, Cfg appdir BY8801, no VID / PID in the Cfg) -> Proto::Sino8801
- FindDev FUN_00432a00: "VID_258A&PID_1007" hard-coded, usage page 0xff00 usage 1, m_hDevice (feature 6,
  0x480 bytes) / m_hGeneral (feature 4, 0x9a) / g_hNotify; checks the password (general [0x93..0x99] vs Cfg
  PSD, all zero here = skipped) and the sensor index (general [0x47] >> 4, FUN_0042bd50: 1 5050, 2 3050,
  3 9800, 4 3320, 5 3330, 6 3360, 7 3610, 8 3212).
- Apply FUN_00408e50: general read-modify-write (rate code at [7] >> 4, stage count [0x46] low nibble, 16
  DPI codes [0x49..] & 0x7f | 0x80 disabled, LED [0x5c..0x60], colours [0x63..]); matrix report 6 = macro
  buffers [0..0x400], current mode + 1 at [0x400], 3 x 10 dwords at [0x401] (FUN_00408a10 puts the key
  number 1-10 in the low nibble), password at [0x479]. Data offsets after the report id.
- Key codes FUN_00408430: mouse 10 f0/f1/f2/f3/f4 (left/right/middle/forward/back), 20 f0 32 02 double
  click, key 60 mods usage usage2, 20 usage .. single key (kind 8 0xa4), 40 00/20/40 DPI cycle/+/-,
  40 n|80 DPI lock, 50 01 disabled, 50 02 / 50 06 others, macros 90.
- DPI tables FUN_0042b550 (built-in, code = index + 1): 3610 = 400 800 1000 1200 1600 2000 2400 2800 3000
  3200 4000 4800 5600 6400 (all 8 in gen_sino.py T8801). The current DPI stage is in neither block.
- Photo: skins/key_nr.png cropped (callout numbers = K line order).

## K598-KNS / K599 (MosArt 062a:8566 / 8556, C++Builder + JVCL; decompile k598kns.c) -> Kbd.mk
- Protocol and fields: src/kbd.rs header (MosArt part). Output report 8 `08 CMD ADDR_lo ADDR_hi LEN DATA`,
  echo on input report 8; 1 sync, 2 begin write, 3 read, 5 write, 7 end. Area 0 (0x18, feature 0x14),
  area 1 (0x6a, feature 0x15: 8 x 13 custom-mode key flags), area 2 (4 x 0x100 key-function blocks, not
  used: the skins comment the macro buttons out). Checksum u16 = 0x8032 + sum (FUN_00437d9c).
- Generator `fleet/gen_mk.py` (18 effects of the DFM list, flags DAT_006b697c). Lighting only; G-mode bits
  ([1]) and the side light ([0x12], [0x13]) are kept as read. Not hardware-verified.

## PixArt M709-1 (Qt + PXICtrl3169.dll, 093a:2534; decompile pxi3169.c) -> Proto::Pixart, src/pxi.rs
- EEPROM 0x2000 bytes of (register, value) pairs, accessed through registers 0x33/0x36/0x37 with feature
  report 5 (pxi.rs header). Unpack FUN_1001bce0 / pack FUN_1001ae70 give the value bits: DPI levels,
  count, default, report rate, button codes (13-bit stream). Button code maps = the static std::maps
  DAT_100cc0d0..e8 (initialisers FUN_10001020..200); UI names from the exe (name -> group << 8 | field at
  0x404c7d..0x405467). The app reads 0..0x600 and rewrites only changed 8-byte rows + the XOR checksum
  row; it refuses when the register bytes are not where FUN_1001ae70 puts them.
- Generator `fleet/gen_pxi.py`. Lighting (BreathingMode / DPI colours) and macros kept as read.
  Not hardware-verified.

## Magnetic K729RGB-M (MFC "Redragon Driver.exe" + mui.dll, 19f5:af08 / af28; decompile k729m.c) -> Kbd.mg
- Protocol: src/kbd.rs header (magnetic part). Commands seen: 1 begin, 2 end, 4 active profile, 5 / 6
  settings block (0x20 at profile << 6), 7 / 8 key map (0x200), 9, 0xa / 0xb per-key RGB (0x180 at
  profile * 0x200), 0xa0 (0x400 at profile * 0x400: switch performance table), 0xa1 / 0xa3 / 0xa5 / 0xa7,
  0xd macros, 0xdd / 0xde, 0xee. SQLite tables (t_light_data, t_key_perf_data ...) name the fields.
- Generator `fleet/gen_mg.py`: 22 effects (music and per-key left out). Lighting only. Not hardware-verified.

## Still unsupported (2026-11-06)
- M601-3 / S101-3 (Config.exe + hid.exe + REDRAGON_MOUSE.sys, PixArt 093a:2530): a kernel filter driver
  does the remapping on Windows (DeviceIoControl); nothing to port without reversing the .sys.
- NOEXE (no driver in the download): S139, K552W-RGB-JPTI, K617GGB-RGB-M.
