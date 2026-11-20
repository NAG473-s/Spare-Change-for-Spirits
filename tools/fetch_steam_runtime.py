#!/usr/bin/env python3
"""Fetch a desktop platform's Steam redistributable from the pinned crate."""

import argparse
import hashlib
import io
from pathlib import Path
import sys
import tarfile
from urllib.request import urlopen

VERSION = "0.13.0"
SHA256 = "ae139f051204a4af015d4f0ed3a1f7a51020d03a44c7cf249168c543d88131e9"
LIBRARIES = {
    "windows": "win64/steam_api64.dll",
    "linux": "linux64/libsteam_api.so",
    "macos-universal": "osx/libsteam_api.dylib",
}


def fetch(destination, library):
    crate = f"steamworks-sys-{VERSION}"
    with urlopen(f"https://static.crates.io/crates/steamworks-sys/{crate}.crate", timeout=90) as response:
        payload = response.read()
    if hashlib.sha256(payload).hexdigest() != SHA256:
        raise ValueError(f"{crate} does not match its checksum")
    with tarfile.open(fileobj=io.BytesIO(payload), mode="r:gz") as archive:
        member = archive.getmember(f"{crate}/lib/steam/redistributable_bin/{library}")
        if not member.isfile():
            raise ValueError(f"expected regular file: {member.name}")
        destination.mkdir(parents=True, exist_ok=True)
        with archive.extractfile(member) as source:
            (destination / Path(library).name).write_bytes(source.read())


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("destination", type=Path)
    parser.add_argument("--platform", choices=LIBRARIES,
                        default={"win32": "windows", "darwin": "macos-universal"}.get(sys.platform, sys.platform))
    args = parser.parse_args()
    fetch(args.destination, LIBRARIES[args.platform])
