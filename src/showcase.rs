//! The shop window: in the evening the player puts one ware from stock in
//! the window. While there is one left in stock, ghosts who wish for it come
//! more often (`showcase_attract` in `config.ron`, see `sets::odds_tonight`).
//! The ware stands on the window sill – its model or picture, the same as on
//! the shelves – and stays there until something else is chosen.

use bevy::prelude::*;

use crate::data::GameData;
use crate::day_cycle::{Phase, Progress};
use crate::inventory::Stock;
use crate::scene::ROOM_BACK_Z;
use crate::shop::product_model;
use crate::ui::Action;

/// Where the ware stands: on the sill of the shop window, in front of the
/// open/closed sign's corner, and how large it is shown.
const SPOT: Vec3 = Vec3::new(-2.62, 1.06, ROOM_BACK_Z + 0.38);
const SCALE: f32 = 1.8;

pub struct ShowcasePlugin;

impl Plugin for ShowcasePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                choose.run_if(in_state(Phase::Evening)),
                show.run_if(resource_changed::<Progress>.or_else(resource_changed::<Stock>)),
            )
                .chain(),
        );
    }
}

/// The ware on the sill.
#[derive(Component)]
struct InWindow(String);

fn choose(mut actions: MessageReader<Action>, stock: Res<Stock>, mut progress: ResMut<Progress>) {
    for action in actions.read() {
        if let Action::Showcase(ware) = action {
            let ware = ware.clone().filter(|w| stock.count(w) > 0);
            if progress.journal.showcase != ware {
                progress.journal.showcase = ware;
            }
        }
    }
}

/// Shows the chosen ware while one is in stock.
fn show(
    mut commands: Commands,
    data: Res<GameData>,
    progress: Res<Progress>,
    stock: Res<Stock>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    shown: Query<(Entity, &InWindow)>,
) {
    let wanted = crate::sets::showcased(&progress, &stock)
        .filter(|w| data.product(w).is_some())
        .map(str::to_string);
    let now = shown.iter().next().map(|(_, w)| w.0.clone());
    if wanted == now {
        return;
    }
    for (e, _) in &shown {
        commands.entity(e).despawn();
    }
    if let Some(ware) = wanted {
        let e = product_model(
            &mut commands,
            &asset_server,
            &mut meshes,
            &mut materials,
            &ware,
            SPOT,
        );
        commands.entity(e).insert((
            InWindow(ware),
            Transform::from_translation(SPOT).with_scale(Vec3::splat(SCALE)),
            Visibility::default(),
        ));
    }
}
