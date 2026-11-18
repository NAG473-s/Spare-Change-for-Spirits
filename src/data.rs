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
    #[serde(default)]
    pub quirks: Quirks,
    /// `name` is a proper name ("Edwin"), so texts use it without "the".
    #[serde(default)]
    pub proper_name: bool,
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
    /// Name for the start of a sentence: "The Shy Ghost" or "Edwin".
    pub fn the_cap(&self) -> String {
        if self.proper_name {
            self.name.clone()
        } else {
            format!("The {}", self.name)
        }
    }

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
    /// Keepsakes cannot be bought; they are gifts from story ghosts.
    #[serde(default)]
    pub keepsake: bool,
}

/// A story ghost who returns night after night.
#[derive(Deserialize, Clone, Debug)]
pub struct StoryDef {
    /// Ghost id from ghosts.ron (usually with spawn_weight 0).
    pub ghost: String,
    /// Display name used in texts, e.g. "Edwin".
    pub name: String,
    /// When the ghost arrives, as a fraction of the night (0..1).
    pub arrive_at: f32,
    pub chapters: Vec<ChapterDef>,
    /// Shown when the last chapter is completed.
    pub ending: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ChapterDef {
    /// Product the ghost needs in this chapter.
    pub wish: String,
    /// What the ghost says at the counter (replaces the product hint).
    pub intro: String,
    /// What the ghost says when served correctly.
    pub thanks: String,
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
    pub nights_in_demo: u32,
    pub base_atmosphere: Atmosphere,
    /// Assist mode: shows the change due at the register.
    pub show_change_due: bool,
    /// Descriptions of genuine coins at the register.
    pub coin_tells_genuine: Vec<String>,
    /// Descriptions that give a counterfeit coin away.
    pub coin_tells_fake: Vec<String>,
}

/// Tips for the first night, one per situation.
#[derive(Deserialize, Clone, Debug)]
pub struct TipsDef {
    pub order: String,
    pub shelve: String,
    pub open: String,
    pub browse: String,
    pub whisper: String,
    pub change: String,
    pub double_pay: String,
    pub first_sale: String,
    pub morning: String,
}

#[derive(Resource)]
pub struct GameData {
    pub config: Config,
    pub products: Vec<ProductDef>,
    pub ghosts: Vec<GhostDef>,
    pub decor: Vec<DecorDef>,
    pub story: StoryDef,
    pub tips: TipsDef,
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
            tips: load_ron("data/tips.ron"),
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
        assert!(
            self.ghost_index(&self.story.ghost).is_some(),
            "story ghost '{}' is not in ghosts.ron",
            self.story.ghost
        );
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
        assert!(
            !self.config.coin_tells_genuine.is_empty() && !self.config.coin_tells_fake.is_empty(),
            "config.ron needs coin_tells_genuine and coin_tells_fake"
        );
    }

    pub fn ghost_index(&self, id: &str) -> Option<usize> {
        self.ghosts.iter().position(|g| g.id == id)
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Loads the real data files, so a broken RON file fails `cargo test`.
    #[test]
    fn data_files_are_valid() {
        let data = GameData::load();
        assert!(!data.products.is_empty());
        // Every product is wished for by some ghost or the story.
        for p in &data.products {
            let wished = data.ghosts.iter().any(|g| g.wishes.contains(&p.id))
                || data.story.chapters.iter().any(|c| c.wish == p.id);
            assert!(wished, "nobody ever asks for '{}'", p.id);
        }
    }
}
