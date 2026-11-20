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
| **Custom Volume Controls** | Music, sound effects and narration each have a volume control from 0 to 100 % in 10 % steps. The sound switch mutes all three. The ferryman’s captions stay visible when audio is muted or unavailable. |
| **Adjustable Text Size** | Settings → *Interface size* from 70 to 200 %, on top of the automatic scaling with the window height (designed for 720 p). At 1080 p and 200 %, body text (14 px at 720 p) is 42 px, above Valve's recommended 38 px. Menus scroll when they no longer fit. Hovering a painted direction board or the open/closed sign shows its complete wording at the selected interface size. |
| **Color Alternatives** | Selected tools, wares and menu options have a check mark. Money gains and losses carry + and − signs, good and bad tooltip lines are worded, coins differ by picture and name, a crate you cannot buy says why in its tooltip, a ghost running out of patience fades and looks worried. A tip shows the coin's picture rising over the ghost and is named in the log; hovering the top bar lists in words what the reputation does right now. Furniture sets say in words how many pieces stand and what the set brings; the ghost book pairs each discovered ghost’s portrait with its name and describes needs and payment in words. When the drawer cannot give the change exactly, a button lets the ghost pay with smaller coins. |
| **Camera Comfort** | The shop, harbour, market, opening and title screen use a fixed camera. Scene changes use a fade. Weather (rain, snow, fog) moves only inside the scene, never the camera; there are no lightning flashes. During the opening, the ferryman works his pole, the hull rocks and the lantern swings. Their reflection follows the boat and small rings spread where the pole touches the water. |
| **Mouse Only Option** | The first-launch opening can be skipped anywhere with a click, a tap or a key. It also finishes on its own. The main menu then opens in the automatically selected language. Language selection is available from that menu. The game remembers the first launch across profiles, even if the window closes during playback. The *Menu* button in the top bar pauses play. |
| **Playable without Quick Time Events** | There are no timed button presses. The shade that steals from shops with a bad reputation cannot be caught by clicking; bright light keeps it away. Mishaps during the night (a draught, a crow, a dripping roof) only worsen the atmosphere until the player clicks them; nothing is lost by fixing them late. |
| **Save Anytime** | Pause menu → *Save game*, in every phase, into three manual slots per profile; the autosave (start of every evening and night, dawn) has its own slot and never overwrites them. Three profiles keep separate games. A save made during a night continues at the start of that night (the night itself is a few minutes long). |
| **Playable at Your Own Pace** | With *Patient ghosts* on, ghosts wait calmly at the counter as long as needed. At dawn waiting ghosts leave without hurting the reputation. The game can be paused at any time. |

## Not (yet) declared

| Steam feature | Why not / what it would take |
|---|---|
| Contrast Controls | No high-contrast mode. Would need e.g. a setting for solid, higher-contrast panels and text. |
| Keyboard Only Option | The shop, the harbour and the market are used by pointing at things in the 3D scene. Would need keyboard focus and selection for every clickable thing. |
| Narrated Game Menus / Playable without Vision | The ferryman narrates the opening story. The menus do not have spoken navigation or screen reader support. |
| Stereo Sound / Surround Sound | Sounds are not positioned in space. |
| Subtitle Options | Each spoken line in the opening has a matching subtitle in the active language on a dark backing. Captions follow the interface size. The game does not offer a separate subtitle toggle or style menu. |
| Touch Only, Chat Text-to-speech, Chat Speech-to-text | Not a touch game; no chat. |

## Rules for new content

- Never show important information by colour alone: add a sign, an icon, a shape or a word.
- Sounds only add atmosphere: whatever a sound tells (a ghost whispering its wish, something
  falling off a shelf) is also shown on screen.
- Every new camera or screen movement (shake, zoom, sway) gets a setting to turn it off, or
  *Camera Comfort* has to be withdrawn.
- Everything must stay possible with the mouse alone.
- Nothing may require quick or precisely timed input; anything with a time limit must stop
  counting when *Patient ghosts* is on.
- New text goes through the interface scale (no fixed pixel positions outside the UI), so it
  grows with *Interface size*.
