//! The same river bend and mill seen from the market road and from the quay.
use crate::data::asset_exists;
use crate::day_cycle::Phase;
use crate::harbor::{HARBOR, Location};
use crate::market::MARKET;
use crate::save::Settings;
use crate::weather::{Weather, WeatherNow};
use bevy::asset::RenderAssetUsages;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

const DAY: &str = "sprites/harbor/river_valley_day.png";
const DUSK: &str = "sprites/harbor/river_valley_dusk.png";
const BANK: &str = "models/harbor/riverbank.glb";
const MILL: &str = "models/harbor/bank_windmill.glb";
const SAILS: &str = "models/harbor/windmill_sails.glb";
const AXLE: Vec3 = Vec3::new(0.0, 2.08, 0.92);
const START_ANGLE: f32 = 0.54;

#[derive(Resource)]
pub(crate) struct Breeze {
    elapsed: f32,
    angle: f32,
}

impl Default for Breeze {
    fn default() -> Self {
        Self {
            elapsed: 0.,
            angle: START_ANGLE,
        }
    }
}

impl Breeze {
    /// A gust reaches the pennants in order along the line.
    pub(crate) fn gust(&self, along: f32) -> f32 {
        0.55 + 0.28 * (self.elapsed * 0.91 - along * 0.39).sin()
            + 0.17 * (self.elapsed * 1.73 - along * 0.57).sin()
    }

    pub(crate) fn flutter(&self, phase: f32) -> f32 {
        (self.elapsed * 2.6 + phase).sin()
    }

    fn advance(&mut self, dt: f32, storm: bool) {
        self.elapsed += dt;
        let rate = if storm { 0.84 } else { 0.39 };
        self.angle = (self.angle - dt * rate * (0.68 + 0.6 * self.gust(0.)))
            .rem_euclid(std::f32::consts::TAU);
    }
}

#[derive(Component)]
struct Sails;

#[derive(Component)]
struct Valley(Location);

pub(super) fn install(app: &mut App) {
    app.init_resource::<Breeze>()
        .add_systems(Startup, spawn)
        .add_systems(PreUpdate, turn.run_if(crate::menu::shop_unpaused))
        .add_systems(
            Update,
            grade.run_if(
                resource_changed::<WeatherNow>
                    .or_else(state_changed::<Phase>)
                    .or_else(resource_changed::<Location>),
            ),
        );
}

fn turn(
    time: Res<Time>,
    settings: Res<Settings>,
    weather: Res<WeatherNow>,
    phase: Res<State<Phase>>,
    mut breeze: ResMut<Breeze>,
    mut sails: Query<&mut Transform, With<Sails>>,
) {
    if *phase.get() == Phase::Menu || time.delta_secs() == 0. || settings.reduced_motion {
        return;
    }
    breeze.advance(time.delta_secs(), weather.stormy());
    for mut at in &mut sails {
        at.rotation = Quat::from_rotation_z(breeze.angle);
    }
}

/// Extend the upper sky above the header and reflect the side margins on ultrawide screens.
fn panorama() -> Mesh {
    let mut vertices = Vec::new();
    let mut uv = Vec::new();
    let mut colors = Vec::new();
    let mut indices = Vec::new();
    for (row, (y, v, shade)) in [(-1.05, 1., 0.64), (3.2, 0., 1.), (13., 0., 1.)]
        .into_iter()
        .enumerate()
    {
        for (col, (x, u)) in [(-34., 0.5), (-17., 0.), (17., 1.), (34., 0.5)]
            .into_iter()
            .enumerate()
        {
            vertices.push([x, y, 0.]);
            uv.push([u, v]);
            colors.push([shade, shade, shade, 1.]);
            if row < 2 && col < 3 {
                let a = (row * 4 + col) as u32;
                indices.extend_from_slice(&[a, a + 1, a + 4, a + 1, a + 5, a + 4]);
            }
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vertices)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0., 0., 1.]; 12])
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(indices))
}

fn model(
    commands: &mut Commands,
    assets: &AssetServer,
    parent: Entity,
    path: &'static str,
) -> bool {
    if !asset_exists(path) {
        return false;
    }
    commands.spawn((
        WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(path))),
        Transform::default(),
        Pickable::IGNORE,
        ChildOf(parent),
    ));
    true
}

fn spawn(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let painting = meshes.add(panorama());
    // The mill sits beyond the rows of houses. From the quay we see its other side.
    for (site, origin, edge, mill, scale, turn, texture, fallback) in [
        (
            Location::Market,
            MARKET,
            -11.0,
            Vec3::new(-8.7, 0.06, -10.08),
            0.55,
            -0.22,
            DAY,
            Color::srgb(0.63, 0.69, 0.73),
        ),
        (
            Location::Harbor,
            HARBOR,
            -9.0,
            Vec3::new(6.12, 0.08, -8.32),
            0.68,
            0.25,
            DUSK,
            Color::srgb(0.17, 0.22, 0.32),
        ),
    ] {
        let drawn = asset_exists(texture);
        let backdrop = materials.add(StandardMaterial {
            base_color: if drawn { Color::WHITE } else { fallback },
            base_color_texture: drawn.then(|| assets.load(texture)),
            unlit: true,
            ..default()
        });
        commands.spawn((
            Valley(site),
            Mesh3d(painting.clone()),
            MeshMaterial3d(backdrop),
            Transform::from_translation(origin + Vec3::new(0., 0., edge - 0.30)),
            NotShadowCaster,
            NotShadowReceiver,
            Pickable::IGNORE,
        ));

        let bank = commands
            .spawn((
                Transform::from_translation(origin + Vec3::new(0., 0., edge + 0.24)),
                Visibility::default(),
            ))
            .id();
        if !model(&mut commands, &assets, bank, BANK) {
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(35.8, 0.18, 1.2))),
                MeshMaterial3d(materials.add(Color::srgb(0.34, 0.42, 0.35))),
                Transform::default(),
                Pickable::IGNORE,
                ChildOf(bank),
            ));
        }
        let mill = commands
            .spawn((
                Transform::from_translation(origin + mill)
                    .with_scale(Vec3::splat(scale))
                    .with_rotation(Quat::from_rotation_y(turn)),
                Visibility::default(),
            ))
            .id();
        if !model(&mut commands, &assets, mill, MILL) {
            commands.spawn((
                Mesh3d(meshes.add(ConicalFrustum {
                    radius_top: 0.49,
                    radius_bottom: 0.82,
                    height: 2.13,
                })),
                MeshMaterial3d(materials.add(Color::srgb(0.72, 0.67, 0.54))),
                Transform::from_xyz(0., 1.065, 0.),
                Pickable::IGNORE,
                ChildOf(mill),
            ));
            commands.spawn((
                Mesh3d(meshes.add(Cone::new(0.63, 0.65))),
                MeshMaterial3d(materials.add(Color::srgb(0.32, 0.33, 0.36))),
                Transform::from_xyz(0., 2.40, 0.),
                Pickable::IGNORE,
                ChildOf(mill),
            ));
        }
        let hub = commands
            .spawn((
                Sails,
                Transform::from_translation(AXLE).with_rotation(Quat::from_rotation_z(START_ANGLE)),
                Visibility::default(),
                ChildOf(mill),
            ))
            .id();
        if !model(&mut commands, &assets, hub, SAILS) {
            let linen = materials.add(Color::srgb(0.84, 0.76, 0.60));
            let sail = meshes.add(Cuboid::new(0.22, 1.22, 0.05));
            for arm in 0..4 {
                let rotation = Quat::from_rotation_z(arm as f32 * std::f32::consts::FRAC_PI_2);
                commands.spawn((
                    Mesh3d(sail.clone()),
                    MeshMaterial3d(linen.clone()),
                    Transform::from_translation(rotation * Vec3::new(0.075, 0.61, 0.))
                        .with_rotation(rotation),
                    Pickable::IGNORE,
                    ChildOf(hub),
                ));
            }
        }
    }
}

fn grade(
    weather: Res<WeatherNow>,
    phase: Res<State<Phase>>,
    valley: Query<(&Valley, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (site, handle) in &valley {
        let Some(mut mat) = materials.get_mut(handle) else {
            continue;
        };
        // A missing painting keeps the softly shaded sky of its own scene.
        if mat.base_color_texture.is_none() {
            continue;
        }
        mat.base_color = match site.0 {
            Location::Harbor if *phase.get() == Phase::Morning => Color::srgb(1., 0.90, 0.81),
            Location::Harbor if weather.stormy() => Color::srgb(0.67, 0.72, 0.85),
            Location::Harbor if weather.is(Weather::Fog) => Color::srgb(0.85, 0.85, 0.93),
            _ => Color::WHITE,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn storm_turns_the_same_axle_faster_and_zero_time_holds_its_angle() {
        let mut calm = Breeze::default();
        let mut storm = Breeze::default();
        calm.advance(0.25, false);
        storm.advance(0.25, true);
        assert!(storm.angle < calm.angle && calm.angle < START_ANGLE);
        let at = storm.angle;
        storm.advance(0., true);
        assert_eq!(at, storm.angle);
    }
}
