#!/usr/bin/env python3
"""For every RDCfg.exe build, finds the functions (and the one data address)
that EmuModels.java / EmuDpi.java use, expressed as "ref name -> this build".

Ref = the build the app's registry came from (REDRAGON Gaming Mouse.exe, 2022).
Uses match_funcs.py (call-graph/constant matching) for the protocol functions
and the decompiled C of the build for the per-model ones:
  - sensor getter = the call whose result is compared with 0xcfd in the DPI writer
  - LED variant getter = first call of the lighting writer
  - button table walker = the caller of the button writer with the most calls
  - stage values array = first `&DAT_x + param_1 * N` read in the DPI writer
usage: resolve_rdcfg.py OUTDIR  (reads OUTDIR/holtek-rdcfg-*.feat.tsv and *.c)
writes OUTDIR/<build>.map: lines "REF NEW"
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(__file__))
src = open(os.path.join(os.path.dirname(__file__), 'match_funcs.py')).read().replace('\nmain()\n', '\n')
mf = {}
exec(src, mf)

OUT = sys.argv[1]
WANT = ['431a30', '4314d0', '406780', '45d5c0', '45c570', '436d40', '432150', '457b30', '423c30',
        '4a24b0', '4a24f0', '490870', '435e40', '456d90']
REF_STAGE = '4ed1f0'


def funcs(path):
    """name -> body text of a decompiled file"""
    out, cur, buf = {}, None, []
    for line in open(path, errors='replace'):
        m = re.match(r'// ===== (\S+) @ ([0-9a-f]+)', line)
        if m:
            if cur:
                out[cur] = ''.join(buf)
            cur, buf = m.group(2).lstrip('0'), []
        else:
            buf.append(line)
    if cur:
        out[cur] = ''.join(buf)
    return out


def calls_in(body):
    return re.findall(r'FUN_00([0-9a-f]{6})\(', body)


ref = mf['load'](f'{OUT}/holtek-rdcfg-ref.feat.tsv')
for fn in sorted(os.listdir(OUT)):
    m = re.match(r'holtek-rdcfg-([0-9a-f]{8})\.feat\.tsv$', fn)
    if not m:
        continue
    b = m.group(1)
    new = mf['load'](f'{OUT}/{fn}')
    mp = mf['match'](ref, new)
    code = funcs(f'{OUT}/holtek-rdcfg-{b}.c')
    res = {k: mp.get(k) for k in WANT}
    notes = []
    if not res['457b30']:
        c = [a for a, body in code.items() if '== 0xcfd' in body and '0xf3' in body and '+ 4 + ' in body]
        if len(c) == 1:
            res['457b30'] = c[0]
    if not res['456d90']:
        c = [a for a, body in code.items() if "_3_1_ = 'F'" in body and '0xf3' in body]
        if len(c) == 1:
            res['456d90'] = c[0]
    if not res['423c30']:
        c = {}
        for body in code.values():
            for g in re.findall(r'FUN_00([0-9a-f]{6})\(&(local_\w+)\);\s*\w+ = QString::compare_helper\s*\(\2', body):
                c[g[0]] = c.get(g[0], 0) + 1
        if c:
            res['423c30'] = max(c, key=c.get)
    dpi = res['457b30'] and code.get(res['457b30'])
    if dpi:
        s = re.search(r'(\w+) = FUN_00([0-9a-f]{6})\(\);\s*if \(\1 == 0xcfd\)', dpi)
        if s:
            res['4314d0'] = res['4314d0'] or s.group(2)
        d = re.search(r'&DAT_00([0-9a-f]{6}) \+ param_1 \* 0x', dpi)
        if d:
            res['stage'] = d.group(1)
    led = res['456d90'] and code.get(res['456d90'])
    if led and not res['431a30']:
        c = [x for x in calls_in(led) if x != res['456d90']]
        if c:
            res['431a30'] = c[0]
    if res['45c570'] and not res['45d5c0']:
        best = max(((body.count(f'FUN_00{res["45c570"]}('), a) for a, body in code.items()), default=(0, None))
        if best[0]:
            res['45d5c0'] = best[1]
    # remaining per-model functions: best similarity candidate
    rn = {n: r for r, n in mp.items()}
    for k in ('406780', '436d40', '431a30', '4314d0'):
        if res.get(k):
            continue
        fr = mf['feats'](ref[k], {r: r for r in mp})
        sc = sorted(((mf['sim'](fr, mf['feats'](f, rn)), a) for a, f in new.items() if a not in rn), reverse=True)
        if sc and sc[0][0] > 0.2:
            res[k] = sc[0][1]
            notes.append(f'{k}~{sc[0][1]}({sc[0][0]:.2f})')
    # multi-model builds: the two name chains have near-identical features; the sensor
    # one returns sensor ids
    for k in ('431a30', '4314d0'):
        a = res.get(k)
        if a and a in code:
            has_ids = '0xcfd' in code[a] or '0xcff' in code[a]
            other = '4314d0' if k == '431a30' else '431a30'
            if (k == '431a30') == has_ids and res.get(other) and res[other] in code:
                res[k], res[other] = res[other], res[k]
    with open(f'{OUT}/{b}.map', 'w') as o:
        for k in WANT:
            if res.get(k):
                print(f'FUN_00{k} FUN_00{res[k]}', file=o)
        if res.get('stage'):
            print(f'DAT_{REF_STAGE} DAT_{res["stage"]}', file=o)
    missing = [k for k in WANT if not res.get(k)] + ([] if res.get('stage') else ['stage'])
    print(b, 'missing:', missing, ' '.join(notes))
