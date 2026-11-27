//! Ghosts: they come in, browse, queue at the counter and leave again.
//!
//! A ghost's flow:
//! Entering → Browsing → ToCounter → Waiting → AtRegister → Leaving / Ascending
//! (if the atmosphere does not suit them: Entering → Fleeing)

pub(crate) mod memories;
mod motion;

use bevy::ecs::system::SystemParam;
use bevy::math::FloatOrd;
use bevy::picking::hover::HoverMap;
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

use crate::scene::{
    Billboard, LabelEnabled, LabelKind, PlaceWorldLabels, billboard_material, floor, world_label,
};
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
        memories::install(app);
        app.add_message::<Finale>()
            .add_message::<Visitor>()
            .init_resource::<Queue>()
            .init_resource::<HoveredGhost>()
            .insert_resource(SpawnTimer(2.0))
            .init_resource::<ThiefTonight>()
            .add_systems(
                OnEnter(Phase::Night),
                (
                    |mut q: ResMut<Queue>,
                     mut t: ResMut<SpawnTimer>,
                     mut h: ResMut<HoveredGhost>| {
                        h.0 = None;
                        q.0.clear();
                        t.0 = 2.0;
                    },
                    plan_thief,
                ),
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
                    say_farewell,
                    spawn_visitor,
                    spawn_thief,
                    spawn_ordering,
                    ghost_ai,
                    ghost_murmurs,
                    find_hovered_ghost,
                    ghost_tooltip,
                    select_queue_nameplate.before(PlaceWorldLabels),
                    steal_from_shelf,
                    (move_ghosts, ground_glows).chain(),
                    show_carried_wares.after(crate::scene::face_camera),
                    fade_ghosts,
                    animate_ghosts,
                    motion::animate,
                    show_gestures,
                    play_sheets,
                )
                    .chain()
                    .run_if(in_state(Phase::Night))
                    .run_if(crate::menu::shop_unpaused)
                    .run_if(crate::story_film::outside_film),
            )
            // Deliver the finale guest in its chapter's frame, before any
            // film pause. The guest then holds its place until playback ends.
            .add_systems(
                Update,
                come_home
                    .after(crate::register::make_change)
                    .run_if(in_state(Phase::Night))
                    .run_if(crate::menu::shop_unpaused)
                    .run_if(crate::story_film::outside_film),
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
    /// The customer introduced by the first-sale guide.
    pub first_customer: bool,
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
    /// (story, chapter) this visit belongs to (story ghosts only).
    pub chapter: Option<(usize, usize)>,
    /// A story ghost that has waited for its wish long enough names it.
    pub plain: bool,
    /// It comes back for a ware it ordered last night.
    pub order: Option<usize>,
    /// Shelf cell the ghost browses, if any.
    browse_cell: Option<usize>,
    /// The ware picked up at a shelf or collected after payment.
    /// A guest arriving empty-handed whispers its wish.
    pub carrying: Option<String>,
    /// A regular customer: its index in `Progress::journal.regulars`.
    pub regular: Option<usize>,
    /// What this particular visitor recalled, even if another arrives later.
    pub memory: Option<crate::regulars::Memory>,
    bob_offset: f32,
}

#[cfg(test)]
impl Ghost {
    /// A plain customer of `GameData::ghosts[def]` waiting at the counter,
    /// for tests of the register and the orders.
    pub(crate) fn waiting(def: usize, wish: &str) -> Self {
        Self {
            first_customer: false,
            def,
            wish: wish.into(),
            hint: String::new(),
            state: GhostState::Waiting,
            pos: Vec2::ZERO,
            target: Vec2::ZERO,
            timer: 0.0,
            patience: 10.0,
            max_patience: 10.0,
            lift: 0.0,
            vel: Vec2::ZERO,
            facing: 1.0,
            age: 0.0,
            chapter: None,
            plain: false,
            order: None,
            browse_cell: None,
            carrying: None,
            regular: None,
            memory: None,
            bob_offset: 0.0,
        }
    }
}

impl Ghost {
    /// How the log names this ghost at the start of a sentence: a regular
    /// by its name ("Mrs Wispel"), any other ghost by its kind ("The Shy Ghost").
    pub fn the(&self, data: &GameData, progress: &Progress, lang: &Lang) -> String {
        match self.regular.and_then(|i| progress.journal.regulars.get(i)) {
            Some(regular) => regular.name(lang),
            None => lang.ghost_the(&data.ghosts[self.def].id),
        }
    }

    /// What the ghost whispers, in the current language.
    pub fn whisper(&self, lang: &Lang) -> String {
        let text = match &self.memory {
            Some(memory) => format!("{} {}", memory.words(lang), lang.t(&self.hint)),
            None => lang.t(&self.hint),
        };
        if self.plain {
            // A fare chapter asks for a coin, not a ware.
            let plainly = if self.wish.is_empty() {
                lang.t("story.fare_plainly")
            } else {
                lang.f("story.plainly", &[("item", &lang.product(&self.wish))])
            };
            format!("{text} {plainly}")
        } else {
            text
        }
    }

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
    mut progress: ResMut<Progress>,
    lang: Res<Lang>,
    mut art: GhostArt,
    mut timer: ResMut<SpawnTimer>,
    mut rng: ResMut<Rng>,
    mut log: ResMut<GameLog>,
    mut cues: MessageWriter<Cue>,
    mut report: ResMut<NightReport>,
    ghosts: Query<&Ghost>,
    (upgrades, stock): (Res<Upgrades>, Res<crate::inventory::Stock>),
) {
    if progress.guiding_first_sale() && !ghosts.is_empty() {
        return;
    }
    timer.0 -= time.delta_secs();
    // Nobody new arrives in the last 20 seconds.
    let resuming =
        progress.guiding_first_sale() && progress.journal.first_sale_checkpoint.is_some();
    if (timer.0 > 0.0 && !resuming) || clock.elapsed > clock.length - 20.0 {
        return;
    }
    // Good reputation = more customers.
    let (min, max) = data.config.spawn_interval_secs;
    let rep_factor = data.config.reputation_spawn_factor(progress.reputation);
    let night = progress.calendar(&data);
    timer.0 =
        rng.range(min, max) * rep_factor * data.spawn_factor(night) * upgrades.spawn_factor(&data);

    if ghosts.iter().count() >= data.config.max_ghosts_in_shop {
        return;
    }

    let atmosphere = layout.atmosphere(&data, night);
    let guided_ware = if progress.guiding_first_sale() {
        let ware = crate::guide::displayed_ware(&layout, &stock);
        if ware.is_none() {
            // A shelf may have been emptied while the opening sign was turning.
            progress.journal.first_sale = crate::guide::FirstSale::Skipped;
        }
        ware
    } else {
        None
    };
    let (def_idx, mut wish) = if let Some(ware) = guided_ware {
        // A familiar kind that likes this ware, or a first-night visitor curious about it.
        let Some((idx, _)) = data
            .ghosts
            .iter()
            .enumerate()
            .filter(|(_, def)| def.spawn_weight > 0.0 && def.unlock_night <= night)
            .min_by_key(|(_, def)| {
                (
                    !def.wishes.iter().any(|w| w == ware),
                    def.unmet_need(&atmosphere).is_some(),
                )
            })
        else {
            return;
        };
        (idx, ware.to_owned())
    } else {
        let weights = crate::sets::odds_tonight(&data, &layout, &progress, &stock);
        let Some(idx) = rng.weighted(&weights) else {
            return;
        };
        let wishes = data.wishes_tonight(&data.ghosts[idx], night);
        (idx, wishes[rng.index(wishes.len())].to_string())
    };
    let def = &data.ghosts[def_idx];
    let first_customer = guided_ware.is_some();
    let unmet = if first_customer {
        None
    } else {
        def.unmet_need(&atmosphere)
    };
    let regular = if first_customer || unmet.is_some() {
        None
    } else {
        crate::regulars::returning(
            &data,
            &mut progress,
            &def.id,
            ghosts.iter().filter_map(|g| g.regular),
            &mut rng,
        )
    };
    let regular = regular.map(|i| {
        let r = &progress.journal.regulars[i];
        let wishes = data.wishes_tonight(def, night);
        wish = crate::regulars::wish(&data, r, &wishes, wish.clone(), &mut rng);
        crate::regulars::greet(i, &mut progress, &wish, &data, &layout, &lang, &mut log)
    });
    // A silent sighting at the door has not taught the player this guest's wishes yet.
    let unfamiliar = progress
        .journal
        .book
        .get(&def.id)
        .is_none_or(|entry| entry.wishes.is_empty());
    let hints = lang.count(&format!("product.{wish}.hint."));
    let hint_number = if first_customer {
        1
    } else {
        rng.index(hints) + 1
    };
    let hint = format!("product.{wish}.hint.{hint_number}");

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
        kind: VisitKind::Customer,
        first_customer,
        def_idx,
        wish,
        hint,
        chapter: None,
        plain: first_customer || unfamiliar || regular.is_some(),
        order: None,
        spot: None,
        flees: unmet.is_some(),
        patience_factor: upgrades.patience_factor(&data),
        regular,
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
        &mut progress.journal,
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

/// The ghost identified by both its tooltip and its queue nameplate.
#[derive(Resource, Default, PartialEq)]
struct HoveredGhost(Option<Entity>);

#[derive(Component)]
struct GhostHitbox;

/// The hitboxes let clicks through, so several ghosts can be under the same pointer.
/// Use the nearest current hit, including when the pointer leaves only one of them.
fn find_hovered_ghost(
    hits: Res<HoverMap>,
    hitboxes: Query<&ChildOf, With<GhostHitbox>>,
    mut hovered: ResMut<HoveredGhost>,
) {
    let nearest = hits
        .iter()
        .flat_map(|(pointer, hits)| {
            hits.iter()
                .map(move |(entity, hit)| (pointer, entity, hit.depth))
        })
        .filter_map(|(pointer, entity, depth)| {
            let ghost = hitboxes.get(*entity).ok()?.parent();
            Some((!pointer.is_mouse(), FloatOrd(depth), ghost))
        })
        .min();
    hovered.set_if_neq(HoveredGhost(nearest.map(|(_, _, ghost)| ghost)));
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
    ghosts: Query<(&Ghost, Has<Thief>, Has<Farewell>)>,
    queue: Res<Queue>,
    data: Res<GameData>,
    lang: Res<Lang>,
    settings: Res<Settings>,
    asset_server: Res<AssetServer>,
    mut tip: ResMut<HoverTip>,
    mut shown: Local<Option<String>>,
    progress: Res<Progress>,
) {
    // Another tooltip (a product on a shelf) took over: leave it alone.
    if shown.as_ref().is_some_and(|title| *title != tip.title) {
        *shown = None;
    }
    let ghost = hovered.0.and_then(|e| ghosts.get(e).ok().map(|g| (e, g)));
    let Some((entity, (g, thief, farewell))) = ghost.filter(|_| shown.is_some() || tip.is_empty())
    else {
        if shown.take().is_some() {
            HoverTip::hide(&mut tip);
        }
        return;
    };
    let def = &data.ghosts[g.def];
    let portrait = format!("sprites/ghosts/{}.png", def.id);
    let regular = g.regular.and_then(|i| progress.journal.regulars.get(i));
    let mut t = HoverTip {
        icon: asset_exists(&portrait).then(|| asset_server.load(portrait)),
        title: match regular {
            Some(r) => lang.f(
                "ghost_tip.regular_title",
                &[("name", &r.name(&lang)), ("ghost", &lang.ghost(&def.id))],
            ),
            None => lang.ghost(&def.id),
        },
        below_pointer: queue.0.contains(&entity),
        ..default()
    };
    let patience = (g.patience / g.max_patience.max(0.01)).clamp(0.0, 1.0);
    let (kind, mood) = match g.state {
        GhostState::Entering | GhostState::Browsing => {
            (TipKind::Text, lang.t("ghost_tip.browsing"))
        }
        GhostState::ToCounter => (TipKind::Text, lang.t("ghost_tip.coming")),
        GhostState::Waiting
            if settings.patient_ghosts || (g.first_customer && progress.guiding_first_sale()) =>
        {
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
    if !thief && !farewell {
        if g.order.is_some() && g.carrying.is_none() {
            t.line(
                TipKind::Good,
                lang.f("ghost_tip.ordered", &[("item", &lang.product(&g.wish))]),
            );
        } else if let Some(ware) = &g.carrying {
            t.line(
                TipKind::Good,
                lang.f("ghost_tip.carries", &[("item", &lang.product(ware))]),
            );
        } else if at_counter && queue.front() == Some(entity) {
            t.line(
                TipKind::Text,
                lang.f(
                    "ghost_tip.whispers",
                    &[("quote", &lang.f("quote", &[("text", &g.whisper(&lang))]))],
                ),
            );
        } else if !matches!(
            g.state,
            GhostState::Fleeing | GhostState::Leaving | GhostState::Ascending
        ) {
            t.line(TipKind::Hint, lang.t("ghost_tip.wish_later"));
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

/// Run the customer and story logic in the packaged handoff check without a display.
pub(crate) fn install_headless_customers(app: &mut App) {
    app.insert_resource(SpawnTimer(2.0))
        .init_resource::<Queue>()
        .add_systems(
            Update,
            (
                spawn_ghosts,
                spawn_story_ghost,
                spawn_ordering,
                ghost_ai,
                move_ghosts,
            )
                .chain()
                .run_if(in_state(Phase::Night)),
        );
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum VisitKind {
    Customer,
    Thief,
    Farewell,
    /// Drops by to see someone, stays a moment and leaves (`Visitor`).
    Visit,
}

/// What a newly arriving ghost comes for.
struct Visit {
    kind: VisitKind,
    first_customer: bool,
    def_idx: usize,
    wish: String,
    hint: String,
    chapter: Option<(usize, usize)>,
    plain: bool,
    order: Option<usize>,
    /// Where it goes instead of a shelf.
    spot: Option<Vec2>,
    flees: bool,
    /// The waiting bench: ghosts wait longer.
    patience_factor: f32,
    regular: Option<crate::regulars::RegularVisit>,
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
    journal: &mut crate::book::Journal,
    visit: Visit,
) -> Entity {
    let def: &GhostDef = &data.ghosts[visit.def_idx];
    // Only a returning order customer has already asked for the chosen product.
    // Other customers reveal their wish at the shelf or the counter.
    let wish = if visit.order.is_some() {
        visit.wish.as_str()
    } else {
        ""
    };
    journal.met(&def.id, wish);
    // A ghost goes straight to the shelf that shows its wish; otherwise it
    // looks around a shelf at random.
    let shelves = layout.shelf_cells(data);
    let browse_cell = layout
        .cells
        .iter()
        .position(|c| c.products.contains(&visit.wish))
        .or_else(|| (!shelves.is_empty()).then(|| shelves[rng.index(shelves.len())]));
    let (state, target) = if visit.flees {
        (GhostState::Fleeing, layout.door_pos() + Vec2::new(1.0, 0.0))
    } else {
        let target = match (visit.spot, browse_cell) {
            (Some(spot), _) => spot,
            // In front of the shelf, on the camera side.
            (None, Some(cell)) => layout.cell_pos(cell) + Vec2::new(0.0, 0.7),
            (None, None) => layout.cell_pos(rng.index(layout.cells.len())),
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
                first_customer: visit.first_customer,
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
                plain: visit.plain,
                order: visit.order,
                browse_cell,
                carrying: None,
                regular: visit.regular.as_ref().map(|r| r.index),
                memory: visit.regular.as_ref().map(|r| r.memory.clone()),
                bob_offset: rng.range(0.0, std::f32::consts::TAU),
            },
            Billboard,
            Transform::from_translation(floor(start, HOVER)),
            Visibility::default(),
            DespawnOnExit(Phase::Night),
        ))
        .id();
    match visit.kind {
        VisitKind::Thief => {
            commands.entity(ghost).insert(Thief);
        }
        VisitKind::Farewell => {
            commands.entity(ghost).insert(Farewell);
        }
        VisitKind::Visit => {
            commands.entity(ghost).insert(Visiting);
        }
        VisitKind::Customer => {}
    }
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
    commands.entity(body).insert(Look {
        ghost: def.id.clone(),
        base: (asset_exists(&image)).then(|| asset_server.load(image.clone())),
        base_frames: sheet,
        shown: None,
    });
    commands.entity(ghost).add_child(body);
    // A soft pool of borrowed light grounds the painted guest in the 3D room.
    let glow = "sprites/fx/glow.png";
    let textured = asset_exists(glow);
    commands.spawn((
        FloorGlow,
        Mesh3d(meshes.add(Ellipse::new(0.57, 0.37))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: rgb(def.color).with_alpha(if textured { 0.25 } else { 0.055 }),
            base_color_texture: textured.then(|| asset_server.load(glow)),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        })),
        Transform::from_xyz(0.0, 0.019 - HOVER, 0.0)
            .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
        NotShadowCaster,
        Pickable::IGNORE,
        ChildOf(ghost),
    ));
    // An invisible box over the body for the tooltip. It does not block the
    // pointer, so the shelf items behind a ghost stay clickable.
    commands.spawn((
        GhostHitbox,
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
    let name = match &visit.regular {
        Some(r) => r.name.clone(),
        None => lang.ghost(&def.id),
    };
    commands.spawn((
        GhostNameplate(ghost),
        LabelEnabled(false),
        world_label(
            font,
            Text::new(name),
            LabelKind::Name,
            ghost,
            Vec3::new(0.0, GHOST_SIZE.y + 0.15, 0.0),
        ),
    ));
    ghost
}

/// The visual belongs to the guest, including while it rests on the counter.
/// This keeps a single copy through a sale, even when another guest steps up.
#[derive(Component)]
struct ShowsCarried(Entity);

#[derive(Component)]
struct WareMotion {
    on_counter: bool,
    from: Vec3,
    elapsed: f32,
}

const CARRY_SPOT: Vec3 = Vec3::new(0.42, 0.6, 0.15);
const CARRY_SCALE: f32 = 0.85;
const COUNTER_WARE_OFFSET: Vec3 = Vec3::new(-0.18, crate::shop::COUNTER_TOP + 0.018, 0.05);
const STOCK_SIDE: Vec3 = Vec3::new(0.18, 0.14, -0.20);
const HANDOVER_TIME: f32 = 0.44;
const HANDOVER_ARC: f32 = 0.13;

impl WareMotion {
    fn position(
        &mut self,
        guest: &Transform,
        local: Vec3,
        counter: Option<Vec3>,
        dt: f32,
        still: bool,
    ) -> Vec3 {
        // The guest turns to face the camera. Interpolate in shop space so
        // that its rotation and hovering cannot move a ware off the counter.
        let pose = guest.compute_affine();
        if self.on_counter != counter.is_some() {
            self.on_counter = counter.is_some();
            self.from = pose.transform_point3(local);
            self.elapsed = 0.0;
        }
        self.elapsed = if still {
            HANDOVER_TIME
        } else {
            (self.elapsed + dt).min(HANDOVER_TIME)
        };
        if self.elapsed >= HANDOVER_TIME {
            return counter.map_or(CARRY_SPOT, |spot| pose.inverse().transform_point3(spot));
        }
        let target = counter.unwrap_or_else(|| pose.transform_point3(CARRY_SPOT));
        let t = self.elapsed / HANDOVER_TIME;
        let travel = t * t * (3.0 - 2.0 * t);
        let world = self.from.lerp(target, travel)
            + Vec3::Y * (HANDOVER_ARC * 4.0 * travel * (1.0 - travel));
        pose.inverse().transform_point3(world)
    }
}

/// The purchased ware passes between the guest and the counting mat.
/// Motion follows accepted sales, independent of how the player operates the register.
fn show_carried_wares(
    mut commands: Commands,
    time: Res<Time>,
    settings: Res<Settings>,
    register: Res<crate::register::Register>,
    layout: Res<ShopLayout>,
    ghosts: Query<(Entity, &Ghost, &Transform, Option<&ShowsCarried>)>,
    mut wares: Query<(&mut WareMotion, &mut Transform), Without<Ghost>>,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let counter = floor(layout.counter_pos(), 0.0) + COUNTER_WARE_OFFSET;
    for (entity, ghost, guest, shown) in &ghosts {
        let sale = register.sale.as_ref().filter(|sale| sale.ghost == entity);
        let on_counter = sale.is_some();
        let id = sale
            .map(|sale| sale.product.as_str())
            .or(ghost.carrying.as_deref());
        let Some(id) = id else {
            if let Some(visual) = shown {
                commands.entity(visual.0).despawn();
                commands.entity(entity).remove::<ShowsCarried>();
            }
            continue;
        };
        let counter_spot = on_counter.then_some(counter);
        if let Some(visual) = shown {
            if let Ok((mut motion, mut pose)) = wares.get_mut(visual.0) {
                pose.translation = motion.position(
                    guest,
                    pose.translation,
                    counter_spot,
                    time.delta_secs(),
                    settings.reduced_motion,
                );
            }
            continue;
        }
        let from = if on_counter && ghost.carrying.is_none() {
            counter + STOCK_SIDE
        } else {
            guest.transform_point(CARRY_SPOT)
        };
        let mut motion = WareMotion {
            on_counter,
            from,
            elapsed: 0.0,
        };
        let position = motion.position(
            guest,
            CARRY_SPOT,
            counter_spot,
            0.0,
            settings.reduced_motion,
        );
        let visual = commands
            .spawn((
                motion,
                Transform::from_translation(position).with_scale(Vec3::splat(CARRY_SCALE)),
                Visibility::default(),
                Pickable::IGNORE,
                ChildOf(entity),
            ))
            .id();
        let model = crate::shop::product_model(
            &mut commands,
            &assets,
            &mut meshes,
            &mut materials,
            id,
            Vec3::ZERO,
        );
        commands
            .entity(model)
            .insert((ChildOf(visual), Pickable::IGNORE));
        commands.entity(entity).insert(ShowsCarried(visual));
    }
}

/// Identifies the guest this plate belongs to, even if several share a name.
#[derive(Component)]
struct GhostNameplate(Entity);

/// The front guest owns the queue's one visible nameplate until the pointer
/// rests on another guest in line. Browsing and departing visitors have tooltips.
fn select_queue_nameplate(
    queue: Res<Queue>,
    hovered: Res<HoveredGhost>,
    mut labels: Query<(&GhostNameplate, &mut LabelEnabled)>,
) {
    let selected = hovered
        .0
        .filter(|guest| queue.0.contains(guest))
        .or_else(|| queue.front());
    for (plate, mut enabled) in &mut labels {
        enabled.set_if_neq(LabelEnabled(Some(plate.0) == selected));
    }
}

/// Ghosts come back for the wares they ordered last night, early in the
/// night and whatever the atmosphere.
fn spawn_ordering(
    mut commands: Commands,
    data: Res<GameData>,
    layout: Res<ShopLayout>,
    clock: Res<NightClock>,
    mut progress: ResMut<Progress>,
    lang: Res<Lang>,
    mut art: GhostArt,
    mut rng: ResMut<Rng>,
    mut cues: MessageWriter<Cue>,
    mut report: ResMut<NightReport>,
    upgrades: Res<Upgrades>,
    mut log: ResMut<GameLog>,
    ghosts: Query<&Ghost>,
) {
    if clock.is_over() {
        return;
    }
    let mut due = progress
        .orders
        .iter()
        .enumerate()
        .filter(|(_, o)| o.night == progress.night);
    let total = due.clone().count();
    let arrived = report.dispatched_orders.len();
    if arrived >= total {
        return;
    }
    let (from, to) = data.config.orders.arrive_at;
    let at = from + (to - from) * arrived as f32 / total as f32;
    if clock.elapsed < at * clock.length {
        return;
    }
    let Some((index, order)) = due.find(|(idx, order)| {
        !report.dispatched_orders.contains(idx)
            && order.state == crate::orders::OrderState::Open
            && !order.regular_name.is_some_and(|name| {
                ghosts
                    .iter()
                    .filter_map(|g| g.regular)
                    .filter_map(|i| progress.journal.regulars.get(i))
                    .any(|r| r.name == name && r.ghost == order.ghost)
            })
    }) else {
        return;
    };
    report.dispatched_orders.push(index);
    let Some(def_idx) = data.ghost_index(&order.ghost) else {
        return;
    };
    let order = order.clone();
    let regular = order.regular_name.and_then(|name| {
        let index = progress
            .journal
            .regulars
            .iter()
            .position(|r| r.name == name && r.ghost == order.ghost)?;
        progress.journal.regulars[index].visits += 1;
        Some(crate::regulars::greet(
            index,
            &mut progress,
            &order.product,
            &data,
            &layout,
            &lang,
            &mut log,
        ))
    });
    let visit = Visit {
        kind: VisitKind::Customer,
        first_customer: false,
        def_idx,
        wish: order.product.clone(),
        hint: "order.whisper".into(),
        chapter: None,
        plain: true,
        order: Some(index),
        spot: None,
        flees: false,
        patience_factor: upgrades.patience_factor(&data) * data.config.orders.patience_factor,
        regular,
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
        &mut progress.journal,
        visit,
    );
    cues.write(Cue::GhostEnters);
}

/// The ghost id of the shade that steals from shops with a bad reputation.
pub const THIEF_GHOST: &str = "shade";

/// When tonight's shade comes, as a fraction of the night, if it comes.
#[derive(Resource, Default)]
struct ThiefTonight(Option<f32>);

/// A shade that has come to steal.
#[derive(Component)]
struct Thief;

/// As the night begins: a shop with a bad reputation may get a visit from a
/// shade.
fn plan_thief(
    data: Res<GameData>,
    progress: Res<Progress>,
    mut rng: ResMut<Rng>,
    mut tonight: ResMut<ThiefTonight>,
) {
    let t = data.config.thief;
    let comes = progress.calendar(&data) >= t.from_night
        && progress.reputation < t.below
        && rng.chance(t.chance);
    tonight.0 = comes.then(|| rng.range(t.arrive_at.0, t.arrive_at.1));
}

/// The shade slips in – unless the shop is bright enough to drive it away.
fn spawn_thief(
    mut commands: Commands,
    data: Res<GameData>,
    layout: Res<ShopLayout>,
    clock: Res<NightClock>,
    mut progress: ResMut<Progress>,
    lang: Res<Lang>,
    mut art: GhostArt,
    mut rng: ResMut<Rng>,
    mut log: ResMut<GameLog>,
    mut cues: MessageWriter<Cue>,
    mut tonight: ResMut<ThiefTonight>,
) {
    let Some(at) = tonight.0 else {
        return;
    };
    if clock.elapsed < at * clock.length {
        return;
    }
    tonight.0 = None;
    let Some(def_idx) = data.ghost_index(THIEF_GHOST) else {
        return;
    };
    let wares = layout.displayed_products();
    if wares.is_empty() {
        return;
    }
    if layout.atmosphere(&data, progress.calendar(&data)).light >= data.config.thief.scared_by_light
    {
        log.push(lang.t("log.thief_scared"));
        return;
    }
    let visit = Visit {
        kind: VisitKind::Thief,
        first_customer: false,
        def_idx,
        wish: wares[rng.index(wares.len())].clone(),
        hint: "ghost_tip.thief".into(),
        chapter: None,
        plain: false,
        order: None,
        spot: None,
        flees: false,
        patience_factor: 1.0,
        regular: None,
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
        &mut progress.journal,
        visit,
    );
    cues.write(Cue::GhostEnters);
}

/// At the shelf the shade snatches a ware and makes for the door.
fn steal_from_shelf(
    mut ghosts: Query<&mut Ghost, With<Thief>>,
    data: Res<GameData>,
    mut stock: ResMut<Stock>,
    lang: Res<Lang>,
    mut log: ResMut<GameLog>,
    mut cues: MessageWriter<Cue>,
    mut report: ResMut<NightReport>,
) {
    for mut g in &mut ghosts {
        if g.state != GhostState::Browsing {
            continue;
        }
        g.state = GhostState::Leaving;
        if !stock.take_one(&g.wish) {
            continue;
        }
        g.carrying = Some(g.wish.clone());
        report.stolen.push(g.wish.clone());
        cues.write(Cue::GhostVoice(Voice::Flee, data.ghosts[g.def].voice));
        log.push(lang.f("log.thief", &[("item", &lang.product(&g.wish))]));
    }
}

/// A story with a finale guest is finished: that ghost comes through the
/// shop door to say goodbye (Hedda at the end of the first season).
#[derive(Message, Clone)]
pub struct Finale(pub String);

/// A story ghost who drops by to see someone at `spot` (Lotte visiting
/// Pfote on his rug, `pfote.rs`): it stays a moment and leaves.
#[derive(Message, Clone)]
pub struct Visitor {
    pub ghost: String,
    pub spot: Vec2,
}

/// A ghost on a visit: it does not shop.
#[derive(Component)]
pub struct Visiting;

/// Seconds a visitor stays at its spot.
const VISIT_SECS: f32 = 6.0;

fn spawn_visitor(
    mut visitors: MessageReader<Visitor>,
    mut commands: Commands,
    data: Res<GameData>,
    layout: Res<ShopLayout>,
    lang: Res<Lang>,
    mut art: GhostArt,
    mut rng: ResMut<Rng>,
    mut progress: ResMut<Progress>,
    mut cues: MessageWriter<Cue>,
) {
    for v in visitors.read() {
        let Some(def_idx) = data.ghost_index(&v.ghost) else {
            continue;
        };
        let visit = Visit {
            kind: VisitKind::Visit,
            first_customer: false,
            def_idx,
            wish: data.ghosts[def_idx]
                .wishes
                .first()
                .cloned()
                .unwrap_or_default(),
            hint: String::new(),
            chapter: None,
            plain: false,
            order: None,
            spot: Some(v.spot),
            flees: false,
            patience_factor: 1.0,
            regular: None,
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
            &mut progress.journal,
            visit,
        );
        cues.write(Cue::GhostEnters);
    }
}

/// The finale guest: she does not shop, she says goodbye.
#[derive(Component)]
struct Farewell;

fn come_home(
    mut finale: MessageReader<Finale>,
    mut commands: Commands,
    data: Res<GameData>,
    mut progress: ResMut<Progress>,
    layout: Res<ShopLayout>,
    lang: Res<Lang>,
    mut art: GhostArt,
    mut rng: ResMut<Rng>,
    mut log: ResMut<GameLog>,
    mut cues: MessageWriter<Cue>,
    mut report: ResMut<NightReport>,
) {
    let guests: Vec<String> = finale.read().map(|f| f.0.clone()).collect();
    for guest in guests {
        let Some(def_idx) = data.ghost_index(&guest) else {
            continue;
        };
        let def = &data.ghosts[def_idx];
        let farewell = format!("finale.{guest}.farewell");
        // The goodbye is told at once: the finale can come just before dawn.
        log.push(lang.t(&format!("finale.{guest}.arrives")));
        log.push(lang.f(
            "log.story_says",
            &[("name", &lang.ghost(&guest)), ("text", &lang.t(&farewell))],
        ));
        report.story_lines.push((guest.clone(), farewell.clone()));
        let visit = Visit {
            kind: VisitKind::Farewell,
            first_customer: false,
            def_idx,
            wish: def.wishes[0].clone(),
            hint: farewell,
            chapter: None,
            plain: false,
            order: None,
            spot: None,
            flees: false,
            patience_factor: 1.0,
            regular: None,
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
            &mut progress.journal,
            visit,
        );
        cues.write(Cue::GhostEnters);
    }
}

/// Once inside, the finale guest passes into the light.
fn say_farewell(mut ghosts: Query<&mut Ghost, With<Farewell>>, mut cues: MessageWriter<Cue>) {
    for mut g in &mut ghosts {
        if g.state != GhostState::Browsing {
            continue;
        }
        cues.write(Cue::Ascend(g.pos));
        g.state = GhostState::Ascending;
        g.timer = 0.0;
    }
}

/// Each story ghost arrives once per night until its story is finished.
fn spawn_story_ghost(
    mut commands: Commands,
    data: Res<GameData>,
    layout: Res<ShopLayout>,
    clock: Res<NightClock>,
    mut progress: ResMut<Progress>,
    lang: Res<Lang>,
    mut art: GhostArt,
    mut rng: ResMut<Rng>,
    mut report: ResMut<NightReport>,
    mut log: ResMut<GameLog>,
    mut cues: MessageWriter<Cue>,
    upgrades: Res<Upgrades>,
) {
    for (i, story) in data.stories.iter().enumerate() {
        if report.story_arrived.contains(&i)
            || !progress.story_due(&data, i)
            || clock.elapsed < story.arrive_at * clock.length
        {
            continue;
        }
        let Some(def_idx) = data.ghost_index(&story.ghost) else {
            continue;
        };
        report.story_arrived.push(i);
        let chapter = progress.chapter(i);
        let c = &story.chapters[chapter];
        log.push(lang.t(&format!("story.{}.arrives", story.ghost)));
        let visit = Visit {
            kind: VisitKind::Customer,
            first_customer: false,
            def_idx,
            wish: c.wish.clone(),
            hint: format!("story.{}.{}.intro", story.ghost, chapter + 1),
            chapter: Some((i, chapter)),
            plain: progress.asks_plainly(i),
            order: None,
            spot: None,
            flees: false,
            patience_factor: upgrades.patience_factor(&data),
            regular: None,
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
            &mut progress.journal,
            visit,
        );
        cues.write(Cue::GhostEnters);
    }
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
    visiting: Query<(), With<Visiting>>,
) {
    let dt = time.delta_secs();
    for (entity, mut g) in &mut ghosts {
        if g.first_customer
            && g.state == GhostState::Entering
            && progress.journal.first_sale_checkpoint.is_some()
        {
            g.state = GhostState::Waiting;
            progress.journal.heard(&data.ghosts[g.def].id, &g.wish);
            g.pos = layout.queue_pos(0, 1);
            g.target = g.pos;
            queue.0.push(entity);
        }
        match g.state {
            // A visitor stays a moment at its spot, then goes.
            GhostState::Entering if g.arrived() && visiting.contains(entity) => {
                g.state = GhostState::Browsing;
                g.timer = VISIT_SECS;
            }
            GhostState::Browsing if visiting.contains(entity) => {
                g.timer -= dt;
                if g.timer <= 0.0 {
                    g.state = GhostState::Leaving;
                }
            }
            GhostState::Entering if g.arrived() => {
                g.state = GhostState::Browsing;
                g.timer = rng.range(2.0, 4.5);
                let quirks = &data.ghosts[g.def].quirks;
                let calm = quirks
                    .calm_with
                    .is_some_and(|n| n.is_met(&layout.atmosphere(&data, progress.calendar(&data))));
                if !g.first_customer
                    && !calm
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
                            ("who", &g.the(&data, &progress, &lang)),
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
                    // Its wish is on this shelf: it takes it to the counter.
                    let on_shelf = g
                        .browse_cell
                        .is_some_and(|c| layout.cells[c].products.contains(&g.wish));
                    if on_shelf
                        && stock.count(&g.wish) > 0
                        && g.chapter.is_none()
                        && !(g.first_customer && progress.guiding_first_sale())
                    {
                        g.carrying = Some(g.wish.clone());
                        progress.journal.heard(&data.ghosts[g.def].id, &g.wish);
                        log.push(lang.f(
                            "log.takes_from_shelf",
                            &[
                                ("who", &g.the(&data, &progress, &lang)),
                                ("item", &lang.product(&g.wish)),
                            ],
                        ));
                    }
                    g.state = GhostState::ToCounter;
                    if !queue.0.contains(&entity) {
                        queue.0.push(entity);
                    }
                }
            }
            GhostState::ToCounter if g.arrived() => {
                g.state = GhostState::Waiting;
                // At the front of the line it whispers what it is looking for
                // (unless it brings the ware itself).
                if queue.front() == Some(entity) && g.carrying.is_none() {
                    progress.journal.heard(&data.ghosts[g.def].id, &g.wish);
                    cues.write(Cue::GhostVoice(Voice::Whisper, data.ghosts[g.def].voice));
                }
            }
            GhostState::Waiting => {
                // Assist setting: patient ghosts never give up.
                if !settings.patient_ghosts && !(g.first_customer && progress.guiding_first_sale())
                {
                    g.patience -= dt;
                }
                if g.patience <= 0.0 {
                    let who = g.the(&data, &progress, &lang);
                    progress.set_order_state(g.order, crate::orders::OrderState::Expired);
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

fn move_ghosts(
    time: Res<Time>,
    settings: Res<Settings>,
    mut ghosts: Query<(&mut Ghost, &mut Transform)>,
) {
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
        let bob = if settings.reduced_motion {
            0.0
        } else {
            (t * 2.2 + g.bob_offset).sin() * 0.045
        };
        transform.translation = floor(g.pos, HOVER + g.lift + bob);
    }
}

#[derive(Component)]
struct FloorGlow;

/// The reflection stays on the floor and recedes as the guest passes into the light.
fn ground_glows(
    ghosts: Query<(&Ghost, &Transform)>,
    mut glows: Query<(&ChildOf, &mut Transform), (With<FloorGlow>, Without<Ghost>)>,
) {
    for (parent, mut glow) in &mut glows {
        if let Ok((g, at)) = ghosts.get(parent.parent()) {
            glow.translation.y = 0.019 - at.translation.y;
            glow.scale = Vec3::splat((1.0 - g.lift / 1.65).clamp(0.0, 1.0));
        }
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
            // Finish the turn even if the ghost has already reached the queue.
            g.facing.signum()
        };
        if settings.reduced_motion {
            g.facing = wanted;
            body.scale = Vec3::new(wanted, 1.0, 1.0);
            body.rotation = Quat::IDENTITY;
            continue;
        }
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
        // Let the portrait settle after a stop; keep its silhouette readable as it rises.
        let rise = if g.state == GhostState::Ascending {
            (g.timer / 1.8).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let stretch = 1.0 + 0.22 * rise;
        let flip = g.facing.signum() * (0.62 + 0.38 * g.facing.abs());
        let target = Quat::from_rotation_z(roll + 0.14 * rise);
        body.rotation = body.rotation.slerp(target, 1.0 - (-9.0 * dt).exp());
        body.scale = Vec3::new(
            pop * (1.0 - breathe * 0.75) / stretch.sqrt() * flip,
            pop * (1.0 + breathe) * stretch,
            1.0,
        );
    }
}

/// Steps through the frames of animated sprite sheets.
/// What a ghost's body shows: its portrait or idle sheet, or the drawn
/// sheet of what it is doing right now.
#[derive(Component)]
struct Look {
    ghost: String,
    base: Option<Handle<Image>>,
    base_frames: Option<u32>,
    /// The drawn activity on display; `None` for the portrait or idle sheet.
    shown: Option<motion::Act>,
}

/// A ghost doing something it has a drawn sheet for shows that sheet; the
/// rest of the time its portrait (or idle loop) moves by itself.
fn show_gestures(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut bodies: Query<(
        Entity,
        &motion::Rig,
        &mut Look,
        &MeshMaterial3d<StandardMaterial>,
    )>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, rig, mut look, handle) in &mut bodies {
        let want = rig.drawn_act();
        if want == look.shown {
            continue;
        }
        look.shown = want;
        let (image, frames) = match want {
            Some(act) => {
                let path = motion::gesture_path(&look.ghost, act);
                let frames = sheet_frames(&path);
                (Some(asset_server.load(path)), frames)
            }
            None => (look.base.clone(), look.base_frames),
        };
        let Some(mut material) = materials.get_mut(&handle.0) else {
            continue;
        };
        material.base_color_texture = image;
        match frames {
            Some(frames) => {
                commands.entity(entity).insert(SheetAnim {
                    frames,
                    fps: SHEET_FPS,
                    shown: u32::MAX,
                });
            }
            None => {
                commands.entity(entity).remove::<SheetAnim>();
                material.uv_transform = bevy::math::Affine2::IDENTITY;
            }
        }
    }
}

fn play_sheets(
    time: Res<Time>,
    settings: Res<Settings>,
    mut sheets: Query<(
        &mut SheetAnim,
        &MeshMaterial3d<StandardMaterial>,
        &ChildOf,
        &Look,
    )>,
    ghosts: Query<(&Ghost, Option<&memories::Gesture>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let t = time.elapsed_secs();
    for (mut anim, handle, child_of, look) in &mut sheets {
        let Ok((ghost, gesture)) = ghosts.get(child_of.parent()) else {
            continue;
        };
        let frame = if look.shown == Some(motion::Act::Salute)
            && let Some(gesture) = gesture
        {
            gesture.salute_frame(anim.frames, settings.reduced_motion)
        } else if settings.reduced_motion {
            0
        } else {
            ((t + ghost.bob_offset) * anim.fps) as u32 % anim.frames
        };
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
    fn three_identical_guests_share_one_queue_nameplate_as_hover_and_service_change() {
        use bevy::picking::{backend::HitData, pointer::PointerId};

        let mut app = App::new();
        app.init_resource::<Queue>()
            .init_resource::<HoverMap>()
            .init_resource::<HoveredGhost>()
            .add_systems(Update, (find_hovered_ghost, select_queue_nameplate).chain());
        let world = app.world_mut();
        let camera = world.spawn_empty().id();
        let guests = [(); 3].map(|()| world.spawn_empty().id());
        let hitboxes = guests.map(|guest| world.spawn((GhostHitbox, ChildOf(guest))).id());
        let plates = guests.map(|guest| {
            world
                .spawn((
                    GhostNameplate(guest),
                    LabelEnabled(false),
                    Text::new("Shy Ghost"),
                ))
                .id()
        });
        let point_at = |app: &mut App, hits: &[(usize, f32)]| {
            let mut pointers = app.world_mut().resource_mut::<HoverMap>();
            let under_pointer = pointers.entry(PointerId::Mouse).or_default();
            under_pointer.clear();
            for &(guest, depth) in hits {
                under_pointer.insert(hitboxes[guest], HitData::new(camera, depth, None, None));
            }
        };
        let expect = |app: &mut App, selected: Option<Entity>| {
            app.update();
            assert_eq!(
                plates.map(|e| app.world().get::<LabelEnabled>(e).unwrap().0),
                guests.map(|e| Some(e) == selected),
            );
        };

        expect(&mut app, None);
        app.insert_resource(Queue(guests.to_vec()));
        expect(&mut app, Some(guests[0]));
        point_at(&mut app, &[(1, 5.), (2, 3.)]);
        expect(&mut app, Some(guests[2]));
        point_at(&mut app, &[(1, 5.)]);
        expect(&mut app, Some(guests[1]));
        point_at(&mut app, &[]);
        expect(&mut app, Some(guests[0]));
        app.world_mut().resource_mut::<Queue>().remove(guests[0]);
        expect(&mut app, Some(guests[1]));
        point_at(&mut app, &[(2, 3.)]);
        expect(&mut app, Some(guests[2]));
        app.world_mut().resource_mut::<Queue>().remove(guests[2]);
        expect(&mut app, Some(guests[1]));
        app.world_mut().resource_mut::<Queue>().remove(guests[1]);
        expect(&mut app, None);
    }

    fn ordering_fixture() -> (App, Progress) {
        let data = GameData::load();
        let mut progress = Progress::new(0);
        let first = crate::orders::Order::new("cat", "fish_bone", progress.night, Some(1));
        progress.orders = vec![
            first.clone(),
            first,
            crate::orders::Order::new("shy", "letter_paper", progress.night, None),
        ];
        progress
            .journal
            .regulars
            .push(ron::from_str(r#"(ghost:"cat",name:1,visits:2,favourite:"fish_bone")"#).unwrap());
        let layout = ShopLayout::new(data.config.grid_cols, data.config.grid_rows);
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Image>()
            .init_asset::<Font>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .insert_resource(Lang::load("en"))
            .init_resource::<UiFont>()
            .init_resource::<Upgrades>()
            .insert_resource(layout)
            .insert_resource(data)
            .insert_resource(Rng::new(1))
            .insert_resource(NightClock {
                elapsed: 50.,
                length: 100.,
            })
            .add_message::<Cue>()
            .init_resource::<GameLog>()
            .add_systems(Update, spawn_ordering);
        (app, progress)
    }

    #[test]
    fn first_sight_records_fleeing_special_and_story_visitors_without_imaginary_wishes() {
        let (mut app, mut progress) = ordering_fixture();
        progress.orders.clear();
        progress.journal = Default::default();
        app.insert_resource(progress)
            .init_resource::<NightReport>()
            .init_resource::<crate::accounts::Accounts>()
            .init_resource::<Stock>()
            .init_resource::<Queue>()
            .init_resource::<crate::save::Settings>()
            .init_resource::<HoveredGhost>()
            .init_resource::<HoverTip>()
            .insert_resource(SpawnTimer(0.0))
            .insert_resource(ThiefTonight(Some(0.0)))
            .add_message::<Finale>()
            .add_systems(
                Update,
                (
                    spawn_ghosts,
                    spawn_story_ghost,
                    spawn_thief,
                    come_home,
                    ghost_ai,
                    ghost_tooltip,
                )
                    .chain(),
            );
        let mut data = app.world_mut().resource_mut::<GameData>();
        for ghost in &mut data.ghosts {
            ghost.spawn_weight = if ghost.id == "vain" { 1.0 } else { 0.0 };
        }
        app.update();
        let world = app.world_mut();
        let journal = &world.resource::<Progress>().journal;
        assert!(journal.has_seen("vain") && journal.has_seen("edwin"));
        assert!(journal.book["vain"].wishes.is_empty() && journal.book["edwin"].wishes.is_empty());
        assert!(
            !journal.has_seen(THIEF_GHOST)
                && !journal.has_seen("hedda")
                && !journal.has_seen("lotte")
        );

        let vain = world.resource::<GameData>().ghost_index("vain").unwrap();
        let edwin = world.resource::<GameData>().ghost_index("edwin").unwrap();
        let mut guests = world.query::<(Entity, &mut Ghost)>();
        let mut queue = Queue::default();
        let mut hovered = None;
        for (e, mut ghost) in guests.iter_mut(world) {
            if ghost.def == edwin {
                ghost.state = GhostState::ToCounter;
                ghost.pos = ghost.target;
                queue.0.push(e);
            } else if ghost.def == vain {
                hovered = Some(e);
            }
        }
        assert!(hovered.is_some());
        world.insert_resource(HoveredGhost(hovered));
        world.insert_resource(queue);
        world.resource_mut::<ShopLayout>().cells[12]
            .products
            .push("lavender".into());
        world
            .resource_mut::<GameData>()
            .config
            .thief
            .scared_by_light = i32::MAX;
        world.insert_resource(ThiefTonight(Some(0.0)));
        world.write_message(Finale("hedda".into()));
        app.update();
        let world = app.world_mut();
        let journal = &world.resource::<Progress>().journal;
        assert_eq!(journal.book["edwin"].wishes, ["rain_jar"]);
        for id in [THIEF_GHOST, "hedda", "vain"] {
            assert!(journal.has_seen(id));
            let entry = &journal.book[id];
            assert_eq!((entry.met, entry.served), (1, 0));
            assert!(entry.wishes.is_empty() && entry.fulfilled_wishes.is_empty());
        }
        assert_eq!(world.resource::<HoverTip>().lines.len(), 1);
        assert_eq!(
            world
                .query_filtered::<&Ghost, With<Thief>>()
                .iter(world)
                .count(),
            1
        );
        assert_eq!(
            world
                .query_filtered::<&Ghost, With<Farewell>>()
                .iter(world)
                .count(),
            1
        );
    }

    #[test]
    fn replaying_a_night_brings_back_its_ordered_customer() {
        let (mut app, progress) = ordering_fixture();
        for _ in 0..2 {
            // Loading the evening snapshot starts a fresh report even for the same night.
            app.insert_resource(progress.clone())
                .insert_resource(NightReport::default());
            app.update();
            app.update();
            let played = app.world().resource::<Progress>();
            assert_eq!(played.journal.book["cat"].met, 1);
            assert_eq!(played.journal.book["cat"].wishes, ["fish_bone"]);
            assert_eq!(played.journal.regulars[0].visits, 3);
            assert_eq!(played.journal.regulars[0].encounters.len(), 1);
            let world = app.world_mut();
            assert_eq!(world.resource::<NightReport>().dispatched_orders, [0, 2]);
            let first = world
                .query::<(Entity, &Ghost)>()
                .iter(world)
                .find(|(_, g)| g.order == Some(0) && g.regular == Some(0))
                .unwrap()
                .0;
            world.despawn(first);
            app.update();
            let world = app.world_mut();
            assert_eq!(world.resource::<NightReport>().dispatched_orders, [0, 2, 1]);
            assert_eq!(world.resource::<Progress>().journal.regulars[0].visits, 4);
            let guests: Vec<_> = world
                .query_filtered::<Entity, With<Ghost>>()
                .iter(world)
                .collect();
            for guest in guests {
                world.despawn(guest);
            }
        }
    }

    #[test]
    fn an_order_waiting_for_the_same_customer_expires_when_the_first_visit_ends_at_dawn() {
        use crate::orders::OrderState;
        let (mut app, progress) = ordering_fixture();
        app.insert_resource(progress)
            .init_resource::<NightReport>()
            .init_resource::<crate::accounts::Accounts>();
        app.update();
        app.update();
        let world = app.world_mut();
        assert_eq!(world.resource::<NightReport>().dispatched_orders, [0, 2]);
        let first = world
            .query::<(Entity, &Ghost)>()
            .iter(world)
            .find(|(_, ghost)| ghost.order == Some(0))
            .unwrap()
            .0;
        world
            .resource_mut::<Progress>()
            .set_order_state(Some(0), OrderState::Collected);
        world.despawn(first);
        let mut clock = world.resource_mut::<NightClock>();
        clock.elapsed = clock.length;
        app.update();

        let world = app.world_mut();
        assert_eq!(world.resource::<NightReport>().dispatched_orders, [0, 2]);
        let mut progress = world.resource_mut::<Progress>();
        assert_eq!(progress.journal.regulars[0].encounters.len(), 1);
        progress.archive_due_orders();
        assert_eq!(
            progress.journal.order_history[0].state,
            OrderState::Collected
        );
        assert_eq!(progress.journal.order_history[1].state, OrderState::Expired);
    }

    #[test]
    fn the_guided_customer_waits_for_real_stock_and_resumes_counted_change_once() {
        use crate::economy::{Coin, Drawer};
        use crate::guide::{FirstSale, FirstSaleCheckpoint};
        use crate::register::{Register, RegisterPlugin};
        use crate::shop::Storeroom;
        use crate::ui::Action;

        let ware = "powder_compact";
        let open = |checkpoint| {
            let data = GameData::load();
            let mut layout = ShopLayout::new(data.config.grid_cols, data.config.grid_rows);
            let shelf = layout.shelf_cells(&data)[0];
            layout.cells[shelf].products.push(ware.into());
            let mut stock = Stock::default();
            stock.add(ware, 2);
            stock.add("rain_jar", 1);
            let mut progress = Progress::new(0);
            progress.journal.first_sale = FirstSale::Following;
            progress.journal.first_sale_checkpoint = Some(checkpoint);
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, AssetPlugin::default(), RegisterPlugin))
                .init_asset::<Image>()
                .init_asset::<Font>()
                .init_resource::<Assets<Mesh>>()
                .init_resource::<Assets<StandardMaterial>>()
                .insert_resource(Lang::load("en"))
                .init_resource::<UiFont>()
                .init_resource::<Upgrades>()
                .init_resource::<Storeroom>()
                .init_resource::<Drawer>()
                .init_resource::<GameLog>()
                .init_resource::<NightReport>()
                .init_resource::<crate::accounts::Accounts>()
                .init_resource::<Queue>()
                .init_resource::<Settings>()
                .insert_resource(State::new(Phase::Night))
                .insert_resource(SpawnTimer(0.0))
                .insert_resource(NightClock {
                    elapsed: 0.,
                    length: 100.,
                })
                .insert_resource(data)
                .insert_resource(layout)
                .insert_resource(stock)
                .insert_resource(progress)
                .insert_resource(Rng::new(9))
                .add_message::<Action>()
                .add_message::<Cue>()
                .add_message::<Finale>()
                .add_message::<crate::story_film::PlayStoryFilm>()
                .add_systems(Update, (spawn_ghosts, ghost_ai).chain());
            for _ in 0..3 {
                app.update();
            }
            app
        };

        let mut app = open(FirstSaleCheckpoint::Wish);
        let customer = app.world().resource::<Queue>().front().unwrap();
        let g = app.world().get::<Ghost>(customer).unwrap();
        assert!(g.first_customer && g.plain && g.carrying.is_none());
        assert_eq!(g.state, GhostState::Waiting);
        assert_eq!(g.wish, ware);
        assert_eq!(g.patience, g.max_patience);

        app.world_mut()
            .write_message(Action::Offer("rain_jar".into()));
        app.world_mut().write_message(Action::Decline);
        app.update();
        assert_eq!(app.world().resource::<Queue>().front(), Some(customer));
        assert_eq!(app.world().resource::<Stock>().count("rain_jar"), 1);
        assert_eq!(app.world().resource::<Progress>().reputation, 0);
        app.world_mut().write_message(Action::Offer(ware.into()));
        app.update();
        app.world_mut().write_message(Action::CheckCoin(0));
        app.world_mut()
            .write_message(Action::AddChange(Coin::Button));
        app.world_mut()
            .write_message(Action::AddChange(Coin::Button));
        app.world_mut().write_message(Action::GiveChange);
        app.update();
        let sale = app.world().resource::<Register>().sale.as_ref().unwrap();
        assert!(sale.recount);
        let checkpoint = FirstSaleCheckpoint::Change {
            coins: sale.coins.clone(),
            given: sale.given,
            recount: sale.recount,
        };
        let drawer = app.world().resource::<Drawer>().0;
        let price = sale.price;
        let mut resumed = open(ron::from_str(&ron::to_string(&checkpoint).unwrap()).unwrap());
        let world = resumed.world_mut();
        let sale = world.resource::<Register>().sale.as_ref().unwrap();
        assert_eq!(sale.given.buttons, 2);
        assert!(sale.recount);
        assert!(sale.coins[0].checked);
        assert_eq!(world.resource::<Stock>().count(ware), 1);
        assert_eq!(world.resource::<Drawer>().0, drawer);
        assert_eq!(world.resource::<Progress>().reputation, -1);
        world.write_message(Action::GiveChangeAnyway);
        resumed.update();
        assert!(resumed.world().resource::<Register>().is_busy());
        let world = resumed.world_mut();
        world.write_message(Action::ResetChange);
        world.write_message(Action::AddChange(Coin::Button));
        world.write_message(Action::GiveChange);
        resumed.update();
        let world = resumed.world();
        assert_eq!(world.resource::<NightReport>().sales, 1);
        assert_eq!(world.resource::<Stock>().count(ware), 1);
        assert_eq!(world.resource::<Drawer>().total(), price);
        assert_eq!(
            world.resource::<Progress>().journal.first_sale,
            FirstSale::Complete
        );
    }

    #[test]
    fn the_ware_tracks_its_guest_after_crossing_the_counter() {
        let counter = Vec3::new(6.1, 0.76, 0.25);
        let mut guest =
            Transform::from_xyz(5.4, 0.25, 1.0).with_rotation(Quat::from_rotation_y(0.65));
        let hand = guest.transform_point(CARRY_SPOT);
        let mut motion = WareMotion {
            on_counter: false,
            from: hand,
            elapsed: HANDOVER_TIME,
        };
        let local = motion.position(
            &guest,
            CARRY_SPOT,
            Some(counter),
            HANDOVER_TIME * 0.5,
            false,
        );
        let mid = guest.transform_point(local);
        assert!(mid.y > hand.y && mid.z < hand.z && mid.z > counter.z);
        guest.rotate_y(0.30);
        guest.translation.y += 0.04;
        let settled = motion.position(&guest, local, Some(counter), HANDOVER_TIME, false);
        assert!(guest.transform_point(settled).distance(counter) < 1e-5);
        assert_eq!(
            motion.position(&guest, settled, None, 0.01, true),
            CARRY_SPOT
        );
        guest.translation += Vec3::X;
        assert_eq!(
            motion.position(&guest, CARRY_SPOT, None, 0.01, false),
            CARRY_SPOT
        );
    }

    #[test]
    fn reads_png_size() {
        let mut header = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        header.extend_from_slice(&704u32.to_be_bytes());
        header.extend_from_slice(&264u32.to_be_bytes());
        assert_eq!(png_size(&header), Some((704, 264)));
        assert_eq!(png_size(b"not a png at all, sorry"), None);
    }

    fn lavender_sale() -> (App, Entity) {
        let (mut app, ghost) = customer_at_counter();
        app.update();
        (app, ghost)
    }

    fn customer_at_counter() -> (App, Entity) {
        use crate::economy::{Coin, Drawer, Purse};
        use crate::inventory::Stock;
        use crate::register::RegisterPlugin;
        use crate::shop::Storeroom;
        use crate::ui::Action;

        let mut data = GameData::load();
        let shy = data.ghost_index("shy").unwrap();
        data.ghosts[shy].pays_with = vec![Coin::Candle];
        let layout = ShopLayout::new(data.config.grid_cols, data.config.grid_rows);
        let mut app = App::new();
        app.add_plugins(RegisterPlugin)
            .add_message::<Action>()
            .add_message::<Cue>()
            .add_message::<Finale>()
            .add_message::<crate::story_film::PlayStoryFilm>()
            .insert_resource(State::new(Phase::Night))
            .insert_resource(Progress::new(0))
            .insert_resource(layout)
            .insert_resource(data)
            .insert_resource(Lang::load("en"))
            .insert_resource(Rng::new(1))
            .insert_resource(Drawer(Purse {
                buttons: 4,
                ..default()
            }))
            .init_resource::<Stock>()
            .init_resource::<Storeroom>()
            .init_resource::<GameLog>()
            .init_resource::<NightReport>()
            .init_resource::<crate::accounts::Accounts>();
        let world = app.world_mut();
        let ware = "lavender";
        world.resource_mut::<Stock>().add(ware, 1);
        let ghost = world
            .spawn(Ghost {
                first_customer: false,
                def: shy,
                wish: ware.into(),
                hint: String::new(),
                state: GhostState::Waiting,
                pos: Vec2::ZERO,
                target: Vec2::ZERO,
                timer: 0.0,
                patience: 10.0,
                max_patience: 10.0,
                lift: 0.0,
                vel: Vec2::ZERO,
                facing: 1.0,
                age: 0.0,
                chapter: None,
                plain: false,
                order: None,
                browse_cell: None,
                carrying: Some(ware.into()),
                regular: None,
                memory: None,
                bob_offset: 0.0,
            })
            .id();
        world.insert_resource(Queue(vec![ghost]));
        (app, ghost)
    }

    #[test]
    fn each_story_plays_one_film_after_its_final_sale_parcel_or_fare() {
        use crate::economy::Coin;
        use crate::story_film::PlayStoryFilm;
        use crate::ui::Action;

        let (mut app, customer) = customer_at_counter();
        let stories = app.world().resource::<GameData>().stories.clone();
        for (i, story) in stories.iter().enumerate() {
            let def = app
                .world()
                .resource::<GameData>()
                .ghost_index(&story.ghost)
                .unwrap();
            app.world_mut().resource_mut::<GameData>().ghosts[def].pays_with = vec![Coin::Button];
            let last = story.chapters.len() - 1;
            // Re-deliver the last chapter to the already completed progress.
            for (visit, chapter) in (0..=last).chain([last]).enumerate() {
                let world = app.world_mut();
                let wish = &story.chapters[chapter].wish;
                {
                    let mut ghost = world.get_mut::<Ghost>(customer).unwrap();
                    ghost.def = def;
                    ghost.wish.clone_from(wish);
                    ghost.carrying = None;
                    ghost.state = GhostState::Waiting;
                    ghost.chapter = Some((i, chapter));
                }
                world.insert_resource(Queue(vec![customer]));
                if story.chapters[chapter].fare {
                    world.write_message(Action::GiveFare);
                    world.write_message(Action::GiveFare);
                } else {
                    if (i + chapter) % 2 == 0 {
                        world.resource_mut::<Stock>().add(wish, 1);
                        world.write_message(Action::Offer(wish.clone()));
                    } else {
                        world.resource_mut::<Progress>().parcels.push(i);
                        world.write_message(Action::HandParcel);
                    }
                    world.write_message(Action::GiveChange);
                    world.write_message(Action::GiveChange);
                }
                app.update();
                let films: Vec<_> = app
                    .world_mut()
                    .resource_mut::<Messages<PlayStoryFilm>>()
                    .drain()
                    .map(|film| film.0)
                    .collect();
                if visit == last {
                    assert_eq!(films.as_slice(), std::slice::from_ref(&story.ghost));
                } else {
                    assert!(films.is_empty(), "{} on visit {visit}", story.ghost);
                }
                assert_eq!(
                    app.world().resource::<Progress>().chapter(i),
                    (visit + 1).min(last + 1)
                );
            }
        }
    }

    #[test]
    fn settling_one_of_two_identical_orders_updates_only_that_customers_slip() {
        use crate::economy::Coin;
        use crate::orders::{Order, OrderState};
        use crate::register::Register;
        use crate::ui::Action;

        let (mut app, ghost) = customer_at_counter();
        let world = app.world_mut();
        let first = Order::new("shy", "lavender", 1, None);
        world.resource_mut::<Progress>().orders = vec![first.clone(), first];
        world.get_mut::<Ghost>(ghost).unwrap().order = Some(1);
        app.update();
        let world = app.world_mut();
        assert!(world.resource::<Register>().is_busy_with(ghost));
        assert_eq!(
            world.resource::<Progress>().orders[0].state,
            OrderState::Open
        );
        assert_eq!(
            world.resource::<Progress>().orders[1].state,
            OrderState::AtRegister
        );
        let change = world
            .resource::<Register>()
            .sale
            .as_ref()
            .unwrap()
            .change_due();
        for _ in 0..change {
            world.write_message(Action::AddChange(Coin::Button));
        }
        world.write_message(Action::GiveChange);
        app.update();
        let world = app.world_mut();
        assert_eq!(world.resource::<NightReport>().orders_fulfilled, 1);
        assert_eq!(
            world.resource::<Progress>().orders[0].state,
            OrderState::Open
        );
        assert_eq!(
            world.resource::<Progress>().orders[1].state,
            OrderState::Collected
        );
    }

    #[test]
    fn accepting_the_last_ware_reserves_it_until_change_is_given() {
        use crate::economy::{Coin, Drawer};
        use crate::inventory::Stock;
        use crate::register::Register;
        use crate::ui::Action;

        let (mut app, ghost) = lavender_sale();
        let world = app.world_mut();
        let ware = "lavender";
        assert!(world.resource::<Register>().is_busy_with(ghost));
        assert!(!world.resource_mut::<Stock>().take_one(ware));
        let received = world.resource::<Drawer>().0;
        let reputation = world.resource::<Progress>().reputation;
        world.resource_mut::<Stock>().add(ware, 1);
        // The customer paid five for a four-button ware. Neither mistake
        // commits the sale, moves the money or penalises the shopkeeper.
        world.write_message(Action::GiveChange);
        app.update();
        let world = app.world_mut();
        assert!(world.resource::<Register>().sale.as_ref().unwrap().recount);
        assert_eq!(world.resource::<Queue>().front(), Some(ghost));
        world.write_message(Action::AddChange(Coin::Button));
        world.write_message(Action::AddChange(Coin::Button));
        world.write_message(Action::GiveChange);
        app.update();
        let world = app.world_mut();
        let sale = world.resource::<Register>().sale.as_ref().unwrap();
        assert_eq!(sale.given.buttons, 2);
        assert!(sale.recount);
        assert_eq!(world.resource::<NightReport>().sales, 0);
        assert_eq!(world.resource::<NightReport>().change_wrong, 0);
        assert_eq!(world.resource::<Drawer>().0, received);
        assert_eq!(world.resource::<Progress>().reputation, reputation);
        assert_eq!(
            world.get::<Ghost>(ghost).unwrap().state,
            GhostState::AtRegister
        );
        world.write_message(Action::RemoveChange(Coin::Button));
        // A stale confirmation cannot submit the corrected tray.
        world.write_message(Action::GiveChangeAnyway);
        app.update();
        let world = app.world_mut();
        let sale = world.resource::<Register>().sale.as_ref().unwrap();
        assert_eq!(sale.given.buttons, 1);
        assert!(!sale.recount);
        let amounts = (sale.price, sale.paid().total(), sale.given.total());
        assert!(world.resource::<NightReport>().receipts.is_empty());
        world.write_message(Action::GiveChange);
        world.write_message(Action::GiveChange);
        app.update();
        let report = app.world().resource::<NightReport>();
        let entry = &app.world().resource::<Progress>().journal.book["shy"];
        assert_eq!(entry.fulfilled_wishes, ["lavender"]);
        assert_eq!(report.sales, 1);
        assert_eq!(report.change_correct, 1);
        let receipt = &report.receipts[0];
        assert_eq!(receipt.product, ware);
        assert_eq!((receipt.price, receipt.paid, receipt.given), amounts);
        assert_eq!(app.world().resource::<Stock>().count(ware), 1);
    }

    #[test]
    fn a_paused_register_keeps_its_guest_counted_change_and_inspected_coin() {
        use crate::economy::{Coin, Drawer};
        use crate::menu::Paused;
        use crate::register::Register;
        use crate::ui::Action;
        let (mut app, guest) = lavender_sale();
        app.world_mut()
            .write_message(Action::AddChange(Coin::Button));
        app.world_mut().write_message(Action::CheckCoin(0));
        app.update();
        let drawer = app.world().resource::<Drawer>().0;
        let counted = app
            .world()
            .resource::<Register>()
            .sale
            .as_ref()
            .unwrap()
            .given;
        assert_eq!(counted.buttons, 1);
        app.insert_resource(Paused(true));
        app.world_mut().write_message(Action::ResetChange);
        for _ in 0..4 {
            app.update();
        }
        app.world_mut().resource_mut::<Paused>().0 = false;
        app.update();
        let sale = app.world().resource::<Register>().sale.as_ref().unwrap();
        assert_eq!(sale.ghost, guest);
        assert!(sale.coins[0].checked);
        assert_eq!(sale.given, counted);
        assert_eq!(app.world().resource::<Drawer>().0, drawer);
    }

    #[test]
    fn an_ordinary_customer_can_be_given_wrong_change_after_confirmation() {
        use crate::register::Register;
        use crate::ui::Action;

        let (mut app, ghost) = lavender_sale();
        app.world_mut().write_message(Action::GiveChangeAnyway);
        app.update();
        assert!(app.world().resource::<Register>().is_busy_with(ghost));
        app.world_mut().write_message(Action::GiveChange);
        app.update();
        app.world_mut().write_message(Action::GiveChangeAnyway);
        app.update();
        let report = app.world().resource::<NightReport>();
        assert_eq!(
            (report.sales, report.change_wrong, report.change_kept),
            (1, 1, 1)
        );
        assert!(!app.world().resource::<Register>().is_busy());
    }

    #[test]
    fn the_guided_customer_waits_for_the_corrected_amount() {
        use crate::economy::Coin;
        use crate::guide::FirstSale;
        use crate::register::Register;
        use crate::ui::Action;

        let (mut app, ghost) = lavender_sale();
        let world = app.world_mut();
        world.get_mut::<Ghost>(ghost).unwrap().first_customer = true;
        world.resource_mut::<Progress>().journal.first_sale = FirstSale::Following;
        world.write_message(Action::GiveChange);
        world.write_message(Action::GiveChangeAnyway);
        app.update();
        let world = app.world_mut();
        assert!(world.resource::<Register>().is_busy_with(ghost));
        assert!(world.resource::<Progress>().guiding_first_sale());
        assert_eq!(world.resource::<NightReport>().sales, 0);
        world.write_message(Action::AddChange(Coin::Button));
        world.write_message(Action::GiveChange);
        app.update();
        assert_eq!(
            app.world().resource::<Progress>().journal.first_sale,
            FirstSale::Complete
        );
        assert_eq!(app.world().resource::<NightReport>().change_correct, 1);
    }
}
