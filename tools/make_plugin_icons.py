"""Toolbar icons of the example Notepad9 plugins (same pixel style as make_icons.py).

python tools/make_plugin_icons.py  ->  Notepad9Rust/plugins/text_tools/icon.png
"""
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
SS = 8  # supersampling


def wand(size: int = 32) -> Image.Image:
    s = size * SS
    img = Image.new("RGBA", (s, s), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    u = s / 32.0

    def P(x, y):
        return (x * u, y * u)

    # stick: dark outline, body, light tip
    d.line([P(5, 27), P(19, 13)], fill=(40, 34, 30, 255), width=int(6 * u))
    d.line([P(5, 27), P(19, 13)], fill=(150, 104, 62, 255), width=int(3.6 * u))
    d.line([P(16, 16), P(19, 13)], fill=(40, 34, 30, 255), width=int(6 * u))
    d.line([P(16, 16), P(19.2, 12.8)], fill=(245, 245, 240, 255), width=int(3.6 * u))

    def star(cx, cy, r, color):
        pts = []
        for i in range(8):
            import math
            a = math.pi / 4 * i - math.pi / 2
            rr = r if i % 2 == 0 else r * 0.36
            pts.append(P(cx + rr * math.cos(a), cy + rr * math.sin(a)))
        d.polygon(pts, fill=(60, 44, 10, 255))
        inner = []
        for i in range(8):
            import math
            a = math.pi / 4 * i - math.pi / 2
            rr = (r - 1.3) if i % 2 == 0 else (r - 1.3) * 0.36
            inner.append(P(cx + rr * math.cos(a), cy + rr * math.sin(a)))
        d.polygon(inner, fill=color)

    star(23.5, 8.5, 7.5, (255, 214, 64, 255))
    star(10.5, 6.5, 4.0, (120, 200, 255, 255))
    star(27.0, 20.0, 3.6, (255, 150, 200, 255))
    return img.resize((size, size), Image.LANCZOS)


def main() -> None:
    out = ROOT / "Notepad9Rust" / "plugins" / "text_tools" / "icon.png"
    out.parent.mkdir(parents=True, exist_ok=True)
    wand(32).save(out)
    print("wrote", out)


if __name__ == "__main__":
    main()
