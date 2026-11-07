#!/bin/bash
# regenerate app/src/models_data.rs + photos from all generator inputs
set -e
# inputs: ~/.cache/redragon-all (work dir: emu/*.json from gen_singles/gen_sino/gen_compx/gen_sino_old)
AP="$(cd "$(dirname "$0")/../../.." && pwd)"
S=$(mktemp -d)
cd ~/.cache/redragon-all
mapfile -t A < "$AP/tools/re/fleet/regen_args.txt"   # the 54-model RDCfg build's Config.ini + skin dir
mkdir -p $S/gen/img
python3 "$AP/tools/re/rdcfg/gen_models.py" --cfg "${A[0]}" --emu emu/9e8c21b1 --skin "${A[1]}" --out $S/gen/m.rs --img $S/gen/img \
  --extra emu/singles.json --sino emu/sino.json --compx emu/compx.json,emu/sino_old.json,emu/du_mice.json,emu/xy_mice.json,emu/bd_mice.json,emu/mosart_mice.json,emu/dc_mice.json,emu/pxi_mice.json --kbd emu/kbd.json,emu/xyui_kbd.json,emu/via_kbd.json,emu/hs_kbd.json,emu/jl_kbd.json,emu/du_kbd.json,emu/st_kbd.json,emu/mk_kbd.json,emu/mg_kbd.json "$@" 2>&1 | tail -1
cd "$AP"
python3 - "$S" <<'PY'
import sys, re, os, shutil
S = sys.argv[1]
g = open(S + '/gen/m.rs').read().split('\n')
cur = open('src/models_data.rs').read().split('\n')
first = lambda ls: next(i for i, l in enumerate(ls) if l.startswith('use crate::'))
open('src/models_data.rs', 'w').write('\n'.join(cur[:first(cur)] + g[first(g):]))
s = open('src/models_data.rs').read()
for kind, src in (('mice', 'img'), ('kbd', 'kbd')):
    used = set(re.findall(r'photo: Some\("' + kind + r'/([^"]+)"\)', s))
    d = f'assets/vendor/{kind}'
    os.makedirs(d, exist_ok=True)
    for f in os.listdir(d):
        if f[:-4] not in used:
            os.remove(f'{d}/{f}'); print('rm', f)
    for k in used:
        # keyboards: always refreshed (the generator scales and quantizes them)
        if kind == 'kbd' or not os.path.exists(f'{d}/{k}.png'):
            shutil.copy(f'{S}/gen/{src}/{k}.png', d)
print(len(re.findall(r'Model \{ key', s)), 'models')
PY
