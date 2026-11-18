//! Ghosts: they come in, browse, queue at the counter and leave again.
//!
//! A ghost's flow:
//! Entering → Browsing → ToCounter → Waiting → AtRegister → Leaving / Ascending
//! (if the atmosphere does not suit them: Entering → Fleeing)

use bevy::prelude::*;

use crate::data::{GameData, rgb};
use crate::day_cycle::{NightClock, NightReport, Phase, Progress};
use crate::rng::Rng;
use crate::shop::{ShopLayout, small_font, sprite_for};
use crate::ui::{GameLog, UiFont};

const SPEED: f32 = 110.0;
const GHOST_SIZE: Vec2 = Vec2::new(40.0, 56.0);

pub struct GhostPlugin;

impl Plugin for GhostPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Queue>()
            .insert_resource(SpawnTimer(2.0))
            .add_systems(
                OnEnter(Phase::Night),
                |mut q: ResMut<Queue>, mut t: ResMut<SpawnTimer>| {
                    q.0.clear();
                    t.0 = 2.0;
                },
            )
            .add_systems(
                Update,
                (spawn_ghosts, ghost_ai, move_ghosts, fade_ghosts)
                    .chain()
                    .run_if(in_state(Phase::Night)),
            );
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GhostState {
    Entering,
    Browsing,
    ToCounter,
    Waiting,
    AtRegister,
    Leaving,
    /// Wish fulfilled and correct change given: the ghost passes into the light.
    Ascending,
    /// The atmosphere does not suit the ghost.
    Fleeing,
}

#[derive(Component)]
pub struct Ghost {
    /// Index in `GameData::ghosts`.
    pub def: usize,
    pub wish: String,
    pub hint: String,
    pub state: GhostState,
    pub pos: Vec2,
    pub target: Vec2,
    pub timer: f32,
    pub patience: f32,
    pub max_patience: f32,
    bob_offset: f32,
}

impl Ghost {
    fn arrived(&self) -> bool {
        self.pos.distance(self.target) < 2.0
    }
}

/// Order at the counter. The first one is served.
#[derive(Resource, Default)]
pub struct Queue(pub Vec<Entity>);

impl Queue {
    pub fn front(&self) -> Option<Entity> {
        self.0.first().copied()
    }

    pub fn remove(&mut self, e: Entity) {
        self.0.retain(|x| *x != e);
    }
}

#[derive(Resource)]
pub struct SpawnTimer(pub f32);

fn spawn_ghosts(
    mut commands: Commands,
    time: Res<Time>,
    data: Res<GameData>,
    layout: Res<ShopLayout>,
    clock: Res<NightClock>,
    progress: Res<Progress>,
    font: Res<UiFont>,
    asset_server: Res<AssetServer>,
    mut timer: ResMut<SpawnTimer>,
    mut rng: ResMut<Rng>,
    mut log: ResMut<GameLog>,
    mut report: ResMut<NightReport>,
    ghosts: Query<(), With<Ghost>>,
) {
    timer.0 -= time.delta_secs();
    // Nobody new arrives in the last 20 seconds.
    if timer.0 > 0.0 || clock.elapsed > clock.length - 20.0 {
        return;
    }
    // Good reputation = more customers.
    let (min, max) = data.config.spawn_interval_secs;
    let rep_factor = (1.0 - progress.reputation as f32 * 0.02).clamp(0.5, 1.5);
    timer.0 = rng.range(min, max) * rep_factor;

    if ghosts.iter().count() >= data.config.max_ghosts_in_shop {
        return;
    }

    let atmosphere = layout.atmosphere(&data);
    let weights: Vec<f32> = data
        .ghosts
        .iter()
        .map(|g| {
            let fits = g.unmet_need(&atmosphere).is_none();
            g.spawn_weight * if fits { 1.0 } else { 0.35 }
        })
        .collect();
    let Some(def_idx) = rng.weighted(&weights) else {
        return;
    };
    let def = &data.ghosts[def_idx];

    let wish = def.wishes[rng.index(def.wishes.len())].clone();
    let product = data.product(&wish).expect("checked in GameData::validate");
    let hint = product.hints[rng.index(product.hints.len())].clone();

    let shelves = layout.shelf_cells(&data);
    let browse_target = if shelves.is_empty() {
        layout.cell_pos(rng.index(layout.cells.len()))
    } else {
        layout.cell_pos(shelves[rng.index(shelves.len())]) + Vec2::new(0.0, -28.0)
    };

    let (state, target) = match def.unmet_need(&atmosphere) {
        Some(need) => {
            log.push(format!(
                "The {} hesitates at the door – {}.",
                def.name,
                need.complaint()
            ));
            report.fled += 1;
            (
                GhostState::Fleeing,
                layout.door_pos() + Vec2::new(70.0, 0.0),
            )
        }
        None => (GhostState::Entering, browse_target),
    };

    let start = layout.door_pos();
    let color = rgb(def.color).with_alpha(0.75);
    let sprite = sprite_for(
        &asset_server,
        &format!("sprites/ghosts/{}.png", def.id),
        color,
        GHOST_SIZE,
    );
    commands.spawn((
        Ghost {
            def: def_idx,
            wish,
            hint,
            state,
            pos: start,
            target,
            timer: 0.0,
            patience: def.patience_secs,
            max_patience: def.patience_secs,
            bob_offset: rng.range(0.0, std::f32::consts::TAU),
        },
        sprite,
        Transform::from_translation(start.extend(5.0)),
        DespawnOnExit(Phase::Night),
        children![(
            Text2d::new(def.name.clone()),
            small_font(&font, 10.0),
            TextColor(Color::srgba(0.9, 0.9, 1.0, 0.8)),
            Transform::from_xyz(0.0, 38.0, 1.0),
        )],
    ));
}

fn ghost_ai(
    mut commands: Commands,
    time: Res<Time>,
    data: Res<GameData>,
    layout: Res<ShopLayout>,
    mut queue: ResMut<Queue>,
    mut progress: ResMut<Progress>,
    mut report: ResMut<NightReport>,
    mut log: ResMut<GameLog>,
    mut rng: ResMut<Rng>,
    mut ghosts: Query<(Entity, &mut Ghost)>,
) {
    let dt = time.delta_secs();
    for (entity, mut g) in &mut ghosts {
        match g.state {
            GhostState::Entering if g.arrived() => {
                g.state = GhostState::Browsing;
                g.timer = rng.range(2.0, 4.5);
            }
            GhostState::Browsing => {
                g.timer -= dt;
                if g.timer <= 0.0 {
                    g.state = GhostState::ToCounter;
                    if !queue.0.contains(&entity) {
                        queue.0.push(entity);
                    }
                }
            }
            GhostState::ToCounter if g.arrived() => {
                g.state = GhostState::Waiting;
            }
            GhostState::Waiting => {
                g.patience -= dt;
                if g.patience <= 0.0 {
                    let name = &data.ghosts[g.def].name;
                    log.push(format!(
                        "The {name} has waited long enough and fades away, grumbling."
                    ));
                    g.state = GhostState::Leaving;
                    queue.remove(entity);
                    progress.reputation -= 1;
                    report.unhappy += 1;
                }
            }
            GhostState::Fleeing if g.arrived() => {
                g.state = GhostState::Leaving;
            }
            GhostState::Leaving => {
                g.target = layout.door_pos();
                if g.arrived() {
                    commands.entity(entity).despawn();
                }
            }
            GhostState::Ascending => {
                g.timer += dt;
                g.target = g.pos + Vec2::Y * 100.0;
                if g.timer > 1.8 {
                    commands.entity(entity).despawn();
                }
            }
            _ => {}
        }
    }

    // Let the queue move up.
    for (place, e) in queue.0.iter().enumerate() {
        if let Ok((_, mut g)) = ghosts.get_mut(*e)
            && matches!(
                g.state,
                GhostState::ToCounter | GhostState::Waiting | GhostState::AtRegister
            )
        {
            g.target = layout.queue_pos(place);
            if g.state == GhostState::Waiting && !g.arrived() {
                g.state = GhostState::ToCounter;
            }
        }
    }
}

fn move_ghosts(time: Res<Time>, mut ghosts: Query<(&mut Ghost, &mut Transform)>) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    for (mut g, mut transform) in &mut ghosts {
        let speed = if g.state == GhostState::Ascending {
            60.0
        } else {
            SPEED
        };
        let pos = g.pos;
        g.pos = pos.move_towards(g.target, speed * dt);
        let bob = (t * 2.2 + g.bob_offset).sin() * 4.0;
        transform.translation = (g.pos + Vec2::new(0.0, bob)).extend(5.0);
    }
}

/// Impatient ghosts grow paler; ascending ones glow and dissolve.
fn fade_ghosts(data: Res<GameData>, mut ghosts: Query<(&Ghost, &mut Sprite)>) {
    for (g, mut sprite) in &mut ghosts {
        let base = rgb(data.ghosts[g.def].color);
        let alpha = match g.state {
            GhostState::Waiting | GhostState::ToCounter => {
                0.3 + 0.5 * (g.patience / g.max_patience).clamp(0.0, 1.0)
            }
            GhostState::Ascending => (1.0 - g.timer / 1.8).clamp(0.0, 1.0),
            _ => 0.8,
        };
        let color = if g.state == GhostState::Ascending {
            Color::srgb(1.0, 0.97, 0.8)
        } else {
            base
        };
        // With real images only change the transparency, not the colour.
        sprite.color = if sprite.image == Handle::default() {
            color.with_alpha(alpha)
        } else {
            Color::WHITE.with_alpha(alpha)
        };
    }
}
