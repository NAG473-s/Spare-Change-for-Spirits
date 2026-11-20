# Steam setup

What has to be entered in Steamworks for the full game and the demo. The game itself needs no
Steamworks SDK at build time: it loads `steam_api` (`steam_api64.dll`, `libsteam_api.so`,
`libsteam_api.dylib`) at launch if the file lies beside the executable and the game was started
by Steam (`src/i18n/steam.rs`). Without the file, or started outside Steam, the game simply
runs without Steam.

The [Desktop builds workflow](../.github/workflows/build.yml) packages the Steam runtime for
both editions on each platform. It verifies the SHA-256 of `steamworks-sys` 0.13.0 and takes
Valve’s redistributable from that package. Windows and Linux load the library beside the
executable. The macOS Universal app includes both architectures in `Contents/Frameworks`.
Use the full and demo artifacts for their respective depots. Unpack the Linux tarball and
macOS app archive before uploading. See the [artifact names](../README.md#desktop-builds).

## Steam Cloud – full game and demo

Saves are small `.ron` files. Use **Auto-Cloud** (Steamworks → App Admin → *Steam Cloud*) for
**both** apps, the full game and the demo, with the same settings:

- Byte quota per user: 10 MB, number of files: 100 (a profile holds at most four saves).
- Root paths (one per platform), all **recursive**, pattern `*.ron`:

| Platform | Root | Subdirectory |
|---|---|---|
| Windows | `WinAppDataRoaming` | `SpareChangeForSpirits` |
| macOS | `MacAppSupport` | `SpareChangeForSpirits` |
| Linux | `LinuxXdgDataHome` | `spare-change-for-spirits` |

The pattern `*.ron` syncs `settings.ron` and every `profile<n>/auto.ron` and
`profile<n>/slot<n>.ron`; the playtest logs (`logs/*.txt`, `logs/nights.csv`) stay local.
The demo and the full game use the same folder, so on one computer the full game continues a
finished demo save. Each app has its own cloud: a demo save played on another computer reaches
the full game through the demo's cloud only once the demo has been started there.

## Achievements – full game only

The demo has no achievements (`--features demo` leaves them out). Create these in Steamworks
(App Admin → *Stats & Achievements* → *Achievements*), with the **API name** exactly as below,
then publish. Names and descriptions: the English text below, the other 30 languages from
`assets/i18n/<language>.ron` (`achievement.<id>.name` / `.text`). Icons: 256 × 256, one for
"achieved" and a greyed one for "unachieved", see *Achievement icons* in
[ART_PROMPTS.md](ART_PROMPTS.md).

| API name | Name | Description | Icon |
|---|---|---|---|
| `FIRST_SALE` | Open for Business | Sell your first ware to a ghost. | `art/steam/achievements/first_sale.png` |
| `EXACT_CHANGE` | Exact Change | Give exactly the right change 50 times. | `art/steam/achievements/exact_change.png` |
| `INTO_THE_LIGHT` | Into the Light | Send 100 ghosts into the light. | `art/steam/achievements/into_the_light.png` |
| `HONEST` | Honest Shopkeeper | Tell a forgetful ghost the truth about paying twice, 10 times. | `art/steam/achievements/honest.png` |
| `SHARP_EYE` | Sharp Eye | Spot 10 counterfeit coins at the register. | `art/steam/achievements/sharp_eye.png` |
| `WELL_LIKED` | Well Liked | Receive 25 tips. | `art/steam/achievements/well_liked.png` |
| `ON_ORDER` | Worth the Wait | Fulfil 10 orders for the next night. | `art/steam/achievements/on_order.png` |
| `HANDY` | Handy | Fix 20 mishaps during the night. | `art/steam/achievements/handy.png` |
| `TINKERER` | Tinkerer | Repair 10 broken things at the workbench. | `art/steam/achievements/tinkerer.png` |
| `GOOD_NAME` | A Good Name | Reach a reputation of 25. | `art/steam/achievements/good_name.png` |
| `FULL_DRAWER` | A Full Drawer | Have 500 in the drawer at once. | `art/steam/achievements/full_drawer.png` |
| `ROOM_TO_BREATHE` | Room to Breathe | Clear all of Hedda's old junk. | `art/steam/achievements/room_to_breathe.png` |
| `EDWIN` | Dear Margaret | Finish Edwin's story. | `art/steam/achievements/edwin.png` |
| `LIGHTHOUSE` | The Light Returns | Relight the lighthouse with Ida and finish the first season. | `art/steam/achievements/lighthouse.png` |
| `PFOTE` | Good Dog | Help Lotte find her dog Pfote. | `art/steam/achievements/pfote.png` |
| `FIRST_SNOW` | First Snow | Set the tower clock going with Anselm and finish the second season. | `art/steam/achievements/first_snow.png` |
| `SPRING` | In Bloom | Bring spring back to the village with Mira. | `art/steam/achievements/spring.png` |
| `FERRYMAN` | The Ferryman's Fare | Pay Kasimir's own crossing and finish the third season. | `art/steam/achievements/ferryman.png` |

The game keeps its own record in each profile's save, shows it under *Achievements* in the main
menu (all profiles together) and the pause menu, and tells Steam each unlock. An achievement
earned while Steam was not running is sent to Steam the next time the game runs through Steam.
New achievements: add them to `assets/data/achievements.ron`, translate the texts, and create
them in Steamworks under the upper-case id.

## Steam Deck and controllers

The game reads controllers itself (`src/gamepad.rs`): the left stick moves the mouse pointer,
A clicks, B right-clicks, the right stick scrolls, the d-pad nudges the pointer and Start opens
the pause menu. In Steamworks set *Controller support* to **Full** only after a test round on a
Steam Deck; until then *Partial*. Choose the **Gamepad** template as the default Steam Input
configuration so the Deck sends controller input rather than keyboard and mouse. The interface
scales with the window height, so 1280 × 800 needs no extra layout; check the text size at 100 %
interface size on the Deck before applying for *Verified*.
