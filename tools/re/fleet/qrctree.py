# Qt rcc v1: names blob (qrcnames.py), tree of 14-byte nodes (name u32, flags u16, then dir: count u32
# first u32 / file: country u16 lang u16 data u32), data blob [u32 size][bytes] (zlib if flags & 1).
import sys, json, zlib, struct, pathlib, re
d = open(sys.argv[1], 'rb').read()
N = json.load(open(sys.argv[2])); nstart = N['start']; names = {int(k): v for k, v in N['names'].items()}
out = pathlib.Path(sys.argv[3]); out.mkdir(parents=True, exist_ok=True)
def node(t, i):
    return struct.unpack_from('>IH', d, t + 14 * i)
def start_of(ref):
    # tree start: walk back while the node's name is a known name offset (or 0 for root)
    t = ref
    while True:
        nm, fl = struct.unpack_from('>IH', d, t - 14)
        if (nm in names or nm == 0) and fl in (0, 1, 2): t -= 14
        else: break
    return t
t = None
for key in (k for k, v in names.items() if v.startswith('KeyInfo_')):
    ref = d.find(struct.pack('>I', key))
    while ref >= 0:
        c = start_of(ref)
        if struct.unpack_from('>IH', d, c) == (0, 2):
            t = c
            break
        ref = d.find(struct.pack('>I', key), ref + 1)
    if t is not None:
        break
assert t is not None
# data base: the zlib header must follow for compressed files
files = []
def walk(i, path):
    nm, fl = struct.unpack_from('>IH', d, t + 14 * i)
    name = names.get(nm, '') if i else ''
    p = f'{path}/{name}' if name else path
    if fl & 2:
        cnt, first = struct.unpack_from('>II', d, t + 14 * i + 6)
        for c in range(cnt): walk(first + c, p)
    else:
        off = struct.unpack_from('>I', d, t + 14 * i + 10)[0]
        files.append((p, fl, off))
walk(0, '')
comp = [f for f in files if f[1] & 1]
cand = {}
for m in re.finditer(rb'\x78[\x01\x5e\x9c\xda]', d[:t]):
    b = m.start() - 8 - comp[0][2]
    if b >= 0 and all(d[b + o + 8] == 0x78 for _, _, o in comp[:20] if b + o + 5 < len(d)):
        cand[b] = 1; break
base = next(iter(cand))
n = 0
for p, fl, off in files:
    size = int.from_bytes(d[base + off:base + off + 4], 'big')
    blob = d[base + off + 4:base + off + 4 + size]
    if fl & 1: blob = zlib.decompress(blob[4:])
    q = out / p.lstrip('/'); q.parent.mkdir(parents=True, exist_ok=True); q.write_bytes(blob); n += 1
print(hex(t), hex(base), n, file=sys.stderr)
