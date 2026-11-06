#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = ["Pillow==12.0.0"]
# ///
"""Regenerate the fnOS package and launcher icons from the brand master.

Run: uv run --script scripts/fn-knock-icon-squircle.py [--check | --preview PATH]
The committed PNGs are used directly by the FPK and SPK builds.
"""

import argparse
from io import BytesIO
from pathlib import Path

from PIL import Image, ImageDraw, ImageOps, __version__ as pillow_version

ROOT = Path(__file__).resolve().parents[1]
MASTER = ROOT / "packages/icons/favicon-512x512.png"

# Blue-Mink's measured left edge on a 224 px fnOS tile, through its first
# straight row. The measurements approximate the 50% alpha contour.
# https://github.com/kci-lnk/fn-knock-turborepo/issues/68
LEFT_EDGE = (
    57, 46, 41, 37, 34, 32, 30, 28, 26, 25, 23, 22, 21, 20, 18, 17,
    16, 15, 15, 14, 13, 12, 11, 10, 10, 9, 8, 8, 7, 7, 6, 6,
    5, 5, 4, 4, 4, 3, 3, 3, 3, 2, 2, 2, 2, 2, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0,
)
SIZES = (64, 192, 256)


def corner_points():
    # Sample the shallower half of the measured edge, averaging neighbouring
    # rows to smooth out pixel quantization. Reflect it across the diagonal.
    diagonal = (LEFT_EDGE[16] + 16.5) / 2
    half = [(diagonal, diagonal)]
    for row in range(20, 57, 4):
        x = sum(LEFT_EDGE[row - 2:row + 3]) / 5
        half.append((x, row + 0.5))
    half.extend([(0, 58.5), (0, 62)])
    points = [(y, x) for x, y in reversed(half[1:])] + half

    # Chaikin subdivision rounds the knots and preserves monotonicity.
    for _ in range(4):
        curve = [points[0]]
        for (x1, y1), (x2, y2) in zip(points, points[1:]):
            curve.append(((3 * x1 + x2) / 4, (3 * y1 + y2) / 4))
            curve.append(((x1 + 3 * x2) / 4, (y1 + 3 * y2) / 4))
        points = curve + [points[-1]]
    return points


def render_icon(master, size):
    # Rasterize one quadrant, then mirror its coverage exactly at every size.
    side = size * 8
    middle = side // 2
    quadrant = Image.new("L", (middle, middle))
    scale = side / 224
    outline = [(round(x * scale), round(y * scale)) for x, y in corner_points()]
    ImageDraw.Draw(quadrant).polygon(
        outline + [(0, middle), (middle, middle), (middle, 0)], fill=255
    )
    corner = quadrant.resize((size // 2, size // 2), Image.Resampling.BOX)
    mask = Image.new("L", (size, size))
    mask.paste(corner, (0, 0))
    mask.paste(ImageOps.mirror(corner), (size // 2, 0))
    mask.paste(ImageOps.flip(mask.crop((0, 0, size, size // 2))), (0, size // 2))
    icon = master.resize((size, size), Image.Resampling.LANCZOS)
    icon.putalpha(mask)
    return icon


def targets():
    for package in ("fn-knock", "fn-knock-lite"):
        root = ROOT / "apps" / package
        for directory, default_size in [(root, 64), (root / "app/ui/images", 192)]:
            yield directory / "ICON.PNG", default_size
            yield directory / "ICON_64.PNG", 64
            yield directory / "ICON_256.PNG", 256


def preview(icons, path):
    margin = 16
    width = sum(SIZES) + margin * (len(SIZES) + 1)
    height = max(SIZES) + margin * 2
    sheet = Image.new("RGB", (width, 2 * height))
    for row, background in enumerate(("#eeeeee", "#292929")):
        top = row * height
        sheet.paste(background, (0, top, width, top + height))
        left = margin
        for size, icon in icons.items():
            sheet.paste(icon, (left, top + (height - size) // 2), icon)
            left += size + margin
    sheet.save(path, format="PNG")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--check", action="store_true", help="verify committed icons without writing")
    mode.add_argument("--preview", type=Path, help="save a light/dark contact sheet and exit")
    args = parser.parse_args()
    if pillow_version != "12.0.0":
        parser.exit(1, "Run with uv run --script to use the pinned Pillow version.\n")
    with Image.open(MASTER) as master:
        if master.size != (512, 512) or master.mode != "RGB":
            parser.exit(1, f"Expected a 512 px RGB master: {MASTER}\n")
        icons = {size: render_icon(master, size) for size in SIZES}
    slots = dict(targets())
    if args.preview:
        if args.preview.resolve() in {*slots, MASTER}:
            parser.error("choose a preview path distinct from the source and package icons")
        preview(icons, args.preview)
        return

    encoded = {}
    for size, icon in icons.items():
        buffer = BytesIO()
        icon.save(buffer, format="PNG")
        encoded[size] = buffer.getvalue()

    # Read every existing slot before any writes. A missing file or wrong
    # checkout must fail without leaving the packages partially updated.
    changed = [path for path, size in slots.items() if path.read_bytes() != encoded[size]]
    if args.check and changed:
        parser.exit(1, "Outdated icons:\n" + "\n".join(str(p.relative_to(ROOT)) for p in changed) + "\n")
    for path in changed:
        path.write_bytes(encoded[slots[path]])
    print(f"Verified {len(slots)} icon slots ({len(changed)} updated).")


if __name__ == "__main__":
    try:
        main()
    except OSError as error:
        raise SystemExit(str(error)) from error
