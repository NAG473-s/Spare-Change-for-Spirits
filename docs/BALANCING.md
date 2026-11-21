# Balancing

The numbers live in `assets/data/*.ron`. The simulation plays every night many times with simple
strategies and prints averages per night:

```bash
BEVY_ASSET_ROOT=. cargo run --release -- --simulate 500
```

It models arrivals, browsing, the queue at the counter (9 s per sale, 3 s to turn a ghost away,
5 s to write an order slip), patience, fading wares, the poltergeist, counterfeits (70 % spotted),
the ferryman's loan, next-night orders and the workbench. The player gives the right change and
is honest. Strategies:

- **demand** – orders what the ghosts will want until the ware fades (or the game ends), when it
  pays off. A sensible player.
- **demand + furniture** – the same, and buys furniture at the market so that more ghosts stay.
- **eager** – like demand, but always fills every crate.
- **cheapest** – always the cheapest crates, whatever the ghosts want. The careless player.

Every strategy except *cheapest* offers a ghost whose wish is missing an order for the next
night (up to `max_per_night`) and buys the crate for it the next evening. The furniture
strategies also buy the workbench, rummage once an evening and repair the broken thing that
sells for the most above its repair cost.

Below each strategy's nights the simulation prints one line per season: what the drawer gained,
the share of wishes fulfilled, unhappy ghosts (and how many of them gave up waiting), the
average and longest wait at the counter, loans and what was paid back, orders taken and the
share collected, and repairs with their cost.

## Comparing with real games

The game writes a playtest log next to the saves (`logs/nights.csv`, one row per night, and
`logs/session-<time>.txt` with every message). `python3 tools/playtest_report.py [nights.csv]`
prints it in the simulation's columns, plus the real time per ghost at the counter – the
simulation assumes 9 s per sale and 3 s to turn a ghost away. The summary says how many nights
were played with an assist on, as those are not directly comparable.

## Goals

- A sensible player is back above the start money by night 3 and ends the demo clearly richer.
- Furniture pays off: the furniture strategy ends richer than plain demand.
- Careless buying does not work, but the ferryman's loan keeps the game going.
- Few ghosts leave unhappy from waiting (under 5 % for a sensible player).
- A good share of wishes can be fulfilled (aim: 40 % or more); the rest is the puzzle of what to
  stock.

## Pass 1 (Nov 2026)

Before: 4 crates per night, crate costs as first written, reputation shortened the time between
ghosts by 2 % per point down to half. Result: only 24 % of wishes fulfilled, night 1 ended at 69
of 92, and reputation reached 55–75 by night 5, so the shop was crowded with ghosts the player
could never serve.

Changes:

- `crates_per_night` 4 → 6 (the storm night brings 5).
- Every `crate_cost` about 15 % lower.
- `reputation_spawn` (new in `config.ron`): 1 % per point, at most 35 % shorter (was 2 % and 50 %).

Result (500 runs, money at dawn after repaying the ferryman, start 92):

| Strategy | Night 1 | 2 | 3 | 4 | 5 | Wishes fulfilled | Fled | Unhappy |
|---|---|---|---|---|---|---|---|---|
| demand | 66 | 78 | 128 | 195 | 315 | 38 % | 21 % | 1 % |
| demand + furniture | 66 | 76 | 127 | 199 | 410 | 38 % | 6 % | 5 % |
| eager | 66 | 77 | 127 | 195 | 315 | 38 % | 21 % | 1 % |
| cheapest | 52 | 24 | 5 | 4 | 1 | 4 % | 21 % | 0 % |

The careless strategy needs the ferryman's loan in night 5 in 28 % of the runs.

## Pass 2: nights 6–10 and shop improvements (Nov 2026)

The full game now has 10 nights, and the simulation knows two more things:

- **Shelf slots:** only as many kinds of wares can be sold as there are shelf slots (2 shelves with
  2 slots at the start). Each evening the simulated player puts the most wanted wares out and
  orders crates only for those; the furniture strategies buy shelves until every product the
  ghosts often ask for has a slot.
- **Shop improvements** (`upgrades.ron`): a new strategy buys them – the floor only when it is
  full, the others when 60 coins stay in the drawer.

Result (400 runs, start 92):

| Strategy | Night 1 | 5 | 10 | Wishes fulfilled | Fled | Unhappy |
|---|---|---|---|---|---|---|
| demand | 55 | 263 | 594 | 34 % | 24 % | 1 % |
| demand + furniture | 55 | 299 | 1252 | 36 % | 8 % | 7 % |
| demand + furniture + improvements | 55 | 256 | 1116 | 35 % | 8 % | 6 % |
| eager | 55 | 263 | 567 | 35 % | 24 % | 1 % |
| cheapest | 52 | 1 | 0 | 2 % | 24 % | 0 % |

Findings:

- Neither crates nor floor space are the bottleneck for a sensible player: even two extra crates
  from the ferryman or a smaller start area hardly change the result. What limits sales is how
  many different wares the ghosts ask for.
- The improvements therefore pay back less than they cost in the simulation (−140 of +1160 by
  night 10). They were made cheaper and the ferryman's deal brings two crates; for now they are
  mainly a visible way to spend money.
- **Money piles up in nights 6–10:** a furniture player ends with over 1000 coins and nothing left
  to buy.

## Open points for the next pass

- **Night 1 is an investment:** a sensible player buys wares for several nights and ends the first
  night below the start money. The ledger now also shows the wares in stock at cost and the
  result including them, so the purchase reads as an investment. Check in playtests whether that
  is enough or the first night should be a little more generous.
- **Wishes fulfilled are still below 40 %:** the wishes spread over 11 products on the first night.
  More products per crate for cheap wares, or fewer wishes per ghost, would help.
- **Too much money late:** nights 6–10 need either more to spend it on (bigger improvements such
  as a second counter, rarer furniture) or lower income (the price factors of the Masquerade
  and the Longest Night, or crates that cost more later).
- **Improvements should pay back** for a player who buys them sensibly, at least by night 10:
  e.g. let the lantern and the bench matter more once the shop is crowded, or tie more shelf
  slots to the cleared floor.
- **Reputation keeps growing** (about 70 by night 5) although its effect on arrivals stops at
  35. It now also unlocks things (`reputation_perks` in `config.ron`): tips for exact change
  from 3, the Reaper's rare pieces from 8, the knight from 5 and the bride from 10
  (`min_reputation` in `ghosts.ron`). Below 0 a shade may come and steal one displayed ware a
  night unless the shop has light 5 or more (`thief`). Raising the arrival effect to 2 % per
  point was tried again and dropped: it crowded the shop as before.
- Ghosts no longer pay with a coin worth 1.7 times the price or more when they have smaller
  ones, so the drawer runs out of change far less often; when it does, the register offers
  "No fitting change" and the ghost pays with smaller coins.
- The simulation does not model the story ghosts, tips, the shade or the market's rare pieces,
  hourglass and exchange; it does model the picky guests.

## Season 2 (nights 11–20)

The simulation now plays all 20 nights. 100 runs: demand +1088, demand + furniture +2493,
demand + furniture + improvements +2581, cheapest crates −92 (loans in 38 % of the nights);
39–42 % of the wishes fulfilled. Money grows about twice as much over twice the nights, so the
winter neither breaks nor starves the shop. Not simulated yet: mishaps (orders and the
workbench since pass 3).

## Season 3 (nights 21–30)

100 runs over all 30 nights: demand +1685, demand + furniture +3737, demand + furniture +
improvements +4151, eager +1676, cheapest crates −92 (loans in 42 % of the nights); 40–47 % of
the wishes fulfilled. The money keeps growing at about the same pace per season.

## Story pacing (Nov 2026)

The simulation now plays the stories with the game's own rules (`Progress`): story ghosts come
while their chapter is due, get their ware from stock or from a parcel, or try again the next
night, and each season ends once its finale is done (it plays on past night 30 until the last
finale). The table has two more columns, chapters finished (`story`) and ferryman parcels
(`parcels`), and each strategy ends with the nights of the seasons' finales.

**Finding:** shopkeepers who planned ahead were punished. With the announced ware in stock no
parcel came, ordinary customers often bought the last one before the story ghost arrived, and
the chapter slipped by a night. Over 200 runs the finales came on these nights:

| Shop | Season 1 | Season 2 | Season 3 |
|---|---|---|---|
| demand (buys the story ware) | 11.3 (10–13) | 21.8 (20–24) | 32.5 (30–36) |
| demand + furniture + improvements | 10.1 (10–11) | 20.4 (20–22) | 31.0 (30–33) |
| cheapest crates (never has it) | 10.0 | 20.0 | 30.0 (30–32) |

**Change:** at the start of a night one unit of the announced ware in stock is set aside for
the story ghost (`deliver_parcels`) and handed over like a parcel; if the ghost brings another one
from the shelf, the set-aside unit goes back into stock. After the change every strategy reaches
the finales on nights 10, 20 and 30. Only a bankrupt shop can still reach season 3's finale a
night or two late: its last chapter asks for one coin as the ferryman's fare, and an empty
drawer cannot pay it until the next evening. The game handles that late finale: the season
waits for it. `sim::tests::every_season_reaches_its_finale_with_a_full_or_a_struggling_shop`
checks the pacing for every strategy.


## Pass 3: orders, repairs and the queue (Nov 2026)

The simulation now takes next-night orders and repairs at the workbench, and reports waiting
times and loan repayments per season (see the top of this file).

**Finding:** the counter is the bottleneck. Orders keep the shelves fuller, so more ghosts are
served, every sale holds the counter for 9 s and the queue grows. With orders the sensible shop
earned more (+2215 against +1859 over 30 nights), but 11 % of its ghosts left unhappy (5 %
before) and 18–26 % of the orders were lost, almost all because the ghost back for its order
gave up in the queue. The four big nights (5, 10, 20, 30) were the worst, with up to 17 % unhappy.
Reputation reached 35 by night 4, after which every night ran at the fastest arrival rate.

Changes:

- `orders.patience_factor` 1.5 (new in `config.ron`): a ghost back for its order came on purpose
  and waits longer (game and simulation).
- `reputation_spawn.fastest` 0.65 → 0.75: a famous shop is busy, not overrun.
- `max_ghosts_in_shop` 6 → 5.
- The big nights' `spawn_factor` 0.65–0.7 → 0.8 (All Souls, the Longest Night, Midwinter, the
  Last Crossing); they keep their higher prices and longer nights.
- The six least patient customers wait 6 s longer (`patience_secs` 12–18 → 18–24); the shade
  keeps its 10 s.

Result (300 runs, start 92, money at dawn after repaying the ferryman):

| Strategy | Night 1 | 3 | 5 | 30 | Wishes fulfilled | Unhappy | Orders collected |
|---|---|---|---|---|---|---|---|
| demand | 73 | 183 | 352 | +2174 | 48 % | 5 % | 93–97 % |
| demand + furniture | 73 | 174 | 395 | +3857 | 50 % | 6 % | 94–97 % |
| demand + furniture + improvements | 73 | 137 | 285 | +4035 | 60 % | 5 % | 91–96 % |
| cheapest | 58 | 17 | 6 | −89 | 2 % | 0 % | – |

Per season for the sensible shop: +699, +731, +745; 45 %, 48 %, 50 % fulfilled; 4.5 %, 5.5 %,
6.3 % unhappy; an average wait of 10–12 s with the longest about 29 s. On the big nights
2–4 of 35–47 ghosts leave unhappy (was 6–10 of 44–62). The workbench repairs about one broken
thing a night for 2–3 coins; it pays a little but mainly gives the evening something to do.

Open points:

- **No loans any more:** the careless shop never qualifies for the ferryman's loan, because its
  unsold cheap stock counts as wares (`loan::is_stuck`). It limps along near zero instead. In
  rare runs its empty drawer cannot pay the ferryman's coin fare, and season 3's finale comes a
  night or two late. The game waits for it, and the pacing test allows that for this strategy.
- **Orders are always full:** three slips a night are taken every night, with about 10 wishes
  missed a night. A fourth slip, or a slip limit that grows with a shop improvement, would turn
  more missed wishes into income.
- The service times are assumptions. Compare them with `tools/playtest_report.py` from real
  nights, especially the late-game queue.
