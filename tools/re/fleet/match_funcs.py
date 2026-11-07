#!/usr/bin/env python3
"""Finds, in another build of a program, the functions matching given ones of a
reference build, from FuncFeatures.java dumps of both.

1. functions with a mnemonic hash unique in both builds are matched directly;
2. the rest are matched iteratively by similarity of what they reference:
   callees (already-matched internal functions are renamed to their reference
   address, so call graphs line up), constants and strings.

usage: match_funcs.py REF.tsv NEW.tsv ADDR[,ADDR...]   -> prints "ref_addr new_addr score"
       match_funcs.py REF.tsv NEW.tsv --all              -> full map
"""
import sys
from collections import Counter, defaultdict


def load(path):
    fs = {}
    for line in open(path, encoding='utf-8', errors='replace'):
        p = line.rstrip('\n').split('\t')
        while len(p) < 6:
            p.append('')
        a = p[0].split(':')[-1].lstrip('0') or '0'
        fs[a] = dict(n=int(p[1]), h=p[2], calls=[c for c in p[3].split(',') if c],
                     consts=set(c for c in p[4].split(',') if c), strs=set(s for s in p[5].split('\x01') if s))
    return fs


def fun_addr(name):
    if name.startswith('FUN_'):
        return name[4:].lstrip('0') or '0'
    return None


def feats(f, rename):
    s = Counter()
    for c in f['calls']:
        a = fun_addr(c)
        if a is None:
            s['x:' + c] += 1
        elif a in rename:
            s['m:' + rename[a]] += 1
        else:
            s['u'] += 1
    for c in f['consts']:
        s['c:' + c] += 1
    for t in f['strs']:
        s['s:' + t] += 3
    return s


def sim(a, b):
    inter = sum((a & b).values())
    union = sum((a | b).values())
    return inter / union if union else 0.0


def match(ref, new):
    m = {}  # ref -> new
    hr = defaultdict(list)
    hn = defaultdict(list)
    for a, f in ref.items():
        hr[f['h']].append(a)
    for a, f in new.items():
        hn[f['h']].append(a)
    for h, ra in hr.items():
        if h != '0' and len(ra) == 1 and len(hn.get(h, [])) == 1:
            m[ra[0]] = hn[h][0]
    for _ in range(6):
        inv = {v: k for k, v in m.items()}
        rename_new = {n: r for r, n in m.items()}
        rename_ref = {r: r for r in m}
        free_new = [a for a in new if a not in inv]
        fn = {a: feats(new[a], rename_new) for a in free_new}
        by_tok = defaultdict(set)
        for a, s in fn.items():
            for t in s:
                if t != 'u':
                    by_tok[t].add(a)
        added = 0
        for r, f in ref.items():
            if r in m:
                continue
            fr = feats(f, rename_ref)
            cands = Counter()
            for t in fr:
                for a in by_tok.get(t, ()):
                    cands[a] += 1
            best = sorted(((sim(fr, fn[a]) - abs(f['n'] - new[a]['n']) / max(f['n'], new[a]['n'], 1) * 0.2, a)
                           for a, _ in cands.most_common(40)), reverse=True)
            if best and best[0][0] > 0.55 and (len(best) == 1 or best[0][0] - best[1][0] > 0.08):
                m[r] = best[0][1]
                added += 1
        # resolve collisions: keep the best-scoring ref for each new function
        by_new = defaultdict(list)
        for r, n in m.items():
            by_new[n].append(r)
        for n, rs in by_new.items():
            if len(rs) > 1:
                for r in rs:
                    del m[r]
        if not added:
            break
    return m


def main():
    ref, new = load(sys.argv[1]), load(sys.argv[2])
    m = match(ref, new)
    if sys.argv[3] == '--all':
        for r, n in sorted(m.items()):
            print(r, n)
        return
    for a in sys.argv[3].split(','):
        a = a.lower().lstrip('0')
        print(a, m.get(a, '-'))
    print(f'# matched {len(m)}/{len(ref)}', file=sys.stderr)


main()
