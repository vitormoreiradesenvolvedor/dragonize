//! CPU framebuffer that reproduces the GDI drawing of the original ldcfg.exe:
//! integer-aligned fills, GDI-style rounded rectangles, bitmap blits (plain and
//! stretched) and text rasterised on whole pixels. Everything is deterministic,
//! so a screenshot of this buffer can be diffed against captures of the original.

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rgba(pub u8, pub u8, pub u8, pub u8);

pub const fn rgb(hex: u32) -> Rgba {
    Rgba((hex >> 16) as u8, (hex >> 8) as u8, hex as u8, 255)
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }
    /// Rect from inclusive corner coordinates, as measured on screenshots.
    pub const fn ltrb(l: i32, t: i32, r: i32, b: i32) -> Self {
        Self { x: l, y: t, w: r - l + 1, h: b - t + 1 }
    }
    pub fn contains(&self, p: (i32, i32)) -> bool {
        p.0 >= self.x && p.0 < self.x + self.w && p.1 >= self.y && p.1 < self.y + self.h
    }
    pub fn offset(&self, dx: i32, dy: i32) -> Rect {
        Rect::new(self.x + dx, self.y + dy, self.w, self.h)
    }
    pub fn shrink(&self, d: i32) -> Rect {
        Rect::new(self.x + d, self.y + d, self.w - 2 * d, self.h - 2 * d)
    }
}

pub struct Image {
    pub w: i32,
    pub h: i32,
    pub px: Vec<Rgba>,
}

impl Image {
    pub fn decode(bytes: &[u8]) -> Image {
        let img = image::load_from_memory(bytes).expect("embedded image").to_rgba8();
        let (w, h) = img.dimensions();
        let px = img.pixels().map(|p| Rgba(p[0], p[1], p[2], p[3])).collect();
        Image { w: w as i32, h: h as i32, px }
    }
    pub fn get(&self, x: i32, y: i32) -> Rgba {
        self.px[(y * self.w + x) as usize]
    }

    /// Nearest-neighbour resample, matching GDI StretchBlt(COLORONCOLOR).
    pub fn resize_nearest(&self, nw: i32, nh: i32) -> Image {
        let mut px = Vec::with_capacity((nw * nh) as usize);
        for y in 0..nh {
            // sample pixel centres, like GDI's COLORONCOLOR stretch
            let sy = (((2 * y as i64 + 1) * self.h as i64) / (2 * nh as i64)) as i32;
            for x in 0..nw {
                let sx = (((2 * x as i64 + 1) * self.w as i64) / (2 * nw as i64)) as i32;
                px.push(self.get(sx, sy));
            }
        }
        Image { w: nw, h: nh, px }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Align {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Weight {
    Regular,
    Bold,
}

#[derive(Clone, Copy, Debug)]
pub struct TextStyle {
    pub size: f32,
    pub weight: Weight,
    pub color: Rgba,
    pub italic: bool,
    /// Em size of linked CJK glyphs (0 = size * Fonts::cjk_scale). GDI font
    /// linking gave every control its own Latin/CJK size pair.
    pub cjk: f32,
    /// Vertical nudge applied by text_in, measured per control.
    pub dy: i32,
}

impl TextStyle {
    pub const fn new(size: f32, weight: Weight, color: Rgba) -> Self {
        Self { size, weight, color, italic: false, cjk: 0.0, dy: 0 }
    }
    pub const fn italic(mut self) -> Self {
        self.italic = true;
        self
    }
    pub const fn cjk(mut self, px: f32) -> Self {
        self.cjk = px;
        self
    }
    pub const fn dy(mut self, dy: i32) -> Self {
        self.dy = dy;
        self
    }
}

/// Latin glyphs come from Tahoma, everything else from the CJK face, which is
/// how Windows font linking resolved the original UI (Tahoma -> JhengHei).
pub struct Fonts {
    latin: FontRef<'static>,
    latin_bold: FontRef<'static>,
    cjk: FontRef<'static>,
    cjk_bold: Option<FontRef<'static>>,
    /// CJK glyphs are drawn this much larger than the Latin em (font-link scaling).
    pub cjk_scale: f32,
    /// Exponent applied to glyph coverage to approximate hinted GDI strokes.
    pub gamma: f32,
}

fn tune(name: &str, default: f32) -> f32 {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

impl Fonts {
    pub fn new(
        latin: &'static [u8],
        latin_bold: &'static [u8],
        cjk: (&'static [u8], u32),
        cjk_bold: Option<(&'static [u8], u32)>,
    ) -> Self {
        Fonts {
            latin: FontRef::try_from_slice(latin).expect("latin font"),
            latin_bold: FontRef::try_from_slice(latin_bold).expect("latin bold font"),
            cjk: FontRef::try_from_slice_and_index(cjk.0, cjk.1).expect("cjk font"),
            cjk_bold: cjk_bold.and_then(|(b, i)| FontRef::try_from_slice_and_index(b, i).ok()),
            cjk_scale: tune("M801_CJK_SCALE", 1.13),
            gamma: tune("M801_GAMMA", 0.75),
        }
    }

    fn is_latin(&self, c: char, w: Weight) -> bool {
        // Wine's tahomabd.ttf only emboldens Cyrillic letters shared with Latin;
        // the rest are regular-weight outlines, so bold Cyrillic uses the bold
        // fallback face instead (drawn at the Latin size, see glyph_scale).
        if w == Weight::Bold && is_cyrillic(c) && self.cjk_bold.as_ref().is_some_and(|f| f.glyph_id(c).0 != 0) {
            return false;
        }
        let latin = if w == Weight::Bold { &self.latin_bold } else { &self.latin };
        c.is_ascii() || latin.glyph_id(c).0 != 0
    }

    /// Scale for `c` at style size: Latin em = size, linked CJK em = size * cjk_scale.
    fn glyph_scale(&self, c: char, st: &TextStyle) -> (&FontRef<'static>, bool, PxScale) {
        let latin = self.is_latin(c, st.weight);
        let (f, emb) = if latin { self.face(c, st.weight) } else { self.cjk_face(st.weight) };
        let size = if latin || is_cyrillic(c) { st.size } else if st.cjk > 0.0 { st.cjk } else { st.size * self.cjk_scale };
        (f, emb, Self::scale(f, size))
    }

    /// Baseline offset from the text top: the Latin (base font) ascent, shared by linked glyphs.
    fn ascent(&self, st: &TextStyle) -> f32 {
        let f = if st.weight == Weight::Bold { &self.latin_bold } else { &self.latin };
        f.as_scaled(Self::scale(f, st.size)).ascent().round()
    }

    fn cjk_face(&self, w: Weight) -> (&FontRef<'static>, bool) {
        match (w, &self.cjk_bold) {
            (Weight::Bold, Some(b)) => (b, false),
            (Weight::Bold, None) => (&self.cjk, true),
            _ => (&self.cjk, false),
        }
    }

    fn face(&self, c: char, w: Weight) -> (&FontRef<'static>, bool) {
        let latin = if w == Weight::Bold { &self.latin_bold } else { &self.latin };
        if c.is_ascii() || latin.glyph_id(c).0 != 0 {
            return (latin, false);
        }
        match (w, &self.cjk_bold) {
            (Weight::Bold, Some(b)) => (b, false),
            // No bold CJK face: embolden like GDI does for linked fonts.
            (Weight::Bold, None) => (&self.cjk, true),
            _ => (&self.cjk, false),
        }
    }

    /// Em-size scaling, i.e. a GDI font created with lfHeight = -size.
    fn scale(f: &FontRef<'static>, size: f32) -> PxScale {
        let upem = f.units_per_em().unwrap_or(1000.0);
        PxScale::from(size * f.height_unscaled() / upem)
    }

    pub fn measure(&self, s: &str, st: &TextStyle) -> i32 {
        let mut w = 0f32;
        for c in s.chars() {
            let (f, emb, scale) = self.glyph_scale(c, st);
            let sf = f.as_scaled(scale);
            w += snap_adv(c, sf.h_advance(f.glyph_id(c))) + if emb { 1.0 } else { 0.0 };
        }
        w.round() as i32
    }
}

fn is_cyrillic(c: char) -> bool {
    ('\u{0400}'..='\u{04FF}').contains(&c)
}

/// CJK cells advance by whole pixels (as GDI did); Latin and Cyrillic keep
/// fractional advances so small text does not get uneven letter gaps.
fn snap_adv(c: char, adv: f32) -> f32 {
    if (c as u32) >= 0x2E80 { adv.round() } else { adv }
}

pub struct Canvas {
    pub w: i32,
    pub h: i32,
    pub px: Vec<Rgba>,
    clip: Rect,
}

impl Canvas {
    pub fn new(w: i32, h: i32) -> Self {
        Canvas { w, h, px: vec![Rgba(0, 0, 0, 0); (w * h) as usize], clip: Rect::new(0, 0, w, h) }
    }

    pub fn clear(&mut self) {
        self.px.fill(Rgba(0, 0, 0, 0));
        self.clip = Rect::new(0, 0, self.w, self.h);
    }

    pub fn set_clip(&mut self, r: Option<Rect>) {
        self.clip = r.unwrap_or(Rect::new(0, 0, self.w, self.h));
    }

    #[inline]
    fn inside(&self, x: i32, y: i32) -> bool {
        self.clip.contains((x, y)) && x >= 0 && y >= 0 && x < self.w && y < self.h
    }

    #[inline]
    pub fn blend(&mut self, x: i32, y: i32, c: Rgba, cov: f32) {
        if !self.inside(x, y) {
            return;
        }
        let a = (c.3 as f32 / 255.0) * cov;
        if a <= 0.0 {
            return;
        }
        let d = &mut self.px[(y * self.w + x) as usize];
        if a >= 1.0 {
            *d = Rgba(c.0, c.1, c.2, 255);
            return;
        }
        let da = d.3 as f32 / 255.0;
        let oa = a + da * (1.0 - a);
        let mix = |s: u8, t: u8| ((s as f32 * a + t as f32 * da * (1.0 - a)) / oa).round() as u8;
        *d = Rgba(mix(c.0, d.0), mix(c.1, d.1), mix(c.2, d.2), (oa * 255.0).round() as u8);
    }

    pub fn fill(&mut self, r: Rect, c: Rgba) {
        for y in r.y..r.y + r.h {
            for x in r.x..r.x + r.w {
                self.blend(x, y, c, 1.0);
            }
        }
    }

    /// Rectangle outline `t` pixels thick, drawn inside `r`.
    pub fn frame(&mut self, r: Rect, t: i32, c: Rgba) {
        self.fill(Rect::new(r.x, r.y, r.w, t), c);
        self.fill(Rect::new(r.x, r.y + r.h - t, r.w, t), c);
        self.fill(Rect::new(r.x, r.y + t, t, r.h - 2 * t), c);
        self.fill(Rect::new(r.x + r.w - t, r.y + t, t, r.h - 2 * t), c);
    }

    pub fn blit(&mut self, img: &Image, x: i32, y: i32) {
        self.blit_part(img, Rect::new(0, 0, img.w, img.h), x, y);
    }

    pub fn blit_part(&mut self, img: &Image, src: Rect, x: i32, y: i32) {
        for j in 0..src.h {
            for i in 0..src.w {
                let p = img.get(src.x + i, src.y + j);
                if p.3 > 0 {
                    self.blend(x + i, y + j, Rgba(p.0, p.1, p.2, 255), p.3 as f32 / 255.0);
                }
            }
        }
    }

    /// Draws `s` with its top-left at (x, y) (GDI DrawText top alignment).
    pub fn text(&mut self, fonts: &Fonts, x: i32, y: i32, s: &str, st: &TextStyle) -> i32 {
        let mut pen = x as f32;
        // GDI puts every glyph, linked ones included, on the base font's baseline.
        let base = y as f32 + fonts.ascent(st);
        for c in s.chars() {
            let (f, emb, scale) = fonts.glyph_scale(c, st);
            let sf = f.as_scaled(scale);
            let gid = f.glyph_id(c);
            let g = gid.with_scale_and_position(scale, ab_glyph::point(pen.round(), base));
            if let Some(og) = f.outline_glyph(g) {
                let bb = og.px_bounds();
                let col = st.color;
                let mut cov_map: Vec<(i32, i32, f32)> = Vec::new();
                og.draw(|gx, gy, cov| {
                    cov_map.push((bb.min.x as i32 + gx as i32, bb.min.y as i32 + gy as i32, cov));
                });
                for (px, py, cov) in &cov_map {
                    // synthetic oblique, like GDI's lfItalic on a face without italics
                    let px = &if st.italic { px + ((base - *py as f32) * 0.21).round() as i32 } else { *px };
                    // GDI grayscale AA renders noticeably heavier than linear coverage.
                    let c = cov.clamp(0.0, 1.0).powf(fonts.gamma);
                    self.blend(*px, *py, col, c);
                    if emb {
                        self.blend(*px + 1, *py, col, c);
                    }
                }
            }
            pen += snap_adv(c, sf.h_advance(gid)) + if emb { 1.0 } else { 0.0 };
        }
        pen as i32 - x
    }

    /// Text centred in `r` horizontally and vertically (DT_CENTER|DT_VCENTER).
    pub fn text_in(&mut self, fonts: &Fonts, r: Rect, s: &str, st: &TextStyle, align: Align, dy: i32) {
        let w = fonts.measure(s, st);
        let lh = st.size.round() as i32 + 2;
        let x = match align {
            Align::Left => r.x,
            Align::Center => r.x + (r.w - w) / 2,
            Align::Right => r.x + r.w - w,
        };
        let y = r.y + (r.h - lh) / 2 + dy + st.dy;
        self.text(fonts, x, y, s, st);
    }

    pub fn to_rgba_bytes(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(self.px.len() * 4);
        for p in &self.px {
            v.extend_from_slice(&[p.0, p.1, p.2, p.3]);
        }
        v
    }
}
