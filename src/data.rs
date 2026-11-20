//! Game data from `assets/data/*.ron`.
//!
//! Products, ghosts, decor and balancing values live in RON files so they can
//! be tuned without recompiling (restarting the game is enough).

use std::path::PathBuf;

use bevy::prelude::*;
use serde::Deserialize;

use crate::economy::{Coin, Purse};
use crate::i18n::Lang;

#[derive(Deserialize, Clone, Debug)]
pub struct ProductDef {
    pub id: String,
    /// Base price in button units.
    pub price: u32,
    /// Price of one crate from the ferryman.
    pub crate_cost: u32,
    /// Units per crate.
    pub crate_size: u32,
    /// After this many nights in the shop the item fades away.
    pub fade_after: u32,
    /// First night the ferryman offers it.
    #[serde(default = "one")]
    pub unlock_night: u32,
    /// What it comes in on the ferryman's boat: one of `CONTAINERS`.
    #[serde(default = "default_container")]
    pub container: String,
}

/// Containers the ferryman ships products in (each a model and a lid,
/// `assets/models/shop/<id>.glb` and `<id>_lid.glb`).
/// Letters from Hedda (`letter.1` …): the first waits on the first evening,
/// the others turn up when improvements clear her old junk.
pub const HEDDA_LETTERS: u32 = 4;

/// How much less often a ghost comes when the atmosphere does not suit it.
pub const UNFIT_GHOST_ODDS: f32 = 0.35;

pub const CONTAINERS: [&str; 4] = ["crate", "chest", "basket", "keg"];

fn default_container() -> String {
    "crate".to_string()
}

/// Which diorama a prop stands in.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scene {
    Shop,
    Harbor,
    Market,
}

/// Placeholder shape of a prop until its model exists.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Shape {
    #[default]
    Box,
    Cylinder,
}

/// A piece of decoration that only adds atmosphere (`props.ron`).
#[derive(Deserialize, Clone, Debug)]
pub struct PropDef {
    /// Model id: `assets/models/props/<id>.glb`. Several props may share one.
    pub id: String,
    pub scene: Scene,
    /// Where its origin (bottom centre) stands, in the scene's metres.
    pub pos: (f32, f32, f32),
    /// Turn about the vertical axis, in degrees.
    #[serde(default)]
    pub turn: f32,
    /// Size of the model and of the placeholder (w, h, d).
    pub size: (f32, f32, f32),
    pub color: (f32, f32, f32),
    #[serde(default)]
    pub shape: Shape,
    /// A flickering warm light at this height above the origin, and its
    /// strength.
    #[serde(default)]
    pub light: Option<(f32, f32)>,
}

fn one_f32() -> f32 {
    1.0
}

fn one() -> u32 {
    1
}

fn one_f() -> f32 {
    1.0
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stat {
    Light,
    Warmth,
    Silence,
    Reflection,
}

impl Stat {
    /// Catalog key of the name inside a sentence ("light").
    pub fn key(self) -> &'static str {
        match self {
            Stat::Light => "stat.light",
            Stat::Warmth => "stat.warmth",
            Stat::Silence => "stat.silence",
            Stat::Reflection => "stat.reflection",
        }
    }
}

#[derive(Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(default)]
pub struct Atmosphere {
    pub light: i32,
    pub warmth: i32,
    pub silence: i32,
    pub reflection: i32,
}

impl Atmosphere {
    pub fn get(&self, stat: Stat) -> i32 {
        match stat {
            Stat::Light => self.light,
            Stat::Warmth => self.warmth,
            Stat::Silence => self.silence,
            Stat::Reflection => self.reflection,
        }
    }

    /// Only what changes, with signs: "light −2 · silence +1".
    pub fn changes(&self, lang: &Lang) -> String {
        let parts: Vec<String> = [
            (Stat::Light, self.light),
            (Stat::Warmth, self.warmth),
            (Stat::Silence, self.silence),
            (Stat::Reflection, self.reflection),
        ]
        .into_iter()
        .filter(|(_, v)| *v != 0)
        .map(|(s, v)| {
            format!(
                "{} {}{}",
                lang.t(s.key()),
                if v > 0 { "+" } else { "−" },
                v.abs()
            )
        })
        .collect();
        parts.join(" · ")
    }

    /// Every value times `k` (−1 undoes an effect).
    pub fn scale(&mut self, k: i32) {
        self.light *= k;
        self.warmth *= k;
        self.silence *= k;
        self.reflection *= k;
    }

    pub fn add(&mut self, other: &Atmosphere) {
        self.light += other.light;
        self.warmth += other.warmth;
        self.silence += other.silence;
        self.reflection += other.reflection;
    }

    pub fn describe(&self, lang: &Lang) -> String {
        lang.f(
            "atmosphere",
            &[
                ("light", &self.light),
                ("warmth", &self.warmth),
                ("silence", &self.silence),
                ("reflection", &self.reflection),
            ],
        )
    }
}

/// A condition a ghost places on the shop's atmosphere.
#[derive(Deserialize, Clone, Copy, Debug)]
pub enum Need {
    AtLeast(Stat, i32),
    AtMost(Stat, i32),
}

impl Need {
    pub fn is_met(&self, a: &Atmosphere) -> bool {
        match *self {
            Need::AtLeast(s, v) => a.get(s) >= v,
            Need::AtMost(s, v) => a.get(s) <= v,
        }
    }

    /// What the ghost complains about when the condition is not met.
    /// "Stays only with light 3 or more", for the ghost tooltip.
    pub fn describe(&self, lang: &Lang) -> String {
        let (stat, value, key) = match *self {
            Need::AtLeast(s, v) => (s, v, "ghost_tip.needs_at_least"),
            Need::AtMost(s, v) => (s, v, "ghost_tip.needs_at_most"),
        };
        lang.f(key, &[("stat", &lang.t(stat.key())), ("value", &value)])
    }

    pub fn complaint(&self, lang: &Lang) -> String {
        let (stat, bound) = match *self {
            Need::AtLeast(s, _) => (s, "at_least"),
            Need::AtMost(s, _) => (s, "at_most"),
        };
        let stat = match stat {
            Stat::Light => "light",
            Stat::Warmth => "warmth",
            Stat::Silence => "silence",
            Stat::Reflection => "reflection",
        };
        lang.t(&format!("need.{stat}.{bound}"))
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct GhostDef {
    pub id: String,
    /// Placeholder colour until there is an image.
    pub color: (f32, f32, f32),
    pub spawn_weight: f32,
    /// Factor on the product price (vain ghosts pay more …).
    pub pay_multiplier: f32,
    /// Seconds the ghost waits at the counter.
    pub patience_secs: f32,
    /// Pitch of its sounds (1 = normal, higher = a smaller, brighter voice).
    #[serde(default = "one_f32")]
    pub voice: f32,
    #[serde(default)]
    pub needs: Vec<Need>,
    /// Product ids this ghost may ask for.
    pub wishes: Vec<String>,
    /// Allowed coins. Empty = all.
    #[serde(default)]
    pub pays_with: Vec<Coin>,
    #[serde(default)]
    pub quirks: Quirks,
    /// First night this ghost can appear.
    #[serde(default = "one")]
    pub unlock_night: u32,
    /// Picky ghosts only visit a shop with at least this reputation.
    #[serde(default)]
    pub min_reputation: Option<i32>,
}

/// Special behaviour of a ghost type. All chances are 0..1.
#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Quirks {
    /// Chance to knock one item off the shelf it browses.
    pub knock_chance: f32,
    /// While this condition is met the ghost is calm and knocks nothing over.
    pub calm_with: Option<Need>,
    /// Chance that one of the paid coins is counterfeit.
    pub counterfeit_chance: f32,
    /// Chance to forget it has paid and offer to pay a second time.
    pub double_pay_chance: f32,
}

impl GhostDef {
    /// First unmet condition, if any.
    pub fn unmet_need(&self, a: &Atmosphere) -> Option<Need> {
        self.needs.iter().copied().find(|n| !n.is_met(a))
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct DecorDef {
    pub id: String,
    /// Price at the market.
    pub cost: u32,
    /// Shelves only: where each product stands, relative to the decor's
    /// origin (x, height, z). One product per slot.
    #[serde(default)]
    pub slots: Vec<(f32, f32, f32)>,
    #[serde(default)]
    pub effect: Atmosphere,
    pub color: (f32, f32, f32),
    /// Keepsakes cannot be bought; they are gifts from story ghosts.
    #[serde(default)]
    pub keepsake: bool,
    /// (width, height, depth) in metres: placeholder box and model target size.
    #[serde(default = "default_size")]
    pub size: (f32, f32, f32),
    /// Height of the light source for decor with a light effect.
    #[serde(default = "default_light_height")]
    pub light_height: f32,
    /// First night it can be bought.
    #[serde(default = "one")]
    pub unlock_night: u32,
}

impl DecorDef {
    /// Products can be displayed on it.
    pub fn is_shelf(&self) -> bool {
        !self.slots.is_empty()
    }
}

/// Something special about one night (fog, storm, …).
#[derive(Deserialize, Clone, Debug)]
pub struct NightEvent {
    pub id: String,
    pub night: u32,
    /// Multiplies the time between ghosts (above 1 = fewer customers).
    #[serde(default = "one_f")]
    pub spawn_factor: f32,
    /// Multiplies what ghosts pay.
    #[serde(default = "one_f")]
    pub price_factor: f32,
    /// Extra (or fewer) crates from the ferryman.
    #[serde(default)]
    pub crate_bonus: i32,
    /// Added to the shop's atmosphere.
    #[serde(default)]
    pub atmosphere: Atmosphere,
    /// (ghost id, factor) for how often a ghost comes.
    #[serde(default)]
    pub ghost_weights: Vec<(String, f32)>,
    /// Multiplies how long the night lasts.
    #[serde(default = "one_f")]
    pub length_factor: f32,
    /// What it looks like outside (rain, storm, snow, fog), see `weather.rs`.
    #[serde(default)]
    pub weather: Option<crate::weather::Weather>,
}

fn default_size() -> (f32, f32, f32) {
    (0.6, 0.6, 0.6)
}

fn default_light_height() -> f32 {
    1.0
}

/// A story ghost who returns night after night, one chapter per night
/// (`story.ron`). Texts: `story.<ghost>.<n>.intro` / `.thanks`,
/// `story.<ghost>.arrives`, `story.<ghost>.ending`.
#[derive(Deserialize, Clone, Debug)]
pub struct StoryDef {
    /// Ghost id from ghosts.ron (usually with spawn_weight 0).
    pub ghost: String,
    /// The first night this ghost comes.
    #[serde(default = "one")]
    pub start_night: u32,
    /// When the ghost arrives, as a fraction of the night (0..1).
    pub arrive_at: f32,
    pub chapters: Vec<ChapterDef>,
    /// Finishing this story ends the season (the finale). The season lasts
    /// until it is finished, however many nights that takes.
    #[serde(default)]
    pub finale: bool,
    /// Each chapter of this story repairs the harbour lighthouse a little.
    #[serde(default)]
    pub lighthouse: bool,
    /// Each chapter sets the market's tower clock going a little more.
    #[serde(default)]
    pub tower_clock: bool,
    /// Once this story is finished, snow falls over the village.
    #[serde(default)]
    pub snow: bool,
    /// A critter that lives in the shop once this story is finished.
    #[serde(default)]
    pub companion: Option<String>,
    /// A ghost who comes through the door to say goodbye when this story is
    /// finished (texts `finale.<ghost>.arrives`, `finale.<ghost>.farewell`).
    #[serde(default)]
    pub finale_guest: Option<String>,
    /// The first chapter of this story ends the snow (the thaw).
    #[serde(default)]
    pub thaw: bool,
    /// Each chapter puts flowers on the market's windows.
    #[serde(default)]
    pub blossom: bool,
    /// Once this story is finished the ferryman wears his name.
    #[serde(default)]
    pub names_ferryman: bool,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ChapterDef {
    /// Product the ghost needs in this chapter.
    #[serde(default)]
    pub wish: String,
    /// Instead of a ware the ghost asks for one coin from the drawer: the
    /// ferryman's own fare.
    #[serde(default)]
    pub fare: bool,
    /// Decor id of the keepsake left behind.
    #[serde(default)]
    pub keepsake: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Config {
    pub start_coins: Purse,
    pub start_reputation: i32,
    pub crates_per_night: u32,
    pub night_length_secs: f32,
    pub spawn_interval_secs: (f32, f32),
    pub max_ghosts_in_shop: usize,
    pub grid_cols: usize,
    pub grid_rows: usize,
    /// The last night with written content (events, stories) in the full
    /// game; later seasons keep the shop open without a story.
    pub last_night: u32,
    /// Nights per season; each season tells its own story.
    pub season_length: u32,
    /// The last night of the demo (built with `--features demo`). A save from
    /// the end of the demo continues in the full game with the night after.
    pub demo_nights: u32,
    pub base_atmosphere: Atmosphere,
    /// What the ferryman lends a shop that cannot afford a single crate.
    pub ferryman_loan: crate::loan::LoanTerms,
    /// How reputation changes the time between ghosts.
    pub reputation_spawn: ReputationSpawn,
    /// What a good reputation brings besides more customers.
    pub reputation_perks: ReputationPerks,
    /// The shade that steals from shops with a bad reputation.
    pub thief: ThiefConfig,
    /// How much cheaper a trader's special offer of the day is (0.3 = 30 %).
    pub special_discount: f32,
    /// Wares a ghost orders for the next night.
    pub orders: OrdersConfig,
    /// Mishaps during the night (`incidents.ron`).
    pub incidents: IncidentsConfig,
    /// Ghosts who come back as regulars (`regulars.rs`).
    pub regulars: RegularsConfig,
    /// How strongly the ware in the shop window draws ghosts who want it.
    pub showcase_attract: f32,
    /// The part of the floor free of old junk at the start; the rest is
    /// cleared by improvements (`upgrades.ron`).
    pub start_area: StartArea,
}

/// Each reputation point shortens (or, below zero, lengthens) the time
/// between ghosts by `per_point`, within `fastest` and `slowest`.
#[derive(Deserialize, Clone, Copy, Debug)]
pub struct ReputationSpawn {
    pub per_point: f32,
    pub fastest: f32,
    pub slowest: f32,
}

/// How many mishaps a night brings (config.ron).
#[derive(Deserialize, Clone, Copy, Debug)]
pub struct IncidentsConfig {
    /// From this calendar night on.
    pub from_night: u32,
    /// How many per night (at least, at most).
    pub per_night: (u32, u32),
    /// When they happen, as a fraction of the night (from, to).
    pub arrive_at: (f32, f32),
}

/// Regular customers (config.ron).
#[derive(Deserialize, Clone, Copy, Debug)]
pub struct RegularsConfig {
    /// Chance that a ghost served well becomes a regular.
    pub become_chance: f32,
    /// Chance that a ghost of a regular's kind is the regular.
    pub return_chance: f32,
    /// Chance that a returning regular wants its favourite ware.
    pub favourite_chance: f32,
    /// At most this many regulars.
    pub max: usize,
    /// Each visit adds this to the chance of a tip, up to `max_tip_bonus`.
    pub tip_per_visit: f32,
    pub max_tip_bonus: f32,
}

/// Orders for the next night (config.ron).
#[derive(Deserialize, Clone, Copy, Debug)]
pub struct OrdersConfig {
    /// How many orders can wait for one night.
    pub max_per_night: usize,
    /// What a ghost pays on top for an ordered ware (0.5 = 50 % more).
    pub price_bonus: f32,
    /// Reputation lost when an ordered ware is still missing.
    pub missed_penalty: i32,
    /// When the ordering ghosts come back, as a fraction of the night (from, to).
    pub arrive_at: (f32, f32),
}

/// When the shade comes to steal (config.ron).
#[derive(Deserialize, Clone, Copy, Debug)]
pub struct ThiefConfig {
    /// The first night it can come.
    pub from_night: u32,
    /// It only comes while the reputation is below this.
    pub below: i32,
    /// Chance per night.
    pub chance: f32,
    /// When it arrives, as a fraction of the night (from, to).
    pub arrive_at: (f32, f32),
    /// A shop at least this bright drives it away.
    pub scared_by_light: i32,
}

/// Tips for exact change and the Reaper's rare pieces (config.ron).
#[derive(Deserialize, Clone, Copy, Debug)]
pub struct ReputationPerks {
    /// From this reputation on ghosts sometimes leave a tip.
    pub tip_from: i32,
    /// Chance of a tip per point above `tip_from - 1`, up to `tip_chance_max`.
    pub tip_chance_per_point: f32,
    pub tip_chance_max: f32,
    /// From this reputation on a tip can be a candle stub instead of a button.
    pub candle_tip_from: i32,
    /// The Reaper sells rare pieces only from this reputation on.
    pub rare_from: i32,
}

impl ReputationPerks {
    /// Chance that a ghost served with exact change leaves a tip.
    pub fn tip_chance(&self, reputation: i32) -> f32 {
        ((reputation - self.tip_from + 1).max(0) as f32 * self.tip_chance_per_point)
            .min(self.tip_chance_max)
    }
}

impl Config {
    /// Factor on the time between ghosts at this reputation.
    pub fn reputation_spawn_factor(&self, reputation: i32) -> f32 {
        let r = self.reputation_spawn;
        (1.0 - reputation as f32 * r.per_point).clamp(r.fastest, r.slowest)
    }

    /// The last night of this build: the demo stops earlier than the full game.
    pub fn final_night(&self) -> u32 {
        if cfg!(feature = "demo") {
            self.demo_nights.min(self.last_night)
        } else {
            self.last_night
        }
    }
}

#[derive(Resource)]
pub struct GameData {
    pub config: Config,
    pub products: Vec<ProductDef>,
    pub ghosts: Vec<GhostDef>,
    pub decor: Vec<DecorDef>,
    pub stories: Vec<StoryDef>,
    /// Mishaps during the night (`incidents.ron`).
    pub incidents: Vec<crate::incidents::IncidentDef>,
    /// Furniture sets (`sets.ron`).
    pub sets: Vec<crate::sets::SetDef>,
    /// Achievements of the full game (`achievements.ron`).
    pub achievements: Vec<crate::achievements::AchievementDef>,
    /// Broken things in the junk and their repair (`workbench.ron`).
    pub workbench: crate::workbench::WorkbenchDef,
    pub events: Vec<NightEvent>,
    pub props: Vec<PropDef>,
    pub vendors: Vec<VendorDef>,
    pub upgrades: Vec<UpgradeDef>,
}

/// A trader at the village market (`market.ron`).
#[derive(Deserialize, Clone, Debug)]
pub struct VendorDef {
    /// Names and texts: `vendor.<id>.name`, `vendor.<id>.text`. Models:
    /// `assets/models/market/stall_<id>.glb` and `vendor_<id>.glb`.
    pub id: String,
    /// Where the stall stands (x, z) in the market's metres.
    pub pos: (f32, f32),
    /// Turn of the stall about the vertical axis, in degrees.
    #[serde(default)]
    pub turn: f32,
    /// Awning and placeholder colour.
    pub color: (f32, f32, f32),
    pub wares: Vec<Ware>,
    /// How many of its wares are on special offer each day.
    #[serde(default)]
    pub specials: u32,
    /// A seasonal trader comes only from this calendar night …
    #[serde(default = "one")]
    pub from_night: u32,
    /// … until this one.
    #[serde(default)]
    pub until_night: Option<u32>,
}

impl VendorDef {
    /// Is this trader at the market on this calendar night?
    pub fn in_season(&self, calendar: u32) -> bool {
        calendar >= self.from_night && self.until_night.is_none_or(|u| calendar <= u)
    }
}

/// What a trader offers.
#[derive(Deserialize, Clone, Debug, PartialEq)]
pub enum Ware {
    /// Furniture from `decor.ron` (at its cost, once it is unlocked).
    Decor(String),
    /// The raven changes coins.
    Exchange(Exchange),
    /// Makes the coming night last longer; one a day.
    Hourglass,
    /// This many single pieces of different products, a new choice every day.
    RarePieces(u32),
    /// One piece of this product, every day.
    Single(String),
    /// A shop improvement from `upgrades.ron`, bought once.
    Upgrade(String),
}

/// A shop improvement (`upgrades.ron`), bought once at the market.
#[derive(Deserialize, Clone, Debug)]
pub struct UpgradeDef {
    pub id: String,
    pub cost: u32,
    #[serde(default = "one")]
    pub unlock_night: u32,
    /// Another improvement that has to be bought first.
    #[serde(default)]
    pub requires: Option<String>,
    #[serde(default)]
    pub effect: UpgradeEffect,
    /// What stands in the shop once it is bought.
    #[serde(default)]
    pub model: Option<UpgradeModel>,
    /// The letter from Hedda found when this improvement clears old junk
    /// (`letter.<n>`; letter 1 waits on the first evening).
    #[serde(default)]
    pub letter: Option<u32>,
}

/// What an improvement changes; see `upgrades.ron`.
#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(default)]
pub struct UpgradeEffect {
    pub cols: usize,
    pub rows: usize,
    pub extra_crates: i32,
    pub fade_bonus: u32,
    pub spawn_factor: f32,
    pub patience_factor: f32,
    /// A helper ghost serves this many customers a night (`apprentice.rs`).
    pub apprentice: u32,
}

impl Default for UpgradeEffect {
    fn default() -> Self {
        Self {
            cols: 0,
            rows: 0,
            extra_crates: 0,
            fade_bonus: 0,
            spawn_factor: 1.0,
            patience_factor: 1.0,
            apprentice: 0,
        }
    }
}

#[derive(Deserialize, Clone, Copy, Debug)]
pub struct UpgradeModel {
    pub pos: (f32, f32, f32),
    #[serde(default)]
    pub turn: f32,
    pub size: (f32, f32, f32),
    pub color: (f32, f32, f32),
    #[serde(default)]
    pub light_height: f32,
}

/// Columns and rows of the floor that are free of old junk at the start.
#[derive(Deserialize, Clone, Copy, Debug)]
pub struct StartArea {
    pub cols: usize,
    pub rows: usize,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exchange {
    /// A silver spoon for 4 candle stubs and 4 buttons (fee: 1 button).
    SpoonToCandles,
    /// A candle stub for 4 buttons (fee: 1 button).
    CandleToButtons,
    /// 5 candle stubs for a silver spoon (no fee).
    CandlesToSpoon,
}

impl GameData {
    /// Loads all data files. Panics with a readable message if something is wrong.
    pub fn load() -> Self {
        let data = Self {
            config: load_ron("data/config.ron"),
            products: load_ron("data/products.ron"),
            ghosts: load_ron("data/ghosts.ron"),
            decor: load_ron("data/decor.ron"),
            stories: load_ron("data/story.ron"),
            incidents: load_ron("data/incidents.ron"),
            sets: load_ron("data/sets.ron"),
            workbench: load_ron("data/workbench.ron"),
            achievements: load_ron("data/achievements.ron"),
            events: load_ron("data/events.ron"),
            props: load_ron("data/props.ron"),
            vendors: load_ron("data/market.ron"),
            upgrades: load_ron("data/upgrades.ron"),
        };
        data.validate();
        data
    }

    fn validate(&self) {
        let c = &self.config;
        for set in &self.sets {
            assert!(set.decor.len() >= 2, "set '{}' needs two pieces", set.id);
            for d in &set.decor {
                assert!(
                    self.decor(d).is_some(),
                    "set '{}' has unknown decor '{d}'",
                    set.id
                );
            }
            for (g, _) in &set.ghost_weights {
                assert!(
                    self.ghost_index(g).is_some(),
                    "set '{}' draws unknown ghost '{g}'",
                    set.id
                );
            }
        }
        assert!(
            c.start_area.cols <= c.grid_cols && c.start_area.rows <= c.grid_rows,
            "start_area is bigger than the grid"
        );
        for u in &self.upgrades {
            let sellers = self
                .vendors
                .iter()
                .filter(|v| v.wares.contains(&Ware::Upgrade(u.id.clone())))
                .count();
            assert_eq!(
                sellers, 1,
                "upgrade '{}' must be sold by exactly one trader",
                u.id
            );
            if let Some(r) = &u.requires {
                assert!(
                    self.upgrade(r).is_some(),
                    "upgrade '{}' requires unknown '{r}'",
                    u.id
                );
            }
        }
        for v in &self.vendors {
            for w in &v.wares {
                if let Ware::Upgrade(id) = w {
                    assert!(
                        self.upgrade(id).is_some(),
                        "vendor '{}' sells unknown upgrade '{id}'",
                        v.id
                    );
                }
            }
        }
        for v in &self.vendors {
            for w in &v.wares {
                if let Ware::Decor(d) = w {
                    assert!(
                        self.decor(d).is_some_and(|d| !d.keepsake),
                        "vendor '{}' sells '{}', which is not buyable decor",
                        v.id,
                        d
                    );
                }
            }
        }
        for d in self.decor.iter().filter(|d| !d.keepsake) {
            assert!(
                self.vendors
                    .iter()
                    .any(|v| v.wares.contains(&Ware::Decor(d.id.clone()))),
                "nobody at the market sells '{}'",
                d.id
            );
        }
        for p in &self.products {
            assert!(
                CONTAINERS.contains(&p.container.as_str()),
                "product '{}' comes in an unknown container '{}' (known: {:?})",
                p.id,
                p.container,
                CONTAINERS
            );
        }
        for g in &self.ghosts {
            assert!(!g.wishes.is_empty(), "ghost '{}' has no wishes", g.id);
            assert!(
                g.wishes.iter().any(|w| self
                    .product(w)
                    .is_some_and(|p| p.unlock_night <= g.unlock_night)),
                "ghost '{}' wishes only for products unlocked after it appears",
                g.id
            );
            for w in &g.wishes {
                assert!(
                    self.product(w).is_some(),
                    "ghost '{}' wishes for unknown product '{}'",
                    g.id,
                    w
                );
            }
        }
        for story in &self.stories {
            assert!(
                self.ghost_index(&story.ghost).is_some(),
                "story ghost '{}' is not in ghosts.ron",
                story.ghost
            );
            assert!(
                !story.chapters.is_empty(),
                "story '{}' has no chapters",
                story.ghost
            );
        }
        for e in &self.events {
            for (g, _) in &e.ghost_weights {
                assert!(
                    self.ghost_index(g).is_some(),
                    "event '{}' names unknown ghost '{}'",
                    e.id,
                    g
                );
            }
        }
        for story in &self.stories {
            if let Some(g) = &story.finale_guest {
                assert!(
                    self.ghost_index(g).is_some(),
                    "finale guest '{g}' is not in ghosts.ron"
                );
            }
            for (i, c) in story.chapters.iter().enumerate() {
                assert!(
                    c.fare || self.product(&c.wish).is_some(),
                    "{} chapter {} wishes for unknown product '{}'",
                    story.ghost,
                    i + 1,
                    c.wish
                );
                if let Some(k) = &c.keepsake {
                    assert!(
                        self.decor(k).is_some_and(|d| d.keepsake),
                        "{} chapter {} gives '{}', which is not a keepsake in decor.ron",
                        story.ghost,
                        i + 1,
                        k
                    );
                }
            }
        }
        for a in &self.achievements {
            if let crate::achievements::Goal::Story(g) = &a.goal {
                assert!(
                    self.stories.iter().any(|s| &s.ghost == g),
                    "achievement '{}' waits for an unknown story '{g}'",
                    a.id
                );
            }
        }
        for v in &self.vendors {
            for w in &v.wares {
                match w {
                    Ware::Single(p) => {
                        assert!(self.product(p).is_some(), "{}: unknown product '{p}'", v.id)
                    }
                    Ware::Decor(d) => {
                        assert!(self.decor(d).is_some(), "{}: unknown decor '{d}'", v.id)
                    }
                    _ => {}
                }
            }
        }
        for j in &self.workbench.junk {
            assert!(
                self.product(&j.product).is_some(),
                "workbench: unknown product '{}'",
                j.product
            );
        }
        assert!(
            self.decor(crate::workbench::WORKBENCH).is_some(),
            "decor.ron has no workbench"
        );
        for u in &self.upgrades {
            if let Some(n) = u.letter {
                assert!(
                    (1..=HEDDA_LETTERS).contains(&n),
                    "improvement '{}' reveals unknown letter {}",
                    u.id,
                    n
                );
            }
        }
    }

    /// Is the story with this index still running on this calendar night:
    /// started, and not every chapter done?
    pub fn story_due(&self, i: usize, calendar: u32, chapter: usize) -> bool {
        let s = &self.stories[i];
        calendar >= s.start_night && chapter < s.chapters.len()
    }

    /// The season a calendar night belongs to.
    pub fn season_of(&self, calendar: u32) -> u32 {
        (calendar.max(1) - 1) / self.config.season_length + 1
    }

    pub fn event(&self, night: u32) -> Option<&NightEvent> {
        self.events.iter().find(|e| e.night == night)
    }

    /// Crates the ferryman brings tonight (at least one).
    pub fn crates_tonight(&self, night: u32) -> u32 {
        let bonus = self.event(night).map_or(0, |e| e.crate_bonus);
        (self.config.crates_per_night as i32 + bonus).max(1) as u32
    }

    /// How much more (or less) often a ghost comes tonight.
    pub fn ghost_weight_factor(&self, night: u32, ghost: &str) -> f32 {
        self.event(night)
            .and_then(|e| e.ghost_weights.iter().find(|(id, _)| id == ghost))
            .map_or(1.0, |(_, f)| *f)
    }

    /// Chance of each ghost type (same order as `ghosts`) to be the next one
    /// through the door tonight, relative to the others. A ghost whose needs
    /// the atmosphere does not meet still comes, but less often.
    pub fn ghost_odds(&self, night: u32, atmosphere: &Atmosphere, reputation: i32) -> Vec<f32> {
        self.ghosts
            .iter()
            .map(|g| {
                if g.unlock_night > night || g.min_reputation.is_some_and(|m| reputation < m) {
                    return 0.0;
                }
                let fits = g.unmet_need(atmosphere).is_none();
                g.spawn_weight
                    * if fits { 1.0 } else { UNFIT_GHOST_ODDS }
                    * self.ghost_weight_factor(night, &g.id)
            })
            .collect()
    }

    /// What a ghost may wish for tonight: only what the ferryman already sells.
    pub fn wishes_tonight<'a>(&'a self, ghost: &'a GhostDef, night: u32) -> Vec<&'a str> {
        ghost
            .wishes
            .iter()
            .filter(|w| self.product(w).is_some_and(|p| p.unlock_night <= night))
            .map(String::as_str)
            .collect()
    }

    pub fn price_factor(&self, night: u32) -> f32 {
        self.event(night).map_or(1.0, |e| e.price_factor)
    }

    pub fn spawn_factor(&self, night: u32) -> f32 {
        self.event(night).map_or(1.0, |e| e.spawn_factor)
    }

    /// Real seconds this night lasts (before the Reaper's hourglass).
    pub fn night_length(&self, night: u32) -> f32 {
        self.config.night_length_secs * self.event(night).map_or(1.0, |e| e.length_factor)
    }

    /// Ids of products, decor and ghosts that become available on this night.
    pub fn unlocked_on(&self, night: u32) -> (Vec<&str>, Vec<&str>, Vec<&str>) {
        let products = self
            .products
            .iter()
            .filter(|p| p.unlock_night == night)
            .map(|p| p.id.as_str());
        let decor = self
            .decor
            .iter()
            .filter(|d| d.unlock_night == night)
            .map(|d| d.id.as_str());
        let ghosts = self
            .ghosts
            .iter()
            .filter(|g| g.unlock_night == night)
            .map(|g| g.id.as_str());
        (products.collect(), decor.collect(), ghosts.collect())
    }

    pub fn ghost_index(&self, id: &str) -> Option<usize> {
        self.ghosts.iter().position(|g| g.id == id)
    }

    pub fn product(&self, id: &str) -> Option<&ProductDef> {
        self.products.iter().find(|p| p.id == id)
    }

    pub fn upgrade(&self, id: &str) -> Option<&UpgradeDef> {
        self.upgrades.iter().find(|u| u.id == id)
    }

    pub fn decor(&self, id: &str) -> Option<&DecorDef> {
        self.decor.iter().find(|d| d.id == id)
    }
}

fn load_ron<T: for<'de> Deserialize<'de>>(rel: &str) -> T {
    let path = assets_dir().join(rel);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    ron::from_str(&text).unwrap_or_else(|e| panic!("error in {}: {e}", path.display()))
}

/// Same lookup as Bevy's AssetServer: `BEVY_ASSET_ROOT`, else the Cargo project
/// (with `cargo run`), else the directory of the executable.
pub fn assets_dir() -> PathBuf {
    let base = std::env::var_os("BEVY_ASSET_ROOT")
        .or_else(|| std::env::var_os("CARGO_MANIFEST_DIR"))
        .map(PathBuf::from)
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        })
        .unwrap_or_default();
    base.join("assets")
}

/// Is there an image file? Then it is used instead of the coloured placeholder.
pub fn asset_exists(rel: &str) -> bool {
    assets_dir().join(rel).is_file()
}

pub fn rgb((r, g, b): (f32, f32, f32)) -> Color {
    Color::srgb(r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reputation_brings_tips_and_picky_guests() {
        let data = GameData::load();
        let perks = data.config.reputation_perks;
        assert_eq!(perks.tip_chance(perks.tip_from - 1), 0.0);
        assert!(perks.tip_chance(perks.tip_from) > 0.0);
        assert_eq!(perks.tip_chance(1000), perks.tip_chance_max);
        // A picky ghost stays away below its reputation and comes above it.
        let (i, g) = data
            .ghosts
            .iter()
            .enumerate()
            .find(|(_, g)| g.min_reputation.is_some())
            .expect("a picky ghost");
        let min = g.min_reputation.unwrap();
        let bright = Atmosphere {
            light: 5,
            warmth: 5,
            silence: 5,
            reflection: 5,
        };
        let night = data.config.last_night;
        assert_eq!(data.ghost_odds(night, &bright, min - 1)[i], 0.0);
        assert!(data.ghost_odds(night, &bright, min)[i] > 0.0);
    }

    #[test]
    fn night_events_change_the_night() {
        let data = GameData::load();
        let base = data.config.crates_per_night;
        let storm = data
            .events
            .iter()
            .find(|e| e.crate_bonus < 0)
            .expect("a night with fewer crates");
        assert!(data.crates_tonight(storm.night) < base);
        assert!(data.crates_tonight(storm.night) >= 1);
        // A night without an event changes nothing.
        let quiet = 1000;
        assert_eq!(data.crates_tonight(quiet), base);
        assert_eq!(data.price_factor(quiet), 1.0);
        assert_eq!(data.ghost_weight_factor(quiet, "shy"), 1.0);
        // Every night of this build has an event, and every ghost unlocks within the
        // full game (the demo stops earlier and leaves the later ghosts locked).
        for night in 1..=data.config.final_night() {
            assert!(data.event(night).is_some(), "night {night} has no event");
        }
        for g in &data.ghosts {
            assert!(
                g.unlock_night <= data.config.last_night,
                "{} never appears",
                g.id
            );
        }
        let (products, _, ghosts) = data.unlocked_on(2);
        assert!(!products.is_empty() && !ghosts.is_empty());
    }

    /// Loads the real data files, so a broken RON file fails `cargo test`.
    #[test]
    fn data_files_are_valid() {
        let data = GameData::load();
        // Every id has its texts in the English catalog.
        let lang = Lang::load("en");
        let has = |key: String| lang.t(&key) != key;
        for p in &data.products {
            assert!(
                has(format!("product.{}.name", p.id)),
                "no name for {}",
                p.id
            );
            assert!(
                lang.count(&format!("product.{}.hint.", p.id)) > 0,
                "no hints for {}",
                p.id
            );
        }
        for g in &data.ghosts {
            assert!(
                has(format!("ghost.{}.name", g.id)) && has(format!("ghost.{}.the", g.id)),
                "texts for {}",
                g.id
            );
        }
        for d in &data.decor {
            assert!(has(format!("decor.{}.name", d.id)), "no name for {}", d.id);
        }
        for story in &data.stories {
            let g = &story.ghost;
            assert!(
                has(format!("story.{g}.arrives")) && has(format!("story.{g}.ending")),
                "story {g}"
            );
            for n in 1..=story.chapters.len() {
                assert!(
                    has(format!("story.{g}.{n}.intro")) && has(format!("story.{g}.{n}.thanks")),
                    "story {g} {n}"
                );
            }
        }
        for g in data.stories.iter().filter_map(|s| s.finale_guest.as_ref()) {
            assert!(
                has(format!("finale.{g}.arrives")) && has(format!("finale.{g}.farewell")),
                "finale guest {g}"
            );
        }
        for a in &data.achievements {
            assert!(
                has(format!("achievement.{}.name", a.id))
                    && has(format!("achievement.{}.text", a.id)),
                "achievement {}",
                a.id
            );
        }
        // Every season that ends with a finale has an epilogue.
        for story in data.stories.iter().filter(|s| s.finale) {
            let season = data.season_of(story.start_night);
            assert!(has(format!("season.{season}.epilogue")), "season {season}");
        }
        for n in 1..=HEDDA_LETTERS {
            assert!(has(format!("letter.{n}")), "letter {n}");
        }
        assert!(lang.count("tell.genuine.") > 0 && lang.count("tell.fake.") > 0);
        for e in &data.events {
            assert!(
                has(format!("event.{}.name", e.id)) && has(format!("event.{}.text", e.id)),
                "event {}",
                e.id
            );
        }
        assert!(!data.products.is_empty());
        // Every product is wished for by some ghost or the story.
        for p in &data.products {
            let wished = data.ghosts.iter().any(|g| g.wishes.contains(&p.id))
                || data
                    .stories
                    .iter()
                    .any(|s| s.chapters.iter().any(|c| c.wish == p.id));
            assert!(wished, "nobody ever asks for '{}'", p.id);
        }
    }

    /// Largest number of decor items the satisfiability search below combines.
    const MAX_DECOR_COMBINATION: usize = 4;

    /// Is there a combination of up to `left` more items from `decor[start..]` (repeats
    /// allowed) that, added to `a`, meets every need?
    fn can_meet(
        needs: &[Need],
        decor: &[&DecorDef],
        start: usize,
        a: Atmosphere,
        left: usize,
    ) -> bool {
        if needs.iter().all(|n| n.is_met(&a)) {
            return true;
        }
        left > 0
            && (start..decor.len()).any(|i| {
                let mut next = a;
                next.add(&decor[i].effect);
                can_meet(needs, decor, i, next, left - 1)
            })
    }

    /// Every ghost can be made to stay on every night it can appear: some combination
    /// of decor that can be bought by then meets all its needs, including the night's event.
    #[test]
    fn every_ghost_can_be_satisfied() {
        let data = GameData::load();
        for g in &data.ghosts {
            for night in g.unlock_night..=data.config.last_night {
                let decor: Vec<&DecorDef> = data
                    .decor
                    .iter()
                    .filter(|d| !d.keepsake && d.unlock_night <= night)
                    .collect();
                let mut a = data.config.base_atmosphere;
                if let Some(e) = data.event(night) {
                    a.add(&e.atmosphere);
                }
                assert!(
                    can_meet(&g.needs, &decor, 0, a, MAX_DECOR_COMBINATION),
                    "no decor of night {night} satisfies '{}'",
                    g.id
                );
            }
        }
    }
}
