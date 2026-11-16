"""
Notepad9 icon generator.

Classic 16x16-grid toolbar icons (gradients + 1px dark outline, like the
reference). Every icon is rendered separately for each pixel size
(16, 20, 24, 28, 32, 40, 48, 64), with edges and outlines snapped to that
size's pixel grid, so nothing is resampled at runtime and icons stay crisp at
100% / 125% / 150% / 175% / 200% ... display scaling.

For each icon and size it writes  <name>_<size>.png  and  <name>_<size>_d.png
(gray "disabled" look). Also builds app_<size>.png and the multi-size app.ico.

Usage:  python tools/make_icons.py [out_dir] [preview_png]
"""
import glob
import math
import os
import sys

import numpy as np
from PIL import Image, ImageChops, ImageDraw, ImageFilter, ImageFont

G = 16                 # design grid (all coordinates below are in 0..16 units)
TB_SIZES = [16, 20, 24, 28, 32, 40, 48, 64]
APP_SIZES = [16, 20, 24, 28, 32, 40, 48, 64, 80, 96, 128, 256]

# set by setup(size)
SIZE = 16              # output pixels
PX = 1.0               # output pixels per grid unit
SS = 8                 # supersampling factor
K = 8.0                # canvas pixels per grid unit
N = 128                # canvas size


def setup(size):
    global SIZE, PX, SS, K, N
    SIZE = size
    PX = size / G
    SS = 8 if size <= 48 else (4 if size <= 96 else 2)
    K = PX * SS
    N = size * SS


# ---------------------------------------------------------------- helpers
def rgba(c, a=255):
    if isinstance(c, tuple):
        return c if len(c) == 4 else c + (a,)
    c = c.lstrip('#')
    return (int(c[0:2], 16), int(c[2:4], 16), int(c[4:6], 16), a)


def P(v):
    """grid units -> canvas pixels (no snapping)"""
    return v * K


def S(v):
    """grid units -> canvas pixels, snapped to the output pixel grid"""
    return round(v * PX) * SS


def wpx(w):
    """stroke width in whole output pixels (at least 1)"""
    return max(1, int(round(w * PX)))


def draw_shape(d, s, val):
    kind = s[0]
    if kind == 'poly':
        pts = s[1]
        f = S if len(pts) <= 8 else P   # snap simple shapes, keep curves/stars smooth
        d.polygon([(f(x), f(y)) for x, y in pts], fill=val)
    elif kind == 'rect':
        x0, y0, x1, y1 = s[1:5]
        r = s[5] if len(s) > 5 else 0
        box = [S(x0), S(y0), S(x1) - 1, S(y1) - 1]
        if box[2] <= box[0] or box[3] <= box[1]:
            return
        if r:
            d.rounded_rectangle(box, radius=max(1, P(r)), fill=val)
        else:
            d.rectangle(box, fill=val)
    elif kind == 'ellipse':
        x0, y0, x1, y1 = s[1:5]
        d.ellipse([S(x0), S(y0), S(x1) - 1, S(y1) - 1], fill=val)
    elif kind == 'union':
        for sub in s[1]:
            draw_shape(d, sub, val)
    else:
        raise ValueError(kind)


def mask_of(s):
    m = Image.new('L', (N, N), 0)
    if s[0] == 'diff':
        draw_shape(ImageDraw.Draw(m), s[1], 255)
        for sub in s[2:]:
            draw_shape(ImageDraw.Draw(m), sub, 0)
    else:
        draw_shape(ImageDraw.Draw(m), s, 255)
    return m


def erode(m, w):
    """inset a mask by a whole number of output pixels"""
    px = wpx(w) * SS
    out = m
    while px > 0:
        step = min(px, 4)
        out = out.filter(ImageFilter.MinFilter(step * 2 + 1))
        px -= step
    return out


_grid_cache = {}


def _grid():
    if N not in _grid_cache:
        yy, xx = np.mgrid[0:N, 0:N].astype(np.float32)
        _grid_cache[N] = (xx, yy)
    return _grid_cache[N]


def gradient(fill, bbox):
    """RGBA image (N x N) painted with fill over bbox."""
    if not isinstance(fill, tuple) or (len(fill) and isinstance(fill[0], int)):
        return Image.new('RGBA', (N, N), rgba(fill))
    kind = fill[0]
    x0, y0, x1, y1 = bbox
    w = max(1, x1 - x0)
    h = max(1, y1 - y0)
    xx, yy = _grid()
    c0 = np.array(rgba(fill[1]), np.float32)
    c1 = np.array(rgba(fill[2]), np.float32)
    if kind == 'v':
        t = (yy - y0) / h
    elif kind == 'h':
        t = (xx - x0) / w
    elif kind == 'd':
        t = ((xx - x0) / w + (yy - y0) / h) / 2
    elif kind == 'r':
        fx = fill[3] if len(fill) > 3 else 0.4
        fy = fill[4] if len(fill) > 4 else 0.35
        cx, cy = x0 + w * fx, y0 + h * fy
        t = np.hypot(xx - cx, yy - cy) / (max(w, h) * 0.85)
    else:
        raise ValueError(kind)
    t = np.clip(t, 0, 1)[..., None]
    arr = (c0 * (1 - t) + c1 * t + 0.5).astype(np.uint8)
    return Image.fromarray(arr, 'RGBA')


class Icon:
    def __init__(self):
        self.img = Image.new('RGBA', (N, N), (0, 0, 0, 0))

    def _paint(self, layer, m):
        self.img.alpha_composite(Image.composite(layer, Image.new('RGBA', (N, N), (0, 0, 0, 0)), m))

    def fill(self, shape, fill, outline=None, w=0.8):
        m = mask_of(shape)
        bb = m.getbbox()
        if not bb:
            return
        if outline:
            self._paint(gradient(outline, bb), m)
            self._paint(gradient(fill, bb), erode(m, w))
        else:
            self._paint(gradient(fill, bb), m)

    def stroke(self, shape, color, w=0.8):
        m = mask_of(shape)
        ring = ImageChops.subtract(m, erode(m, w))
        self._paint(gradient(color, m.getbbox()), ring)

    def line(self, pts, color, w=1.0, caps=True):
        wp = wpx(w)
        if wp % 2:   # odd width: run through pixel centers
            f = lambda v: (math.floor(v * PX) + 0.5) * SS
        else:
            f = lambda v: round(v * PX) * SS
        pp = [(f(x), f(y)) for x, y in pts]
        m = Image.new('L', (N, N), 0)
        ld = ImageDraw.Draw(m)
        ld.line(pp, fill=255, width=wp * SS, joint='curve')
        if caps:
            r = wp * SS / 2
            for x, y in (pp[0], pp[-1]):
                ld.ellipse([x - r, y - r, x + r, y + r], fill=255)
        self._paint(gradient(color, m.getbbox()), m)

    def text(self, s, box, fill, outline=None, font='arialbd.ttf', size=10, w=0.6):
        x0, y0, x1, y1 = box
        f = ImageFont.truetype(font, max(4, int(P(size))))
        m = Image.new('L', (N, N), 0)
        d = ImageDraw.Draw(m)
        bb = d.textbbox((0, 0), s, font=f)
        tw, th = bb[2] - bb[0], bb[3] - bb[1]
        cx = P((x0 + x1) / 2) - tw / 2 - bb[0]
        cy = P((y0 + y1) / 2) - th / 2 - bb[1]
        d.text((cx, cy), s, font=f, fill=255)
        if outline:
            om = m.filter(ImageFilter.MaxFilter(wpx(w) * SS * 2 + 1))
            self._paint(gradient(outline, om.getbbox()), om)
        self._paint(gradient(fill, m.getbbox()), m)

    def result(self):
        return self.img.reduce(SS)


def page(x0, y0, x1, y1, fold=3.0):
    return ('poly', [(x0, y0), (x1 - fold, y0), (x1, y0 + fold), (x1, y1), (x0, y1)])


def fold_tri(x1, y0, fold=3.0):
    return ('poly', [(x1 - fold, y0), (x1 - fold, y0 + fold), (x1, y0 + fold)])


def star(cx, cy, R, r, n=5):
    pts = []
    for i in range(n * 2):
        a = -math.pi / 2 + i * math.pi / n
        rr = R if i % 2 == 0 else r
        pts.append((cx + rr * math.cos(a), cy + rr * math.sin(a)))
    return ('poly', pts)


def rot_rect(cx, cy, w, h, ang_deg):
    a = math.radians(ang_deg)
    ca, sa = math.cos(a), math.sin(a)
    pts = []
    for dx, dy in ((-w / 2, -h / 2), (w / 2, -h / 2), (w / 2, h / 2), (-w / 2, h / 2)):
        pts.append((cx + dx * ca - dy * sa, cy + dx * sa + dy * ca))
    return ('poly', pts)


# colour sets ---------------------------------------------------------------
BLUE = ('v', '#8ab8f0', '#2c5ea8')
BLUE_OL = '#173a6e'
GREEN = ('v', '#86e45e', '#279a18')
GREEN_OL = '#11520a'
MANILA_OL = '#7a5518'
PAPER = ('v', '#d4e3e3', '#fbfefe')
PAPER_OL = '#2c3c3c'


# ---------------------------------------------------------------- icons
def i_new():
    ic = Icon()
    ic.fill(page(2.5, 0.5, 13.5, 15.5, 3.5), PAPER, PAPER_OL, 0.8)
    ic.fill(fold_tri(13.5, 0.5, 3.5), ('v', '#ffffff', '#c4d6d6'), PAPER_OL, 0.7)
    return ic


def i_window_new():
    ic = Icon()
    for (x0, y0, x1, y1) in ((0.5, 1.5, 11.5, 10.5), (4.5, 5.5, 15.5, 14.5)):
        ic.fill(('rect', x0, y0, x1, y1), ('v', '#ffffff', '#d4e4f0'), '#1d4c80', 0.8)
        ic.fill(('rect', x0, y0, x1, y0 + 2.6), ('v', '#6cb0e8', '#2a74b6'))
        ic.stroke(('rect', x0, y0, x1, y1), '#1d4c80', 0.8)
    return ic


def i_open():
    ic = Icon()
    ic.fill(('poly', [(0.5, 2.5), (5.5, 2.5), (6.8, 3.8), (13.5, 3.8), (13.5, 13.5), (0.5, 13.5)]),
            ('v', '#efd187', '#c4943a'), MANILA_OL, 0.8)
    ic.fill(('rect', 3.5, 1.5, 12.5, 8.0), ('v', '#ffffff', '#e6eef4'), '#7d8c9a', 0.6)
    ic.fill(('rect', 8.8, 2.2, 11.8, 3.2), '#61a2de')
    ic.fill(('poly', [(2.6, 6.5), (15.6, 6.5), (13.5, 13.5), (0.5, 13.5)]),
            ('v', '#fff1bc', '#e6bd5c'), MANILA_OL, 0.8)
    return ic


def i_save():
    ic = Icon()
    ic.fill(('poly', [(1, 1), (13.5, 1), (15, 2.5), (15, 15), (1, 15)]), ('v', '#7a9ed6', '#33558c'), '#1a3260', 0.8)
    ic.fill(('rect', 3.5, 1, 12.5, 7.6), ('v', '#ffffff', '#d9e2ee'), '#1a3260', 0.6)
    ic.line([(5, 3.4), (11, 3.4)], '#94a8cc', 0.6, caps=False)
    ic.line([(5, 5.3), (11, 5.3)], '#94a8cc', 0.6, caps=False)
    ic.fill(('rect', 4, 9.6, 12, 15), ('v', '#e8e8e8', '#a4a4a4'), '#262626', 0.6)
    ic.fill(('rect', 5.4, 10.8, 7.4, 14), '#3a5788')
    return ic


def i_save_as():
    ic = Icon()
    ic.fill(page(5, 0.5, 14.5, 15.5, 3), ('v', '#f8e6a4', '#d9ae52'), MANILA_OL, 0.8)
    ic.fill(fold_tri(14.5, 0.5, 3), ('v', '#fff6d4', '#e4c070'), MANILA_OL, 0.6)
    ic.fill(('rect', 12.4, 9, 14.5, 14.3), ('v', '#9cc8f0', '#4a8ad0'))
    ic.fill(('poly', [(0.5, 6), (5, 6), (5, 3.4), (9.8, 8), (5, 12.6), (5, 10), (0.5, 10)]),
            ('v', '#fff7d6', '#e6c46c'), MANILA_OL, 0.8)
    return ic


def i_recent():
    ic = Icon()
    ic.fill(page(4.5, 6, 12.5, 15.5, 2.2), PAPER, '#45524f', 0.7)
    ic.fill(page(2.5, 4.2, 10.5, 13.7, 2.2), PAPER, '#45524f', 0.7)
    ic.fill(fold_tri(10.5, 4.2, 2.2), '#ffffff', '#45524f', 0.6)
    ic.fill(('ellipse', 7.6, 0.4, 15.6, 8.4), ('r', '#ffffff', '#bdbdbd'), '#3a3a3a', 0.8)
    ic.line([(11.6, 4.4), (11.6, 2.0)], '#202020', 0.7)
    ic.line([(11.6, 4.4), (13.6, 4.4)], '#202020', 0.7)
    ic.fill(('poly', [(8.2, 1.2), (8.2, 3.6), (5.6, 3.6), (5.6, 5.4), (7.0, 5.4), (4.4, 8.6),
                      (1.6, 5.4), (3.2, 5.4), (3.2, 1.2)]), GREEN, GREEN_OL, 0.7)
    return ic


def undo_poly(mirror=False):
    pts = []
    cx, cy = 9.0, 13.0
    for i in range(0, 19):
        t = math.radians(-90 + i * 5)
        pts.append((cx + 6.0 * math.cos(t), cy + 8.5 * math.sin(t)))
    for i in range(18, -1, -1):
        t = math.radians(-90 + i * 5)
        pts.append((cx + 2.6 * math.cos(t), cy + 4.4 * math.sin(t)))
    pts += [(6.6, 8.6), (6.6, 11.2), (0.8, 6.55), (6.6, 1.9), (6.6, 4.5)]
    if mirror:
        pts = [(16 - x, y) for x, y in pts]
    return ('poly', pts)


def i_undo():
    ic = Icon()
    ic.fill(undo_poly(), BLUE, BLUE_OL, 0.8)
    return ic


def i_redo():
    ic = Icon()
    ic.fill(undo_poly(True), BLUE, BLUE_OL, 0.8)
    return ic


def i_cut():
    ic = Icon()
    for a, b in (((4.2, 0.8), (11.2, 10.4)), ((11.8, 0.8), (4.8, 10.4))):
        ic.line([a, b], '#3a3a3a', 1.9)
        ic.line([a, b], ('v', '#ffffff', '#b8b8b8'), 1.0)
    for x0 in (0.6, 9.4):
        ring = ('diff', ('ellipse', x0, 9.4, x0 + 6, 15.4), ('ellipse', x0 + 1.6, 11.0, x0 + 4.4, 13.8))
        ic.fill(ring, ('v', '#ee5b4a', '#a3150e'), '#4a0c08', 0.6)
    ic.fill(('ellipse', 7.2, 6.4, 8.8, 8.0), '#2a2a2a')
    return ic


def i_copy():
    ic = Icon()
    for (x0, y0, x1, y1) in ((1, 0.5, 10.5, 11.5), (5.5, 4.5, 15, 15.5)):
        ic.fill(page(x0, y0, x1, y1, 2.6), ('v', '#ffffff', '#d8e2ea'), '#46525e', 0.7)
        ic.fill(fold_tri(x1, y0, 2.6), '#ffffff', '#46525e', 0.6)
    for y in (8.2, 10.2, 12.2):
        ic.line([(7.6, y), (12.8, y)], '#6f97c4', 0.6, caps=False)
    return ic


def i_paste():
    ic = Icon()
    ic.fill(('rect', 1.5, 2, 14.5, 15.5, 1.0), ('v', '#bb6531', '#77300c'), '#3e1704', 0.8)
    ic.fill(('rect', 3.4, 4.2, 12.6, 14.4), ('v', '#f6fcfd', '#cfe1e3'), '#78898a', 0.5)
    ic.fill(('rect', 5, 0.5, 11, 4.6, 1.0), ('v', '#f2f2f2', '#949494'), '#3a3a3a', 0.7)
    ic.fill(('ellipse', 7.2, 1.2, 8.8, 2.7), '#5a5a5a')
    return ic


def i_find():
    ic = Icon()
    met = ('h', '#b4bcca', '#3c4456')
    ol = '#1a202c'
    ic.fill(('rect', 2, 1.5, 6.2, 7.5, 0.8), met, ol, 0.7)
    ic.fill(('rect', 9.8, 1.5, 14, 7.5, 0.8), met, ol, 0.7)
    ic.fill(('rect', 6.0, 6.5, 10.0, 11.0), ('v', '#8a93a3', '#3a4252'), ol, 0.6)
    ic.fill(('rect', 0.6, 6, 7.4, 15.5, 1.4), met, ol, 0.8)
    ic.fill(('rect', 8.6, 6, 15.4, 15.5, 1.4), met, ol, 0.8)
    ic.fill(('rect', 1.6, 13.4, 6.4, 14.6, 0.5), '#9fc6ee')
    ic.fill(('rect', 9.6, 13.4, 14.4, 14.6, 0.5), '#9fc6ee')
    return ic


def i_replace():
    ic = Icon()
    loop = ('diff', ('rect', 0.8, 2.2, 15.2, 12.8, 3.6), ('rect', 3.4, 4.8, 12.6, 10.2, 1.2),
            ('rect', 6.0, 9.0, 10.6, 14))
    ic.fill(loop, BLUE, BLUE_OL, 0.8)
    ic.fill(('poly', [(10.8, 8.4), (10.8, 15.6), (5.6, 12.0)]), BLUE, BLUE_OL, 0.8)
    return ic


def i_find_files():
    ic = Icon()
    ic.fill(page(4.5, 2.5, 15.5, 15.5, 2.6), PAPER, '#45524f', 0.7)
    ring = ('diff', ('ellipse', 1.4, 0.4, 12.6, 11.6), ('ellipse', 3.0, 2.0, 11.0, 10.0))
    ic.fill(('ellipse', 3.0, 2.0, 11.0, 10.0), ('r', '#ffffff', '#c9eed6'))
    ic.fill(ring, ('v', '#74e29a', '#159446'), '#08441e', 0.6)
    ic.line([(3.6, 9.6), (1.1, 14.6)], '#141414', 2.4)
    ic.line([(3.4, 10.4), (1.6, 13.8)], '#6a6a6a', 0.7)
    return ic


def i_wrap():
    ic = Icon()
    ic.fill(('rect', 1, 1.5, 15, 14.5, 1.6), ('v', '#ffffff', '#bcbcbc'), '#575757', 0.8)
    ic.fill(('rect', 2.3, 2.5, 13.7, 12.0, 1.0), ('v', '#fdfdfd', '#e0e0e0'))
    ic.line([(11.4, 4.8), (11.4, 8.4), (6.0, 8.4)], '#3c3c3c', 1.0)
    ic.fill(('poly', [(3.9, 8.4), (6.4, 6.4), (6.4, 10.4)]), '#3c3c3c')
    return ic


def i_fold():
    ic = Icon()
    c = '#55606c'
    ic.line([(8, 4.5), (8, 7.6)], c, 0.8, caps=False)
    ic.line([(3.5, 7.6), (12.5, 7.6)], c, 0.8, caps=False)
    ic.line([(3.5, 7.6), (3.5, 10)], c, 0.8, caps=False)
    ic.line([(12.5, 7.6), (12.5, 10)], c, 0.8, caps=False)
    for (x0, y0) in ((5.8, 0.5), (1.3, 9.8), (10.3, 9.8)):
        ic.fill(page(x0, y0, x0 + 4.4, y0 + 5.4, 1.6), ('v', '#ffffff', '#c3d1df'), '#3c4a58', 0.6)
    return ic


def i_mark():
    ic = Icon()
    ic.line([(6.4, 9.6), (1.2, 14.8)], '#2a2a2a', 2.0)
    ic.line([(6.0, 10.2), (1.8, 14.2)], '#9a9a9a', 0.7)
    ic.fill(('ellipse', 4.8, 0.4, 15.6, 11.2), ('r', '#ffffff', '#b9c7d6'), '#36475a', 0.9)
    col = '#e0661a'
    x0, y0, x1, y1 = 7.4, 3.0, 13.0, 8.6
    for (a, b) in (((x0, y0), (x1, y0)), ((x1, y0), (x1, y1)), ((x1, y1), (x0, y1)), ((x0, y1), (x0, y0))):
        L = math.hypot(b[0] - a[0], b[1] - a[1])
        n = 3
        for i in range(n):
            t0 = i / n
            t1 = t0 + 0.55 / n
            ic.line([(a[0] + (b[0] - a[0]) * t0, a[1] + (b[1] - a[1]) * t0),
                     (a[0] + (b[0] - a[0]) * t1, a[1] + (b[1] - a[1]) * t1)], col, 0.8, caps=False)
    return ic


def i_star():
    ic = Icon()
    ic.fill(star(8, 8.6, 7.7, 3.3), ('v', '#fff9a6', '#e4ae00'), '#6e4e00', 0.8)
    return ic


def i_star_add():
    ic = Icon()
    ic.fill(star(7.0, 7.4, 6.8, 2.9), ('v', '#fff9a6', '#e4ae00'), '#6e4e00', 0.8)
    plus = ('union', [('rect', 10.6, 8.4, 13.2, 15.6), ('rect', 8.3, 10.7, 15.5, 13.3)])
    ic.fill(plus, GREEN, GREEN_OL, 0.6)
    return ic


def zoom_base():
    ic = Icon()
    ic.line([(6.2, 9.8), (1.0, 15.0)], '#202020', 1.9)
    ic.line([(5.8, 10.4), (1.6, 14.4)], '#a8a8a8', 0.7)
    ic.fill(('ellipse', 4.4, 0.4, 15.6, 11.6), ('r', '#ffffff', '#b7cbf4'), '#1a36a0', 1.0)
    return ic


def i_zoom_in():
    ic = zoom_base()
    ic.line([(7.2, 6), (12.8, 6)], '#1a36a0', 1.4)
    ic.line([(10, 3.2), (10, 8.8)], '#1a36a0', 1.4)
    return ic


def i_zoom_out():
    ic = zoom_base()
    ic.line([(7.2, 6), (12.8, 6)], '#1a36a0', 1.4)
    return ic


def i_zoom_reset():
    ic = zoom_base()
    ic.text('1', (6.5, 1.0, 13.5, 11.0), '#1a36a0', font='arialbd.ttf', size=8)
    return ic


def i_ruler():
    ic = Icon()
    ic.fill(('rect', 0.5, 4, 15.5, 12, 0.8), ('v', '#f3db8b', '#c49a38'), '#5e400c', 0.8)
    for i in range(8):
        x = 2.4 + i * 1.6
        ic.line([(x, 4.9), (x, 9.4 if i % 2 == 0 else 7.6)], '#3c2a06', 0.5, caps=False)
    return ic


def i_terminal():
    ic = Icon()
    ic.fill(('rect', 0.5, 2, 15.5, 14, 1.0), ('v', '#a6a6a6', '#565656'), '#161616', 0.7)
    ic.fill(('rect', 1.8, 3.2, 14.2, 12.8), ('v', '#1e2a36', '#0e141a'))
    ic.line([(3.6, 5.4), (5.6, 7.1), (3.6, 8.8)], '#dcdcdc', 0.7)
    ic.line([(6.6, 9.3), (9.6, 9.3)], '#dcdcdc', 0.7, caps=False)
    return ic


def i_pin():
    ic = Icon()
    ic.fill(('rect', 0.5, 6.5, 15.5, 15.5), ('v', '#ffffff', '#cdd6df'), '#44505c', 0.8)
    ic.line([(4.2, 12.0), (8.6, 7.6)], '#4a4a4a', 0.8)
    ic.fill(rot_rect(8.9, 7.3, 5.0, 1.5, -45), ('v', '#7db8ff', '#1f58c4'), '#0b2a6e', 0.5)
    ic.fill(rot_rect(10.5, 5.7, 2.2, 3.0, -45), ('v', '#5d9cf0', '#1a4cb0'), '#0b2a6e', 0.5)
    ic.fill(('ellipse', 10.6, 0.4, 15.6, 5.4), ('r', '#b4dcff', '#1f58c4', 0.35, 0.3), '#0b2a6e', 0.6)
    return ic


def i_exit():
    ic = Icon()
    ic.fill(('ellipse', 0.4, 0.4, 15.6, 15.6), ('r', '#9ccbff', '#1849ac', 0.35, 0.3), '#0a2660', 0.8)
    ic.stroke(('ellipse', 1.5, 1.5, 14.5, 14.5), (255, 255, 255, 215), 0.8)
    ic.line([(5.3, 5.3), (10.7, 10.7)], '#ffffff', 1.9)
    ic.line([(10.7, 5.3), (5.3, 10.7)], '#ffffff', 1.9)
    return ic


def i_print():
    ic = Icon()
    ic.fill(('rect', 3.5, 0.8, 12.5, 7), ('v', '#ffffff', '#dfe6ea'), '#4a5560', 0.6)
    ic.fill(('rect', 0.5, 5.8, 15.5, 12.4, 1.0), ('v', '#dadada', '#828282'), '#262626', 0.8)
    ic.fill(('rect', 12.3, 7.2, 14, 8.4), '#3ed62a')
    ic.fill(('rect', 3.5, 10.2, 12.5, 15.5), ('v', '#ffffff', '#dfe6ea'), '#4a5560', 0.6)
    for y in (12.2, 13.8):
        ic.line([(5, y), (11, y)], '#7f9fc4', 0.6, caps=False)
    return ic


def i_font():
    ic = Icon()
    ic.text('A', (0.0, 0.5, 11.0, 15.5), ('v', '#7ab0f0', '#1f4ea0'), BLUE_OL, font='georgiab.ttf', size=14, w=0.5)
    ic.text('a', (8.5, 6.5, 16.0, 15.8), ('v', '#f07a6a', '#a8180e'), '#4a0c08', font='georgiab.ttf', size=9, w=0.5)
    return ic


def i_about():
    ic = Icon()
    ic.fill(('ellipse', 0.4, 0.4, 15.6, 15.6), ('r', '#9ccbff', '#1849ac', 0.35, 0.3), '#0a2660', 0.8)
    ic.fill(('ellipse', 6.9, 2.6, 9.1, 4.8), '#ffffff')
    ic.fill(('rect', 6.9, 6.0, 9.1, 13.0, 0.4), '#ffffff')
    return ic


def i_goto():
    ic = Icon()
    ic.fill(page(1.5, 0.5, 12.5, 15.5, 2.6), PAPER, '#45524f', 0.7)
    for y in (4, 6.5, 11.5, 13.5):
        ic.line([(3.5, y), (10, y)], '#9ab0c4', 0.6, caps=False)
    ic.fill(('poly', [(5.5, 7.3), (11, 7.3), (11, 5.3), (15.6, 9), (11, 12.7), (11, 10.7), (5.5, 10.7)]),
            GREEN, GREEN_OL, 0.7)
    return ic


def i_settings():
    ic = Icon()
    teeth = []
    for i in range(32):
        a = math.radians(i * 11.25 - 5.6)
        r = 7.6 if (i // 2) % 2 == 0 else 5.6
        teeth.append((8 + r * math.cos(a), 8 + r * math.sin(a)))
    gear = ('diff', ('poly', teeth), ('ellipse', 5.6, 5.6, 10.4, 10.4))
    ic.fill(gear, ('v', '#d8d8d8', '#6c6c6c'), '#222222', 0.8)
    return ic


def i_app():
    ic = Icon()
    ic.fill(('rect', 1.5, 2.2, 13.5, 15.6, 0.6), PAPER, PAPER_OL, 0.7)
    ic.fill(('rect', 1.5, 1.4, 13.5, 4.4, 0.6), ('v', '#6cb0e8', '#2a6cb0'), '#173a6e', 0.6)
    for x in (3.6, 6.0, 8.4, 10.8):
        ic.fill(('rect', x, 0.3, x + 1.0, 3.4, 0.5), ('v', '#ffffff', '#9a9a9a'), '#333333', 0.4)
    ic.line([(3.8, 4.6), (3.8, 15.2)], '#e07070', 0.45, caps=False)
    for y in (6.6, 8.6, 10.6, 12.6):
        ic.line([(2.6, y), (12.4, y)], '#9ab8d8', 0.45, caps=False)
    ic.fill(('ellipse', 7.6, 7.6, 15.9, 15.9), ('r', '#ffe680', '#e68a00', 0.35, 0.3), '#6e3e00', 0.7)
    ic.text('9', (7.6, 7.4, 15.9, 15.7), '#3a2000', font='arialbd.ttf', size=7.4)
    return ic


# ---------------------------------------------------------------- run / console
def i_run():
    ic = Icon()
    ic.fill(('poly', [(3, 1.5), (14.5, 8), (3, 14.5)]), ('v', '#8ef06a', '#22961a'), GREEN_OL, 0.8)
    return ic


def i_stop():
    ic = Icon()
    ic.fill(('rect', 2.5, 2.5, 13.5, 13.5, 1.5), ('v', '#ff8270', '#b51f16'), '#470b07', 0.8)
    return ic


def i_clear():
    ic = Icon()
    ang = -40
    d = (math.cos(math.radians(ang)), math.sin(math.radians(ang)))
    ic.fill(rot_rect(8.4, 7.6, 13.5, 6.4, ang), ('v', '#ffb6c9', '#de6488'), '#4e1426', 0.7)
    ic.fill(rot_rect(8.4 - 3.4 * d[0], 7.6 - 3.4 * d[1], 6.6, 6.4, ang), ('v', '#f4f8ff', '#9fb4d4'), '#2c3a52', 0.7)
    return ic


def i_preview():
    ic = Icon()
    ic.fill(('rect', 0.5, 1.5, 15.5, 14.5), ('v', '#ffffff', '#d4e4f0'), '#1d4c80', 0.8)
    ic.fill(('rect', 0.5, 1.5, 15.5, 4.2), ('v', '#6cb0e8', '#2a74b6'))
    ic.stroke(('rect', 0.5, 1.5, 15.5, 14.5), '#1d4c80', 0.8)
    ic.line([(8, 4.2), (8, 14)], '#1d4c80', 0.7, caps=False)
    for y, l in ((6.4, 4.5), (8.4, 3.5), (10.4, 4.8), (12.4, 3.0)):
        ic.line([(2.2, y), (2.2 + l, y)], '#7a8a9a', 0.7, caps=False)
    ic.fill(('rect', 9.3, 5.6, 14, 7.2), '#2a74b6')
    ic.line([(9.3, 8.8), (14, 8.8)], '#9aa8b6', 0.7, caps=False)
    ic.fill(('rect', 9.3, 10.2, 12, 12.8), ('v', '#86e45e', '#279a18'))
    return ic


# ---------------------------------------------------------------- markdown toolbar
LETTER = ('v', '#ffffff', '#c4c4c4')
LETTER_OL = '#161616'


def i_md_bold():
    ic = Icon()
    ic.text('B', (0, 0, 16, 16), LETTER, LETTER_OL, font='arialbd.ttf', size=13, w=0.6)
    return ic


def i_md_italic():
    ic = Icon()
    ic.text('I', (0, 0, 16, 16), LETTER, LETTER_OL, font='georgiai.ttf', size=14, w=0.6)
    return ic


def i_md_strike():
    ic = Icon()
    ic.text('S', (0, 0, 16, 16), LETTER, LETTER_OL, font='arial.ttf', size=13, w=0.6)
    ic.line([(1.5, 8.5), (14.5, 8.5)], '#2a0606', 2.0)
    ic.line([(1.5, 8.5), (14.5, 8.5)], '#f04a3a', 1.0)
    return ic


def i_md_heading():
    ic = Icon()
    ic.text('H', (0, 0, 12, 16), LETTER, LETTER_OL, font='arialbd.ttf', size=12, w=0.6)
    ic.text('1', (10, 6, 16, 16), ('v', '#8ab8f0', '#2c5ea8'), BLUE_OL, font='arialbd.ttf', size=7.5, w=0.5)
    return ic


def i_md_code():
    ic = Icon()
    for pts in ([(5.6, 3.5), (1.8, 8), (5.6, 12.5)], [(10.4, 3.5), (14.2, 8), (10.4, 12.5)]):
        ic.line(pts, '#0c3a10', 2.6)
        ic.line(pts, '#6ee05a', 1.2)
    return ic


def i_md_codeblock():
    ic = Icon()
    ic.fill(page(1.5, 0.5, 14.5, 15.5, 3), PAPER, PAPER_OL, 0.8)
    ic.fill(fold_tri(14.5, 0.5, 3), '#ffffff', PAPER_OL, 0.6)
    ic.text('{}', (1.5, 3, 14.5, 15), ('v', '#4a8ad8', '#1d4c96'), font='consolab.ttf', size=9)
    return ic


def stadium(cx, cy, length, rad, ang):
    a = math.radians(ang)
    ux, uy = math.cos(a), math.sin(a)
    vx, vy = -uy, ux
    pts = []
    for end, start_deg in ((1, -90), (-1, 90)):
        ex, ey = cx + ux * length / 2 * end, cy + uy * length / 2 * end
        for i in range(0, 19):
            t = math.radians(start_deg + i * 10)
            pts.append((ex + (ux * math.cos(t) + vx * math.sin(t)) * rad * 1,
                        ey + (uy * math.cos(t) + vy * math.sin(t)) * rad * 1))
    pts.append(pts[0])
    return pts


def i_md_link():
    ic = Icon()
    for cx, cy in ((5.2, 10.8), (10.8, 5.2)):
        pts = stadium(cx, cy, 3.6, 2.3, -45)
        ic.line(pts, '#0b2a6e', 2.4, caps=False)
        ic.line(pts, ('v', '#a8d2ff', '#2a6cd0'), 1.1, caps=False)
    return ic


def i_md_image():
    ic = Icon()
    ic.fill(('rect', 0.5, 2, 15.5, 14), ('v', '#ffffff', '#cfd8e0'), '#44505c', 0.8)
    ic.fill(('rect', 2, 3.5, 14, 12.5), ('v', '#86c6ff', '#dcf2ff'))
    ic.fill(('ellipse', 10.2, 4.4, 13, 7.2), ('r', '#fff6a0', '#f0b800'))
    ic.fill(('poly', [(2, 12.5), (6.4, 6.6), (9.4, 10.2), (11, 8.6), (14, 12.5)]), ('v', '#78d866', '#2a8a20'))
    return ic


def _list_lines(ic, ys, x0=6):
    for y in ys:
        ic.line([(x0, y), (15, y)], ('v', '#f0f0f0', '#b8b8b8'), 1.2, caps=False)


def i_md_ul():
    ic = Icon()
    for y in (4, 8, 12):
        ic.fill(('ellipse', 1, y - 1.6, 4.2, y + 1.6), ('r', '#a8d2ff', '#1f58c4'), '#0b2a6e', 0.5)
    _list_lines(ic, (4, 8, 12))
    return ic


def i_md_ol():
    ic = Icon()
    for n, y in (('1', 4), ('2', 8), ('3', 12)):
        ic.text(n, (0, y - 2.6, 5, y + 2.6), ('v', '#8ab8f0', '#2c5ea8'), font='arialbd.ttf', size=5.2)
    _list_lines(ic, (4, 8, 12))
    return ic


def i_md_task():
    ic = Icon()
    ic.fill(('rect', 0.5, 1.5, 7, 8), ('v', '#ffffff', '#d8e0e8'), '#3a4652', 0.7)
    ic.fill(('rect', 0.5, 9.5, 7, 16), ('v', '#ffffff', '#d8e0e8'), '#3a4652', 0.7)
    ic.line([(1.9, 4.6), (3.5, 6.4), (6.6, 2.2)], '#0c4a0a', 2.0)
    ic.line([(1.9, 4.6), (3.5, 6.4), (6.6, 2.2)], '#5ad84a', 1.0)
    _list_lines(ic, (4.8, 12.8), x0=9)
    return ic


def i_md_quote():
    ic = Icon()
    ic.text('“', (0, 0, 16, 16), BLUE, BLUE_OL, font='georgiab.ttf', size=24, w=0.5)
    return ic


def i_md_table():
    ic = Icon()
    ic.fill(('rect', 0.5, 1.5, 15.5, 14.5), ('v', '#ffffff', '#d8e0e8'), '#3a4652', 0.8)
    ic.fill(('rect', 0.5, 1.5, 15.5, 5.2), ('v', '#6cb0e8', '#2a6cb0'))
    ic.stroke(('rect', 0.5, 1.5, 15.5, 14.5), '#3a4652', 0.8)
    for x in (5.5, 10.5):
        ic.line([(x, 1.5), (x, 14.5)], '#3a4652', 0.7, caps=False)
    ic.line([(0.5, 9.8), (15.5, 9.8)], '#3a4652', 0.7, caps=False)
    return ic


def i_md_hr():
    ic = Icon()
    ic.line([(1.5, 3.5), (14.5, 3.5)], '#8a8a8a', 0.8, caps=False)
    ic.line([(0.8, 8), (15.2, 8)], BLUE_OL, 2.6)
    ic.line([(0.8, 8), (15.2, 8)], ('v', '#9cc8ff', '#2a6cd0'), 1.2)
    ic.line([(1.5, 12.5), (14.5, 12.5)], '#8a8a8a', 0.8, caps=False)
    return ic


# ---------------------------------------------------------------- autocomplete kinds
def i_ac_key():
    ic = Icon()
    ic.fill(('rect', 1.5, 1.5, 14.5, 14.5, 2), BLUE, BLUE_OL, 0.8)
    ic.text('K', (1.5, 1.5, 14.5, 14.5), '#ffffff', font='arialbd.ttf', size=9)
    return ic


def i_ac_word():
    ic = Icon()
    ic.fill(('rect', 1.5, 1.5, 14.5, 14.5, 2), ('v', '#a8a8a8', '#5c5c5c'), '#1e1e1e', 0.8)
    ic.text('ab', (1.5, 1.5, 14.5, 14.5), '#ffffff', font='arialbd.ttf', size=7)
    return ic


ICONS = {
    'run': i_run, 'stop': i_stop, 'clear': i_clear, 'preview': i_preview,
    'md_bold': i_md_bold, 'md_italic': i_md_italic, 'md_strike': i_md_strike,
    'md_heading': i_md_heading, 'md_code': i_md_code, 'md_codeblock': i_md_codeblock,
    'md_link': i_md_link, 'md_image': i_md_image, 'md_ul': i_md_ul, 'md_ol': i_md_ol,
    'md_task': i_md_task, 'md_quote': i_md_quote, 'md_table': i_md_table, 'md_hr': i_md_hr,
    'ac_key': i_ac_key, 'ac_word': i_ac_word,
    'new': i_new, 'window_new': i_window_new, 'open': i_open, 'save': i_save,
    'save_as': i_save_as, 'recent': i_recent, 'undo': i_undo, 'redo': i_redo,
    'cut': i_cut, 'copy': i_copy, 'paste': i_paste, 'find': i_find,
    'replace': i_replace, 'find_files': i_find_files, 'wrap': i_wrap,
    'fold': i_fold, 'mark': i_mark, 'star': i_star, 'star_add': i_star_add,
    'zoom_in': i_zoom_in, 'zoom_reset': i_zoom_reset, 'zoom_out': i_zoom_out,
    'ruler': i_ruler, 'terminal': i_terminal, 'pin': i_pin, 'exit': i_exit,
    'print': i_print, 'font': i_font, 'about': i_about, 'goto': i_goto,
    'settings': i_settings,
}


def disabled(img):
    """Gray, flat look used by the reference for disabled toolbar buttons."""
    a = np.asarray(img.convert('RGBA')).astype(np.float32)
    lum = 0.3 * a[..., 0] + 0.59 * a[..., 1] + 0.11 * a[..., 2]
    v = 52 + lum * 0.40
    out = np.stack([v, v, v, a[..., 3] * 0.92], axis=-1)
    return Image.fromarray((out + 0.5).astype(np.uint8), 'RGBA')


def render(fn, size):
    setup(size)
    return fn().result()


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    out_dir = sys.argv[1] if len(sys.argv) > 1 else os.path.join(here, '..', 'Notepad9', 'Assets', 'Icons')
    preview = sys.argv[2] if len(sys.argv) > 2 else None
    os.makedirs(out_dir, exist_ok=True)
    for old in glob.glob(os.path.join(out_dir, '*.png')):
        os.remove(old)

    rendered = {}
    for name, fn in ICONS.items():
        for size in TB_SIZES:
            im = render(fn, size)
            im.save(os.path.join(out_dir, f'{name}_{size}.png'))
            disabled(im).save(os.path.join(out_dir, f'{name}_{size}_d.png'))
            rendered[(name, size)] = im
        print('icon', name)

    # app icon: see make_app_icon.py
    sys.path.insert(0, here)
    import make_app_icon
    make_app_icon.build(out_dir)
    print('app icon')

    if preview:
        names = list(ICONS)
        z = 3
        rows = [(16, False), (16, True), (20, False), (20, True)]
        cell = 20 * z + 8
        sheet = Image.new('RGBA', (cell * len(names) + 8, cell * len(rows) + 140), (20, 20, 20, 255))
        for r, (size, dis) in enumerate(rows):
            for i, n in enumerate(names):
                im = rendered[(n, size)]
                if dis:
                    im = disabled(im)
                sheet.alpha_composite(im.resize((size * z, size * z), Image.NEAREST), (8 + i * cell, 8 + r * cell))
        x = 8
        for size in (16, 20, 24, 32, 48, 64):
            zz = 2 if size <= 48 else 1
            sheet.alpha_composite(apps[size].resize((size * zz, size * zz), Image.NEAREST), (x, cell * len(rows) + 10))
            x += size * zz + 12
        sheet.save(preview)


if __name__ == '__main__':
    main()
