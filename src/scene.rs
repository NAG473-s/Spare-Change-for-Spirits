//! The 3D diorama: camera, room, lighting per time of day, billboards (flat
//! sprites that always face the camera) and labels that follow 3D objects.
//!
//! Coordinates: the floor is the XZ plane at y = 0, one grid cell is one
//! metre. Gameplay positions are `Vec2(x, z)`; [`floor`] lifts them into 3D.

use bevy::light::{CascadeShadowConfigBuilder, NotShadowCaster};
use bevy::prelude::*;

use crate::data::asset_exists;
use crate::day_cycle::{NightClock, Phase};
use crate::ui::{TEXT, UiFont};

/// Where the camera looks; shifted right so the scene sits left of the side panel.
pub const CAMERA_TARGET: Vec3 = Vec3::new(2.9, 0.0, -0.7);
pub const CAMERA_POS: Vec3 = Vec3::new(2.9, 11.4, 14.8);

/// Room bounds (metres).
pub const ROOM_MIN_X: f32 = -7.0;
pub const ROOM_MAX_X: f32 = 7.5;
pub const ROOM_BACK_Z: f32 = -3.0;
pub const ROOM_FRONT_Z: f32 = 3.5;
pub const WALL_HEIGHT: f32 = 3.4;

pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(GlobalAmbientLight {
            color: Color::srgb(0.75, 0.75, 1.0),
            brightness: 300.0,
            ..default()
        })
        .add_systems(
            Startup,
            (spawn_camera, spawn_room, spawn_lights, spawn_wall_clock),
        )
        .add_systems(
            Update,
            (
                time_of_day_light,
                face_camera,
                follow_labels,
                turn_clock_hands,
            ),
        );
    }
}

/// Lifts a floor position `(x, z)` to 3D at height `y`.
pub fn floor(pos: Vec2, y: f32) -> Vec3 {
    Vec3::new(pos.x, y, pos.y)
}

#[derive(Component)]
pub struct MainCamera;

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        MainCamera,
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: 0.62,
            ..default()
        }),
        Transform::from_translation(CAMERA_POS).looking_at(CAMERA_TARGET, Vec3::Y),
    ));
}

fn color_material(
    materials: &mut Assets<StandardMaterial>,
    color: Color,
) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: color,
        perceptual_roughness: 0.9,
        ..default()
    })
}

/// Floor, back and left wall and a moonlit window. A model at
/// `assets/models/shop/room.glb` replaces the walls.
fn spawn_room(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let width = ROOM_MAX_X - ROOM_MIN_X;
    let depth = ROOM_FRONT_Z - ROOM_BACK_Z;
    let center_x = (ROOM_MIN_X + ROOM_MAX_X) / 2.0;
    let center_z = (ROOM_BACK_Z + ROOM_FRONT_Z) / 2.0;

    // The floor below the grid; the grid tiles lie slightly above it.
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(width, depth))),
        MeshMaterial3d(color_material(
            &mut materials,
            Color::srgb(0.16, 0.11, 0.09),
        )),
        Transform::from_xyz(center_x, -0.01, center_z),
    ));

    if asset_exists("models/shop/room.glb") {
        commands.spawn((
            WorldAssetRoot(
                asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/shop/room.glb")),
            ),
            Transform::default(),
        ));
        return;
    }

    let wall = color_material(&mut materials, Color::srgb(0.2, 0.18, 0.3));
    let wainscot = color_material(&mut materials, Color::srgb(0.24, 0.15, 0.11));
    // Back wall
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(width, WALL_HEIGHT, 0.2))),
        MeshMaterial3d(wall.clone()),
        Transform::from_xyz(center_x, WALL_HEIGHT / 2.0, ROOM_BACK_Z - 0.1),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(width, 0.9, 0.05))),
        MeshMaterial3d(wainscot.clone()),
        Transform::from_xyz(center_x, 0.45, ROOM_BACK_Z + 0.02),
    ));
    // Left wall
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.2, WALL_HEIGHT, depth))),
        MeshMaterial3d(wall),
        Transform::from_xyz(ROOM_MIN_X - 0.1, WALL_HEIGHT / 2.0, center_z),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.05, 0.9, depth))),
        MeshMaterial3d(wainscot),
        Transform::from_xyz(ROOM_MIN_X + 0.02, 0.45, center_z),
    ));
    // Window with moonlight on the back wall
    let frame = color_material(&mut materials, Color::srgb(0.3, 0.2, 0.14));
    let glass = materials.add(StandardMaterial {
        base_color: Color::srgb(0.25, 0.35, 0.6),
        emissive: LinearRgba::rgb(0.6, 0.8, 1.6),
        ..default()
    });
    let window = Vec3::new(-3.2, 1.9, ROOM_BACK_Z + 0.03);
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.9, 2.1, 0.06))),
        MeshMaterial3d(frame),
        Transform::from_translation(window),
        children![(
            Mesh3d(meshes.add(Cuboid::new(1.6, 1.8, 0.02))),
            MeshMaterial3d(glass),
            Transform::from_xyz(0.0, 0.0, 0.03),
            NotShadowCaster,
        )],
    ));
}

#[derive(Component)]
struct Moon;

fn spawn_lights(mut commands: Commands) {
    commands.spawn((
        Moon,
        DirectionalLight {
            illuminance: 2000.0,
            color: Color::srgb(0.7, 0.75, 1.0),
            shadow_maps_enabled: true,
            ..default()
        },
        // From the window, upper left behind the room.
        Transform::from_xyz(-4.0, 6.0, -6.0).looking_at(Vec3::new(1.0, 0.0, 1.0), Vec3::Y),
        CascadeShadowConfigBuilder {
            first_cascade_far_bound: 8.0,
            maximum_distance: 25.0,
            ..default()
        }
        .build(),
    ));
}

/// (ambient brightness, ambient colour, moon illuminance, moon colour)
fn lighting_for(phase: Phase) -> (f32, Color, f32, Color) {
    match phase {
        Phase::Menu => (
            220.0,
            Color::srgb(0.7, 0.7, 1.0),
            900.0,
            Color::srgb(0.7, 0.75, 1.0),
        ),
        Phase::Evening => (
            420.0,
            Color::srgb(1.0, 0.82, 0.7),
            2600.0,
            Color::srgb(1.0, 0.7, 0.5),
        ),
        Phase::Night => (
            70.0,
            Color::srgb(0.55, 0.6, 1.0),
            600.0,
            Color::srgb(0.6, 0.7, 1.0),
        ),
        Phase::Morning => (
            650.0,
            Color::srgb(0.9, 0.9, 1.0),
            4000.0,
            Color::srgb(1.0, 0.9, 0.8),
        ),
        Phase::DemoEnd => (
            350.0,
            Color::srgb(1.0, 0.9, 0.8),
            2000.0,
            Color::srgb(1.0, 0.85, 0.7),
        ),
    }
}

/// Fades ambient and moon light towards the current time of day.
fn time_of_day_light(
    time: Res<Time<Real>>,
    phase: Res<State<Phase>>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut moon: Single<&mut DirectionalLight, With<Moon>>,
) {
    let (brightness, color, lux, moon_color) = lighting_for(*phase.get());
    let k = (time.delta_secs() * 0.8).min(1.0);
    ambient.brightness += (brightness - ambient.brightness) * k;
    ambient.color = ambient.color.mix(&color, k);
    moon.illuminance += (lux - moon.illuminance) * k;
    moon.color = moon.color.mix(&moon_color, k);
}

/// A flat sprite that turns around the Y axis to face the camera.
#[derive(Component)]
pub struct Billboard;

fn face_camera(
    camera: Single<&GlobalTransform, With<MainCamera>>,
    mut billboards: Query<&mut Transform, With<Billboard>>,
) {
    let cam = camera.translation();
    for mut t in &mut billboards {
        let d = cam - t.translation;
        t.rotation = Quat::from_rotation_y(d.x.atan2(d.z));
    }
}

/// A quad mesh for billboards, `size` in metres, anchored at its bottom centre.
pub fn billboard_mesh(meshes: &mut Assets<Mesh>, size: Vec2) -> Handle<Mesh> {
    let mesh = Rectangle::new(size.x, size.y)
        .mesh()
        .build()
        .translated_by(Vec3::new(0.0, size.y / 2.0, 0.0));
    meshes.add(mesh)
}

/// Unlit, alpha-blended material for billboards: an image if it exists,
/// otherwise a flat colour.
pub fn billboard_material(
    materials: &mut Assets<StandardMaterial>,
    asset_server: &AssetServer,
    image: &str,
    color: Color,
) -> Handle<StandardMaterial> {
    let texture = asset_exists(image).then(|| asset_server.load(image.to_string()));
    materials.add(StandardMaterial {
        base_color: if texture.is_some() {
            Color::WHITE
        } else {
            color
        },
        base_color_texture: texture,
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        cull_mode: None,
        ..default()
    })
}

/// A UI text that follows a 3D entity on screen (names, shelf labels).
#[derive(Component)]
pub struct WorldLabel {
    pub target: Entity,
    pub offset: Vec3,
}

const LABEL_WIDTH: f32 = 120.0;

pub fn world_label(
    font: &UiFont,
    text: String,
    size: f32,
    target: Entity,
    offset: Vec3,
) -> impl Bundle {
    (
        WorldLabel { target, offset },
        Text::new(text),
        crate::ui::text_font(&font.regular, size),
        TextColor(TEXT),
        TextShadow::default(),
        TextLayout::justify(Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            width: px(LABEL_WIDTH),
            ..default()
        },
        Pickable::IGNORE,
    )
}

fn follow_labels(
    mut commands: Commands,
    camera: Single<(&Camera, &GlobalTransform), With<MainCamera>>,
    targets: Query<&GlobalTransform>,
    mut labels: Query<(Entity, &WorldLabel, &mut Node, &mut Visibility)>,
) {
    let (camera, cam_transform) = *camera;
    for (entity, label, mut node, mut visibility) in &mut labels {
        let Ok(target) = targets.get(label.target) else {
            commands.entity(entity).despawn();
            continue;
        };
        match camera.world_to_viewport(cam_transform, target.translation() + label.offset) {
            Ok(pos) => {
                node.left = px(pos.x - LABEL_WIDTH / 2.0);
                node.top = px(pos.y);
                *visibility = Visibility::Inherited;
            }
            Err(_) => *visibility = Visibility::Hidden,
        }
    }
}

/// Where the wall clock hangs: on the back wall, right of the window.
const CLOCK_POS: Vec3 = Vec3::new(0.9, 2.45, -2.93);
const CLOCK_RADIUS: f32 = 0.34;

/// A clock hand; the value is its length.
#[derive(Component)]
struct ClockHand {
    minute: bool,
}

/// A round wall clock whose hands show the time of night: a little before
/// midnight in the evening, midnight to five while the shop is open, five
/// o'clock at dawn. `models/shop/wall_clock.glb` replaces the face.
fn spawn_wall_clock(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let clock = commands
        .spawn((
            Transform::from_translation(CLOCK_POS),
            Visibility::default(),
            Pickable::IGNORE,
        ))
        .id();
    // Faces +z (towards the camera); cylinders stand along y, so tip them over.
    let facing = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
    let dark = color_material(&mut materials, Color::srgb(0.2, 0.13, 0.09));
    if asset_exists("models/shop/wall_clock.glb") {
        commands.spawn((
            WorldAssetRoot(
                asset_server
                    .load(GltfAssetLabel::Scene(0).from_asset("models/shop/wall_clock.glb")),
            ),
            Transform::default(),
            ChildOf(clock),
        ));
    } else {
        commands.spawn((
            Mesh3d(meshes.add(Cylinder::new(CLOCK_RADIUS + 0.05, 0.06))),
            MeshMaterial3d(dark.clone()),
            Transform::from_rotation(facing),
            ChildOf(clock),
        ));
        commands.spawn((
            Mesh3d(meshes.add(Cylinder::new(CLOCK_RADIUS, 0.02))),
            MeshMaterial3d(color_material(&mut materials, Color::srgb(0.86, 0.8, 0.66))),
            Transform::from_xyz(0.0, 0.0, 0.03).with_rotation(facing),
            ChildOf(clock),
        ));
        let tick = meshes.add(Cuboid::new(0.025, 0.06, 0.01));
        for i in 0..12 {
            let a = i as f32 / 12.0 * std::f32::consts::TAU;
            let r = CLOCK_RADIUS - 0.05;
            commands.spawn((
                Mesh3d(tick.clone()),
                MeshMaterial3d(dark.clone()),
                Transform::from_xyz(a.sin() * r, a.cos() * r, 0.045)
                    .with_rotation(Quat::from_rotation_z(-a)),
                ChildOf(clock),
            ));
        }
    }
    for (minute, length, width) in [(false, 0.17, 0.035), (true, 0.27, 0.022)] {
        let hand = commands
            .spawn((
                ClockHand { minute },
                Transform::from_xyz(0.0, 0.0, if minute { 0.065 } else { 0.055 }),
                Visibility::default(),
                ChildOf(clock),
            ))
            .id();
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(width, length, 0.008))),
            MeshMaterial3d(dark.clone()),
            Transform::from_xyz(0.0, length / 2.0 - 0.03, 0.0),
            NotShadowCaster,
            ChildOf(hand),
        ));
    }
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(0.025))),
        MeshMaterial3d(color_material(&mut materials, Color::srgb(0.7, 0.55, 0.3))),
        Transform::from_xyz(0.0, 0.0, 0.075),
        ChildOf(clock),
    ));
}

/// Minutes after midnight shown by the shop's clock (negative before midnight).
pub fn shop_minutes(phase: Phase, clock: &NightClock) -> f32 {
    match phase {
        Phase::Night => clock.elapsed / clock.length.max(1.0) * 300.0,
        Phase::Morning | Phase::DemoEnd => 300.0,
        Phase::Evening | Phase::Menu => -25.0,
    }
    .min(300.0)
}

fn turn_clock_hands(
    phase: Res<State<Phase>>,
    clock: Res<NightClock>,
    mut hands: Query<(&ClockHand, &mut Transform)>,
) {
    let minutes = shop_minutes(*phase.get(), &clock).rem_euclid(720.0);
    let tau = std::f32::consts::TAU;
    for (hand, mut transform) in &mut hands {
        // Clockwise as seen from the front means negative about +z.
        let turn = if hand.minute {
            minutes / 60.0
        } else {
            minutes / 720.0
        };
        transform.rotation = Quat::from_rotation_z(-turn * tau);
    }
}
