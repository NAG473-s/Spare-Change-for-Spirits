#!/usr/bin/env python3
"""Package one CI-built desktop edition and run the staged game outside the checkout."""

import argparse
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
from tempfile import TemporaryDirectory

ROOT = Path(__file__).resolve().parents[1]
GAME = "SpareChangeForSpirits"
BINARY = "spare_change_for_spirits"


def stage_desktop(package, binary, library, windows):
    package.mkdir(parents=True, exist_ok=True)
    executable = package / (f"{GAME}.exe" if windows else BINARY)
    shutil.copy2(binary, executable)
    shutil.copy2(library, package)
    shutil.copy2(ROOT / "LICENSE", package / ("LICENSE.txt" if windows else "LICENSE"))
    shutil.copy2(ROOT / "THIRD_PARTY_LICENSES.txt", package)
    shutil.copytree(ROOT / "assets", package / "assets", ignore=shutil.ignore_patterns(".gitkeep"))
    if not windows:
        shutil.copy2(ROOT / "packaging/linux/spare-change-for-spirits.desktop", package)
        shutil.copy2(ROOT / "art/icons/spare-change-for-spirits.png", package)
    return executable


def archive_mac(binary, library, archive, unpack):
    for path in (binary, library):
        subprocess.run(["lipo", path, "-verify_arch", "x86_64", "arm64"], check=True)
    shutil.copy2(library, binary.parent)
    app = subprocess.check_output(
        [sys.executable, ROOT / "tools/package_macos.py", binary], text=True
    ).strip()
    subprocess.run(["ditto", "-c", "-k", "--keepParent", app, archive], check=True)
    subprocess.run(["ditto", "-x", "-k", archive, unpack], check=True)
    bundle = unpack / Path(app).name
    subprocess.run(["plutil", "-lint", bundle / "Contents/Info.plist"], check=True)
    subprocess.run(["codesign", "--verify", "--deep", "--strict", bundle], check=True)
    subprocess.run(["lipo", bundle / "Contents/Frameworks" / library.name,
                    "-verify_arch", "x86_64", "arm64"], check=True)
    executable = bundle / "Contents/MacOS" / BINARY
    subprocess.run(["lipo", executable, "-verify_arch", "x86_64", "arm64"], check=True)
    if not (executable.parent / "assets").is_symlink():
        raise ValueError("the macOS app archive lost its assets symlink")
    return executable


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("platform", choices=["windows", "linux", "macos-universal"])
    parser.add_argument("edition", choices=["full", "demo"])
    parser.add_argument("steam_directory", type=Path)
    args = parser.parse_args()

    windows = args.platform == "windows"
    filename = BINARY + (".exe" if windows else "")
    binary = (Path("editions") / args.edition / filename).resolve()
    output = Path("dist").resolve() / args.edition
    output.mkdir(parents=True, exist_ok=True)
    name = f"{GAME}-{args.platform}" + ("-demo" if args.edition == "demo" else "")

    with TemporaryDirectory() as temporary:
        work = Path(temporary)
        unpack = work / "unpacked"
        unpack.mkdir()
        if args.platform == "macos-universal":
            executable = archive_mac(binary, args.steam_directory / "libsteam_api.dylib",
                                     output / f"{name}.zip", unpack)
        elif windows:
            # Preserve the existing Windows artifact layout at the ZIP root.
            executable = stage_desktop(output, binary, args.steam_directory / "steam_api64.dll", True)
        else:
            package = work / GAME
            stage_desktop(package, binary, args.steam_directory / "libsteam_api.so", False)
            archive = output / f"{name}.tar.gz"
            with tarfile.open(archive, "w:gz") as tar:
                tar.add(package, arcname=GAME)
            with tarfile.open(archive, "r:gz") as tar:
                tar.extractall(unpack, filter="data")
            executable = unpack / GAME / BINARY

        # Use the assets shipped beside the executable, exactly as a player would.
        env = os.environ.copy()
        env.pop("BEVY_ASSET_ROOT", None)
        env.pop("CARGO_MANIFEST_DIR", None)
        subprocess.run([executable, "--simulate", "1"], check=True, cwd=work, env=env)


if __name__ == "__main__":
    main()
