"""Retouches assets/background/cfgMainback.png (vendor art).

The shipped image has a stray 1px bar on top of the "M" of "GAMING MOUSE"
(row 104, x 658-664, plus its anti-aliased fringe on row 105). The original
program showed it too; the port removes it on request. Idempotent.
usage: python patch_background.py assets/background/cfgMainback.png
"""
import sys
from PIL import Image

p = sys.argv[1]
im = Image.open(p).convert('RGBA')
px = im.load()
for x in range(658, 665):
    px[x, 104] = (0, 0, 0, 255)
for x in range(659, 662):
    px[x, 105] = (0, 0, 0, 255)
im.save(p)
print('patched', p)
