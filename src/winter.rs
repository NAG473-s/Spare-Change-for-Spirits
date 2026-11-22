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
use crate::day_cycle::{Phase, Progress};
use crate::harbor::HARBOR;
use crate::harbor::Location;
use crate::i18n::Lang;
use crate::market::MARKET;
use crate::rng::Rng;
use crate::ui::GameLog;

pub struct WinterPlugin;

impl Plugin for WinterPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_clock_tower)
            .add_systems(Update, turn_clock_hands);
        if !cfg!(feature = "demo") {
            app.add_systems(
                Startup,
                (
                    spawn_snow,
                    spawn_snow_cover,
                    spawn_window_flowers,
                    spawn_butterfly,
                ),
            )
            .add_systems(
                Update,
                (
                    fall_snow,
                    settle_snow,
                    show_window_flowers,
                    fly_butterfly,
                    notice_butterfly,
                ),
            );
        }
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
        .map_or(0.0, |(i, s)| {
            progress.chapter(i).min(s.chapters.len()) as f32 / s.chapters.len() as f32
        })
}

/// Is it winter – the story that brings the snow finished, and the thaw
/// not begun?
pub fn snowing(data: &GameData, progress: &Progress) -> bool {
    let stories = || data.stories.iter().enumerate();
    stories().any(|(i, s)| s.snow && progress.chapter(i) >= s.chapters.len())
        && !stories().any(|(i, s)| s.thaw && progress.chapter(i) > 0)
}

/// How far spring has come to the market, from 0 to 1: the story marked
/// `blossom` puts flowers on the windows chapter by chapter.
pub fn blossom_level(data: &GameData, progress: &Progress) -> f32 {
    data.stories
        .iter()
        .enumerate()
        .find(|(_, s)| s.blossom)
        .map_or(0.0, |(i, s)| {
            progress.chapter(i).min(s.chapters.len()) as f32 / s.chapters.len() as f32
        })
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
    strike: Res<crate::tower_clock::ClockStrike>,
    settings: Res<crate::save::Settings>,
    mut hands: Query<(&Hand, &mut Transform)>,
    mut running: Local<f32>,
) {
    let level = clock_level(&data, &progress);
    // Reduced motion: the hands stand at twelve without sweeping round.
    let sweeping = strike
        .0
        .filter(|t| *t < crate::tower_clock::SWEEP_SECS && !settings.reduced_motion);
    let minutes = if let Some(t) = sweeping {
        // The first strike: the minute hand sweeps a whole round up to
        // twelve, so the moment shows even with the sound off.
        let k = t / crate::tower_clock::SWEEP_SECS;
        *running = 60.0 * k * k * (3.0 - 2.0 * k);
        60.0 + *running
    } else if level >= 1.0 {
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

/// Snow lying on the market square and the quay once the tower clock has
/// struck: (centre, size) of each patch, how opaque it gets and how many
/// seconds it takes to settle.
const SNOW_COVER: [(Vec3, Vec2); 2] = [
    (Vec3::new(0.5, 0.03, -1.5), Vec2::new(17.0, 9.0)),
    (Vec3::new(1.0, 0.012, -3.2), Vec2::new(20.0, 4.6)),
];
const SNOW_COVER_ALPHA: f32 = 0.5;
const SNOW_SETTLE_SECS: f32 = 10.0;

/// A patch of settled snow and its material (faded in by `settle_snow`).
#[derive(Component)]
struct SnowCover(Handle<StandardMaterial>);

fn spawn_snow_cover(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (i, (centre, size)) in SNOW_COVER.into_iter().enumerate() {
        let origin = if i == 0 { MARKET } else { HARBOR };
        let material = materials.add(StandardMaterial {
            base_color: Color::srgba(0.95, 0.97, 1.0, 0.0),
            perceptual_roughness: 1.0,
            alpha_mode: AlphaMode::Blend,
            ..default()
        });
        commands.spawn((
            SnowCover(material.clone()),
            Mesh3d(meshes.add(Plane3d::new(Vec3::Y, size / 2.0))),
            MeshMaterial3d(material),
            Transform::from_translation(origin + centre),
            NotShadowCaster,
            Pickable::IGNORE,
        ));
    }
}

/// The snow settles slowly after the first strike and stays all winter.
fn settle_snow(
    time: Res<Time<Real>>,
    data: Res<GameData>,
    progress: Res<Progress>,
    covers: Query<&SnowCover>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let target = if snowing(&data, &progress) && progress.journal.clock_struck {
        SNOW_COVER_ALPHA
    } else {
        0.0
    };
    let step = SNOW_COVER_ALPHA * time.delta_secs() / SNOW_SETTLE_SECS;
    for cover in &covers {
        let Some(m) = materials.get(&cover.0) else {
            continue;
        };
        let alpha = m.base_color.alpha();
        if (alpha - target).abs() < f32::EPSILON {
            continue;
        }
        let next = alpha + (target - alpha).clamp(-step, step);
        if let Some(mut m) = materials.get_mut(&cover.0) {
            m.base_color.set_alpha(next);
        }
    }
}

/// Flower boxes under the windows of the market houses (market metres), in
/// the order spring brings them. Model (optional):
/// `assets/models/market/window_flowers.glb`, origin at the bottom centre.
const WINDOW_FLOWERS: [Vec3; 5] = [
    Vec3::new(0.6, 1.05, -5.25),
    Vec3::new(-5.0, 1.05, -5.05),
    Vec3::new(3.4, 1.05, -5.75),
    Vec3::new(-2.2, 1.05, -5.65),
    Vec3::new(6.0, 1.05, -5.15),
];

/// A flower box; it shows once spring has come far enough.
#[derive(Component)]
struct WindowFlowers(usize);

fn spawn_window_flowers(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let model = "models/market/window_flowers.glb";
    let wood = materials.add(StandardMaterial {
        base_color: Color::srgb(0.45, 0.3, 0.18),
        perceptual_roughness: 0.9,
        ..default()
    });
    let colours = [
        Color::srgb(0.95, 0.55, 0.65),
        Color::srgb(0.98, 0.85, 0.35),
        Color::srgb(0.7, 0.55, 0.95),
    ];
    for (n, at) in WINDOW_FLOWERS.into_iter().enumerate() {
        let box_ = commands
            .spawn((
                WindowFlowers(n),
                Transform::from_translation(MARKET + at),
                Visibility::Hidden,
                Pickable::IGNORE,
            ))
            .id();
        if asset_exists(model) {
            commands.spawn((
                WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(model))),
                Transform::default(),
                ChildOf(box_),
            ));
            continue;
        }
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.7, 0.15, 0.18))),
            MeshMaterial3d(wood.clone()),
            Transform::from_xyz(0.0, 0.075, 0.0),
            ChildOf(box_),
        ));
        for k in 0..5 {
            let colour = colours[(n + k) % colours.len()];
            commands.spawn((
                Mesh3d(meshes.add(Sphere::new(0.06))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: colour,
                    perceptual_roughness: 0.8,
                    ..default()
                })),
                Transform::from_xyz(-0.26 + 0.13 * k as f32, 0.2, 0.0),
                NotShadowCaster,
                ChildOf(box_),
            ));
        }
    }
}

fn show_window_flowers(
    data: Res<GameData>,
    progress: Res<Progress>,
    mut boxes: Query<(&WindowFlowers, &mut Visibility)>,
) {
    let shown = (blossom_level(&data, &progress) * WINDOW_FLOWERS.len() as f32).round() as usize;
    for (WindowFlowers(n), mut visibility) in &mut boxes {
        let wanted = if *n < shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
}

/// A small butterfly that comes back to Mira's window boxes every day once
/// they are all in bloom. Models (optional): `assets/models/critters/
/// butterfly_body.glb` and `butterfly_wing.glb` (one wing, hinge at the
/// origin, spreading along +X; the other is mirrored). The flight and the
/// wing beat are done here.
#[derive(Component)]
struct Butterfly;

#[derive(Component)]
struct Wing(f32);

/// Size of the placeholder wing, seconds from one window box to the next,
/// how high it bobs above a box and how fast its wings beat.
const WING_SIZE: Vec2 = Vec2::new(0.12, 0.1);
const BOX_TO_BOX_SECS: f32 = 4.0;
const FLUTTER_HEIGHT: f32 = 0.35;
const WING_BEATS: f32 = 9.0;

fn spawn_butterfly(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let root = commands
        .spawn((
            Butterfly,
            Transform::from_translation(MARKET + WINDOW_FLOWERS[0]),
            Visibility::Hidden,
            Pickable::IGNORE,
        ))
        .id();
    let body_model = "models/critters/butterfly_body.glb";
    let wing_model = "models/critters/butterfly_wing.glb";
    if asset_exists(body_model) {
        commands.spawn((
            WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(body_model))),
            Transform::default(),
            ChildOf(root),
        ));
    } else {
        commands.spawn((
            Mesh3d(meshes.add(Capsule3d::new(0.012, 0.06))),
            MeshMaterial3d(materials.add(Color::srgb(0.25, 0.2, 0.25))),
            Transform::from_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            NotShadowCaster,
            ChildOf(root),
        ));
    }
    let wing_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.98, 0.7, 0.35),
        emissive: LinearRgba::rgb(0.2, 0.12, 0.05),
        cull_mode: None,
        ..default()
    });
    for side in [1.0f32, -1.0] {
        let wing = commands
            .spawn((
                Wing(side),
                Transform::from_scale(Vec3::new(side, 1.0, 1.0)),
                Visibility::default(),
                ChildOf(root),
            ))
            .id();
        if asset_exists(wing_model) {
            commands.spawn((
                WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(wing_model))),
                Transform::default(),
                ChildOf(wing),
            ));
        } else {
            commands.spawn((
                Mesh3d(meshes.add(Rectangle::new(WING_SIZE.x, WING_SIZE.y))),
                MeshMaterial3d(wing_material.clone()),
                Transform::from_xyz(WING_SIZE.x / 2.0, 0.0, 0.0)
                    .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
                NotShadowCaster,
                ChildOf(wing),
            ));
        }
    }
}

/// How far the wings stand open while the butterfly rests (reduced motion).
const CALM_WINGS: f32 = 0.5;

/// From window box to window box, bobbing, with beating wings.
fn fly_butterfly(
    time: Res<Time>,
    data: Res<GameData>,
    progress: Res<Progress>,
    settings: Res<crate::save::Settings>,
    mut butterfly: Query<(&mut Transform, &mut Visibility), (With<Butterfly>, Without<Wing>)>,
    mut wings: Query<(&Wing, &mut Transform), Without<Butterfly>>,
) {
    let show = blossom_level(&data, &progress) >= 1.0;
    // Reduced motion: it rests on the first window box, wings half open.
    let calm = settings.reduced_motion;
    let t = if calm { 0.0 } else { time.elapsed_secs() };
    for (mut transform, mut visibility) in &mut butterfly {
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
        let leg = t / BOX_TO_BOX_SECS;
        let i = leg.floor() as usize % WINDOW_FLOWERS.len();
        let from = MARKET + WINDOW_FLOWERS[i];
        let to = MARKET + WINDOW_FLOWERS[(i + 1) % WINDOW_FLOWERS.len()];
        let k = leg.fract();
        let k = k * k * (3.0 - 2.0 * k);
        let mut pos = from.lerp(to, k);
        pos.y += FLUTTER_HEIGHT * (1.0 + (k * std::f32::consts::PI).sin()) + 0.05 * (t * 5.0).sin();
        let heading = (to - from).with_y(0.0);
        transform.translation = pos;
        if heading.length_squared() > 0.0 {
            // The butterfly's head faces glTF +Z. Bevy's look_to aligns -Z.
            transform.look_to(-heading, Vec3::Y);
        }
    }
    for (wing, mut transform) in &mut wings {
        let beat = if calm {
            CALM_WINGS
        } else {
            0.9 * (t * WING_BEATS * std::f32::consts::TAU).sin().abs()
        };
        transform.rotation = Quat::from_rotation_z(wing.0 * beat);
    }
}

/// The first market day the butterfly is there, the log says so.
fn notice_butterfly(
    data: Res<GameData>,
    mut progress: ResMut<Progress>,
    phase: Res<State<Phase>>,
    location: Res<Location>,
    lang: Res<Lang>,
    mut log: ResMut<GameLog>,
) {
    if *phase.get() != Phase::Day
        || *location != Location::Market
        || progress.journal.butterfly_seen
        || blossom_level(&data, &progress) < 1.0
    {
        return;
    }
    progress.journal.butterfly_seen = true;
    log.push(lang.t("log.butterfly"));
}
