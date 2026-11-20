//! The apprentice (`apprentice` effect in `upgrades.ron`): a small helper
//! ghost behind the counter. On a busy night it serves a few customers who
//! are waiting in line – not the one at the register – once they have
//! waited a little: only those who bring a ware from the shelves and need
//! nothing else (no story, no order). It takes the exact price, so no change
//! is due, and the ghost passes into the light.
//!
//! Picture (optional): `assets/sprites/ghosts/apprentice.png`, otherwise a
//! pale placeholder. Its bobbing is done here.

use std::collections::HashMap;

use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use crate::data::GameData;
use crate::day_cycle::{NightReport, Phase, Progress};
use crate::economy::{Drawer, Purse};
use crate::fx::Cue;
use crate::ghosts::{Ghost, GhostState, Queue};
use crate::i18n::Lang;
use crate::inventory::Stock;
use crate::scene::{Billboard, billboard_material, billboard_mesh, floor};
use crate::shop::ShopLayout;
use crate::ui::GameLog;
use crate::upgrades::Upgrades;

/// Seconds a ghost waits in line before the apprentice serves it.
const SERVE_AFTER_SECS: f32 = 6.0;
/// Where the apprentice floats, relative to the counter's centre (x, z),
/// its size and how high it hovers.
const OFFSET: Vec2 = Vec2::new(0.55, -1.3);
const SIZE: Vec2 = Vec2::new(0.7, 1.05);
const HOVER: f32 = 0.35;
const IMAGE: &str = "sprites/ghosts/apprentice.png";

pub struct ApprenticePlugin;

impl Plugin for ApprenticePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Tonight>()
            .add_systems(OnEnter(Phase::Night), |mut t: ResMut<Tonight>| {
                *t = Tonight::default()
            })
            .add_systems(
                Update,
                serve
                    .run_if(in_state(Phase::Night))
                    .run_if(crate::story_film::outside_film),
            )
            .add_systems(
                Update,
                (show_apprentice, bob).run_if(not(in_state(Phase::Menu))),
            );
    }
}

/// Tonight: how many it has served, and how long each ghost in line waited.
#[derive(Resource, Default)]
struct Tonight {
    served: u32,
    waited: HashMap<Entity, f32>,
}

/// The helper ghost behind the counter.
#[derive(Component)]
struct Apprentice;

impl Upgrades {
    /// Customers the apprentice serves a night (0 without one).
    pub fn apprentice(&self, data: &GameData) -> u32 {
        data.upgrades
            .iter()
            .filter(|u| self.has(&u.id))
            .map(|u| u.effect.apprentice)
            .sum()
    }
}

/// May the apprentice serve this ghost? It brings a ware from the shelves
/// and wants nothing else.
fn simple_customer(g: &Ghost) -> bool {
    g.state == GhostState::Waiting
        && g.carrying.is_some()
        && g.chapter.is_none()
        && g.order.is_none()
}

fn serve(
    time: Res<Time>,
    data: Res<GameData>,
    upgrades: Res<Upgrades>,
    lang: Res<Lang>,
    mut tonight: ResMut<Tonight>,
    mut queue: ResMut<Queue>,
    mut ghosts: Query<&mut Ghost>,
    mut drawer: ResMut<Drawer>,
    mut stock: ResMut<Stock>,
    mut progress: ResMut<Progress>,
    mut report: ResMut<NightReport>,
    mut log: ResMut<GameLog>,
    mut cues: MessageWriter<Cue>,
) {
    let limit = upgrades.apprentice(&data);
    if limit == 0 || tonight.served >= limit {
        return;
    }
    let dt = time.delta_secs();
    let line: Vec<Entity> = queue.0.iter().skip(1).copied().collect();
    tonight.waited.retain(|e, _| line.contains(e));
    for e in line {
        let Ok(mut g) = ghosts.get_mut(e) else {
            continue;
        };
        if !simple_customer(&g) {
            continue;
        }
        let waited = tonight.waited.entry(e).or_default();
        *waited += dt;
        if *waited < SERVE_AFTER_SECS {
            continue;
        }
        let Some(ware) = g.carrying.clone() else {
            continue;
        };
        if !stock.take_one(&ware) {
            continue;
        }
        let def = &data.ghosts[g.def];
        let base = data.product(&ware).map_or(1, |p| p.price);
        let factor = def.pay_multiplier * data.price_factor(progress.calendar(&data));
        let price = ((base as f32 * factor).round() as u32).max(1);
        drawer.merge(&Purse::greedy(price));
        report.record_sale(&ware, price, price, 0);
        report.redeemed.push(def.id.clone());
        report.apprentice_served += 1;
        progress.journal.served(&def.id, &ware, g.regular);
        cues.write(Cue::Paid);
        cues.write(Cue::Ascend(g.pos));
        g.state = GhostState::Ascending;
        g.timer = 0.0;
        log.push(lang.f(
            "log.apprentice_served",
            &[
                ("who", &lang.ghost_the(&def.id)),
                ("item", &lang.product(&ware)),
                ("price", &price),
            ],
        ));
        queue.remove(e);
        tonight.waited.remove(&e);
        tonight.served += 1;
        if tonight.served >= limit {
            break;
        }
    }
}

/// The apprentice floats behind the counter once it is hired.
fn show_apprentice(
    mut commands: Commands,
    data: Res<GameData>,
    upgrades: Res<Upgrades>,
    layout: Res<ShopLayout>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    shown: Query<Entity, With<Apprentice>>,
) {
    let hired = upgrades.apprentice(&data) > 0;
    match (hired, shown.iter().next()) {
        (true, None) => {
            commands.spawn((
                Apprentice,
                Mesh3d(billboard_mesh(&mut meshes, SIZE)),
                MeshMaterial3d(billboard_material(
                    &mut materials,
                    &asset_server,
                    IMAGE,
                    Color::srgba(0.75, 0.95, 0.85, 0.7),
                )),
                Transform::from_translation(floor(layout.counter_pos() + OFFSET, HOVER)),
                Billboard,
                NotShadowCaster,
                Pickable::IGNORE,
            ));
        }
        (false, Some(e)) => commands.entity(e).despawn(),
        _ => {}
    }
}

fn bob(
    time: Res<Time>,
    layout: Res<ShopLayout>,
    mut apprentice: Query<&mut Transform, With<Apprentice>>,
) {
    let t = time.elapsed_secs();
    for mut transform in &mut apprentice {
        transform.translation = floor(
            layout.counter_pos() + OFFSET,
            HOVER + 0.06 * (t * 1.7).sin(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_apprentice_is_hired_at_the_market() {
        let data = GameData::load();
        let mut upgrades = Upgrades::default();
        assert_eq!(upgrades.apprentice(&data), 0);
        upgrades.0.insert("apprentice".into());
        assert!(upgrades.apprentice(&data) > 0);
    }
}
