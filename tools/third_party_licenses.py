#!/usr/bin/env python3
"""Writes THIRD_PARTY_LICENSES.txt: every Rust library compiled into the
Windows build, its licence, and the licence texts shipped with the crates
(identical texts are printed once, with the crates that use them).

    python3 tools/third_party_licenses.py

Run it again after changing dependencies (Cargo.lock) and commit the result;
the Windows build puts the file next to the game.
"""

import json
import re
import subprocess
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "THIRD_PARTY_LICENSES.txt"
TARGET = "x86_64-pc-windows-msvc"
FILES = re.compile(r"^(LICEN[CS]E|COPYING|NOTICE|UNLICENSE)", re.IGNORECASE)


def main() -> int:
    meta = json.loads(
        subprocess.run(
            ["cargo", "metadata", "--format-version", "1", "--filter-platform", TARGET],
            cwd=ROOT, check=True, capture_output=True, text=True,
        ).stdout
    )
    used = {node["id"] for node in meta["resolve"]["nodes"]}
    crates = sorted(
        (p for p in meta["packages"] if p["id"] in used and p["source"]),
        key=lambda p: (p["name"], p["version"]),
    )
    texts = defaultdict(list)  # licence text -> crates
    lines = []
    for p in crates:
        name = f"{p['name']} {p['version']}"
        lines.append(f"{name:<44} {p.get('license') or 'see licence file'}")
        folder = Path(p["manifest_path"]).parent
        for f in sorted(folder.iterdir()):
            if f.is_file() and FILES.match(f.name):
                text = f.read_text(encoding="utf-8", errors="replace").strip()
                texts[text].append(name)

    rule = "=" * 78
    out = [
        "Third-party software in Spare Change for Spirits",
        "",
        "The game is built with the Rust libraries below. Each is used under the",
        "licence named next to it (for a choice such as \"MIT OR Apache-2.0\", under",
        "either). The licence texts and copyright notices shipped with them follow.",
        "The fonts are listed in LICENSE and their licences are in assets/fonts/.",
        "",
        rule,
        "",
        *lines,
        "",
    ]
    for text, names in sorted(texts.items(), key=lambda kv: kv[1][0]):
        out += [rule, "Used by: " + ", ".join(names), rule, "", text, ""]
    OUT.write_text("\n".join(out) + "\n", encoding="utf-8")
    print(f"wrote {OUT.name}: {len(crates)} crates, {len(texts)} distinct licence texts, "
          f"{OUT.stat().st_size // 1024} KB")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
