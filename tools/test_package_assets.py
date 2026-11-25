"""Release asset regressions; run with python3 -B tools/test_package_assets.py."""

from hashlib import sha256
from pathlib import Path
import re
import shutil
from tempfile import TemporaryDirectory
import unittest

from i18n_sync import read_catalog
from package_assets import DemoAssets, copy_assets, fields, items

ASSETS = Path(__file__).resolve().parents[1] / "assets"


def hashes(root):
    return {path.relative_to(root): sha256(path.read_bytes()).digest()
            for path in root.rglob("*") if path.is_file() and path.name != ".gitkeep"}


class PackageAssetsTests(unittest.TestCase):
    def test_full_is_byte_identical_and_demo_keeps_playable_content_in_every_language(self):
        source_hashes = hashes(ASSETS)
        with TemporaryDirectory() as tmp:
            full = Path(tmp) / "full"
            demo = Path(tmp) / "demo"
            copy_assets(ASSETS, full, "full")
            copy_assets(ASSETS, demo, "demo")
            self.assertEqual(source_hashes, hashes(full))
            self.assertEqual(source_hashes, hashes(ASSETS))
            self.assertEqual({p.stem for p in (demo / "sprites/postcards").glob("*.png")},
                             {"shy", "vain", "poltergeist", "forgetful", "child", "cat", "sailor", "knight", "bride"})
            for rel in ("sprites/memories/edwin_rain.png", "fonts/ShopSigns.ttf", "videos/endings/edwin.avi", "sprites/ghosts/edwin_pay.png", "sprites/ghosts/shade.png",
                        "models/shop/door.glb", "models/shop/door_leaf.glb", "models/shop/door_bell.glb",
                        "models/props/change_tray.glb", "sprites/ui/change_tray.png",
                        "models/shop/postcard_stand.glb", "sprites/postcards/bride.png",
                        "sprites/ghosts/apprentice.png", "sprites/ghosts/bride_browse.png", "models/critters/cat_tail.glb",
                        "models/products/ship_in_bottle.glb", "models/decor/pocket_lantern.glb", "models/props/storm_lantern.glb",
                        "models/harbor/lighthouse.glb", "models/market/clock_tower.glb", "models/market/clock_hand_hour.glb",
                        "models/market/clock_hand_minute.glb", "sounds/CREDITS.txt", "fonts/Noto-LICENSE.txt"):
                with self.subTest(asset=rel):
                    self.assertEqual((demo / rel).read_bytes(), (full / rel).read_bytes())
            for rel in ("data/achievements.ron", "videos/endings/ida.avi", "sounds/endings/anselm",
                        "sprites/ghosts/hedda.png", "sprites/ghosts/lantern_child.png", "sprites/ghosts/ida_pay.png",
                        "sprites/ghosts/shade_pay.png", "sprites/ghosts/edwin_flee.png",
                        "models/products/storm_lantern.glb", "models/decor/apron.glb", "models/critters/dog_tail.glb",
                        "models/decor/toy_sledge.glb", "sprites/decor/toy_sledge.png",
                        "models/errands/lotte_sledge_clue_1.glb", "models/errands/lotte_sledge_clue_2.glb",
                        "models/critters/butterfly_body.glb", "models/critters/butterfly_wing.glb",
                        "models/market/vendor_frost.glb", "models/market/window_flowers.glb", "sounds/evening_music_spring.ogg"):
                with self.subTest(asset=rel):
                    self.assertFalse((demo / rel).exists())

            for case in ("post_case", "sea_trunk", "bottle_hamper", "hat_box"):
                for part in (case, case + "_lid"):
                    self.assertTrue((demo / "models/shop" / (part + ".glb")).is_file(), part)
            self.assertFalse((demo / "sprites/ghosts/sailor_salute.png").exists())
            english = read_catalog(demo / "i18n/en.ron")
            self.assertEqual(items((demo / "data/errands.ron").read_text(encoding="utf-8")), [])
            for key in ("memory.edwin_rain", "shelf.dawn_count", "book.note.shade", "story.edwin.5.thanks", "story.edwin.ending", "letter.4", "story.plainly",
                        "event.all_souls.name", "end.carry_over", "set.edwins_porch.name", "register.undo_hint"):
                self.assertIn(key, english)
            for key in ("memory.ida_sailor", "book.note.ida", "story.ida.1.intro", "finale.hedda.arrives", "season.1.epilogue",
                        "event.new_moon.name", "set.seafarers_corner.name", "achievement.edwin.name",
                        "errand.lotte_sledge.hint.1", "errand.clue_title"):
                self.assertNotIn(key, english)

            # The registry, translation files and voice assets must agree.
            locales = re.findall(r'code:\s*"([^"]+)"', (full / "i18n/languages.ron").read_text(encoding="utf-8"))
            self.assertGreater(len(locales), 1)
            for locale in locales:
                with self.subTest(locale=locale):
                    rel = Path("i18n") / f"{locale}.ron"
                    retained = read_catalog(demo / rel)
                    self.assertEqual(retained.keys(), english.keys())
                    self.assertEqual(retained, {k: v for k, v in read_catalog(full / rel).items() if k in english})
                    for rel in (f"sounds/intro_voice_{locale}.ogg", f"sounds/endings/edwin/{locale}.ogg"):
                        self.assertEqual((demo / rel).read_bytes(), (full / rel).read_bytes())

    def test_errand_and_butterfly_assets_follow_story_completion(self):
        with TemporaryDirectory() as tmp:
            source = Path(tmp) / "assets"
            shutil.copytree(ASSETS / "data", source / "data")
            # Give the first demo story a lost item and a flowering window box.
            for table, old, new in [
                ("errands", 'ghost: "lotte"', 'ghost: "edwin"'),
                ("story", 'ghost: "edwin",', 'ghost: "edwin", blossom: true,'),
            ]:
                path = source / "data" / f"{table}.ron"
                path.write_text(path.read_text(encoding="utf-8").replace(old, new, 1), encoding="utf-8")
            config = source / "data/config.ron"
            original = config.read_text(encoding="utf-8")
            for nights in (4, 5):
                config.write_text(original.replace("demo_nights: 5,", f"demo_nights: {nights},"), encoding="utf-8")
                demo = DemoAssets(source)
                complete = nights == 5
                with self.subTest(nights=nights):
                    self.assertTrue(demo.include_file(Path("models/market/window_flowers.glb")))
                    for rel in ("models/critters/butterfly_wing.glb", "models/errands/lotte_sledge_clue_2.glb",
                                "models/decor/toy_sledge.glb", "sprites/decor/toy_sledge.png"):
                        self.assertEqual(demo.include_file(Path(rel)), complete)
                    self.assertEqual(bool(items(demo.tables["errands"])), complete)
                    self.assertEqual(demo.include_key("errand.lotte_sledge.title"), complete)
                    self.assertEqual(demo.include_key("errand.clue_title"), complete)

    def test_incompatible_saved_offer_indices_are_rejected(self):
        with TemporaryDirectory() as tmp:
            source = Path(tmp) / "assets"
            shutil.copytree(ASSETS / "data", source / "data")
            market = source / "data/market.ron"
            text = market.read_text(encoding="utf-8")
            market.write_text(text.replace('id: "carpenter",', 'id: "carpenter", from_night: 6,'), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "save indices"):
                DemoAssets(source)

    def test_partial_story_keeps_progress_indices_without_shipping_the_ending(self):
        with TemporaryDirectory() as tmp:
            source = Path(tmp) / "assets"
            shutil.copytree(ASSETS / "data", source / "data")
            config = source / "data/config.ron"
            text = config.read_text(encoding="utf-8")
            config.write_text(text.replace("demo_nights: 5,", "demo_nights: 7,"), encoding="utf-8")
            projection = DemoAssets(source)
            stories = list(map(fields, items(projection.tables["story"])))
            self.assertEqual([s["ghost"] for s in stories], ['"edwin"', '"ida"'])
            self.assertEqual(len(items(stories[1]["chapters"])), 5)
            self.assertEqual(projection.chapters["ida"], 2)
            self.assertTrue(projection.include_key("story.ida.2.intro"))
            self.assertFalse(projection.include_key("story.ida.3.intro"))
            self.assertFalse(projection.include_key("story.ida.ending"))
            self.assertFalse(projection.include_file(Path("videos/endings/ida.avi")))
            self.assertNotIn("finale_guest", stories[1])
            self.assertIn("ship_bell", projection.decor)
            self.assertNotIn("sea_chart", projection.decor)
            self.assertNotIn("hedda", projection.guests)


if __name__ == "__main__":
    unittest.main()
