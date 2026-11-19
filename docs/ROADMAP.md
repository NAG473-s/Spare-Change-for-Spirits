# Roadmap to the festival and Early Access

**Goal:** show the demo at the Steam shop management festival (registration until
**2027-01-25**) and open **Early Access right after the festival**, so its attention lands on a
game that can be bought – not on a wishlist that waits for months while others copy the idea.

Until the store page goes live the repository stays private and nothing about the game is shown
publicly.

## What goes where

| | Demo (festival) | Early Access launch | First updates |
|---|---|---|---|
| Nights | 1–5, ends with "continue with night 6 in the full game" | 1–10 | 11–15, endless mode |
| Ghosts | all 10 | + 4 new | more, seasonal ones |
| Story | Edwin, all 5 chapters | + a second story ghost for nights 6–10 | a third story |
| Shop | furniture, harbour, ferryman | + shop upgrades (bigger shop, second counter) | decoration sets |
| Polish | everything shown must look finished | all models in, no placeholders | |

**Save games carry over:** the demo and the full game save to the same folder. A save from the
end of the demo (night 5 played) continues in the full game with the evening of night 6 – shop,
coins, stock, keepsakes and reputation included. The demo is built with `cargo build --features
demo` (it stops after `demo_nights` in `assets/data/config.ron`); the full game stops after
`last_night`. From the public demo release on, saves must stay compatible
between the two (new save fields get `#[serde(default)]` instead of a new save version); until
then the format may still change.

## Week by week

Checkboxes are ticked in the commit that finishes them.

### Week 1 · Nov 19 – Nov 29: ready for players

- [x] UI scales with the window; fullscreen/windowed; music and effect volume; saved in settings
- [x] Bankruptcy rule: what happens when the drawer cannot pay for a single crate (the ferryman
      lends `ferryman_loan` in `config.ron` and takes it back with a surcharge at dawn)
- [x] Simulation that plays all nights with simple strategies and prints money, reputation and
      sales per night – the basis for balancing (`cargo run --release -- --simulate [runs]`)
- [ ] First balancing pass in `assets/data/*.ron`
- [ ] Windows build run and the `.exe` played on a real Windows PC (assets, fonts, saves, sound)
- [ ] Ledger frame (`assets/sprites/ui/ledger_frame.png`) used for the morning accounts
- [ ] Repository private

### Week 2 · Nov 30 – Dec 6: first playtests

- [ ] 3–5 people play the demo nights without help; notes on where they get stuck
- [ ] Fix what they stumbled over (tips, clarity, bugs)
- [ ] Models from ChatGPT arriving one by one; each checked with `tools/check_models.py`
- [ ] Ghost tooltips (name, mood, wish hint when waiting)

### Week 3 · Dec 7 – Dec 13: content for Early Access

- [ ] Nights 6–10: new events in `events.ron`, rising difficulty
- [ ] 4 new ghosts with wishes, sprites and sheets ordered
- [ ] A second story ghost for nights 6–10 (Edwin's story ends in the demo)
- [ ] Shop upgrades (bigger shop, more shelves) as a reason to save money

### Week 4 · Dec 14 – Dec 20: store page live

- [ ] Store page text, tags, screenshots (only once the models are in)
- [ ] Trailer v1 (30–60 s, captured in the game)
- [ ] Store page submitted for review and set to "coming soon" – wishlists start counting
- [ ] Demo set up as its own app in Steamworks

### Week 5 · Dec 21 – Jan 3: holidays, buffer

- [ ] Light polish only, no big features
- [ ] Translations of the most important languages (German, French, Spanish, Japanese,
      Simplified Chinese) checked by native speakers if possible

### Week 6 · Jan 4 – Jan 10: demo build

- [ ] Demo build with `--features demo` (nights 1–5); end screen with "continue with night 6"
      and a wishlist link; check that a finished demo save continues in the full build
- [ ] Windows workflow builds both editions
- [ ] Demo build uploaded and submitted for Steam's build review
- [ ] Second playtest round with the Early Access content

### Week 7 · Jan 11 – Jan 17: Early Access build

- [ ] Nights 1–10 complete and balanced, no placeholders left
- [ ] Early Access FAQ on the store page (what is planned, how long, price)
- [ ] Price and Early Access release date set

### Week 8 · Jan 18 – Jan 25: festival registration

- [ ] Registered for the festival by **Jan 25** with the approved demo
- [ ] List of streamers and press for cozy and management games; short mail with a key
- [ ] Social posts: a few short clips (the ferryman, the sign at midnight, a ghost passing
      into the light)

### Week 9 · Jan 26 – Jan 31: launch ready

- [ ] Early Access build uploaded and approved, release button ready for right after the festival
- [ ] Plan for the first two updates (nights 11–15, second story ghost)

## Steam rules to check in Steamworks

Check these against the current Steamworks documentation, they change:

- How long the store page and each build take to review (plan with about a week).
- How long a page has to be visible as "coming soon" before release.
- What the festival requires from the demo (its own app, release date, page language).
- The disclosure of AI-generated content in the content survey.

## Risks

- **Models arrive late:** every model has a placeholder, but screenshots and the trailer need
  the real ones – order them first, store page assets after.
- **Balancing:** nights 6–10 are new; without the simulation and playtests they will be too easy
  or too hard.
- **Holidays:** week 5 is deliberately light; anything that slips moves there, not into
  week 8.
