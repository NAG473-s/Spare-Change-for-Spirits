//! The curtain rail and tiebacks stay fixed. Only the loose hems catch a draught.
use super::{ROOM_BACK_Z, village::Breeze};
use crate::data::asset_exists;
use crate::day_cycle::{NightClock, Phase};
use crate::harbor::Location;
use crate::save::Settings;
use crate::shop::{entrance::DoorSwing, model_or_box};
use crate::weather::{Weather, WeatherNow};
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;

const RAIL: Vec3 = Vec3::new(-3.2, 2.83, ROOM_BACK_Z + 0.17);
const TIE_X: f32 = 0.973;
const TIE_Y: f32 = -0.945;
const POOL: Vec3 = Vec3::new(-3.35, 0.013, 0.34);
const HANGING: &str = "models/shop/window_drapery.glb";
const HEM: &str = "models/shop/curtain_hem.glb";
const LIGHT: &str = "sprites/shop/window_light.png";

#[derive(Component)]
struct Hem(f32);
#[derive(Component)]
struct WindowPool;

pub(super) fn install(app: &mut App) {
    app.add_systems(Startup, spawn).add_systems(
        Update,
        dress
            .after(crate::shop::entrance::animate)
            .run_if(resource_equals(Location::Shop))
            .run_if(crate::menu::shop_unpaused),
    );
}

fn spawn(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let rail = commands
        .spawn((Transform::from_translation(RAIL), Visibility::default()))
        .id();
    if asset_exists(HANGING) {
        commands.spawn((
            WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(HANGING))),
            Transform::default(),
            Pickable::IGNORE,
            ChildOf(rail),
        ));
    } else {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(2.46, 0.06, 0.05))),
            MeshMaterial3d(materials.add(Color::srgb(0.55, 0.42, 0.23))),
            Transform::default(),
            Pickable::IGNORE,
            ChildOf(rail),
        ));
        let cloth = meshes.add(Cuboid::new(0.24, 0.95, 0.08));
        let teal = materials.add(Color::srgb(0.30, 0.46, 0.47));
        for side in [-1., 1.] {
            commands.spawn((
                Mesh3d(cloth.clone()),
                MeshMaterial3d(teal.clone()),
                Transform::from_xyz(side * TIE_X, TIE_Y * 0.5, 0.07),
                Pickable::IGNORE,
                ChildOf(rail),
            ));
        }
    }
    for side in [-1., 1.] {
        let pivot = commands
            .spawn((
                Hem(side),
                Transform::from_xyz(side * TIE_X, TIE_Y, 0.08),
                Visibility::default(),
                ChildOf(rail),
            ))
            .id();
        let cloth = model_or_box(
            &mut commands,
            &assets,
            &mut meshes,
            &mut materials,
            HEM,
            Vec3::new(0.38, 0.84, 0.10),
            Color::srgb(0.30, 0.46, 0.47),
        );
        commands
            .entity(cloth)
            .insert((Pickable::IGNORE, ChildOf(pivot)));
        if !asset_exists(HEM) {
            commands
                .entity(cloth)
                .insert(Transform::from_xyz(0., -0.42, 0.));
        }
    }
    let texture = if asset_exists(LIGHT) {
        Some(assets.load(LIGHT))
    } else {
        None
    };
    let light = materials.add(StandardMaterial {
        base_color: Color::NONE,
        base_color_texture: texture,
        alpha_mode: AlphaMode::Add,
        unlit: true,
        ..default()
    });
    commands.spawn((
        WindowPool,
        Mesh3d(meshes.add(Plane3d::default().mesh().size(1.7, 2.88))),
        MeshMaterial3d(light),
        Transform::from_translation(POOL),
        Pickable::IGNORE,
        NotShadowCaster,
        NotShadowReceiver,
    ));
}

fn hem_rotation(side: f32, aperture: f32, wind: &Breeze, storm: bool, still: bool) -> Quat {
    let turn = if still {
        0.0
    } else {
        let gust = wind.gust(side * 0.65);
        let outside = if storm { 0.205 } else { 0.035 };
        outside * gust + aperture * (0.24 + 0.22 * gust)
    };
    // A hanging hem moves out towards the room. The knot remains on its peg.
    Quat::from_rotation_z(side * (0.065 + 0.18 * turn)) * Quat::from_rotation_x(-turn)
}

fn dress(
    phase: Res<State<Phase>>,
    clock: Res<NightClock>,
    settings: Res<Settings>,
    wind: Res<Breeze>,
    weather: Res<WeatherNow>,
    doors: Query<&DoorSwing>,
    mut hems: Query<(&Hem, &mut Transform)>,
    mut light: Single<
        (&mut Transform, &MeshMaterial3d<StandardMaterial>),
        (With<WindowPool>, Without<Hem>),
    >,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let phase = *phase.get();
    if phase == Phase::Menu {
        return;
    }
    let aperture = doors.single().map_or(0., DoorSwing::aperture);
    for (hem, mut at) in &mut hems {
        let rotation = hem_rotation(
            hem.0,
            aperture,
            &wind,
            weather.stormy(),
            settings.reduced_motion,
        );
        if at.rotation != rotation {
            at.rotation = rotation;
        }
    }
    let (tint, strength) = match phase {
        Phase::Night => (Color::srgb(0.64, 0.77, 1.0), 0.33),
        Phase::Morning | Phase::DemoEnd => (Color::srgb(1.0, 0.84, 0.61), 0.39),
        Phase::Evening => (Color::srgb(0.72, 0.81, 1.0), 0.24),
        _ => (Color::WHITE, 0.0),
    };
    let moon = if phase == Phase::Night && !settings.reduced_motion {
        (clock.elapsed / clock.length.max(1.)).clamp(0., 1.) - 0.5
    } else {
        0.
    };
    let (at, handle) = &mut *light;
    at.set_if_neq(
        Transform::from_translation(POOL + Vec3::new(0.66 * moon, 0., 0.18 * moon))
            .with_rotation(Quat::from_rotation_y(-0.31 + 0.12 * moon)),
    );
    let veil = if weather.stormy() || weather.is(Weather::Fog) {
        0.52
    } else if weather.0.is_some() {
        0.77
    } else {
        1.
    };
    if let Some(mut mat) = materials.get_mut(*handle) {
        let alpha = if mat.base_color_texture.is_some() {
            strength * veil
        } else {
            strength * veil * 0.22
        };
        let color = tint.with_alpha(alpha);
        if mat.base_color != color {
            mat.base_color = color;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_open_door_lifts_the_hem_away_from_the_glass_and_still_mode_keeps_the_tie_pose() {
        let breeze = Breeze::default();
        let pose = |open, still| hem_rotation(1., open, &breeze, false, still) * Vec3::NEG_Y;
        assert!(pose(1., false).z > pose(0., false).z + 0.2);
        assert_eq!(pose(1., true), pose(0., true));
    }
}
