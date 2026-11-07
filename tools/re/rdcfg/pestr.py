# usage: pestr.py exe hexaddr...  -> prints C string at each VA
import sys,struct
d=open(sys.argv[1],'rb').read()
pe=struct.unpack_from('<I',d,0x3c)[0]; n=struct.unpack_from('<H',d,pe+6)[0]; so=struct.unpack_from('<H',d,pe+20)[0]
ib=struct.unpack_from('<I',d,pe+24+28)[0]; sec=pe+24+so
S=[struct.unpack_from('<8sIIII',d,sec+40*i) for i in range(n)]
def off(va):
    r=va-ib
    for nm,vs,va2,rs,ro in S:
        if va2<=r<va2+max(vs,rs): return ro+r-va2
for a in sys.argv[2:]:
    o=off(int(a,16)); e=d.index(b'\0',o); print(a, repr(d[o:e].decode('latin1')))
