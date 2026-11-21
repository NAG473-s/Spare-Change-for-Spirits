# Spare Change for Spirits

A shop management game for ghosts: your shop only opens between midnight and dawn.
Stock the shelves, furnish the shop for shy and vain spirits, guess their last wishes
and hand out change in buttons, candle stubs and silver spoons.

- Engine: [Bevy](https://bevy.org) 0.20
- Steam App ID: 5657500
- Full game target: 25 January 2027

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

Debug builds keep only line tables (`Cargo.toml`), so they stay small. Each build variant
(features, targets) still adds its own copies under `target/`; run `cargo clean` now and then
if disk space runs short.

Balancing simulation (no window; plays every night many times with simple strategies and prints
money, reputation, sales, missed wishes, fled and unhappy ghosts, story chapters and parcels per
night, and the nights on which each season's finale was done):

```bash
BEVY_ASSET_ROOT=. cargo run --release -- --simulate 500
```

Trailers and store screenshots: start with `--trailer` (players never see these keys). **F9**
hides the whole interface, **F10** starts or stops a slow camera drive around the current view
(the camera returns to its place), **F12** saves a screenshot to `screenshots/`:

```bash
cargo run --release -- --trailer
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
drifting dust, the logo and the main menu. **Esc** pauses (resume, save and quit to menu, settings,
quit; the *Menu* button in the top bar does the same). Settings (main menu and pause menu): tips,
sound, music, effects and narration volume, interface size (on top of the automatic scaling with the window
height), fullscreen, and the assists *Patient ghosts* and *Show change due*. 

**Controller** (Steam Deck, Xbox, PlayStation …, `src/gamepad.rs`): the left stick moves the
pointer, **A** clicks, **B** right-clicks, the right stick scrolls panels, the d-pad nudges the
pointer and **Start** pauses. The controller drives the mouse cursor itself, so everything that
works with the mouse works with it; a ring marks the pointer until the real mouse moves again.

Saves: three profiles, each a game of its own with an autosave (start of every evening and night,
and dawn) and three manual slots (pause menu → *Save game*, at any time; autosaves never overwrite
them). *Continue* loads the newest playable save across all profiles. *Load game* lists all slots,
and *New game* asks for the profile. A save continues at the market, in the evening, or at the
completed season’s epilogue. During the guided first sale, a save resumes at the customer or the change tray, including coins already counted. Once that sale is complete, the first night can restart from its receipt. Other nights restart at their opening preparations.
Market offers, the hourglass, returnable crates and the day’s accounts carry over.
Profile files are stored in `profile<N>/auto.ron` and `profile<N>/slot<M>.ron`
(an old `save.ron` is moved to `profile1/auto.ron`). Saves and settings live in
`%APPDATA%\SpareChangeForSpirits` (Windows), `~/Library/Application Support/SpareChangeForSpirits`
(macOS) or `~/.local/share/spare-change-for-spirits` (Linux). Set `SCFS_SAVE_DIR` to use another
folder, e.g. for test runs:

```bash
SCFS_SAVE_DIR=/tmp/scfs-test cargo run
```

With tips enabled, a new profile starts with Hedda’s optional first-sale guide. Buy any crate from the ferry, put a ware on a shelf and open the shop. The first customer asks for that ware and waits while the night clock stands still. The ghost book keeps the guide for later. At the register, a right-click on a coin puts one of that denomination back. At the ferry it returns a crate, and in the shop it takes a ware off display during preparation. The first morning’s ledger shows an actual transaction and the wares still in stock.
The coin bar at the top right shows the cash drawer in every phase; the clock on the shop wall
shows the time of night.

Each of the six ghost stories ends with a short, narrated film. The three painted scenes play when the last chapter is settled, including Kasimir’s fare. Speech and captions follow the game language. A click, tap or controller button skips the film and returns to the same night.

## Desktop icons

Windows builds embed the ghost-and-button icon in the executable. On macOS, run
`cargo build --release --locked` followed by `python3 tools/package_macos.py`. The resulting
`target/macos/Spare Change for Spirits.app` includes the Finder and Dock icon and the game assets.
For a binary built with `--features demo`, run `python3 tools/package_macos.py --edition demo`.
The demo bundle is written to `target/macos/demo/Spare Change for Spirits.app`.
Pass a prebuilt macOS executable to package an Intel, Apple Silicon or Universal build. For Steam,
place the matching `libsteam_api.dylib` beside that executable before packaging. The packager
includes it in `Contents/Frameworks` before signing the bundle on macOS.

On Linux the game sets its window icon and application ID. To add it to the application menu,
place `spare_change_for_spirits` on your PATH with `assets/` beside the real executable. Install
the launcher and its icon from the repository root.

```sh
xdg-icon-resource install --mode user --novendor --size 512 art/icons/spare-change-for-spirits.png spare-change-for-spirits
xdg-desktop-menu install --mode user --novendor packaging/linux/spare-change-for-spirits.desktop
```

## Desktop builds

The [Desktop builds workflow](.github/workflows/build.yml) builds the full game and demo in
one run. Windows, Linux and macOS run in parallel, each with its own Cargo cache. The jobs run
the Rust tests for both editions and start each staged game with `--simulate 1` to check that
it finds the packaged assets.

For Windows packages alone, choose **Actions → Windows builds → Run workflow**. This manual workflow builds the x86-64 full game and demo on one Ubuntu runner using MinGW-w64. Both editions share their compiled dependencies, and Wine runs the tests and starts each packaged executable. [Windows builds workflow](.github/workflows/windows.yml)

Both workflows keep the packaged games on the runner for the duration of the job. Local runs
of [`tools/package_ci.py`](tools/package_ci.py) leave the packages in `dist/full/<platform>/`
and `dist/demo/<platform>/`. The Windows package contains the executable and its files.
Extract the Linux `.tar.gz` to keep the executable permissions. Extract the macOS
`.zip` to get the ad-hoc signed Universal `.app`. Each edition includes its Steam runtime,
assets and licenses. All three platforms use the same five-night asset selection for the demo.

The run's artifacts: the Windows edition is the game folder, which GitHub hands out as a ZIP.
The macOS `.zip` and the Linux `.tar.gz` are uploaded as they are (`archive: false`), so the
download is the archive itself and not a ZIP inside a ZIP. Double-click the macOS `.zip` in
Finder to get the `.app`. The app is ad-hoc signed, not notarized: macOS blocks it after a
browser download. Allow it once under *System Settings → Privacy & Security → Open Anyway*, or
remove the download mark in Terminal:

```bash
xattr -dr com.apple.quarantine "Spare Change for Spirits.app"
```

The workflow runs on pushes to `main` and tags matching `v*`, and can be started manually under
*Actions → Desktop builds → Run workflow*. Commits marked `[skip ci]` skip the automatic run.
Runs for the same ref queue behind the current run.

## Gameplay

| Phase | What happens |
|---|---|
| **Market day** | From the second day on, the village market opens before the evening: seven traders sell furniture (shelves of every size, candles and lamps, plants and incense, mirrors, rugs and screens), the raven changes coins and the Reaper sells rare single pieces and an hourglass that makes the coming night longer. Bought furniture waits in the storeroom; it can be sold back the same day. |
| **Evening** | Place furniture from the storeroom (removing it puts it back), step out through the shop door to the harbour, where the ferryman's boat waits: hover over a crate to see the stock and cash after purchase and the dawn when the new wares would fade. Click to buy a crate, or right-click to give one back. Back in the shop, pick a tool and click cells: build shelves, candles, mirrors …, put products on shelves. Turn the sign in the window (or press the button) to open: the clock runs on to midnight and strikes. |
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
- **Story ghosts** (`story.ron`) – **Edwin** (from night 1) and **Ida**, the lighthouse keeper
  (from night 6), come once per night with one wish per chapter. Each fulfilled wish leaves a
  **keepsake**: free decor that only they can give. The evening panel announces their wish. An
  unfulfilled chapter repeats the next night; from the second failed night on the ghost names the
  ware, and whenever it is not in stock at opening time the ferryman brings a parcel with it, so
  no story can get stuck. Ida's chapters relight the harbour lighthouse; her last one is the
  finale, and the season lasts until it is done.
- **Seasons** – the game is told in seasons of ten nights, each with its own story and
  finale: Hedda's (1–10), "The Long Winter" with Lotte, her dog Pfote and Anselm the clockmaker
  (11–20), and "The Thaw" with Mira the gardener and Kasimir the ferryman, whose last wish is
  one coin for his own crossing (21–30). Later seasons keep the shop open without a story.
- **Market** – each trader's wares can be listed and bought from the side panel; five traders
  have one ware on special offer each day (`specials`, `special_discount`), and seasonal traders
  come only in their season: Ilse Icicle in winter, Bluma the florist in spring (`market.ron`).
- **Hedda's letters** – the player inherited the shop from great-aunt Hedda. Her first letter
  waits on the first evening, the others turn up when improvements clear her old junk
  (`letter` in `upgrades.ron`). At the finale she comes home through the shop door. At the
  following dawn the view goes out to the quay, where Ida's restored beam sweeps the water, and
  Hedda's farewell letter waits; the accounts appear once the player continues, and the letter
  stays in the book's Letters chapter (`finale.rs`).
- **Lost-and-found errands** (`errands.ron`, `errands.rs`) – after Lotte's story she has lost
  her little sledge in the snow. Tracks on the quay (evening) and behind the market stalls (by
  day) are clicked to follow them; the sledge, found with a cracked runner, is mended at the
  workbench and given back, Lotte lets Pfote keep it as a keepsake, and her book page remembers
  the night. No step is timed.
- **Ghost book** – seeing a visitor for the first time adds their name to the index, even if they turn away at the door. Select the name to see a painted portrait, a short character note and the traits you can use at the counter. The page also keeps track of visits, wishes heard and wares supplied. The book is one overlay with coloured chapter tabs along the top – ghosts, keepsakes, Hedda's letters, order slips, the first-sale guide and, in the full game, the achievements – so new pages become chapters instead of menu entries; Close returns to wherever the book was opened. Each keepsake has its own page with its picture, the ghost and chapter it came from and the words said when it was given; in the evening, clicking a keepsake standing in the shop opens that page. A keepsake marked `logbook` in `story.ron` keeps a log of its story: Ida's logbook lists each repair of the lighthouse with the night it was done, and later repairs stay unwritten. In the shop and at the harbour, the expected guest list links familiar names to their pages. An unfamiliar visitor is listed anonymously.
  Next-night orders have a slip showing the customer, ware, quantity and collection night. Pin a slip to mark its crate on the ferry. The book keeps the outcome when an order is collected or expires.
- **Regulars** – a ghost served well may come back now and then by name (Mrs Wispel, Captain
  Brine …), often for the ware it bought the first time, and tips more often as it returns. A regular can recall its last purchase or recognise a keepsake it has seen in the shop before. The ghost book records the remark and the wish from that visit (`regulars` in `config.ron`, `regulars.rs`).
- **Shop window** – in the evening one ware from stock goes in the window; ghosts who wish for
  it come more often while one is left (`showcase_attract`, `showcase.rs`).
- **Furniture sets** – three matching pieces in the shop draw certain ghosts and add to the
  atmosphere, e.g. the dressing room (mirror, vanity, folding screen) for vain ghosts and
  brides, or Ida's keepsakes as the seafarer's corner (`sets.ron`, `sets.rs`). With a furnishing or removal tool selected, hover over a floor square to see how the atmosphere would change and which familiar guests would gain or lose a condition they need, including the effects of replacing a piece or completing a set.
- **Apprentice** – an improvement from the Reaper: a helper ghost behind the counter serves up
  to three customers a night who wait in line with a ware from the shelves (`apprentice.rs`).

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
  upgrades.rs    shop improvements bought at the market, junk still on the floor
  loan.rs        the bankruptcy rule: the ferryman's loan for a fresh start
  workbench.rs   repairing Hedda's broken junk at the workbench into wares
  incidents.rs   small mishaps during the night (draught, crow, leak) to click away
  winter.rs      season 2: the tower clock on the market square and the snow
  tooltip.rs     the tooltip that follows the pointer (market wares, shelf products)
  audio.rs       sound effects and music
  fx.rs          flickering lights, particles, the Cue message
  save.rs        saving, loading, new game
  data.rs        loading the RON files
  i18n.rs        language catalogs, placeholders, language detection
  rng.rs         small random number generator
  sim.rs         balancing simulation (`--simulate`), plays every night without a window
  book.rs        the ghost book, regulars and shop window in the save
  regulars.rs    regular customers who come back by name
  showcase.rs    the ware in the shop window
  sets.rs        furniture sets and tonight's ghost odds
  apprentice.rs  the helper ghost behind the counter
  errands.rs     lost-and-found errands with clues to click
  finale.rs      the quiet dawn scene after a season's finale
  weather.rs     rain, storm, snow and fog of tonight's event
  gamepad.rs     controller: the sticks drive the mouse pointer, A/B click
  trailer.rs     helpers for trailers and screenshots (`--trailer`)
  playtest.rs    playtest logs next to the saves, to compare real games with the simulation
  guide.rs       Hedda's guided first sale on the first evening and night
  orders.rs      wares promised to a customer for the next night
  achievements.rs achievements of the full game, kept in the save and passed to Steam
  story_film.rs  full-screen illustrated films at the end of a ghost's last chapter
assets/
  data/          config.ron, products.ron, ghosts.ron, decor.ron, story.ron, events.ron,
                 props.ron, market.ron, upgrades.ron, workbench.ron, incidents.ron, sets.ron,
                 errands.ron,
                 achievements.ron
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
languages), each with its name in its own language for the language menu. On each launch the
game uses the first supported desktop language, with English as the fallback. A supported
per-game language from Steam takes precedence when launched through Steam. The opening and
menus use the same language. Choosing a language in the main menu saves an override.
Choose *Automatic* to follow Steam or the desktop again.

On Steam launches the game reads the per-title language from the Steamworks API when its
redistributable is installed beside the executable or in the macOS bundle’s Frameworks folder.
In a standard Steam library installation it can also read the game’s own app manifest, including
offline. It checks the user’s selection before the installed depot language.

- Data files (`assets/data/*.ron`) hold only ids and numbers. Texts for an id use keys like
  `product.<id>.name`, `product.<id>.hint.1`, `ghost.<id>.name`, `ghost.<id>.the` (the subject at
  the start of a sentence, with article), `decor.<id>.name`, `story.<ghost>.<n>.intro`, `letter.<n>`.
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

`tools/make_sounds.py` synthesises the sound effects and instrumental music into
`assets/sounds/*.ogg`, including the evening music box, night ambience and four-bar ferry score. The long winter
(season 2) has a slower waltz with sleigh bells and a gusty, whistling night wind; the thaw
(season 3) a brighter tune with birdsong and a night of crickets and frogs
(`evening_music_<season>.ogg`, `night_ambience_<season>.ogg`; other seasons keep the first
season's tracks).
On the first launch the ferryman tells a short story in the active language.
Each of the 31 game languages has a recorded reading and timed captions. The game plays the local
`intro_voice_<language>.ogg` alongside `intro_music.ogg`. Skipping ends both tracks. Music,
narration and effects have independent volume controls. Audio production credits are in
[`assets/sounds/CREDITS.txt`](assets/sounds/CREDITS.txt).

Weather (`weather` in `events.ron`, `src/weather.rs`): tonight's event shows outside from the
evening until dawn – rain running down the shop window and falling over the harbour, slanting
rain or a snowstorm with harder-flickering candles, quiet snowfall, or fog drifting through the
shop at knee height and over the harbour water.

Visual atmosphere: candles, lamps and other light-giving decor are flickering point lights
(`src/fx.rs`), ambient and moon light follow the time of day (`src/scene.rs`), dust drifts through
the air at night, and particles mark ghosts passing into the light and items crashing off shelves.
Gameplay triggers sounds and effects through the `Cue` message.

## Next steps (ideas)

The [game roadmap](docs/ROADMAP.md) covers the complete campaign, its ending and free play through 25 January 2027.
What to enter in Steamworks (Steam Cloud for both apps, achievements for the full game) is in
[docs/STEAM.md](docs/STEAM.md).

- [x] Poltergeist: knocks items off shelves, sometimes pays with counterfeit coins (spot them!)
- [x] Forgetful ghost: wants to pay twice – accept or be honest?
- [x] Edwin: a story across five nights, keepsakes as decor
- [x] Ida, Hedda's letters and the finale; stories can never get stuck, the season waits for them
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
