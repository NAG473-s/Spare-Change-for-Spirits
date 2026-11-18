#!/usr/bin/env python3
"""Prepare a generated image (e.g. from ChatGPT) for the game.

Trims empty transparent borders, fits the image into the aspect ratio the
game draws it at (padding with transparency, never stretching), scales it
and writes it to the right file under assets/sprites/.

    python3 tools/prepare_sprite.py INPUT KIND [ID] [--remove-bg] [--colors N]

KIND is one of: ghost, decor, product, door, counter, floor, background.
ID is the id from the RON data files (needed for ghost, decor, product).
"""

import argparse
import sys
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent

# kind: (output width, output height, folder, cover)
# Output is 4x the size the game draws (2x/1.5x for big images), so it
# stays sharp on high-DPI screens. "cover" images are cropped to fill the
# frame (floor, background); all others are padded and keep their shape.
KINDS = {
    "ghost": (176, 264, "ghosts", False),
    "decor": (208, 208, "decor", False),
    "product": (96, 96, "products", False),
    "door": (256, 384, "shop", False),
    "counter": (192, 1088, "shop", False),
    "floor": (256, 256, "shop", True),
    "background": (1920, 1080, "shop", True),
}
NAMED = {"door", "counter", "floor", "background"}


def remove_background(img: Image.Image, threshold: int) -> Image.Image:
    """Flood-fills the background from all four corners with transparency."""
    img = img.copy()
    w, h = img.size
    for corner in [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)]:
        if img.getpixel(corner)[3] > 0:
            ImageDraw.floodfill(img, corner, (0, 0, 0, 0), thresh=threshold)
    return img


def trim(img: Image.Image) -> Image.Image:
    alpha = img.getchannel("A").point(lambda a: 255 if a > 8 else 0)
    box = alpha.getbbox()
    return img.crop(box) if box else img


def fit(img: Image.Image, width: int, height: int) -> Image.Image:
    """Scales to fit inside width x height and centres it (bottom-aligned)."""
    scale = min(width / img.width, height / img.height)
    size = (max(1, round(img.width * scale)), max(1, round(img.height * scale)))
    img = img.resize(size, Image.LANCZOS)
    out = Image.new("RGBA", (width, height), (0, 0, 0, 0))
    out.paste(img, ((width - size[0]) // 2, height - size[1]), img)
    return out


def cover(img: Image.Image, width: int, height: int) -> Image.Image:
    """Scales to fill width x height and crops the overflow evenly."""
    scale = max(width / img.width, height / img.height)
    size = (round(img.width * scale), round(img.height * scale))
    img = img.resize(size, Image.LANCZOS)
    left = (size[0] - width) // 2
    top = (size[1] - height) // 2
    return img.crop((left, top, left + width, top + height))


def reduce_colors(img: Image.Image, colors: int) -> Image.Image:
    alpha = img.getchannel("A")
    rgb = img.convert("RGB").quantize(colors=colors, method=Image.Quantize.MEDIANCUT)
    out = rgb.convert("RGBA")
    out.putalpha(alpha)
    return out


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("input", type=Path)
    parser.add_argument("kind", choices=sorted(KINDS))
    parser.add_argument("id", nargs="?", help="id from the RON files (ghost, decor, product)")
    parser.add_argument("--remove-bg", action="store_true", help="make a plain background transparent")
    parser.add_argument("--threshold", type=int, default=40, help="colour tolerance for --remove-bg")
    parser.add_argument("--colors", type=int, help="reduce to this many colours")
    parser.add_argument("--out", type=Path, help="write here instead of assets/sprites/...")
    args = parser.parse_args()

    width, height, folder, is_cover = KINDS[args.kind]
    if args.kind in NAMED:
        name = args.kind
    elif args.id:
        name = args.id
    else:
        parser.error(f"{args.kind} needs an id, e.g. 'shy' or 'candle'")

    img = Image.open(args.input).convert("RGBA")
    if is_cover:
        img = cover(img, width, height)
    else:
        if args.remove_bg:
            img = remove_background(img, args.threshold)
        img = fit(trim(img), width, height)
    if args.colors:
        img = reduce_colors(img, args.colors)

    out = args.out or ROOT / "assets" / "sprites" / folder / f"{name}.png"
    out.parent.mkdir(parents=True, exist_ok=True)
    img.save(out, optimize=True)
    print(f"wrote {out.relative_to(ROOT) if out.is_relative_to(ROOT) else out} ({width}x{height})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
