//! Stock and orders from the ferryman.

use std::collections::BTreeMap;

use bevy::prelude::*;

use crate::data::GameData;
use crate::day_cycle::Phase;
use crate::economy::Drawer;
use crate::ui::{Action, GameLog};

pub struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Stock>()
            .init_resource::<CratesOrdered>()
            .add_systems(OnEnter(Phase::Evening), |mut c: ResMut<CratesOrdered>| c.0 = 0)
            .add_systems(Update, handle_orders.run_if(in_state(Phase::Evening)));
    }
}

/// Per product, a list with the age (in nights) of every unit.
#[derive(Resource, Default)]
pub struct Stock {
    units: BTreeMap<String, Vec<u32>>,
}

impl Stock {
    pub fn count(&self, id: &str) -> u32 {
        self.units.get(id).map_or(0, |v| v.len() as u32)
    }

    pub fn add(&mut self, id: &str, n: u32) {
        self.units
            .entry(id.to_string())
            .or_default()
            .extend(std::iter::repeat_n(0, n as usize));
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
    pub fn age_and_fade(&mut self, data: &GameData) -> Vec<(String, u32)> {
        let mut faded = Vec::new();
        for (id, units) in &mut self.units {
            let limit = data.product(id).map_or(u32::MAX, |p| p.fade_after);
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

fn handle_orders(
    mut actions: MessageReader<Action>,
    data: Res<GameData>,
    mut stock: ResMut<Stock>,
    mut drawer: ResMut<Drawer>,
    mut crates: ResMut<CratesOrdered>,
    mut log: ResMut<GameLog>,
) {
    for action in actions.read() {
        let Action::Order(id) = action else { continue };
        let Some(product) = data.product(id) else { continue };
        if crates.0 >= data.config.crates_per_night {
            log.push("The ferryman shakes his head: his boat is full.");
            continue;
        }
        if !drawer.pay(product.crate_cost) {
            log.push(format!(
                "Not enough in the drawer for {} ({}).",
                product.name, product.crate_cost
            ));
            continue;
        }
        crates.0 += 1;
        stock.add(id, product.crate_size);
        log.push(format!(
            "The ferryman brings {}× {}.",
            product.crate_size, product.name
        ));
    }
}
