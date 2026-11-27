# 3D model prompts

The shop is a small 3D diorama. Furniture, products and the shop itself are 3D models; the ghosts
stay hand-drawn sprites (`assets/sprites/ghosts/`) that float in the room. Until a model exists the
game shows a simple coloured placeholder, so models can be added one by one.

Hand this whole file to ChatGPT (or another tool that builds models in Blender). Start with the
**technical rules** and the **style block**, then go through the lists.

## Open orders

The willow shopping basket, wrapped parcel and sold ticket are delivered for the village market. The ferry's braided lines and netted cork fenders are delivered for the harbour. Replacement specifications follow below.

The riverbank, mill tower and sails, market bunting line, shop curtain rail and separate hems have also been delivered. All models currently requested in this list have been delivered.

The crab apple wood, seasonal foliage, loose leaf and petal, and the shop's glazed pitcher are delivered. Their replacement specifications are under *Village crab apples*.

The market paving, herb beds, public bench, five upper storeys, fish vane and all nine fitted stall fronts are delivered. Their replacement specifications are under *The village market*.

## Delivered models

- The four working storage cases and their separate lids are in `assets/models/shop/`. Their silhouettes, contents and hinge specifications are listed under *Shop improvements*. The existing chair, crate and birdcage piles still occupy some locked tiles.

- The [postcard stand](../assets/models/shop/postcard_stand.glb) on the counter has worn walnut
  rails and brass corners. Its groove runs from end to end. The game seats the postcard in the
  groove and lifts it out to turn it over.
- All five quay lanterns use `assets/models/harbor/quay_lantern.glb`. A ladder bar sits
  below the square glass housing, centred 1.95 m above the foot. The game lights each lantern
  when the player restores it.
- Lotte's repaired `toy_sledge` is in `assets/models/decor/`. The two snow trails
  `lotte_sledge_clue_1` and `lotte_sledge_clue_2` are in `assets/models/errands/`.
  The second trail ends at a red mitten.
- Mira's `butterfly_body` and `butterfly_wing` are in `assets/models/critters/`.
  The game mirrors the right wing and animates both from their hinges.
- The seasonal market stalls and traders `stall_frost`, `vendor_frost`, `stall_florist`
  and `vendor_florist`, plus the daily offer's `special_tag`, in `assets/models/market/`.
  Their furniture `fairy_lights`, `fur_rug`, `flower_pot` and `birdhouse` is in
  `assets/models/decor/`.
- Season 3, "The Thaw": the products `seed_packet`, `watering_can` and `storm_lantern`
  in `assets/models/products/`; the keepsakes `seedling_pot`, `flower_box`, `nameplate`
  and `oar` in `assets/models/decor/`; the market's `window_flowers` in `assets/models/market/`.
- Season 2, "The Long Winter": the five products `gear`, `clock_key`, `little_bell`, `leash`
  and `snow_globe` in `assets/models/products/`; the four keepsakes `chalk_drawing`,
  `dog_basket`, `cog_wheel` and `cuckoo_clock` in `assets/models/decor/`.
- The tower clock `clock_tower` and its separate pivoted hands `clock_hand_hour` and
  `clock_hand_minute` in `assets/models/market/`. The dial centre is at 2.75 m, aligned
  with the hands placed by the game.
- Lotte's ghost dog Pfote, `dog_body` and the separate `dog_tail`, in `assets/models/critters/`.
- The `workbench` in `assets/models/decor/`.
- The three night mishaps `draught`, `crow` and `leak` in `assets/models/incidents/`.

The models for Ida’s four keepsakes (logbook, ship’s bell,
sea chart and lifebuoy) and Hedda’s apron are in `assets/models/decor/`. Their dimensions
and replacement prompts are listed under *Keepsakes*.

The shop's three junk piles, waiting bench, door lantern,
herb sachets, ferry contract and wooden way board are in `assets/models/shop/`.
The painted return post for the quay and market is in `assets/models/harbor/signpost.glb`.
The seven market awnings carry embroidered trade emblems, and the quay has a storm lantern.
The carpenter's rolled building plan is in `assets/models/market/plan.glb`.
The four village façades, harbour hut, rowboat, ferry hull, shop panelling and counter are
finished. The village pump and flower tubs are in `assets/models/props/`.

To check a fresh checkout, pull `main` and run `python3 tools/check_models.py`. A `missing` line
identifies an absent asset. The sections below retain the dimensions, pivots and prompts for
replacements.

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
  or one baked colour texture per model (≤ 1024 px). Embed baked colour textures as PNG
  so the game can load them. Procedural node textures do **not** export.
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
| `shelf` | 0.9 × 1.5 × 0.45 | A small wooden shop shelf, old dark wood, open front, three boards at 0.35 m, 0.75 m and 1.15 m height (products are placed on the middle and the top board), a little carved lace trim along the top. |
| `small_shelf` | 0.6 × 0.9 × 0.35 | A small, simple wooden shelf on short legs with **one board at 0.55 m** where a product stands, slightly crooked, a few nail heads. |
| `display_table` | 0.9 × 0.8 × 0.6 | A low shop display table with turned legs and a dark green cloth on top (**top at 0.8 m**, two products stand on it side by side). |
| `tall_bookcase` | 0.9 × 2.0 × 0.4 | A tall, narrow bookcase of dark wood with a carved crown, open front, **boards at 0.45, 0.95 and 1.45 m** (one product on each), a few old books lying at the sides. |
| `glass_cabinet` | 0.8 × 1.7 × 0.45 | An elegant glass display cabinet with a brass frame and glass doors (the glass a pale, slightly transparent-looking material), **glass shelves at 0.5, 0.95 and 1.4 m**. |
| `folding_screen` | 1.0 × 1.6 × 0.3 | A three-panel folding screen of dark wood with faded dusty-rose silk panels painted with moths and moons, standing slightly zig-zagged. |
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
| `workbench` | 1.0 × 0.95 × 0.6 | A sturdy little wooden workbench with a vice on one end, a few tools (hammer, screwdriver, small pliers) and a tin of screws on top, a half-repaired music box waiting in the middle; a pegboard is not needed. |
| `fairy_lights` | 0.9 × 0.6 × 0.3 | A small wooden frame strung with warm fairy lights in loops, little glass bulbs glowing amber (emissive). |
| `fur_rug` | 0.95 × 0.03 × 0.95 | A soft, round cream-and-grey fur rug, slightly shaggy at the edge. |
| `flower_pot` | 0.5 × 0.8 × 0.5 | A big glazed terracotta pot with a bushy flowering plant, pink and white blossoms. |
| `birdhouse` | 0.4 × 1.4 × 0.4 | A painted wooden birdhouse with a little round hole and a tiny roof, on a slim post. |

## Keepsakes (gifts from the story ghosts) – `assets/models/decor/<id>.glb`

Placed like furniture on one floor cell; origin at the bottom centre, front towards −Y.

| id | Size (w × h × d, m) | Prompt |
|---|---|---|
| `umbrella` | 0.35 × 0.9 × 0.35 | A closed black umbrella with a curved wooden handle standing in a small brass umbrella stand. |
| `teacup` | 0.5 × 0.7 × 0.5 | A tiny round side table with a floral porcelain teacup and saucer on top; the cup has a small chip on the rim. |
| `blanket` | 0.8 × 0.9 × 0.8 | A small cosy armchair in faded green velvet with a hand-knitted rust-and-cream blanket draped over the backrest. |
| `wedding_photo` | 0.5 × 0.9 × 0.4 | A small wooden easel holding an old sepia photograph in a gilded frame (the photo shows two blurry figures dancing, no faces needed). |
| `pocket_lantern` | 0.4 × 1.2 × 0.4 | A short wooden post with a small brass lantern hanging from a hook at the top; warm emissive glass panes. |
| `logbook` | 0.5 × 1.0 × 0.4 | A small wooden lectern holding a thick open lighthouse logbook with a cracked leather cover; a pencil on a string, a few loose pages sticking out. |
| `ship_bell` | 0.5 × 1.3 × 0.4 | A polished brass ship's bell hanging from a small wooden gallows on a square foot; a short braided rope hangs from the clapper. The bell is part of the model and does not move. |
| `sea_chart` | 0.7 × 1.2 × 0.3 | A wooden easel holding a framed, yellowed sea chart of a small harbour with a lighthouse; a tiny ghost doodle in one corner. |
| `lifebuoy` | 0.6 × 1.1 × 0.3 | A red-and-white striped ring lifebuoy hanging on a weathered wooden post, a coil of rope looped over the top. |
| `apron` | 0.5 × 1.6 × 0.5 | A slim wooden coat stand with a faded lavender shop apron hanging on it; the apron has a pocket with a small sprig of dried lavender peeking out. |
| `chalk_drawing` | 0.5 × 0.9 × 0.3 | A small wooden easel holding a child's chalk drawing on dark paper: a fluffy little dog with a bell on its collar, drawn with wobbly white and yellow lines. |
| `dog_basket` | 0.7 × 0.35 × 0.5 | A round wicker dog basket with a soft knitted blanket inside and a little bell lying on the rim. |
| `cog_wheel` | 0.6 × 0.8 × 0.3 | A large, old brass cog from a tower clock, leaning upright in a simple wooden stand; worn teeth, a bit of patina. |
| `cuckoo_clock` | 0.45 × 1.6 × 0.3 | A carved wooden cuckoo clock hanging on a slim wooden post; small roof, pine cones on chains, the little door shut. Static – the game moves nothing on it. |
| `seedling_pot` | 0.4 × 0.6 × 0.4 | A small clay pot on a little wooden stool with a fresh green seedling, two round leaves and a bud; a few crumbs of earth on the stool. |
| `flower_box` | 0.8 × 0.5 × 0.3 | A painted wooden flower box on short legs, overflowing with pink, yellow and violet spring flowers and trailing ivy. |
| `nameplate` | 0.6 × 0.9 × 0.2 | A weathered little wooden board on a post, painted by hand with a simple rowing boat and a lantern; no lettering. |
| `oar` | 0.3 × 1.8 × 0.3 | The ferryman's old punting pole, dark and worn smooth, standing upright in a small iron stand with a rope wound round its top. |
| `toy_sledge` | 0.7 × 0.3 × 0.35 | Lotte's little wooden toy sledge with curved runners, a red-painted seat with a worn flower pattern and a short rope to pull it; one runner freshly mended with a small brass plate. |

## Mishaps – `assets/models/incidents/<id>.glb`

Small things that go wrong during the night (`assets/data/incidents.ron`); the player clicks
them to fix them. Origin at the bottom centre, front towards −Y. Static models – the game makes
the crow hop, the leaves swirl and the puddle shimmer.

| id | Size (w × h × d, m) | Prompt |
|---|---|---|
| `draught` | 0.45 × 0.25 × 0.2 | Three dry autumn leaves (orange, brown, ochre) caught in a little swirl of wind, floating just above a window sill; no sill, only the leaves and a faint curl of pale wind ribbon. |
| `crow` | 0.3 × 0.32 × 0.4 | A small, glossy black crow standing on two feet, beak half open as if cawing, head slightly cocked, wings folded; friendly, a little scruffy. |
| `leak` | 0.6 × 0.04 × 0.6 | A flat, irregular puddle of water on the floor with two small ripple rings and a single drop splashing in the middle. |

## Errand clues – `assets/models/errands/<errand>_clue_<n>.glb`

Tracks for the lost-and-found errands (`assets/data/errands.ron`), lying on the quay or the
market square. Origin at the centre on the ground, front towards −Y, flat and static – the
game adds the pulsing glow that makes them easy to spot.

| id | Size (w × h × d, m) | Prompt |
|---|---|---|
| `lotte_sledge_clue_1` | 1.0 × 0.08 × 0.5 | A short trail in a thin layer of snow on stone: small dog paw prints beside the two narrow parallel grooves of sledge runners, curving slightly. |
| `lotte_sledge_clue_2` | 1.0 × 0.08 × 0.5 | Runner grooves and paw prints in snow ending at a little heap of snow with a dropped red knitted mitten on top. |

### The tower clock (season 2)

The clockmaker's story sets it going chapter by chapter; the game turns the hands.

| id | Size (w × h × d, m) | Prompt |
|---|---|---|
| `clock_tower` | 1.2 × 4.2 × 1.2 | A slim old stone clock tower with a pointed slate roof, a round clock face **without hands** centred at **2.75 m** height on the front (−Y), Roman numerals, a little balcony rail below the face. Origin at the bottom centre. |
| `clock_hand_hour` | 0.06 × 0.22 × 0.02 | The short, ornate black iron hour hand, pointing up along +Z (Blender), origin at its **pivot (bottom end)**. |
| `clock_hand_minute` | 0.04 × 0.32 × 0.02 | The long, slim black iron minute hand, pointing up, origin at its **pivot (bottom end)**. |
| `window_flowers` | 0.7 × 0.35 × 0.2 | A wooden window box full of spring flowers (pink, yellow, violet), hung under a house window; origin at the bottom centre, front −Y. Five of them appear on the houses as spring comes. |

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

Season 2 products:

| id | Prompt |
|---|---|
| `gear` | A single shiny brass gear wheel lying flat, with a smaller gear leaning against it. |
| `clock_key` | An ornate brass clock-winding key with a heart-shaped bow, on a short ribbon. |
| `little_bell` | A small silver bell on a red ribbon tied in a bow. |
| `leash` | A coiled red dog leash with a brass clip and a looped handle. |
| `snow_globe` | A snow globe on a wooden base with a tiny house and a pine tree inside, a few white flakes floating. |

Season 3 products:

| id | Prompt |
|---|---|
| `seed_packet` | A small paper seed packet with a flower drawn on it, folded at the top, a few seeds spilled beside it. |
| `watering_can` | A little green tin watering can with a long spout and a rose on the end. |
| `storm_lantern` | A sturdy iron storm lantern with a glass chimney and a carrying handle; the flame glows warm (emissive). |

## Shop – `assets/models/shop/<id>.glb`

| id | Size (w × h × d, m) | Prompt |
|---|---|---|
| `counter` | 4.5 × 1.2 × 0.81 | A long walnut counter with the customer side facing −Y. Set its surface at 0.742 m. Inlay the panelled front with a ghost and brass corners. Give the teal counting mat three stitched coin circles. At the right end fit an antique register with an ivory roller, two rows of keys and a drawer pull. Keep space for the loose bell, ledger and coin jars. |
| `postcard_stand` | 0.2 × 0.04 × 0.07 | A small, low wooden card stand with a slot along its top, slightly worn, a brass corner on each end. Origin at the **centre of its base**, slot along X. The game puts the postcard into the slot and turns it. |
| `door` | 1.1 × 2.3 × 0.2 | Keep the carved walnut jamb, lintel and brass bell bracket fixed. Inside the jamb, show a narrow layered view of the moonlit river, with a moored sailboat, a lantern and lit warehouse windows. The original pane, panels and fittings belong to the moving leaf. |
| `door_leaf` | 0.90 × 2.024 × 0.15 | Original plank leaf with bevelled rails, rain-streaked round glass, hinge straps and brass handle. Put the hinge on the far jamb, with the origin at its foot and the leaf extending along −X in glTF. It swings into the shop and clears the view of the quay. |
| `door_bell` | 0.11 × 0.105 × 0.11 | Keep the old cast brass shop bell, clapper and suspension stem together. Its origin is at the top of the stem, with the bell below. |
| `window_drapery` | 2.46 × 1.03 × 0.30 | A dark brass rail with rings above two gathered petrol-blue curtains. Stitch a thin gold border down each fold and leave the window centre open. Centre the origin on the rod, front towards +Z in glTF. Tie points at x ±0.973, y −0.945. |
| `curtain_hem` | 0.41 × 0.89 × 0.14 | Pleated cloth below one tieback, with a gold-edged scallop and tassels. Put the origin at the top knot, with the hem below it and the finished face towards +Z. The game swings a copy from each tie. |
| `room` | see below | *(optional)* The walls of the shop, replacing the plain placeholder walls. |
| `wall_clock` | 0.78 × 0.78 × 0.08 | A round wall clock in a dark wooden frame with a cream face and twelve tick marks, **no hands** (the game adds and turns them). Origin at the **centre of the back** of the clock, face towards −Y. |
| `way_board` | 0.8 × 0.24 × 0.05 | A weathered wooden direction board with one pointed end and two old nails, **no lettering** (the game writes "To the harbour" and "To the market" on it). The game mounts two of these flush with the left wall immediately beside the doorframe and paints the anchor or market awning beside the localised wording. Keep the broad front face flat at +0.039 m in glTF Z. Origin at the **centre of the back** of the board, front towards −Y. |
| `shop_sign` | 1.0 × 0.75 × 0.05 | The hanging "closed / open" sign for the shop window: a painted wooden board 0.98 × 0.44 m in a thin frame, hanging from two cords that meet 0.3 m above it. The **front (−Y) is painted dark red**, the **back (+Y) green**, both with a thin cream border and **no lettering** (the game writes "closed"/"open" in the player's language). Origin at the **centre of the board**. The game paints each side separately and flips the entire sign. Keep the red and green faces flat at +0.025 and −0.025 m in glTF Z. |

**Room layout** (only if you make `room.glb`; Blender coordinates, Z up, origin at 0,0,0, do not
apply an offset): the floor is **not** part of the room model (the game draws the floor grid).

- Back wall: an inner face at **Y = +3.0**, from X = −7.0 to X = +7.5, height 3.4 m.
- Left wall: an inner face at **X = −7.0**, from Y = −3.5 to Y = +3.0, height 3.4 m; leave a
  1.2 m wide, 2.4 m high opening centred at **Y = 0** for the door.
- A tall window in the back wall centred at **X = −3.2**, bottom edge at 0.9 m, 1.6 × 1.8 m,
  with frame and emissive moonlit glass; dark wallpaper with a faded pattern, wooden wainscot up
  to 0.9 m, a few cobwebs in the upper corners. Run a teal frieze with gilt willow leaves
  along both walls at 3.0 m. Hang the framed ferry and crescent to the right of the wall shelf.
- Leave the right side and the front open (the camera looks in from the front).

## Shop improvements – `assets/models/shop/<id>.glb`

Improvements are bought at the village market (`assets/data/upgrades.ron`). Hedda’s old cargo
blocks part of the starting floor. The three junk piles share the room with four hinged
storage cases. Each sits at a slight angle and keeps its place until the carpenter clears
its tile. The other improvements stand in the shop once bought. The game adds the lantern’s light.

Origin at the **bottom centre**; the front faces **−Y** (towards the camera for things on the
back wall; the game turns the wall pieces on the left wall towards the room).

| id | Size (w × h × d, m) | Prompt |
|---|---|---|
| `junk_1` | 0.85 × 0.75 × 0.85 | A pile of old junk filling one floor tile: two stacked dusty wooden crates under a grey dust sheet, a cobweb between them. |
| `junk_2` | 0.85 × 0.75 × 0.85 | A pile of old junk: a broken chair upside down on a small trunk, a rolled carpet leaning against it, dust. |
| `junk_3` | 0.85 × 0.75 × 0.85 | A pile of old junk: a covered birdcage, a stack of old books and a cracked flower pot on a low box, all under a thin layer of dust. |
| `waiting_bench` | 1.4 × 0.55 × 0.45 | A worn little wooden bench with a curved backrest and two embroidered cushions, standing against the back wall. |
| `door_lantern` | 0.3 × 0.45 × 0.3 | A small brass wall lantern with glass panes, its glass front towards −Y and a wrought-iron bracket at the back (+Y) that holds it against the wall; a candle inside with an emissive flame. Mount above the entrance lintel. |
| `herb_sachets` | 1.0 × 0.5 × 0.15 | Four bundles of dried herbs and small linen sachets tied with string, hanging from a short wooden rail; lavender, sage and rosemary colours. |
| `ferry_contract` | 0.5 × 0.65 × 0.05 | A framed old contract on yellowed paper with ornate handwriting (no readable words), a red wax seal and a tiny drawing of a boat, in a thin dark wooden frame. Mount above the direction boards beside the shop door. |

### Hedda’s storage cases (delivered)

Each case has a separate `<id>_lid.glb`. The body stands on the centre of its footprint. The lid begins at the rear centre of the rim and extends forwards (+Z in the game). Closed body and lid dimensions are listed separately in `assets/data/workbench.ron`. Keep decoration large enough to read at the normal shop camera distance. The engine opens the lid and presents the ware the player found.

| Model | Body dimensions (W × H × D, m) | Art direction |
|---|---|---|
| `post_case` | 0.76 × 0.32 × 0.48 | Low terracotta dispatch case with brass corner stays. Tie a large cream envelope to the lid, fold two dark lines towards a red wax seal and stack old letters inside. |
| `sea_trunk` | 0.80 × 0.36 × 0.50 | Sailor’s sea-green plank trunk with a barrel-vaulted lid. Two leather straps follow the curve. Use an oversized brass keyhole and brass carrying handles. |
| `bottle_hamper` | 0.56 × 0.54 × 0.47 | Tapered wicker carrier. Leave the upper weave open around three stoppered apothecary bottles of different colours. Put the arched carrying handle on the woven lid. |
| `hat_box` | 0.61 × 0.40 × 0.61 | Round plum milliner’s box with a pale hatband, lid piping and a dark lining. Two hollow ribbon loops and loose ends make a recognisable bow on top. |

## The ferryman – `assets/models/shop/<id>.glb`

In the evening the player walks out to the harbour, where the ferryman sells crates from his boat.

| id | Size (w × h × d, m) | Prompt |
|---|---|---|
| `ferry_boat` | 7.4 × 0.66 × 1.85 | A low, flat-bottomed wooden ferry with a raised bow and stern. Run two faded coachlines along the boards, hang rope fenders outside, and set a small brass capstan at the bow. Keep the cargo deck clear for the ferryman, lamp and clickable crates. Origin at the centre of the deck surface, with the hull extending 0.4 m below it. |
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
| `harbour_hut` | 2.6 × 2.9 × 2.1 | The harbour master's plank hut with a shingled roof, left-hand door, amber window and a lantern 1.75 m up. Hang a ship's wheel over the entrance, mount a striped tide staff by the door, and set a flower box below the window. Keep the wall rope and lifebuoy. |
| `lighthouse` | 1.2 × 4.3 × 1.2 | A small, stout lighthouse on a round stone base: tapering tower with white and dusty-red bands, a little iron gallery, a glass lamp room **centred at 3.6 m height** glowing pale gold (emissive), a dark conical roof. The game dims the emissive glass while the story keeps the lighthouse dark. |
| `lamp_post` | 0.4 × 2.7 × 0.4 | An old cast-iron harbour lamp post with a square glass lantern on top, the glass **centred at 2.45 m** and glowing warm amber (emissive). |
| `quay_lantern` | 0.3 × 2.2 × 0.3 | An old, slightly crooked cast-iron lantern post for the quay, origin at the **centre of its foot**, with a small square lantern housing whose glass is **centred at 1.95 m**. Leave the glass plain and **not emissive**, with a little soot and rust, because the game puts its own glow and light inside when the player restores the lantern. Add a ladder bar under the housing for the lamplighter. |
| `village_house` | 2.2 × 2.4 × 1.8 | A narrow, crooked old fisherman's house in dark stone and timber with a steep roof and a chimney, one small window glowing amber (emissive) on the front. Seen from afar behind the harbour; used several times, so keep it plain. |
| `barrel` | 0.6 × 0.72 × 0.6 | An old wooden barrel with iron hoops. |
| `mooring_line` | 1.0 × 0.16 × 0.045 | Three strands of warm hemp with a shallow hanging curve. Start at the origin and finish at (1, 0, 0) in glTF. Both ends remain level. The game fits the span between a cleat and its bollard. |
| `bank_windmill` | 1.70 × 2.86 × 1.76 | A limewashed round mill with exposed bricks, a plank door, timber gallery and a slate cap. Origin at the bottom centre. Face the door and the axle towards +Z in glTF. The axle meets the sails at (0, 2.08, 0.92). |
| `windmill_sails` | 2.47 × 2.47 × 0.30 | Four tapered lattice sweeps, each carrying a narrow, stitched strip of linen. Rivet a timber cross to the hub. Centre its origin on the mill axle and spread the sails in the XY plane so they turn about Z. |
| `riverbank` | 36.0 × 0.66 × 1.57 | A long verge with interrupted courses of dry stone, uneven grass and reeds. Centre the origin along the verge at paving height. Face the exposed stones towards +Z and run the wall along X. |
| `cork_fender` | 0.34 × 0.51 × 0.31 | A pear-shaped cork fender covered in a diamond net, rope collars and a dark middle binding. Hang from a narrow rope eye at the origin. The cork extends down along glTF −Y. |
| `mooring_post` | 0.3 × 0.6 × 0.3 | A short weathered wooden mooring post (bollard) with a coil of thick rope around it. |
| `bell_post` | 0.6 × 1.95 × 0.2 | A wooden post with its origin at the foot. A short arm sticks out to the right (+X), with a hook **0.38 m right of the post, 1.85 m high** for the separate bell. |
| `ferry_bell` | 0.25 × 0.22 × 0.25 | A small, slightly tarnished brass bell with a clapper and a ring at the top. Origin at the **top of the ring** (where it hangs), the bell below it, so the game can swing it. |
| `hand_cart` | 2.0 × 0.75 × 1.0 | A wooden two-wheeled hand cart: a flat loading bed of planks **0.42 m high, 1.2 × 0.8 m**, with a low rim, two spoked wheels under the middle, two long handles reaching out to the left (−X). The origin is on the ground below the centre of the bed where the game stacks bought crates. |
| `signpost` | 0.95 × 1.95 × 0.2 | A crooked wooden signpost with its origin at the foot and two arrow-shaped boards pointing to the right, with broad flat faces centred at 1.64 m and 1.22 m high. In glTF the upper face is at Z +0.098 m and the lower at +0.091 m. Carve a few long grain lines and a worn gilt chevron into the arrow tips. Leave the wood unlettered for the game’s painted lantern, shop crest and localised return directions. |
| `rowboat` | 0.8 × 0.45 × 2.0 | A small wooden rowboat, length along Y, with two seats, crossed oars and a rolled canvas bundle lashed with rope. |
| `jetty` | 1.1 × 0.7 × 3.8 | A narrow wooden jetty: a deck of planks running along the Y axis (3.8 m long, 1.0 m wide) on four posts. Origin at the **centre of the deck surface**; the posts reach about 0.6 m below it. |
| `paper_lantern` | 0.26 × 0.34 × 0.26 | A small floating paper lantern on a tiny round wooden raft: a box of warm paper glowing amber (emissive). |

### Village crab apples

The trees stand on the market road and the dry side of the quay. Use the same crooked grey-brown wood for the cut sprays in the shop. Keep the spaces between twigs open against the painted rooftops. Dimensions below are width × height × depth in metres, in glTF coordinates (+Y up). The game bends each bough at its graft and carries the seasonal layer with it.

| File | Size (m) | Replacement design and origin |
| --- | --- | --- |
| `harbor/apple_trunk.glb` | 1.48 × 1.81 × 1.48 | Low forked trunk with flared roots and an oval pruning scar, set in a segmented stone surround with cast-iron ribs over the earth. Origin at the foot. Keep the five graft sockets at the positions in `src/scene/orchard.rs`. |
| `harbor/apple_bough.glb` | 1.24 × 1.04 × 0.91 | A tapered fruiting branch growing towards +X and upwards, with short spurs and closed copper buds. Put its graft at the origin. |
| `harbor/apple_autumn.glb` | 1.36 × 0.65 × 1.25 | Attach folded ochre and olive leaves and a handful of small russet crab apples to the bough. Use the same graft origin for all three seasonal layers. |
| `harbor/apple_snow.glb` | 1.17 × 0.97 × 0.90 | Broken ridges of blue-shadowed snow resting on the upper surfaces of the wood. Leave the forks visible. |
| `harbor/apple_blossom.glb` | 1.26 × 0.69 × 1.16 | Clusters of ivory and blush five-petalled flowers with ochre centres and small sage-green leaves at the twig tips. |
| `harbor/apple_leaf.glb`, `harbor/apple_petal.glb` | 0.17 × 0.06 × 0.12 and 0.095 × 0.021 × 0.058 | One creased ochre leaf and one cupped pale petal, each with its origin near the centre and faces visible from both sides. |
| `shop/apple_jug.glb` | 0.60 × 0.75 × 0.53 | A heavy teal salt-glaze pitcher with a curved handle, a small pouring lip and a cream slip-painted leaf motif. Origin at the foot. Leave the neck open for cuttings at 0.715 m. |

## Containers on the boat – `assets/models/shop/<id>.glb`

The ferryman ships every product in one of four containers (`container` in
`assets/data/products.ron`). Each container is two models: the body, **open at the top and filled
with straw almost to the rim**, and its lid. The game opens and shuts the lid, so the lid's origin
is the **middle of its back edge** (the hinge) and the lid extends forwards (−Y in Blender), lying
flat. Origin of the body: bottom centre.

| id | Size (w × h × d, m) | Prompt |
|---|---|---|
| `chest` | 0.5 × 0.28 × 0.36 | A small wooden treasure-style chest of dark red-brown wood with brass corner fittings and two brass bands, a little brass lock plate on the front. |
| `chest_lid` | 0.52 × 0.06 × 0.38 | The matching flat lid with brass edges and two brass bands. |
| `basket` | 0.44 × 0.24 × 0.44 | A round woven wicker basket, honey coloured, with two small handles. |
| `basket_lid` | 0.46 × 0.05 × 0.46 | Its round, slightly domed woven lid. |
| `keg` | 0.38 × 0.34 × 0.38 | A small wooden keg standing upright, dark iron hoops. |
| `keg_lid` | 0.4 × 0.04 × 0.4 | Its round wooden lid of three planks. |

## Village houses – `assets/models/harbor/<id>.glb`

Four different houses for the village behind the harbour, each **2.2 × 2.4 × 1.8 m** (the game
scales them a little), seen from afar at night; keep them simple, with one or two small windows
glowing amber (emissive) on the front.

| id | Prompt |
|---|---|
| `village_house_1` | A crooked grey fisherman's cottage with courses of slate, sea-green shutters, ivy and a window box of violets. |
| `village_house_2` | A leaning plaster and timber house with slate courses, crossed braces, shutters and flowers on the balcony. |
| `village_house_3` | A little bakery with a clay-tiled roof, a striped awning, three scored loaves in its window and a gilded ear of wheat on the gable. |
| `village_house_4` | A small chapel with a pointed bell tower, a stained-glass rose window, dressed stone and aged brass around the opening. |

## Props in the shop – `assets/models/props/<id>.glb`

Decoration only; the positions are in `assets/data/props.ron` (several entries can use one model).
Origin at the **bottom centre**, front facing −Y. Items on the counter or a shelf stand on that
surface; items on the wall hang with their back against it (back at +Y in Blender).

| id | Size (w × h × d, m) | Prompt |
|---|---|---|
| `counter_candle` | 0.14 × 0.26 × 0.14 | A thick cream candle in a brass holder with a finger loop, wax drips, the flame glowing (emissive) at about 0.26 m. |
| `change_tray` | 0.64 × 0.052 × 0.256 | A low chamfered walnut tray with a teal felt liner divided into three pockets. Add a stitched inner seam, brass corner patches and dark pins. Origin at the bottom centre, long edge along X. The register illustration uses a view of this same model. |
| `ledger_book` | 0.4 × 0.06 × 0.3 | An open old ledger book with yellowed pages and a red ribbon bookmark, a quill pen in a tiny inkwell beside it. |
| `button_jar` | 0.16 × 0.22 × 0.16 | A glass jar full of colourful old buttons, cork lid. |
| `brass_scale` | 0.4 × 0.32 × 0.22 | An old brass balance scale with two small pans. |
| `potted_succulent` | 0.18 × 0.22 × 0.18 | A tiny succulent in a chipped teacup used as a pot. |
| `wall_shelf` | 1.4 × 0.3 × 0.26 | A short wooden wall shelf on two carved brackets (it hangs on the wall; the items on it are separate). |
| `glass_jar` | 0.16 × 0.24 × 0.16 | An apothecary glass jar with a ground-glass stopper and a blank paper label, filled with dried petals. |
| `potion_bottles` | 0.35 × 0.28 × 0.14 | Three small odd-shaped glass bottles with corks, softly coloured liquids (one faintly glowing). |
| `picture_frame` | 0.62 × 0.76 × 0.05 | A gilded oval frame with a faded painted portrait of a lady in an old-fashioned dress. |
| `picture_frame_small` | 0.42 × 0.5 × 0.05 | A small dark wooden frame with a painted landscape of a moonlit lake. |
| `herb_bundle` | 0.26 × 0.5 × 0.14 | A bundle of dried herbs and flowers hanging upside down from a nail with a string. |
| `button_sack` | 0.5 × 0.55 × 0.45 | A bulging burlap sack, slightly open, buttons spilling from the top. |
| `stacked_boxes` | 0.7 × 0.85 × 0.55 | Two old cardboard and wooden boxes stacked, a ribbon and some paper sticking out. |
| `potted_palm` | 0.6 × 1.0 × 0.6 | A slightly drooping parlour palm in a big brass pot. |
| `coat_stand` | 0.5 × 1.8 × 0.5 | A bentwood coat stand with a long scarf and a bowler hat hanging on it. |
| `broom` | 0.3 × 1.4 × 0.2 | An old straw broom leaning back a little. |

## Village street furniture – `assets/models/props/<id>.glb`

| id | Size (w × h × d, m) | Prompt |
|---|---|---|
| `town_pump` | 0.95 × 1.37 × 0.67 | A green cast-iron village pump with a curved handle and brass spout over a low stone trough. Centre the origin on the ground, face the trough towards −Y. |
| `village_planter` | 0.54 × 0.54 × 0.48 | A small terracotta pot with a gold rim, filled with leaves and cottage flowers. Origin at the bottom centre. The same pot fits beside the market pump and outside the harbour hut. |

## Props at the harbour – `assets/models/props/<id>.glb`

| id | Size (w × h × d, m) | Prompt |
|---|---|---|
| `crate_stack` | 0.95 × 0.95 × 0.7 | Three weathered wooden crates stacked unevenly, one with a rope around it. |
| `storm_lantern` | 0.27 × 0.49 × 0.22 | A portable green enamel storm lantern with a low brass oil reservoir, an amber chimney, crossed wire guards and an arched carrying handle. Origin at the centre of the foot, flame 0.20 m up. It stands on the western mooring post. |
| `fishing_net` | 1.3 × 1.2 × 0.4 | A fishing net draped over a simple wooden drying frame, a few cork floats hanging from it. |
| `lobster_trap` | 0.6 × 0.4 × 0.45 | A wooden lobster pot with a rounded top of slats and netting. |
| `anchor` | 0.7 × 1.0 × 0.3 | An old rusty iron anchor leaning on its side, a bit of chain. |
| `oars` | 0.3 × 1.6 × 0.15 | A pair of wooden oars leaning upright side by side. |
| `fish_basket` | 0.5 × 0.35 × 0.5 | A round wicker basket with a few silvery fish inside. |
| `rope_pile` | 0.6 × 0.22 × 0.6 | A thick coil of ship's rope lying flat. |
| `bench` | 1.3 × 0.5 × 0.45 | A worn wooden harbour bench with a backrest. |

## Animals – `assets/models/critters/<id>.glb`

The game moves the animals (breathing, tail, head turns, flying, jumping), so moving parts are
separate models whose **origin is the pivot**. Bodies: origin at the bottom centre unless noted.
Night colours, cute and simple, no rigs or animations.

| id | Size (w × h × d, m) | Prompt |
|---|---|---|
| `cat_sleeping` | 0.42 × 0.2 × 0.32 | A ginger cat curled up asleep, tail wrapped round, eyes closed. |
| `cat_sitting` | 0.28 × 0.42 × 0.26 | A black cat sitting upright, front facing −Y, green-yellow eyes glowing faintly (emissive), **without its tail**. |
| `cat_tail` | 0.06 × 0.32 × 0.08 | The black cat's tail, hanging down with a slight curl at the tip. Origin at its **root (top end)**. |
| `seagull` | 0.22 × 0.3 × 0.36 | A plump grey-and-white seagull standing, yellow beak, front facing −Y. |
| `owl_body` | 0.24 × 0.3 × 0.22 | A small brown owl's body sitting, **without its head**, feet at the bottom. |
| `owl_head` | 0.2 × 0.18 × 0.18 | The owl's round head with ear tufts and big amber eyes (emissive), facing −Y. Origin at the **neck (bottom centre)**. |
| `bat_body` | 0.07 × 0.06 × 0.12 | A tiny dark bat's body with ears, **without wings**, head towards −Y. Origin at the **centre**. |
| `bat_wing` | 0.12 × 0.02 × 0.08 | One bat wing, spread flat out to the right (+X). Origin at the **shoulder (its left end)**; the game mirrors it for the other side. |
| `fish` | 0.08 × 0.1 × 0.26 | A small silvery fish, length along the Y axis, head towards −Y. Origin at the **centre**. |
| `dog_body` | 0.5 × 0.36 × 0.24 | Pfote, a small fluffy ghost dog lying down with its head up, pale blue-white and slightly translucent-looking, a little bell on its collar, **without its tail**; head towards +X. |
| `dog_tail` | 0.06 × 0.16 × 0.06 | Pfote's fluffy tail, pointing up and back. Origin at its **root (bottom end)**. |
| `butterfly_body` | 0.03 × 0.03 × 0.08 | The slim body of a small spring butterfly with two curled antennae, **without wings**, head towards −Y. Origin at the **centre**. |
| `butterfly_wing` | 0.12 × 0.01 × 0.1 | One butterfly wing (the right one), soft apricot with a cream edge and two small violet eye spots, lying flat. Origin at its **hinge on the body**, the wing spreading along +X; the game mirrors it for the left wing and beats both. |

## The village market – `assets/models/market/<id>.glb`

By day the player visits a market of seven regular traders of the other side, joined by
Ilse Icicle in winter and Bluma in spring (`assets/data/market.ron`).
Each has a **stall** and the **trader** standing beside it (the game moves them: breathing, a
nod after a sale, the wisp floating, the raven hopping). The game puts small samples of the wares
on the stall's table, so the table top stays **empty** at **0.85 m** height. Stalls: origin at the
bottom centre, front (the customer side) towards −Y, table about 1.7 × 0.7 m, two posts at the
back carrying an awning at about 2.2 m. The cloth follows its folds and bears an embroidered
trade emblem edged with old gold. In table order the emblems are a hand plane, three candles,
a herb sprig, stained glass, a weaving shuttle, scales and an hourglass. Traders stand on
their origin and face −Y.

| id | Size (w × h × d, m) | Prompt |
|---|---|---|
| `button_paving` | 3.20 × 0.11 × 3.20 | A flush circular pavement of worn limestone setts, a laurel ring in green and ochre tile, and four dark buttonholes joined by two short brass stitches. Origin at the ground centre, face up (+Y in glTF). |
| `shopfront_walk` | 13.60 × 0.17 × 1.31 | Three staggered courses of flagstones with capped kerbs, drain grilles and three shallow entrance steps. Ground-centred, running along X with the steps facing the square (+Z in glTF). |
| `square_bench` | 1.87 × 0.95 × 0.59 | Slatted green wooden seat with cast scroll ends, brass handrails, a small button on the back and four stone feet. Origin at the ground centre, seat facing +Z. |
| `herb_border` | 3.98 × 1.09 × 0.74 | A low dressed-stone herb bed between two capped piers. Open iron hoops rise behind silver sage and rosemary. Ground-centred, long axis X, planting facing +Z. |
| `gable_sail_loft` | 1.74 × 1.88 × 1.21 | Teal shingle courses over a timber sail loft with a compass roundel. Include the fixed copper spindle ending at (0, 1.88, −0.04) for the separate vane. |
| `gable_cloth_hall` | 1.68 × 1.80 × 1.31 | Stepped brick gable with pale coping, double arched windows and a projecting loft pulley. |
| `gable_bakehouse` | 1.87 × 1.62 × 1.18 | A broad clay-tiled dormer with a flared eave, an amber fanlight and three gilded ears of wheat. |
| `gable_bell_lodge` | 1.28 × 2.03 × 1.00 | Open stone belfry with a hanging brass bell, an octagonal verdigris cap and a narrow finial. Keep the arches open to the sky. |
| `gable_stationer` | 1.73 × 1.71 × 1.18 | Plum slate over three narrow windows, timber braces and an open painted book in the peak. |
| `fish_vane` | 1.06 × 0.31 × 0.09 | A cut copper fish on an arrow, worked on both sides. Origin at the bottom swivel, +Y up, arrow pointing +X. The game turns it in the village breeze. |
| `shopping_basket` | 1.5 × 1.02 × 0.94 | Open willow trug with tapered sides, a wrapped bentwood handle, copper rivets and a madder-and-cream checked napkin draped over the front lip. Origin at the bottom centre. Keep room inside for purchases. |
| `bunting_line` | 14.01 × 3.35 × 0.21 | Two dark timber poles with collars and finials, strung with a thin hemp line. Origin at ground level halfway between them. In glTF run the line from (−6.9, 3.10, 0) to (6.9, 3.10, 0) with a 0.55 m sag at its centre. The game attaches the cloth separately. |
| `wrapped_parcel` | 0.40 × 0.245 × 0.30 | A small kraft-paper parcel with folded ends, string in both directions and a wax-sealed paper slip. Origin at the bottom centre, front facing +Z in glTF. |
| `sold_ticket` | 0.30 × 0.224 × 0.16 | A little standing envelope on a brass foot, closed with red wax. Face it towards the customer (+Z in glTF) and place the origin at the bottom centre. No lettering. |
| `stall_carpenter` | 1.9 × 2.5 × 1.2 | A carpenter's market stall: a sturdy workbench table, a saw and planks leaning on the side, wood shavings, a sand-brown canvas awning. |
| `stall_candlemaker` | 1.9 × 2.5 × 1.2 | A candle maker's stall: a table with a rack of hanging candles on the side, dripped wax, a honey-yellow awning. |
| `stall_herbalist` | 1.9 × 2.5 × 1.2 | A witch's herb stall: bundles of herbs hanging from the awning frame, a small bubbling cauldron beside the table (not on it), a moss-green awning. |
| `stall_glazier` | 1.9 × 2.5 × 1.2 | A vampire glazier's stall: a dark lacquered table, a tall covered mirror on a stand beside it, a deep red velvet awning with gold fringe. |
| `stall_weaver` | 1.9 × 2.5 × 1.2 | A weaver's stall: rolled rugs and bolts of cloth leaning against the table, a small loom at the side, a dusty-rose awning. |
| `stall_raven` | 1.9 × 2.5 × 1.2 | A money changer's stall: a narrow counter with a brass coin scale and stacked trays, a slate-blue awning. |
| `stall_reaper` | 1.9 × 2.5 × 1.2 | The Reaper's stall: a black-draped table with a lace cloth edge, a few old clocks and a candelabra at the sides, a ragged black awning. |
| `vendor_carpenter` | 0.7 × 1.7 × 0.6 | Bones the carpenter: a friendly skeleton in a leather apron with a pencil behind where his ear would be, a folding rule in his hand. |
| `vendor_candlemaker` | 0.5 × 1.6 × 0.5 | Flicker, a will-o'-the-wisp candle maker: a softly glowing pale-blue flame-shaped spirit with a tiny face (emissive), floating at about 1.3 m above a small candle-wax base; origin at the bottom of the base. |
| `vendor_herbalist` | 0.7 × 1.7 × 0.6 | Mother Moss, a kindly old witch with a mossy green shawl, a crooked pointed hat with a sprig on it, holding a bundle of lavender. |
| `vendor_glazier` | 0.7 × 1.7 × 0.6 | Count Glass, a pale, elegant vampire in a black high-collared cape with red lining, polishing a small hand mirror (no reflection in it). |
| `vendor_weaver` | 0.7 × 1.7 × 0.6 | Wolfgang the weaver, a shaggy, gentle werewolf in a knitted waistcoat with a measuring tape round his neck. |
| `vendor_raven` | 0.4 × 1.4 × 0.4 | Corvus, a large black raven with a tiny monocle sitting on top of a wooden perch post (the post 1.0 m high); origin at the bottom of the post. |
| `vendor_reaper` | 0.7 × 1.7 × 0.6 | The Reaper, tall and polite, in a long black hooded robe, two pale glowing eyes (emissive) in the dark hood, holding his scythe upright. |
| `hourglass` | 0.2 × 0.3 × 0.2 | A brass-framed hourglass with pale sand, standing upright. |
| `plan` | 0.4 × 0.12 × 0.12 | A rolled-up building plan on yellowed paper, tied with a red ribbon, lying on its side (it stands for the carpenter's work on the shop). |
| `special_tag` | 0.14 × 0.1 × 0.02 | A little brass price tag on a loop of red string, a star stamped into it; marks a trader's special offer of the day. Origin at the bottom centre. |
| `stall_frost` | 1.9 × 2.5 × 1.2 | Ilse Icicle's winter stall: like the other stalls (table top empty at 0.85 m), with a pale ice-blue awning, icicles hanging from its edge, a garland of fir and tiny lanterns, the emblem a snowflake. |
| `vendor_frost` | 0.7 × 1.7 × 0.6 | Ilse Icicle, a tall, friendly frost spirit in a long white-blue coat with fur trim, frost on her eyebrows, rosy cheeks, holding a steaming cup. |
| `stall_florist` | 1.9 × 2.5 × 1.2 | Bluma's spring stall: like the other stalls (table top empty at 0.85 m), with a pastel pink awning, flower garlands and ivy, buckets of tulips beside the table, the emblem a tulip. |
| `vendor_florist` | 0.7 × 1.7 × 0.6 | Bluma the florist, a cheerful plump ghost lady with a flowery headscarf, a green apron, garden gloves and a watering can. |

The five gable models use glTF +Y up and +Z towards the market. Centre each origin on the underside of its sill. They fit the upper floor of the village houses and leave the spring flower boxes in front of the lower windows.

The nine stall models include woven borders to either side of the roof embroidery. Give the seven year-round traders individually cut front hems. Keep the winter icicles and spring garland visible. Set each trade's fittings below the 0.85 m tabletop on the customer side (+Z in glTF). Use a plank stack and chisel rack for Bones, tubs of wax tapers for Flicker, glass jars and drying sprigs for Mother Moss, leaded sample panes for Count Glass, folded and rolled cloth for Wolfgang, coin drawers for Corvus, and small framed hourglasses for the Reaper. Ilse keeps folded wool and cut ice on her lower shelf. Bluma keeps flowering terracotta pots on hers.

## Delivery

Put finished models into the repository **as soon as they are done** – do not wait for the
others. Several models may share a commit.

- Repository: `https://github.com/NAG473-s/Spare-Change-for-Spirits`, branch `main`
  (commit directly to `main`, no pull requests).
- Each `.glb` at the path given in its section (e.g. `assets/models/decor/shelf.glb`), file
  name = the id.
- Commit message in English, e.g. `Add shelf model [skip ci]` or `Add winter keepsake models [skip ci]`.
  Use the repository's existing author identity (see `AGENTS.md`); no co-author lines.
- Run `git pull --rebase origin main` before each push, then push right away.
- Commit only the `.glb` (textures embedded); no `.blend` files, renders or other changes.
- A replacement for an existing model: `Update <id> model [skip ci]`.

## After generating

1. Each `.glb` is in its folder with the exact id as file name (and pushed, see above).
2. Check of size, origin and triangle count (the rules were tested with Blender 5.3):
   `python3 tools/check_models.py` (lists every model and flags problems).
3. Start the game – the model replaces the placeholder.

**Steam:** AI-generated content must be disclosed in the Steamworks content survey.
