#!/usr/bin/env python3
"""Assemble the game and its Dock icon into a macOS application bundle."""

import argparse
import json
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
from tempfile import TemporaryDirectory

ROOT = Path(__file__).resolve().parents[1]
NAME = "Spare Change for Spirits"
BINARY = "spare_change_for_spirits"
ICON = "spare-change-for-spirits.icns"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", nargs="?", type=Path, help="macOS executable (defaults to the release build)")
    args = parser.parse_args()
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"], cwd=ROOT
    ))
    target = Path(metadata["target_directory"])
    binary = args.binary or target / "release" / BINARY
    if not binary.is_file():
        parser.error(f"build the game with cargo build --release first, or pass the executable: {binary}")
    version = next(p["version"] for p in metadata["packages"] if p["name"] == BINARY)
    dest = target / "macos" / f"{NAME}.app"
    dest.parent.mkdir(parents=True, exist_ok=True)
    with TemporaryDirectory(dir=dest.parent) as tmp:
        app = Path(tmp) / dest.name
        macos = app / "Contents/MacOS"
        resources = app / "Contents/Resources"
        macos.mkdir(parents=True)
        resources.mkdir()
        shutil.copy2(binary, macos / BINARY)
        shutil.copy2(ROOT / "art/icons" / ICON, resources / ICON)
        shutil.copytree(ROOT / "assets", resources / "assets", ignore=shutil.ignore_patterns(".gitkeep"))
        # The game and Bevy both look for assets beside the executable.
        (macos / "assets").symlink_to("../Resources/assets", target_is_directory=True)
        for license_name in ("LICENSE", "THIRD_PARTY_LICENSES.txt"):
            shutil.copy2(ROOT / license_name, resources)
        with (app / "Contents/Info.plist").open("wb") as plist:
            plistlib.dump({
                "CFBundleName": NAME,
                "CFBundleDisplayName": NAME,
                "CFBundleExecutable": BINARY,
                "CFBundleIdentifier": "io.github.nag473-s.SpareChangeForSpirits",
                "CFBundleInfoDictionaryVersion": "6.0",
                "CFBundleIconFile": ICON,
                "CFBundlePackageType": "APPL",
                "CFBundleVersion": version,
                "CFBundleShortVersionString": version,
                "LSApplicationCategoryType": "public.app-category.games",
                "NSHighResolutionCapable": True,
            }, plist)
        if sys.platform == "darwin":
            subprocess.run(["codesign", "--force", "--sign", "-", str(app)], check=True)
        previous = Path(tmp) / "previous.app"
        if dest.exists():
            dest.rename(previous)
        try:
            app.rename(dest)
        except OSError:
            if previous.exists():
                previous.rename(dest)
            raise
    print(dest)


if __name__ == "__main__":
    main()
