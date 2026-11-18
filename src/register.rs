//! Counter and register: guess the wish, take payment, hand out change.

use bevy::prelude::*;

use crate::data::GameData;
use crate::day_cycle::{NightReport, Phase, Progress};
use crate::economy::{Drawer, Purse, ghost_payment};
use crate::ghosts::{Ghost, GhostState, Queue};
use crate::inventory::Stock;
use crate::rng::Rng;
use crate::ui::{Action, GameLog};

pub struct RegisterPlugin;

impl Plugin for RegisterPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Register>()
            .add_systems(OnExit(Phase::Night), |mut r: ResMut<Register>| {
                r.sale = None
            })
            .add_systems(
                Update,
                (serve_front_ghost, make_change).run_if(in_state(Phase::Night)),
            );
    }
}

/// A sale in progress at the register.
#[derive(Clone, Debug)]
pub struct Sale {
    pub ghost: Entity,
    pub product: String,
    pub price: u32,
    /// What the ghost put on the counter (already in the cash drawer).
    pub paid: Purse,
    /// What the player has counted out as change so far.
    pub given: Purse,
}

impl Sale {
    pub fn change_due(&self) -> u32 {
        self.paid.total().saturating_sub(self.price)
    }
}

#[derive(Resource, Default)]
pub struct Register {
    pub sale: Option<Sale>,
}

/// The player hands an item to the ghost at the front – or declines.
fn serve_front_ghost(
    mut actions: MessageReader<Action>,
    data: Res<GameData>,
    stock: Res<Stock>,
    mut register: ResMut<Register>,
    mut queue: ResMut<Queue>,
    mut drawer: ResMut<Drawer>,
    mut progress: ResMut<Progress>,
    mut report: ResMut<NightReport>,
    mut rng: ResMut<Rng>,
    mut log: ResMut<GameLog>,
    mut ghosts: Query<&mut Ghost>,
) {
    for action in actions.read() {
        if register.sale.is_some() {
            continue;
        }
        let Some(front) = queue.front() else { continue };
        let Ok(mut ghost) = ghosts.get_mut(front) else {
            continue;
        };
        if ghost.state != GhostState::Waiting {
            continue;
        }
        let def = &data.ghosts[ghost.def];

        match action {
            Action::Offer(product) => {
                if stock.count(product) == 0 {
                    continue;
                }
                if *product != ghost.wish {
                    log.push(format!(
                        "\"That … is not it.\" The {} drifts away, disappointed.",
                        def.name
                    ));
                    ghost.state = GhostState::Leaving;
                    queue.remove(front);
                    progress.reputation -= 1;
                    report.unhappy += 1;
                    continue;
                }
                let base = data.product(product).map_or(1, |p| p.price);
                let price = ((base as f32 * def.pay_multiplier).round() as u32).max(1);
                let paid = ghost_payment(price, &def.pays_with, &mut rng);
                // Payment goes straight into the drawer – change is taken from there too.
                drawer.merge(&paid);
                ghost.state = GhostState::AtRegister;
                log.push(format!(
                    "The {} places {} on the counter.",
                    def.name,
                    paid.describe()
                ));
                register.sale = Some(Sale {
                    ghost: front,
                    product: product.clone(),
                    price,
                    paid,
                    given: Purse::default(),
                });
            }
            Action::Decline => {
                log.push(format!(
                    "You apologise. The {} nods silently and leaves.",
                    def.name
                ));
                ghost.state = GhostState::Leaving;
                queue.remove(front);
            }
            _ => {}
        }
    }
}

/// Count out and hand over change.
fn make_change(
    mut actions: MessageReader<Action>,
    data: Res<GameData>,
    mut register: ResMut<Register>,
    mut drawer: ResMut<Drawer>,
    mut stock: ResMut<Stock>,
    mut queue: ResMut<Queue>,
    mut progress: ResMut<Progress>,
    mut report: ResMut<NightReport>,
    mut log: ResMut<GameLog>,
    mut ghosts: Query<&mut Ghost>,
) {
    for action in actions.read() {
        let Some(sale) = register.sale.as_mut() else {
            continue;
        };
        match action {
            Action::AddChange(coin) => {
                if sale.given.get(*coin) < drawer.get(*coin) {
                    sale.given.add(*coin, 1);
                }
            }
            Action::ResetChange => sale.given = Purse::default(),
            Action::GiveChange => {
                let sale = register.sale.take().expect("checked above");
                drawer.remove(&sale.given);
                stock.take_one(&sale.product);
                queue.remove(sale.ghost);
                report.sales += 1;
                report.revenue += sale.price;

                let due = sale.change_due();
                let given = sale.given.total();
                let Ok(mut ghost) = ghosts.get_mut(sale.ghost) else {
                    continue;
                };
                let name = data.ghosts[ghost.def].name.clone();

                if given == due {
                    report.change_correct += 1;
                    progress.reputation += 1;
                    ghost.state = GhostState::Ascending;
                    ghost.timer = 0.0;
                    report.redeemed.push(name.clone());
                    log.push(format!(
                        "Exactly right! The {name} clutches the {} and dissolves into warm light.",
                        data.product_name(&sale.product)
                    ));
                } else if given < due {
                    report.change_wrong += 1;
                    progress.reputation -= 2;
                    ghost.state = GhostState::Leaving;
                    log.push(format!(
                        "Too little change ({given} instead of {due}). The {name} feels cheated."
                    ));
                } else {
                    report.change_wrong += 1;
                    ghost.state = GhostState::Leaving;
                    log.push(format!(
                        "Too much change ({given} instead of {due}). The {name} giggles softly."
                    ));
                }
            }
            _ => {}
        }
    }
}
