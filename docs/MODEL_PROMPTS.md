# 3D model prompts

The shop is a small 3D diorama. Furniture, products and the shop itself are 3D models; the ghosts
stay hand-drawn sprites (`assets/sprites/ghosts/`) that float in the room. Until a model exists the
game shows a simple coloured placeholder, so models can be added one by one.

Hand this whole file to ChatGPT (or another tool that builds models in Blender). Start with the
**technical rules** and the **style block**, then go through the lists.

## Technical rules (all models)

- **Format:** glTF binary, one `.glb` file per model, file name = the id in the tables below.
  Export with *+Y Up* (Blender's default glTF export), materials included, no cameras, no lights,
  no animations.
- **Scale:** 1 Blender unit = 1 metre. Keep each model inside the size given in its table row
  (width × height × depth). The game does not rescale models.
- **Origin:** at the **bottom centre** of the model (the point that stands on the floor).
  Apply all transforms (Ctrl+A → All Transforms) before exporting.
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

## Shop – `assets/models/shop/<id>.glb`

| id | Size (w × h × d, m) | Prompt |
|---|---|---|
| `counter` | 4.5 × 1.0 × 0.8 | A long wooden shop counter (front = customer side facing −Y): panelled front with a carved ghost emblem in the middle, a polished top, a small brass bell and an old brass cash register near the right end. The game turns it so it runs along the right side of the shop. |
| `door` | 1.1 × 2.3 × 0.2 | An old wooden shop door in its frame with a small round window, a brass handle and a little brass bell hanging above it; faint blue light in the window (emissive). The game places it in the left wall. |
| `room` | see below | *(optional)* The walls of the shop, replacing the plain placeholder walls. |

**Room layout** (only if you make `room.glb`; Blender coordinates, Z up, origin at 0,0,0, do not
apply an offset): the floor is **not** part of the room model (the game draws the floor grid).

- Back wall: an inner face at **Y = +3.0**, from X = −7.0 to X = +7.5, height 3.4 m.
- Left wall: an inner face at **X = −7.0**, from Y = −3.5 to Y = +3.0, height 3.4 m; leave a
  1.2 m wide, 2.4 m high opening centred at **Y = 0** for the door.
- A tall window in the back wall centred at **X = −3.2**, bottom edge at 0.9 m, 1.6 × 1.8 m,
  with frame and emissive moonlit glass; dark wallpaper with a faded pattern, wooden wainscot up
  to 0.9 m, a few cobwebs in the upper corners.
- Leave the right side and the front open (the camera looks in from the front).

## After generating

1. Put each `.glb` in its folder with the exact id as file name.
2. Check of size, origin and triangle count (the rules were tested with Blender 5.3):
   `python3 tools/check_models.py` (lists every model and flags problems).
3. Start the game – the model replaces the placeholder.

**Steam:** AI-generated content must be disclosed in the Steamworks content survey.
