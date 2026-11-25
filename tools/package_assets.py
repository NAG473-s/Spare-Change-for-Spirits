#!/usr/bin/env python3
"""Stage a full asset tree or the playable demo subset into a fresh directory."""

import argparse
import json
from pathlib import Path
import re
import shutil

from i18n_sync import read_catalog


# Split only at the current nesting level. Keep uninspected RON values verbatim
# (enums, typed tuples, numbers, strings) when writing a subset of a table.
STRING = r'"(?:\\.|[^"\\])*"'
COMMENTS = re.compile(STRING + r'|//[^\n]*')
TOKENS = re.compile(STRING + r'|[][(){},]')


def items(value):
    value = value.strip()
    pairs = {"(": ")", "[": "]", "{": "}"}
    if not value or pairs.get(value[0]) != value[-1]:
        raise ValueError(f"Expected a RON collection: {value[:80]}")
    body = value[1:-1]
    stack = []
    start = 0
    result = []
    for match in TOKENS.finditer(body):
        token = match[0]
        if token in pairs:
            stack.append(pairs[token])
        elif token in pairs.values():
            if not stack or stack.pop() != token:
                raise ValueError(f"Unmatched delimiter near {body[match.start():][:80]}")
        elif token == "," and not stack:
            result.append(body[start:match.start()].strip())
            start = match.end()
    if stack:
        raise ValueError("Unclosed RON collection")
    if tail := body[start:].strip():
        result.append(tail)
    return result


def fields(record):
    return dict((name.strip(), value.strip())
                for name, value in (entry.split(":", 1) for entry in items(record)))


def record(fields):
    return "(" + ", ".join(f"{name}: {value}" for name, value in fields.items()) + ")"


def sequence(values):
    return "[" + ",\n".join(values) + "]"


def option(value):
    if value == "None":
        return None
    if not value.startswith("Some("):
        raise ValueError(f"Expected a RON option: {value}")
    return json.loads(value[5:-1])


class DemoAssets:
    def __init__(self, source):
        def load(table):
            text = (source / "data" / f"{table}.ron").read_text(encoding="utf-8")
            return COMMENTS.sub(lambda m: "" if m[0].startswith("//") else m[0], text).strip()

        def rows(table):
            return [fields(row) for row in items(load(table))]

        def check_saved_indices(original, retained, key):
            # Shared saves refer to story chapters and the day's market offers by
            # position. Removing a row between two retained rows would remap them.
            if [row[key] for row in retained] != [row[key] for row in original[:len(retained)]]:
                raise ValueError(f"Demo {key} rows must retain their full-edition save indices")

        self.tables = {}
        self.catalog_ids = {}

        def select(table, candidates, namespace):
            self.tables[table] = sequence(record(row) for row in candidates)
            retained = {json.loads(row["id"]) for row in candidates}
            self.catalog_ids[namespace] = retained
            return retained

        config = fields(load("config"))
        night_limit = min(int(config["demo_nights"]), int(config["last_night"]))
        # The demo goes straight to its end screen. Its calendar cannot advance
        # past season one, even if that season's last night were repeated.
        last = min(night_limit, int(config["season_length"]))

        def unlocked(row):
            return int(row.get("unlock_night", "1")) <= last

        stories = []
        self.chapters = {}
        self.films = set()
        self.companions = set()
        self.finale_guests = set()
        self.flowers = False
        self.butterfly = False
        self.fares = False
        keepsakes = set()
        story_wares = set()
        story_rows = rows("story")
        for story in story_rows:
            start = int(story.get("start_night", "1"))
            if start > last:
                continue
            ghost = json.loads(story["ghost"])
            chapters = items(story["chapters"])
            available = chapters[:night_limit - start + 1]
            self.chapters[ghost] = len(available)
            self.flowers |= story.get("blossom") == "true"
            for chapter in map(fields, available):
                if "wish" in chapter:
                    story_wares.add(json.loads(chapter["wish"]))
                self.fares |= chapter.get("fare") == "true"
                if gift := option(chapter.get("keepsake", "None")):
                    keepsakes.add(gift)
            if len(available) == len(chapters):
                self.films.add(ghost)
                self.butterfly |= story.get("blossom") == "true"
                if visitor := option(story.get("finale_guest", "None")):
                    self.finale_guests.add(visitor)
                if pet := option(story.get("companion", "None")):
                    self.companions.add(pet)
            else:
                # Keep the true chapter count for progress, the final screen and
                # save handoff. Unreachable slots contain no future story content.
                story["chapters"] = sequence(available + ["(fare: true)"] * (len(chapters) - len(available)))
                story.pop("finale_guest", None)
                story.pop("companion", None)
            stories.append(story)
        check_saved_indices(story_rows, stories, "ghost")
        self.tables["story"] = sequence(map(record, stories))
        # Lost-and-found errands open when their owner's story is complete.
        errands = [e for e in rows("errands") if json.loads(e["ghost"]) in self.films]
        self.tables["errands"] = sequence(map(record, errands))
        self.errands = {json.loads(e["id"]) for e in errands}
        keepsakes.update(json.loads(e["item"]) for e in errands)

        self.products = select("products", [p for p in rows("products")
                                            if unlocked(p) or json.loads(p["id"]) in story_wares], "product")
        ghost_rows = rows("ghosts")
        special = self.finale_guests | self.chapters.keys()
        if int(fields(config["thief"])["from_night"]) <= last:
            special.add("shade")  # ghosts::THIEF_GHOST
        self.regulars = {json.loads(g["id"]) for g in ghost_rows if unlocked(g) and float(g["spawn_weight"]) > 0}
        guests = [g for g in ghost_rows if json.loads(g["id"]) in special | self.regulars]
        self.shoppers = self.regulars | self.chapters.keys()
        self.redeemable = self.regulars | self.films | self.finale_guests
        self.can_flee = {json.loads(g["id"]) for g in guests
                         if json.loads(g["id"]) in self.regulars and items(g.get("needs", "[]"))}
        for guest in guests:
            guest["wishes"] = sequence(w for w in items(guest["wishes"]) if json.loads(w) in self.products)
        self.guests = select("ghosts", guests, "ghost")
        # Match the longest full species ID before the animation suffix.
        self.portrait_ids = sorted([json.loads(g["id"]) for g in ghost_rows] + ["apprentice"],
                                   key=len, reverse=True)

        vendors = []
        last_offered = {}
        vendor_rows = rows("market")
        for vendor in vendor_rows:
            until = option(vendor.get("until_night", "None"))
            end = min(last, until) if until is not None else last
            if max(2, int(vendor.get("from_night", "1"))) > end:
                continue  # The first market opens before the second night.
            vendors.append(vendor)
            for ware in items(vendor["wares"]):
                last_offered[ware] = max(last_offered.get(ware, 0), end)

        def sold(kind, row):
            return int(row.get("unlock_night", "1")) <= last_offered.get(f'{kind}({row["id"]})', 0)

        decor = [d for d in rows("decor") if json.loads(d["id"]) in keepsakes | {"shelf", "candle"}
                 or sold("Decor", d)]
        self.decor = select("decor", decor, "decor")
        upgrade_rows = rows("upgrades")
        self.upgrade_ids = {json.loads(u["id"]) for u in upgrade_rows}
        self.upgrades = select("upgrades", [u for u in upgrade_rows if sold("Upgrade", u)], "upgrade")
        ware_types = {"Decor": self.decor, "Upgrade": self.upgrades, "Single": self.products}
        for vendor in vendors:
            wares = []
            for ware in items(vendor["wares"]):
                kind, _, arg = ware.partition("(")
                if kind not in ware_types or json.loads(arg[:-1]) in ware_types[kind]:
                    wares.append(ware)
            vendor["wares"] = sequence(wares)
        check_saved_indices(vendor_rows, vendors, "id")
        self.vendors = select("market", vendors, "vendor")

        sets = [s for s in rows("sets") if all(json.loads(d) in self.decor for d in items(s["decor"]))]
        events = [e for e in rows("events") if int(e["night"]) <= last]
        for row in sets + events:
            if "ghost_weights" in row:
                row["ghost_weights"] = sequence(w for w in items(row["ghost_weights"])
                                                if json.loads(items(w)[0]) in self.guests)
        select("sets", sets, "set")
        select("events", events, "event")
        bench = fields(load("workbench"))
        bench["junk"] = sequence(j for j in items(bench["junk"]) if json.loads(fields(j)["product"]) in self.products)
        self.tables["workbench"] = record(bench)

    def include_key(self, key):
        parts = key.split(".")
        namespace = parts[0]
        if namespace in self.catalog_ids:
            return parts[1] in self.catalog_ids[namespace]
        if key == "memory.ida_sailor":
            return "ida" in self.guests
        if namespace == "errand":
            return parts[1] in self.errands if len(parts) > 2 else bool(self.errands)
        if namespace == "story" and len(parts) > 2:
            ghost, chapter = parts[1:3]
            if chapter == "ending":
                return ghost in self.films
            return ghost in self.chapters and (not chapter.isdigit() or int(chapter) <= self.chapters[ghost])
        if namespace == "finale":
            return parts[1] in self.finale_guests
        if key.startswith("book.note."):
            return parts[2] in self.guests
        if namespace in {"achievement", "achievements", "season"}:
            return False
        if key in {"night.give_fare", "night.fare_empty", "story.fare_plainly", "log.fare_given"}:
            return self.fares
        return key not in {"pause.achievements", "log.achievement", "menu.version_full",
                           "end.thanks_full", "end.next_season", "end.keep_playing"}

    def include_file(self, path):
        if path.name == ".gitkeep":
            return False
        if path.name.upper().startswith(("CREDITS", "LICENSE", "COPYING")):
            return True
        parts = path.parts
        stem = path.stem
        # The demo has no achievements: neither their data nor their icons.
        if parts == ("data", "achievements.ron") or parts[:2] == ("sprites", "achievements"):
            return False
        if parts[:2] == ("videos", "endings"):
            return stem in self.films
        if parts[:2] == ("sounds", "endings"):
            return parts[2] in self.films
        if parts[0] == "sounds":
            return not stem.startswith(("night_ambience_", "evening_music_"))
        if parts[:2] == ("sprites", "postcards"):
            return stem in self.regulars
        if parts[:2] == ("sprites", "ghosts"):
            ghost = next((g for g in self.portrait_ids if stem == g or stem.startswith(g + "_")), None)
            if ghost not in self.guests:
                return ghost == "apprentice" and ghost in self.upgrades
            # The thief never joins the queue. Only random arrivals with unmet
            # needs can flee; story visits and the thief always enter the shop.
            action = stem.removeprefix(ghost + "_")
            if action == "salute":
                return "ida" in self.guests and ghost == "sailor"
            if action in {"approach", "impatient", "pay"}:
                return ghost in self.shoppers
            if action == "ascend":
                return ghost in self.redeemable
            if action == "flee":
                return ghost in self.can_flee
            return True
        if parts[0] in {"sprites", "models"} and parts[1] in {"products", "decor"}:
            return stem in (self.products if parts[1] == "products" else self.decor)
        if parts[:2] == ("models", "shop") and stem in self.upgrade_ids:
            return stem in self.upgrades
        if parts[:2] == ("models", "market"):
            if stem.startswith(("stall_", "vendor_")):
                return stem.split("_", 1)[1] in self.vendors
            if stem == "window_flowers":
                return self.flowers
        if parts[:2] == ("models", "critters") and stem.startswith("dog_"):
            return "dog" in self.companions
        if parts[:2] == ("models", "critters") and stem.startswith("butterfly_"):
            return self.butterfly
        if parts[:2] == ("models", "errands"):
            return stem.rpartition("_clue_")[0] in self.errands
        # Remaining assets are shared: the town and shop, equipment, weather,
        # opening, language/font support, common music, UI and effect sounds.
        return True


def copy_assets(source: Path, destination: Path, edition: str):
    """Stage into a new directory. Full releases copy the original asset bytes."""
    if edition not in {"demo", "full"}:
        raise ValueError(f"Unknown edition: {edition}")
    if source.resolve() in (destination.resolve(), *destination.resolve().parents):
        raise ValueError("Asset destination must be outside the source tree")
    if edition == "full":
        shutil.copytree(source, destination, ignore=shutil.ignore_patterns(".gitkeep"))
        return

    demo = DemoAssets(source)
    destination.mkdir(parents=True, exist_ok=False)
    for path in source.rglob("*"):
        rel = path.relative_to(source)
        if not path.is_file() or not demo.include_file(rel):
            continue
        out = destination / rel
        out.parent.mkdir(parents=True, exist_ok=True)
        if rel.parts[0] == "data" and path.stem in demo.tables:
            out.write_text(demo.tables[path.stem] + "\n", encoding="utf-8", newline="\n")
        elif rel.parts[0] == "i18n" and path.suffix == ".ron" and path.name != "languages.ron":
            # The catalog helper returns the original escaped value, even for
            # right-to-left text or RON escapes that JSON cannot decode.
            values = read_catalog(path)
            if not values:
                raise ValueError(f"Empty translation catalog: {path}")
            lines = (f'    "{k}": "{v}",' for k, v in values.items() if demo.include_key(k))
            out.write_text("{\n" + "\n".join(lines) + "\n}\n", encoding="utf-8", newline="\n")
        else:
            shutil.copy2(path, out)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("edition", choices=("full", "demo"))
    parser.add_argument("source", type=Path)
    parser.add_argument("destination", type=Path)
    args = parser.parse_args()
    copy_assets(args.source, args.destination, args.edition)
