"""
Notepad9 application icon.

A sheet of paper with a folded corner and syntax-colored code lines, a pencil
and a glossy "9" badge. Drawn at high resolution and scaled down for every
size; 16-24 px get a simplified drawing (no pencil, fewer, thicker lines) so
they stay readable.

Writes  <icons_dir>/app_<size>.png,  <assets>/app.png  and  <assets>/app.ico.

Usage:  python tools/make_app_icon.py [icons_dir] [preview_png]
"""
import os
import sys

from PIL import Image, ImageChops, ImageDraw, ImageFilter, ImageFont

APP_SIZES = [16, 20, 24, 28, 32, 40, 48, 64, 80, 96, 128, 256]
ICO_SIZES = [16, 20, 24, 32, 40, 48, 64, 256]
FONTS = os.path.join(os.environ.get('WINDIR', r'C:\Windows'), 'Fonts')

U = 4  # canvas pixels per design unit (design space is 256 x 256)


def rgb(c, a=255):
    c = c.lstrip('#')
    return (int(c[0:2], 16), int(c[2:4], 16), int(c[4:6], 16), a)


def u(v):
    return v * U


def box(x0, y0, x1, y1):
    return [u(x0), u(y0), u(x1), u(y1)]


class Canvas:
    def __init__(self, size=256):
        self.n = size * U
        self.img = Image.new('RGBA', (self.n, self.n), (0, 0, 0, 0))

    def mask(self):
        return Image.new('L', (self.n, self.n), 0)

    def paint(self, mask, fill):
        """fill: '#rrggbb', (r,g,b,a) or ('v'|'h'|'d', c0, c1) gradient"""
        if isinstance(fill, tuple) and isinstance(fill[0], str) and fill[0] in 'vhd':
            layer = gradient(self.n, *fill)
        else:
            col = rgb(fill) if isinstance(fill, str) else fill
            layer = Image.new('RGBA', (self.n, self.n), col)
        a = ImageChops.multiply(layer.getchannel('A'), mask)
        layer.putalpha(a)
        self.img = Image.alpha_composite(self.img, layer)

    def shadow(self, mask, dx, dy, blur, alpha, color=(10, 20, 40)):
        m = ImageChops.offset(mask, int(u(dx)), int(u(dy))).filter(ImageFilter.GaussianBlur(u(blur)))
        m = m.point(lambda v: int(v * alpha))
        layer = Image.new('RGBA', (self.n, self.n), color + (255,))
        layer.putalpha(m)
        self.img = Image.alpha_composite(self.img, layer)

    def layer(self, im):
        self.img = Image.alpha_composite(self.img, im)


def gradient(n, kind, c0, c1):
    a, b = rgb(c0) if isinstance(c0, str) else c0, rgb(c1) if isinstance(c1, str) else c1
    if kind == 'v':
        base = Image.linear_gradient('L').resize((n, n))
    elif kind == 'h':
        base = Image.linear_gradient('L').rotate(90).transpose(Image.FLIP_LEFT_RIGHT).resize((n, n))
    else:  # diagonal, top-left -> bottom-right
        g = Image.linear_gradient('L').resize((n * 2, n * 2)).rotate(45, resample=Image.BICUBIC)
        base = g.crop((n // 2, n // 2, n // 2 + n, n // 2 + n))
    chans = [base.point(lambda v, i=i: int(a[i] + (b[i] - a[i]) * v / 255)) for i in range(4)]
    return Image.merge('RGBA', chans)


def rrect(c, x0, y0, x1, y1, r):
    m = c.mask()
    ImageDraw.Draw(m).rounded_rectangle(box(x0, y0, x1, y1), radius=u(r), fill=255)
    return m


def poly(c, pts):
    m = c.mask()
    ImageDraw.Draw(m).polygon([(u(x), u(y)) for x, y in pts], fill=255)
    return m


def ellipse(c, x0, y0, x1, y1):
    m = c.mask()
    ImageDraw.Draw(m).ellipse(box(x0, y0, x1, y1), fill=255)
    return m


def ring(c, cx, cy, r, w):
    outer = ellipse(c, cx - r, cy - r, cx + r, cy + r)
    inner = ellipse(c, cx - r + w, cy - r + w, cx + r - w, cy + r - w)
    return ImageChops.subtract(outer, inner)


def outline(c, mask, w):
    """ring of width w (design units) just inside the mask edge"""
    k = max(3, int(u(w)) * 2 + 1)
    return ImageChops.subtract(mask, mask.filter(ImageFilter.MinFilter(k)))


# ---------------------------------------------------------------- parts
SHEET = (30, 18, 186, 232)      # x0, y0, x1, y1
FOLD = 46


def sheet_mask(c, x0, y0, x1, y1, fold, r):
    m = rrect(c, x0, y0, x1, y1, r)
    cut = poly(c, [(x1 - fold, y0 - 2), (x1 + 2, y0 - 2), (x1 + 2, y0 + fold)])
    return ImageChops.subtract(m, cut)


def draw_sheet(c, simple=False):
    x0, y0, x1, y1 = SHEET
    fold = FOLD if not simple else 58
    r = 14 if not simple else 18
    m = sheet_mask(c, x0, y0, x1, y1, fold, r)
    c.shadow(m, 0, 7, 9, 0.38)
    c.shadow(m, 0, 2, 2, 0.25)
    c.paint(m, ('v', '#ffffff', '#e6ebf3'))
    c.paint(outline(c, m, 2.2 if not simple else 9), '#8b98ab' if not simple else '#4a5a72')

    if not simple:
        # line-number gutter
        g = ImageChops.multiply(m, rrect(c, x0, y0, x0 + 30, y1, 0))
        c.paint(g, ('v', '#eef2f8', '#dde4ee'))
        gl = c.mask()
        ImageDraw.Draw(gl).rectangle(box(x0 + 30, y0 + 4, x0 + 31.2, y1 - 4), fill=255)
        c.paint(ImageChops.multiply(gl, m), '#c9d2df')
        c.paint(outline(c, m, 2.2), '#8b98ab')

    # folded corner: the turned-down flap with its shadow on the sheet
    flap = poly(c, [(x1 - fold, y0), (x1 - fold + 3, y0 + fold - 6), (x1 - fold + 6, y0 + fold - 3), (x1, y0 + fold)])
    flap_r = flap.filter(ImageFilter.GaussianBlur(u(0.6)))
    c.shadow(flap, -3, 4, 5, 0.35)
    c.paint(flap_r, ('d', '#f7f9fc', '#c2ccda'))
    c.paint(outline(c, flap, 2.0 if not simple else 5), '#8b98ab' if not simple else '#55657c')


CODE = [  # (indent, [(length, color), ...]) per line
    (0, [(22, '#a855f7'), (40, '#2f7ff0')]),
    (1, [(18, '#a855f7'), (22, '#475569'), (30, '#16a34a')]),
    (1, [(34, '#2f7ff0'), (20, '#f08a24')]),
    (2, [(26, '#e2475b'), (34, '#16a34a')]),
    (1, [(18, '#64748b')]),
    (0, []),
    (0, [(52, '#9aa6b8')]),
    (0, [(20, '#a855f7'), (26, '#2f7ff0'), (16, '#f08a24')]),
]


def draw_code(c):
    x0, y0, x1, y1 = SHEET
    top = y0 + 52
    step = 19
    for i, (ind, parts) in enumerate(CODE):
        y = top + i * step
        # line number
        num = c.mask()
        nw = 8 if i < 9 else 12
        ImageDraw.Draw(num).rounded_rectangle(box(x0 + 22 - nw, y - 2.5, x0 + 22, y + 2.5), radius=u(2.5), fill=255)
        c.paint(num, '#aab4c3')
        x = x0 + 42 + ind * 14
        for ln, col in parts:
            if x + ln > x1 - 10:
                ln = x1 - 10 - x
            if ln <= 4:
                break
            m = c.mask()
            ImageDraw.Draw(m).rounded_rectangle(box(x, y - 4.5, x + ln, y + 4.5), radius=u(4.5), fill=255)
            c.paint(m, col)
            x += ln + 7
        if i == 2:
            # text cursor after the line
            cur = c.mask()
            ImageDraw.Draw(cur).rounded_rectangle(box(x - 1, y - 9, x + 2.5, y + 9), radius=u(1.5), fill=255)
            c.paint(cur, '#1d4ed8')
    # faint current-line highlight
    hl = c.mask()
    yy = top + 2 * step
    ImageDraw.Draw(hl).rectangle(box(x0 + 31.2, yy - 9.5, x1 - 2.2, yy + 9.5), fill=40)
    c.paint(hl, '#3b82f6')


def draw_code_simple(c):
    x0, y0, x1, y1 = SHEET
    rows = [(0, 66, '#a855f7'), (22, 70, '#2f7ff0'), (22, 50, '#16a34a'), (0, 84, '#f08a24')]
    for i, (ind, ln, col) in enumerate(rows):
        y = y0 + 78 + i * 34
        m = c.mask()
        ImageDraw.Draw(m).rounded_rectangle(box(x0 + 22 + ind, y - 9, x0 + 22 + ind + ln, y + 9), radius=u(9), fill=255)
        c.paint(m, col)


def pencil_layer(n):
    """horizontal pencil, tip at the left; returns RGBA image (design units * U)"""
    L, W = 150, 30
    pc = Canvas(1)
    pc.n = u(L + 8)
    pc.img = Image.new('RGBA', (u(L + 8), u(W + 8)), (0, 0, 0, 0))
    h = W + 8

    def m():
        return Image.new('L', pc.img.size, 0)

    def grad(kind, c0, c1):
        a, b = rgb(c0), rgb(c1)
        w, hh = pc.img.size
        base = Image.linear_gradient('L').resize((w, hh))
        if kind == 'h':
            base = Image.linear_gradient('L').rotate(90).transpose(Image.FLIP_LEFT_RIGHT).resize((w, hh))
        return Image.merge('RGBA', [base.point(lambda v, i=i: int(a[i] + (b[i] - a[i]) * v / 255)) for i in range(4)])

    def paint(mask, fill):
        layer = grad(*fill) if isinstance(fill, tuple) else Image.new('RGBA', pc.img.size, rgb(fill))
        layer.putalpha(ImageChops.multiply(layer.getchannel('A'), mask))
        pc.img = Image.alpha_composite(pc.img, layer)

    y0, y1 = 4, 4 + W
    cy = (y0 + y1) / 2
    tip, cone, body_end, fer_end, end = 4, 38, 112, 128, 150

    def P(pts):
        mm = m()
        ImageDraw.Draw(mm).polygon([(u(x), u(y)) for x, y in pts], fill=255)
        return mm

    def R(x0, ya, x1, yb, r=0):
        mm = m()
        ImageDraw.Draw(mm).rounded_rectangle([u(x0), u(ya), u(x1), u(yb)], radius=u(r), fill=255)
        return mm

    # eraser
    paint(R(fer_end - 4, y0, end, y1, 7), ('v', '#ffb3c4', '#d9506f'))
    paint(R(fer_end - 4, y0 + 3, end - 4, y0 + 8, 2), '#ffd6df')
    # ferrule
    paint(R(body_end, y0 - 1, fer_end, y1 + 1, 2), ('v', '#f3f5f8', '#8a95a5'))
    for x in (body_end + 4, body_end + 8, body_end + 12):
        paint(R(x, y0, x + 1.4, y1, 0), '#6b7686')
    # body: three facets
    third = W / 3
    paint(R(cone - 1, y0, body_end, y0 + third), ('v', '#ffe08a', '#ffc53a'))
    paint(R(cone - 1, y0 + third, body_end, y0 + 2 * third), ('v', '#ffbe2e', '#f5a300'))
    paint(R(cone - 1, y0 + 2 * third, body_end, y1), ('v', '#e89200', '#b86a00'))
    paint(R(cone - 1, y0 + third - 0.6, body_end, y0 + third + 0.6), '#d68a00')
    paint(R(cone - 1, y0 + 2 * third - 0.6, body_end, y0 + 2 * third + 0.6), '#a86200')
    # sharpened wood cone (wavy cut where it meets the paint)
    wood = P([(tip, cy), (cone, y0), (cone + 3, y0 + third / 2), (cone, y0 + third),
              (cone + 3, cy), (cone, y0 + 2 * third), (cone + 3, y0 + 2.5 * third), (cone, y1)])
    paint(wood, ('v', '#f6dcb0', '#c9935a'))
    # graphite
    lead = P([(tip, cy), (tip + 13, cy - 5), (tip + 13, cy + 5)])
    paint(lead, ('v', '#5b6573', '#1d232c'))
    # dark outline around the whole pencil
    full = Image.new('L', pc.img.size, 0)
    full.paste(pc.img.getchannel('A'))
    ring_ = ImageChops.subtract(full.point(lambda v: 255 if v > 128 else 0),
                                full.point(lambda v: 255 if v > 128 else 0).filter(ImageFilter.MinFilter(int(u(1.6)) * 2 + 1)))
    paint(ring_, '#5a3a10')
    return pc.img


def draw_pencil(c):
    import math
    p = pencil_layer(c.n)
    w0, h0 = p.size
    dx = u(4) - w0 / 2  # graphite tip, relative to the image center
    p = p.rotate(45, resample=Image.BICUBIC, expand=True)
    w, h = p.size
    k = math.cos(math.radians(45))
    tip_x, tip_y = w / 2 + dx * k, h / 2 - dx * k
    tx, ty = 136, 158  # where the tip touches the sheet (design units)
    ox = int(u(tx) - tip_x)
    oy = int(u(ty) - tip_y)
    layer = Image.new('RGBA', (c.n, c.n), (0, 0, 0, 0))
    layer.paste(p, (ox, oy), p)
    sh = layer.getchannel('A')
    c.shadow(sh, 4, 6, 5, 0.40)
    c.layer(layer)


def draw_badge(c, cx, cy, r, simple=False):
    disc = ellipse(c, cx - r, cy - r, cx + r, cy + r)
    c.shadow(disc, 0, 5 if not simple else 3, 7 if not simple else 3, 0.45)
    c.paint(disc, ('d', '#5b8cff', '#5b2fd6'))
    # inner bevel ring
    if not simple:
        c.paint(ring(c, cx, cy, r - 4, 2.5), (255, 255, 255, 70))
    # gloss on the top half
    gloss = ellipse(c, cx - r * 0.78, cy - r * 0.92, cx + r * 0.78, cy + r * 0.12)
    gloss = ImageChops.multiply(gloss, disc)
    c.paint(gloss, ('v', (255, 255, 255, 95), (255, 255, 255, 0)))
    c.paint(outline(c, disc, 2.4 if not simple else 7), '#2a1a7a')

    # the "9"
    size = r * (1.55 if not simple else 1.75)
    font = ImageFont.truetype(os.path.join(FONTS, 'segoeuib.ttf'), int(u(size)))
    t = c.mask()
    d = ImageDraw.Draw(t)
    l, tp, rr, bt = d.textbbox((0, 0), '9', font=font)
    d.text((u(cx) - (l + rr) / 2, u(cy) - (tp + bt) / 2 + u(1)), '9', font=font, fill=255)
    c.shadow(t, 0, 2.5 if not simple else 0, 2 if not simple else 1, 0.55, (20, 10, 70))
    c.paint(t, ('v', '#ffffff', '#dfe6ff'))


# ---------------------------------------------------------------- compose
def full():
    c = Canvas()
    draw_sheet(c)
    draw_code(c)
    draw_pencil(c)
    draw_badge(c, 190, 192, 50)
    return c.img


def simple():
    c = Canvas()
    draw_sheet(c, simple=True)
    draw_code_simple(c)
    draw_badge(c, 182, 184, 66, simple=True)
    return c.img


def scaled(big, size):
    im = big.resize((size, size), Image.LANCZOS)
    if size <= 48:
        im = im.filter(ImageFilter.UnsharpMask(radius=0.6, percent=60, threshold=0))
    return im


def build(icons_dir):
    big, small = full(), simple()
    out = {}
    for s in APP_SIZES:
        out[s] = scaled(small if s <= 24 else big, s)
        out[s].save(os.path.join(icons_dir, f'app_{s}.png'))
    assets = os.path.normpath(os.path.join(icons_dir, '..'))
    out[256].save(os.path.join(assets, 'app.png'))
    out[256].save(os.path.join(assets, 'app.ico'), format='ICO', sizes=[(s, s) for s in ICO_SIZES],
                  append_images=[out[s] for s in ICO_SIZES if s != 256])
    return out


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    icons_dir = sys.argv[1] if len(sys.argv) > 1 else os.path.join(here, '..', 'Notepad9Rust', 'assets', 'icons')
    out = build(icons_dir)
    if len(sys.argv) > 2:
        sizes = [16, 20, 24, 32, 48, 64, 128, 256]
        w = sum(sizes) + 20 * (len(sizes) + 1)
        sheet = Image.new('RGBA', (w, 2 * 276), (243, 243, 243, 255))
        dark = Image.new('RGBA', (w, 276), (32, 32, 32, 255))
        sheet.paste(dark, (0, 276))
        x = 20
        for s in sizes:
            for row in (0, 1):
                sheet.alpha_composite(out[s], (x, row * 276 + 10 + (256 - s)))
            x += s + 20
        sheet.save(sys.argv[2])
    print('app icon')


if __name__ == '__main__':
    main()
