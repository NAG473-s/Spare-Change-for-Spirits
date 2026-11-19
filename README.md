# Spare Change for Spirits

A shop management game for ghosts: your shop only opens between midnight and dawn.
Stock the shelves, furnish the shop for shy and vain spirits, guess their last wishes
and hand out change in buttons, candle stubs and silver spoons.

- Engine: [Bevy](https://bevy.org) 0.20
- Steam App ID: 5657500
- Target: Steam shop management festival (registration deadline 2027-01-25)

## Running

```bash
cargo run
```

The first build takes a few minutes (Bevy is compiled), after that it is fast.
On Linux you need the usual Bevy dependencies
(`libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev`).

```bash
cargo test
```

tests the game logic and loads every data file.

Balancing simulation (no window; plays every night many times with simple strategies and prints
money, reputation, sales, missed wishes, fled and unhappy ghosts per night):

```bash
BEVY_ASSET_ROOT=. cargo run --release -- --simulate 500
```

Goals, results and open points of the balancing passes: [`docs/BALANCING.md`](docs/BALANCING.md).

Playtest logs: the game writes `logs/nights.csv` (one row per night with the simulation's figures,
the time each ghost spent at the counter and how long each phase took) and
`logs/session-<time>.txt` (every game message with night and clock) into the save folder. They
stay on the computer. Sum them up like the simulation with:

```bash
python3 tools/playtest_report.py
```

The game starts on the title screen: its own painted background with flickering lamp light,
drifting dust and ghosts floating past, the logo and the main menu. **Esc** pauses (resume, save and quit to menu, settings,
quit; the *Menu* button in the top bar does the same). Settings (main menu and pause menu): tips,
sound, music and effect volume, interface size (on top of the automatic scaling with the window
height), fullscreen, and the assists *Patient ghosts* and *Show change due*. 

Saves: three profiles, each a game of its own with an autosave (start of every evening and night,
and dawn) and three manual slots (pause menu → *Save game*, at any time; autosaves never overwrite
them). *Continue* loads the newest save of all profiles, *Load game* lists them all, *New game*
asks for the profile. A save continues at the market or at an evening; one made during a night
continues at the start of that night. Files: `profile<N>/auto.ron` and `profile<N>/slot<M>.ron`
(an old `save.ron` is moved to `profile1/auto.ron`). Saves and settings live in
`%APPDATA%\SpareChangeForSpirits` (Windows), `~/Library/Application Support/SpareChangeForSpirits`
(macOS) or `~/.local/share/spare-change-for-spirits` (Linux). Set `SCFS_SAVE_DIR` to use another
folder, e.g. for test runs:

```bash
SCFS_SAVE_DIR=/tmp/scfs-test cargo run
```

During the first night a tips box explains each step (texts `tip.*` in the language catalogs).
The coin bar at the top right shows the cash drawer in every phase; the clock on the shop wall
shows the time of night.

## Desktop icons

Windows builds embed the ghost-and-button icon in the executable. On macOS, run
`cargo build --release --locked` followed by `python3 tools/package_macos.py`. The resulting
`target/macos/Spare Change for Spirits.app` includes the Finder and Dock icon and the game assets.

On Linux the game sets its window icon and application ID. To add it to the application menu,
place `spare_change_for_spirits` on your PATH with `assets/` beside the real executable. Install
the launcher and its icon from the repository root.

```sh
xdg-icon-resource install --mode user --novendor --size 512 art/icons/spare-change-for-spirits.png spare-change-for-spirits
xdg-desktop-menu install --mode user --novendor packaging/linux/spare-change-for-spirits.desktop
```

## Windows build

The GitHub Actions workflow [`build-windows.yml`](.github/workflows/build-windows.yml) builds a
release `.exe`, packages it together with `assets/`, `LICENSE` and `THIRD_PARTY_LICENSES.txt` and
uploads it as the artifact
`SpareChangeForSpirits-windows`.

- Runs on pushes to `main`, on tags `v*`, and manually via
  *Actions → Build Windows → Run workflow*.
- Commits containing `[skip ci]` do not trigger it (see [AGENTS.md](AGENTS.md)).
- Build results are cached. The cache is saved right after building and testing, even when a
  step failed or the run was cancelled, so the next run continues instead of starting over.
  Only the very first run (or one after a Rust update) compiles everything.
- A new run of the same branch waits for a running one instead of cancelling it.

## Gameplay

| Phase | What happens |
|---|---|
| **Market day** | From the second day on, the village market opens before the evening: seven traders sell furniture (shelves of every size, candles and lamps, plants and incense, mirrors, rugs and screens), the raven changes coins and the Reaper sells rare single pieces and an hourglass that makes the coming night longer. Bought furniture waits in the storeroom; it can be sold back the same day. |
| **Evening** | Place furniture from the storeroom (removing it puts it back), step out through the shop door to the harbour, where the ferryman's boat waits: click a crate to buy it (max. 4 per evening), right-click to give one back. Back in the shop, pick a tool and click cells: build shelves, candles, mirrors …, put products on shelves. Turn the sign in the window (or press the button) to open: the clock runs on to midnight and strikes. |
| **Night** | Ghosts come in, browse and queue at the counter. The ghost at the front whispers a hint – you hand over a product from the shelves (click it on the shelf or in the panel; hovering a product on a shelf shows its price, stock and when it fades). Guess right → it pays and you count out the change. |
| **Morning** | The accounts: a ledger of the cash drawer (dusk, ferryman, furniture, takings, counterfeits, profit or loss) and how the night went – visitors, sales, change, reputation, ghosts that passed into the light, faded items and what fades next. |

If the **atmosphere** (light, warmth, silence, reflection) does not suit a ghost, it flees at the
door. Served correctly **and** given exact change → the ghost passes into the light and your
reputation rises. Better reputation = more customers.

**Five demo nights, each different** (the demo is built with `cargo build --features demo`; a save
from its end continues in the full game with night 6): every night has an event (`assets/data/events.ron`) –
a quiet first night, fog (darker, more shy ghosts), a harvest moon (higher prices), a storm (one
crate less, colder, sailors) and All Souls' Night (crowds, good prices). New products, decor and
ghosts unlock on later nights (`unlock_night`); the evening panel lists what is new.

Special customers:

- **Poltergeist** – knocks items off the shelf it browses unless the shop is quiet enough
  (silence ≥ 5), and sometimes pays with a counterfeit coin. Every paid coin is listed with a
  description at the register; click a suspicious one to inspect it. Doubting a genuine coin costs
  reputation, and counterfeits left in the drawer turn to dust at dawn.
- **Forgetful Ghost** – sometimes forgets it has paid and offers to pay again. Keep the money or
  tell it the truth.
- **Edwin** – a story ghost who comes once per night with one wish per chapter (`story.ron`).
  Each fulfilled wish leaves a **keepsake**: free decor that only he can give. An unfulfilled
  chapter repeats the next night.

## Project layout

```
src/
  main.rs        app, window, plugins
  day_cycle.rs   evening/night/morning phases, clock, night report, progress
  inventory.rs   stock (with age → fading), orders from the ferryman
  scene.rs       3D camera, room, time-of-day light, billboards, labels following 3D objects
  shop.rs        floor grid, decor models/placeholders, lights, tools, atmosphere, door/counter
  ghosts.rs      ghosts: spawning, flow (Entering → … → Leaving/Ascending), queue
  register.rs    handing over items, payment, change
  economy.rs     coins, purses, paying and change logic (+ tests)
  ui.rs          top bar, side panel per phase, message log, buttons → actions
  menu.rs        main menu, pause menu, tips box
  title.rs       title screen behind the main menu (background, glow, dust, ghosts)
  harbor.rs      the harbour behind the shop (quay, lighthouse, boats, mist) and walking there
  ferry.rs       the ferryman's boat: crates to buy, tooltip, his arrival and departure
  accounts.rs    the day's cash ledger and the morning accounts screen
  props.rs       decoration from assets/data/props.ron (counter, walls, quay)
  critters.rs    animals: shop cat, harbour cat, gull, owl, bats, fish, moths
  sign.rs        the closed/open sign in the window and opening the shop at midnight
  market.rs      the village market by day: stalls, traders, wares, coin changing
  tooltip.rs     the tooltip that follows the pointer (market wares, shelf products)
  audio.rs       sound effects and music
  fx.rs          flickering lights, particles, the Cue message
  save.rs        saving, loading, new game
  data.rs        loading the RON files
  i18n.rs        language catalogs, placeholders, language detection
  rng.rs         small random number generator
assets/
  data/          config.ron, products.ron, ghosts.ron, decor.ron, story.ron, events.ron,
                 props.ron, market.ron
  i18n/          languages.ron and one text catalog per language (en.ron is the source)
  fonts/         Inter and Noto subsets (SIL Open Font License, see *-LICENSE.txt)
  models/        3D models (.glb), see docs/MODEL_PROMPTS.md
  sprites/       ghost sprites, product, furniture and coin icons, floor and title screen images
```

Buttons carry an `Action`. A global observer forwards it as a message, and every plugin reads
the actions it cares about. New button = new variant in `ui::Action` + handling in the right plugin.

## Languages

All player-facing text lives in `assets/i18n/<code>.ron`, one catalog per language; `en.ron` is the
source and defines the keys. 31 languages are listed in `assets/i18n/languages.ron` (the Steam
languages), each with its name in its own language for the language menu. The game picks the saved
language, else the system language, else shows the language menu on first start.

- Data files (`assets/data/*.ron`) hold only ids and numbers. Texts for an id use keys like
  `product.<id>.name`, `product.<id>.hint.1`, `ghost.<id>.name`, `ghost.<id>.the` (the subject at
  the start of a sentence, with article), `decor.<id>.name`, `story.<n>.intro`.
- Placeholders such as `{who}` and `{item}` must stay in every translation; sentences may be
  rearranged. Coin amounts are shown as "silver spoon ×2" to avoid plural rules.
- `cargo test` checks that every language has every key with the same placeholders.
- New keys: add them to `en.ron`, translate them into a `<code>.json` per language and merge with
  `python3 tools/i18n_sync.py --add <folder>` (keeps every catalog in en.ron's order;
  `--missing` lists gaps).
- Chinese, Japanese, Korean, Thai and Arabic use Noto fonts reduced to the characters actually
  used. After changing any translation run `python3 tools/subset_fonts.py` (check with `--check`).

## Content and balancing

All content lives in `assets/data/*.ron` (numbers) and `assets/i18n/*.ron` (texts) – restarting
the game is enough, no recompiling. Add new products, ghosts or decor as new entries, with their
texts in `en.ron` and every other language. On startup the game checks that every ghost
only wishes for products that exist.

Accessibility (assists, interface size, volumes, mouse-only play) and what to declare on the
Steam store page: [`docs/ACCESSIBILITY.md`](docs/ACCESSIBILITY.md).

## 3D models and images

The shop is a small 3D diorama (Bevy 3D): a fixed camera looks into the room, furniture and the shop
are models, candles and lamps are real light sources, and the ghosts are hand-drawn sprites that
float in the room and always face the camera.

**Models** (`assets/models/`, glTF binary `.glb`): [docs/MODEL_PROMPTS.md](docs/MODEL_PROMPTS.md) has the
technical rules (metres, origin at the bottom centre, front facing −Y in Blender, polygon budget,
simple materials), a style block and a prompt with target size for every model. Until a model
exists, the game shows a coloured box of the size given in `assets/data/decor.ron`.

| What | Path |
|---|---|
| Furniture and keepsakes | `assets/models/decor/<id>.glb` |
| Products (stand on shelves) | `assets/models/products/<id>.glb` |
| Counter, door, walls, wall clock, window sign | `assets/models/shop/counter.glb`, `door.glb`, `room.glb`, `wall_clock.glb`, `shop_sign.glb` |
| The ferryman, his boat and crates | `assets/models/shop/ferryman.glb`, `ferry_boat.glb`, `crate.glb`, `crate_lid.glb` |
| The harbour (hut, lighthouse, jetty, boats …) | `assets/models/harbor/<id>.glb` |

Check models (size, origin, triangles, scenes, missing files) with:

```bash
python3 tools/check_models.py
```

A model exported from a Blender file with several scenes contains extra empty scenes that Bevy
cannot load; `python3 tools/fix_models.py` removes them in place.

Tested with Blender 5.3 (any recent version with the bundled glTF exporter works).

**Images** ([docs/ART_PROMPTS.md](docs/ART_PROMPTS.md), prepared with `tools/prepare_sprite.py`):
ghosts (`assets/sprites/ghosts/<id>.png`) are always sprites, animated by the game; an optional
sheet `<id>_sheet.png` (frames side by side) replaces the single image and plays as a loop.
Icons of products (`assets/sprites/products/<id>.png`) and furniture
(`assets/sprites/decor/<id>.png`) appear on the side panel's buttons; a product picture is also
shown on the shelf while the product has no model. The floor texture is
`assets/sprites/shop/floor.png`, the title screen uses `assets/sprites/title/`.

**Steam:** AI-generated content must be disclosed in the Steamworks content survey.

## Sound and effects

All sounds are synthesised by `tools/make_sounds.py` (numpy + ffmpeg) into `assets/sounds/*.ogg`:
shop bell, coins, chime, the clock striking midnight, crash, click, sigh, a music-box waltz for the evening and a night
ambience loop. They are generated from scratch, so there are no licence questions. To use your own
recording, replace the file of the same name. Sound can be switched off in the menus.

Visual atmosphere: candles, lamps and other light-giving decor are flickering point lights
(`src/fx.rs`), ambient and moon light follow the time of day (`src/scene.rs`), dust drifts through
the air at night, and particles mark ghosts passing into the light and items crashing off shelves.
Gameplay triggers sounds and effects through the `Cue` message.

## Next steps (ideas)

The plan up to the festival and the Early Access launch is in [docs/ROADMAP.md](docs/ROADMAP.md).

- [x] Poltergeist: knocks items off shelves, sometimes pays with counterfeit coins (spot them!)
- [x] Forgetful ghost: wants to pay twice – accept or be honest?
- [x] Edwin: a story across five nights, keepsakes as decor
- [x] Main menu, pause, save games, first-night tips
- [x] Sound: shop bell, coins, chime, music box, night ambience
- [x] Particles when passing into the light, candle glow, night shade
- [x] Ghost animations (procedural, plus optional hand-drawn sprite sheets)
- [x] Localisation into the 31 Steam languages

## License

Copyright © 2026 Sebastian Rode. **All rights reserved** – see [LICENSE](LICENSE). The code and all
content may not be copied, changed or redistributed without written permission.

Third-party parts keep their own licences: the Inter and Noto fonts (SIL Open Font License, see
`assets/fonts/`) and the Rust libraries the game is built with (listed with their licence texts in
[THIRD_PARTY_LICENSES.txt](THIRD_PARTY_LICENSES.txt); regenerate it with
`python3 tools/third_party_licenses.py` after changing dependencies). The Windows build ships both
files next to the game.
