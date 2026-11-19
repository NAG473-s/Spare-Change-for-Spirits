//! Ghosts: they come in, browse, queue at the counter and leave again.
//!
//! A ghost's flow:
//! Entering → Browsing → ToCounter → Waiting → AtRegister → Leaving / Ascending
//! (if the atmosphere does not suit them: Entering → Fleeing)

mod motion;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::data::{GameData, GhostDef, asset_exists, rgb};
use crate::day_cycle::{NightClock, NightReport, Phase, Progress};
use crate::fx::{Cue, Voice};
use crate::i18n::Lang;
use crate::inventory::Stock;
use crate::rng::Rng;
use crate::save::Settings;
use crate::tooltip::{HoverTip, TipKind};
use crate::upgrades::Upgrades;
use bevy::light::NotShadowCaster;

use crate::scene::{Billboard, LabelMuted, billboard_material, floor, world_label};
use crate::shop::ShopLayout;
use crate::ui::{GameLog, UiFont};

/// Floating speed in metres per second.
const SPEED: f32 = 1.7;
/// Ghosts float this high above the floor.
const HOVER: f32 = 0.25;
/// Drawn size of a ghost (2:3, like the generated portrait images).
const GHOST_SIZE: Vec2 = Vec2::new(1.1, 1.65);

pub struct GhostPlugin;

impl Plugin for GhostPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Queue>()
            .init_resource::<HoveredGhost>()
            .insert_resource(SpawnTimer(2.0))
            .add_systems(
                OnEnter(Phase::Night),
                |mut q: ResMut<Queue>, mut t: ResMut<SpawnTimer>, mut h: ResMut<HoveredGhost>| {
                    h.0 = None;
                    q.0.clear();
                    t.0 = 2.0;
                },
            )
            // The ghosts are gone: so is a ghost tooltip.
            .add_systems(OnExit(Phase::Night), |mut tip: ResMut<HoverTip>| {
                HoverTip::hide(&mut tip)
            })
            .add_systems(
                Update,
                (
                    spawn_ghosts,
                    spawn_story_ghost,
                    ghost_ai,
                    ghost_murmurs,
                    ghost_tooltip,
                    mute_browsing_names,
                    move_ghosts,
                    fade_ghosts,
                    animate_ghosts,
                    motion::animate,
                    play_sheets,
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
    /// Current floating velocity (x, z), for leaning and facing.
    vel: Vec2,
    /// Smoothed facing: 1 = as drawn, -1 = mirrored.
    facing: f32,
    /// Seconds since the ghost appeared.
    age: f32,
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
    upgrades: Res<Upgrades>,
) {
    timer.0 -= time.delta_secs();
    // Nobody new arrives in the last 20 seconds.
    if timer.0 > 0.0 || clock.elapsed > clock.length - 20.0 {
        return;
    }
    // Good reputation = more customers.
    let (min, max) = data.config.spawn_interval_secs;
    let rep_factor = data.config.reputation_spawn_factor(progress.reputation);
    let night = progress.night;
    timer.0 =
        rng.range(min, max) * rep_factor * data.spawn_factor(night) * upgrades.spawn_factor(&data);

    if ghosts.iter().count() >= data.config.max_ghosts_in_shop {
        return;
    }

    let atmosphere = layout.atmosphere(&data, night);
    let weights: Vec<f32> = data
        .ghosts
        .iter()
        .map(|g| {
            if g.unlock_night > night {
                return 0.0;
            }
            let fits = g.unmet_need(&atmosphere).is_none();
            g.spawn_weight * if fits { 1.0 } else { 0.35 } * data.ghost_weight_factor(night, &g.id)
        })
        .collect();
    let Some(def_idx) = rng.weighted(&weights) else {
        return;
    };
    let def = &data.ghosts[def_idx];

    // Only wish for what the ferryman already sells.
    let wishes: Vec<&String> = def
        .wishes
        .iter()
        .filter(|w| data.product(w).is_some_and(|p| p.unlock_night <= night))
        .collect();
    let wish = wishes[rng.index(wishes.len())].clone();
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
        patience_factor: upgrades.patience_factor(&data),
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
    let voice = data.ghosts[def_idx].voice;
    if unmet.is_some() {
        cues.write(Cue::GhostVoice(Voice::Flee, voice));
    } else if rng.chance(ENTER_VOICE_CHANCE) {
        cues.write(Cue::GhostVoice(Voice::Whoo, voice));
    }
}

/// Chance that a ghost says "wooo" as it comes in.
const ENTER_VOICE_CHANCE: f32 = 0.5;
/// Seconds between two murmurs from the ghosts browsing the shelves.
const MURMUR_SECS: (f32, f32) = (7.0, 15.0);
/// Share of murmurs that are a "wooo" instead of a hum.
const MURMUR_WHOO: f32 = 0.3;

/// Now and then one of the browsing ghosts hums or sighs to itself.
fn ghost_murmurs(
    time: Res<Time>,
    data: Res<GameData>,
    ghosts: Query<&Ghost>,
    mut rng: ResMut<Rng>,
    mut cues: MessageWriter<Cue>,
    mut wait: Local<f32>,
) {
    *wait -= time.delta_secs();
    if *wait > 0.0 {
        return;
    }
    *wait = rng.range(MURMUR_SECS.0, MURMUR_SECS.1);
    let browsing: Vec<&Ghost> = ghosts
        .iter()
        .filter(|g| g.state == GhostState::Browsing)
        .collect();
    if browsing.is_empty() {
        return;
    }
    let ghost = browsing[rng.index(browsing.len())];
    let kind = if rng.chance(MURMUR_WHOO) {
        Voice::Whoo
    } else {
        Voice::Hum
    };
    cues.write(Cue::GhostVoice(kind, data.ghosts[ghost.def].voice));
}

/// The ghost under the mouse pointer.
#[derive(Resource, Default)]
struct HoveredGhost(Option<Entity>);

fn on_ghost_over(over: On<PointerOver>, mut hovered: ResMut<HoveredGhost>) {
    hovered.0 = Some(over.event_target());
}

fn on_ghost_out(out: On<PointerOut>, mut hovered: ResMut<HoveredGhost>) {
    if hovered.0 == Some(out.event_target()) {
        hovered.0 = None;
    }
}

/// How a ghost's patience reads in its tooltip.
const PATIENCE_CALM: f32 = 0.6;
const PATIENCE_IMPATIENT: f32 = 0.3;

/// Hovering a ghost tells who it is, how it feels, what it needs and – at
/// the front of the counter – what it whispers. Products on the shelves
/// have their own tooltip, which wins.
#[allow(clippy::too_many_arguments)]
fn ghost_tooltip(
    hovered: Res<HoveredGhost>,
    ghosts: Query<&Ghost>,
    queue: Res<Queue>,
    data: Res<GameData>,
    lang: Res<Lang>,
    settings: Res<Settings>,
    asset_server: Res<AssetServer>,
    mut tip: ResMut<HoverTip>,
    mut shown: Local<Option<String>>,
) {
    // Another tooltip (a product on a shelf) took over: leave it alone.
    if shown.as_ref().is_some_and(|title| *title != tip.title) {
        *shown = None;
    }
    let ghost = hovered.0.and_then(|e| ghosts.get(e).ok().map(|g| (e, g)));
    let Some((entity, g)) = ghost.filter(|_| shown.is_some() || tip.is_empty()) else {
        if shown.take().is_some() {
            HoverTip::hide(&mut tip);
        }
        return;
    };
    let def = &data.ghosts[g.def];
    let portrait = format!("sprites/ghosts/{}.png", def.id);
    let mut t = HoverTip {
        icon: asset_exists(&portrait).then(|| asset_server.load(portrait)),
        title: lang.ghost(&def.id),
        lines: Vec::new(),
    };
    let patience = (g.patience / g.max_patience.max(0.01)).clamp(0.0, 1.0);
    let (kind, mood) = match g.state {
        GhostState::Entering | GhostState::Browsing => {
            (TipKind::Text, lang.t("ghost_tip.browsing"))
        }
        GhostState::ToCounter => (TipKind::Text, lang.t("ghost_tip.coming")),
        GhostState::Waiting if settings.patient_ghosts => {
            (TipKind::Good, lang.t("ghost_tip.patient"))
        }
        GhostState::Waiting if patience > PATIENCE_CALM => {
            (TipKind::Good, lang.t("ghost_tip.calm"))
        }
        GhostState::Waiting if patience > PATIENCE_IMPATIENT => {
            (TipKind::Text, lang.t("ghost_tip.impatient"))
        }
        GhostState::Waiting => (TipKind::Bad, lang.t("ghost_tip.leaving_soon")),
        GhostState::AtRegister => (TipKind::Text, lang.t("ghost_tip.paying")),
        GhostState::Fleeing => (TipKind::Bad, lang.t("ghost_tip.flees")),
        GhostState::Leaving => (TipKind::Text, lang.t("ghost_tip.leaving")),
        GhostState::Ascending => (TipKind::Good, lang.t("ghost_tip.ascending")),
    };
    t.line(kind, mood);
    let at_counter = matches!(g.state, GhostState::Waiting | GhostState::AtRegister);
    if at_counter && queue.front() == Some(entity) {
        t.line(
            TipKind::Text,
            lang.f(
                "ghost_tip.whispers",
                &[("quote", &lang.f("quote", &[("text", &lang.t(&g.hint))]))],
            ),
        );
    } else if !matches!(
        g.state,
        GhostState::Fleeing | GhostState::Leaving | GhostState::Ascending
    ) {
        t.line(TipKind::Hint, lang.t("ghost_tip.wish_later"));
    }
    for need in &def.needs {
        t.line(TipKind::Text, need.describe(&lang));
    }
    if !def.pays_with.is_empty() {
        let coins: Vec<String> = def.pays_with.iter().map(|c| c.name(&lang)).collect();
        t.line(
            TipKind::Text,
            lang.f("ghost_tip.pays_with", &[("coins", &lang.join(&coins))]),
        );
    }
    let percent = ((def.pay_multiplier - 1.0) * 100.0).round() as i32;
    if percent > 0 {
        t.line(
            TipKind::Good,
            lang.f("ghost_tip.pays_more", &[("percent", &percent)]),
        );
    } else if percent < 0 {
        t.line(
            TipKind::Bad,
            lang.f("ghost_tip.pays_less", &[("percent", &-percent)]),
        );
    }
    let quirks = &def.quirks;
    for (chance, key) in [
        (quirks.knock_chance, "ghost_tip.knocks"),
        (quirks.counterfeit_chance, "ghost_tip.counterfeit"),
        (quirks.double_pay_chance, "ghost_tip.double_pay"),
    ] {
        if chance > 0.0 {
            t.line(TipKind::Hint, lang.t(key));
        }
    }
    *shown = Some(t.title.clone());
    HoverTip::show(&mut tip, t);
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
    /// The waiting bench: ghosts wait longer.
    patience_factor: f32,
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
    // An animated sprite sheet wins over the single picture.
    let sheet_path = format!("sprites/ghosts/{}_sheet.png", def.id);
    let sheet = sheet_frames(&sheet_path);
    let image = if sheet.is_some() {
        sheet_path
    } else {
        format!("sprites/ghosts/{}.png", def.id)
    };
    let material = billboard_material(
        materials,
        asset_server,
        &image,
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
                patience: def.patience_secs * visit.patience_factor,
                max_patience: def.patience_secs * visit.patience_factor,
                lift: 0.0,
                vel: Vec2::ZERO,
                facing: 1.0,
                age: 0.0,
                chapter: visit.chapter,
                browse_cell,
                bob_offset: rng.range(0.0, std::f32::consts::TAU),
            },
            Billboard,
            Transform::from_translation(floor(start, HOVER)),
            Visibility::default(),
            DespawnOnExit(Phase::Night),
        ))
        .id();
    // The visible body is a child, so it can sway and squash while the
    // parent keeps facing the camera.
    let body = commands
        .spawn((
            GhostBody,
            motion::Rig::new(&def.id, def.voice),
            Mesh3d(meshes.add(motion::mesh())),
            MeshMaterial3d(material),
            NotShadowCaster,
            Pickable::IGNORE,
            Transform::default(),
        ))
        .id();
    if let Some(frames) = sheet {
        commands.entity(body).insert(SheetAnim {
            frames,
            fps: SHEET_FPS,
            shown: u32::MAX,
        });
    }
    commands.entity(ghost).add_child(body);
    // An invisible box over the body for the tooltip. It does not block the
    // pointer, so the shelf items behind a ghost stay clickable.
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(GHOST_SIZE.x * 0.7, GHOST_SIZE.y * 0.9, 0.05))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::NONE,
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        })),
        Transform::from_xyz(0.0, GHOST_SIZE.y * 0.45, 0.0),
        NotShadowCaster,
        Pickable {
            should_block_lower: false,
            is_hoverable: true,
        },
        ChildOf(ghost),
    ));
    commands
        .entity(ghost)
        .observe(on_ghost_over)
        .observe(on_ghost_out);
    let label = commands
        .spawn(world_label(
            font,
            lang.ghost(&def.id),
            10.0,
            ghost,
            Vec3::new(0.0, GHOST_SIZE.y + 0.15, 0.0),
        ))
        .id();
    commands.entity(ghost).insert(GhostLabel(label));
}

/// The name label of a ghost.
#[derive(Component)]
struct GhostLabel(Entity);

/// Names show on the way to the counter and in the line; while a ghost
/// browses it stands right in front of a shelf, and its name would cover
/// the shelf's own label (hovering it still shows the name).
fn mute_browsing_names(
    mut commands: Commands,
    ghosts: Query<(&Ghost, &GhostLabel), Changed<Ghost>>,
    muted: Query<(), With<LabelMuted>>,
) {
    for (ghost, GhostLabel(label)) in &ghosts {
        let quiet = matches!(ghost.state, GhostState::Entering | GhostState::Browsing);
        let is_muted = muted.get(*label).is_ok();
        if quiet && !is_muted {
            commands.entity(*label).try_insert(LabelMuted);
        } else if !quiet && is_muted {
            commands.entity(*label).try_remove::<LabelMuted>();
        }
    }
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
    upgrades: Res<Upgrades>,
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
        patience_factor: upgrades.patience_factor(&data),
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
    settings: Res<crate::save::Settings>,
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
                    .is_some_and(|n| n.is_met(&layout.atmosphere(&data, progress.night)));
                if !calm
                    && rng.chance(quirks.knock_chance)
                    && let Some(product) = g.browse_cell.and_then(|c| {
                        let on_shelf = &layout.cells[c].products;
                        (!on_shelf.is_empty()).then(|| on_shelf[rng.index(on_shelf.len())].clone())
                    })
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
                // At the front of the line it whispers what it is looking for.
                if queue.front() == Some(entity) {
                    cues.write(Cue::GhostVoice(Voice::Whisper, data.ghosts[g.def].voice));
                }
            }
            GhostState::Waiting => {
                // Assist setting: patient ghosts never give up.
                if !settings.patient_ghosts {
                    g.patience -= dt;
                }
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

/// The visible part of a ghost (child of the entity with [`Ghost`]).
#[derive(Component)]
pub struct GhostBody;

/// A ghost sprite sheet: frames side by side, played as a loop.
#[derive(Component)]
struct SheetAnim {
    frames: u32,
    fps: f32,
    /// Frame currently set on the material.
    shown: u32,
}

const SHEET_FPS: f32 = 8.0;

/// Number of frames in a sheet: frames are 2:3 like the single sprites and
/// placed side by side, so the count follows from the image size.
fn sheet_frames(path: &str) -> Option<u32> {
    let file = crate::data::assets_dir().join(path);
    let (w, h) = png_size(&std::fs::read(file).ok()?)?;
    let frames = (w as f32 / (h as f32 * GHOST_SIZE.x / GHOST_SIZE.y)).round() as u32;
    (frames >= 2).then_some(frames)
}

/// Width and height from a PNG header.
fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" {
        return None;
    }
    let w = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    Some((w, h))
}

fn move_ghosts(time: Res<Time>, mut ghosts: Query<(&mut Ghost, &mut Transform)>) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    for (mut g, mut transform) in &mut ghosts {
        let pos = g.pos;
        g.pos = pos.move_towards(g.target, SPEED * dt);
        g.vel = if dt > 0.0 {
            (g.pos - pos) / dt
        } else {
            Vec2::ZERO
        };
        g.age += dt;
        let bob = (t * 2.2 + g.bob_offset).sin() * 0.06;
        transform.translation = floor(g.pos, HOVER + g.lift + bob);
    }
}

/// Makes the body alive: breathing, swaying, leaning into the movement,
/// turning towards where it floats, trembling when impatient, popping in
/// on arrival and stretching while passing into the light.
fn animate_ghosts(
    time: Res<Time>,
    settings: Res<Settings>,
    mut ghosts: Query<&mut Ghost>,
    mut bodies: Query<(&ChildOf, &mut Transform), With<GhostBody>>,
) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    for (child_of, mut body) in &mut bodies {
        let Ok(mut g) = ghosts.get_mut(child_of.parent()) else {
            continue;
        };
        // Turn towards the direction of travel; the smoothed flip reads as a squash.
        let wanted = if g.vel.x < -0.05 {
            -1.0
        } else if g.vel.x > 0.05 {
            1.0
        } else {
            g.facing
        };
        let turn = (wanted - g.facing).clamp(-dt * 6.0, dt * 6.0);
        g.facing += turn;

        let phase = g.bob_offset;
        let breathe = (t * 1.6 + phase).sin() * 0.04;
        let mut roll = (t * 1.1 + phase).sin() * 0.05 - (g.vel.x * 0.12).clamp(-0.2, 0.2);
        if g.state == GhostState::Waiting && !settings.patient_ghosts {
            let worry = 1.0 - (g.patience / g.max_patience).clamp(0.0, 1.0);
            if worry > 0.65 {
                roll += (t * 25.0).sin() * 0.06 * (worry - 0.65) / 0.35;
            }
        }
        // Pop in when appearing (ease-out from 60 % size).
        let appear = (g.age / 0.4).clamp(0.0, 1.0);
        let pop = 0.6 + 0.4 * (1.0 - (1.0 - appear).powi(3));
        let (stretch, spin) = if g.state == GhostState::Ascending {
            (1.0 + g.timer * 0.35, g.timer * 5.0)
        } else {
            (1.0, 0.0)
        };
        let flip = g.facing.signum() * g.facing.abs().max(0.15);
        body.rotation = Quat::from_rotation_y(spin) * Quat::from_rotation_z(roll);
        body.scale = Vec3::new(
            pop * (1.0 - breathe * 0.75) / stretch.sqrt() * flip,
            pop * (1.0 + breathe) * stretch,
            1.0,
        );
    }
}

/// Steps through the frames of animated sprite sheets.
fn play_sheets(
    time: Res<Time>,
    mut sheets: Query<(&mut SheetAnim, &MeshMaterial3d<StandardMaterial>, &ChildOf)>,
    ghosts: Query<&Ghost>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let t = time.elapsed_secs();
    for (mut anim, handle, child_of) in &mut sheets {
        let offset = ghosts.get(child_of.parent()).map_or(0.0, |g| g.bob_offset);
        let frame = ((t + offset) * anim.fps) as u32 % anim.frames;
        if frame == anim.shown {
            continue;
        }
        anim.shown = frame;
        if let Some(mut material) = materials.get_mut(&handle.0) {
            let n = anim.frames as f32;
            material.uv_transform = bevy::math::Affine2::from_scale_angle_translation(
                Vec2::new(1.0 / n, 1.0),
                0.0,
                Vec2::new(frame as f32 / n, 0.0),
            );
        }
    }
}

/// Impatient ghosts grow paler; ascending ones glow and dissolve.
fn fade_ghosts(
    data: Res<GameData>,
    bodies: Query<(&ChildOf, &MeshMaterial3d<StandardMaterial>), With<GhostBody>>,
    ghosts: Query<&Ghost>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (child_of, handle) in &bodies {
        let Ok(g) = ghosts.get(child_of.parent()) else {
            continue;
        };
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_png_size() {
        let mut header = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        header.extend_from_slice(&704u32.to_be_bytes());
        header.extend_from_slice(&264u32.to_be_bytes());
        assert_eq!(png_size(&header), Some((704, 264)));
        assert_eq!(png_size(b"not a png at all, sorry"), None);
    }
}
