"""Regenerates the bundled Noto Sans CJK TC subsets from every string file.
usage: python subset_fonts.py   (run from app/; needs fontTools and the
system Noto Sans CJK .ttc files, face index 3 = TC)
"""
import glob
from fontTools.ttLib import TTCollection
from fontTools import subset

chars = set()
for f in glob.glob('assets/strings/*/*.ini'):
    chars |= set(open(f, encoding='utf-8').read())
chars |= set('①②③④⑤⑥⑦⑧⑨：，。…—繁體简体中文ÁÉÍÓÚáéíóúãõçâêôàüÇ')
chars |= {chr(c) for c in range(0x20, 0x7f)} | {chr(c) for c in range(0x400, 0x460)}
text = ''.join(sorted(chars))
for weight in ('Regular', 'Bold'):
    src = f'/usr/share/fonts/google-noto-sans-cjk-fonts/NotoSansCJK-{weight}.ttc'
    font = TTCollection(src).fonts[3]
    opts = subset.Options()
    opts.name_IDs = ['*']
    opts.notdef_outline = True
    opts.layout_features = []
    opts.hinting = False
    s = subset.Subsetter(opts)
    s.populate(text=text)
    s.subset(font)
    font.save(f'assets/fonts/NotoSansCJKtc-{weight}-subset.otf')
    print(weight, len(text), 'chars')
