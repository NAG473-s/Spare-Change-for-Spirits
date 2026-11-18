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

tests the money logic (paying, change).

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
  data.rs        loading the RON files
  rng.rs         small random number generator
assets/
  data/          config.ron, products.ron, ghosts.ron, decor.ron, story.ron  ← content and balancing
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

Until there is an image, the game shows coloured placeholders. Put a PNG with a transparent
background under the matching name and it is picked up automatically:

| What | Path | Display size |
|---|---|---|
| Ghost | `assets/sprites/ghosts/<id>.png` | 40 × 56 |
| Decor | `assets/sprites/decor/<id>.png` | 52 × 52 |
| Product | `assets/sprites/products/<id>.png` | 22 × 22 |
| Door / counter | `assets/sprites/shop/door.png`, `counter.png` | 50 × 110 / 40 × 260 |

`<id>` is the `id` from the corresponding RON file (e.g. `shy.png`, `candle.png`).
Generate them at 2–4× size; the game scales them down.

Tips for consistent AI-generated images:
- Prepend a fixed style description to every prompt (perspective, palette, light direction, line weight).
- Afterwards reduce all images to one shared colour palette.
- **Steam:** AI-generated content must be disclosed in the Steamworks content survey.

## Next steps (ideas)

- [x] Poltergeist: knocks items off shelves, sometimes pays with counterfeit coins (spot them!)
- [x] Forgetful ghost: wants to pay twice – accept or be honest?
- [x] Edwin: a story across five nights, keepsakes as decor
- [ ] Main menu, save games
- [ ] Sound: shop bell, coins, ticking grandfather clock
- [ ] Animations (ghost sprite sheets), particles when passing into the light
- [ ] Localisation (e.g. German)
