//! Balancing simulation: plays every night many times without a window,
//! with simple player strategies, and prints money, reputation and sales
//! per night. Run it with
//!
//! ```text
//! cargo run --release -- --simulate [runs]
//! ```
//!
//! It reads the same `assets/data/*.ron` as the game, so a balancing change
//! can be checked right away. The night is modelled in seconds: ghosts
//! arrive as in the game, browse, queue at the counter and are served one
//! after another; a ghost that waits longer than its patience leaves
//! unhappy. The player is assumed to give the right change and to be
//! honest; counterfeits are caught with a fixed chance.

use crate::data::DecorDef;
use crate::data::GameData;
use crate::economy::Purse;
use crate::ferry::sale_price;
use crate::inventory::Stock;
use crate::loan::is_stuck;
use crate::rng::Rng;
use crate::shop::ShopLayout;
use crate::upgrades::Upgrades;

/// Seconds a ghost spends between the door and the queue (walking, browsing).
const TO_COUNTER_SECS: f32 = 6.0;
/// Seconds the player needs at the register per sale.
const SERVICE_SECS: f32 = 9.0;
/// Seconds to turn a ghost away whose wish is not in stock.
const DECLINE_SECS: f32 = 3.0;
/// Chance that the player spots a counterfeit coin.
const SPOT_COUNTERFEIT: f32 = 0.7;
/// Nobody new arrives in the last seconds of the night (as in the game).
const LAST_ARRIVAL_SECS: f32 = 20.0;
/// Money kept back at the market for the ferryman.
const MARKET_RESERVE: u32 = 30;
/// Money kept back when buying a shop improvement.
const IMPROVEMENT_RESERVE: u32 = 60;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Strategy {
    /// Orders what the ghosts are likely to ask for until the ware fades
    /// (or the game ends), when it pays off.
    Demand,
    /// Like `Demand`, but always uses every crate the ferryman brings.
    Eager,
    /// Like `Demand`, and buys furniture at the market so that more ghosts
    /// feel at home.
    DemandDecor,
    /// Always orders the cheapest crates.
    Cheapest,
    /// Like `DemandDecor`, and buys the shop improvements at the market.
    Improvements,
}

impl Strategy {
    const ALL: [Strategy; 5] = [
        Strategy::Demand,
        Strategy::DemandDecor,
        Strategy::Improvements,
        Strategy::Eager,
        Strategy::Cheapest,
    ];

    fn buys_furniture(self) -> bool {
        matches!(self, Strategy::DemandDecor | Strategy::Improvements)
    }

    fn name(self) -> &'static str {
        match self {
            Strategy::Demand => "demand: orders what the ghosts will want, when it pays off",
            Strategy::Eager => "eager: like demand, but always fills every crate",
            Strategy::DemandDecor => "demand + furniture for unmet needs",
            Strategy::Cheapest => "cheapest crates",
            Strategy::Improvements => "demand + furniture + shop improvements",
        }
    }
}

/// Figures of one night of one run.
#[derive(Clone, Copy, Default, Debug)]
pub struct NightStats {
    /// Drawer at dawn (after the ferryman's loan was repaid).
    pub money: f64,
    pub reputation: f64,
    pub visitors: f64,
    pub sales: f64,
    pub revenue: f64,
    /// Asked for something that was not in stock.
    pub missed: f64,
    /// Turned round at the door (unmet need).
    pub fled: f64,
    /// Waited too long at the counter.
    pub unhappy: f64,
    pub faded: f64,
    pub crates: f64,
    pub furniture: f64,
    /// 1 if the ferryman had to lend money this evening.
    pub loans: f64,
    pub owed: f64,
}

impl NightStats {
    fn add(&mut self, o: &NightStats) {
        self.money += o.money;
        self.reputation += o.reputation;
        self.visitors += o.visitors;
        self.sales += o.sales;
        self.revenue += o.revenue;
        self.missed += o.missed;
        self.fled += o.fled;
        self.unhappy += o.unhappy;
        self.faded += o.faded;
        self.crates += o.crates;
        self.furniture += o.furniture;
        self.loans += o.loans;
        self.owed += o.owed;
    }

    fn scaled(&self, f: f64) -> NightStats {
        NightStats {
            money: self.money * f,
            reputation: self.reputation * f,
            visitors: self.visitors * f,
            sales: self.sales * f,
            revenue: self.revenue * f,
            missed: self.missed * f,
            fled: self.fled * f,
            unhappy: self.unhappy * f,
            faded: self.faded * f,
            crates: self.crates * f,
            furniture: self.furniture * f,
            loans: self.loans * f,
            owed: self.owed * f,
        }
    }
}

/// The state of one simulated game.
struct Game<'a> {
    data: &'a GameData,
    rng: Rng,
    night: u32,
    drawer: u32,
    reputation: i32,
    stock: Stock,
    layout: ShopLayout,
    owed: u32,
    upgrades: Upgrades,
    /// Products on the shelves tonight; only these can be sold.
    displayed: Vec<String>,
}

impl<'a> Game<'a> {
    fn new(data: &'a GameData, seed: u64) -> Self {
        Self {
            data,
            rng: Rng::new(seed),
            night: 1,
            drawer: data.config.start_coins.total(),
            reputation: data.config.start_reputation,
            stock: Stock::default(),
            layout: ShopLayout::new(data.config.grid_cols, data.config.grid_rows),
            owed: 0,
            upgrades: Upgrades::default(),
            displayed: Vec::new(),
        }
    }

    /// A cell that is free and not blocked by old junk.
    fn free_cell(&self) -> Option<usize> {
        (0..self.layout.cells.len()).find(|&i| {
            self.layout.cells[i].decor.is_none() && self.upgrades.usable(self.data, &self.layout, i)
        })
    }

    /// Shelf slots in the shop: how many kinds of wares can be offered.
    fn slots(&self) -> usize {
        self.layout
            .cells
            .iter()
            .filter_map(|c| c.decor.as_deref().and_then(|d| self.data.decor(d)))
            .map(|d| d.slots.len())
            .sum()
    }

    /// Products by expected demand tonight, most wanted first.
    fn ranked(&self) -> Vec<(&'a str, f32)> {
        let mut demand = self.expected_demand();
        demand.sort_by(|a, b| b.1.total_cmp(&a.1));
        demand
    }

    /// Improvements: the floor only when it is full, the others when there
    /// is money to spare.
    fn buy_improvements(&mut self) -> u32 {
        let mut spent = 0;
        let mut offered: Vec<_> = self
            .data
            .upgrades
            .iter()
            .filter(|u| self.upgrades.offered(u, self.night))
            .collect();
        offered.sort_by_key(|u| u.cost);
        for u in offered {
            let floor = u.effect.cols + u.effect.rows > 0;
            if floor && self.free_cell().is_some() {
                continue;
            }
            if self.drawer < u.cost + IMPROVEMENT_RESERVE {
                continue;
            }
            self.drawer -= u.cost;
            spent += u.cost;
            self.upgrades.0.insert(u.id.clone());
        }
        spent
    }

    /// Shelves until every product the ghosts often ask for has a slot.
    fn buy_shelves(&mut self) -> u32 {
        let wanted = self.ranked().iter().filter(|(_, d)| *d >= 1.0).count();
        let mut spent = 0;
        while self.slots() < wanted {
            let Some(free) = self.free_cell() else { break };
            let best = self
                .data
                .decor
                .iter()
                .filter(|d| d.is_shelf() && !d.keepsake && d.unlock_night <= self.night)
                .filter(|d| self.drawer >= d.cost + MARKET_RESERVE)
                .max_by(|a, b| {
                    let score = |d: &&DecorDef| d.slots.len() as f32 / d.cost.max(1) as f32;
                    score(a).total_cmp(&score(b))
                });
            let Some(d) = best else { break };
            self.layout.cells[free].decor = Some(d.id.clone());
            self.drawer -= d.cost;
            spent += d.cost;
        }
        spent
    }

    /// Relative weight of every ghost type tonight, and whether it would
    /// stay (its needs are met by the atmosphere).
    fn ghost_weights(&self) -> Vec<(usize, f32, bool)> {
        let atmosphere = self.layout.atmosphere(self.data, self.night);
        self.data
            .ghosts
            .iter()
            .enumerate()
            .filter(|(_, g)| g.unlock_night <= self.night && g.spawn_weight > 0.0)
            .map(|(i, g)| {
                let fits = g.unmet_need(&atmosphere).is_none();
                let w = g.spawn_weight
                    * if fits { 1.0 } else { 0.35 }
                    * self.data.ghost_weight_factor(self.night, &g.id);
                (i, w, fits)
            })
            .collect()
    }

    fn wishes(&self, ghost: usize) -> Vec<&'a str> {
        let data = self.data;
        data.ghosts[ghost]
            .wishes
            .iter()
            .filter(|w| {
                data.product(w)
                    .is_some_and(|p| p.unlock_night <= self.night)
            })
            .map(String::as_str)
            .collect()
    }

    fn mean_interval(&self) -> f32 {
        let (min, max) = self.data.config.spawn_interval_secs;
        let rep_factor = self.data.config.reputation_spawn_factor(self.reputation);
        (min + max) / 2.0
            * rep_factor
            * self.data.spawn_factor(self.night)
            * self.upgrades.spawn_factor(self.data)
    }

    /// Expected units asked for tonight, per product.
    fn expected_demand(&self) -> Vec<(&'a str, f32)> {
        let length = self.data.night_length(self.night);
        let night_secs = length - LAST_ARRIVAL_SECS;
        let visitors = (night_secs / self.mean_interval()).min(length / SERVICE_SECS);
        let weights = self.ghost_weights();
        let total: f32 = weights.iter().map(|(_, w, _)| w).sum();
        let mut demand: Vec<(&str, f32)> = Vec::new();
        for (ghost, w, fits) in weights {
            if !fits || total <= 0.0 {
                continue;
            }
            let wishes = self.wishes(ghost);
            for wish in &wishes {
                let share = visitors * w / total / wishes.len() as f32;
                match demand.iter_mut().find(|(id, _)| id == wish) {
                    Some((_, d)) => *d += share,
                    None => demand.push((wish, share)),
                }
            }
        }
        demand
    }

    /// The village market (from the second day on).
    fn market(&mut self, strategy: Strategy) -> u32 {
        if !strategy.buys_furniture() || self.night < 2 {
            return 0;
        }
        let mut spent = 0;
        if strategy == Strategy::Improvements {
            spent += self.buy_improvements();
        }
        spent += self.buy_shelves();
        loop {
            // The furniture that lets the most (weighted) ghosts stay.
            let staying = |game: &Self| -> f32 {
                game.ghost_weights()
                    .iter()
                    .filter(|(_, _, fits)| *fits)
                    .map(|(_, w, _)| w)
                    .sum()
            };
            let now = staying(self);
            let Some(free) = self.free_cell() else {
                break;
            };
            let mut best: Option<(String, u32, f32)> = None;
            for d in &self.data.decor {
                if d.keepsake || d.unlock_night > self.night || d.cost == 0 {
                    continue;
                }
                if self.drawer < d.cost + MARKET_RESERVE {
                    continue;
                }
                self.layout.cells[free].decor = Some(d.id.clone());
                let gain = staying(self) - now;
                self.layout.cells[free].decor = None;
                let score = gain / d.cost as f32;
                if gain > 0.0 && best.as_ref().is_none_or(|b| score > b.2) {
                    best = Some((d.id.clone(), d.cost, score));
                }
            }
            let Some((id, cost, _)) = best else {
                break;
            };
            self.layout.cells[free].decor = Some(id);
            self.drawer -= cost;
            spent += cost;
        }
        spent
    }

    /// The evening: the ferryman's loan if needed, then the crates.
    fn evening(&mut self, strategy: Strategy, stats: &mut NightStats) {
        let data = self.data;
        if is_stuck(data, self.night, self.drawer, &self.stock) {
            let terms = data.config.ferryman_loan;
            self.drawer += terms.amount;
            self.owed += terms.due();
            stats.loans = 1.0;
        }
        let demand = self.expected_demand();
        // Only what fits on the shelves can be sold: order for those.
        let shelf: Vec<&str> = self
            .ranked()
            .into_iter()
            .take(self.slots())
            .map(|(id, _)| id)
            .collect();
        let limit = self.upgrades.crates_tonight(data, self.night);
        let mut ordered = 0;
        while ordered < limit {
            let affordable = data
                .products
                .iter()
                .filter(|p| p.unlock_night <= self.night && p.crate_cost <= self.drawer);
            let pick = match strategy {
                Strategy::Cheapest => affordable.min_by_key(|p| p.crate_cost),
                Strategy::Demand
                | Strategy::DemandDecor
                | Strategy::Improvements
                | Strategy::Eager => affordable
                    .filter(|p| shelf.contains(&p.id.as_str()))
                    .map(|p| {
                        // Demand until the ware fades or the game ends.
                        let nights_left = data.config.final_night() + 1 - self.night;
                        let lasts = p.fade_after + self.upgrades.fade_bonus(data);
                        let horizon = lasts.min(nights_left) as f32;
                        let wanted = horizon
                            * demand
                                .iter()
                                .find(|(id, _)| *id == p.id)
                                .map_or(0.0, |(_, d)| *d);
                        let missing = (wanted - self.stock.count(&p.id) as f32).max(0.0);
                        let units = missing.min(p.crate_size as f32);
                        let value = units * sale_price(data, &p.id, self.night) as f32
                            - p.crate_cost as f32;
                        (p, value)
                    })
                    .filter(|(_, value)| strategy == Strategy::Eager || *value > 0.0)
                    .max_by(|a, b| a.1.total_cmp(&b.1))
                    .map(|(p, _)| p),
            };
            let Some(p) = pick else {
                break;
            };
            self.drawer -= p.crate_cost;
            self.stock.add(&p.id, p.crate_size);
            ordered += 1;
        }
        stats.crates = ordered as f64;
        // On the shelves: the most wanted products in stock.
        let mut in_stock: Vec<(String, f32)> = self
            .stock
            .units()
            .iter()
            .filter(|(_, u)| !u.is_empty())
            .map(|(id, _)| {
                let d = demand
                    .iter()
                    .find(|(p, _)| *p == id)
                    .map_or(0.0, |(_, d)| *d);
                (id.clone(), d)
            })
            .collect();
        in_stock.sort_by(|a, b| b.1.total_cmp(&a.1));
        self.displayed = in_stock
            .into_iter()
            .take(self.slots())
            .map(|(id, _)| id)
            .collect();
    }

    /// The night, second by second of the arrivals.
    fn night(&mut self, stats: &mut NightStats) {
        let data = self.data;
        let config = &data.config;
        let atmosphere = self.layout.atmosphere(data, self.night);
        let end = data.night_length(self.night) - LAST_ARRIVAL_SECS;
        let mut t = 0.0;
        let mut register_free = 0.0f32;
        // When each ghost in the shop leaves.
        let mut leaving: Vec<f32> = Vec::new();
        loop {
            let (min, max) = config.spawn_interval_secs;
            let rep_factor = data.config.reputation_spawn_factor(self.reputation);
            t += self.rng.range(min, max)
                * rep_factor
                * data.spawn_factor(self.night)
                * self.upgrades.spawn_factor(data);
            if t > end {
                break;
            }
            leaving.retain(|l| *l > t);
            if leaving.len() >= config.max_ghosts_in_shop {
                continue;
            }
            let weights = self.ghost_weights();
            let w: Vec<f32> = weights.iter().map(|(_, w, _)| *w).collect();
            let Some(pick) = self.rng.weighted(&w) else {
                break;
            };
            let (ghost, _, fits) = weights[pick];
            let def = &data.ghosts[ghost];
            stats.visitors += 1.0;
            if !fits {
                stats.fled += 1.0;
                leaving.push(t + 4.0);
                continue;
            }
            // The poltergeist knocks something off a shelf.
            let calm = def.quirks.calm_with.is_some_and(|n| n.is_met(&atmosphere));
            if !calm && self.rng.chance(def.quirks.knock_chance) {
                let on_shelf: Vec<String> = self
                    .stock
                    .units()
                    .iter()
                    .filter(|(_, u)| !u.is_empty())
                    .map(|(id, _)| id.clone())
                    .collect();
                if !on_shelf.is_empty() {
                    let id = &on_shelf[self.rng.index(on_shelf.len())];
                    self.stock.take_one(id);
                }
            }
            let wishes = self.wishes(ghost);
            let wish = wishes[self.rng.index(wishes.len())];
            let at_counter = t + TO_COUNTER_SECS;
            let start = at_counter.max(register_free);
            let patience = def.patience_secs * self.upgrades.patience_factor(data);
            if start - at_counter > patience {
                stats.unhappy += 1.0;
                self.reputation -= 1;
                leaving.push(at_counter + patience);
                continue;
            }
            if !self.displayed.iter().any(|d| d == wish) || !self.stock.take_one(wish) {
                // "Sorry, we have none" is quick.
                register_free = start + DECLINE_SECS;
                leaving.push(register_free);
                stats.missed += 1.0;
                continue;
            }
            register_free = start + SERVICE_SECS;
            leaving.push(register_free);
            let base = data.product(wish).map_or(1, |p| p.price);
            let factor = def.pay_multiplier * data.price_factor(self.night);
            let price = ((base as f32 * factor).round() as u32).max(1);
            let mut paid = price;
            if self.rng.chance(def.quirks.counterfeit_chance) && !self.rng.chance(SPOT_COUNTERFEIT)
            {
                // The fake turns to dust at dawn.
                paid = 0;
            }
            if self.rng.chance(def.quirks.double_pay_chance) {
                // Honest: the second payment is refused.
                self.reputation += 2;
            }
            self.drawer += paid;
            self.reputation += 1;
            stats.sales += 1.0;
            stats.revenue += paid as f64;
        }
    }

    /// Dawn: wares fade, the ferryman is paid back.
    fn morning(&mut self, stats: &mut NightStats) {
        let faded: u32 = self
            .stock
            .age_and_fade(self.data, self.upgrades.fade_bonus(self.data))
            .iter()
            .map(|(_, n)| n)
            .sum();
        stats.faded = faded as f64;
        let repay = self.owed.min(self.drawer);
        self.owed -= repay;
        self.drawer -= repay;
        stats.money = self.drawer as f64;
        stats.reputation = self.reputation as f64;
        stats.owed = self.owed as f64;
    }
}

/// Plays one game; one entry per night.
pub fn play(data: &GameData, strategy: Strategy, seed: u64) -> Vec<NightStats> {
    let mut game = Game::new(data, seed);
    let mut nights = Vec::new();
    for night in 1..=data.config.final_night() {
        game.night = night;
        let mut stats = NightStats {
            furniture: game.market(strategy) as f64,
            ..Default::default()
        };
        game.evening(strategy, &mut stats);
        game.night(&mut stats);
        game.morning(&mut stats);
        nights.push(stats);
    }
    nights
}

/// Averages over many runs, per strategy and night.
pub fn averages(data: &GameData, strategy: Strategy, runs: u32) -> Vec<NightStats> {
    let mut sum = vec![NightStats::default(); data.config.final_night() as usize];
    for seed in 0..runs {
        for (s, n) in sum.iter_mut().zip(play(data, strategy, seed as u64 + 1)) {
            s.add(&n);
        }
    }
    sum.iter().map(|s| s.scaled(1.0 / runs as f64)).collect()
}

/// `--simulate [runs]`: prints the table and returns.
pub fn run(data: &GameData, runs: u32) {
    let start = Purse::total(&data.config.start_coins);
    println!(
        "Balancing simulation: {runs} runs per strategy, {} nights, start money {start}.",
        data.config.final_night()
    );
    println!("Averages per night; money = drawer at dawn after repaying the ferryman.\n");
    for strategy in Strategy::ALL {
        println!("== {}", strategy.name());
        println!(
            "{:>5} {:>7} {:>6} {:>8} {:>6} {:>8} {:>7} {:>6} {:>7} {:>6} {:>6} {:>7} {:>6} {:>6}",
            "night",
            "money",
            "rep",
            "visitors",
            "sales",
            "revenue",
            "missed",
            "fled",
            "unhappy",
            "faded",
            "crates",
            "furnit.",
            "loans",
            "owed"
        );
        let nights = averages(data, strategy, runs);
        for (i, n) in nights.iter().enumerate() {
            println!(
                "{:>5} {:>7.1} {:>6.1} {:>8.1} {:>6.1} {:>8.1} {:>7.1} {:>6.1} {:>7.1} {:>6.1} {:>6.1} {:>7.1} {:>5.0}% {:>6.1}",
                i + 1,
                n.money,
                n.reputation,
                n.visitors,
                n.sales,
                n.revenue,
                n.missed,
                n.fled,
                n.unhappy,
                n.faded,
                n.crates,
                n.furniture,
                n.loans * 100.0,
                n.owed
            );
        }
        let total = |f: fn(&NightStats) -> f64| nights.iter().map(f).sum::<f64>();
        let (visitors, sales) = (total(|n| n.visitors), total(|n| n.sales));
        let (fled, unhappy) = (total(|n| n.fled), total(|n| n.unhappy));
        let last = nights.last().copied().unwrap_or_default();
        println!(
            "total: money {:+.0} · wishes fulfilled {:.0}% · fled {:.0}% · unhappy {:.0}% · loans in {:.0}% of the nights",
            last.money - start as f64,
            100.0 * sales / visitors.max(1.0),
            100.0 * fled / visitors.max(1.0),
            100.0 * unhappy / visitors.max(1.0),
            100.0 * total(|n| n.loans) / nights.len().max(1) as f64,
        );
        println!();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simulation_is_deterministic_and_plays_every_night() {
        let data = GameData::load();
        for strategy in Strategy::ALL {
            let a = play(&data, strategy, 7);
            let b = play(&data, strategy, 7);
            assert_eq!(a.len(), data.config.final_night() as usize);
            assert_eq!(format!("{a:?}"), format!("{b:?}"));
        }
    }

    #[test]
    fn a_sensible_shopkeeper_does_not_go_bankrupt() {
        let data = GameData::load();
        let nights = averages(&data, Strategy::Demand, 50);
        let last = nights.last().unwrap();
        assert!(
            last.money > data.config.start_coins.total() as f64,
            "ends with less than the start money: {last:?}"
        );
    }
}
