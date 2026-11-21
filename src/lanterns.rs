//! Life after the ending: the old lanterns along the quay, dark since Hedda's
//! time, can be restored one at a time with the shop's savings
//! (`assets/data/lanterns.ron`). It is optional and never needed for an
//! ending; it only opens once the campaign is complete.
//!
//! Each restored lantern shines over the harbour from that evening on, and
//! the book remembers the night it was lit and who came to the shop that
//! night (`Journal::lanterns`, filled at dawn).
//!
//! Model: `assets/models/harbor/quay_lantern.glb` (the post with an unlit
//! lantern housing, origin at the foot). The glow and the light are added
//! here; without the model a simple post stands in.

use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::accounts::Accounts;
use crate::data::{GameData, asset_exists};
use crate::day_cycle::{NightReport, Phase, Progress};
use crate::economy::Drawer;
use crate::fx::Flicker;
use crate::harbor::HARBOR;
use crate::i18n::Lang;
use crate::ui::{Action, GameLog, PanelBuilder};

pub struct LanternsPlugin;

impl Plugin for LanternsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_lanterns)
            .add_systems(
                OnEnter(Phase::Morning),
                remember_visitors.before(crate::save::autosave_dawn),
            )
            .add_systems(
                Update,
                (restore.run_if(in_state(Phase::Evening)), light_lanterns),
            );
    }
}

/// One lantern on the quay.
#[derive(Deserialize, Clone, Debug)]
pub struct LanternDef {
    /// Where it stands, relative to the harbour.
    pub pos: (f32, f32, f32),
    /// What restoring it costs (button units).
    pub cost: u32,
}

/// A restored lantern (in the journal, in the order they were restored).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LanternRecord {
    /// The night it was first lit.
    pub night: u32,
    /// Ghosts who passed into the light that night (ghost ids).
    pub visitors: Vec<String>,
}

/// How many visitors a lantern's page names.
const MAX_VISITORS: usize = 4;
/// Height of the lantern glass above the foot of the post.
const GLASS_HEIGHT: f32 = 1.95;
const POST_SIZE: Vec3 = Vec3::new(0.3, 2.2, 0.3);
const MODEL: &str = "models/harbor/quay_lantern.glb";
const LUMENS: f32 = 48_000.0;
const RANGE: f32 = 4.5;
const GLOW: LinearRgba = LinearRgba::rgb(5.0, 2.9, 1.0);
const LIT_GLASS: Color = Color::srgb(1.0, 0.78, 0.45);
const DARK_GLASS: Color = Color::srgb(0.13, 0.13, 0.12);

/// The lantern open for restoration now, if any: (index, cost).
pub fn next(data: &GameData, progress: &Progress) -> Option<(usize, u32)> {
    if !progress.journal.campaign_complete {
        return None;
    }
    let i = progress.journal.lanterns.len();
    data.lanterns.get(i).map(|l| (i, l.cost))
}

/// One a night: was a lantern restored this evening?
fn restored_tonight(progress: &Progress) -> bool {
    progress
        .journal
        .lanterns
        .last()
        .is_some_and(|l| l.night == progress.night)
}

/// The harbour panel's section (only after the campaign).
pub fn harbor_section(
    ui: &mut PanelBuilder,
    lang: &Lang,
    data: &GameData,
    progress: &Progress,
    drawer: &Drawer,
) {
    if !progress.journal.campaign_complete || data.lanterns.is_empty() {
        return;
    }
    let lit = progress.journal.lanterns.len();
    ui.section(&lang.f(
        "harbor.lanterns",
        &[("lit", &lit), ("total", &data.lanterns.len())],
    ));
    if restored_tonight(progress) {
        ui.dim(&lang.t("harbor.lanterns_tonight"));
    } else if let Some((_, cost)) = next(data, progress) {
        ui.dim(&lang.t("harbor.lanterns_hint"));
        let root = ui.root();
        ui.button_in(
            root,
            &lang.f("harbor.lanterns_restore", &[("cost", &cost)]),
            Action::RestoreLantern,
            false,
            drawer.total() >= cost,
        );
    } else {
        ui.dim(&lang.t("harbor.lanterns_done"));
    }
}

fn restore(
    mut actions: MessageReader<Action>,
    data: Res<GameData>,
    lang: Res<Lang>,
    mut progress: ResMut<Progress>,
    mut drawer: ResMut<Drawer>,
    mut accounts: ResMut<Accounts>,
    mut log: ResMut<GameLog>,
) {
    for action in actions.read() {
        if *action != Action::RestoreLantern || restored_tonight(&progress) {
            continue;
        }
        let Some((i, cost)) = next(&data, &progress) else {
            continue;
        };
        if !drawer.pay(cost) {
            continue;
        }
        accounts.upgrades += cost;
        let night = progress.night;
        progress.journal.lanterns.push(LanternRecord {
            night,
            visitors: Vec::new(),
        });
        log.push(lang.f(
            "log.lantern_restored",
            &[("n", &(i + 1)), ("total", &data.lanterns.len())],
        ));
    }
}

/// At dawn: who came on the night a lantern was first lit.
fn remember_visitors(mut progress: ResMut<Progress>, report: Res<NightReport>) {
    let night = progress.night;
    let Some(record) = progress
        .journal
        .lanterns
        .last_mut()
        .filter(|l| l.night == night && l.visitors.is_empty())
    else {
        return;
    };
    record.visitors = visitors(&report.redeemed);
}

fn visitors(redeemed: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for id in redeemed {
        if out.len() < MAX_VISITORS && !out.contains(id) {
            out.push(id.clone());
        }
    }
    out
}

#[derive(Component)]
struct QuayLantern;

/// The glass of a lantern (its material changes when it is lit).
#[derive(Component)]
struct LanternGlass(usize);

#[derive(Component)]
struct LanternLight(usize);

fn spawn_lanterns(
    mut commands: Commands,
    data: Res<GameData>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let model = asset_exists(MODEL);
    let post = meshes.add(Cylinder::new(0.05, POST_SIZE.y - 0.3));
    let foot = meshes.add(Cylinder::new(0.12, 0.12));
    let cap = meshes.add(Cuboid::new(0.26, 0.05, 0.26));
    let glass = meshes.add(Cuboid::new(0.2, 0.28, 0.2));
    let iron = materials.add(StandardMaterial {
        base_color: Color::srgb(0.1, 0.1, 0.12),
        perceptual_roughness: 0.7,
        ..default()
    });
    for (i, l) in data.lanterns.iter().enumerate() {
        let at = HARBOR + Vec3::new(l.pos.0, l.pos.1, l.pos.2);
        let root = commands
            .spawn((
                QuayLantern,
                Transform::from_translation(at),
                Visibility::default(),
            ))
            .id();
        if model {
            commands.spawn((
                WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(MODEL))),
                Transform::default(),
                ChildOf(root),
            ));
        } else {
            for (mesh, y) in [
                (post.clone(), (POST_SIZE.y - 0.3) / 2.0),
                (foot.clone(), 0.06),
                (cap.clone(), GLASS_HEIGHT + 0.16),
            ] {
                commands.spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(iron.clone()),
                    Transform::from_xyz(0.0, y, 0.0),
                    Pickable::IGNORE,
                    ChildOf(root),
                ));
            }
        }
        // The glass: dark until restored, then warm (`light_lanterns`).
        commands.spawn((
            LanternGlass(i),
            Mesh3d(glass.clone()),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: DARK_GLASS,
                ..default()
            })),
            Transform::from_xyz(0.0, GLASS_HEIGHT, 0.0).with_scale(if model {
                Vec3::splat(0.8)
            } else {
                Vec3::ONE
            }),
            NotShadowCaster,
            Pickable::IGNORE,
            ChildOf(root),
        ));
        commands.spawn((
            LanternLight(i),
            PointLight {
                color: Color::srgb(1.0, 0.74, 0.44),
                intensity: LUMENS,
                range: RANGE,
                radius: 0.1,
                ..default()
            },
            Flicker {
                base: LUMENS,
                phase: i as f32 * 1.7,
            },
            Visibility::Hidden,
            Transform::from_xyz(0.0, GLASS_HEIGHT, 0.12),
            ChildOf(root),
        ));
    }
}

/// Restored lanterns glow; the others stay dark (also after loading).
fn light_lanterns(
    progress: Res<Progress>,
    mut shown: Local<Option<usize>>,
    glasses: Query<(&LanternGlass, &MeshMaterial3d<StandardMaterial>)>,
    mut lights: Query<(&LanternLight, &mut Visibility)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let lit = progress.journal.lanterns.len();
    if *shown == Some(lit) || glasses.is_empty() {
        return;
    }
    *shown = Some(lit);
    for (glass, handle) in &glasses {
        if let Some(mut m) = materials.get_mut(&handle.0) {
            let on = glass.0 < lit;
            m.base_color = if on { LIT_GLASS } else { DARK_GLASS };
            m.emissive = if on { GLOW } else { LinearRgba::BLACK };
        }
    }
    for (light, mut visibility) in &mut lights {
        *visibility = if light.0 < lit {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

/// The book's page of restored lanterns (letters chapter).
pub fn book_section(
    ui: &mut PanelBuilder,
    lang: &Lang,
    data: &GameData,
    journal: &crate::book::Journal,
) {
    let records = &journal.lanterns;
    if records.is_empty() {
        return;
    }
    ui.rule();
    ui.section(&lang.f(
        "book.lanterns",
        &[("lit", &records.len()), ("total", &data.lanterns.len())],
    ));
    for (i, r) in records.iter().enumerate() {
        let who = if r.visitors.is_empty() {
            lang.t("book.lantern_nobody")
        } else {
            r.visitors
                .iter()
                .map(|g| lang.ghost(g))
                .collect::<Vec<_>>()
                .join(", ")
        };
        ui.text(&lang.f(
            "book.lantern_entry",
            &[("n", &(i + 1)), ("night", &r.night), ("who", &who)],
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lanterns_open_after_the_campaign_one_a_night() {
        let data = GameData::load();
        assert!(data.lanterns.len() >= 3, "a row of lanterns");
        for pair in data.lanterns.windows(2) {
            assert!(pair[0].cost <= pair[1].cost, "costs rise along the quay");
        }
        let mut progress = Progress::new(0);
        progress.night = 31;
        assert_eq!(next(&data, &progress), None, "not before the ending");
        progress.journal.campaign_complete = true;
        assert_eq!(next(&data, &progress), Some((0, data.lanterns[0].cost)));
        progress.journal.lanterns.push(LanternRecord {
            night: 31,
            visitors: Vec::new(),
        });
        assert!(restored_tonight(&progress));
        progress.night = 32;
        assert!(!restored_tonight(&progress));
        assert_eq!(next(&data, &progress), Some((1, data.lanterns[1].cost)));
        for n in 1..data.lanterns.len() {
            progress.journal.lanterns.push(LanternRecord {
                night: 32 + n as u32,
                visitors: Vec::new(),
            });
        }
        assert_eq!(next(&data, &progress), None, "all lit");
    }

    #[test]
    fn a_lantern_remembers_a_few_different_visitors() {
        let ids: Vec<String> = ["cat", "cat", "shy", "frog", "knight", "bride"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(visitors(&ids), ["cat", "shy", "frog", "knight"]);
    }
}
