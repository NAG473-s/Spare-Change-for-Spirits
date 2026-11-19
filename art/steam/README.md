# Steam artwork

Artwork follows the palette and subjects in [ART_PROMPTS.md](../../docs/ART_PROMPTS.md).
The illustrations were generated without lettering; the title is composed separately
from the editable vector logo. No Steam upload is performed by this repository.

| Export | Dimensions | Lettering |
|---|---|---|
| `header_capsule.png` | 920 × 430 | Included |
| `small_capsule.png` | 462 × 174 | Included |
| `main_capsule.png` | 1232 × 706 | Included |
| `vertical_capsule.png` | 748 × 896 | Included |
| `library_capsule.png` | 600 × 900 | Included |
| `library_hero.png` | 3840 × 1240 | None; use the separate library logo |
| `library_logo.png` | 1280 × 720 | Transparent logo |

`artwork/` contains the text-free crops used for the capsule exports, so the title can
be repositioned without regenerating the scene. `logo.svg` is the editable source;
its lettering is stored as paths and does not require fonts to render. Lettering is
based on Noto Serif; see [Noto-LICENSE.txt](Noto-LICENSE.txt).

These assets live outside `assets/` so the Windows game package does not include
store-only graphics. Dimensions follow the art brief; check the current Steamworks
requirements before uploading. Disclose the generated illustrations in Steamworks'
AI-generated-content survey, as required in the art brief.
