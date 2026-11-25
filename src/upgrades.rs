//! Shop improvements (`assets/data/upgrades.ron`), bought once at the
//! village market: more floor cleared of old junk, a waiting bench, a lantern
//! at the door, herb sachets in the storeroom, a deal with the ferryman.
//!
//! What they change is read through [`Upgrades`]; what they look like is
//! built here: junk on the floor that is not cleared yet, and a model for
//! each improvement that has one (`assets/models/shop/<id>.glb`, otherwise a
//! placeholder box).

pub(crate) mod storage;

use std::collections::BTreeSet;

use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use crate::data::{GameData, UpgradeDef, rgb};
use crate::shop::{ShopLayout, model_or_box};

pub struct UpgradesPlugin;

impl Plugin for UpgradesPlugin {
    fn build(&self, app: &mut App) {
        storage::install(app);
        app.init_resource::<Upgrades>().add_systems(
            Update,
            show_upgrades
                .run_if(resource_changed::<Upgrades>.or_else(resource_added::<ShopLayout>)),
        );
    }
}

/// The improvements bought so far.
#[derive(Resource, Default, Clone, Debug, PartialEq, Eq)]
pub struct Upgrades(pub BTreeSet<String>);

impl Upgrades {
    pub fn has(&self, id: &str) -> bool {
        self.0.contains(id)
    }

    fn owned<'a>(&'a self, data: &'a GameData) -> impl Iterator<Item = &'a UpgradeDef> {
        data.upgrades.iter().filter(|u| self.has(&u.id))
    }

    /// Offered at the market tonight: unlocked, not bought yet, and what it
    /// builds on is already there.
    pub fn offered(&self, def: &UpgradeDef, night: u32) -> bool {
        def.unlock_night <= night
            && !self.has(&def.id)
            && def.requires.as_ref().is_none_or(|r| self.has(r))
    }

    /// Columns (from the door) and rows (from the front) free of junk.
    pub fn area(&self, data: &GameData) -> (usize, usize) {
        let c = &data.config;
        let (cols, rows) = self
            .owned(data)
            .fold((c.start_area.cols, c.start_area.rows), |(x, y), u| {
                (x + u.effect.cols, y + u.effect.rows)
            });
        (cols.min(c.grid_cols), rows.min(c.grid_rows))
    }

    /// Whether a cell of the layout can be furnished.
    pub fn usable(&self, data: &GameData, layout: &ShopLayout, idx: usize) -> bool {
        let (cols, rows) = self.area(data);
        idx % layout.cols < cols && idx / layout.cols < rows
    }

    /// Crates the ferryman brings tonight, with the deal made at the market.
    pub fn crates_tonight(&self, data: &GameData, night: u32) -> u32 {
        let extra: i32 = self.owned(data).map(|u| u.effect.extra_crates).sum();
        (data.crates_tonight(night) as i32 + extra).max(1) as u32
    }

    /// Nights longer the wares last.
    pub fn fade_bonus(&self, data: &GameData) -> u32 {
        self.owned(data).map(|u| u.effect.fade_bonus).sum()
    }

    /// Factor on the time between ghosts.
    pub fn spawn_factor(&self, data: &GameData) -> f32 {
        self.owned(data).map(|u| u.effect.spawn_factor).product()
    }

    /// Factor on how long ghosts wait at the counter.
    pub fn patience_factor(&self, data: &GameData) -> f32 {
        self.owned(data).map(|u| u.effect.patience_factor).product()
    }
}

/// Something built here; rebuilt whenever the improvements change.
#[derive(Component)]
struct UpgradeVisual;

/// Brightness of an improvement's lamp (a lantern at the door).
const LAMP_INTENSITY: f32 = 18_000.0;
const LAMP_RANGE: f32 = 4.0;

fn show_upgrades(
    mut commands: Commands,
    data: Res<GameData>,
    upgrades: Res<Upgrades>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    old: Query<Entity, With<UpgradeVisual>>,
) {
    for e in &old {
        commands.entity(e).despawn();
    }
    for def in upgrades.owned(&data) {
        let Some(m) = def.model else { continue };
        let path = format!("models/shop/{}.glb", def.id);
        let size = Vec3::new(m.size.0, m.size.1, m.size.2);
        let visual = model_or_box(
            &mut commands,
            &asset_server,
            &mut meshes,
            &mut materials,
            &path,
            size,
            rgb(m.color),
        );
        commands.entity(visual).insert((
            UpgradeVisual,
            Pickable::IGNORE,
            NotShadowCaster,
            Transform::from_xyz(m.pos.0, m.pos.1, m.pos.2)
                .with_rotation(Quat::from_rotation_y(m.turn.to_radians())),
        ));
        if m.light_height > 0.0 {
            commands.spawn((
                PointLight {
                    color: Color::srgb(1.0, 0.75, 0.45),
                    intensity: LAMP_INTENSITY,
                    range: LAMP_RANGE,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_xyz(0.0, m.light_height, 0.0),
                ChildOf(visual),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn improvements_clear_the_floor_and_add_up() {
        let data = GameData::load();
        let c = &data.config;
        let mut upgrades = Upgrades::default();
        assert_eq!(upgrades.area(&data), (c.start_area.cols, c.start_area.rows));
        // Everything bought: the whole grid is free.
        for u in &data.upgrades {
            upgrades.0.insert(u.id.clone());
        }
        assert_eq!(upgrades.area(&data), (c.grid_cols, c.grid_rows));
        assert!(upgrades.crates_tonight(&data, 1) >= data.crates_tonight(1));
        assert!(upgrades.spawn_factor(&data) <= 1.0);
        assert!(upgrades.patience_factor(&data) >= 1.0);
    }

    #[test]
    fn the_starting_furniture_stands_on_free_floor() {
        let data = GameData::load();
        let layout = ShopLayout::new(data.config.grid_cols, data.config.grid_rows);
        let upgrades = Upgrades::default();
        for (idx, cell) in layout.cells.iter().enumerate() {
            if cell.decor.is_some() {
                assert!(
                    upgrades.usable(&data, &layout, idx),
                    "cell {idx} is under junk"
                );
            }
        }
    }

    #[test]
    fn an_improvement_is_offered_only_after_what_it_needs() {
        let data = GameData::load();
        let mut upgrades = Upgrades::default();
        for u in data.upgrades.iter().filter(|u| u.requires.is_some()) {
            assert!(!upgrades.offered(u, 99));
            upgrades.0.insert(u.requires.clone().unwrap());
            assert!(upgrades.offered(u, 99));
        }
    }
}
