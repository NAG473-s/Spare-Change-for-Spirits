#!/usr/bin/env python3
"""Run both packaged desktops against a single fresh profile, outside the checkout."""
import argparse
import os
from pathlib import Path
import subprocess
import tarfile
from tempfile import TemporaryDirectory

from package_ci import BINARY, GAME


def check(platform, dist, runner=None, architecture=None):
    with TemporaryDirectory() as tmp:
        work = Path(tmp).resolve()
        profile = work / "save"
        profile.mkdir()
        (profile / ".handoff-check").touch()
        env = os.environ.copy()
        for key in ("BEVY_ASSET_ROOT", "CARGO_MANIFEST_DIR"):
            env.pop(key, None)
        env["SCFS_SAVE_DIR"] = str(profile)
        if runner:
            env["SCFS_SAVE_DIR"] = subprocess.check_output(
                ["winepath", "-w", str(profile)], text=True
            ).strip()
        for edition in ("demo", "full"):
            source = dist.resolve() / edition / platform
            unpack = work / edition
            unpack.mkdir()
            if platform == "linux":
                archive, = source.glob("*.tar.gz")
                with tarfile.open(archive) as tar:
                    tar.extractall(unpack, filter="data")
                binary = unpack / GAME / BINARY
            elif platform == "macos-universal":
                archive, = source.glob("*.zip")
                subprocess.run(["ditto", "-x", "-k", archive, unpack], check=True)
                binary, = unpack.glob(f"*.app/Contents/MacOS/{BINARY}")
            else:
                binary = source / f"{GAME}.exe"
            prefix = ["arch", architecture] if architecture else ([runner] if runner else [])
            command = [*prefix, str(binary), "--check-demo-handoff"]
            subprocess.run(command, cwd=work, env=env, check=True, timeout=180)
            cpu = architecture.lstrip("-") if architecture else "native"
            print(f"{platform} ({cpu}) packaged {edition} handoff check passed", flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("platform", choices=["windows", "linux", "macos-universal"])
    parser.add_argument("--dist", type=Path, default=Path("dist"))
    parser.add_argument("--runner")
    args = parser.parse_args()
    check(args.platform, args.dist, args.runner)
    if args.platform == "macos-universal" and os.uname().machine == "arm64":
        if subprocess.run(["arch", "-x86_64", "/usr/bin/true"], capture_output=True).returncode == 0:
            check(args.platform, args.dist, architecture="-x86_64")
        else:
            print("Rosetta unavailable on this runner. Both slices verified by lipo, arm64 executed.")
