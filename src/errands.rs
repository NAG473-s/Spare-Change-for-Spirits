//! Lost-and-found errands (`assets/data/errands.ron`): once a ghost's story
//! is finished, it may ask for help with something it lost. Clues are found
//! by clicking tracks in the scene – at the harbour in the evening or at the
//! market by day – and lead to a damaged keepsake. The workbench mends it,
//! the player gives it back, and the ghost remembers that in the book. No
//! step is timed: every step can wait for another evening.
//!
//! Models (optional): `assets/models/errands/<errand>_clue_<n>.glb`,
//! otherwise small paw prints built from primitives. The glow under the
//! tracks pulses in code.

use bevy::light::NotShadowCaster;
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::data::{GameData, asset_exists};
use crate::day_cycle::{Phase, Progress};
use crate::economy::Drawer;
use crate::harbor::{HARBOR, Location};
use crate::i18n::Lang;
use crate::market::MARKET;
use crate::shop::{ShopLayout, Storeroom};
use crate::tooltip::{HoverTip, TipKind};
use crate::ui::{Action, GameLog};

pub struct ErrandsPlugin;

impl Plugin for ErrandsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Phase::Evening), announce)
            .add_systems(
                Update,
                (
                    show_clues,
                    follow_clues,
                    pulse_clues,
                    clue_tooltip,
                    act.run_if(in_state(Phase::Evening)),
                ),
            );
    }
}

/// One errand. Texts: `errand.<id>.title`, `.start`, `.hint.<n>`,
/// `.clue.<n>`, `.repair_hint`, `.return`, `.thanks`, `.memory`.
#[derive(Deserialize, Clone, Debug)]
pub struct ErrandDef {
    pub id: String,
    /// The ghost who asks (its story must be finished first).
    pub ghost: String,
    /// The lost keepsake (decor with `keepsake: true`).
    pub item: String,
    /// What mending it at the workbench costs (button units).
    pub repair_cost: u32,
    pub clues: Vec<ClueDef>,
}

/// Where a clue lies: (x, y, z) relative to the harbour or the market.
#[derive(Deserialize, Clone, Debug)]
pub struct ClueDef {
    pub place: CluePlace,
    pub pos: (f32, f32, f32),
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum CluePlace {
    /// On the quay, in the evening.
    Harbor,
    /// On the market square, by day.
    Market,
}

/// How far an errand has come (saved in the journal).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ErrandState {
    pub clues: usize,
    pub repaired: bool,
    /// The night the keepsake was given back.
    pub returned: Option<u32>,
}

impl ErrandDef {
    /// The errand can be done: its ghost's story is finished.
    pub fn open(&self, data: &GameData, progress: &Progress) -> bool {
        data.stories
            .iter()
            .enumerate()
            .any(|(i, s)| s.ghost == self.ghost && progress.chapter(i) >= s.chapters.len())
    }

    pub fn found(&self, state: &ErrandState) -> bool {
        state.clues >= self.clues.len()
    }
}

/// An errand's state in the journal (or its start).
pub fn state<'a>(progress: &'a Progress, id: &str) -> Option<&'a ErrandState> {
    progress.journal.errands.get(id)
}

/// The first evening an errand is open: the ghost asks for help.
fn announce(
    data: Res<GameData>,
    lang: Res<Lang>,
    mut progress: ResMut<Progress>,
    mut log: ResMut<GameLog>,
) {
    for e in &data.errands {
        if e.open(&data, &progress) && !progress.journal.errands.contains_key(&e.id) {
            progress
                .journal
                .errands
                .insert(e.id.clone(), ErrandState::default());
            log.push(lang.t(&format!("errand.{}.start", e.id)));
        }
    }
}

/// Tracks of the clue that is next, where and when it can be found.
#[derive(Component)]
struct Clue {
    errand: usize,
}

#[derive(Component)]
struct ClueGlow;

#[derive(Component)]
struct HoveredClue;

/// Size of the clickable area around a clue.
const CLUE_HITBOX: Vec3 = Vec3::new(1.1, 0.4, 0.8);

/// Which clue is next for each errand, if it can be found here and now.
fn wanted_clues(
    data: &GameData,
    progress: &Progress,
    phase: Phase,
    location: Location,
) -> Vec<(usize, usize)> {
    data.errands
        .iter()
        .enumerate()
        .filter_map(|(i, e)| {
            let state = state(progress, &e.id)?;
            let n = state.clues;
            let clue = e.clues.get(n)?;
            let here = match clue.place {
                CluePlace::Harbor => phase == Phase::Evening && location == Location::Harbor,
                CluePlace::Market => phase == Phase::Day && location == Location::Market,
            };
            here.then_some((i, n))
        })
        .collect()
}

fn show_clues(
    mut commands: Commands,
    data: Res<GameData>,
    progress: Res<Progress>,
    phase: Res<State<Phase>>,
    location: Res<Location>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    shown: Query<(Entity, &Clue)>,
    mut last: Local<Vec<(usize, usize)>>,
) {
    let wanted = wanted_clues(&data, &progress, *phase.get(), *location);
    if wanted == *last && shown.iter().count() == wanted.len() {
        return;
    }
    *last = wanted.clone();
    for (e, _) in &shown {
        commands.entity(e).despawn();
    }
    for (i, n) in wanted {
        let def = &data.errands[i];
        let clue = &def.clues[n];
        let origin = match clue.place {
            CluePlace::Harbor => HARBOR,
            CluePlace::Market => MARKET,
        };
        let base = origin + Vec3::from(clue.pos);
        let root = commands
            .spawn((
                Clue { errand: i },
                Transform::from_translation(base),
                Visibility::default(),
            ))
            .observe(on_clue_click)
            .observe(on_clue_over)
            .observe(on_clue_out)
            .id();
        let model = format!("models/errands/{}_clue_{}.glb", def.id, n + 1);
        if asset_exists(&model) {
            commands.spawn((
                WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(model))),
                Transform::default(),
                ChildOf(root),
            ));
        } else {
            // A little trail of paw prints.
            let print = meshes.add(Sphere::new(0.07).mesh().ico(1).unwrap());
            let dark = materials.add(StandardMaterial {
                base_color: Color::srgb(0.18, 0.17, 0.2),
                perceptual_roughness: 0.9,
                ..default()
            });
            for k in 0..4 {
                let side = if k % 2 == 0 { -0.08 } else { 0.08 };
                commands.spawn((
                    Mesh3d(print.clone()),
                    MeshMaterial3d(dark.clone()),
                    Transform::from_xyz(-0.36 + k as f32 * 0.24, 0.01, side)
                        .with_scale(Vec3::new(1.0, 0.15, 0.8)),
                    NotShadowCaster,
                    Pickable::IGNORE,
                    ChildOf(root),
                ));
            }
        }
        // A soft glow so the tracks can be spotted (it pulses).
        commands.spawn((
            ClueGlow,
            Mesh3d(meshes.add(Circle::new(0.6))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgba(0.85, 0.92, 1.0, 0.25),
                emissive: LinearRgba::rgb(0.4, 0.5, 0.7),
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                ..default()
            })),
            Transform::from_xyz(0.0, 0.005, 0.0)
                .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
            NotShadowCaster,
            Pickable::IGNORE,
            ChildOf(root),
        ));
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::from_size(CLUE_HITBOX))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::NONE,
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            })),
            Transform::from_xyz(0.0, CLUE_HITBOX.y / 2.0, 0.0),
            NotShadowCaster,
            ChildOf(root),
        ));
    }
}

fn pulse_clues(
    time: Res<Time<Real>>,
    clues: Query<(&Clue, &Children)>,
    mut glows: Query<&mut Transform, With<ClueGlow>>,
) {
    let k = 1.0 + 0.12 * (time.elapsed_secs() * 2.2).sin();
    for (_, children) in &clues {
        for child in children.iter() {
            if let Ok(mut t) = glows.get_mut(child) {
                t.scale = Vec3::splat(k);
            }
        }
    }
}

/// Clicking the tracks finds the clue (as does the panel's button).
fn on_clue_click(click: On<PointerClick>, clues: Query<&Clue>, mut actions: MessageWriter<Action>) {
    if click.button != PointerButton::Primary {
        return;
    }
    if let Ok(clue) = clues.get(click.event_target()) {
        actions.write(Action::FollowClue(clue.errand));
    }
}

fn follow_clues(
    mut actions: MessageReader<Action>,
    data: Res<GameData>,
    lang: Res<Lang>,
    phase: Res<State<Phase>>,
    location: Res<Location>,
    mut progress: ResMut<Progress>,
    mut log: ResMut<GameLog>,
    mut tip: ResMut<HoverTip>,
) {
    for action in actions.read() {
        let Action::FollowClue(errand) = action else {
            continue;
        };
        // Only the clue that lies here and now.
        if !wanted_clues(&data, &progress, *phase.get(), *location)
            .iter()
            .any(|(i, _)| i == errand)
        {
            continue;
        }
        let def = &data.errands[*errand];
        let Some(state) = progress.journal.errands.get_mut(&def.id) else {
            continue;
        };
        state.clues += 1;
        log.push(lang.t(&format!("errand.{}.clue.{}", def.id, state.clues)));
        HoverTip::hide(&mut tip);
    }
}

/// A button for each clue that can be found here and now (keyboard and
/// controller players; the tracks can also be clicked in the scene).
pub fn clue_buttons(
    ui: &mut crate::ui::PanelBuilder,
    lang: &Lang,
    data: &GameData,
    progress: &Progress,
    phase: Phase,
    location: Location,
) {
    for (i, _) in wanted_clues(data, progress, phase, location) {
        let root = ui.root();
        ui.button_in(
            root,
            &lang.t("errand.follow_tracks"),
            Action::FollowClue(i),
            false,
            true,
        );
    }
}

fn on_clue_over(over: On<PointerOver>, mut commands: Commands) {
    commands.entity(over.event_target()).insert(HoveredClue);
}

fn on_clue_out(out: On<PointerOut>, mut commands: Commands, mut tip: ResMut<HoverTip>) {
    commands.entity(out.event_target()).remove::<HoveredClue>();
    HoverTip::hide(&mut tip);
}

fn clue_tooltip(
    hovered: Query<&Clue, With<HoveredClue>>,
    lang: Res<Lang>,
    mut tip: ResMut<HoverTip>,
    mut shown: Local<bool>,
) {
    if hovered.iter().next().is_none() {
        // The tracks were found (and removed) while the pointer was on them.
        if std::mem::take(&mut *shown) {
            HoverTip::hide(&mut tip);
        }
        return;
    }
    *shown = true;
    let mut t = HoverTip {
        title: lang.t("errand.clue_title"),
        ..default()
    };
    t.line(TipKind::Hint, lang.t("errand.clue_hint"));
    HoverTip::show(&mut tip, t);
}

/// Mend the found keepsake at the workbench, then give it back.
fn act(
    mut actions: MessageReader<Action>,
    data: Res<GameData>,
    lang: Res<Lang>,
    layout: Res<ShopLayout>,
    mut progress: ResMut<Progress>,
    mut drawer: ResMut<Drawer>,
    mut storeroom: ResMut<Storeroom>,
    mut log: ResMut<GameLog>,
) {
    for action in actions.read() {
        let night = progress.night;
        match action {
            Action::RepairErrand(i) => {
                let Some(def) = data.errands.get(*i) else {
                    continue;
                };
                let can = crate::workbench::has_workbench(&layout)
                    && progress.repaired_on != night
                    && progress
                        .journal
                        .errands
                        .get(&def.id)
                        .is_some_and(|s| def.found(s) && !s.repaired);
                if !can || !drawer.pay(def.repair_cost) {
                    continue;
                }
                progress.repaired_on = night;
                if let Some(s) = progress.journal.errands.get_mut(&def.id) {
                    s.repaired = true;
                }
                log.push(lang.f("log.repaired_keepsake", &[("item", &lang.decor(&def.item))]));
            }
            Action::ReturnErrand(i) => {
                let Some(def) = data.errands.get(*i) else {
                    continue;
                };
                let Some(s) = progress.journal.errands.get_mut(&def.id) else {
                    continue;
                };
                if !s.repaired || s.returned.is_some() {
                    continue;
                }
                s.returned = Some(night);
                storeroom.give(&def.item);
                log.push(lang.f(
                    "log.story_says",
                    &[
                        ("name", &lang.ghost(&def.ghost)),
                        ("text", &lang.t(&format!("errand.{}.thanks", def.id))),
                    ],
                ));
            }
            _ => {}
        }
    }
}

/// The errand's part of the evening panel: where to look next, mending it
/// and giving it back – or a note that it can wait.
pub fn evening_section(
    ui: &mut crate::ui::PanelBuilder,
    lang: &Lang,
    data: &GameData,
    progress: &Progress,
    layout: &ShopLayout,
    drawer: &Drawer,
) {
    for (i, def) in data.errands.iter().enumerate() {
        let Some(s) = state(progress, &def.id) else {
            continue;
        };
        if s.returned.is_some() {
            continue;
        }
        ui.section(&lang.t(&format!("errand.{}.title", def.id)));
        if !def.found(s) {
            ui.text(&lang.t(&format!("errand.{}.hint.{}", def.id, s.clues + 1)));
        } else if !s.repaired {
            ui.text(&lang.t(&format!("errand.{}.repair_hint", def.id)));
            if crate::workbench::has_workbench(layout) {
                let label = lang.f(
                    "errand.repair",
                    &[("item", &lang.decor(&def.item)), ("cost", &def.repair_cost)],
                );
                let enabled =
                    progress.repaired_on != progress.night && drawer.total() >= def.repair_cost;
                let root = ui.root();
                ui.button_in(root, &label, Action::RepairErrand(i), false, enabled);
            } else {
                ui.dim(&lang.t("errand.needs_workbench"));
            }
        } else {
            let root = ui.root();
            ui.button_in(
                root,
                &lang.t(&format!("errand.{}.return", def.id)),
                Action::ReturnErrand(i),
                false,
                true,
            );
        }
        ui.dim(&lang.t("errand.can_wait"));
    }
}

/// At the market: where to look, if the next clue lies here.
pub fn market_hint(
    ui: &mut crate::ui::PanelBuilder,
    lang: &Lang,
    data: &GameData,
    progress: &Progress,
) {
    for def in &data.errands {
        let Some(s) = state(progress, &def.id) else {
            continue;
        };
        if def
            .clues
            .get(s.clues)
            .is_some_and(|c| c.place == CluePlace::Market)
        {
            ui.section(&lang.t(&format!("errand.{}.title", def.id)));
            ui.text(&lang.t(&format!("errand.{}.hint.{}", def.id, s.clues + 1)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lotte_s_errand_opens_after_her_story_and_names_every_step() {
        let data = GameData::load();
        let lang = Lang::load("en");
        let mut progress = Progress::new(0);
        let def = data.errands.iter().find(|e| e.ghost == "lotte").unwrap();
        assert!(!def.open(&data, &progress));
        let i = data
            .stories
            .iter()
            .position(|s| s.ghost == "lotte")
            .unwrap();
        for _ in 0..data.stories[i].chapters.len() {
            progress.finish_chapter(i);
        }
        assert!(def.open(&data, &progress));
        assert!(data.decor(&def.item).is_some_and(|d| d.keepsake));
        let mut keys = vec![
            "title".to_string(),
            "start".into(),
            "repair_hint".into(),
            "return".into(),
            "thanks".into(),
            "memory".into(),
        ];
        for n in 1..=def.clues.len() {
            keys.push(format!("hint.{n}"));
            keys.push(format!("clue.{n}"));
        }
        for k in keys {
            let key = format!("errand.{}.{k}", def.id);
            assert_ne!(lang.t(&key), key, "missing {key}");
        }
    }
}
