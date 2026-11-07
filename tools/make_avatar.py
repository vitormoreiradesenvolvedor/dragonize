"""Builds assets/vortron.png: circular avatar with the UI's red ring.
usage: python make_avatar.py SOURCE.jpg OUT.png [size]
"""
import sys
from PIL import Image, ImageDraw

src, out = sys.argv[1], sys.argv[2]
size = int(sys.argv[3]) if len(sys.argv) > 3 else 64
ss = 8  # supersampling for smooth edges
big = size * ss
im = Image.open(src).convert('RGB')
side = min(im.size)
im = im.crop(((im.width - side) // 2, (im.height - side) // 2, (im.width + side) // 2, (im.height + side) // 2))
photo = im.resize((big, big), Image.LANCZOS).convert('RGBA')
ring = 2 * ss
canvas = Image.new('RGBA', (big, big), (0, 0, 0, 0))
mask = Image.new('L', (big, big), 0)
ImageDraw.Draw(mask).ellipse((ring, ring, big - 1 - ring, big - 1 - ring), fill=255)
canvas.paste((230, 0, 18, 255), (0, 0, big, big), Image.new('L', (big, big), 0))
d = ImageDraw.Draw(canvas)
d.ellipse((0, 0, big - 1, big - 1), fill=(230, 0, 18, 255))
canvas.paste(photo, (0, 0), mask)
canvas.resize((size, size), Image.LANCZOS).save(out)
print('ok', out)
