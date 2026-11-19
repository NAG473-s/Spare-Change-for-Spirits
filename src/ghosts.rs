//! Ghosts: they come in, browse, queue at the counter and leave again.
//!
//! A ghost's flow:
//! Entering → Browsing → ToCounter → Waiting → AtRegister → Leaving / Ascending
//! (if the atmosphere does not suit them: Entering → Fleeing)

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::data::{GameData, GhostDef, rgb};
use crate::day_cycle::{NightClock, NightReport, Phase, Progress};
use crate::fx::Cue;
use crate::i18n::Lang;
use crate::inventory::Stock;
use crate::rng::Rng;
use bevy::light::NotShadowCaster;

use crate::scene::{Billboard, billboard_material, billboard_mesh, floor, world_label};
use crate::shop::ShopLayout;
use crate::ui::{GameLog, UiFont};

/// Floating speed in metres per second.
const SPEED: f32 = 1.7;
/// Ghosts float this high above the floor.
const HOVER: f32 = 0.25;
/// Drawn size of a ghost (2:3, like the generated portrait images).
const GHOST_SIZE: Vec2 = Vec2::new(0.9, 1.35);

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
                (
                    spawn_ghosts,
                    spawn_story_ghost,
                    ghost_ai,
                    move_ghosts,
                    fade_ghosts,
                )
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
    /// Catalog key of what the ghost whispers (a product hint or a story intro).
    pub hint: String,
    pub state: GhostState,
    pub pos: Vec2,
    pub target: Vec2,
    pub timer: f32,
    pub patience: f32,
    pub max_patience: f32,
    /// Height gained while passing into the light.
    lift: f32,
    /// Story chapter this visit belongs to (story ghost only).
    pub chapter: Option<usize>,
    /// Shelf cell the ghost browses, if any.
    browse_cell: Option<usize>,
    bob_offset: f32,
}

impl Ghost {
    fn arrived(&self) -> bool {
        self.pos.distance(self.target) < 0.03
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
    lang: Res<Lang>,
    mut art: GhostArt,
    mut timer: ResMut<SpawnTimer>,
    mut rng: ResMut<Rng>,
    mut log: ResMut<GameLog>,
    mut cues: MessageWriter<Cue>,
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
    let hints = lang.count(&format!("product.{wish}.hint."));
    let hint = format!("product.{wish}.hint.{}", rng.index(hints) + 1);

    let unmet = def.unmet_need(&atmosphere);
    if let Some(need) = unmet {
        log.push(lang.f(
            "log.hesitates",
            &[
                ("who", &lang.ghost_the(&def.id)),
                ("reason", &need.complaint(&lang)),
            ],
        ));
        report.fled += 1;
    }
    let visit = Visit {
        def_idx,
        wish,
        hint,
        chapter: None,
        flees: unmet.is_some(),
    };
    spawn_ghost(
        &mut commands,
        &data,
        &layout,
        &art.font,
        &lang,
        &art.asset_server,
        &mut art.meshes,
        &mut art.materials,
        &mut rng,
        visit,
    );
    cues.write(Cue::GhostEnters);
}

/// Everything needed to draw a new ghost.
#[derive(SystemParam)]
struct GhostArt<'w> {
    font: Res<'w, UiFont>,
    asset_server: Res<'w, AssetServer>,
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<StandardMaterial>>,
}

/// What a newly arriving ghost comes for.
struct Visit {
    def_idx: usize,
    wish: String,
    hint: String,
    chapter: Option<usize>,
    flees: bool,
}

fn spawn_ghost(
    commands: &mut Commands,
    data: &GameData,
    layout: &ShopLayout,
    font: &UiFont,
    lang: &Lang,
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    rng: &mut Rng,
    visit: Visit,
) {
    let def: &GhostDef = &data.ghosts[visit.def_idx];
    let shelves = layout.shelf_cells(data);
    let browse_cell = (!shelves.is_empty()).then(|| shelves[rng.index(shelves.len())]);
    let (state, target) = if visit.flees {
        (GhostState::Fleeing, layout.door_pos() + Vec2::new(1.0, 0.0))
    } else {
        let target = match browse_cell {
            // In front of the shelf, on the camera side.
            Some(cell) => layout.cell_pos(cell) + Vec2::new(0.0, 0.7),
            None => layout.cell_pos(rng.index(layout.cells.len())),
        };
        (GhostState::Entering, target)
    };

    let start = layout.door_pos();
    let material = billboard_material(
        materials,
        asset_server,
        &format!("sprites/ghosts/{}.png", def.id),
        rgb(def.color).with_alpha(0.75),
    );
    let ghost = commands
        .spawn((
            Ghost {
                def: visit.def_idx,
                wish: visit.wish,
                hint: visit.hint,
                state,
                pos: start,
                target,
                timer: 0.0,
                patience: def.patience_secs,
                max_patience: def.patience_secs,
                lift: 0.0,
                chapter: visit.chapter,
                browse_cell,
                bob_offset: rng.range(0.0, std::f32::consts::TAU),
            },
            Mesh3d(billboard_mesh(meshes, GHOST_SIZE)),
            MeshMaterial3d(material),
            Billboard,
            NotShadowCaster,
            Pickable::IGNORE,
            Transform::from_translation(floor(start, HOVER)),
            DespawnOnExit(Phase::Night),
        ))
        .id();
    commands.spawn(world_label(
        font,
        lang.ghost(&def.id),
        10.0,
        ghost,
        Vec3::new(0.0, GHOST_SIZE.y + 0.15, 0.0),
    ));
}

/// The story ghost arrives once per night until his story is finished.
fn spawn_story_ghost(
    mut commands: Commands,
    data: Res<GameData>,
    layout: Res<ShopLayout>,
    clock: Res<NightClock>,
    progress: Res<Progress>,
    lang: Res<Lang>,
    mut art: GhostArt,
    mut rng: ResMut<Rng>,
    mut report: ResMut<NightReport>,
    mut log: ResMut<GameLog>,
    mut cues: MessageWriter<Cue>,
) {
    let story = &data.story;
    let chapter = progress.story_chapter;
    if report.story_arrived
        || chapter >= story.chapters.len()
        || clock.elapsed < story.arrive_at * clock.length
    {
        return;
    }
    let Some(def_idx) = data.ghost_index(&story.ghost) else {
        return;
    };
    report.story_arrived = true;
    let c = &story.chapters[chapter];
    log.push(lang.t("story.arrives"));
    let visit = Visit {
        def_idx,
        wish: c.wish.clone(),
        hint: format!("story.{}.intro", chapter + 1),
        chapter: Some(chapter),
        flees: false,
    };
    spawn_ghost(
        &mut commands,
        &data,
        &layout,
        &art.font,
        &lang,
        &art.asset_server,
        &mut art.meshes,
        &mut art.materials,
        &mut rng,
        visit,
    );
    cues.write(Cue::GhostEnters);
}

fn ghost_ai(
    mut commands: Commands,
    time: Res<Time>,
    data: Res<GameData>,
    layout: Res<ShopLayout>,
    mut stock: ResMut<Stock>,
    mut queue: ResMut<Queue>,
    mut progress: ResMut<Progress>,
    mut report: ResMut<NightReport>,
    mut log: ResMut<GameLog>,
    mut cues: MessageWriter<Cue>,
    mut rng: ResMut<Rng>,
    mut ghosts: Query<(Entity, &mut Ghost)>,
    lang: Res<Lang>,
) {
    let dt = time.delta_secs();
    for (entity, mut g) in &mut ghosts {
        match g.state {
            GhostState::Entering if g.arrived() => {
                g.state = GhostState::Browsing;
                g.timer = rng.range(2.0, 4.5);
                let quirks = &data.ghosts[g.def].quirks;
                let calm = quirks
                    .calm_with
                    .is_some_and(|n| n.is_met(&layout.atmosphere(&data)));
                if !calm
                    && rng.chance(quirks.knock_chance)
                    && let Some(product) =
                        g.browse_cell.and_then(|c| layout.cells[c].product.clone())
                    && stock.take_one(&product)
                {
                    log.push(lang.f(
                        "log.knock",
                        &[
                            ("who", &lang.ghost_the(&data.ghosts[g.def].id)),
                            ("item", &lang.product(&product)),
                        ],
                    ));
                    if let Some(cell) = g.browse_cell {
                        cues.write(Cue::Knock(layout.cell_pos(cell)));
                    }
                    report.knocked.push(product);
                }
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
                    let who = lang.ghost_the(&data.ghosts[g.def].id);
                    log.push(lang.f("log.waited_too_long", &[("who", &who)]));
                    g.state = GhostState::Leaving;
                    queue.remove(entity);
                    progress.reputation -= 1;
                    report.unhappy += 1;
                    cues.write(Cue::Disappointed);
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
                g.lift += dt * 0.9;
                if g.timer > 1.8 {
                    commands.entity(entity).despawn();
                }
            }
            _ => {}
        }
    }

    // Let the queue move up.
    let len = queue.0.len();
    for (place, e) in queue.0.iter().enumerate() {
        if let Ok((_, mut g)) = ghosts.get_mut(*e)
            && matches!(
                g.state,
                GhostState::ToCounter | GhostState::Waiting | GhostState::AtRegister
            )
        {
            g.target = layout.queue_pos(place, len);
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
        let pos = g.pos;
        g.pos = pos.move_towards(g.target, SPEED * dt);
        let bob = (t * 2.2 + g.bob_offset).sin() * 0.06;
        transform.translation = floor(g.pos, HOVER + g.lift + bob);
    }
}

/// Impatient ghosts grow paler; ascending ones glow and dissolve.
fn fade_ghosts(
    data: Res<GameData>,
    ghosts: Query<(&Ghost, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (g, handle) in &ghosts {
        let Some(mut material) = materials.get_mut(&handle.0) else {
            continue;
        };
        let alpha = match g.state {
            GhostState::Waiting | GhostState::ToCounter => {
                0.35 + 0.55 * (g.patience / g.max_patience).clamp(0.0, 1.0)
            }
            GhostState::Ascending => (1.0 - g.timer / 1.8).clamp(0.0, 1.0),
            _ => 0.9,
        };
        let color = if material.base_color_texture.is_some() {
            Color::WHITE
        } else if g.state == GhostState::Ascending {
            Color::srgb(1.0, 0.97, 0.8)
        } else {
            rgb(data.ghosts[g.def].color)
        };
        let new = color.with_alpha(alpha);
        // Only touch the asset when it changes, so it is not re-uploaded every frame.
        if material.base_color != new {
            material.base_color = new;
        }
    }
}
