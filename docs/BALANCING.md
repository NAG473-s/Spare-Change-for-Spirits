# Balancing

The numbers live in `assets/data/*.ron`. The simulation plays every night many times with simple
strategies and prints averages per night:

```bash
BEVY_ASSET_ROOT=. cargo run --release -- --simulate 500
```

It models arrivals, browsing, the queue at the counter (9 s per sale, 3 s to turn a ghost away),
patience, fading wares, the poltergeist, counterfeits (70 % spotted) and the ferryman's loan. The
player gives the right change and is honest. Strategies:

- **demand** – orders what the ghosts will want until the ware fades (or the game ends), when it
  pays off. A sensible player.
- **demand + furniture** – the same, and buys furniture at the market so that more ghosts stay.
- **eager** – like demand, but always fills every crate.
- **cheapest** – always the cheapest crates, whatever the ghosts want. The careless player.

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
- **Reputation keeps growing** (about 70 by night 5) although its effect stops at 35. It could
  grow more slowly, or unlock something above that.
- The simulation does not model the story ghost or the market's rare pieces, hourglass and
  exchange.
