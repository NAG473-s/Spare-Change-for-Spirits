//! Visual effects in the diorama: flickering candle lights and particles.
//! Also defines `Cue`, the message gameplay sends for sounds and effects.
//! (Time-of-day lighting lives in `scene.rs`.)

use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use crate::day_cycle::Phase;
use crate::rng::Rng;
use crate::scene::floor;
use crate::shop::{CELL, ShopLayout};

/// Something happened that should be heard or seen.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub enum Cue {
    GhostEnters,
    Paid,
    ChangeGiven,
    /// A ghost passes into the light at this floor position.
    Ascend(Vec2),
    /// Something was knocked off a shelf at this floor position.
    Knock(Vec2),
    Disappointed,
    Click,
}

pub struct FxPlugin;

impl Plugin for FxPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Cue>()
            .init_resource::<FxAssets>()
            .add_systems(
                Update,
                (
                    flicker,
                    cue_particles,
                    move_particles,
                    dust_motes.run_if(in_state(Phase::Night)),
                ),
            );
    }
}

/// Shared meshes and materials for particles.
#[derive(Resource)]
struct FxAssets {
    dot: Handle<Mesh>,
    shard: Handle<Mesh>,
    sparkle: Handle<StandardMaterial>,
    glass: Handle<StandardMaterial>,
    dust: Handle<StandardMaterial>,
}

impl FromWorld for FxAssets {
    fn from_world(world: &mut World) -> Self {
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        let dot = meshes.add(Sphere::new(1.0).mesh().ico(1).unwrap());
        let shard = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        let unlit = |color: Color, emissive: LinearRgba| StandardMaterial {
            base_color: color,
            emissive,
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        };
        Self {
            dot,
            shard,
            sparkle: materials.add(unlit(
                Color::srgba(1.0, 0.95, 0.75, 0.9),
                LinearRgba::rgb(3.0, 2.6, 1.6),
            )),
            glass: materials.add(StandardMaterial {
                base_color: Color::srgb(0.75, 0.8, 1.0),
                perceptual_roughness: 0.2,
                ..default()
            }),
            dust: materials.add(unlit(
                Color::srgba(0.85, 0.85, 1.0, 0.35),
                LinearRgba::BLACK,
            )),
        }
    }
}

/// A point light that flickers like a flame.
#[derive(Component)]
pub struct Flicker {
    pub base: f32,
    pub phase: f32,
}

fn flicker(time: Res<Time<Real>>, mut lights: Query<(&Flicker, &mut PointLight)>) {
    let t = time.elapsed_secs();
    for (f, mut light) in &mut lights {
        let wobble =
            0.85 + 0.09 * (t * 7.3 + f.phase).sin() + 0.06 * (t * 13.1 + f.phase * 2.3).sin();
        light.intensity = f.base * wobble;
    }
}

#[derive(Component)]
struct Particle {
    vel: Vec3,
    /// Downward pull (positive) or upward drift (negative).
    gravity: f32,
    life: f32,
    max_life: f32,
    size: f32,
}

#[derive(Component)]
struct Mote;

fn spawn_particle(
    commands: &mut Commands,
    mesh: &Handle<Mesh>,
    material: &Handle<StandardMaterial>,
    pos: Vec3,
    vel: Vec3,
    gravity: f32,
    life: f32,
    size: f32,
) -> Entity {
    commands
        .spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(pos).with_scale(Vec3::splat(size)),
            Particle {
                vel,
                gravity,
                life,
                max_life: life,
                size,
            },
            NotShadowCaster,
            Pickable::IGNORE,
        ))
        .id()
}

fn cue_particles(
    mut commands: Commands,
    mut cues: MessageReader<Cue>,
    fx: Res<FxAssets>,
    mut rng: ResMut<Rng>,
) {
    for cue in cues.read() {
        match *cue {
            Cue::Ascend(pos) => {
                for _ in 0..22 {
                    let start = floor(pos, rng.range(0.2, 1.4))
                        + Vec3::new(rng.range(-0.3, 0.3), 0.0, rng.range(-0.3, 0.3));
                    let vel = Vec3::new(
                        rng.range(-0.2, 0.2),
                        rng.range(0.4, 1.0),
                        rng.range(-0.2, 0.2),
                    );
                    let size = rng.range(0.02, 0.05);
                    spawn_particle(
                        &mut commands,
                        &fx.dot,
                        &fx.sparkle,
                        start,
                        vel,
                        -0.4,
                        rng.range(1.2, 2.2),
                        size,
                    );
                }
            }
            Cue::Knock(pos) => {
                for _ in 0..12 {
                    let start = floor(pos, 0.8) + Vec3::new(0.0, 0.0, 0.3);
                    let angle = rng.range(0.0, std::f32::consts::TAU);
                    let speed = rng.range(0.8, 2.0);
                    let vel = Vec3::new(
                        angle.cos() * speed,
                        rng.range(1.0, 2.5),
                        angle.sin() * speed,
                    );
                    let size = rng.range(0.02, 0.05);
                    spawn_particle(
                        &mut commands,
                        &fx.shard,
                        &fx.glass,
                        start,
                        vel,
                        9.0,
                        rng.range(0.5, 0.8),
                        size,
                    );
                }
            }
            _ => {}
        }
    }
}

/// Particles follow the game clock, so they freeze while paused. They
/// shrink away instead of fading, so they can share one material.
fn move_particles(
    mut commands: Commands,
    time: Res<Time>,
    mut particles: Query<(Entity, &mut Particle, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (entity, mut p, mut t) in &mut particles {
        p.life -= dt;
        if p.life <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        p.vel.y -= p.gravity * dt;
        t.translation += p.vel * dt;
        if t.translation.y < 0.01 && p.gravity > 0.0 {
            // Shards come to rest on the floor.
            t.translation.y = 0.01;
            p.vel = Vec3::ZERO;
        }
        let age = 1.0 - p.life / p.max_life;
        let grow = (age * 8.0).min(1.0);
        let fade = (p.life / p.max_life).sqrt();
        t.scale = Vec3::splat(p.size * grow * fade);
    }
}

/// Faint dust drifting through the shop at night.
fn dust_motes(
    mut commands: Commands,
    layout: Res<ShopLayout>,
    fx: Res<FxAssets>,
    mut rng: ResMut<Rng>,
    motes: Query<(), With<Mote>>,
) {
    if motes.iter().count() >= 30 || !rng.chance(0.2) {
        return;
    }
    let a = layout.cell_pos(0);
    let b = layout.cell_pos(layout.cells.len() - 1);
    let (min, max) = (a.min(b) - Vec2::splat(CELL), a.max(b) + Vec2::splat(CELL));
    let pos = Vec3::new(
        rng.range(min.x, max.x),
        rng.range(0.3, 2.6),
        rng.range(min.y, max.y),
    );
    let vel = Vec3::new(
        rng.range(-0.08, 0.08),
        rng.range(-0.03, 0.05),
        rng.range(-0.08, 0.08),
    );
    let size = rng.range(0.01, 0.02);
    let id = spawn_particle(
        &mut commands,
        &fx.dot,
        &fx.dust,
        pos,
        vel,
        0.0,
        rng.range(6.0, 11.0),
        size,
    );
    commands
        .entity(id)
        .insert((Mote, DespawnOnExit(Phase::Night)));
}
