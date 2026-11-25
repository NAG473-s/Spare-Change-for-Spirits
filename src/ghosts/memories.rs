//! Memories witnessed in the shop and kept in the guest's book entry.

use super::*;

pub(crate) const RAIN_PAINTING: &str = "sprites/memories/edwin_rain.png";
const GLASS_RADII: Vec2 = Vec2::new(0.302, 0.505);
const GLASS_HEIGHT: f32 = 1.03;
const GESTURE_SECONDS: f32 = 4.8;
const RETURN_AT: f32 = 0.18;

pub(super) fn install(app: &mut App) {
    app.init_resource::<Invitations>()
        .add_systems(
            OnEnter(Phase::Night),
            |mut invitations: ResMut<Invitations>| {
                *invitations = Invitations::default();
            },
        )
        .add_systems(
            Update,
            (
                invite_returning_guests,
                join_queue,
                witness_meeting,
                reveal_mirror,
                animate_glass,
                finish_gestures,
            )
                .chain()
                .after(ghost_ai)
                .before(move_ghosts)
                .run_if(in_state(Phase::Night))
                .run_if(crate::story_film::outside_film)
                .run_if(crate::menu::shop_unpaused),
        );
}

// Returning story guests are visitors. They never take a customer's place or create a sale.
#[derive(Resource, Default)]
struct Invitations {
    quay: bool,
    mirror: bool,
}

#[derive(Component)]
pub(crate) struct AtQueueEnd(usize);

#[derive(Component)]
pub(crate) struct Gesture {
    pub(super) act: motion::Act,
    pub(super) elapsed: f32,
}

impl Gesture {
    pub(super) fn salute_frame(&self, frames: u32, still: bool) -> u32 {
        // Lift the cap once, hold it over the heart, then put it back on.
        let last = frames.saturating_sub(1);
        if still {
            return last;
        }
        let phase = (self.elapsed / 1.2)
            .min(1.0)
            .min(((GESTURE_SECONDS - self.elapsed) / 0.8).clamp(0.0, 1.0));
        (phase * last as f32).round() as u32
    }
}

fn mirror_cell(layout: &ShopLayout) -> Option<usize> {
    layout
        .cells
        .iter()
        .position(|c| c.decor.as_deref() == Some("mirror"))
}

fn queue_end(layout: &ShopLayout, queue: &Queue, slot: usize) -> Vec2 {
    let tail = queue.0.len().max(1);
    layout.queue_pos(tail + slot, tail + 2)
}

fn completed_before_tonight(
    data: &GameData,
    progress: &Progress,
    report: &NightReport,
    id: &str,
) -> bool {
    data.stories.iter().enumerate().any(|(i, story)| {
        story.ghost == id
            && !story.chapters.is_empty()
            && progress.chapter(i) >= story.chapters.len()
            && !report.story_done.contains(&i)
            && !progress
                .journal
                .chapter_nights
                .get(&i)
                .is_some_and(|nights| nights.last() == Some(&progress.night))
    })
}

fn invite_returning_guests(
    mut commands: Commands,
    data: Res<GameData>,
    layout: Res<ShopLayout>,
    queue: Res<Queue>,
    mut progress: ResMut<Progress>,
    clock: Res<NightClock>,
    report: Res<NightReport>,
    mut invited: ResMut<Invitations>,
    guests: Query<&Ghost>,
    mut visitors: MessageWriter<Visitor>,
    mut art: GhostArt,
    lang: Res<Lang>,
    mut rng: ResMut<Rng>,
) {
    if clock.elapsed < clock.length * RETURN_AT || clock.is_over() {
        return;
    }
    let active = |id: &str| {
        guests.iter().any(|g| {
            data.ghosts[g.def].id == id
                && !matches!(
                    g.state,
                    GhostState::Leaving | GhostState::Ascending | GhostState::Fleeing
                )
        })
    };
    // A completed campaign can still see Edwin's memory when a mirror is placed later.
    if !invited.mirror
        && !progress.journal.edwin_mirror_seen
        && let Some(cell) = mirror_cell(&layout)
        && completed_before_tonight(&data, &progress, &report, "edwin")
        && !active("edwin")
    {
        invited.mirror = true;
        visitors.write(Visitor {
            ghost: "edwin".into(),
            spot: layout.cell_pos(cell) + Vec2::Y * 0.68,
        });
    }
    if invited.quay
        || progress.journal.ida_sailor_met
        || !completed_before_tonight(&data, &progress, &report, "ida")
        || active("ida")
    {
        return;
    }
    let sailor_waiting = queue
        .0
        .last()
        .and_then(|e| guests.get(*e).ok())
        .is_some_and(|g| data.ghosts[g.def].id == "sailor");
    if active("sailor") && !sailor_waiting {
        return;
    }
    invited.quay = true;
    let pair: &[&str] = if sailor_waiting {
        &["ida"]
    } else {
        &["ida", "sailor"]
    };
    for (slot, id) in pair.iter().enumerate() {
        let Some(def_idx) = data.ghost_index(id) else {
            continue;
        };
        let visit = Visit {
            kind: VisitKind::Visit,
            first_customer: false,
            def_idx,
            wish: String::new(),
            hint: String::new(),
            chapter: None,
            plain: false,
            order: None,
            flees: false,
            patience_factor: 1.0,
            regular: None,
            spot: Some(queue_end(&layout, &queue, slot)),
        };
        let entity = spawn_ghost(
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
        commands.entity(entity).insert(AtQueueEnd(slot));
    }
}

fn join_queue(
    layout: Res<ShopLayout>,
    queue: Res<Queue>,
    mut visitors: Query<(&AtQueueEnd, &mut Ghost)>,
) {
    for (slot, mut ghost) in &mut visitors {
        if matches!(ghost.state, GhostState::Entering | GhostState::Browsing) {
            ghost.target = queue_end(&layout, &queue, slot.0);
        }
    }
}

fn witness_meeting(
    mut commands: Commands,
    data: Res<GameData>,
    mut progress: ResMut<Progress>,
    queue: Res<Queue>,
    ghosts: Query<&Ghost>,
    visitors: Query<(Entity, &AtQueueEnd)>,
    lang: Res<Lang>,
    mut log: ResMut<GameLog>,
    mut cues: MessageWriter<Cue>,
) {
    if progress.journal.ida_sailor_met || crate::harbor::lighthouse_level(&data, &progress) < 1.0 {
        return;
    }
    let mut tail: Vec<_> = visitors.iter().collect();
    tail.sort_by_key(|(_, place)| place.0);
    let line: Vec<_> = queue
        .0
        .iter()
        .copied()
        .chain(tail.iter().map(|(e, _)| *e))
        .collect();
    for pair in line.windows(2) {
        let Ok([left, right]) = ghosts.get_many([pair[0], pair[1]]) else {
            continue;
        };
        let ready = |g: &Ghost| {
            matches!(
                g.state,
                GhostState::Waiting | GhostState::AtRegister | GhostState::Browsing
            ) && g.arrived()
        };
        if !ready(left) || !ready(right) {
            continue;
        }
        let (ida, sailor, voice) = match (
            data.ghosts[left.def].id.as_str(),
            data.ghosts[right.def].id.as_str(),
        ) {
            ("ida", "sailor") => (pair[0], pair[1], data.ghosts[right.def].voice),
            ("sailor", "ida") => (pair[1], pair[0], data.ghosts[left.def].voice),
            _ => continue,
        };
        progress.journal.ida_sailor_met = true;
        commands.entity(ida).insert(Gesture {
            act: motion::Act::Nod,
            elapsed: 0.0,
        });
        commands.entity(sailor).insert(Gesture {
            act: motion::Act::Salute,
            elapsed: 0.0,
        });
        log.push(lang.t("memory.ida_sailor"));
        cues.write(Cue::GhostVoice(Voice::Whisper, voice));
        break;
    }
}

fn finish_gestures(
    mut commands: Commands,
    time: Res<Time>,
    mut gestures: Query<(Entity, &mut Gesture)>,
) {
    for (entity, mut gesture) in &mut gestures {
        gesture.elapsed += time.delta_secs();
        if gesture.elapsed >= GESTURE_SECONDS {
            commands.entity(entity).remove::<Gesture>();
        }
    }
}

#[derive(Component)]
struct GlassMemory {
    guest: Entity,
    opacity: f32,
}
#[derive(Component)]
struct RainStroke {
    lane: f32,
    phase: f32,
}

fn reveal_mirror(
    mut commands: Commands,
    data: Res<GameData>,
    layout: Res<ShopLayout>,
    mut progress: ResMut<Progress>,
    guests: Query<(Entity, &Ghost)>,
    lang: Res<Lang>,
    mut log: ResMut<GameLog>,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if progress.journal.edwin_mirror_seen {
        return;
    }
    let Some(cell) = mirror_cell(&layout) else {
        return;
    };
    let Some((story, _)) = data
        .stories
        .iter()
        .enumerate()
        .find(|(_, s)| s.ghost == "edwin")
    else {
        return;
    };
    if progress.chapter(story) == 0 {
        return;
    }
    let Some((guest, _)) = guests.iter().find(|(_, g)| {
        data.ghosts[g.def].id == "edwin"
            && g.chapter
                .is_none_or(|(_, arriving_chapter)| arriving_chapter > 0)
            && matches!(
                g.state,
                GhostState::Entering
                    | GhostState::Browsing
                    | GhostState::ToCounter
                    | GhostState::Waiting
                    | GhostState::AtRegister
            )
    }) else {
        return;
    };
    progress.journal.edwin_mirror_seen = true;
    log.push(lang.t("memory.edwin_rain"));
    let painting = [RAIN_PAINTING, "sprites/ghosts/edwin.png"]
        .into_iter()
        .find(|p| asset_exists(p));
    let glass_z = if asset_exists("models/decor/mirror.glb") {
        0.122
    } else {
        0.155
    };
    let glass = commands
        .spawn((
            GlassMemory {
                guest,
                opacity: 0.0,
            },
            Mesh3d(meshes.add(Ellipse::new(GLASS_RADII.x, GLASS_RADII.y))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color_texture: painting.map(|path| assets.load(path)),
                base_color: Color::srgba(0.94, 0.91, 0.88, 0.0),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            })),
            Transform::from_translation(
                floor(layout.cell_pos(cell), GLASS_HEIGHT) + Vec3::Z * glass_z,
            ),
            NotShadowCaster,
            Pickable::IGNORE,
            DespawnOnExit(Phase::Night),
        ))
        .id();
    let stroke = meshes.add(Rectangle::new(0.006, 0.060));
    let silver = materials.add(StandardMaterial {
        base_color: Color::srgba(0.79, 0.88, 0.98, 0.55),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    for (i, lane) in [-0.78, -0.57, -0.38, -0.16, 0.16, 0.36, 0.59, 0.78]
        .into_iter()
        .enumerate()
    {
        commands.spawn((
            RainStroke {
                lane,
                phase: (i as f32 * 0.38197).fract(),
            },
            Mesh3d(stroke.clone()),
            MeshMaterial3d(silver.clone()),
            Transform::default(),
            Visibility::Hidden,
            NotShadowCaster,
            Pickable::IGNORE,
            ChildOf(glass),
        ));
    }
}

fn animate_glass(
    mut commands: Commands,
    time: Res<Time>,
    settings: Res<Settings>,
    guests: Query<&Ghost>,
    mut panes: Query<(Entity, &mut GlassMemory, &MeshMaterial3d<StandardMaterial>)>,
    mut rain: Query<(&RainStroke, &ChildOf, &mut Transform, &mut Visibility)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, mut pane, handle) in &mut panes {
        let present = guests
            .get(pane.guest)
            .is_ok_and(|g| !matches!(g.state, GhostState::Leaving | GhostState::Ascending));
        let target = if present { 1.0 } else { 0.0 };
        let alpha = if settings.reduced_motion {
            target
        } else {
            pane.opacity.move_towards(target, time.delta_secs() * 1.5)
        };
        if !present && alpha == 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        if alpha != pane.opacity {
            pane.opacity = alpha;
            if let Some(mut mat) = materials.get_mut(handle) {
                mat.base_color.set_alpha(alpha * 0.96);
            }
        }
    }
    for (drop, parent, mut transform, mut visible) in &mut rain {
        let show = !settings.reduced_motion
            && panes
                .get(parent.parent())
                .is_ok_and(|(_, p, _)| p.opacity > 0.7);
        *visible = if show {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if show {
            // Each stroke runs inside a chord of the oval, including its two tips.
            let travel = GLASS_RADII.y * (1.0 - drop.lane.powi(2)).sqrt() - 0.055;
            let fraction = (time.elapsed_secs() * 0.72 + drop.phase).fract();
            transform.translation = Vec3::new(
                drop.lane * GLASS_RADII.x,
                travel * (1.0 - 2.0 * fraction),
                0.005,
            );
            transform.rotation = Quat::from_rotation_z(-0.12);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mirror_waits_for_a_placed_mirror_and_a_different_edwin_visit() {
        let data = GameData::load();
        let layout = ShopLayout::new(data.config.grid_cols, data.config.grid_rows);
        let def = data.ghost_index("edwin").unwrap();
        let story = data
            .stories
            .iter()
            .position(|s| s.ghost == "edwin")
            .unwrap();
        let mut progress = Progress::new(0);
        progress.story.resize(story + 1, 0);
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Image>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .insert_resource(data)
            .insert_resource(progress)
            .insert_resource(layout)
            .insert_resource(Lang::load("en"))
            .init_resource::<GameLog>()
            .add_systems(Update, reveal_mirror);
        let mut edwin = Ghost::waiting(def, "rain_jar");
        edwin.chapter = Some((story, 0));
        let guest = app.world_mut().spawn(edwin).id();
        app.update();
        app.world_mut().resource_mut::<Progress>().story[story] = 1;
        app.world_mut().resource_mut::<ShopLayout>().cells[0].decor = Some("mirror".into());
        app.update();
        assert!(!app.world().resource::<Progress>().journal.edwin_mirror_seen);
        app.world_mut().get_mut::<Ghost>(guest).unwrap().chapter = Some((story, 1));
        app.world_mut().resource_mut::<ShopLayout>().cells[0].decor = None;
        app.update();
        assert!(!app.world().resource::<Progress>().journal.edwin_mirror_seen);
        app.world_mut().resource_mut::<ShopLayout>().cells[1].decor = Some("mirror".into());
        app.update();
        app.update();
        let world = app.world_mut();
        assert!(world.resource::<Progress>().journal.edwin_mirror_seen);
        assert_eq!(world.query::<&GlassMemory>().iter(world).count(), 1);
        assert_eq!(world.query::<&RainStroke>().iter(world).count(), 8);
    }

    #[test]
    fn only_neighbouring_guests_at_the_repaired_lighthouse_unlock_the_exchange() {
        let data = GameData::load();
        // A staged demo has no lighthouse storyline.
        let Some(story) = data.stories.iter().position(|s| s.lighthouse) else {
            return;
        };
        let chapters = data.stories[story].chapters.len();
        let mut progress = Progress::new(0);
        progress.story.resize(story + 1, 0);
        let mut app = App::new();
        let ids = ["sailor", "shy", "ida"].map(|id| data.ghost_index(id).unwrap());
        let [sailor, shy, ida] = ids.map(|i| app.world_mut().spawn(Ghost::waiting(i, "")).id());
        app.insert_resource(data)
            .insert_resource(progress)
            .insert_resource(Queue(vec![sailor, ida]))
            .insert_resource(Lang::load("en"))
            .init_resource::<GameLog>()
            .add_message::<Cue>()
            .add_systems(Update, witness_meeting);
        app.update();
        assert!(!app.world().resource::<Progress>().journal.ida_sailor_met);
        app.world_mut().resource_mut::<Progress>().story[story] = chapters;
        app.world_mut().resource_mut::<Queue>().0.insert(1, shy);
        app.update();
        assert!(!app.world().resource::<Progress>().journal.ida_sailor_met);
        app.world_mut().resource_mut::<Queue>().remove(shy);
        app.world_mut().get_mut::<Ghost>(ida).unwrap().state = GhostState::ToCounter;
        app.update();
        assert!(!app.world().resource::<Progress>().journal.ida_sailor_met);
        app.world_mut().get_mut::<Ghost>(ida).unwrap().state = GhostState::Waiting;
        app.update();
        assert!(app.world().resource::<Progress>().journal.ida_sailor_met);
        assert_eq!(
            app.world().get::<Gesture>(sailor).unwrap().act,
            motion::Act::Salute
        );
        assert_eq!(
            app.world().get::<Gesture>(ida).unwrap().act,
            motion::Act::Nod
        );
        app.world_mut().get_mut::<Gesture>(sailor).unwrap().elapsed = 2.;
        app.update();
        let held = app.world().get::<Gesture>(sailor).unwrap();
        assert_eq!(held.salute_frame(4, false), 3);
        assert_eq!(held.salute_frame(4, true), 3);
    }
}
