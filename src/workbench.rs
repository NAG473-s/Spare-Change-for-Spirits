//! The workbench (`assets/data/workbench.ron`): once an evening the player
//! rummages through Hedda's old junk and finds something broken; at the
//! workbench one broken thing an evening is repaired for a few coins and
//! becomes an ordinary ware in the storeroom – no crate from the ferryman.

use bevy::prelude::*;
use serde::Deserialize;

use crate::data::GameData;
use crate::day_cycle::{Phase, Progress};
use crate::economy::Drawer;
use crate::i18n::Lang;
use crate::inventory::Stock;
use crate::rng::Rng;
use crate::shop::ShopLayout;
use crate::ui::{Action, GameLog};
use crate::upgrades::Upgrades;

/// The decor id of the workbench (decor.ron).
pub const WORKBENCH: &str = "workbench";

pub struct WorkbenchPlugin;

impl Plugin for WorkbenchPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, use_workbench.run_if(in_state(Phase::Evening)));
    }
}

/// `workbench.ron`.
#[derive(Deserialize, Clone, Debug)]
pub struct WorkbenchDef {
    /// What turns up in the junk, how often, and what repairing it costs.
    pub junk: Vec<JunkDef>,
    /// Once all junk is cleared: the chance an evening that the ferryman has
    /// left something broken by the door.
    pub after_cleared_chance: f32,
}

#[derive(Deserialize, Clone, Debug)]
pub struct JunkDef {
    /// The product it becomes once repaired.
    pub product: String,
    /// Repair cost in button units.
    pub cost: u32,
    #[serde(default = "one_f")]
    pub weight: f32,
}

fn one_f() -> f32 {
    1.0
}

impl WorkbenchDef {
    pub fn cost(&self, product: &str) -> Option<u32> {
        self.junk
            .iter()
            .find(|j| j.product == product)
            .map(|j| j.cost)
    }
}

/// Is a workbench standing in the shop?
pub fn has_workbench(layout: &ShopLayout) -> bool {
    layout
        .cells
        .iter()
        .any(|c| c.decor.as_deref() == Some(WORKBENCH))
}

#[allow(clippy::too_many_arguments)]
fn use_workbench(
    mut actions: MessageReader<Action>,
    data: Res<GameData>,
    layout: Res<ShopLayout>,
    upgrades: Res<Upgrades>,
    lang: Res<Lang>,
    mut progress: ResMut<Progress>,
    mut drawer: ResMut<Drawer>,
    mut stock: ResMut<Stock>,
    mut rng: ResMut<Rng>,
    mut log: ResMut<GameLog>,
) {
    for action in actions.read() {
        if !has_workbench(&layout) {
            continue;
        }
        let night = progress.night;
        match action {
            Action::Rummage if progress.rummaged_on != night => {
                progress.rummaged_on = night;
                let junk_left =
                    upgrades.area(&data) != (data.config.grid_cols, data.config.grid_rows);
                if !junk_left && !rng.chance(data.workbench.after_cleared_chance) {
                    log.push(lang.t("log.found_nothing"));
                    continue;
                }
                let calendar = progress.calendar(&data);
                let found: Vec<&JunkDef> = data
                    .workbench
                    .junk
                    .iter()
                    .filter(|j| {
                        data.product(&j.product)
                            .is_some_and(|p| p.unlock_night <= calendar)
                    })
                    .collect();
                let weights: Vec<f32> = found.iter().map(|j| j.weight).collect();
                let Some(i) = rng.weighted(&weights) else {
                    continue;
                };
                let product = found[i].product.clone();
                log.push(lang.f(
                    "log.found_broken",
                    &[(
                        "item",
                        &lang.f(
                            "workbench.broken_item",
                            &[("item", &lang.product(&product))],
                        ),
                    )],
                ));
                progress.broken.push(product);
            }
            Action::Repair(product) if progress.repaired_on != night => {
                let Some(cost) = data.workbench.cost(product) else {
                    continue;
                };
                let Some(at) = progress.broken.iter().position(|b| b == product) else {
                    continue;
                };
                if !drawer.pay(cost) {
                    continue;
                }
                progress.broken.remove(at);
                progress.repaired_on = night;
                stock.add(product, 1);
                log.push(lang.f("log.repaired", &[("item", &lang.product(product))]));
            }
            _ => {}
        }
    }
}
