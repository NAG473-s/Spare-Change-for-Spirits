#!/usr/bin/env python3
"""Prepare a generated image (e.g. from ChatGPT) for the game.

Trims empty transparent borders, fits the image into the aspect ratio the
game draws it at (padding with transparency, never stretching), scales it
and writes it to the right file under assets/sprites/.

    python3 tools/prepare_sprite.py INPUT KIND [ID] [--remove-bg] [--colors N]

KIND is one of: ghost, ghost_sheet, decor, product, door, counter, floor,
background. ID is the id from the RON data files (needed for ghost,
ghost_sheet, decor, product).

ghost_sheet turns an animation drawn as a grid (default 2×2 on a 1024 × 1536
canvas, frames read row by row) into assets/sprites/ghosts/<id>_sheet.png:
all frames are trimmed with the same box, so the ghost does not jitter, and
placed side by side. With --gesture NAME it writes the drawn sheet of one
activity instead, assets/sprites/ghosts/<id>_<NAME>.png (see GESTURES).
"""

import argparse

# Activities a ghost can have a drawn sheet for (src/ghosts/motion.rs, Act::file).
GESTURES = ("arrive", "browse", "approach", "impatient", "pay", "goodbye", "flee", "ascend", "salute")
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
    "ghost_sheet": (176, 264, "ghosts", False),  # size of one frame
    "decor": (96, 96, "decor", False),  # UI icon
    "product": (96, 96, "products", False),
    "door": (256, 384, "shop", False),
    "counter": (192, 1088, "shop", False),
    "floor": (512, 512, "shop", True),
    "background": (1920, 1080, "title", True),
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
    out.paste(img, ((width - size[0]) // 2, height - size[1]))
    return out


def cover(img: Image.Image, width: int, height: int) -> Image.Image:
    """Scales to fill width x height and crops the overflow evenly."""
    scale = max(width / img.width, height / img.height)
    size = (round(img.width * scale), round(img.height * scale))
    img = img.resize(size, Image.LANCZOS)
    left = (size[0] - width) // 2
    top = (size[1] - height) // 2
    return img.crop((left, top, left + width, top + height))


def split_grid(img: Image.Image, cols: int, rows: int) -> list:
    w, h = img.width // cols, img.height // rows
    return [img.crop((c * w, r * h, (c + 1) * w, (r + 1) * h)) for r in range(rows) for c in range(cols)]


def sheet(frames: list, width: int, height: int, remove_bg: bool, threshold: int) -> Image.Image:
    if remove_bg:
        frames = [remove_background(f, threshold) for f in frames]
    # One common box for all frames keeps the ghost in place between frames.
    boxes = [f.getchannel("A").point(lambda a: 255 if a > 8 else 0).getbbox() for f in frames]
    boxes = [b for b in boxes if b]
    if boxes:
        box = (min(b[0] for b in boxes), min(b[1] for b in boxes), max(b[2] for b in boxes), max(b[3] for b in boxes))
        frames = [f.crop(box) for f in frames]
    out = Image.new("RGBA", (width * len(frames), height), (0, 0, 0, 0))
    for i, f in enumerate(frames):
        out.paste(fit(f, width, height), (i * width, 0))
    return out


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
    parser.add_argument("--grid", default="2x2", help="ghost_sheet: COLSxROWS of the frame grid (default 2x2)")
    parser.add_argument("--gesture", choices=GESTURES, help="ghost_sheet: the activity this sheet shows")
    args = parser.parse_args()

    width, height, folder, is_cover = KINDS[args.kind]
    if args.kind in NAMED:
        name = args.kind
    elif args.id:
        name = args.id
    else:
        parser.error(f"{args.kind} needs an id, e.g. 'shy' or 'candle'")

    img = Image.open(args.input).convert("RGBA")
    if args.kind == "ghost_sheet":
        cols, rows = (int(v) for v in args.grid.lower().split("x"))
        img = sheet(split_grid(img, cols, rows), width, height, args.remove_bg, args.threshold)
        width *= cols * rows
    elif is_cover:
        img = cover(img, width, height)
    else:
        if args.remove_bg:
            img = remove_background(img, args.threshold)
        img = fit(trim(img), width, height)
    if args.colors:
        img = reduce_colors(img, args.colors)

    if args.gesture and args.kind != "ghost_sheet":
        parser.error("--gesture only works with ghost_sheet")
    suffix = f"_{args.gesture or 'sheet'}" if args.kind == "ghost_sheet" else ""
    out = args.out or ROOT / "assets" / "sprites" / folder / f"{name}{suffix}.png"
    out.parent.mkdir(parents=True, exist_ok=True)
    img.save(out, optimize=True)
    print(f"wrote {out.relative_to(ROOT) if out.is_relative_to(ROOT) else out} ({width}x{height})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
