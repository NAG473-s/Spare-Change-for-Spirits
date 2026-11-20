#!/usr/bin/env python3
"""Assemble the game and its Dock icon into a macOS application bundle."""

import argparse
import json
import mmap
from pathlib import Path
import plistlib
import shutil
import struct
import subprocess
import sys
from tempfile import TemporaryDirectory

from package_assets import copy_assets

ROOT = Path(__file__).resolve().parents[1]
NAME = "Spare Change for Spirits"
BINARY = "spare_change_for_spirits"
ICON = "spare-change-for-spirits.icns"
STEAM_LIBRARY = "libsteam_api.dylib"


def publish_directory(staged, destination):
    """Replace a complete package from a temporary sibling on the same filesystem."""
    previous = staged.parent / "previous-package"
    if destination.exists():
        destination.rename(previous)
    try:
        staged.rename(destination)
    except OSError:
        if previous.exists():
            previous.rename(destination)
        raise


def macos_architectures(path, file_type):
    """Read the CPU types of a macOS Mach-O executable or dylib, including universal files."""
    with path.open("rb") as source, mmap.mmap(source.fileno(), 0, access=mmap.ACCESS_READ) as data:
        magic, count = struct.unpack_from(">II", data)
        # Universal binaries have one big-endian fat_arch or fat_arch_64 record per slice.
        if magic in (0xCAFEBABE, 0xCAFEBABF):
            record = struct.Struct(">IIIII" if magic == 0xCAFEBABE else ">IIQQII")
            if not 0 < count <= (len(data) - 8) // record.size:
                raise ValueError("invalid universal binary header")
            slices = [record.unpack_from(data, 8 + i * record.size)[2:4] for i in range(count)]
        else:
            slices = [(0, len(data))]

        architectures = set()
        for start, size in slices:
            if size < 32 or start + size > len(data):
                raise ValueError("invalid Mach-O slice")
            magic, cpu, _, kind, commands, command_bytes = struct.unpack_from("<6I", data, start)
            if magic != 0xFEEDFACF or kind != file_type:
                raise ValueError("unexpected Mach-O file type (expected a 64-bit macOS binary)")
            pos = start + 32  # sizeof(mach_header_64)
            end = pos + command_bytes
            if end > start + size:
                raise ValueError("invalid Mach-O load commands")
            macos = False
            for _ in range(commands):
                if pos + 8 > end:
                    raise ValueError("truncated Mach-O load command")
                cmd, length = struct.unpack_from("<II", data, pos)
                if length < 8 or pos + length > end:
                    raise ValueError("invalid Mach-O load command size")
                # LC_VERSION_MIN_MACOSX or LC_BUILD_VERSION with PLATFORM_MACOS.
                if cmd == 0x24 and length >= 16:
                    macos = True
                if cmd == 0x32 and length >= 24 and struct.unpack_from("<I", data, pos + 8)[0] == 1:
                    macos = True
                pos += length
            if not macos:
                raise ValueError("the binary does not target macOS")
            architectures.add(cpu)
        return architectures


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--edition", choices=["full", "demo"], default="full",
                        help="match the edition of the supplied game executable")
    parser.add_argument("binary", nargs="?", type=Path, help="macOS executable (defaults to the release build)")
    args = parser.parse_args()
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"], cwd=ROOT
    ))
    target = Path(metadata["target_directory"])
    binary = args.binary or target / "release" / BINARY
    if not binary.is_file():
        parser.error(f"build the game with cargo build --release first, or pass the executable: {binary}")
    steam_library = binary.with_name(STEAM_LIBRARY)
    try:
        arches = macos_architectures(binary, 2)  # MH_EXECUTE
        if steam_library.exists() and not arches <= macos_architectures(steam_library, 6):  # MH_DYLIB
            parser.error(f"{steam_library} is missing an architecture used by {binary}")
    except (OSError, ValueError, struct.error) as error:
        parser.error(f"cannot package {binary}: {error}")
    version = next(p["version"] for p in metadata["packages"] if p["name"] == BINARY)
    output = target / "macos"
    if args.edition == "demo":
        output = output / "demo"
    dest = output / f"{NAME}.app"
    dest.parent.mkdir(parents=True, exist_ok=True)
    with TemporaryDirectory(dir=dest.parent) as tmp:
        app = Path(tmp) / dest.name
        macos = app / "Contents/MacOS"
        resources = app / "Contents/Resources"
        macos.mkdir(parents=True)
        resources.mkdir()
        executable = macos / BINARY
        shutil.copy2(binary, executable)
        executable.chmod(0o755)
        if steam_library.exists():
            frameworks = app / "Contents/Frameworks"
            frameworks.mkdir()
            shutil.copy2(steam_library, frameworks / STEAM_LIBRARY)
        shutil.copy2(ROOT / "art/icons" / ICON, resources / ICON)
        copy_assets(ROOT / "assets", resources / "assets", args.edition)
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
        publish_directory(app, dest)
    print(dest)


if __name__ == "__main__":
    main()
