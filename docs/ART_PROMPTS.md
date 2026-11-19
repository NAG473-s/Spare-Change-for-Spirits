# Art prompts

> Since the switch to the 3D diorama, furniture, the counter, the door and the walls are 3D models
> (see [MODEL_PROMPTS.md](MODEL_PROMPTS.md)). Images are still used for the **ghosts**, the **floor
> texture** and the **product pictures** (shown on shelves until a product has a model) and for the
> Steam store art.

Prompts for generating every image the game uses (e.g. with ChatGPT), plus the Steam store art.
Until an image exists the game shows a coloured placeholder, so images can be added one by one.

## Prepared assets

The complete set is included under `assets/sprites/`: 6 ghosts, 11 decor/keepsake
images, 12 products and 4 shop images. Sprites and the floor texture were prepared
with `tools/prepare_sprite.py --colors 48`; the background keeps its full colour
range. PNG dimensions match the script's output sizes, with transparency on all
isolated objects and opaque floor/background images.

The seven Steam exports, text-free artwork and editable vector logo are in
[`art/steam/`](../art/steam/README.md). They are kept outside the game's packaged
assets. The logo lettering was composed separately from the generated scenes.

## Workflow

1. Start a new ChatGPT conversation and paste the **style block** below once, with
   "Use this style for every image in this conversation."
2. Send one asset prompt at a time. Ask for a **transparent background** (except floor and
   background) and the canvas size given in the table.
3. Download the PNG and run it through the prepare script, which trims empty borders, fits the
   image to the game's aspect ratio and writes it to the right place:

   ```bash
   python3 tools/prepare_sprite.py ~/Downloads/image.png ghost shy
   ```

   Kinds: `ghost`, `decor`, `product`, `door`, `counter`, `floor`, `background`.
   Options: `--remove-bg` if the background did not come out transparent,
   `--colors 48` to reduce the image to 48 colours (helps consistency).
   Needs Pillow: `pip install -r tools/requirements.txt`.
4. Start the game – the image replaces the placeholder automatically.

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
256 px size. 3/4 top-down view (like a cozy farming game). Single centered
object, no text, no letters, no frame, no drop shadow on the ground,
transparent background, PNG.
```

## Ghosts – canvas 1024 × 1536 (portrait)

Ghosts are friendly, semi-transparent, softly glowing and float slightly; no legs, a wispy tail.
Keep the same body shape for all of them and vary colour, face and one accessory.

| File id | Prompt |
|---|---|
| `shy` | A shy little ghost, pale blue glow, hiding half its face behind a long wispy sleeve, big worried eyes looking down, slightly hunched. |
| `vain` | A vain ghost, soft pink glow, holding a tiny hand mirror, eyelashes and a beauty mark, chin up, a pearl necklace floating around its neck. |
| `poltergeist` | A mischievous poltergeist, warm coral-red glow, wide grin, messy flame-like wisps on its head, a teacup and a spoon floating around it. |
| `forgetful` | A forgetful old ghost, faded beige glow, puzzled expression, a knot tied in a wispy finger, spectacles pushed up on its forehead. |
| `child` | A small child ghost, mint green glow, round and bouncy, missing front tooth grin, holding a paper pinwheel. |
| `edwin` | Edwin, an elderly gentleman ghost, warm pale-yellow glow, kind tired eyes, neat moustache, flat cap and a long coat, holding his hat to his chest. |

## Decor – canvas 1024 × 1024 (square)

| File id | Prompt |
|---|---|
| `shelf` | A small wooden shop shelf with three empty boards, old dark wood, a little lace trim on the top board. |
| `candle` | A cluster of three melted candles on a brass dish, warm glowing flames, wax drips. |
| `mirror` | A standing oval mirror in an ornate silver frame, the glass shows a faint blue shimmer. |
| `incense` | A brass incense burner on a small stand, thin curling purple smoke. |
| `rug` | A small round woven rug, dusty rose and indigo pattern, seen from above, slightly wrinkled. |
| `grandfather_clock` | A tall narrow grandfather clock in dark wood, brass pendulum, hands stopped at midnight. |

## Keepsakes (Edwin's gifts) – canvas 1024 × 1024

| File id | Prompt |
|---|---|
| `umbrella` | A closed black umbrella with a curved wooden handle, a few raindrops still on it, leaning upright. |
| `teacup` | A floral porcelain teacup on a saucer, with a small chip on the rim. |
| `blanket` | A folded hand-knitted blanket in warm rust and cream, slightly uneven stitches. |
| `wedding_photo` | An old sepia wedding photograph in a small standing wooden frame, a couple dancing. |
| `pocket_lantern` | A small brass hand lantern with a warm glowing candle inside. |

## Products – canvas 1024 × 1024

Products are shown as small icons on the shelves, so keep the silhouette simple and readable.

| File id | Prompt |
|---|---|
| `rain_jar` | A corked glass jar holding a tiny grey rain cloud with falling droplets. |
| `letter_paper` | A small stack of yellowed letter paper tied with a ribbon, a quill beside it. |
| `music_box` | A small wooden music box with the lid open and a tiny brass crank. |
| `pocket_watch` | A silver pocket watch on a chain, the glass cracked, hands stopped. |
| `wool_scarf` | A rolled-up knitted wool scarf in mustard yellow with tassels. |
| `marbles` | A handful of colourful glass marbles with swirls inside. |
| `tea_tin` | An old round tea tin with a faded floral label and a dented lid. |
| `faceless_photo` | An old photograph in a paper frame showing a person whose face is a soft blur. |
| `rusty_key` | A large old rusty iron key with an ornate bow. |
| `powder_compact` | A round silver powder compact, slightly open, with a tiny puff. |
| `lavender` | A small bundle of dried lavender tied with twine. |
| `tin_soldier` | A small painted tin toy soldier with a drum. |

## Shop

| File id | Kind | Canvas | Prompt |
|---|---|---|---|
| `door` | `door` | 1024 × 1536 | An old wooden shop door with a small round window, a brass bell hanging above it, faint blue light leaking through the gap. |
| `counter` | `counter` | 1024 × 1536 | A very long, narrow wooden shop counter seen from above at 3/4 view, standing vertically and filling a tall thin strip in the middle of the canvas, a brass cash register at the top end. |
| `floor` | `floor` | 1024 × 1024 | Seamless tileable texture of old dark wooden floorboards seen straight from above, subtle wear, no objects. Not transparent. |
| `background` | `background` | 1536 × 1024 | Interior back wall of a small cosy ghost shop at night: dark wallpaper with a faded pattern, a window with moonlight on the left, cobwebs in the corners, empty in the middle. Not transparent. |

## Steam store art

Steam requires several capsule images. Generate the artwork **without any text** and add the
game logo afterwards in an image editor – AI images render lettering badly. Sizes as listed in
Steamworks (check there before uploading):

| Image | Size | Prompt idea |
|---|---|---|
| Header capsule | 920 × 430 | The cosy ghost shop at night seen from the street, warm light in the window, a few ghosts queueing at the door, empty space on the left for the logo. |
| Small capsule | 462 × 174 | Close-up of the shop counter with a candle, a silver spoon and a few buttons, a ghost hand reaching in. |
| Main capsule | 1232 × 706 | Inside the shop: shelves with odd items, Edwin at the counter, the shy ghost peeking from behind a shelf, candlelight. |
| Vertical capsule | 748 × 896 | The shop door with the bell, ghosts drifting towards it under the moon. |
| Library capsule | 600 × 900 | Edwin holding his hat, standing in the warm shop doorway, rain outside. |
| Library hero | 3840 × 1240 | Wide panorama of the shop interior, no characters in the centre (the logo goes on top). |
| Library logo | 1280 × 720 | The logo only, transparent – best made by hand. |

**Steam requires AI-generated content to be disclosed** in the Steamworks content survey.
