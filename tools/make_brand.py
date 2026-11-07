"""Generates every image the UI draws (assets/brand/*.png), from scratch.

Original Dragonize artwork: no pixel comes from the vendor's installer or from
screenshots of its program. Sizes and anchor margins match what src/ui.rs
expects, so the layout is unchanged. Run from app/:
    python tools/make_brand.py
"""
import math, os
from PIL import Image, ImageDraw, ImageFilter, ImageFont

OUT = 'assets/brand'
os.makedirs(OUT, exist_ok=True)
SS = 4  # supersampling for anti-aliased shapes
BOLD = 'assets/fonts/NotoSansCJKtc-Bold-subset.otf'
REG = 'assets/fonts/NotoSansCJKtc-Regular-subset.otf'
TAHOMA_B = 'assets/fonts/tahomabd.ttf'

RED = (230, 0, 18)
BRIGHT = (255, 0, 0)
DARK_RED = (102, 0, 0)
MENU = (68, 1, 6)
PANEL = (39, 39, 39)
TAB_OFF = (93, 29, 29)
GREY = (90, 90, 90)
FRAME = (96, 96, 98)
FRAME_D = (52, 52, 54)
WHITE = (255, 255, 255)


def canvas(w, h):
    return Image.new('RGBA', (w * SS, h * SS), (0, 0, 0, 0))


def done(im, name):
    w, h = im.size[0] // SS, im.size[1] // SS
    im.resize((w, h), Image.LANCZOS).save(os.path.join(OUT, name + '.png'))


def s(*v):
    return [int(round(x * SS)) for x in v]


def rrect(d, box, r, fill=None, outline=None, width=1):
    d.rounded_rectangle(s(*box), radius=r * SS, fill=fill, outline=outline, width=int(width * SS))


def font(path, px):
    return ImageFont.truetype(path, int(px * SS))


# ------------------------------------------------------------------ simple widgets

def button(name, w, h, inner, fill, edge, radius=3):
    """Rounded button; `inner` = (x0, y0, x1, y1) of the UI rect inside the piece."""
    im = canvas(w, h); d = ImageDraw.Draw(im)
    x0, y0, x1, y1 = inner
    rrect(d, (x0 - 0.5, y0 - 0.5, x1 + 0.5, y1 + 0.5), radius, fill=edge)
    rrect(d, (x0 + 0.5, y0 + 0.5, x1 - 0.5, y1 - 0.5), radius - 0.5, fill=fill)
    done(im, name)


def framed(name, w, h, inner, fill, border, bw=1.5, radius=2):
    im = canvas(w, h); d = ImageDraw.Draw(im)
    x0, y0, x1, y1 = inner
    rrect(d, (x0, y0, x1, y1), radius, fill=border)
    rrect(d, (x0 + bw, y0 + bw, x1 - bw, y1 - bw), max(radius - 1, 0), fill=fill)
    done(im, name)


button('tab_on', 164, 22, (2, 2, 162, 20), BRIGHT, (90, 0, 0), 2)
framed('tab_off', 164, 22, (2, 2, 162, 20), (0, 0, 0), BRIGHT, 2, 1)
button('btn_func', 149, 22, (2, 2, 143, 20), DARK_RED, (40, 0, 0), 3)
framed('btn_func_hover', 149, 22, (2, 2, 143, 20), DARK_RED, BRIGHT, 1.5, 3)
button('profile_on', 87, 23, (2, 2, 85, 21), BRIGHT, (90, 0, 0), 3)
button('profile_off', 88, 23, (2, 2, 85, 21), DARK_RED, (40, 0, 0), 3)
button('bottom_btn', 93, 28, (2, 2, 91, 26), GREY, (60, 60, 60), 4)
button('dlg_btn', 93, 28, (2, 2, 91, 26), GREY, (60, 60, 60), 4)
button('mac_gray_btn', 89, 27, (2, 2, 87, 25), GREY, (60, 60, 60), 4)
button('mac_btn', 125, 21, (2, 2, 123, 19), DARK_RED, (40, 0, 0), 2)
button('mac_btn_small', 101, 20, (2, 2, 99, 18), DARK_RED, (40, 0, 0), 2)
button('mac_item', 175, 25, (2, 2, 173, 23), (85, 84, 84), (50, 50, 50), 2)
button('dpi_label', 65, 25, (2, 2, 63, 23), BRIGHT, (90, 0, 0), 4)

# popup menu rows
im = canvas(148, 20); d = ImageDraw.Draw(im)
d.rectangle(s(0, 0, 148, 20), fill=MENU)
d.line(s(0, 19.5, 148, 19.5), fill=(52, 0, 4), width=SS)
done(im, 'menu_item')
im = canvas(148, 20); d = ImageDraw.Draw(im)
d.rectangle(s(0, 0, 148, 20), fill=MENU)
rrect(d, (1, 1, 147, 19), 2, outline=BRIGHT, width=1.5)
done(im, 'menu_item_hover')

# check boxes
def checkbox(name, w, h, box, checked, fill=(74, 0, 0), border=None):
    im = canvas(w, h); d = ImageDraw.Draw(im)
    x0, y0, x1, y1 = box
    if border:
        rrect(d, (x0, y0, x1, y1), 1.5, fill=border)
        rrect(d, (x0 + 1.5, y0 + 1.5, x1 - 1.5, y1 - 1.5), 1, fill=(0, 0, 0))
    else:
        rrect(d, (x0, y0, x1, y1), 1.5, fill=fill)
    if checked:
        cx0, cy0, cx1, cy1 = x0 + 2.5, y0 + 2.5, x1 - 2.5, y1 - 2.5
        pts = [(cx0, cy0 + (cy1 - cy0) * 0.55), (cx0 + (cx1 - cx0) * 0.4, cy1), (cx1, cy0)]
        d.line([tuple(s(*p)) for p in pts], fill=WHITE, width=int(2.2 * SS), joint='curve')
    done(im, name)


checkbox('poll_off', 16, 18, (1, 1, 15, 17), False)
checkbox('poll_on', 19, 18, (2, 1, 17, 17), True, border=BRIGHT)
checkbox('dlg_check_off', 19, 20, (2, 2, 17, 18), False)
checkbox('dlg_check_on', 19, 20, (2, 2, 17, 18), True, border=BRIGHT)
checkbox('mac_check_off', 13, 13, (1, 1, 12, 12), False)
checkbox('mac_check_on', 13, 13, (1, 1, 12, 12), True, border=BRIGHT)

# lighting level rows: checkbox + dark label slot
for on in (False, True):
    im = canvas(91, 18); d = ImageDraw.Draw(im)
    if on:
        rrect(d, (1, 1, 17, 17), 1.5, fill=BRIGHT)
        rrect(d, (2.5, 2.5, 15.5, 15.5), 1, fill=(0, 0, 0))
        d.line([tuple(s(*p)) for p in [(4.5, 9.5), (8, 14), (14, 4.5)]], fill=WHITE, width=int(2.2 * SS), joint='curve')
    else:
        rrect(d, (1, 1, 17, 17), 1.5, fill=(74, 0, 0))
    d.rectangle(s(20, 2, 90, 16), fill=(0, 0, 0))
    done(im, 'lrow_on' if on else 'lrow_off')

# edit fields
framed('dlg_edit_red', 304, 30, (1, 1, 303, 29), (0, 0, 0), BRIGHT, 2, 1)
framed('dlg_edit_red_small', 102, 30, (2, 2, 100, 28), (0, 0, 0), BRIGHT, 2, 1)
framed('dlg_edit_gray', 65, 23, (1, 1, 64, 22), GREY, (130, 130, 130), 1, 1)
framed('dlg_edit_gray_wide', 304, 34, (1, 1, 303, 33), GREY, (130, 130, 130), 1, 1)
framed('mac_edit', 35, 17, (0.5, 0.5, 34.5, 16.5), (0, 0, 0), (200, 200, 200), 1, 0)

# lighting mode tabs (top row hangs from the panel top, bottom row sits on its floor)
def ltab(name, w, h, top, on):
    im = canvas(w, h); d = ImageDraw.Draw(im)
    body = PANEL if on else TAB_OFF
    if top:
        d.rectangle(s(0, 1, w - 1, h - 1), fill=body)
        d.rectangle(s(0, h - 2, w, h), fill=RED if not on else PANEL)
        if on:
            d.line(s(0.5, 1, 0.5, h), fill=BRIGHT, width=SS)
            d.line(s(0, 1.5, w, 1.5), fill=BRIGHT, width=2 * SS)
            d.line(s(w - 1.5, 1, w - 1.5, h), fill=BRIGHT, width=SS)
        else:
            d.line(s(w - 0.5, 1, w - 0.5, h), fill=(39, 39, 39), width=SS)
    else:
        d.rectangle(s(0, 2, w - 1, h - 1), fill=body)
        d.rectangle(s(0, 0, w, 2), fill=RED if not on else PANEL)
        if on:
            d.line(s(0.5, 0, 0.5, h - 1), fill=BRIGHT, width=SS)
            d.line(s(0, h - 1.5, w, h - 1.5), fill=BRIGHT, width=2 * SS)
            d.line(s(w - 1.5, 0, w - 1.5, h - 1), fill=BRIGHT, width=SS)
        else:
            d.line(s(w - 0.5, 2, w - 0.5, h), fill=(39, 39, 39), width=SS)
    done(im, name)


ltab('ltab_top_on', 150, 28, True, True)
ltab('ltab_top_off', 149, 28, True, False)
ltab('ltab_bot_on', 150, 30, False, True)
ltab('ltab_bot_off', 150, 30, False, False)

# ------------------------------------------------------------------ sliders

def track_h():
    im = canvas(215, 22); d = ImageDraw.Draw(im)
    rrect(d, (2, 2, 212, 20), 3, fill=BRIGHT)
    # dark-to-red gradient groove
    g = Image.new('RGBA', (int(206 * SS), int(14 * SS)))
    gd = ImageDraw.Draw(g)
    for x in range(g.size[0]):
        t = x / g.size[0]
        gd.line([(x, 0), (x, g.size[1])], fill=(int(10 + 220 * t ** 1.4), 0, int(10 + 8 * t)))
    mask = Image.new('L', g.size, 0)
    ImageDraw.Draw(mask).rounded_rectangle([0, 0, g.size[0] - 1, g.size[1] - 1], radius=2 * SS, fill=255)
    im.paste(g, (4 * SS, 4 * SS), mask)
    for k in range(20):
        x = 9 + k * 10.3
        hh = 4 if k % 2 else 2.5
        d.rectangle(s(x, 11 - hh, x + 1.2, 11 + hh), fill=WHITE)
    done(im, 'track')


def track_v():
    im = canvas(37, 195); d = ImageDraw.Draw(im)
    rrect(d, (13, 8, 23, 190), 3, fill=BRIGHT)
    g = Image.new('RGBA', (int(6 * SS), int(178 * SS)))
    gd = ImageDraw.Draw(g)
    for y in range(g.size[1]):
        t = 1 - y / g.size[1]
        gd.line([(0, y), (g.size[0], y)], fill=(int(40 + 180 * t), 0, 10))
    im.paste(g, (15 * SS, 10 * SS))
    for k in range(18):
        y = 16 + k * 9.6
        w = 1.4 if k % 2 else 2.4
        d.rectangle(s(18 - w / 2, y, 18 + w / 2, y + 1.2), fill=WHITE)
    done(im, 'dpi_track')


def thumb(name, w, h, vertical):
    im = canvas(w, h); d = ImageDraw.Draw(im)
    if vertical:   # slider thumb of the horizontal tracks (15x18)
        rrect(d, (0.5, 0.5, w - 0.5, h - 0.5), 3, fill=(40, 40, 42), outline=BRIGHT, width=1.2)
        rrect(d, (3, 3, w - 3, h - 3), 2, fill=(150, 150, 155))
        d.rectangle(s(w / 2 - 1.3, 4, w / 2 + 1.3, h - 4), fill=BRIGHT)
    else:          # DPI thumb (51x15): grip centred on the 37 px track column
        cx = 26
        rrect(d, (cx - 9, 0.5, cx + 9, h - 0.5), 3, fill=(40, 40, 42), outline=BRIGHT, width=1.2)
        rrect(d, (cx - 6.5, 3, cx + 6.5, h - 3), 2, fill=(150, 150, 155))
        d.rectangle(s(cx - 5, h / 2 - 1.2, cx + 5, h / 2 + 1.2), fill=BRIGHT)
    done(im, name)


track_h(); track_v()
thumb('y', 15, 18, True)
thumb('x2', 51, 15, False)

# ------------------------------------------------------------------ colour picker / masks

im = Image.new('RGBA', (130, 122)); px = im.load()
for y in range(122):
    for x in range(130):
        u, v = x / 129, y / 121
        r = (255 + (255 - 255) * u) * (1 - v)
        g = (255 + (0 - 255) * u) * (1 - v)
        b = (255 + (0 - 255) * u) * (1 - v)
        px[x, y] = (int(r), int(g), int(b), 255)
d = ImageDraw.Draw(im)
d.rectangle([0, 0, 129, 121], outline=(20, 20, 20))
im.save(os.path.join(OUT, 'picker.png'))

im = canvas(24, 24); rrect(ImageDraw.Draw(im), (0, 0, 24, 24), 3, fill=WHITE); done(im, 'swatch_mask')
im = canvas(17, 16); d = ImageDraw.Draw(im)
d.line([tuple(s(*p)) for p in [(2, 8.5), (6.5, 13.5), (15, 2.5)]], fill=WHITE, width=int(3.4 * SS), joint='curve')
done(im, 'check_mask')

# ------------------------------------------------------------------ small icons

def numbered(name, label):
    im = canvas(22, 22); d = ImageDraw.Draw(im)
    d.ellipse(s(2.5, 2.5, 19.5, 19.5), outline=WHITE, width=int(1.3 * SS))
    if label in ('up', 'down'):
        a = 6 if label == 'up' else 16
        b = 16 if label == 'up' else 6
        d.line(s(11, 6, 11, 16), fill=WHITE, width=int(1.3 * SS))
        d.line([tuple(s(7.5, a + (3.5 if label == 'up' else -3.5))), tuple(s(11, a)), tuple(s(14.5, a + (3.5 if label == 'up' else -3.5)))],
               fill=WHITE, width=int(1.3 * SS), joint='curve')
    else:
        f = font(TAHOMA_B, 12)
        d.text(s(11, 11.3), label, font=f, fill=WHITE, anchor='mm')
    done(im, name)


for i in range(1, 26):
    numbered(f'num_{i}', str(i))
numbered('num_up', 'up')
numbered('num_down', 'down')


def winbtn(kind, state):
    im = canvas(15, 14); d = ImageDraw.Draw(im)
    bg = {1: (60, 60, 62), 2: (110, 110, 112), 3: (200, 0, 16)}[state]
    d.rectangle(s(0, 0, 15, 14), fill=bg)
    d.rectangle(s(0.5, 0.5, 14.5, 13.5), outline=(150, 150, 150), width=SS)
    if kind == 'min':
        d.rectangle(s(3.5, 6.2, 11.5, 7.8), fill=(225, 225, 225))
    else:
        d.line(s(4, 3.5, 11, 10.5), fill=(225, 225, 225), width=int(1.6 * SS))
        d.line(s(11, 3.5, 4, 10.5), fill=(225, 225, 225), width=int(1.6 * SS))
    done(im, f'{kind}-{state}')


for k in ('min', 'close'):
    for st in (1, 2, 3):
        winbtn(k, st)

im = canvas(8, 8); ImageDraw.Draw(im).polygon([tuple(s(1.5, 0.5)), tuple(s(7, 4)), tuple(s(1.5, 7.5))], fill=WHITE); done(im, 'RightArrow')
for name, up in (('scrollup', True), ('scrolldown', False)):
    im = canvas(15, 8); d = ImageDraw.Draw(im)
    pts = [(2, 7), (7.5, 1), (13, 7)] if up else [(2, 1), (7.5, 7), (13, 1)]
    d.polygon([tuple(s(*p)) for p in pts], fill=(200, 200, 200)); done(im, name)
# key down / key up markers of the macro list (7x9)
for name, down in (('423', True), ('424', False)):
    im = canvas(7, 9); d = ImageDraw.Draw(im)
    if down:
        d.line(s(3.5, 0.5, 3.5, 6), fill=WHITE, width=int(1.4 * SS))
        d.polygon([tuple(s(0.5, 4.5)), tuple(s(6.5, 4.5)), tuple(s(3.5, 8.5))], fill=WHITE)
    else:
        d.line(s(3.5, 3, 3.5, 8.5), fill=WHITE, width=int(1.4 * SS))
        d.polygon([tuple(s(0.5, 4.5)), tuple(s(6.5, 4.5)), tuple(s(3.5, 0.5))], fill=WHITE)
    done(im, name)
# record / stop
for name, rec, lit in (('iconstart1', True, False), ('iconstart2', True, True), ('iconstop1', False, False), ('iconstop2', False, True)):
    im = canvas(31, 24); d = ImageDraw.Draw(im)
    rrect(d, (0.5, 0.5, 30.5, 23.5), 3, fill=(255, 60, 60) if lit else BRIGHT, outline=(120, 0, 0), width=1)
    if rec:
        d.ellipse(s(9.5, 6, 21.5, 18), fill=WHITE)
    else:
        d.rectangle(s(10, 6.5, 21, 17.5), fill=WHITE)
    done(im, name)

# ------------------------------------------------------------------ frames

def chamfer_poly(w, h, c):
    return [(c, 0), (w - c, 0), (w, c), (w, h - c), (w - c, h), (c, h), (0, h - c), (0, c)]


def hatch(d, x, y, n, flip=False, w=9, gap=5):
    for k in range(n):
        yy = y + k * gap
        if flip:
            d.polygon([tuple(s(x, yy + 3)), tuple(s(x + w, yy)), tuple(s(x + w, yy + 2.6)), tuple(s(x, yy + 5.6))], fill=RED)
        else:
            d.polygon([tuple(s(x, yy)), tuple(s(x + w, yy + 3)), tuple(s(x + w, yy + 5.6)), tuple(s(x, yy + 2.6))], fill=RED)


def window_frame(w, h, notch):
    """Outer bevelled frame with chamfered corners, side notches and red hatching."""
    im = canvas(w, h); d = ImageDraw.Draw(im)
    poly = chamfer_poly(w - 1, h - 1, 14)
    d.polygon([tuple(s(x + 0.5, y + 0.5)) for x, y in poly], fill=FRAME_D)
    inner = chamfer_poly(w - 9, h - 9, 11)
    d.polygon([tuple(s(x + 4.5, y + 4.5)) for x, y in inner], fill=FRAME)
    core = chamfer_poly(w - 15, h - 15, 9)
    d.polygon([tuple(s(x + 7.5, y + 7.5)) for x, y in core], fill=(0, 0, 0))
    # side notches with red hatching (top and bottom thirds)
    for top in (notch, h - notch - 70):
        for side in (0, 1):
            x = 1 if side == 0 else w - 13
            d.rectangle(s(x, top, x + 12, top + 70), fill=(0, 0, 0))
            d.line(s(x + (11.5 if side == 0 else 0.5), top, x + (11.5 if side == 0 else 0.5), top + 70), fill=FRAME, width=2 * SS)
            hatch(d, x + 2, top + 14, 6, flip=(side == 1))
    # top and bottom accents
    d.rectangle(s(w * 0.27, 2, w * 0.73, 5), fill=FRAME)
    d.rectangle(s(w * 0.27, h - 6, w * 0.73, h - 3), fill=FRAME)
    return im, d


DRAGON = Image.open('assets/brand-src/dragon.png').convert('RGBA')        # from the user's logo
WORDMARK = Image.open('assets/brand-src/wordmark.png').convert('RGBA')    # from the user's logo


def fit(img, w, h):
    k = min(w / img.width, h / img.height)
    return img.resize((max(1, int(img.width * k)), max(1, int(img.height * k))), Image.LANCZOS)


def emblem(size):
    """Dark disc with a silver ring and the Dragonize dragon inside (supersampled)."""
    im = Image.new('RGBA', (size * SS, size * SS)); d = ImageDraw.Draw(im)
    r = size * SS / 2
    d.ellipse([0, 0, 2 * r - 1, 2 * r - 1], fill=(22, 22, 24))
    d.ellipse([SS * 2, SS * 2, 2 * r - SS * 2, 2 * r - SS * 2], outline=(150, 150, 155), width=int(max(1.2, size / 36) * SS))
    dr = fit(DRAGON, size * SS * 0.74, size * SS * 0.74)
    im.alpha_composite(dr, (int(r - dr.width / 2), int(r - dr.height / 2)))
    return im


def wordmark(text, px, color=WHITE, skew=0.18, spacing=1.5):
    f = font(TAHOMA_B, px)
    tw = sum(f.getlength(ch) for ch in text) + spacing * SS * (len(text) - 1)
    im = Image.new('RGBA', (int(tw + px * SS), int(px * 1.5 * SS)))
    d = ImageDraw.Draw(im)
    x = px * SS * 0.3
    for ch in text:
        d.text((x, px * 0.1 * SS), ch, font=f, fill=color)
        x += f.getlength(ch) + spacing * SS
    return im.transform(im.size, Image.AFFINE, (1, skew, -skew * im.size[1] * 0.6, 0, 1, 0), Image.BICUBIC)


def main_background():
    w, h = 795, 625
    im, d = window_frame(w, h, 96)
    # content panel
    rrect(d, (62, 142, 737, 551), 4, fill=(0, 0, 0), outline=(70, 70, 72), width=1.4)
    # emblem centre top
    e = emblem(86)
    im.alpha_composite(e, s(354, 35)[0:2] and (354 * SS, 35 * SS))
    # wordmark left: DRAGONIZE with red Z
    wm = fit(WORDMARK, 236 * SS, 44 * SS)
    im.alpha_composite(wm, (56 * SS, int((80 - wm.height / SS / 2) * SS)))
    # right block: product line text (the app writes the model name for other mice)
    f = font(TAHOMA_B, 12)
    d.line(s(575, 102, 733, 102), fill=RED, width=int(1.6 * SS))
    f2 = font(TAHOMA_B, 10)
    d.text(s(742, 599), 'dragonize  ·  unofficial', font=f2, fill=(200, 20, 30), anchor='rm')
    # keyboards (kbd.rs) get the same window with their product line
    kb = im.copy()
    ImageDraw.Draw(kb).text(s(733, 110), 'GAMING KEYBOARD', font=f, fill=(235, 235, 235), anchor='rm')
    done(kb, 'cfgMainback_kbd')
    d.text(s(733, 110), 'GAMING MOUSE', font=f, fill=(235, 235, 235), anchor='rm')
    done(im.copy(), 'cfgMainback_plain')
    rgb = wordmark('RGB', 38, color=RED, skew=0.22, spacing=3)
    im.alpha_composite(rgb, (612 * SS, 48 * SS))
    done(im, 'cfgMainback')
    ic = Image.new('RGBA', (256, 256)); idr = ImageDraw.Draw(ic)
    idr.rounded_rectangle([4, 4, 251, 251], radius=44, fill=(14, 14, 16), outline=(120, 0, 10), width=4)
    dr = fit(DRAGON, 200, 200)
    ic.alpha_composite(dr, ((256 - dr.width) // 2, (256 - dr.height) // 2))
    ic.save(os.path.join(OUT, 'icon.png'))


def dialog(name, w, h):
    im, d = window_frame(w, h, int(h * 0.12))
    done(im, name)


main_background()
dialog('dlg_small', 478, 292)
dialog('dlg_big', 757, 582)

# ------------------------------------------------------------------ mouse illustration (189x270)

def mouse_art():
    import colorsys
    W, H = 189, 270
    body = [(62, 34), (96, 29), (128, 30), (142, 46), (150, 70), (157, 110), (160, 150), (157, 185), (152, 210),
            (138, 234), (120, 252), (100, 258), (82, 256), (62, 247), (48, 232), (37, 208), (30, 180), (27, 150),
            (26, 118), (30, 92), (36, 70), (47, 48)]
    # RGB rim: continuous hue around the outline, softly glowing
    rim = canvas(W, H); rd = ImageDraw.Draw(rim)
    n = len(body)
    for i in range(n):
        (x0, y0), (x1, y1) = body[i], body[(i + 1) % n]
        for k in range(8):
            t = (i + k / 8) / n
            r, g, b = colorsys.hsv_to_rgb(t, 0.9, 1.0)
            xa, ya = x0 + (x1 - x0) * k / 8, y0 + (y1 - y0) * k / 8
            xb, yb = x0 + (x1 - x0) * (k + 1) / 8, y0 + (y1 - y0) * (k + 1) / 8
            rd.line([tuple(s(xa, ya)), tuple(s(xb, yb))], fill=(int(r * 255), int(g * 255), int(b * 255)), width=int(4 * SS))
    glow = rim.filter(ImageFilter.GaussianBlur(3 * SS))
    im = Image.alpha_composite(glow, rim)
    # body with a top-left highlight
    shade = canvas(W, H); sd = ImageDraw.Draw(shade)
    for k in range(40):
        t = k / 39
        c = int(58 - 26 * t)
        sd.ellipse(s(20 + t * 30, 20 + t * 60, 170 - t * 10, 262 - t * 4), fill=(c, c, c + 2))
    mask = Image.new('L', im.size, 0)
    ImageDraw.Draw(mask).polygon([tuple(s(x, y)) for x, y in body], fill=255)
    mask = mask.filter(ImageFilter.MinFilter(int(5 * SS) | 1))
    im.paste(shade, (0, 0), mask)
    d = ImageDraw.Draw(im)
    # button seam and palm split
    d.line(s(94, 34, 95, 128), fill=(14, 14, 16), width=int(2 * SS))
    d.line([tuple(s(36, 128)), tuple(s(95, 134)), tuple(s(156, 124))], fill=(14, 14, 16), width=int(2 * SS))
    # wheel
    rrect(d, (86, 52, 102, 96), 6, fill=(18, 18, 20), outline=RED, width=1.3)
    for k in range(6):
        d.line(s(89, 58 + k * 6, 99, 58 + k * 6), fill=(70, 70, 74), width=SS)
    # top DPI buttons behind the wheel
    rrect(d, (86, 104, 102, 116), 2, fill=(22, 22, 24), outline=(95, 95, 100), width=1)
    rrect(d, (86, 122, 102, 134), 2, fill=(22, 22, 24), outline=(95, 95, 100), width=1)
    # side buttons (left edge)
    for y0 in (70, 103, 128, 153):
        rrect(d, (27, y0, 39, y0 + 18), 3, fill=(26, 26, 28), outline=(105, 105, 110), width=1)
    # emblem on the palm (rendered large, then reduced)
    e = emblem(120).resize((30 * SS, 30 * SS), Image.LANCZOS)
    im.alpha_composite(e, (80 * SS, 198 * SS))
    d = ImageDraw.Draw(im)
    marks = {1: (68, 69), 2: (123, 69), 3: (94, 81), 4: (44, 79), 5: (37, 110), 6: (40, 135), 7: (44, 159),
             8: (76, 112), 9: (76, 132)}
    f = font(TAHOMA_B, 9)
    for k, (x, y) in marks.items():
        d.ellipse(s(x - 6.5, y - 6.5, x + 6.5, y + 6.5), fill=(16, 16, 18), outline=WHITE, width=int(1.1 * SS))
        d.text(s(x, y + 0.3), str(k), font=f, fill=WHITE, anchor='mm')
    done(im, 'mouse-layout-front')


# The General tab shows the manufacturer's product photo when it is present in
# assets/vendor/ (the owner chose to keep it, knowing it is the vendor's
# copyrighted photo); otherwise the original illustration above is used.
VENDOR_PHOTO = 'assets/vendor/mouse-layout-front.png'
if os.path.exists(VENDOR_PHOTO):
    Image.open(VENDOR_PHOTO).convert('RGBA').save(os.path.join(OUT, 'mouse-layout-front.png'))
else:
    mouse_art()
print('generated', len(os.listdir(OUT)), 'images in', OUT)
