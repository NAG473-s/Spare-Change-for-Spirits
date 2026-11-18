//! Visual atmosphere: flickering light around candles and lamps, a shade
//! that darkens the shop at night, and particles. Also defines `Cue`, the
//! message gameplay sends for sounds and effects.

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::day_cycle::Phase;
use crate::rng::Rng;
use crate::shop::{CELL, ShopLayout};

/// Something happened that should be heard or seen.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub enum Cue {
    GhostEnters,
    Paid,
    ChangeGiven,
    /// A ghost passes into the light at this position.
    Ascend(Vec2),
    /// Something was knocked off a shelf at this position.
    Knock(Vec2),
    Disappointed,
    Click,
}

/// World layers (z) of the effects.
pub const Z_SHADE: f32 = 4.0;
pub const Z_GLOW: f32 = 4.5;
pub const Z_PARTICLES: f32 = 6.0;

pub struct FxPlugin;

impl Plugin for FxPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Cue>()
            .init_resource::<GlowTexture>()
            .add_systems(Startup, spawn_shade)
            .add_systems(
                Update,
                (
                    flicker,
                    fade_shade,
                    cue_particles,
                    move_particles,
                    dust_motes.run_if(in_state(Phase::Night)),
                ),
            );
    }
}

/// A soft white radial gradient, tinted per use.
#[derive(Resource)]
pub struct GlowTexture(pub Handle<Image>);

impl FromWorld for GlowTexture {
    fn from_world(world: &mut World) -> Self {
        const SIZE: u32 = 128;
        let mut data = Vec::with_capacity((SIZE * SIZE * 4) as usize);
        for y in 0..SIZE {
            for x in 0..SIZE {
                let dx = (x as f32 + 0.5) / SIZE as f32 * 2.0 - 1.0;
                let dy = (y as f32 + 0.5) / SIZE as f32 * 2.0 - 1.0;
                let d = (dx * dx + dy * dy).sqrt().min(1.0);
                let a = (1.0 - d).powi(2);
                data.extend_from_slice(&[255, 255, 255, (a * 255.0) as u8]);
            }
        }
        let image = Image::new(
            Extent3d {
                width: SIZE,
                height: SIZE,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            data,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD,
        );
        Self(world.resource_mut::<Assets<Image>>().add(image))
    }
}

/// Soft light around a light-giving decor item.
#[derive(Component)]
pub struct Glow {
    base_alpha: f32,
    phase: f32,
}

/// Light glow for a decor item with the given light value; `phase` keeps
/// neighbouring candles from flickering in sync.
pub fn glow(texture: &GlowTexture, light: i32, phase: f32) -> impl Bundle {
    let light = light.max(1) as f32;
    let base_alpha = (0.1 + 0.06 * light).min(0.4);
    (
        Sprite {
            image: texture.0.clone(),
            custom_size: Some(Vec2::splat(CELL * (1.2 + 0.45 * light))),
            color: Color::srgba(1.0, 0.78, 0.4, base_alpha),
            ..default()
        },
        Glow { base_alpha, phase },
        Transform::from_xyz(0.0, 0.0, Z_GLOW),
        Pickable::IGNORE,
    )
}

fn flicker(time: Res<Time<Real>>, mut glows: Query<(&Glow, &mut Sprite)>) {
    let t = time.elapsed_secs();
    for (g, mut sprite) in &mut glows {
        let wobble =
            0.82 + 0.11 * (t * 7.3 + g.phase).sin() + 0.07 * (t * 13.1 + g.phase * 2.3).sin();
        sprite.color.set_alpha(g.base_alpha * wobble);
    }
}

/// Darkens the shop (not the UI) depending on the time of day.
#[derive(Component)]
struct Shade;

fn spawn_shade(mut commands: Commands) {
    commands.spawn((
        Shade,
        Sprite::from_color(Color::srgba(0.02, 0.03, 0.1, 0.0), Vec2::new(1400.0, 800.0)),
        Transform::from_xyz(0.0, 0.0, Z_SHADE),
        Pickable::IGNORE,
    ));
}

fn fade_shade(
    time: Res<Time<Real>>,
    phase: Res<State<Phase>>,
    mut shade: Single<&mut Sprite, With<Shade>>,
) {
    let target = match phase.get() {
        Phase::Menu => 0.0,
        Phase::Evening => 0.12,
        Phase::Night => 0.4,
        Phase::Morning => 0.05,
        Phase::DemoEnd => 0.15,
    };
    let current = shade.color.alpha();
    let step = time.delta_secs() * 0.5;
    let next = current + (target - current).clamp(-step, step);
    shade.color.set_alpha(next);
}

#[derive(Component)]
struct Particle {
    vel: Vec2,
    /// Upward pull (negative gravity) or fall (positive).
    gravity: f32,
    life: f32,
    max_life: f32,
    alpha: f32,
}

#[derive(Component)]
struct Mote;

fn spawn_particle(
    commands: &mut Commands,
    sprite: Sprite,
    pos: Vec2,
    vel: Vec2,
    gravity: f32,
    life: f32,
) -> Entity {
    let alpha = sprite.color.alpha();
    commands
        .spawn((
            sprite,
            Transform::from_translation(pos.extend(Z_PARTICLES)),
            Particle {
                vel,
                gravity,
                life,
                max_life: life,
                alpha,
            },
            Pickable::IGNORE,
        ))
        .id()
}

fn cue_particles(
    mut commands: Commands,
    mut cues: MessageReader<Cue>,
    texture: Res<GlowTexture>,
    mut rng: ResMut<Rng>,
) {
    for cue in cues.read() {
        match *cue {
            Cue::Ascend(pos) => {
                for _ in 0..18 {
                    let size = rng.range(5.0, 11.0);
                    let sprite = Sprite {
                        image: texture.0.clone(),
                        custom_size: Some(Vec2::splat(size)),
                        color: Color::srgba(1.0, 0.95, 0.75, 0.9),
                        ..default()
                    };
                    let offset = Vec2::new(rng.range(-18.0, 18.0), rng.range(-24.0, 24.0));
                    let vel = Vec2::new(rng.range(-15.0, 15.0), rng.range(20.0, 60.0));
                    let life = rng.range(1.2, 2.2);
                    spawn_particle(&mut commands, sprite, pos + offset, vel, -25.0, life);
                }
            }
            Cue::Knock(pos) => {
                for _ in 0..10 {
                    let shade = rng.range(0.6, 0.9);
                    let sprite = Sprite::from_color(
                        Color::srgba(shade, shade, 1.0, 0.9),
                        Vec2::splat(rng.range(2.0, 4.0)),
                    );
                    let angle = rng.range(0.3, std::f32::consts::PI - 0.3);
                    let speed = rng.range(60.0, 150.0);
                    let vel = Vec2::new(angle.cos(), angle.sin()) * speed;
                    spawn_particle(&mut commands, sprite, pos, vel, 320.0, rng.range(0.4, 0.7));
                }
            }
            _ => {}
        }
    }
}

/// Particles follow the game clock, so they freeze while paused.
fn move_particles(
    mut commands: Commands,
    time: Res<Time>,
    mut particles: Query<(Entity, &mut Particle, &mut Transform, &mut Sprite)>,
) {
    let dt = time.delta_secs();
    for (entity, mut p, mut transform, mut sprite) in &mut particles {
        p.life -= dt;
        if p.life <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        p.vel.y -= p.gravity * dt;
        transform.translation += (p.vel * dt).extend(0.0);
        // Fade in quickly, fade out slowly.
        let age = 1.0 - p.life / p.max_life;
        let fade = (age * 8.0).min(1.0) * (p.life / p.max_life).sqrt();
        sprite.color.set_alpha(p.alpha * fade);
    }
}

/// Faint dust drifting through the shop at night.
fn dust_motes(
    mut commands: Commands,
    layout: Res<ShopLayout>,
    texture: Res<GlowTexture>,
    mut rng: ResMut<Rng>,
    motes: Query<(), With<Mote>>,
) {
    if motes.iter().count() >= 24 || !rng.chance(0.2) {
        return;
    }
    let min = layout.cell_pos(0) - Vec2::splat(CELL / 2.0);
    let max = layout.cell_pos(layout.cells.len() - 1) + Vec2::splat(CELL / 2.0);
    let pos = Vec2::new(rng.range(min.x, max.x), rng.range(min.y, max.y));
    let sprite = Sprite {
        image: texture.0.clone(),
        custom_size: Some(Vec2::splat(rng.range(3.0, 6.0))),
        color: Color::srgba(0.85, 0.85, 1.0, 0.35),
        ..default()
    };
    let vel = Vec2::new(rng.range(-6.0, 6.0), rng.range(-3.0, 5.0));
    let id = spawn_particle(&mut commands, sprite, pos, vel, 0.0, rng.range(6.0, 11.0));
    commands
        .entity(id)
        .insert((Mote, DespawnOnExit(Phase::Night)));
}
