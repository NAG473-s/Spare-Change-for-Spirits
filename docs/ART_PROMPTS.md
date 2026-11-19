# Art prompts

The shop is a 3D diorama: furniture, products, the counter, the door and the walls are 3D models
(see [MODEL_PROMPTS.md](MODEL_PROMPTS.md)). Images are only needed for:

- **Ghosts** – hand-drawn sprites that float in the room and always face the camera.
- **Icons** – small pictures of the products, furniture and coins used in the interface. A product
  picture is also shown on the shelf while the product has no 3D model yet.
- **The interface frame** – painted walnut and brass trim for menus, shop panels, crate tooltips
  and the traders' nameplates at the village market.
- **Surface textures and light** – the shop floor, market cobblestones and the soft halo around harbour lamps.
- **The title screen** – a dedicated Full HD background and the game logo.
- **The desktop icon** – the same ghost and brass button in the Linux, macOS and Windows launchers.
- **Steam store art** – done, see [`art/steam/`](../art/steam/README.md).

The furniture and keepsake pictures of the old 2D version are reused as icons; the originals and
the unused door, counter and back wall images live in [`art/legacy-2d/`](../art/legacy-2d/README.md).

See what is still missing (models, ghost sprites, icons):

```bash
python3 tools/check_models.py
```

## To do

There are no open image orders. The sign emblems and the shop door’s porthole light are included. The market nameplates reuse the [painted interface frame](../assets/sprites/ui/ledger_frame.png).
The desktop icon, furniture icons, market cobblestones and harbour lamp halo are included.
The game generates the water surface and its ripple normals at runtime.

To check models and icons on a fresh checkout, pull `main` and run `python3 tools/check_models.py`.
It marks absent files with `missing` or `no icon`. The environment textures and their paths are
listed under [Water and light](#water-and-light--canvas-1024--1024).

## Workflow

1. Start a new ChatGPT conversation and paste the **style block** below once, with
   "Use this style for every image in this conversation." For new ghosts, also attach one or two
   existing ghost sprites from `assets/sprites/ghosts/` as a style reference.
2. Send one prompt at a time. Ask for a **transparent background** (except the floor) and the
   canvas size given in the section.
3. Prepare the download – this trims empty borders, fits the image to the game's aspect ratio
   and writes it to the right place:

   ```bash
   python3 tools/prepare_sprite.py ~/Downloads/image.png ghost cat --colors 48
   python3 tools/prepare_sprite.py ~/Downloads/image.png product fish_bone --colors 48
   python3 tools/prepare_sprite.py ~/Downloads/image.png decor fern --colors 48
   python3 tools/prepare_sprite.py ~/Downloads/image.png floor
   ```

   `--remove-bg` helps if the background did not come out transparent; `--colors 48` matches the
   existing sprites. Needs Pillow: `pip install -r tools/requirements.txt`.
4. Start the game – the image replaces the coloured placeholder automatically.

If one image looks off next to the others, regenerate it in the same conversation with
"Same style as the previous images, but …".

## Delivery: push every image on its own

Put each prepared image into the repository as soon as it is finished: repository
`https://github.com/NAG473-s/Spare-Change-for-Spirits`, branch `main`, **one commit per image**
at the path `tools/prepare_sprite.py` wrote, message `Add <id> sprite [skip ci]` (English, the
repository's author identity from `AGENTS.md`, no co-author lines), `git pull --rebase origin main`
before each push.

## Style block

```
Style for all images: cozy-spooky hand-painted 2D game art for a small shop
management game set in a ghost shop that opens at night. Soft painterly
shading with visible brush texture, gentle rounded shapes, slightly chunky
proportions. Muted night palette: deep indigo, slate blue, teal, dusty rose,
old wood brown, with warm candlelight amber as the only strong accent.
Light comes from the upper left. Clean dark-brown outline about 2 px at
256 px size. Front view, seen very slightly from above. Single centred
character or object, no text, no letters, no frame, no shadow on the ground,
transparent background, PNG.
```

## Ghosts – canvas 1024 × 1536 (portrait)

Ghosts are friendly, semi-transparent, softly glowing and float slightly; no legs, a wispy tail.
Keep the same body shape and size as the existing ghosts; vary colour, face and one accessory.
In the game they are about 1.35 m tall and face the camera.

All ten sprites are included in `assets/sprites/ghosts/`. The prompts below can be used to redraw them.

**Ghosts introduced in later nights:**

| File id | Prompt |
|---|---|
| `cat` | A small ghost cat, pale lavender glow, round and fluffy with a wispy tail instead of legs, big curious eyes, one paw raised. |
| `sailor` | A drowned sailor ghost, sea-green glow, striped shirt and a knitted cap, a little seaweed in his beard, holding a coil of rope, wistful smile. |
| `knight` | A friendly headless knight ghost, silver-blue glow, slightly rusty armour, carrying his smiling helmeted head under one arm. |
| `bride` | A ghostly bride, pearly white glow, old-fashioned lace dress, no veil, a small bouquet held close, hopeful but sad eyes. |

**Original ghosts:**

| File id | Prompt |
|---|---|
| `shy` | A shy little ghost, pale blue glow, hiding half its face behind a long wispy sleeve, big worried eyes looking down, slightly hunched. |
| `vain` | A vain ghost, soft pink glow, holding a tiny hand mirror, eyelashes and a beauty mark, chin up, a pearl necklace floating around its neck. |
| `poltergeist` | A mischievous poltergeist, warm coral-red glow, wide grin, messy flame-like wisps on its head, a teacup and a spoon floating around it. |
| `forgetful` | A forgetful old ghost, faded beige glow, puzzled expression, a knot tied in a wispy finger, spectacles pushed up on its forehead. |
| `child` | A small child ghost, mint green glow, round and bouncy, missing front tooth grin, holding a paper pinwheel. |
| `edwin` | Edwin, an elderly gentleman ghost, warm pale-yellow glow, kind tired eyes, neat moustache, flat cap and a long coat, holding his hat to his chest. |

## Ghost animation sheets – canvas 1024 × 1536

The game already animates every ghost (bobbing, breathing, swaying, leaning into its movement,
trembling when worried, stretching as it passes into the light). A sheet adds a hand-drawn idle
loop on top: if `assets/sprites/ghosts/<id>_sheet.png` exists, it replaces `<id>.png` and plays
at 8 frames per second.

All ten ghosts have four-frame sheets. To redraw a sheet, ask for a **2 × 2 grid** on one
1024 × 1536 canvas – four frames, read left to right, top to bottom – with the existing sprite
attached as reference:

```
Four frames of a looping idle animation of this ghost, arranged as a 2 x 2
grid on one 1024 x 1536 canvas, frames read left to right, top to bottom.
The ghost is exactly the same size, shape, colours and position in every
frame; only small changes: frame 1 neutral, frame 2 the wispy tail curls to
the left and the eyes half close (blink), frame 3 neutral with the tail
straight, frame 4 the tail curls to the right. Same style as the reference,
no grid lines, no text, transparent background, PNG.
```

Prepare it (trims all frames with the same box so the ghost does not jitter, writes
`assets/sprites/ghosts/<id>_sheet.png`):

```bash
python3 tools/prepare_sprite.py ~/Downloads/image.png ghost_sheet shy --colors 48
```

`--grid 4x1` reads a strip of four frames instead. Delivery as for sprites, one commit per sheet:
`Add <id> sheet [skip ci]`.

## Title screen – canvas 1920 × 1080 (done)

The title screen shows `assets/sprites/title/background.png` scaled to cover the window, the logo
`assets/sprites/title/logo.png` and the main menu on the **left third**, which the game darkens.
The background is a dedicated Full HD illustration. The logo comes from the Steam library art
(`art/steam/library_logo.png`, trimmed). The game adds flickering light on the candle, the hanging
lamp and the window (`GLOWS` in `src/title.rs` – adjust the positions for a new picture), drifting
dust and a ghost floating past.

Use the prompt below to redraw the background.

```
The inside of the ghost shop at night, wide 16:9 scene, 1920 x 1080. Edwin,
the elderly gentleman ghost, stands behind the wooden counter on the right,
holding his flat cap to his chest; a shy blue ghost peeks out from behind a
shelf. A candle on the counter, a hanging lamp and a tall moonlit window give
warm and cool light. The left third is calm, dark patterned wallpaper with a
few wisps of mist (the logo and the menu go there). Same style as the
reference, no text, not transparent.
```

Prepare it with `python3 tools/prepare_sprite.py ~/Downloads/image.png background --out
assets/sprites/title/background.png` and deliver it as `Add title background [skip ci]`.

## Desktop application icon (done)

A friendly ivory and seafoam ghost holds a single worn brass button with four dark holes.
Keep the two eyes, the curved tail and the button readable at 16 px. Use a dark plum outline,
warm highlights, a transparent margin and no lettering. The three exports are in `art/icons/`.
Linux uses a 512 px PNG, the Windows ICO contains 16–256 px images, and the macOS ICNS includes
standard and Retina representations through 1024 px.

## Icons – canvas 1024 × 1024

One object per image, centred, seen from the front and slightly above, filling most of the canvas;
the game shows it at 24 px on a button (and products at shelf size while they have no model), so
keep the silhouette simple and bold. Prepare with kind `product` or `decor` (both 96 × 96):
`assets/sprites/products/<id>.png` and `assets/sprites/decor/<id>.png`. A button without an icon
just shows its text. Delivery: one commit per image, `Add <id> icon [skip ci]`.

**Delivered furniture icons:**

| File id | Prompt |
|---|---|
| `small_shelf` | A small, simple wooden shelf on short legs with one board, slightly crooked. |
| `display_table` | A low shop display table with turned legs and a dark green cloth on top. |
| `tall_bookcase` | A tall, narrow dark wooden bookcase with a carved crown and three boards. |
| `glass_cabinet` | An elegant glass display cabinet with a brass frame and glass doors. |
| `folding_screen` | A three-panel folding screen with dusty-rose silk panels painted with moths and moons. |

**Done:** every product and the other furniture have icons; the furniture and keepsake icons
`shelf`, `candle`, `mirror`, `incense`, `rug`, `grandfather_clock`, `umbrella`, `teacup`,
`blanket`, `wedding_photo` and `pocket_lantern` come from the old 2D version. Prompts for redoing
the later ones:

| File id | Prompt |
|---|---|
| `ship_in_bottle` | A tiny sailing ship inside a corked glass bottle lying on a small wooden stand. |
| `brass_compass` | An open round brass compass, lid up, needle pointing north. |
| `fish_bone` | A cartoonish fish skeleton (head, spine, tail) on a small plate. |
| `ball_of_yarn` | A loose ball of soft blue yarn with a strand hanging out. |
| `polishing_cloth` | A folded soft cloth with a small round tin of metal polish on top. |
| `hair_ribbon` | A light blue silk ribbon tied in a bow. |
| `wedding_veil` | A folded white lace veil with a small pearl comb on top. |
| `pressed_flower` | An open old book with a pressed pink flower between the pages. |
| `oil_lamp` | A slender standing oil lamp on a wooden post, glass chimney with a warm glowing flame. |
| `fern` | A lush fern in a round terracotta pot, drooping fronds. |
| `gramophone` | A small wooden cabinet with an old gramophone on top, big brass horn, a record on the turntable. |
| `stove` | A little round cast-iron stove on four feet with a short chimney pipe, its small door glowing warm orange. |
| `vanity` | A vanity table with a large oval mirror, a small drawer, a perfume bottle and a hairbrush on top. |

## Coins – canvas 1024 × 1024 (done)

The coin bar at the top and the change buttons at the register show a small picture of each coin:
`assets/sprites/coins/<coin>.png` (96 × 96). Prepare a painted replacement with
`python3 tools/prepare_sprite.py ~/Downloads/image.png product <coin> --out assets/sprites/coins/<coin>.png --colors 48`
and deliver one commit per image, `Add <coin> coin icon [skip ci]`.

| File id | Prompt |
|---|---|
| `button` | A single large old sewing button made of warm brown horn, four holes, a slightly raised rim, seen from the front. |
| `candle` | A short stub of a cream wax candle with a few drips and a small warm flame. |
| `spoon` | A small tarnished silver teaspoon with an ornate handle, lying diagonally. |

## Interface frame – 512 × 512 (done)

`assets/sprites/ui/ledger_frame.png`: a square painted walnut frame with worn brass leaf corners,
a straight border on each side and a quiet dark plum centre. Keep the area outside the frame
transparent. The game stretches the centre and edges around the corner paintings.

## Painted sign emblems – 512 × 1008 (done)

`assets/sprites/shop/sign_paint.png` contains seven transparent 512 × 144 strips. The first six show
an anchor, market awning, hanging lantern, shop doorway, crescent moon and rising sun.
The last strip centres the shop crest between two flourishes for destinations that fit on a single plank. Use worn gilt pinstripes and cream highlights over muted teal and rose enamel.
Keep the right three quarters of the first six strips clear for localised signwriting. The game lays both the motif
and the wording onto the wood, including the two sides of the turning window sign.

The seven market awnings carry their stitched trade emblems in the stall models. The motifs
follow the cloth folds and use a cream and old-gold thread against each trader’s colours.

## Door glass – 160 × 160 (done)

`assets/sprites/shop/porthole_light.png` is a transparent, soft-edged pool of daylight from
four panes of round glass. Keep the crossbars and outer edge soft and the floorboards visible.
The game stretches the pool along the floor and colours it with the time of day.

## Water and light – canvas 1024 × 1024

The market cobblestones (1024 × 1024) and the lamp halo (512 × 512) are included. The harbour
uses a moving wave mesh and a generated normal map. The two water PNGs below are optional art
overrides. For future replacements, save the PNG directly at its path and deliver one file per
commit with `Add <id> texture [skip ci]`.

| File | Prompt |
|---|---|
| `assets/sprites/harbor/water.png` | *(optional)* Seamless tileable texture of calm dark water at night seen from above, deep indigo and teal with soft painted ripple strokes and a few tiny silver glints, painterly, no objects, no reflections of anything specific. Must tile without visible seams. Not transparent. |
| `assets/sprites/harbor/water_normal.png` | *(optional)* The matching tangent-space normal map of the same ripples (purple-blue normal map colours), seamless. |
| `assets/sprites/market/cobbles.png` | Seamless tileable texture of old grey-brown cobblestones seen straight from above, soft moss in the gaps, painterly, no objects. Must tile without visible seams. Not transparent. |
| `assets/sprites/fx/glow.png` | A soft round glow on a transparent background: white in the middle, fading smoothly to fully transparent at the edge, no rays, no hard ring. Used, tinted, as the halo around lamps. 512 × 512 is enough. |

## Floor – canvas 1024 × 1024 (done)

`floor`: Seamless tileable texture of old dark wooden floorboards seen straight from above,
subtle wear, no objects. Not transparent. (Prepared with kind `floor`; used on every grid tile.)

## Steam store art (done)

The seven Steam images, the text-free artwork and the editable vector logo are in
[`art/steam/`](../art/steam/README.md), outside the game's packaged assets. For new versions:
generate the artwork **without any text**, add the logo afterwards, and check the sizes in
Steamworks before uploading (header 920 × 430, small 462 × 174, main 1232 × 706, vertical
748 × 896, library capsule 600 × 900, library hero 3840 × 1240, library logo 1280 × 720).

**Steam requires AI-generated content to be disclosed** in the Steamworks content survey.
