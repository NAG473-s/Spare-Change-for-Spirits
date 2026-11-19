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
}

fn default_size() -> (f32, f32, f32) {
    (0.6, 0.6, 0.6)
}

fn default_light_height() -> f32 {
    1.0
}

/// A story ghost who returns night after night.
#[derive(Deserialize, Clone, Debug)]
pub struct StoryDef {
    /// Ghost id from ghosts.ron (usually with spawn_weight 0).
    pub ghost: String,
    /// When the ghost arrives, as a fraction of the night (0..1).
    pub arrive_at: f32,
    pub chapters: Vec<ChapterDef>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ChapterDef {
    /// Product the ghost needs in this chapter.
    pub wish: String,
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
    /// The last night of the full game.
    pub last_night: u32,
    /// The last night of the demo (built with `--features demo`). A save from
    /// the end of the demo continues in the full game with the night after.
    pub demo_nights: u32,
    pub base_atmosphere: Atmosphere,
    /// Assist mode: shows the change due at the register.
    pub show_change_due: bool,
}

impl Config {
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
    pub story: StoryDef,
    pub events: Vec<NightEvent>,
    pub props: Vec<PropDef>,
    pub vendors: Vec<VendorDef>,
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
            story: load_ron("data/story.ron"),
            events: load_ron("data/events.ron"),
            props: load_ron("data/props.ron"),
            vendors: load_ron("data/market.ron"),
        };
        data.validate();
        data
    }

    fn validate(&self) {
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
        assert!(
            self.ghost_index(&self.story.ghost).is_some(),
            "story ghost '{}' is not in ghosts.ron",
            self.story.ghost
        );
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
        for (i, c) in self.story.chapters.iter().enumerate() {
            assert!(
                self.product(&c.wish).is_some(),
                "story chapter {} wishes for unknown product '{}'",
                i + 1,
                c.wish
            );
            if let Some(k) = &c.keepsake {
                assert!(
                    self.decor(k).is_some_and(|d| d.keepsake),
                    "story chapter {} gives '{}', which is not a keepsake in decor.ron",
                    i + 1,
                    k
                );
            }
        }
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

    pub fn price_factor(&self, night: u32) -> f32 {
        self.event(night).map_or(1.0, |e| e.price_factor)
    }

    pub fn spawn_factor(&self, night: u32) -> f32 {
        self.event(night).map_or(1.0, |e| e.spawn_factor)
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
        // Every night of the demo has an event, and every later ghost unlocks within the demo.
        for night in 1..=data.config.final_night() {
            assert!(data.event(night).is_some(), "night {night} has no event");
        }
        for g in &data.ghosts {
            assert!(
                g.unlock_night <= data.config.final_night(),
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
        for n in 1..=data.story.chapters.len() {
            assert!(
                has(format!("story.{n}.intro")) && has(format!("story.{n}.thanks")),
                "story {n}"
            );
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
                || data.story.chapters.iter().any(|c| c.wish == p.id);
            assert!(wished, "nobody ever asks for '{}'", p.id);
        }
    }
}
