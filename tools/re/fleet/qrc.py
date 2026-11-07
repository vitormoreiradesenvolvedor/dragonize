import sys, zlib, re, pathlib
d = pathlib.Path(sys.argv[1]).read_bytes()
out = pathlib.Path(sys.argv[2]); out.mkdir(parents=True, exist_ok=True)
n = 0
for m in re.finditer(rb'\x78[\x01\x5e\x9c\xda]', d):
    i = m.start()
    if i < 4: continue
    size = int.from_bytes(d[i-4:i], 'big')
    if not 16 < size < 8_000_000: continue
    try:
        z = zlib.decompressobj(); t = z.decompress(d[i:i+size*4+1000], size+1)
    except Exception: continue
    if len(t) != size: continue
    ext = 'xml' if t.lstrip()[:1] == b'<' else 'png' if t[:4] == b'\x89PNG' else 'bin'
    (out / f'{i:08x}.{ext}').write_bytes(t); n += 1
print(n)
# uncompressed xml too
for m in re.finditer(rb'<\?xml', d):
    e = d.find(b'\0', m.start())
    (out / f'raw{m.start():08x}.xml').write_bytes(d[m.start():e])
