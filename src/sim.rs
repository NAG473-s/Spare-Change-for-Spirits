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

use crate::data::GameData;
use crate::economy::Purse;
use crate::ferry::sale_price;
use crate::inventory::Stock;
use crate::loan::is_stuck;
use crate::rng::Rng;
use crate::shop::ShopLayout;

/// Seconds a ghost spends between the door and the queue (walking, browsing).
const TO_COUNTER_SECS: f32 = 6.0;
/// Seconds the player needs at the register per customer.
const SERVICE_SECS: f32 = 9.0;
/// Chance that the player spots a counterfeit coin.
const SPOT_COUNTERFEIT: f32 = 0.7;
/// Nobody new arrives in the last seconds of the night (as in the game).
const LAST_ARRIVAL_SECS: f32 = 20.0;
/// Money kept back at the market for the ferryman.
const MARKET_RESERVE: u32 = 30;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Strategy {
    /// Orders what the ghosts that can come tonight are likely to ask for.
    Demand,
    /// Like `Demand`, and buys furniture at the market so that more ghosts
    /// feel at home.
    DemandDecor,
    /// Always orders the cheapest crates.
    Cheapest,
}

impl Strategy {
    const ALL: [Strategy; 3] = [Strategy::Demand, Strategy::DemandDecor, Strategy::Cheapest];

    fn name(self) -> &'static str {
        match self {
            Strategy::Demand => "demand: orders what tonight's ghosts want",
            Strategy::DemandDecor => "demand + furniture for unmet needs",
            Strategy::Cheapest => "cheapest crates",
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
        }
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
        let rep_factor = (1.0 - self.reputation as f32 * 0.02).clamp(0.5, 1.5);
        (min + max) / 2.0 * rep_factor * self.data.spawn_factor(self.night)
    }

    /// Expected units asked for tonight, per product.
    fn expected_demand(&self) -> Vec<(&'a str, f32)> {
        let night_secs = self.data.config.night_length_secs - LAST_ARRIVAL_SECS;
        let visitors = (night_secs / self.mean_interval())
            .min(self.data.config.night_length_secs / SERVICE_SECS);
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
        if strategy != Strategy::DemandDecor || self.night < 2 {
            return 0;
        }
        let mut spent = 0;
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
            let Some(free) = self.layout.cells.iter().position(|c| c.decor.is_none()) else {
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
        let limit = data.crates_tonight(self.night);
        let mut ordered = 0;
        while ordered < limit {
            let affordable = data
                .products
                .iter()
                .filter(|p| p.unlock_night <= self.night && p.crate_cost <= self.drawer);
            let pick = match strategy {
                Strategy::Cheapest => affordable.min_by_key(|p| p.crate_cost),
                Strategy::Demand | Strategy::DemandDecor => affordable
                    .map(|p| {
                        let wanted = demand
                            .iter()
                            .find(|(id, _)| *id == p.id)
                            .map_or(0.0, |(_, d)| *d);
                        let missing = (wanted - self.stock.count(&p.id) as f32).max(0.0);
                        let units = missing.min(p.crate_size as f32);
                        let value = units * sale_price(data, &p.id, self.night) as f32
                            - p.crate_cost as f32;
                        (p, value)
                    })
                    .filter(|(_, value)| *value > 0.0)
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
    }

    /// The night, second by second of the arrivals.
    fn night(&mut self, stats: &mut NightStats) {
        let data = self.data;
        let config = &data.config;
        let atmosphere = self.layout.atmosphere(data, self.night);
        let end = config.night_length_secs - LAST_ARRIVAL_SECS;
        let mut t = 0.0;
        let mut register_free = 0.0f32;
        // When each ghost in the shop leaves.
        let mut leaving: Vec<f32> = Vec::new();
        loop {
            let (min, max) = config.spawn_interval_secs;
            let rep_factor = (1.0 - self.reputation as f32 * 0.02).clamp(0.5, 1.5);
            t += self.rng.range(min, max) * rep_factor * data.spawn_factor(self.night);
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
            if start - at_counter > def.patience_secs {
                stats.unhappy += 1.0;
                self.reputation -= 1;
                leaving.push(at_counter + def.patience_secs);
                continue;
            }
            register_free = start + SERVICE_SECS;
            leaving.push(register_free);
            if !self.stock.take_one(wish) {
                stats.missed += 1.0;
                continue;
            }
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
            .age_and_fade(self.data)
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
        for (i, n) in averages(data, strategy, runs).iter().enumerate() {
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
