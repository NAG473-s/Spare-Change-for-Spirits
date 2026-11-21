#!/usr/bin/env python3
"""Makes the in-game achievement icons from the Steam artwork.

Reads art/steam/achievements/<id>.png and <id>_locked.png (256 x 256) and
writes assets/sprites/achievements/ at 128 x 128, which the achievements page
shows (src/achievements.rs). Run it again after replacing Steam artwork.
"""

from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "art" / "steam" / "achievements"
TARGET = ROOT / "assets" / "sprites" / "achievements"
SIZE = 128


def main() -> None:
    TARGET.mkdir(parents=True, exist_ok=True)
    for path in sorted(SOURCE.glob("*.png")):
        icon = Image.open(path).convert("RGBA").resize((SIZE, SIZE), Image.Resampling.LANCZOS)
        icon.save(TARGET / path.name, optimize=True)
        print(f"wrote {(TARGET / path.name).relative_to(ROOT)}")


if __name__ == "__main__":
    main()
