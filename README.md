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

The game starts in the main menu. **Esc** pauses (resume, save and quit to menu, tips on/off,
quit). The game autosaves at the start of every evening and night and at dawn; a save always
continues at an evening (a night in progress restarts). Saves and settings live in
`%APPDATA%\SpareChangeForSpirits` (Windows), `~/Library/Application Support/SpareChangeForSpirits`
(macOS) or `~/.local/share/spare-change-for-spirits` (Linux). Set `SCFS_SAVE_DIR` to use another
folder, e.g. for test runs:

```bash
SCFS_SAVE_DIR=/tmp/scfs-test cargo run
```

During the first night a tips box explains each step (texts `tip.*` in the language catalogs).

## Windows build

The GitHub Actions workflow [`build-windows.yml`](.github/workflows/build-windows.yml) builds a
release `.exe`, packages it together with `assets/` and uploads it as the artifact
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
| **Evening** | Order crates from the ferryman (max. 4 per evening), pick a tool and click cells: build shelves, candles, mirrors …, put products on shelves. |
| **Night** | Ghosts come in, browse and queue at the counter. The ghost at the front whispers a hint – you hand over a product from the shelves. Guess right → it pays and you count out the change. |
| **Morning** | Accounts: sales, change mistakes, ghosts that passed into the light, faded items. |

If the **atmosphere** (light, warmth, silence, reflection) does not suit a ghost, it flees at the
door. Served correctly **and** given exact change → the ghost passes into the light and your
reputation rises. Better reputation = more customers.

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
  audio.rs       sound effects and music
  fx.rs          flickering lights, particles, the Cue message
  save.rs        saving, loading, new game
  data.rs        loading the RON files
  i18n.rs        language catalogs, placeholders, language detection
  rng.rs         small random number generator
assets/
  data/          config.ron, products.ron, ghosts.ron, decor.ron, story.ron
  i18n/          languages.ron and one text catalog per language (en.ron is the source)
  fonts/         Inter and Noto subsets (SIL Open Font License, see *-LICENSE.txt)
  models/        3D models (.glb), see docs/MODEL_PROMPTS.md
  sprites/       ghost, product and floor images
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
- Chinese, Japanese, Korean, Thai and Arabic use Noto fonts reduced to the characters actually
  used. After changing any translation run `python3 tools/subset_fonts.py` (check with `--check`).

## Content and balancing

All content lives in `assets/data/*.ron` (numbers) and `assets/i18n/*.ron` (texts) – restarting
the game is enough, no recompiling. Add new products, ghosts or decor as new entries, with their
texts in `en.ron` and every other language. On startup the game checks that every ghost
only wishes for products that exist.

`show_change_due: true` in `config.ron` shows the change due (assist mode).

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
| Counter, door, walls | `assets/models/shop/counter.glb`, `door.glb`, `room.glb` (optional) |

Check models (size, origin, triangles, missing files) with:

```bash
python3 tools/check_models.py
```

Tested with Blender 5.3 (any recent version with the bundled glTF exporter works).

**Images** ([docs/ART_PROMPTS.md](docs/ART_PROMPTS.md), prepared with `tools/prepare_sprite.py`):
ghosts (`assets/sprites/ghosts/<id>.png`) are always sprites; product pictures
(`assets/sprites/products/<id>.png`) are shown on shelves while a product has no model; the floor
texture is `assets/sprites/shop/floor.png`.

**Steam:** AI-generated content must be disclosed in the Steamworks content survey.

## Sound and effects

All sounds are synthesised by `tools/make_sounds.py` (numpy + ffmpeg) into `assets/sounds/*.ogg`:
shop bell, coins, chime, crash, click, sigh, a music-box waltz for the evening and a night
ambience loop. They are generated from scratch, so there are no licence questions. To use your own
recording, replace the file of the same name. Sound can be switched off in the menus.

Visual atmosphere: candles, lamps and other light-giving decor are flickering point lights
(`src/fx.rs`), ambient and moon light follow the time of day (`src/scene.rs`), dust drifts through
the air at night, and particles mark ghosts passing into the light and items crashing off shelves.
Gameplay triggers sounds and effects through the `Cue` message.

## Next steps (ideas)

- [x] Poltergeist: knocks items off shelves, sometimes pays with counterfeit coins (spot them!)
- [x] Forgetful ghost: wants to pay twice – accept or be honest?
- [x] Edwin: a story across five nights, keepsakes as decor
- [x] Main menu, pause, save games, first-night tips
- [x] Sound: shop bell, coins, chime, music box, night ambience
- [x] Particles when passing into the light, candle glow, night shade
- [ ] Ghost animations (sprite sheets)
- [x] Localisation into the 31 Steam languages
