//! Stock, and how many crates the ferryman has brought this evening.

use std::collections::BTreeMap;

use bevy::prelude::*;

use crate::data::GameData;
use crate::day_cycle::Phase;

pub struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Stock>()
            .init_resource::<CratesOrdered>()
            // Reset for the next evening; a loaded save keeps its own count.
            .add_systems(OnEnter(Phase::Morning), clear_orders);
    }
}

pub(crate) fn clear_orders(
    mut crates: ResMut<CratesOrdered>,
    mut bought: ResMut<crate::ferry::BoughtTonight>,
) {
    crates.0 = 0;
    bought.0.clear();
}

/// Per product, a list with the age (in nights) of every unit.
#[derive(Resource, Default)]
pub struct Stock {
    units: BTreeMap<String, Vec<u32>>,
}

impl Stock {
    /// All units with their ages, for saving.
    pub fn units(&self) -> &BTreeMap<String, Vec<u32>> {
        &self.units
    }

    pub fn from_units(units: BTreeMap<String, Vec<u32>>) -> Self {
        Self { units }
    }

    pub fn count(&self, id: &str) -> u32 {
        self.units.get(id).map_or(0, |v| v.len() as u32)
    }

    pub fn add(&mut self, id: &str, n: u32) {
        self.units
            .entry(id.to_string())
            .or_default()
            .extend(std::iter::repeat_n(0, n as usize));
    }

    /// Removes `n` units that arrived today (age 0), e.g. a crate given back
    /// to the ferryman. Changes nothing if there are not that many.
    pub fn remove_new(&mut self, id: &str, n: u32) -> bool {
        let Some(units) = self.units.get_mut(id) else {
            return false;
        };
        if units.iter().filter(|age| **age == 0).count() < n as usize {
            return false;
        }
        let mut left = n;
        units.retain(|age| {
            if *age == 0 && left > 0 {
                left -= 1;
                false
            } else {
                true
            }
        });
        true
    }

    /// Sells the oldest unit.
    pub fn take_one(&mut self, id: &str) -> bool {
        let Some(units) = self.units.get_mut(id) else {
            return false;
        };
        let Some((idx, _)) = units.iter().enumerate().max_by_key(|(_, age)| **age) else {
            return false;
        };
        units.swap_remove(idx);
        true
    }

    /// Counts only the units that the coming dawn would remove.
    pub fn fading_next_dawn(&self, id: &str, data: &GameData, bonus: u32) -> usize {
        let Some(product) = data.product(id) else {
            return 0;
        };
        let limit = product.fade_after.saturating_add(bonus);
        self.units.get(id).map_or(0, |ages| {
            ages.iter()
                .filter(|age| age.saturating_add(1) >= limit)
                .count()
        })
    }

    /// Ages everything by one night and removes faded items.
    /// Returns how much of what has faded.
    /// `bonus`: nights the wares last longer (herb sachets in the storeroom).
    pub fn age_and_fade(&mut self, data: &GameData, bonus: u32) -> Vec<(String, u32)> {
        let mut faded = Vec::new();
        for (id, units) in &mut self.units {
            let Some(product) = data.product(id) else {
                continue;
            };
            let limit = product.fade_after.saturating_add(bonus);
            for age in units.iter_mut() {
                *age = age.saturating_add(1);
            }
            let before = units.len();
            units.retain(|age| *age < limit);
            let lost = (before - units.len()) as u32;
            if lost > 0 {
                faded.push((id.clone(), lost));
            }
        }
        faded
    }
}

/// How many crates have been ordered this evening.
#[derive(Resource, Default)]
pub struct CratesOrdered(pub u32);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_new_units_can_be_given_back() {
        let mut stock = Stock::from_units(BTreeMap::from([("tea_tin".to_string(), vec![2, 0, 0])]));
        assert!(!stock.remove_new("tea_tin", 3));
        assert_eq!(stock.count("tea_tin"), 3);
        assert!(stock.remove_new("tea_tin", 2));
        assert_eq!(stock.units()["tea_tin"], vec![2]);
    }

    #[test]
    fn the_oldest_unit_is_sold_first() {
        let mut stock = Stock::from_units(BTreeMap::from([("tea_tin".to_string(), vec![0, 2, 1])]));
        assert!(stock.take_one("tea_tin"));
        let mut left = stock.units()["tea_tin"].clone();
        left.sort();
        assert_eq!(left, vec![0, 1]);
        assert!(!stock.take_one("music_box"), "nothing of it in stock");
        assert!(stock.take_one("tea_tin") && stock.take_one("tea_tin"));
        assert!(!stock.take_one("tea_tin"), "sold out");
        assert_eq!(stock.count("tea_tin"), 0);
    }

    #[test]
    fn dawn_count_matches_the_units_lost_after_a_sale_or_shelf_life_upgrade() {
        let data = GameData::load();
        let id = "tea_tin";
        let last_night = data.product(id).unwrap().fade_after - 1;
        let mut stock = Stock::from_units(BTreeMap::from([
            (id.into(), vec![0, last_night, 0, last_night, 0]),
            ("unknown".into(), vec![u32::MAX]),
        ]));
        assert_eq!(
            (stock.count(id), stock.fading_next_dawn(id, &data, 0)),
            (5, 2)
        );
        assert_eq!(stock.fading_next_dawn(id, &data, 1), 0);
        assert!(stock.take_one(id));
        assert_eq!(stock.fading_next_dawn(id, &data, 0), 1);
        assert_eq!(crate::accounts::fading_next_dawn(&stock, &data, 0), [id]);
        assert_eq!(stock.age_and_fade(&data, 0), [(id.to_string(), 1)]);
        assert_eq!(stock.fading_next_dawn(id, &data, 0), 0);
        assert_eq!(stock.count("unknown"), 1);
    }

    /// A unit fades at the dawn on which its age reaches `fade_after`;
    /// the herb sachets' bonus adds nights, and unknown ids never fade.
    #[test]
    fn wares_fade_after_their_nights() {
        let data = GameData::load();
        let product = &data.products[0];
        let id = product.id.as_str();
        let nights = product.fade_after;
        let mut stock = Stock::default();
        stock.add(id, 2);
        stock.add("not_a_product", 1);
        for _ in 1..nights {
            assert!(stock.age_and_fade(&data, 0).is_empty());
        }
        assert_eq!(stock.count(id), 2);
        assert_eq!(stock.age_and_fade(&data, 0), vec![(id.to_string(), 2)]);
        assert_eq!(stock.count(id), 0);
        assert_eq!(stock.count("not_a_product"), 1);

        let mut kept = Stock::default();
        kept.add(id, 1);
        for _ in 0..nights {
            assert!(kept.age_and_fade(&data, 1).is_empty());
        }
        assert_eq!(kept.age_and_fade(&data, 1), vec![(id.to_string(), 1)]);
    }
}
