#!/usr/bin/env python3
"""Draws simple placeholder icons for the three coins (button, candle stub,
silver spoon) into assets/sprites/coins/<coin>.png (96 x 96).

Painted versions (docs/ART_PROMPTS.md) replace them; prepare those with
`python3 tools/prepare_sprite.py <file> product <coin> --out assets/sprites/coins/<coin>.png`.
"""

from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "assets" / "sprites" / "coins"
S = 384  # drawn 4x, scaled down for smooth edges
LINE = (52, 32, 22, 255)


def finish(img: Image.Image, name: str) -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    img.resize((96, 96), Image.LANCZOS).save(OUT / f"{name}.png", optimize=True)
    print(f"wrote {OUT / name}.png")


def button() -> None:
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    d.ellipse((40, 40, S - 40, S - 40), fill=(170, 112, 62, 255), outline=LINE, width=10)
    d.ellipse((78, 78, S - 78, S - 78), fill=(196, 138, 80, 255), outline=(120, 76, 40, 255), width=8)
    d.arc((92, 92, S - 92, S - 92), 190, 260, fill=(232, 186, 128, 255), width=10)  # light from the upper left
    for x, y in [(160, 160), (224, 160), (160, 224), (224, 224)]:
        d.ellipse((x - 20, y - 20, x + 20, y + 20), fill=(70, 42, 26, 255))
    finish(img, "button")


def candle() -> None:
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    glow = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    ImageDraw.Draw(glow).ellipse((122, 20, 262, 170), fill=(255, 190, 90, 120))
    img.alpha_composite(glow.filter(ImageFilter.GaussianBlur(24)))
    d = ImageDraw.Draw(img)
    wax, shade = (238, 226, 196, 255), (206, 190, 156, 255)
    d.rounded_rectangle((116, 170, 268, 340), radius=18, fill=wax, outline=LINE, width=10)
    d.rectangle((222, 180, 258, 330), fill=shade)
    d.ellipse((116, 150, 268, 196), fill=(248, 240, 218, 255), outline=LINE, width=10)
    d.ellipse((150, 186, 182, 250), fill=wax)  # wax drip
    d.line((192, 172, 192, 138), fill=LINE, width=8)  # wick
    d.polygon([(192, 52), (222, 118), (192, 146), (162, 118)], fill=(255, 176, 60, 255), outline=LINE)
    d.polygon([(192, 92), (206, 122), (192, 136), (178, 122)], fill=(255, 236, 150, 255))
    finish(img, "candle")


def spoon() -> None:
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    silver, dark = (206, 212, 222, 255), (150, 158, 172, 255)
    # Handle, from lower left to the bowl at the upper right.
    d.line((70, 320, 220, 170), fill=LINE, width=46)
    d.line((70, 320, 220, 170), fill=silver, width=28)
    d.ellipse((46, 296, 104, 354), fill=silver, outline=LINE, width=9)
    # Bowl.
    bowl = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    b = ImageDraw.Draw(bowl)
    b.ellipse((150, 40, 310, 250), fill=silver, outline=LINE, width=10)
    b.ellipse((186, 80, 286, 222), fill=dark)
    b.ellipse((198, 92, 238, 150), fill=(240, 244, 250, 255))
    img.alpha_composite(bowl.rotate(-38, center=(230, 145), resample=Image.BICUBIC))
    finish(img, "spoon")


if __name__ == "__main__":
    button()
    candle()
    spoon()
