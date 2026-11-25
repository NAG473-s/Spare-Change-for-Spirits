# Accessibility

Steam lists accessibility features on the store page and lets players filter by them. They are
declared in Steamworks (app admin → *Basic Info* → accessibility feature wizard). Valve's
descriptions: <https://partner.steamgames.com/doc/accessibility_features>.

Only declare what the current build really does. When a change touches one of the points below,
update this file in the same change.

## Declare

| Steam feature | How the game meets it |
|---|---|
| **Adjustable Difficulty** | Settings → *Assists*: *Patient ghosts* (no waiting limit at the counter) and *Show change due* (the register shows the change to give back). Each can be switched on and off separately. The register displays the offered coins and the coins on the change tray. It flags too little or too much change before the sale is booked. The visible return button or a right-click on a denomination puts one coin back. The whole tray can also be reset. |
| **Custom Volume Controls** | Music, sound effects and narration each have a volume control from 0 to 100 % in 10 % steps. The sound switch mutes all three. The opening and each of the six painted story films have captions in the active language, including when audio is muted or unavailable. Their speech follows the narration control. |
| **Adjustable Text Size** | Settings → *Interface size* from 70 to 200 %, on top of the automatic scaling with the window height (designed for 720 p). At 1080 p and 200 %, body text (14 px at 720 p) is 42 px, above Valve's recommended 38 px. Menus scroll when they no longer fit. At 200 % on a 1280 × 800 screen the title logo shrinks to leave room for the menu, the top bar takes a second line, the message log gives the side panel more height, the morning ledger spreads over the whole width with its pages one above the other, and the book fits the screen with its page scrolling. Hovering a painted direction board or the open/closed sign shows its complete wording at the selected interface size. Guest names, stock quantities and signs scale with the interface. The queue shows one nameplate. Pointing at another customer in line moves it to that guest. Moving the pointer away returns it to the front. Long captions wrap. Plates stay clear of the top bar and side panel, and take the closest free space to their subject. |
| **Color Alternatives** | Selected tools, wares and menu options have a check mark. Money gains and losses carry + and − signs, good and bad tooltip lines are worded, buttons, candle stubs and spoons have distinct silhouettes, names and handling sounds, a crate you cannot buy says why in its tooltip, a pinned order marks its crate with a star (★) and is named on the ferry panel and in the crate tooltip, a ghost running out of patience fades and looks worried. A tip shows the coin's picture rising over the ghost and is named in the log; hovering the top bar lists in words what the reputation does right now. Order slips use words for stock, collection and expiry, and furniture previews describe atmosphere changes and name affected guests you have seen. Furniture sets say in words how many pieces stand and what the set brings; the ghost book lists guests from their first sighting, including those who turn away. Each name opens a portrait with a written character note, observed wishes and, for customers, information about needs and payment. When the drawer cannot give the change exactly, a button lets the ghost pay with smaller coins. |
| **Camera Comfort** | The shop, harbour, market, opening and title screen use a fixed camera. Each story film uses three fixed paintings with short dissolves. Scene changes use a fade (also the cuts to the quay and the market at dawn after a finale, which wait for a click and have no timer). Weather (rain, snow, fog) moves only inside the scene, never the camera; there are no lightning flashes. Settings → *Seeing and moving* → *Reduced motion* turns every fade into a plain cut, stops the tower clock's hands from sweeping round at the first strike, lets Mira's butterfly rest on a window box instead of flying, turns the postcard by the till over at once and keeps Pfote from hopping when Lotte visits. During the opening, the ferryman works his pole, the hull rocks and the lantern swings. Their reflection follows the boat and small rings spread where the pole touches the water. |
| **Keyboard Only Option** | Tab or the arrow keys move a bright frame over the buttons of the topmost layer (the side panel, or the book, pause menu or a caption above it), Enter or Space presses the framed button, and Esc opens and closes the pause menu. A list scrolls to show the framed button. Every shop action has a panel button. The harbour panel lists each crate on the boat, with *Give back* for crates bought tonight. With a ware or piece of furniture selected, the evening panel can put it on the first shelf or square with room. The *Remove* tool lists every piece in the shop, and a selected ware can be taken off the shelves. Mishaps of the night and Lotte's tracks have their own buttons. Serving, change, orders, the market, saving and the book were already buttons. A controller moves a pointer that works like the mouse. |
| **Contrast Controls** | Settings → *Seeing and moving* → *High contrast*: every panel, menu and tooltip gets a solid near-black background. Body and hint text turn brighter and buttons get light edges. Guest names, shelf and crate tickets, the counter and market signs switch to pale ink on solid dark plates with light borders. Switching it off restores their usual colours and painted frames. |
| **Mouse Only Option** | The first-launch opening and the narrated story endings can be skipped with a click, a tap or a key. It also finishes on its own. The main menu then opens in the automatically selected language. Language selection is available from that menu. The game remembers the first launch across profiles, even if the window closes during playback. The *Menu* button in the top bar pauses play. |
| **Playable without Quick Time Events** | There are no timed button presses. The shade that steals from shops with a bad reputation cannot be caught by clicking; bright light keeps it away. Mishaps during the night (a draught, a crow, a dripping roof) only worsen the atmosphere until the player clicks them; nothing is lost by fixing them late. |
| **Save Anytime** | Pause menu → *Save game*, in every phase, into three manual slots per profile; the autosave (start of every evening and night, dawn) has its own slot and never overwrites them. Three profiles keep separate games. A manual save or a save on leaving during the first guided sale restores the wish or the counted change. Once that sale is settled, its receipt, stock and drawer are saved for the rest of the first night. Other nights restart at their evening. |
| **Playable at Your Own Pace** | During the optional first-sale guide the first customer waits and the night clock stands still until the sale is complete or the guide is skipped. Its steps can be reread in the ghost book. With *Patient ghosts* on, ghosts wait calmly at the counter as long as needed. At dawn waiting ghosts leave without hurting the reputation. The game can be paused at any time. |

## Not (yet) declared

| Steam feature | Why not / what it would take |
|---|---|
| Narrated Game Menus / Playable without Vision | The ferryman narrates the opening story. The menus do not have spoken navigation or screen reader support. |
| Stereo Sound / Surround Sound | Sounds are not positioned in space. |
| Subtitle Options | The opening and all six story endings show their spoken text in the active language on a dark backing. Captions follow the interface size. The game does not offer a separate subtitle toggle or style menu. |
| Touch Only, Chat Text-to-speech, Chat Speech-to-text | Not a touch game; no chat. |

## At the counter and in the queue

A guest’s name opens their discovered book entry by mouse, keyboard or controller while the shop clock and their patience pause. Returning keeps the counted change. Shelf tickets pair the hourglass with the number of units due to fade, and the product tooltip gives the same information in words. Both guest memories have permanent written descriptions. Reduced motion holds the sailor’s cap and Ida’s nod in a single pose, stops the decorative bob and animated rain, and reveals the mirror painting immediately.

Players can search once per evening using the workbench button or by clicking a pile of old cargo. The written result identifies the find. Reduced motion shows the open case and found item in a still pose. With that setting, inspecting a ferry container switches directly to its open pose, and purchased cargo appears in its place on the cart.

The register shows the product picture, its written name and price. The three tray pockets follow the player’s count, including returns. The total and the exact denomination controls stay available as text. Reduced motion places the money and ware directly at their destinations. The entrance switches between open and closed poses and its bell stays still.

Floor corners and an enamel badge mark the target when moving furniture or using a furnishing tool. A check marks a free placement, opposed arrows an exchange, a cross a blocked action and a minus a removal. Reduced motion moves the furniture directly to the indicated square.

At the market, physical purchases fill a wicker basket and sold single pieces show a sealed envelope. The stall's written offer and tooltip remain available. Reduced motion updates the basket and hovered wares immediately and holds the traders and pennants still. A returned crate travels back from the cart to the deck. At the harbour the ferry stays at its appropriate waterline with the mooring lines fast, and unloading or returning cargo makes no rocking or transfer ripples with reduced motion enabled.

## Rules for new content

- Never show important information by colour alone: add a sign, an icon, a shape or a word.
- Sounds only add atmosphere: whatever a sound tells (a ghost whispering its wish, something
  falling off a shelf) is also shown on screen.
- Every new camera or screen movement (shake, zoom, sway) gets a setting to turn it off, or
  *Camera Comfort* has to be withdrawn.
- Everything must stay possible with the mouse alone, and with the keyboard alone: an action in
  the 3D scene also gets a panel button (`ActionButton`), which the keyboard focus reaches.
- Panels use the shared colours (`BG_PANEL`, `TEXT`, `TEXT_DIM`) and `ButtonTone` buttons, so
  *High contrast* applies to them; a new full-screen movement also respects *Reduced motion*.
- Nothing may require quick or precisely timed input; anything with a time limit must stop
  counting when *Patient ghosts* is on.
- New text goes through the interface scale (no fixed pixel positions outside the UI), so it
  grows with *Interface size*.
