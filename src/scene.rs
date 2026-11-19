//! The 3D diorama: camera, room, lighting per time of day, billboards (flat
//! sprites that always face the camera) and labels that follow 3D objects.
//!
//! Coordinates: the floor is the XZ plane at y = 0, one grid cell is one
//! metre. Gameplay positions are `Vec2(x, z)`; [`floor`] lifts them into 3D.

use bevy::light::{CascadeShadowConfigBuilder, NotShadowCaster};
use bevy::prelude::*;

use crate::data::asset_exists;
use crate::day_cycle::{NightClock, Phase};
use crate::harbor::Location;
use crate::ui::{TEXT, UiFont};

/// Where the camera looks; shifted right so the scene sits left of the side panel.
/// Camera at the harbour and the market (relative to their origin).
pub const CAMERA_TARGET: Vec3 = Vec3::new(2.9, 0.0, -0.7);
pub const CAMERA_POS: Vec3 = Vec3::new(2.9, 11.4, 14.8);
/// Camera in the shop. Kept apart from the harbour and market camera so
/// the shop can be framed on its own (a closer, slightly turned view is
/// being considered); for now it is the same view.
pub const SHOP_CAMERA_TARGET: Vec3 = Vec3::new(2.9, 0.0, -0.7);
pub const SHOP_CAMERA_POS: Vec3 = Vec3::new(2.9, 11.4, 14.8);

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
        .init_resource::<ClockOverride>()
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
        Transform::from_translation(SHOP_CAMERA_POS).looking_at(SHOP_CAMERA_TARGET, Vec3::Y),
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
struct Daylight;

#[derive(Component)]
struct WindowLight;

fn spawn_lights(mut commands: Commands) {
    commands.spawn((
        Daylight,
        DirectionalLight {
            illuminance: 720.0,
            color: Color::srgb(0.69, 0.80, 1.0),
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::default().looking_to(Vec3::new(4.0, -6.0, -3.0), Vec3::Y),
        CascadeShadowConfigBuilder {
            first_cascade_far_bound: 8.0,
            maximum_distance: 38.0,
            ..default()
        }
        .build(),
    ));
    // Start on the room side of the glazing, so the panes and their frame
    // cannot block the light. The broad cone grazes the floor and shelves.
    commands.spawn((
        WindowLight,
        SpotLight {
            intensity: 0.0,
            range: 9.0,
            radius: 0.38,
            inner_angle: 0.38,
            outer_angle: 0.91,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(-3.2, 2.32, ROOM_BACK_Z + 0.32)
            .looking_at(Vec3::new(-1.05, 0.0, 0.7), Vec3::Y),
    ));
}

struct DaylightSettings {
    /// Diffuse skylight, including the shadowed side of stalls and furniture.
    sky: (f32, Color),
    /// Direct illuminance, colour and direction towards the source.
    sun: (f32, Color, Vec3),
    /// Light transmitted by the shop window, in lumens.
    window: (f32, Color),
}

fn lighting_for(phase: Phase, location: Location) -> DaylightSettings {
    let cool_glass = Color::srgb(0.66, 0.79, 1.0);
    match phase {
        Phase::Evening if location == Location::Harbor => DaylightSettings {
            sky: (710.0, Color::srgb(0.71, 0.81, 1.0)),
            sun: (
                1750.0,
                Color::srgb(0.72, 0.84, 1.0),
                Vec3::new(-5.0, 6.0, -3.0),
            ),
            window: (0.0, cool_glass),
        },
        Phase::Evening => DaylightSettings {
            sky: (600.0, Color::srgb(0.82, 0.84, 0.98)),
            sun: (
                2300.0,
                Color::srgb(1.0, 0.79, 0.60),
                Vec3::new(-2.0, 5.5, 4.5),
            ),
            window: (70_000.0, cool_glass),
        },
        Phase::Menu | Phase::Night => DaylightSettings {
            sky: (350.0, Color::srgb(0.72, 0.80, 0.99)),
            sun: (
                720.0,
                Color::srgb(0.69, 0.80, 1.0),
                Vec3::new(-3.0, 7.0, -2.0),
            ),
            window: (110_000.0, cool_glass),
        },
        Phase::Day => DaylightSettings {
            sky: (1080.0, Color::srgb(0.86, 0.93, 1.0)),
            sun: (
                5700.0,
                Color::srgb(1.0, 0.94, 0.83),
                Vec3::new(-5.0, 9.0, 5.0),
            ),
            window: (0.0, cool_glass),
        },
        Phase::Morning | Phase::DemoEnd => DaylightSettings {
            sky: (750.0, Color::srgb(0.89, 0.92, 1.0)),
            sun: (
                3400.0,
                Color::srgb(1.0, 0.85, 0.70),
                Vec3::new(5.0, 5.0, 4.0),
            ),
            window: (95_000.0, Color::srgb(1.0, 0.86, 0.74)),
        },
    }
}

/// Fade both the light and the cast shadows as the scene or time of day changes.
fn time_of_day_light(
    time: Res<Time<Real>>,
    phase: Res<State<Phase>>,
    location: Res<Location>,
    mut ambient: ResMut<GlobalAmbientLight>,
    daylight: Single<(&mut DirectionalLight, &mut Transform), With<Daylight>>,
    mut window: Single<&mut SpotLight, With<WindowLight>>,
) {
    let target = lighting_for(*phase.get(), *location);
    let k = 1.0 - (-1.8 * time.delta_secs()).exp();
    let (mut sun, mut transform) = daylight.into_inner();
    let rotation = Transform::default()
        .looking_to(-target.sun.2, Vec3::Y)
        .rotation;
    ambient.brightness = ambient.brightness.lerp(target.sky.0, k);
    ambient.color = ambient.color.mix(&target.sky.1, k);
    sun.illuminance = sun.illuminance.lerp(target.sun.0, k);
    sun.color = sun.color.mix(&target.sun.1, k);
    transform.rotation = transform.rotation.slerp(rotation, k);
    window.intensity = window.intensity.lerp(target.window.0, k);
    window.color = window.color.mix(&target.window.1, k);
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

pub(crate) const LABEL_WIDTH: f32 = 120.0;

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
    ui_scale: Res<UiScale>,
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
                // Window pixels to interface units.
                let pos = pos / ui_scale.0;
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
        Phase::Day | Phase::Evening | Phase::Menu => -25.0,
    }
    .min(300.0)
}

/// Minutes shown instead of the time of night, while the clock runs on to
/// midnight as the shop opens (`sign.rs`).
#[derive(Resource, Default)]
pub struct ClockOverride(pub Option<f32>);

fn turn_clock_hands(
    phase: Res<State<Phase>>,
    clock: Res<NightClock>,
    clock_override: Res<ClockOverride>,
    mut hands: Query<(&ClockHand, &mut Transform)>,
) {
    let minutes = clock_override
        .0
        .unwrap_or_else(|| shop_minutes(*phase.get(), &clock))
        .rem_euclid(720.0);
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
