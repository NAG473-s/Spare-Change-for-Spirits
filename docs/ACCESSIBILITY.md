# Accessibility

Steam lists accessibility features on the store page and lets players filter by them. They are
declared in Steamworks (app admin → *Basic Info* → accessibility feature wizard). Valve's
descriptions: <https://partner.steamgames.com/doc/accessibility_features>.

Only declare what the current build really does. When a change touches one of the points below,
update this file in the same change.

## Declare

| Steam feature | How the game meets it |
|---|---|
| **Adjustable Difficulty** | Settings → *Assists*: *Patient ghosts* (no waiting limit at the counter) and *Show change due* (the register shows the change to give back). Each can be switched on and off separately; the settings page explains both. |
| **Custom Volume Controls** | Settings: music and sound effects each from 0 to 100 % in 10 % steps, plus *Sound on/off*. The game has no speech. |
| **Adjustable Text Size** | Settings → *Interface size* from 70 to 200 %, on top of the automatic scaling with the window height (designed for 720 p). At 1080 p and 200 %, body text (14 px at 720 p) is 42 px, above Valve's recommended 38 px. Menus scroll when they no longer fit. |
| **Color Alternatives** | No information is shown by colour alone: money gains and losses carry + and − signs, good and bad tooltip lines are worded, coins differ by picture and name, a crate you cannot buy says why in its tooltip, a ghost running out of patience fades and looks worried. |
| **Camera Comfort** | Fixed camera: no screen shake, camera bob or motion blur. Moving between the shop, the harbour and the market is a fade. |
| **Mouse Only Option** | Everything is done with the mouse; the *Menu* button in the top bar pauses (Esc does the same). |
| **Playable without Quick Time Events** | There are no timed button presses. |
| **Playable at Your Own Pace** | With *Patient ghosts* on, no input has a time limit: ghosts wait at the counter as long as needed, and at dawn waiting ghosts simply leave without hurting the reputation. The game can be paused at any time. |

## Not (yet) declared

| Steam feature | Why not / what it would take |
|---|---|
| Save Anytime | The game autosaves at every evening, night start and dawn; there are no manual save slots. Would need manual saves in separate slots that autosaves never overwrite. |
| Contrast Controls | No high-contrast mode. Would need e.g. a setting for solid, higher-contrast panels and text. |
| Keyboard Only Option | The shop, the harbour and the market are used by pointing at things in the 3D scene. Would need keyboard focus and selection for every clickable thing. |
| Narrated Game Menus / Playable without Vision | No screen reader support or narration. |
| Stereo Sound / Surround Sound | Sounds are not positioned in space. |
| Subtitle Options | The game has no spoken content. |
| Touch Only, Chat Text-to-speech, Chat Speech-to-text | Not a touch game; no chat. |

## Rules for new content

- Never show important information by colour alone: add a sign, an icon, a shape or a word.
- Every new camera or screen movement (shake, zoom, sway) gets a setting to turn it off, or
  *Camera Comfort* has to be withdrawn.
- Everything must stay possible with the mouse alone.
- Nothing may require quick or precisely timed input; anything with a time limit must stop
  counting when *Patient ghosts* is on.
- New text goes through the interface scale (no fixed pixel positions outside the UI), so it
  grows with *Interface size*.
