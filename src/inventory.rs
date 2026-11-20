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

    /// Ages everything by one night and removes faded items.
    /// Returns how much of what has faded.
    /// `bonus`: nights the wares last longer (herb sachets in the storeroom).
    pub fn age_and_fade(&mut self, data: &GameData, bonus: u32) -> Vec<(String, u32)> {
        let mut faded = Vec::new();
        for (id, units) in &mut self.units {
            let limit = data
                .product(id)
                .map_or(u32::MAX, |p| p.fade_after.saturating_add(bonus));
            for age in units.iter_mut() {
                *age += 1;
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
}
