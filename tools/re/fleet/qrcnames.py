import sys
d = open(sys.argv[1], 'rb').read()
e = d.find('7153'.encode('utf-16-be'))
# find an entry header containing that position: try back up to 200 chars
def ok(o):
    n = int.from_bytes(d[o:o+2], 'big')
    if not 0 < n < 300: return None
    try: t = d[o+6:o+6+2*n].decode('utf-16-be')
    except UnicodeDecodeError: return None
    return (t, o+6+2*n) if t.isprintable() else None
cur = next(o for o in range(e - 400, e, 2) if ok(o) and ok(o)[1] > e and ok(ok(o)[1]))
# walk backwards
while True:
    prev = next((p for p in range(cur - 6 - 2, max(cur - 700, 0), -2) if ok(p) and ok(p)[1] == cur), None)
    if prev is None: break
    cur = prev
start = cur; names = {}; o = cur
while (r := ok(o)):
    names[o - start] = r[0]; o = r[1]
print(hex(start), len(names), file=sys.stderr)
import json; json.dump({'start': start, 'names': names}, open(sys.argv[2], 'w'))
