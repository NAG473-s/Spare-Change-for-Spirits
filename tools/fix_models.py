#!/usr/bin/env python3
"""Repairs .glb files that contain more than the one scene the game loads.

Exporting from a Blender file with several scenes writes every scene into
the .glb, most of them empty; Bevy refuses such files ("missing field
`nodes`") and always loads scene 0. This keeps only scenes that contain
nodes, puts the default scene first and rewrites the file in place.

    python3 tools/fix_models.py                 # every model in assets/models
    python3 tools/fix_models.py path/to/x.glb   # specific files
"""

import json
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
JSON_CHUNK = 0x4E4F534A


def read(path):
    data = path.read_bytes()
    magic, version, _ = struct.unpack_from("<4sII", data, 0)
    if magic != b"glTF":
        raise ValueError("not a .glb file")
    length, kind = struct.unpack_from("<II", data, 12)
    if kind != JSON_CHUNK:
        raise ValueError("first chunk is not JSON")
    gltf = json.loads(data[20 : 20 + length])
    rest = data[20 + length :]  # binary chunk(s), unchanged
    return version, gltf, rest


def write(path, version, gltf, rest):
    text = json.dumps(gltf, separators=(",", ":")).encode("utf-8")
    text += b" " * (-len(text) % 4)  # chunks are 4-byte aligned, JSON padded with spaces
    body = struct.pack("<II", len(text), JSON_CHUNK) + text + rest
    path.write_bytes(struct.pack("<4sII", b"glTF", version, 12 + len(body)) + body)


def fix(path):
    version, gltf, rest = read(path)
    scenes = gltf.get("scenes", [])
    default = gltf.get("scene", 0)
    keep = [s for s in scenes if s.get("nodes")]
    if not keep:
        return "no scene with content – left unchanged"
    main = scenes[default] if default < len(scenes) and scenes[default].get("nodes") else keep[0]
    new = [main] + [s for s in keep if s is not main]
    if new == scenes and default == 0:
        return None
    gltf["scenes"] = new
    gltf["scene"] = 0
    write(path, version, gltf, rest)
    return f"kept {len(new)} of {len(scenes)} scenes, '{main.get('name', '?')}' first"


def main():
    paths = [Path(p) for p in sys.argv[1:]] or sorted((ROOT / "assets" / "models").rglob("*.glb"))
    for p in paths:
        try:
            result = fix(p)
        except Exception as e:  # noqa: BLE001 - report and go on
            result = f"cannot fix: {e}"
        if result:
            print(f"{p}: {result}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
