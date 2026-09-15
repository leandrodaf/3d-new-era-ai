#!/usr/bin/env python3
"""Renders every icon the project ships, from one hand-drawn mark.

The artwork is the same mark as the site header (site/index.html): an outlined
square, the corner run and the accent arc. It is drawn here on the same grid as
assets/icon.svg, so no SVG renderer is needed.

    python3 scripts/make-icons.py

Writes, for each platform:

  macOS    assets/icon.icns, assets/document.icns (the .newera file icon)
  Windows  assets/icon.ico + crates/newera/assets/{icon,document}.ico, linked
           into the executables by crates/newera/build.rs
  Linux    assets/linux/hicolor/**, the icon theme the desktop entry points at
  app      crates/newera-app/assets/icon-256.png, the window/dock/taskbar icon
  docs     assets/icon-*.png, also used as the site's apple-touch-icon

The copies live inside the crates so `cargo package` keeps working.
"""

import math
import pathlib
import shutil
import subprocess
import tempfile

from PIL import Image, ImageDraw

ROOT = pathlib.Path(__file__).resolve().parent.parent
ASSETS = ROOT / "assets"
APP_ASSETS = ROOT / "crates" / "newera-app" / "assets"
CLI_ASSETS = ROOT / "crates" / "newera" / "assets"
HICOLOR = ASSETS / "linux" / "hicolor"
SITE = ROOT / "site"

INK = (22, 21, 19, 255)  # --ink
PAPER = (244, 241, 234, 255)  # --paper on dark
ACCENT = (125, 147, 255, 255)  # --accent on dark
SHEET = (250, 249, 245, 255)  # the document page
FOLD = (214, 209, 197, 255)  # its folded corner

SS = 4  # supersampling
MASTER = 1024

# The mark, on the same 1024 grid as assets/icon.svg. Every number is a stroke
# *centre* line, like an SVG stroke, so the pieces meet exactly.
STROKE = 47.0
LEFT = TOP = 277.0
RIGHT = BOTTOM = 747.0
TURN = 535.5  # the run turns here: set so the arc lands tangent to the top wall
RADIUS = RIGHT - TURN  # the arc springs off the turn and lands on the right wall
SPAN = (RIGHT - LEFT) + STROKE  # the mark's full width, outer edge to outer edge


def zoom(size: int) -> float:
    """How much to grow the mark at small sizes — optical sizing.

    At 16 px the walls are under a pixel wide and the mark turns to mush, so the
    small icons carry a slightly larger mark (thicker strokes, less margin),
    the way a proper icon set is drawn size by size. The tile never changes.
    """
    if size <= 24:
        return 1.28
    if size <= 48:
        return 1.14
    return 1.0


def _mark(draw, at, line_colour: tuple, accent_colour: tuple) -> None:
    """Draws the mark; `at(x, y)` maps a point on the mark grid to pixels.

    The arc goes down first so the run and the right wall, painted over it,
    trim both of its ends flush — it leaves the turn in line with the run going
    down and arrives square on the right wall. Strokes are filled shapes rather
    than Pillow outlines, because `rectangle(outline=...)` draws inwards from
    its box and `arc(width=...)` frays at the ends.
    """
    h = STROKE / 2

    cx, cy = RIGHT, TURN
    steps = 256
    outer, inner = [], []
    for i in range(steps + 1):
        # A few degrees past each end, so the paper covers the seam.
        a = math.radians(176.0 + 98.0 * i / steps)
        dx, dy = math.cos(a), math.sin(a)
        outer.append(at(cx + (RADIUS + h) * dx, cy + (RADIUS + h) * dy))
        inner.append(at(cx + (RADIUS - h) * dx, cy + (RADIUS - h) * dy))
    draw.polygon(outer + inner[::-1], fill=accent_colour)

    def bar(x0: float, y0: float, x1: float, y1: float) -> None:
        """A stroke, given by the box its two edges span."""
        draw.rectangle([*at(x0, y0), *at(x1, y1)], fill=line_colour)

    bar(LEFT - h, TOP - h, RIGHT + h, TOP + h)  # top wall
    bar(LEFT - h, BOTTOM - h, RIGHT + h, BOTTOM + h)  # bottom wall
    bar(LEFT - h, TOP - h, LEFT + h, BOTTOM + h)  # left wall
    bar(RIGHT - h, TOP - h, RIGHT + h, BOTTOM + h)  # right wall
    bar(LEFT - h, TURN - h, TURN + h, TURN + h)  # the run, across
    bar(TURN - h, TURN - h, TURN + h, BOTTOM + h)  # the run, down


def render(size: int) -> Image.Image:
    """The app icon: the mark on the ink rounded square."""
    px = size * SS
    scale = px / MASTER
    img = Image.new("RGBA", (px, px), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    # Rounded square, on the macOS 824-in-1024 grid.
    draw.rounded_rectangle(
        [100 * scale, 100 * scale, 924 * scale - 1, 924 * scale - 1],
        radius=185 * scale,
        fill=INK,
    )
    k = zoom(size)
    _mark(draw, lambda x, y: (((x - 512.0) * k + 512.0) * scale,
                              ((y - 512.0) * k + 512.0) * scale), PAPER, ACCENT)
    return img.resize((size, size), Image.LANCZOS)


# The document page, on the same 1024 grid: a portrait sheet with the top-right
# corner folded back, carrying the mark like a plan on paper.
PAGE = (172.0, 52.0, 852.0, 972.0)  # left, top, right, bottom
PAGE_R = 26.0  # corner radius
PAGE_FOLD = 190.0  # the folded corner, along each edge
MARK_W = 430.0  # the mark's width on the page
MARK_C = (512.0, 552.0)  # and its centre


def render_document(size: int) -> Image.Image:
    """The icon for `.newera` files: the mark drawn on a sheet of paper."""
    px = size * SS
    scale = px / MASTER
    img = Image.new("RGBA", (px, px), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    left, top, right, bottom = PAGE

    def s(v: float) -> tuple:
        return v * scale

    draw.rounded_rectangle(
        [s(left), s(top), s(right), s(bottom)], radius=s(PAGE_R), fill=SHEET
    )
    # Cut the corner away (ImageDraw replaces pixels on RGBA, so this erases),
    # then lay the folded flap in its place.
    draw.polygon(
        [
            (s(right - PAGE_FOLD), s(top - 20)),
            (s(right + 20), s(top - 20)),
            (s(right + 20), s(top + PAGE_FOLD)),
        ],
        fill=(0, 0, 0, 0),
    )
    draw.polygon(
        [
            (s(right - PAGE_FOLD), s(top)),
            (s(right), s(top + PAGE_FOLD)),
            (s(right - PAGE_FOLD), s(top + PAGE_FOLD)),
        ],
        fill=FOLD,
    )

    k = MARK_W * zoom(size) / SPAN
    mark_at = lambda x, y: (  # noqa: E731 - a tiny projection, clearer inline
        s((x - 512.0) * k + MARK_C[0]),
        s((y - 512.0) * k + MARK_C[1]),
    )
    _mark(draw, mark_at, INK, ACCENT)
    return img.resize((size, size), Image.LANCZOS)


def icns(images: dict, path: pathlib.Path) -> None:
    """Packs an .icns with every size macOS asks for, 16 to 1024, plus @2x."""
    with tempfile.TemporaryDirectory() as tmp:
        iconset = pathlib.Path(tmp) / "icon.iconset"
        iconset.mkdir()
        for n in (16, 32, 128, 256, 512):
            images[n].save(iconset / f"icon_{n}x{n}.png")
            images[n * 2].save(iconset / f"icon_{n}x{n}@2x.png")
        subprocess.run(
            ["iconutil", "-c", "icns", str(iconset), "-o", str(path)], check=True
        )


ICO_SIZES = [(n, n) for n in (16, 32, 48, 64, 128, 256)]
# The sizes the hicolor theme installs for a Linux desktop.
LINUX_SIZES = (16, 22, 24, 32, 48, 64, 128, 256, 512)


def main() -> None:
    for folder in (ASSETS, APP_ASSETS, CLI_ASSETS):
        folder.mkdir(parents=True, exist_ok=True)

    sizes = sorted({16, 22, 24, 32, 48, 64, 128, 256, 512, 1024})
    app = {n: render(n) for n in sizes}
    doc = {n: render_document(n) for n in sizes}

    # Documentation, the site's touch icon and the window icon.
    for n in (256, 512, 1024):
        app[n].save(ASSETS / f"icon-{n}.png")
    doc[512].save(ASSETS / "document-512.png")
    app[256].save(APP_ASSETS / "icon-256.png")
    app[256].save(SITE / "icon-256.png")

    # Windows: the app icon is the executable's, the document icon is the one
    # the installer points `.newera` at.
    app[256].save(ASSETS / "icon.ico", sizes=ICO_SIZES)
    app[256].save(CLI_ASSETS / "icon.ico", sizes=ICO_SIZES)
    doc[256].save(ASSETS / "document.ico", sizes=ICO_SIZES)
    doc[256].save(CLI_ASSETS / "document.ico", sizes=ICO_SIZES)

    # macOS: the bundle icon and the document icon Info.plist points at.
    icns(app, ASSETS / "icon.icns")
    icns(doc, ASSETS / "document.icns")

    # Linux: a hicolor theme, apps and mimetypes, PNG at every size plus the
    # scalable SVG masters.
    if HICOLOR.exists():
        shutil.rmtree(HICOLOR)
    for n in LINUX_SIZES:
        apps = HICOLOR / f"{n}x{n}" / "apps"
        mimes = HICOLOR / f"{n}x{n}" / "mimetypes"
        apps.mkdir(parents=True, exist_ok=True)
        mimes.mkdir(parents=True, exist_ok=True)
        app[n].save(apps / "newera.png")
        doc[n].save(mimes / "application-x-newera.png")
    scalable_apps = HICOLOR / "scalable" / "apps"
    scalable_mimes = HICOLOR / "scalable" / "mimetypes"
    scalable_apps.mkdir(parents=True, exist_ok=True)
    scalable_mimes.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(ASSETS / "icon.svg", scalable_apps / "newera.svg")
    shutil.copyfile(ASSETS / "document.svg", scalable_mimes / "application-x-newera.svg")

    print("app icon   assets/icon-*.png, icon.icns, icon.ico,")
    print("           crates/newera/assets/icon.ico,")
    print("           crates/newera-app/assets/icon-256.png, site/icon-256.png")
    print("document   assets/document.icns, document.ico, document-512.png,")
    print("           crates/newera/assets/document.ico")
    print("linux      assets/linux/hicolor/** (apps + mimetypes, 16..512 + scalable)")


if __name__ == "__main__":
    main()
