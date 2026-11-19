# 3D model prompts

The shop is a small 3D diorama. Furniture, products and the shop itself are 3D models; the ghosts
stay hand-drawn sprites (`assets/sprites/ghosts/`) that float in the room. Until a model exists the
game shows a simple coloured placeholder, so models can be added one by one.

Hand this whole file to ChatGPT (or another tool that builds models in Blender). Start with the
**technical rules** and the **style block**, then go through the lists.

## Open orders

The shop clock, hanging sign, ferryman's equipment and harbour models are in the game.
Run `python3 tools/check_models.py` to review their dimensions, origins and triangle counts.

## Technical rules (all models)

- **Format:** glTF binary, one `.glb` file per model, file name = the id in the tables below.
  Export with *+Y Up* (Blender's default glTF export), materials included, no cameras, no lights,
  no animations.
- **Exactly one scene per file.** If several models live in one Blender file, export each with
  *Include → Limit to → Selected Objects* **and** *Active Scene* (or keep one Blender file per
  model). Files with extra, empty scenes do not load in the game; `python3 tools/fix_models.py`
  repairs them.
- **Scale:** 1 Blender unit = 1 metre. Keep each model inside the size given in its table row
  (width × height × depth). The game does not rescale models.
- **Origin:** at the **bottom centre** of the model (the point that stands on the floor) unless
  the table gives a specific gameplay pivot. Apply all transforms (Ctrl+A → All Transforms)
  before exporting.
- **Orientation:** the **front faces Blender's −Y axis**, i.e. what you see in *Front* view
  (numpad 1). Height goes along +Z.
- **Polygons:** low-poly, at most ~3,000 triangles per model (products ~800).
- **Materials:** only *Principled BSDF* with base colour, roughness and optionally emission,
  or one baked colour texture per model (≤ 1024 px). Procedural node textures do **not** export.
- **Flames and glowing parts** (candles, lantern, window glass): give them their own material
  with *Emission* colour and strength; the game adds the real light source itself.
- Smooth shading with a small bevel on edges looks best in the game's soft lighting.
- Put the file in the folder given in each section of this file.

## Style block

```
Style for all models: cozy, slightly whimsical low-poly 3D for a small shop
management game set in a ghost shop that opens at night. Soft rounded shapes
with small bevels, slightly chunky toy-like proportions, no sharp realism.
Muted night palette: old wood browns, deep indigo, slate blue, teal, dusty
rose, aged brass; warm candlelight amber only on flames and glowing parts.
Flat colours or very simple hand-painted textures, no photorealistic detail,
no text or letters on objects. Each object stands alone, centred on its
origin, front facing -Y, Z up, 1 unit = 1 metre.
```

## Furniture – `assets/models/decor/<id>.glb`

| id | Size (w × h × d, m) | Prompt |
|---|---|---|
| `shelf` | 0.9 × 1.5 × 0.45 | A small wooden shop shelf, old dark wood, open front, three boards at 0.35 m, 0.75 m and 1.15 m height (products are placed on the middle board), a little carved lace trim along the top. |
| `candle` | 0.4 × 1.1 × 0.4 | A standing iron candelabra on three curved feet with three melted candles on top (flames around 1.05 m height, emissive amber), wax drips. |
| `mirror` | 0.8 × 1.7 × 0.3 | A standing oval mirror in an ornate silver frame on two small feet; the glass is a pale blue-grey reflective-looking material. |
| `incense` | 0.4 × 0.8 × 0.4 | A small wooden pedestal with a round brass incense burner on top, a thin curl of purple smoke made of a few soft low-poly ribbons. |
| `rug` | 0.95 × 0.03 × 0.95 | A round woven rug, dusty rose and indigo pattern, slightly irregular edge. |
| `grandfather_clock` | 0.6 × 2.1 × 0.4 | A tall narrow grandfather clock in dark wood, round clock face (no numbers, just tick marks), brass pendulum behind a small glass door. |
| `oil_lamp` | 0.35 × 1.3 × 0.35 | A slender standing oil lamp on a wooden post, glass chimney with a warm emissive flame at about 1.2 m height. |
| `fern` | 0.5 × 0.9 × 0.5 | A lush fern in a round terracotta pot, drooping fronds. |
| `gramophone` | 0.6 × 1.1 × 0.6 | A small wooden cabinet with an old gramophone on top, big brass horn, a record on the turntable. |
| `stove` | 0.7 × 1.1 × 0.6 | A little round cast-iron stove on four feet with a short chimney pipe; the small door glows warm orange (emissive) at about 0.5 m height. |
| `vanity` | 0.9 × 1.5 × 0.5 | A vanity table with a large oval mirror, a small drawer, a perfume bottle and a hairbrush on top. |

## Keepsakes (gifts from Edwin) – `assets/models/decor/<id>.glb`

| id | Size (w × h × d, m) | Prompt |
|---|---|---|
| `umbrella` | 0.35 × 0.9 × 0.35 | A closed black umbrella with a curved wooden handle standing in a small brass umbrella stand. |
| `teacup` | 0.5 × 0.7 × 0.5 | A tiny round side table with a floral porcelain teacup and saucer on top; the cup has a small chip on the rim. |
| `blanket` | 0.8 × 0.9 × 0.8 | A small cosy armchair in faded green velvet with a hand-knitted rust-and-cream blanket draped over the backrest. |
| `wedding_photo` | 0.5 × 0.9 × 0.4 | A small wooden easel holding an old sepia photograph in a gilded frame (the photo shows two blurry figures dancing, no faces needed). |
| `pocket_lantern` | 0.4 × 1.2 × 0.4 | A short wooden post with a small brass lantern hanging from a hook at the top; warm emissive glass panes. |

## Products – `assets/models/products/<id>.glb`

Shown standing on the middle board of a shelf. Keep the silhouette simple and readable from a
distance; each product fits inside **0.25 × 0.25 × 0.25 m**.

| id | Prompt |
|---|---|
| `rain_jar` | A corked glass jar holding a tiny grey cloud with a few blue raindrops. |
| `letter_paper` | A small stack of yellowed letter paper tied with a red ribbon, a quill lying on top. |
| `music_box` | A small wooden music box with the lid half open and a tiny brass crank on the side. |
| `pocket_watch` | A silver pocket watch standing upright on its chain, lid open, hands stopped. |
| `wool_scarf` | A rolled-up knitted mustard-yellow wool scarf with tassels. |
| `marbles` | A small shallow wooden bowl with five colourful glass marbles. |
| `tea_tin` | A round old tea tin with a faded floral band and a slightly dented lid. |
| `faceless_photo` | A small standing photo frame whose picture is a soft blurry grey shape. |
| `rusty_key` | A large old rusty iron key lying on a tiny velvet cushion. |
| `powder_compact` | A round silver powder compact, slightly open, with a tiny puff. |
| `lavender` | A small bundle of dried lavender tied with twine, lying on its side. |
| `tin_soldier` | A small painted tin toy soldier with a drum, standing on a round base. |
| `ship_in_bottle` | A tiny sailing ship inside a corked glass bottle lying on a small wooden stand. |
| `brass_compass` | An open round brass compass, lid up, needle pointing north. |
| `fish_bone` | A cartoonish fish skeleton (head, spine, tail) on a small plate. |
| `ball_of_yarn` | A loose ball of soft blue yarn with a strand hanging out. |
| `polishing_cloth` | A folded soft cloth with a small round tin of metal polish on top. |
| `hair_ribbon` | A light blue silk ribbon tied in a bow, lying flat. |
| `wedding_veil` | A folded white lace veil with a small pearl comb on top. |
| `pressed_flower` | An open old book with a pressed pink flower between the pages. |

## Shop – `assets/models/shop/<id>.glb`

| id | Size (w × h × d, m) | Prompt |
|---|---|---|
| `counter` | 4.5 × 1.0 × 0.8 | A long wooden shop counter (front = customer side facing −Y): panelled front with a carved ghost emblem in the middle, a polished top, a small brass bell and an old brass cash register near the right end. The game turns it so it runs along the right side of the shop. |
| `door` | 1.1 × 2.3 × 0.2 | An old wooden shop door in its frame with a small round window, a brass handle and a little brass bell hanging above it; faint blue light in the window (emissive). The game places it in the left wall. |
| `room` | see below | *(optional)* The walls of the shop, replacing the plain placeholder walls. |
| `wall_clock` | 0.78 × 0.78 × 0.08 | A round wall clock in a dark wooden frame with a cream face and twelve tick marks, **no hands** (the game adds and turns them). Origin at the **centre of the back** of the clock, face towards −Y. |
| `shop_sign` | 1.0 × 0.75 × 0.05 | The hanging "closed / open" sign for the shop window: a painted wooden board 0.98 × 0.44 m in a thin frame, hanging from two cords that meet 0.3 m above it. The **front (−Y) is painted dark red**, the **back (+Y) green**, both with a thin cream border and **no lettering** (the game writes "closed"/"open" in the player's language). Origin at the **centre of the board**. The game flips it round. |

**Room layout** (only if you make `room.glb`; Blender coordinates, Z up, origin at 0,0,0, do not
apply an offset): the floor is **not** part of the room model (the game draws the floor grid).

- Back wall: an inner face at **Y = +3.0**, from X = −7.0 to X = +7.5, height 3.4 m.
- Left wall: an inner face at **X = −7.0**, from Y = −3.5 to Y = +3.0, height 3.4 m; leave a
  1.2 m wide, 2.4 m high opening centred at **Y = 0** for the door.
- A tall window in the back wall centred at **X = −3.2**, bottom edge at 0.9 m, 1.6 × 1.8 m,
  with frame and emissive moonlit glass; dark wallpaper with a faded pattern, wooden wainscot up
  to 0.9 m, a few cobwebs in the upper corners.
- Leave the right side and the front open (the camera looks in from the front).

## The ferryman – `assets/models/shop/<id>.glb`

In the evening the player walks out to the harbour, where the ferryman sells crates from his boat.

| id | Size (w × h × d, m) | Prompt |
|---|---|---|
| `ferry_boat` | 7.4 × 0.6 × 1.7 | A long, low, flat-bottomed wooden ferry boat, dark weathered planks, a raised curved bow and stern, a narrow rim along both sides, an empty flat deck (crates, the ferryman and his lantern are added by the game). Origin at the **centre of the deck surface**; the hull reaches about 0.4 m below it. |
| `crate` | 0.5 × 0.3 × 0.42 | A small wooden crate **without a lid**, open at the top, filled with straw almost to the rim, rope handles on the sides. |
| `crate_lid` | 0.52 × 0.05 × 0.44 | The matching flat lid of planks with two battens. Origin at the **middle of its back edge** (the hinge); the lid extends forwards (−Y in Blender), so the game can swing it open. |
| `ferryman` | 0.7 × 1.45 × 0.6 | A tall, gaunt ferryman in a long hooded dark cloak, slightly stooped, the face hidden in shadow except for two pale, softly glowing eyes (emissive), hands folded in front. **No pole** (the game adds it). |

## The harbour – `assets/models/harbor/<id>.glb`

The harbour behind the shop, where the ferryman moors: a stone quay with a wooden boardwalk (drawn
by the game), the harbour master's hut, a lighthouse, a lamp post, a jetty with rowboats, the
village behind. The game adds all lights, the lighthouse beam and every movement (bobbing boats,
the swinging bell, drifting lanterns), so the models stay still and contain **no light sources** –
glowing glass and windows are just emissive materials.

| id | Size (w × h × d, m) | Prompt |
|---|---|---|
| `harbour_hut` | 2.6 × 2.9 × 2.1 | The harbour master's small wooden hut: dark weathered planks, a steep shingled roof, a plank door on the left of the front, a window on the right glowing warm amber (emissive), a lantern on a hook beside the door at about 1.75 m (emissive glass), a coil of rope and a lifebuoy on the wall. |
| `lighthouse` | 1.2 × 4.3 × 1.2 | A small, stout lighthouse on a round stone base: tapering tower with white and dusty-red bands, a little iron gallery, a glass lamp room **centred at 3.6 m height** glowing pale gold (emissive), a dark conical roof. |
| `lamp_post` | 0.4 × 2.7 × 0.4 | An old cast-iron harbour lamp post with a square glass lantern on top, the glass **centred at 2.45 m** and glowing warm amber (emissive). |
| `village_house` | 2.2 × 2.4 × 1.8 | A narrow, crooked old fisherman's house in dark stone and timber with a steep roof and a chimney, one small window glowing amber (emissive) on the front. Seen from afar behind the harbour; used several times, so keep it plain. |
| `barrel` | 0.6 × 0.72 × 0.6 | An old wooden barrel with iron hoops. |
| `mooring_post` | 0.3 × 0.6 × 0.3 | A short weathered wooden mooring post (bollard) with a coil of thick rope around it. |
| `bell_post` | 0.6 × 1.95 × 0.2 | A wooden post with its origin at the foot. A short arm sticks out to the right (+X), with a hook **0.38 m right of the post, 1.85 m high** for the separate bell. |
| `ferry_bell` | 0.25 × 0.22 × 0.25 | A small, slightly tarnished brass bell with a clapper and a ring at the top. Origin at the **top of the ring** (where it hangs), the bell below it, so the game can swing it. |
| `hand_cart` | 2.0 × 0.75 × 1.0 | A wooden two-wheeled hand cart: a flat loading bed of planks **0.42 m high, 1.2 × 0.8 m**, with a low rim, two spoked wheels under the middle, two long handles reaching out to the left (−X). The origin is on the ground below the centre of the bed where the game stacks bought crates. |
| `signpost` | 0.95 × 1.95 × 0.2 | A crooked wooden signpost with its origin at the foot and two arrow-shaped boards pointing to the right, one at 1.6 m and one at 1.3 m, **no lettering**. |
| `rowboat` | 0.8 × 0.45 × 2.0 | A small wooden rowboat, length along the Y axis (front to back), two seats, a pair of oars resting inside. |
| `jetty` | 1.1 × 0.7 × 3.8 | A narrow wooden jetty: a deck of planks running along the Y axis (3.8 m long, 1.0 m wide) on four posts. Origin at the **centre of the deck surface**; the posts reach about 0.6 m below it. |
| `paper_lantern` | 0.26 × 0.34 × 0.26 | A small floating paper lantern on a tiny round wooden raft: a box of warm paper glowing amber (emissive). |

## Delivery: push every model on its own

Put each model into the repository **as soon as it is finished** – do not wait for the others.

- Repository: `https://github.com/NAG473-s/Spare-Change-for-Spirits`, branch `main`
  (commit directly to `main`, no pull requests).
- **One commit per model:** exactly one `.glb` file per commit, at the path given in its section
  (e.g. `assets/models/decor/shelf.glb`), file name = the id.
- Commit message in English: `Add <id> model [skip ci]`, e.g. `Add shelf model [skip ci]`.
  Use the repository's existing author identity (see `AGENTS.md`); no co-author lines.
- Run `git pull --rebase origin main` before each push, then push right away.
- Commit only the `.glb` (textures embedded); no `.blend` files, renders or other changes.
- A replacement for an existing model is again one commit: `Update <id> model [skip ci]`.

## After generating

1. Each `.glb` is in its folder with the exact id as file name (and pushed on its own, see above).
2. Check of size, origin and triangle count (the rules were tested with Blender 5.3):
   `python3 tools/check_models.py` (lists every model and flags problems).
3. Start the game – the model replaces the placeholder.

**Steam:** AI-generated content must be disclosed in the Steamworks content survey.
