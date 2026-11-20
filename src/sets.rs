//! Furniture sets (`assets/data/sets.ron`): once every piece of a set stands
//! in the shop, it draws certain ghosts and adds to the atmosphere. Also the
//! odds of tonight's ghosts with everything that draws them: the sets and the
//! ware in the shop window (`showcase.rs`).

use serde::Deserialize;

use crate::data::{Atmosphere, GameData};
use crate::day_cycle::Progress;
use crate::inventory::Stock;
use crate::shop::{ShopLayout, Storeroom};

/// One set. Text: `set.<id>.name`.
#[derive(Deserialize, Clone, Debug)]
pub struct SetDef {
    pub id: String,
    /// The decor ids that make it up.
    pub decor: Vec<String>,
    #[serde(default)]
    pub atmosphere: Atmosphere,
    /// (ghost id, factor) on how often a ghost comes.
    #[serde(default)]
    pub ghost_weights: Vec<(String, f32)>,
}

impl SetDef {
    /// How many of its pieces stand in the shop.
    pub fn placed(&self, layout: &ShopLayout) -> usize {
        self.decor
            .iter()
            .filter(|d| layout.cells.iter().any(|c| c.decor.as_deref() == Some(d)))
            .count()
    }

    pub fn complete(&self, layout: &ShopLayout) -> bool {
        self.placed(layout) == self.decor.len()
    }

    /// The player has at least one piece (placed or in the storeroom), so the
    /// set is worth showing.
    pub fn started(&self, layout: &ShopLayout, storeroom: &Storeroom) -> bool {
        self.placed(layout) > 0 || self.decor.iter().any(|d| storeroom.count(d) > 0)
    }
}

/// The sets complete in the shop.
pub fn complete<'a>(
    data: &'a GameData,
    layout: &'a ShopLayout,
) -> impl Iterator<Item = &'a SetDef> {
    data.sets.iter().filter(|s| s.complete(layout))
}

/// What the complete sets add to the atmosphere.
pub fn atmosphere(data: &GameData, layout: &ShopLayout) -> Atmosphere {
    let mut a = Atmosphere::default();
    for s in complete(data, layout) {
        a.add(&s.atmosphere);
    }
    a
}

/// The ware in the shop window, if there is one left in stock to show.
pub fn showcased<'a>(progress: &'a Progress, stock: &Stock) -> Option<&'a str> {
    progress
        .journal
        .showcase
        .as_deref()
        .filter(|p| stock.count(p) > 0)
}

/// How likely each ghost type comes tonight: the reputation, the atmosphere
/// and the event, then the complete sets and the shop window.
pub fn odds_tonight(
    data: &GameData,
    layout: &ShopLayout,
    progress: &Progress,
    stock: &Stock,
) -> Vec<f32> {
    let night = progress.calendar(data);
    let mut odds = data.ghost_odds(night, &layout.atmosphere(data, night), progress.reputation);
    for set in complete(data, layout) {
        for (ghost, factor) in &set.ghost_weights {
            if let Some(i) = data.ghost_index(ghost) {
                odds[i] *= factor;
            }
        }
    }
    if let Some(ware) = showcased(progress, stock) {
        for (i, g) in data.ghosts.iter().enumerate() {
            if g.wishes.iter().any(|w| w == ware) {
                odds[i] *= 1.0 + data.config.showcase_attract;
            }
        }
    }
    odds
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_complete_set_draws_its_ghosts() {
        let data = GameData::load();
        let mut layout = ShopLayout::new(data.config.grid_cols, data.config.grid_rows);
        let progress = Progress::new(0);
        let stock = Stock::default();
        let set = data.sets.iter().find(|s| s.id == "dressing_room").unwrap();
        let vain = data.ghost_index("vain").unwrap();
        let before = odds_tonight(&data, &layout, &progress, &stock)[vain];
        for (i, d) in set.decor.iter().enumerate() {
            layout.cells[i].decor = Some(d.clone());
        }
        assert!(set.complete(&layout));
        let after = odds_tonight(&data, &layout, &progress, &stock)[vain];
        assert!(after > before, "{after} > {before}");
        assert_eq!(atmosphere(&data, &layout).reflection, 1);
    }
}
