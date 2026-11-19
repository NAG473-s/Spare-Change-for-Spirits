#!/usr/bin/env python3
"""Checks the 3D models in assets/models/ against docs/MODEL_PROMPTS.md
and lists which ghost sprites (docs/ART_PROMPTS.md) are still missing.

For every .glb it reports the size (width × height × depth in metres),
where the origin sits and the triangle count, and flags models that are
much bigger or smaller than their target size, not standing on their
origin, or over the polygon budget. Also lists which models are still
missing. No extra packages needed.

    python3 tools/check_models.py
"""

import json
import re
import struct
import sys
from pathlib import Path

ROOT = Path(__import__('os').environ.get('SCFS_ROOT', Path(__file__).resolve().parent.parent))
MODELS = ROOT / "assets" / "models"

PRODUCT_SIZE = (0.25, 0.25, 0.25)
SHOP_SIZES = {"counter": (4.5, 1.0, 0.8), "door": (1.1, 2.3, 0.2)}
TRIANGLE_BUDGET = {"products": 1000, "decor": 3500, "shop": 8000, "harbor": 6000}
# The harbour scene (docs/MODEL_PROMPTS.md).
HARBOR_SIZES = {
    "harbour_hut": (2.6, 2.9, 2.1),
    "lighthouse": (1.2, 4.3, 1.2),
    "lamp_post": (0.4, 2.7, 0.4),
    "village_house": (2.2, 2.4, 1.8),
    "barrel": (0.6, 0.72, 0.6),
    "mooring_post": (0.3, 0.6, 0.3),
    "bell_post": (0.6, 1.95, 0.2),
    "ferry_bell": (0.25, 0.22, 0.25),
    "hand_cart": (2.0, 0.75, 1.0),
    "signpost": (0.95, 1.95, 0.2),
    "rowboat": (0.8, 0.45, 2.0),
    "jetty": (1.1, 0.7, 3.8),
    "paper_lantern": (0.26, 0.34, 0.26),
}

# Bounds that meet the origin in glTF coordinates. None denotes an anchor
# inside the bounds, such as the board centre of a hanging sign.
BOTTOM_CENTRE = ("centre", "minimum", "centre")
ORIGIN_AXES = {
    "room": (None, None, None),
    "wall_clock": ("centre", "centre", "minimum"),
    "shop_sign": ("centre", None, "centre"),
    "ferry_boat": ("centre", None, "centre"),
    "crate_lid": ("centre", None, "minimum"),
    "bell_post": (None, "minimum", "centre"),
    "ferry_bell": ("centre", "maximum", "centre"),
    "hand_cart": (None, "minimum", "centre"),
    "signpost": (None, "minimum", "centre"),
    "jetty": ("centre", "maximum", "centre"),
}


def ids(path, pattern):
    return re.findall(pattern, (ROOT / path).read_text(encoding="utf-8"))


def decor_sizes():
    text = (ROOT / "assets/data/decor.ron").read_text(encoding="utf-8")
    out = {}
    for m in re.finditer(r'id: "([^"]+)".*?size: \(([\d.]+), ([\d.]+), ([\d.]+)\)', text, re.S):
        out[m.group(1)] = tuple(float(v) for v in m.group(2, 3, 4))
    return out


def read_glb(path):
    data = path.read_bytes()
    magic, _version, _length = struct.unpack_from("<4sII", data, 0)
    if magic != b"glTF":
        raise ValueError("not a binary glTF (.glb) file")
    chunk_len, chunk_type = struct.unpack_from("<II", data, 12)
    if chunk_type != 0x4E4F534A:
        raise ValueError("first chunk is not JSON")
    return json.loads(data[20 : 20 + chunk_len])


def mat_mul(a, b):
    return [[sum(a[i][k] * b[k][j] for k in range(4)) for j in range(4)] for i in range(4)]


def node_matrix(node):
    if "matrix" in node:
        m = node["matrix"]
        return [[m[c * 4 + r] for c in range(4)] for r in range(4)]
    t = node.get("translation", [0, 0, 0])
    x, y, z, w = node.get("rotation", [0, 0, 0, 1])
    s = node.get("scale", [1, 1, 1])
    r = [
        [1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
        [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
        [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)],
    ]
    return [
        [r[0][0] * s[0], r[0][1] * s[1], r[0][2] * s[2], t[0]],
        [r[1][0] * s[0], r[1][1] * s[1], r[1][2] * s[2], t[1]],
        [r[2][0] * s[0], r[2][1] * s[1], r[2][2] * s[2], t[2]],
        [0, 0, 0, 1],
    ]


def bounds_and_triangles(gltf):
    lo, hi, tris = [float("inf")] * 3, [float("-inf")] * 3, 0
    nodes = gltf.get("nodes", [])
    scene = gltf.get("scenes", [{}])[gltf.get("scene", 0)]

    def visit(index, parent):
        nonlocal tris
        node = nodes[index]
        m = mat_mul(parent, node_matrix(node))
        if "mesh" in node:
            for prim in gltf["meshes"][node["mesh"]]["primitives"]:
                pos = gltf["accessors"][prim["attributes"]["POSITION"]]
                mn, mx = pos.get("min"), pos.get("max")
                if mn and mx:
                    for cx in (mn[0], mx[0]):
                        for cy in (mn[1], mx[1]):
                            for cz in (mn[2], mx[2]):
                                p = [sum(m[r][c] * v for c, v in enumerate((cx, cy, cz, 1))) for r in range(3)]
                                for i in range(3):
                                    lo[i] = min(lo[i], p[i])
                                    hi[i] = max(hi[i], p[i])
                count = gltf["accessors"][prim["indices"]]["count"] if "indices" in prim else pos["count"]
                tris += count // 3
        for child in node.get("children", []):
            visit(child, m)

    identity = [[1 if i == j else 0 for j in range(4)] for i in range(4)]
    for root in scene.get("nodes", []):
        visit(root, identity)
    return lo, hi, tris


def check(path, target, budget):
    problems = []
    try:
        gltf = read_glb(path)
    except Exception as e:  # noqa: BLE001 - report any broken file
        return f"{path.relative_to(ROOT)}: cannot read ({e})", [str(e)]
    lo, hi, tris = bounds_and_triangles(gltf)
    if lo[0] == float("inf"):
        return f"{path.relative_to(ROOT)}: no meshes", ["no meshes"]
    size = [hi[i] - lo[i] for i in range(3)]
    line = (
        f"{path.relative_to(ROOT)}: {size[0]:.2f} × {size[1]:.2f} × {size[2]:.2f} m, "
        f"bottom at y={lo[1]:.2f}, centre x={(lo[0] + hi[0]) / 2:.2f} z={(lo[2] + hi[2]) / 2:.2f}, {tris} triangles"
    )
    for i, anchor in enumerate(ORIGIN_AXES.get(path.stem, BOTTOM_CENTRE)):
        if anchor is None:
            continue
        anchors = {"minimum": lo[i], "maximum": hi[i], "centre": (lo[i] + hi[i]) / 2}
        tolerance = 0.05 if i == 1 else 0.15
        if abs(anchors[anchor]) > tolerance:
            problems.append(f"origin should be at the {anchor} on the {'xyz'[i]} axis")
    if target:
        # Products only need to fit (some lie flat); furniture should match its size.
        lower = 0.0 if target == PRODUCT_SIZE else 0.5
        for axis, (have, want) in zip("whd", zip(size, target)):
            if want > 0.05 and not (lower * want <= have <= 1.3 * want):
                problems.append(f"{axis} is {have:.2f} m, target {want:.2f} m")
    if tris > budget:
        problems.append(f"{tris} triangles (budget {budget})")
    if not gltf.get("materials"):
        problems.append("no materials")
    scenes = gltf.get("scenes", [])
    if len(scenes) != 1 or gltf.get("scene", 0) != 0 or not scenes[0].get("nodes"):
        problems.append(
            f"{len(scenes)} scenes (default {gltf.get('scene', 0)}); the game needs exactly one "
            "– run tools/fix_models.py"
        )
    return line, problems


def main():
    expected = {
        "decor": decor_sizes(),
        "products": {i: PRODUCT_SIZE for i in ids("assets/data/products.ron", r'id: "([^"]+)"')},
        "shop": dict(
            SHOP_SIZES,
            room=None,
            crate=(0.5, 0.3, 0.42),
            ferryman=(0.7, 1.45, 0.6),
            ferry_boat=(7.4, 0.6, 1.7),
            crate_lid=(0.52, 0.05, 0.44),
            wall_clock=(0.78, 0.78, 0.08),
            shop_sign=(1.0, 0.75, 0.05),
        ),
        "harbor": HARBOR_SIZES,
    }
    failures = 0
    for folder, targets in expected.items():
        for model_id, target in targets.items():
            path = MODELS / folder / f"{model_id}.glb"
            if not path.is_file():
                print(f"  missing  models/{folder}/{model_id}.glb")
                continue
            line, problems = check(path, target, TRIANGLE_BUDGET[folder])
            print(("  ok       " if not problems else "  CHECK    ") + line)
            for p in problems:
                print(f"           - {p}")
            failures += bool(problems)
        for extra in sorted((MODELS / folder).glob("*.glb")) if (MODELS / folder).is_dir() else []:
            if extra.stem not in targets:
                print(f"  unknown  {extra.relative_to(ROOT)} (no id with this name)")
    # Images the 3D game uses: one sprite per ghost and the floor texture.
    print()
    for ghost in ids("assets/data/ghosts.ron", r'\(id: "([^"]+)"'):
        sprite = ROOT / "assets" / "sprites" / "ghosts" / f"{ghost}.png"
        print(("  ok       " if sprite.is_file() else "  missing  ") + f"sprites/ghosts/{ghost}.png")
    for name in ("shop/floor.png", "ui/ledger_frame.png", "coins/button.png", "coins/candle.png", "coins/spoon.png"):
        path = ROOT / "assets" / "sprites" / name
        print(("  ok       " if path.is_file() else "  missing  ") + f"sprites/{name}")
    # Button icons (optional: a button without one shows only its text).
    print()
    for folder in ("products", "decor"):
        for item in expected[folder]:
            icon = ROOT / "assets" / "sprites" / folder / f"{item}.png"
            print(("  ok       " if icon.is_file() else "  no icon  ") + f"sprites/{folder}/{item}.png")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
