# Art prompts

The shop is a 3D diorama: furniture, products, the counter, the door and the walls are 3D models
(see [MODEL_PROMPTS.md](MODEL_PROMPTS.md)). Images are only needed for:

- **Ghosts** – hand-drawn sprites that float in the room and always face the camera.
- **Product pictures** – shown on the shelves while a product has no 3D model yet.
- **The floor texture.**
- **Steam store art** – done, see [`art/steam/`](../art/steam/README.md).

Images from the old 2D version (furniture, keepsakes, door, counter, back wall) are no longer used
by the game and live in [`art/legacy-2d/`](../art/legacy-2d/README.md).

See what is still missing (models and ghost sprites):

```bash
python3 tools/check_models.py
```

## To do

| What | Prompts |
|---|---|
| Sprites for the four new ghosts: `cat`, `sailor`, `knight`, `bride` | [Ghosts](#ghosts--canvas-1024--1536-portrait) |
| *(optional)* Pictures for the eight new products – only needed if their 3D models take a while | [Products](#products--canvas-1024--1024) |

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

**Missing:**

| File id | Prompt |
|---|---|
| `cat` | A small ghost cat, pale lavender glow, round and fluffy with a wispy tail instead of legs, big curious eyes, one paw raised. |
| `sailor` | A drowned sailor ghost, sea-green glow, striped shirt and a knitted cap, a little seaweed in his beard, holding a coil of rope, wistful smile. |
| `knight` | A friendly headless knight ghost, silver-blue glow, slightly rusty armour, carrying his smiling helmeted head under one arm. |
| `bride` | A ghostly bride, pearly white glow, old-fashioned lace dress, no veil, a small bouquet held close, hopeful but sad eyes. |

**Done** (prompts kept for redoing a sprite):

| File id | Prompt |
|---|---|
| `shy` | A shy little ghost, pale blue glow, hiding half its face behind a long wispy sleeve, big worried eyes looking down, slightly hunched. |
| `vain` | A vain ghost, soft pink glow, holding a tiny hand mirror, eyelashes and a beauty mark, chin up, a pearl necklace floating around its neck. |
| `poltergeist` | A mischievous poltergeist, warm coral-red glow, wide grin, messy flame-like wisps on its head, a teacup and a spoon floating around it. |
| `forgetful` | A forgetful old ghost, faded beige glow, puzzled expression, a knot tied in a wispy finger, spectacles pushed up on its forehead. |
| `child` | A small child ghost, mint green glow, round and bouncy, missing front tooth grin, holding a paper pinwheel. |
| `edwin` | Edwin, an elderly gentleman ghost, warm pale-yellow glow, kind tired eyes, neat moustache, flat cap and a long coat, holding his hat to his chest. |

## Products – canvas 1024 × 1024

Only a fallback: a product picture appears on the shelves while the product has no 3D model.
Keep the silhouette simple and readable at small size.

**Optional – new products without picture or model yet:**

| File id | Prompt |
|---|---|
| `ship_in_bottle` | A tiny sailing ship inside a corked glass bottle. |
| `brass_compass` | An open round brass compass with a needle pointing north. |
| `fish_bone` | A cartoonish fish skeleton, head, spine and tail. |
| `ball_of_yarn` | A loose ball of soft blue yarn with a strand hanging out. |
| `polishing_cloth` | A folded soft cloth with a little tin of metal polish. |
| `hair_ribbon` | A light blue silk ribbon tied in a bow. |
| `wedding_veil` | A folded white lace veil with a small pearl comb. |
| `pressed_flower` | A pressed pink flower between two pages of an open old book. |

**Done:** `rain_jar`, `letter_paper`, `music_box`, `pocket_watch`, `wool_scarf`, `marbles`,
`tea_tin`, `faceless_photo`, `rusty_key`, `powder_compact`, `lavender`, `tin_soldier`.

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
