//! The 3D diorama: camera, room, lighting per time of day, billboards (flat
//! sprites that always face the camera) and labels that follow 3D objects.
//!
//! Coordinates: the floor is the XZ plane at y = 0, one grid cell is one
//! metre. Gameplay positions are `Vec2(x, z)`; [`floor`] lifts them into 3D.

use bevy::camera::visibility::RenderLayers;
use bevy::light::{CascadeShadowConfigBuilder, NotShadowCaster};
use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;

use crate::data::asset_exists;
use crate::day_cycle::{NightClock, Phase};
use crate::harbor::Location;
use crate::shop::entrance::{DoorSwing, PIVOT as DOOR_PIVOT};
mod labels;
mod orchard;
pub(crate) mod village;
mod window;
pub(crate) use labels::{LabelEnabled, LabelKind, world_label};

pub(crate) const DIORAMA_FOV: f32 = 0.62;

/// Where the camera looks; shifted right so the scene sits left of the side panel.
/// Camera at the harbour and the market (relative to their origin).
pub const CAMERA_TARGET: Vec3 = Vec3::new(2.9, 0.0, -0.7);
pub const CAMERA_POS: Vec3 = Vec3::new(2.9, 11.4, 14.8);
/// Look slightly across the shop so the entrance, its signs and the counter
/// are all visible beside the interface.
pub const SHOP_CAMERA_TARGET: Vec3 = Vec3::new(2.9, 0.0, -0.7);
pub const SHOP_CAMERA_POS: Vec3 = Vec3::new(6.5, 11.4, 14.8);

/// Room bounds (metres).
pub const ROOM_MIN_X: f32 = -7.0;
pub const ROOM_MAX_X: f32 = 7.5;
pub const ROOM_BACK_Z: f32 = -3.0;
pub const ROOM_FRONT_Z: f32 = 3.5;
pub const WALL_HEIGHT: f32 = 3.4;

/// The floor reaches to here (x, z): past the open front and the right side
/// of the room, so no black shows around the shop.
const FLOOR_MAX: Vec2 = Vec2::new(14.0, 12.0);
const FLOOR_TEXTURE: &str = "sprites/shop/floor.png";
/// The floor outside the grid is a little darker than the grid tiles.
const FLOOR_TINT: f32 = 0.76;
/// The walls continue above the room model up to here, and the left wall
/// towards the camera to `FLOOR_MAX.y`, so the walls fill the top and the
/// left edge of the picture.
const WALL_TOP: f32 = 6.5;
const WALL_THICKNESS: f32 = 0.22;
/// The wallpaper's diamonds, as in the room model: columns and rows (m),
/// the first one, its size, and the wainscot along the bottom.
const DIAMOND_STEP: Vec2 = Vec2::new(0.958, 0.57);
const DIAMOND_FIRST: Vec2 = Vec2::new(-6.46, 1.21);
const DIAMOND_SIZE: Vec2 = Vec2::new(0.2, 0.34);
const WAINSCOT_HEIGHT: f32 = 0.9;

pub struct ScenePlugin;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct PlaceWorldLabels;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        village::install(app);
        orchard::install(app);
        window::install(app);
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
        .add_systems(PostStartup, spawn_door_light)
        .add_systems(
            Update,
            (
                set_camera_layers.run_if(state_changed::<Phase>),
                time_of_day_light.after(crate::shop::entrance::animate),
                face_camera,
                (
                    labels::paint_labels,
                    labels::size_labels,
                    labels::follow_labels.in_set(PlaceWorldLabels),
                )
                    .chain(),
                turn_clock_hands,
            ),
        );
    }
}

/// Scene children are instantiated after their root. Let clicks pass through their meshes
/// to the gameplay target behind them, just as they do for primitive scenery.
pub(crate) fn ignore_scene_picking(
    ready: On<WorldInstanceReady>,
    children: Query<&Children>,
    mut commands: Commands,
) {
    for child in children.iter_descendants(ready.entity) {
        commands.entity(child).insert(Pickable::IGNORE);
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
        RenderLayers::none(),
        Projection::Perspective(PerspectiveProjection {
            fov: DIORAMA_FOV,
            ..default()
        }),
        Transform::from_translation(SHOP_CAMERA_POS).looking_at(SHOP_CAMERA_TARGET, Vec3::Y),
    ));
}

/// The opening and title fill the window. Avoid drawing the diorama behind them.
fn set_camera_layers(
    phase: Res<State<Phase>>,
    mut camera: Single<&mut RenderLayers, With<MainCamera>>,
) {
    **camera = if *phase.get() == Phase::Menu {
        RenderLayers::none()
    } else {
        RenderLayers::default()
    };
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

/// Taller walls (above the room) and the left wall carried on towards the
/// camera, in the room model's colours and with its diamond wallpaper.
fn spawn_wall_extension(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let mut paint = |r: f32, g: f32, b: f32, roughness: f32| {
        materials.add(StandardMaterial {
            base_color: Color::linear_rgb(r, g, b),
            perceptual_roughness: roughness,
            ..default()
        })
    };
    // The room model's materials (linear colours).
    let wall = paint(0.034, 0.036, 0.067, 0.92);
    let pattern = paint(0.06, 0.056, 0.095, 0.94);
    let wood = paint(0.125, 0.056, 0.056, 0.73);
    let trim = paint(0.19, 0.089, 0.074, 0.7);
    let mut slab = |size: Vec3, at: Vec3, material: &Handle<StandardMaterial>| {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::from_size(size))),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(at),
            Pickable::IGNORE,
        ));
    };
    let above = WALL_TOP - WALL_HEIGHT;
    let back_width = FLOOR_MAX.x - ROOM_MIN_X;
    let back_z = ROOM_BACK_Z - WALL_THICKNESS / 2.0;
    let left_x = ROOM_MIN_X - WALL_THICKNESS / 2.0;
    // Above the back wall, all the way to the right edge of the floor.
    slab(
        Vec3::new(back_width, above, WALL_THICKNESS),
        Vec3::new(
            ROOM_MIN_X + back_width / 2.0,
            WALL_HEIGHT + above / 2.0,
            back_z,
        ),
        &wall,
    );
    // The back wall goes on to the right of the room, full height.
    let right_width = FLOOR_MAX.x - ROOM_MAX_X;
    slab(
        Vec3::new(right_width, WALL_HEIGHT, WALL_THICKNESS),
        Vec3::new(ROOM_MAX_X + right_width / 2.0, WALL_HEIGHT / 2.0, back_z),
        &wall,
    );
    // Above the left wall, and the left wall on towards the camera.
    let depth = ROOM_FRONT_Z - ROOM_BACK_Z;
    slab(
        Vec3::new(WALL_THICKNESS, above, depth),
        Vec3::new(left_x, WALL_HEIGHT + above / 2.0, ROOM_BACK_Z + depth / 2.0),
        &wall,
    );
    let front = FLOOR_MAX.y - ROOM_FRONT_Z;
    slab(
        Vec3::new(WALL_THICKNESS, WALL_TOP, front),
        Vec3::new(left_x, WALL_TOP / 2.0, ROOM_FRONT_Z + front / 2.0),
        &wall,
    );
    // Wainscot on the new pieces of wall.
    slab(
        Vec3::new(0.05, WAINSCOT_HEIGHT, front),
        Vec3::new(
            ROOM_MIN_X + 0.02,
            WAINSCOT_HEIGHT / 2.0,
            ROOM_FRONT_Z + front / 2.0,
        ),
        &wood,
    );
    slab(
        Vec3::new(right_width, WAINSCOT_HEIGHT, 0.05),
        Vec3::new(
            ROOM_MAX_X + right_width / 2.0,
            WAINSCOT_HEIGHT / 2.0,
            ROOM_BACK_Z + 0.02,
        ),
        &wood,
    );
    // A moulding where the room model's walls end.
    slab(
        Vec3::new(back_width, 0.1, 0.08),
        Vec3::new(
            ROOM_MIN_X + back_width / 2.0,
            WALL_HEIGHT + 0.05,
            ROOM_BACK_Z + 0.03,
        ),
        &trim,
    );
    slab(
        Vec3::new(0.08, 0.1, FLOOR_MAX.y - ROOM_BACK_Z),
        Vec3::new(
            ROOM_MIN_X + 0.03,
            WALL_HEIGHT + 0.05,
            (ROOM_BACK_Z + FLOOR_MAX.y) / 2.0,
        ),
        &trim,
    );

    // The diamonds, wherever the room model has none.
    let diamond = meshes.add(Rhombus::new(DIAMOND_SIZE.x, DIAMOND_SIZE.y));
    let mut place = |at: Vec3, facing: Quat| {
        commands.spawn((
            Mesh3d(diamond.clone()),
            MeshMaterial3d(pattern.clone()),
            Transform::from_translation(at).with_rotation(facing),
            Pickable::IGNORE,
        ));
    };
    let rows = |from: f32| {
        (0..)
            .map(move |k| DIAMOND_FIRST.y + k as f32 * DIAMOND_STEP.y)
            .skip_while(move |y| *y < from)
            .take_while(|y| *y < WALL_TOP - DIAMOND_SIZE.y)
    };
    let model_top = WALL_HEIGHT - 0.2;
    // Back wall: above the model everywhere, full height right of the room.
    for col in 0.. {
        let x = DIAMOND_FIRST.x + col as f32 * DIAMOND_STEP.x;
        if x > FLOOR_MAX.x {
            break;
        }
        let from = if x > ROOM_MAX_X {
            WAINSCOT_HEIGHT + 0.2
        } else {
            model_top
        };
        for y in rows(from) {
            place(Vec3::new(x, y, ROOM_BACK_Z + 0.005), Quat::IDENTITY);
        }
    }
    // Left wall (facing the room, +x): above the model, and in front of it.
    let facing = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    for col in 0.. {
        let z = ROOM_BACK_Z + 0.55 + col as f32 * DIAMOND_STEP.x;
        if z > FLOOR_MAX.y {
            break;
        }
        let from = if z > ROOM_FRONT_Z {
            WAINSCOT_HEIGHT + 0.2
        } else {
            model_top
        };
        for y in rows(from) {
            place(Vec3::new(ROOM_MIN_X + 0.005, y, z), facing);
        }
    }
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

    // The floor below the grid (the grid tiles lie slightly above it). It
    // reaches past the open front and the right side, so the camera sees
    // floorboards instead of black, with the planks of the grid tiles.
    let floor_size = Vec2::new(FLOOR_MAX.x - ROOM_MIN_X, FLOOR_MAX.y - ROOM_BACK_Z);
    let floor_material = if asset_exists(FLOOR_TEXTURE) {
        StandardMaterial {
            base_color: Color::srgb(FLOOR_TINT, FLOOR_TINT, FLOOR_TINT),
            base_color_texture: Some(crate::harbor::repeating(&asset_server, FLOOR_TEXTURE, true)),
            uv_transform: bevy::math::Affine2::from_scale(floor_size),
            perceptual_roughness: 0.85,
            ..default()
        }
    } else {
        StandardMaterial {
            base_color: Color::srgb(0.16, 0.11, 0.09),
            perceptual_roughness: 0.9,
            ..default()
        }
    };
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(floor_size.x, floor_size.y))),
        MeshMaterial3d(materials.add(floor_material)),
        Transform::from_xyz(
            ROOM_MIN_X + floor_size.x / 2.0,
            -0.01,
            ROOM_BACK_Z + floor_size.y / 2.0,
        ),
        Pickable::IGNORE,
    ));
    spawn_wall_extension(&mut commands, &mut meshes, &mut materials);

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
    let window = Vec3::new(-3.2, 1.8, ROOM_BACK_Z + 0.03);
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.9, 2.1, 0.06))),
        MeshMaterial3d(frame.clone()),
        Transform::from_translation(window),
        children![(
            Mesh3d(meshes.add(Cuboid::new(1.6, 1.8, 0.02))),
            MeshMaterial3d(glass),
            Transform::from_xyz(0.0, 0.0, 0.03),
            NotShadowCaster,
        )],
    ));
    // Match the room model's sill so the window display has a surface
    // even when the model is unavailable.
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.9, 0.11, 0.28))),
        MeshMaterial3d(frame),
        Transform::from_xyz(-3.2, WINDOW_SILL_Y - 0.055, ROOM_BACK_Z + 0.15),
    ));
}

/// Top of the window sill in the room model and its fallback.
pub(crate) const WINDOW_SILL_Y: f32 = 0.941;

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

/// Moonlight transmitted through the little bullseye in the entrance.
#[derive(Component)]
struct DoorGlass(Option<Handle<StandardMaterial>>);

const PORTHOLE_LIGHT: &str = "sprites/shop/porthole_light.png";
const PORTHOLE_HEIGHT: f32 = 1.62;

fn spawn_door_light(
    mut commands: Commands,
    hinge: Single<Entity, With<DoorSwing>>,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let reflection = asset_exists(PORTHOLE_LIGHT).then(|| {
        let glass = materials.add(StandardMaterial {
            base_color: Color::NONE,
            base_color_texture: Some(assets.load(PORTHOLE_LIGHT)),
            alpha_mode: AlphaMode::Add,
            unlit: true,
            ..default()
        });
        let patch = Vec3::new(-0.32, 0.003, 1.42) - DOOR_PIVOT;
        commands.spawn((
            Mesh3d(meshes.add(Plane3d::default().mesh().size(1.02, 1.96))),
            MeshMaterial3d(glass.clone()),
            Transform::from_translation(patch).with_rotation(Quat::from_rotation_y(-0.22)),
            ChildOf(*hinge),
            NotShadowCaster,
            Pickable::IGNORE,
        ));
        glass
    });
    commands.spawn((
        DoorGlass(reflection),
        PointLight {
            intensity: 0.0,
            radius: 0.19,
            range: 3.7,
            ..default()
        },
        Transform::from_translation(Vec3::new(0.0, PORTHOLE_HEIGHT, 0.44) - DOOR_PIVOT),
        ChildOf(*hinge),
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
        // Dawn at the quay after a finale (`finale.rs`): still dim enough
        // for the lighthouse beam to show.
        Phase::Morning if location == Location::Harbor => DaylightSettings {
            sky: (520.0, Color::srgb(0.80, 0.80, 0.98)),
            sun: (
                1300.0,
                Color::srgb(1.0, 0.78, 0.62),
                Vec3::new(6.0, 2.5, -4.0),
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
    door: Single<(&DoorGlass, &mut PointLight)>,
    hinge: Single<&DoorSwing>,
    mut materials: ResMut<Assets<StandardMaterial>>,
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
    let (glass, mut light) = door.into_inner();
    // As the leaf turns, the bullseye's cross fades into the broader light from the quay.
    let opening = hinge.aperture();
    const OPEN_QUAY_LIGHT: f32 = 9_400.0;
    light.intensity = window.intensity * 0.32 + OPEN_QUAY_LIGHT * opening;
    light.color = window.color;
    if let Some(paint) = &glass.0 {
        let opacity = (window.intensity / 110_000.0).clamp(0.0, 1.0) * 0.29 * (1.0 - opening);
        let tint = light.color.with_alpha(opacity);
        if materials.get(paint).is_some_and(|m| m.base_color != tint)
            && let Some(mut m) = materials.get_mut(paint)
        {
            m.base_color = tint;
        }
    }
}

/// A flat sprite that turns around the Y axis to face the camera.
#[derive(Component)]
pub struct Billboard;

pub(crate) fn face_camera(
    camera: Single<&Transform, (With<MainCamera>, Without<Billboard>)>,
    parents: Query<&GlobalTransform>,
    mut billboards: Query<(&mut Transform, Option<&ChildOf>), With<Billboard>>,
) {
    for (mut t, parent) in &mut billboards {
        // The coin pictures on a market stall and wares on the ferry are
        // children of moving or rotated models. Yaw in the same local
        // space as the sprite, including the diorama's world offset.
        let cam = match parent {
            Some(parent) => {
                let Ok(at) = parents.get(parent.parent()) else {
                    continue;
                };
                at.affine().inverse().transform_point3(camera.translation)
            }
            None => camera.translation,
        };
        let d = cam - t.translation;
        let rotation = Quat::from_rotation_y(d.x.atan2(d.z));
        if t.rotation != rotation {
            t.rotation = rotation;
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_billboard_on_a_rotated_market_stall_faces_the_camera() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin))
            .add_systems(Update, face_camera);
        let camera = crate::market::MARKET + CAMERA_POS;
        app.world_mut()
            .spawn((MainCamera, Transform::from_translation(camera)));
        let stall = app
            .world_mut()
            .spawn(
                Transform::from_translation(crate::market::MARKET + Vec3::new(-3.0, 0.0, 1.0))
                    .with_rotation(Quat::from_rotation_y(0.5)),
            )
            .id();
        let coin = app
            .world_mut()
            .spawn((
                Billboard,
                Transform::from_xyz(0.4, 0.9, 0.0),
                ChildOf(stall),
            ))
            .id();
        app.update();
        let at = app.world().get::<GlobalTransform>(coin).unwrap();
        let towards_camera = (camera - at.translation()).with_y(0.0).normalize();
        assert!((at.rotation() * Vec3::Z).dot(towards_camera) > 0.9999);
    }
}
