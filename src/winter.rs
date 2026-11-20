//! Season 2, "The Long Winter": the tower clock on the market square, which
//! the clockmaker's story (`tower_clock` in story.ron) sets going chapter by
//! chapter, and the snow that falls over the harbour and the market once the
//! story marked `snow` is finished.
//!
//! Models (optional): `assets/models/market/clock_tower.glb` (static) and the
//! two hands `clock_hand_hour.glb` / `clock_hand_minute.glb` (origin at the
//! pivot, pointing up along +Y); otherwise simple primitives. The hands and
//! the snow move in code.

use std::f32::consts::TAU;

use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use crate::data::{GameData, asset_exists};
use crate::day_cycle::Progress;
use crate::harbor::HARBOR;
use crate::market::MARKET;
use crate::rng::Rng;

pub struct WinterPlugin;

impl Plugin for WinterPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (spawn_clock_tower, spawn_snow))
            .add_systems(Update, (turn_clock_hands, fall_snow));
    }
}

/// Where the tower stands on the market square (market metres), between the
/// houses at the back.
const TOWER: Vec3 = Vec3::new(-0.8, 0.0, -6.5);
/// Size of the placeholder tower and the height of the clock face's centre.
const TOWER_SIZE: Vec3 = Vec3::new(0.9, 3.3, 0.9);
const FACE_HEIGHT: f32 = 2.75;
const FACE_RADIUS: f32 = 0.36;
/// The time the clock stopped at, in minutes after eleven: 11:52.
const STOPPED_AT: f32 = 52.0;
/// Once going again, how fast the minute hand turns (turns per second).
const RUNNING_SPEED: f32 = 1.0 / 90.0;

/// Snowflakes per area, the areas (centre, half size) and how fast they fall.
const FLAKES: usize = 140;
const FALL_SPEED: (f32, f32) = (0.35, 0.7);

#[derive(Component)]
enum Hand {
    Hour,
    Minute,
}

#[derive(Component)]
struct Flake {
    centre: Vec3,
    half: Vec3,
    speed: f32,
    phase: f32,
}

/// How far the clockmaker's story is, from 0 (stopped) to 1 (going again).
pub fn clock_level(data: &GameData, progress: &Progress) -> f32 {
    data.stories
        .iter()
        .enumerate()
        .find(|(_, s)| s.tower_clock)
        .map_or(1.0, |(i, s)| {
            progress.chapter(i).min(s.chapters.len()) as f32 / s.chapters.len() as f32
        })
}

/// Is it winter yet – the story that brings the snow finished?
pub fn snowing(data: &GameData, progress: &Progress) -> bool {
    data.stories
        .iter()
        .enumerate()
        .any(|(i, s)| s.snow && progress.chapter(i) >= s.chapters.len())
}

fn spawn_clock_tower(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut paint = |c: Color| {
        materials.add(StandardMaterial {
            base_color: c,
            perceptual_roughness: 0.85,
            ..default()
        })
    };
    let tower = commands
        .spawn((
            Transform::from_translation(MARKET + TOWER),
            Visibility::default(),
            Pickable::IGNORE,
        ))
        .id();
    let model = "models/market/clock_tower.glb";
    if asset_exists(model) {
        commands.spawn((
            WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(model))),
            Transform::default(),
            ChildOf(tower),
        ));
    } else {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::from_size(TOWER_SIZE))),
            MeshMaterial3d(paint(Color::srgb(0.5, 0.46, 0.44))),
            Transform::from_xyz(0.0, TOWER_SIZE.y / 2.0, 0.0),
            ChildOf(tower),
        ));
        commands.spawn((
            Mesh3d(meshes.add(Cone {
                radius: TOWER_SIZE.x * 0.75,
                height: 0.9,
            })),
            MeshMaterial3d(paint(Color::srgb(0.25, 0.2, 0.22))),
            Transform::from_xyz(0.0, TOWER_SIZE.y + 0.45, 0.0),
            ChildOf(tower),
        ));
        commands.spawn((
            Mesh3d(meshes.add(Cylinder::new(FACE_RADIUS, 0.04))),
            MeshMaterial3d(paint(Color::srgb(0.92, 0.88, 0.75))),
            Transform::from_xyz(0.0, FACE_HEIGHT, TOWER_SIZE.z / 2.0 + 0.02)
                .with_rotation(Quat::from_rotation_x(TAU / 4.0)),
            ChildOf(tower),
        ));
    }
    // The hands turn about the face's centre, just in front of it.
    for (hand, id, length, width) in [
        (Hand::Hour, "clock_hand_hour", 0.2, 0.05),
        (Hand::Minute, "clock_hand_minute", 0.3, 0.035),
    ] {
        let pivot = commands
            .spawn((
                hand,
                Transform::from_xyz(0.0, FACE_HEIGHT, TOWER_SIZE.z / 2.0 + 0.06),
                Visibility::default(),
                ChildOf(tower),
            ))
            .id();
        let path = format!("models/market/{id}.glb");
        if asset_exists(&path) {
            commands.spawn((
                WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(path))),
                Transform::default(),
                ChildOf(pivot),
            ));
        } else {
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(width, length, 0.02))),
                MeshMaterial3d(paint(Color::srgb(0.12, 0.1, 0.1))),
                Transform::from_xyz(0.0, length / 2.0, 0.0),
                NotShadowCaster,
                ChildOf(pivot),
            ));
        }
    }
}

/// Stopped at 11:52 until the clockmaker comes; each chapter moves the hands
/// on, and with the last one the clock strikes twelve and keeps going.
fn turn_clock_hands(
    time: Res<Time>,
    data: Res<GameData>,
    progress: Res<Progress>,
    mut hands: Query<(&Hand, &mut Transform)>,
    mut running: Local<f32>,
) {
    let level = clock_level(&data, &progress);
    let minutes = if level >= 1.0 {
        *running += time.delta_secs() * RUNNING_SPEED * 60.0;
        60.0 + *running
    } else {
        *running = 0.0;
        STOPPED_AT + (60.0 - STOPPED_AT) * level
    };
    // Minutes after eleven: the hour hand starts just short of twelve.
    let minute_turn = minutes / 60.0;
    let hour_turn = (11.0 + minutes / 60.0) / 12.0;
    for (hand, mut t) in &mut hands {
        let turns = match hand {
            Hand::Hour => hour_turn,
            Hand::Minute => minute_turn,
        };
        // Clockwise as seen from the front (+Z).
        t.rotation = Quat::from_rotation_z(-turns * TAU);
    }
}

fn spawn_snow(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rng: ResMut<Rng>,
) {
    let mesh = meshes.add(Sphere::new(0.025).mesh().ico(0).unwrap());
    let material = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 1.0, 1.0, 0.9),
        emissive: LinearRgba::rgb(0.5, 0.55, 0.6),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    // Over the harbour and over the market square.
    for (centre, half) in [
        (
            HARBOR + Vec3::new(0.0, 3.0, -1.0),
            Vec3::new(10.0, 3.0, 5.0),
        ),
        (MARKET + Vec3::new(0.5, 3.0, -2.5), Vec3::new(8.0, 3.0, 4.5)),
    ] {
        for _ in 0..FLAKES {
            let offset = Vec3::new(
                rng.range(-half.x, half.x),
                rng.range(-half.y, half.y),
                rng.range(-half.z, half.z),
            );
            commands.spawn((
                Flake {
                    centre,
                    half,
                    speed: rng.range(FALL_SPEED.0, FALL_SPEED.1),
                    phase: rng.range(0.0, TAU),
                },
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(centre + offset),
                Visibility::Hidden,
                NotShadowCaster,
                Pickable::IGNORE,
            ));
        }
    }
}

/// The flakes drift down and start again at the top; they only show once
/// winter has come.
fn fall_snow(
    time: Res<Time>,
    data: Res<GameData>,
    progress: Res<Progress>,
    mut flakes: Query<(&Flake, &mut Transform, &mut Visibility)>,
) {
    let show = snowing(&data, &progress);
    let (dt, t) = (time.delta_secs(), time.elapsed_secs());
    for (f, mut transform, mut visibility) in &mut flakes {
        let wanted = if show {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
        if !show {
            continue;
        }
        let p = &mut transform.translation;
        p.y -= f.speed * dt;
        p.x += 0.12 * (t * 0.8 + f.phase).sin() * dt;
        if p.y < f.centre.y - f.half.y {
            p.y += 2.0 * f.half.y;
        }
    }
}
