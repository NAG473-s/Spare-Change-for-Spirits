#!/usr/bin/env python3
"""Checks the 3D models in assets/models/ against docs/MODEL_PROMPTS.md
and lists which ghost sprites (docs/ART_PROMPTS.md) are still missing.

For every .glb it reports the size (width × height × depth in metres),
where the origin sits and the triangle count, and flags models that are
much bigger or smaller than their target size, not standing on their
origin, or over the polygon budget. Checks required portraits against the
documented 176 × 264 PNG size, and lists missing assets. No extra packages needed.

    python3 tools/check_models.py
"""

import json
import re
import struct
import sys
from pathlib import Path

from i18n_sync import read_catalog

ROOT = Path(__import__('os').environ.get('SCFS_ROOT', Path(__file__).resolve().parent.parent))
# Activities a ghost can have a drawn sheet for (src/ghosts/motion.rs, Act::file), in the order
# they are worth ordering: the ones seen most often first.
GESTURES = ("pay", "ascend", "impatient", "goodbye", "browse", "arrive", "approach", "flee")
MODELS = ROOT / "assets" / "models"

PRODUCT_SIZE = (0.25, 0.25, 0.25)
SHOP_SIZES = {"counter": (4.5, 1.2, 0.81), "door": (1.1, 2.3, 0.2), "postcard_stand": (0.2, 0.04, 0.07)}
TRIANGLE_BUDGET = {
    "products": 1000,
    "decor": 3500,
    "shop": 8000,
    "harbor": 6000,
    "props": 3000,
    "critters": 2000,
    "market": 6000,
    "incidents": 1500,
    "errands": 1500,
}
# The village market (src/market.rs): a stall and a trader for each vendor in
# assets/data/market.ron, and the Reaper's hourglass.
MARKET_VENDORS = ["carpenter", "candlemaker", "herbalist", "glazier", "weaver", "raven", "reaper",
                  "frost", "florist"]
MARKET_SIZES = {
    **{f"stall_{v}": (1.9, 2.5, 1.2) for v in MARKET_VENDORS},
    **{f"vendor_{v}": (0.7, 1.7, 0.6) for v in MARKET_VENDORS if v not in ("raven", "candlemaker")},
    "vendor_raven": (0.4, 1.4, 0.4),
    "vendor_candlemaker": (0.5, 1.6, 0.5),
    "hourglass": (0.2, 0.3, 0.2),
    "plan": (0.4, 0.12, 0.12),
    "clock_tower": (1.2, 4.2, 1.2),
    "clock_hand_hour": (0.06, 0.22, 0.02),
    "clock_hand_minute": (0.04, 0.32, 0.02),
    "window_flowers": (0.7, 0.35, 0.2),
    "special_tag": (0.14, 0.1, 0.02),
}
# Old junk on the floor that is not cleared yet (src/upgrades.rs, JUNK_SIZE).
JUNK_SIZES = {f"junk_{n}": (0.85, 0.75, 0.85) for n in (1, 2, 3)}


def upgrade_sizes():
    """Models of the shop improvements in assets/data/upgrades.ron that have one."""
    text = (ROOT / "assets" / "data" / "upgrades.ron").read_text(encoding="utf-8")
    sizes = {}
    for chunk in text.split('(id: "')[1:]:
        uid = chunk.split('"', 1)[0]
        m = re.search(r"model: Some\(\(.*?size: \(([^)]*)\)", chunk, re.S)
        if m:
            sizes[uid] = tuple(float(v) for v in m.group(1).split(","))
    return sizes
# Containers on the ferryman's boat besides the crate (src/ferry.rs, CONTAINER_SPECS).
CONTAINER_SIZES = {
    "chest": (0.5, 0.28, 0.36),
    "chest_lid": (0.52, 0.06, 0.38),
    "basket": (0.44, 0.24, 0.44),
    "basket_lid": (0.46, 0.05, 0.46),
    "keg": (0.38, 0.34, 0.38),
    "keg_lid": (0.4, 0.04, 0.4),
}
# Animals (src/critters.rs); moving parts have their pivot as origin.
CRITTER_SIZES = {
    "cat_sleeping": (0.42, 0.2, 0.32),
    "cat_sitting": (0.28, 0.42, 0.26),
    "cat_tail": (0.06, 0.32, 0.08),
    "seagull": (0.22, 0.3, 0.36),
    "owl_body": (0.24, 0.3, 0.22),
    "owl_head": (0.2, 0.18, 0.18),
    "bat_body": (0.07, 0.06, 0.12),
    "bat_wing": (0.12, 0.02, 0.08),
    "fish": (0.08, 0.1, 0.26),
    "dog_body": (0.5, 0.36, 0.24),
    "dog_tail": (0.06, 0.16, 0.06),
    "butterfly_body": (0.03, 0.03, 0.08),
    "butterfly_wing": (0.12, 0.01, 0.1),
}
# The harbour scene (docs/MODEL_PROMPTS.md).
HARBOR_SIZES = {
    "harbour_hut": (2.6, 2.9, 2.1),
    "lighthouse": (1.2, 4.3, 1.2),
    "lamp_post": (0.4, 2.7, 0.4),
    "quay_lantern": (0.3, 2.2, 0.3),
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
    # Four different houses for the village; `village_house` is the shared fallback.
    "village_house_1": (2.2, 2.4, 1.8),
    "village_house_2": (2.2, 2.4, 1.8),
    "village_house_3": (2.2, 2.4, 1.8),
    "village_house_4": (2.2, 2.4, 1.8),
}

# Bounds that meet the origin in glTF coordinates. None denotes an anchor
# inside the bounds, such as the board centre of a hanging sign.
BOTTOM_CENTRE = ("centre", "minimum", "centre")
ORIGIN_AXES = {
    "room": (None, None, None),
    "wall_clock": ("centre", "centre", "minimum"),
    "way_board": ("centre", "centre", "minimum"),
    "shop_sign": ("centre", None, "centre"),
    "ferry_boat": ("centre", None, "centre"),
    "crate_lid": ("centre", None, "minimum"),
    "chest_lid": ("centre", None, "minimum"),
    "basket_lid": ("centre", None, "minimum"),
    "keg_lid": ("centre", None, "minimum"),
    "cat_tail": ("centre", "maximum", "centre"),
    "bat_body": ("centre", "centre", "centre"),
    "bat_wing": ("minimum", "centre", "centre"),
    "butterfly_body": ("centre", "centre", "centre"),
    "butterfly_wing": ("minimum", "centre", "centre"),
    "fish": ("centre", "centre", "centre"),
    "bell_post": (None, "minimum", "centre"),
    "ferry_bell": ("centre", "maximum", "centre"),
    "hand_cart": (None, "minimum", "centre"),
    "signpost": (None, "minimum", "centre"),
    "jetty": ("centre", "maximum", "centre"),
}


def prop_sizes():
    """Every model id in assets/data/props.ron with its size (first entry wins)."""
    text = (ROOT / "assets/data/props.ron").read_text(encoding="utf-8")
    sizes = {}
    for m in re.finditer(r'id: "([^"]+)".*?size: \(([^)]*)\)', text, re.S):
        sizes.setdefault(m.group(1), tuple(float(v) for v in m.group(2).split(",")))
    return sizes


def ids(path, pattern):
    return re.findall(pattern, (ROOT / path).read_text(encoding="utf-8"))


def check_ending_reading(path, language, line):
    """Check the Vorbis stream, its final playback time and its recorded script."""
    if not path.is_file():
        return "missing"
    data = path.read_bytes()
    offset = 0
    first_packet = b""
    last_page = b""
    while offset < len(data):
        header = data[offset:offset + 27]
        if len(header) != 27 or header[:5] != b"OggS\x00":
            return "invalid Ogg page"
        lacing = data[offset + 27:offset + 27 + header[26]]
        payload = offset + 27 + len(lacing)
        end = payload + sum(lacing)
        if len(lacing) != header[26] or end > len(data):
            return "truncated Ogg page"
        if offset == 0:
            first_packet = data[payload:end]
        last_page = header
        offset = end
    if len(first_packet) != 30 or not first_packet.startswith(b"\x01vorbis"):
        return "missing Vorbis identification"
    rate = int.from_bytes(first_packet[12:16], "little")
    samples = int.from_bytes(last_page[6:14], "little", signed=True)
    if not last_page[5] & 4 or not rate or samples < rate:
        return "incomplete or empty reading"
    # Vorbis comments are length-prefixed, which also distinguishes regional language codes.
    comments = data.partition(b"\x03vorbis")[2].partition(b"\x05vorbis")[0]
    for tag in (f"language={language}", f"lyrics={line}"):
        encoded = tag.encode("utf-8")
        if struct.pack("<I", len(encoded)) + encoded not in comments:
            return "language or script mismatch"
    return None


def incident_sizes():
    """Mishaps during the night (assets/data/incidents.ron) with their sizes."""
    text = (ROOT / "assets/data/incidents.ron").read_text(encoding="utf-8")
    out = {}
    for m in re.finditer(r'id: "([^"]+)".*?size: \(([\d.]+), ([\d.]+), ([\d.]+)\)', text, re.S):
        out[m.group(1)] = tuple(float(v) for v in m.group(2, 3, 4))
    return out


# Clues of the lost-and-found errands (assets/data/errands.ron, src/errands.rs):
# tracks lying on the ground, origin at their centre on the ground.
ERRAND_CLUE_SIZE = (1.0, 0.08, 0.5)


def errand_sizes():
    """One clue model per clue of each errand: <errand>_clue_<n>."""
    text = (ROOT / "assets/data/errands.ron").read_text(encoding="utf-8")
    out = {}
    for m in re.finditer(r'\(id: "([^"]+)".*?clues: \[(.*?)\]', text, re.S):
        for n in range(1, m.group(2).count("place:") + 1):
            out[f"{m.group(1)}_clue_{n}"] = ERRAND_CLUE_SIZE
    return out


def decor_sizes():
    text = (ROOT / "assets/data/decor.ron").read_text(encoding="utf-8")
    out = {}
    for m in re.finditer(r'id: "([^"]+)".*?size: \(([\d.]+), ([\d.]+), ([\d.]+)\)', text, re.S):
        out[m.group(1)] = tuple(float(v) for v in m.group(2, 3, 4))
    return out


def read_glb(path):
    data = path.read_bytes()
    magic, version, length = struct.unpack_from("<4sII", data, 0)
    if magic != b"glTF":
        raise ValueError("not a binary glTF (.glb) file")
    if version != 2 or length != len(data):
        raise ValueError("invalid GLB version or file length")
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
        lo, hi, tris = bounds_and_triangles(gltf)
    except Exception as e:  # noqa: BLE001 - report any broken file
        return f"{path.relative_to(ROOT)}: cannot read ({e})", [str(e)]
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
        # A small critter can fit entirely inside the furniture's origin tolerance.
        tolerance = min(0.05 if i == 1 else 0.15, max(0.001, 0.15 * size[i]))
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


def portrait_problem(path):
    with path.open("rb") as file:
        header = file.read(33)
    if len(header) != 33 or not header.startswith(b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR"):
        return "invalid PNG header"
    width, height = struct.unpack_from(">II", header, 16)
    if (width, height) != (176, 264):
        return f"{width} × {height} px, expected 176 × 264 px"
    return None


def main():
    expected = {
        "decor": decor_sizes(),
        "products": {i: PRODUCT_SIZE for i in ids("assets/data/products.ron", r'id: "([^"]+)"')},
        "shop": dict(
            SHOP_SIZES,
            room=None,
            crate=(0.5, 0.3, 0.42),
            ferryman=(0.7, 1.45, 0.6),
            ferry_boat=(7.4, 0.66, 1.85),
            crate_lid=(0.52, 0.05, 0.44),
            wall_clock=(0.78, 0.78, 0.08),
            shop_sign=(1.0, 0.75, 0.05),
            way_board=(0.8, 0.24, 0.05),
            **CONTAINER_SIZES,
            **JUNK_SIZES,
            **upgrade_sizes(),
        ),
        "harbor": HARBOR_SIZES,
        "props": prop_sizes(),
        "critters": CRITTER_SIZES,
        "market": MARKET_SIZES,
        "incidents": incident_sizes(),
        "errands": errand_sizes(),
    }
    failures = 0
    for folder, targets in expected.items():
        for model_id, target in targets.items():
            path = MODELS / folder / f"{model_id}.glb"
            if not path.is_file():
                print(f"  missing  models/{folder}/{model_id}.glb")
                failures += 1
                continue
            line, problems = check(path, target, TRIANGLE_BUDGET[folder])
            print(("  ok       " if not problems else "  CHECK    ") + line)
            for p in problems:
                print(f"           - {p}")
            failures += bool(problems)
        for extra in sorted((MODELS / folder).glob("*.glb")) if (MODELS / folder).is_dir() else []:
            if extra.stem not in targets:
                print(f"  unknown  {extra.relative_to(ROOT)} (no id with this name)")
    # Required portraits, interface art and scene textures.
    print()
    customers = ids("assets/data/ghosts.ron", r'\(id: "([^"]+)"')
    for ghost in [*customers, "apprentice"]:
        sprite = ROOT / "assets" / "sprites" / "ghosts" / f"{ghost}.png"
        if not sprite.is_file():
            print(f"  missing  sprites/ghosts/{ghost}.png")
            failures += 1
            continue
        problem = portrait_problem(sprite)
        if problem:
            print(f"  CHECK    sprites/ghosts/{ghost}.png: {problem}")
            failures += 1
        else:
            print(f"  ok       sprites/ghosts/{ghost}.png")
    # Drawn idle and gesture sheets are optional: without one the portrait performs it.
    print()
    for ghost in customers:
        for gesture in ("sheet", *GESTURES):
            sheet = ROOT / "assets" / "sprites" / "ghosts" / f"{ghost}_{gesture}.png"
            print(("  ok       " if sheet.is_file() else "  no sheet ") + f"sprites/ghosts/{ghost}_{gesture}.png")
    for name in (
        "shop/floor.png", "market/square.png", "market/cobbles.png", "harbor/quay_stone.png", "fx/glow.png",
        "title/background.png", "title/intro_harbor.png", "title/intro_ferry.png", "title/intro_crew.png", "title/logo.png",
        "shop/sign_paint.png", "shop/porthole_light.png", "ui/ledger_frame.png",
        "coins/button.png", "coins/candle.png", "coins/spoon.png",
    ):
        path = ROOT / "assets" / "sprites" / name
        present = path.is_file()
        print(("  ok       " if present else "  missing  ") + f"sprites/{name}")
        failures += not present
    # Button icons (optional: a button without one shows only its text).
    print()
    for folder in ("products", "decor"):
        for item in expected[folder]:
            icon = ROOT / "assets" / "sprites" / folder / f"{item}.png"
            print(("  ok       " if icon.is_file() else "  no icon  ") + f"sprites/{folder}/{item}.png")
    # Postcard pictures from regulars (optional: the portrait stands in).
    ghosts_text = (ROOT / "assets/data/ghosts.ron").read_text(encoding="utf-8")
    for ghost, weight in re.findall(r'\(id: "([^"]+)", color: \([^)]*\),[^\n]*\n\s*spawn_weight: ([0-9.]+)', ghosts_text):
        if float(weight) > 0 and ghost != "shade":
            card = ROOT / "assets" / "sprites" / "postcards" / f"{ghost}.png"
            print(("  ok       " if card.is_file() else "  no card  ") + f"sprites/postcards/{ghost}.png")
    # Achievement icons for Steam (docs/STEAM.md), achieved and greyed.
    for a in ids("assets/data/achievements.ron", r'\(id: "([^"]+)"'):
        for suffix in ("", "_locked"):
            icon = ROOT / "art" / "steam" / "achievements" / f"{a}{suffix}.png"
            print(("  ok       " if icon.is_file() else "  no icon  ") + str(icon.relative_to(ROOT)))
    for ext in ("png", "icns", "ico"):
        icon = ROOT / "art" / "icons" / f"spare-change-for-spirits.{ext}"
        present = icon.is_file()
        print(("  ok       " if present else "  no icon  ") + str(icon.relative_to(ROOT)))
        failures += not present
    # The register has a sound for each coin; the crossing has a score and one reading per language.
    print()
    languages = ids("assets/i18n/languages.ron", r'code: "([^"]+)"')
    for name in ["coin_button", "coin_candle", "coin_spoon", "intro_music", *(f"intro_voice_{code}" for code in languages)]:
        clip = ROOT / "assets" / "sounds" / f"{name}.ogg"
        present = clip.is_file()
        print(("  ok       " if present else "  missing  ") + f"sounds/{clip.name}")
        failures += not present
    stories = ids("assets/data/story.ron", r'ghost: "([^"]+)"')
    catalogs = {code: read_catalog(ROOT / "assets/i18n" / f"{code}.ron") for code in languages}
    for ghost in stories:
        video = Path("videos/endings") / f"{ghost}.avi"
        path = ROOT / "assets" / video
        header = b""
        if path.is_file():
            with path.open("rb") as stream:
                header = stream.read(12)
        valid = (
            header[:4] == b"RIFF" and header[8:12] == b"AVI "
            and int.from_bytes(header[4:8], "little") + 8 == path.stat().st_size
        )
        print(("  ok       " if valid else "  missing or invalid  ") + video.as_posix())
        failures += not valid
        matches = 0
        for language, catalog in catalogs.items():
            path = Path("sounds/endings") / ghost / f"{language}.ogg"
            key = f"story.{ghost}.ending"
            error = "missing catalog text"
            if key in catalog:
                line = json.loads('"' + catalog[key] + '"')
                error = check_ending_reading(ROOT / "assets" / path, language, line)
            if error:
                print(f"  {error}  {path.as_posix()}")
            else:
                matches += 1
        failures += len(languages) - matches
        print(f"  {matches}/{len(languages)} ending readings for {ghost}")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
