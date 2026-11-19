#!/usr/bin/env python3
"""Keeps every language catalog in the same order and layout as en.ron.

Rebuilds assets/i18n/<code>.ron from en.ron's structure (comments, sections,
key order), taking each value from the existing catalog. New translations
can be merged in from JSON files named <code>.json (key -> text):

    python3 tools/i18n_sync.py                    # just re-order / re-layout
    python3 tools/i18n_sync.py --add DIR          # merge DIR/<code>.json files
    python3 tools/i18n_sync.py --missing          # list keys missing per language

Keys missing in a language stay missing (cargo test reports them); keys that
no longer exist in en.ron are dropped.
"""

import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
I18N = ROOT / "assets" / "i18n"
LINE = re.compile(r'^(\s*)"([^"]+)": "((?:[^"\\]|\\.)*)",\s*$')


def read_catalog(path):
    values = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        m = LINE.match(line)
        if m:
            values[m.group(2)] = m.group(3)  # kept escaped, exactly as in the file
    return values


def escape(text):
    return text.replace("\\", "\\\\").replace('"', '\\"')


def languages():
    text = (I18N / "languages.ron").read_text(encoding="utf-8")
    return [c for c in re.findall(r'code: "([^"]+)"', text) if c != "en"]


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--add", type=Path, help="folder with <code>.json translations to merge")
    parser.add_argument("--missing", action="store_true", help="only report missing keys")
    args = parser.parse_args()

    source_lines = (I18N / "en.ron").read_text(encoding="utf-8").splitlines()
    source_keys = [m.group(2) for m in map(LINE.match, source_lines) if m]
    problems = 0
    for code in languages():
        path = I18N / f"{code}.ron"
        values = read_catalog(path) if path.is_file() else {}
        if args.add and (args.add / f"{code}.json").is_file():
            new = json.loads((args.add / f"{code}.json").read_text(encoding="utf-8"))
            values.update({k: escape(v) for k, v in new.items() if k in source_keys})
        missing = [k for k in source_keys if k not in values]
        if missing:
            problems += 1
            print(f"{code}: {len(missing)} missing: {', '.join(missing[:6])}{' …' if len(missing) > 6 else ''}")
        if args.missing:
            continue
        out = []
        for line in source_lines:
            m = LINE.match(line)
            if not m:
                out.append(line)  # comments, braces, blank lines
            elif m.group(2) in values:
                out.append(f'{m.group(1)}"{m.group(2)}": "{values[m.group(2)]}",')
        path.write_text("\n".join(out) + "\n", encoding="utf-8")
    return 1 if (problems and args.missing) else 0


if __name__ == "__main__":
    sys.exit(main())
