#!/usr/bin/env python3
"""Builds small script fonts for languages Inter cannot draw.

For every language in assets/i18n/languages.ron with a `font`, this keeps
only the characters its catalog (and the language menu) needs, so the game
ships a few hundred KB instead of tens of MB. Run it after changing any
translation:

    python3 tools/subset_fonts.py           # write assets/fonts/*-subset.*
    python3 tools/subset_fonts.py --check   # fail if a font (or Inter) lacks a character

Needs fontTools (pip install -r tools/requirements.txt) and the Noto fonts
(Debian/Ubuntu: apt-get install fonts-noto-cjk fonts-noto-core), or pass
--source DIR with NotoSansCJK-Regular.ttc, NotoSansThai-Regular.ttf and
NotoSansArabic-Regular.ttf.
"""

import argparse
import re
import sys
import unicodedata
from pathlib import Path

from fontTools import subset
from fontTools.ttLib import TTCollection, TTFont

ROOT = Path(__file__).resolve().parent.parent
I18N = ROOT / "assets" / "i18n"
FONTS = ROOT / "assets" / "fonts"
SYSTEM_DIRS = [Path("/usr/share/fonts/opentype/noto"), Path("/usr/share/fonts/truetype/noto")]

# output file -> (source file, family name inside a collection or None)
SOURCES = {
    "NotoSansSC-subset.otf": ("NotoSansCJK-Regular.ttc", "Noto Sans CJK SC"),
    "NotoSansTC-subset.otf": ("NotoSansCJK-Regular.ttc", "Noto Sans CJK TC"),
    "NotoSansJP-subset.otf": ("NotoSansCJK-Regular.ttc", "Noto Sans CJK JP"),
    "NotoSansKR-subset.otf": ("NotoSansCJK-Regular.ttc", "Noto Sans CJK KR"),
    "NotoSansThai-subset.ttf": ("NotoSansThai-Regular.ttf", None),
    "NotoSansArabic-subset.ttf": ("NotoSansArabic-Regular.ttf", None),
}
# Always included: digits, Latin punctuation and the symbols the UI uses.
BASE = set(" 0123456789.,:;!?()[]{}-–—'\"“”‘’„«»「」『』、。…×→·−+=/%")


def languages():
    text = (I18N / "languages.ron").read_text(encoding="utf-8")
    pattern = r'\(code: "([^"]+)", name: "([^"]+)"(?:, font: Some\("([^"]+)"\))?\)'
    return re.findall(pattern, text)


def catalog_chars(code):
    text = (I18N / f"{code}.ron").read_text(encoding="utf-8")
    chars = set()
    for value in re.findall(r'^\s*"[^"]+": "((?:[^"\\]|\\.)*)",\s*$', text, re.M):
        chars |= set(value.replace('\\"', '"').replace("\\\\", "\\"))
    return chars


def find_source(name, source_dir):
    dirs = [source_dir] if source_dir else SYSTEM_DIRS
    for d in dirs:
        if (d / name).is_file():
            return d / name
    sys.exit(f"cannot find {name} (install fonts-noto-cjk/fonts-noto-core or pass --source)")


def open_font(path, family):
    if family is None:
        return TTFont(path)
    for font in TTCollection(path).fonts:
        if font["name"].getDebugName(1) == family:
            return font
    sys.exit(f"{family} not found in {path}")


def with_decompositions(chars):
    """Shapers may split a character (e.g. Thai SARA AM into NIKHAHIT + SARA AA),
    so the font must also contain the parts."""
    text = "".join(chars)
    return chars | set(unicodedata.normalize("NFD", text)) | set(unicodedata.normalize("NFKD", text))


def needed_chars(code, native_name):
    return with_decompositions(catalog_chars(code) | set(native_name) | BASE) - {"\n"}


def build(out_name, chars, source_dir):
    src_name, family = SOURCES[out_name]
    font = open_font(find_source(src_name, source_dir), family)
    options = subset.Options()
    options.layout_features = ["*"]  # Arabic and Thai need their shaping rules
    options.name_IDs = ["*"]
    options.notdef_outline = True
    options.hinting = False
    options.desubroutinize = True
    subsetter = subset.Subsetter(options)
    subsetter.populate(unicodes=[ord(c) for c in chars])
    subsetter.subset(font)
    out = FONTS / out_name
    font.save(out)
    return out


def check(out_name, chars):
    path = FONTS / out_name
    if not path.is_file():
        return [f"{out_name} is missing"]
    cmap = TTFont(path).getBestCmap()
    inter = TTFont(FONTS / "Inter-Regular.otf").getBestCmap()
    missing = sorted(c for c in chars if ord(c) not in cmap and ord(c) not in inter and not c.isspace())
    return [f"{out_name} lacks {''.join(missing)!r}"] if missing else []


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--source", type=Path)
    args = parser.parse_args()

    langs = languages()
    names = "".join(name for _, name, _ in langs)
    problems = []
    inter = TTFont(FONTS / "Inter-Regular.otf").getBestCmap()
    for code, native, font in langs:
        if not font:
            # Latin, Cyrillic and Greek: Inter has to draw every character.
            if args.check:
                chars = (catalog_chars(code) | set(native)) - {"\n"}
                missing = sorted(c for c in chars if ord(c) not in inter and not c.isspace())
                if missing:
                    problems.append(f"Inter lacks {''.join(missing)!r} for {code}")
            continue
        if not (I18N / f"{code}.ron").is_file():
            problems.append(f"{code}.ron is missing")
            continue
        # Each script font also draws the language menu's names in its script.
        chars = needed_chars(code, native) | {c for c in names if ord(c) > 0x2FF}
        if args.check:
            # Only what the language really shows; BASE is just a convenience.
            problems += check(font, with_decompositions(catalog_chars(code) | set(native)) - {"\n"})
        else:
            out = build(font, chars, args.source)
            print(f"wrote {out.relative_to(ROOT)} ({out.stat().st_size // 1024} KB, {len(chars)} chars)")
    if problems:
        print("\n".join(problems))
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
