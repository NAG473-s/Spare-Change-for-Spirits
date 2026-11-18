//! Game data from `assets/data/*.ron`.
//!
//! Products, ghosts, decor and balancing values live in RON files so they can
//! be tuned without recompiling (restarting the game is enough).

use std::path::PathBuf;

use bevy::prelude::*;
use serde::Deserialize;

use crate::economy::{Coin, Purse};

#[derive(Deserialize, Clone, Debug)]
pub struct ProductDef {
    pub id: String,
    pub name: String,
    /// Base price in button units.
    pub price: u32,
    /// Price of one crate from the ferryman.
    pub crate_cost: u32,
    /// Units per crate.
    pub crate_size: u32,
    /// After this many nights in the shop the item fades away.
    pub fade_after: u32,
    /// Hints ghosts use when asking for this product.
    pub hints: Vec<String>,
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

    pub fn describe(&self) -> String {
        format!(
            "Light {} · Warmth {} · Silence {} · Reflection {}",
            self.light, self.warmth, self.silence, self.reflection
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
    pub fn complaint(&self) -> &'static str {
        match *self {
            Need::AtMost(Stat::Light, _) => "too bright",
            Need::AtLeast(Stat::Light, _) => "too dark",
            Need::AtMost(Stat::Warmth, _) => "too warm",
            Need::AtLeast(Stat::Warmth, _) => "too cold",
            Need::AtMost(Stat::Silence, _) => "too quiet",
            Need::AtLeast(Stat::Silence, _) => "too noisy",
            Need::AtMost(Stat::Reflection, _) => "too many mirrors",
            Need::AtLeast(Stat::Reflection, _) => "no mirror",
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct GhostDef {
    pub id: String,
    pub name: String,
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
    pub name: String,
    pub cost: u32,
    #[serde(default)]
    pub is_shelf: bool,
    #[serde(default)]
    pub effect: Atmosphere,
    pub color: (f32, f32, f32),
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
    pub nights_in_demo: u32,
    pub base_atmosphere: Atmosphere,
    /// Assist mode: shows the change due at the register.
    pub show_change_due: bool,
}

#[derive(Resource)]
pub struct GameData {
    pub config: Config,
    pub products: Vec<ProductDef>,
    pub ghosts: Vec<GhostDef>,
    pub decor: Vec<DecorDef>,
}

impl GameData {
    /// Loads all data files. Panics with a readable message if something is wrong.
    pub fn load() -> Self {
        let data = Self {
            config: load_ron("data/config.ron"),
            products: load_ron("data/products.ron"),
            ghosts: load_ron("data/ghosts.ron"),
            decor: load_ron("data/decor.ron"),
        };
        data.validate();
        data
    }

    fn validate(&self) {
        for g in &self.ghosts {
            assert!(!g.wishes.is_empty(), "ghost '{}' has no wishes", g.id);
            for w in &g.wishes {
                assert!(
                    self.product(w).is_some(),
                    "ghost '{}' wishes for unknown product '{}'",
                    g.id,
                    w
                );
            }
        }
        for p in &self.products {
            assert!(!p.hints.is_empty(), "product '{}' has no hints", p.id);
        }
    }

    pub fn product(&self, id: &str) -> Option<&ProductDef> {
        self.products.iter().find(|p| p.id == id)
    }

    pub fn product_name<'a>(&'a self, id: &'a str) -> &'a str {
        self.product(id).map(|p| p.name.as_str()).unwrap_or(id)
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
