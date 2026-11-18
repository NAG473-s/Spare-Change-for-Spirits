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

During the first night a tips box explains each step (texts in `assets/data/tips.ron`).

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
  shop.rs        grid, decor, tools, atmosphere, door/counter
  ghosts.rs      ghosts: spawning, flow (Entering → … → Leaving/Ascending), queue
  register.rs    handing over items, payment, change
  economy.rs     coins, purses, paying and change logic (+ tests)
  ui.rs          top bar, side panel per phase, message log, buttons → actions
  menu.rs        main menu, pause menu, tips box
  audio.rs       sound effects and music
  fx.rs          glow, night shade, particles, the Cue message
  save.rs        saving, loading, new game
  data.rs        loading the RON files
  rng.rs         small random number generator
assets/
  data/          config.ron, products.ron, ghosts.ron, decor.ron, story.ron, tips.ron
  fonts/         Inter (SIL Open Font License, see Inter-LICENSE.txt)
  sprites/       your own images (optional, see below)
```

Buttons carry an `Action`. A global observer forwards it as a message, and every plugin reads
the actions it cares about. New button = new variant in `ui::Action` + handling in the right plugin.

## Content and balancing

All content lives in `assets/data/*.ron` – restarting the game is enough, no recompiling.
Add new products, ghosts or decor as new entries. On startup the game checks that every ghost
only wishes for products that exist.

`show_change_due: true` in `config.ron` shows the change due (assist mode).

## Adding images

Until there is an image, the game shows coloured placeholders. [docs/ART_PROMPTS.md](docs/ART_PROMPTS.md)
has a style block and a ready prompt for every image. Prepare each generated image with

```bash
python3 tools/prepare_sprite.py path/to/image.png KIND [ID]
```

which trims, fits and scales it and writes it to the right place (needs Pillow,
`pip install -r tools/requirements.txt`):

| Kind | Written to | Drawn at | Generate at |
|---|---|---|---|
| `ghost` | `assets/sprites/ghosts/<id>.png` | 44 × 66 | 1024 × 1536 |
| `decor` | `assets/sprites/decor/<id>.png` (keepsakes too) | 52 × 52 | 1024 × 1024 |
| `product` | `assets/sprites/products/<id>.png` | 24 × 24 | 1024 × 1024 |
| `door` | `assets/sprites/shop/door.png` | 64 × 96 | 1024 × 1536 |
| `counter` | `assets/sprites/shop/counter.png` | 48 × 272 | 1024 × 1536 |
| `floor` | `assets/sprites/shop/floor.png` (one tile) | 62 × 62 | 1024 × 1024 |
| `background` | `assets/sprites/shop/background.png` | 1280 × 720 | 1536 × 1024 |

`<id>` is the `id` from the corresponding RON file (e.g. `shy`, `candle`).
**Steam:** AI-generated content must be disclosed in the Steamworks content survey.

## Sound and effects

All sounds are synthesised by `tools/make_sounds.py` (numpy + ffmpeg) into `assets/sounds/*.ogg`:
shop bell, coins, chime, crash, click, sigh, a music-box waltz for the evening and a night
ambience loop. They are generated from scratch, so there are no licence questions. To use your own
recording, replace the file of the same name. Sound can be switched off in the menus.

Visual atmosphere (`src/fx.rs`): candles, lamps and other light-giving decor glow and flicker, the
shop darkens at night, dust drifts through the air, and particles mark ghosts passing into the
light and items crashing off shelves. Gameplay triggers sounds and effects through the `Cue` message.

## Next steps (ideas)

- [x] Poltergeist: knocks items off shelves, sometimes pays with counterfeit coins (spot them!)
- [x] Forgetful ghost: wants to pay twice – accept or be honest?
- [x] Edwin: a story across five nights, keepsakes as decor
- [x] Main menu, pause, save games, first-night tips
- [x] Sound: shop bell, coins, chime, music box, night ambience
- [x] Particles when passing into the light, candle glow, night shade
- [ ] Ghost animations (sprite sheets)
- [ ] Localisation (e.g. German)
