//! Small mishaps during the night (`assets/data/incidents.ron`): a draught
//! blows the candles out, a crow flutters in, the roof drips. Each worsens the
//! atmosphere until the player clicks the spot in the shop that shows it.
//! There is no time limit – the shop only feels worse until it is fixed.
//!
//! Models (optional): `assets/models/incidents/<id>.glb`; otherwise a simple
//! coloured placeholder. Every movement (the crow hopping, leaves swirling,
//! drops falling) is done here.

use bevy::light::NotShadowCaster;
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use serde::Deserialize;

use crate::data::{Atmosphere, GameData, asset_exists, rgb};
use crate::day_cycle::{NightClock, Phase, Progress};
use crate::fx::Cue;
use crate::i18n::Lang;
use crate::rng::Rng;
use crate::shop::ShopLayout;
use crate::tooltip::{HoverTip, TipKind};
use crate::ui::{Action, GameLog};

pub struct IncidentsPlugin;

impl Plugin for IncidentsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Mishaps>()
            .add_systems(OnEnter(Phase::Night), plan_mishaps)
            .add_systems(OnExit(Phase::Night), clear_mishaps)
            .add_systems(
                Update,
                (start_mishaps, fix_mishaps, sway_markers, marker_tooltip)
                    .run_if(in_state(Phase::Night)),
            );
    }
}

/// One kind of mishap (`incidents.ron`). Texts: `incident.<id>.name`,
/// `incident.<id>.start`, `incident.<id>.fixed`.
#[derive(Deserialize, Clone, Debug)]
pub struct IncidentDef {
    pub id: String,
    /// Added to the shop's atmosphere until it is fixed.
    pub effect: Atmosphere,
    /// Where it shows in the shop (x, y, z): the origin of its model.
    pub pos: (f32, f32, f32),
    /// Placeholder box size and the model's target size (w, h, d).
    pub size: (f32, f32, f32),
    pub color: (f32, f32, f32),
    /// How often it is picked, relative to the others.
    #[serde(default = "one_f")]
    pub weight: f32,
    /// How the marker moves: "hop", "swirl" or "drip".
    #[serde(default)]
    pub motion: Motion,
}

fn one_f() -> f32 {
    1.0
}

#[derive(Deserialize, Clone, Copy, Debug, Default, PartialEq)]
pub enum Motion {
    #[default]
    Hop,
    Swirl,
    Drip,
}

/// Tonight's mishaps: those still to come (time as a fraction of the night,
/// index in `incidents`), and the effect of those not yet fixed.
#[derive(Resource, Default)]
pub struct Mishaps {
    planned: Vec<(f32, usize)>,
    /// Mishaps happening now, not fixed yet (index in `incidents`); the
    /// night panel offers a button for each.
    pub active: Vec<usize>,
}

/// The clickable spot of a mishap that has not been fixed.
#[derive(Component)]
struct Marker {
    incident: usize,
    base: Vec3,
    phase: f32,
}

/// As the night begins: decide which mishaps come and when.
fn plan_mishaps(
    data: Res<GameData>,
    progress: Res<Progress>,
    mut rng: ResMut<Rng>,
    mut mishaps: ResMut<Mishaps>,
    mut layout: ResMut<ShopLayout>,
) {
    layout.mishaps = Atmosphere::default();
    mishaps.planned.clear();
    mishaps.active.clear();
    let c = data.config.incidents;
    if progress.calendar(&data) < c.from_night || data.incidents.is_empty() {
        return;
    }
    let count = rng.range(c.per_night.0 as f32, c.per_night.1 as f32 + 0.999) as u32;
    let mut weights: Vec<f32> = data.incidents.iter().map(|i| i.weight).collect();
    for _ in 0..count {
        let Some(i) = rng.weighted(&weights) else {
            break;
        };
        // Never the same mishap twice in a night.
        weights[i] = 0.0;
        mishaps
            .planned
            .push((rng.range(c.arrive_at.0, c.arrive_at.1), i));
    }
}

/// A planned mishap happens: its spot appears and the atmosphere worsens.
#[allow(clippy::too_many_arguments)]
fn start_mishaps(
    mut commands: Commands,
    clock: Res<NightClock>,
    data: Res<GameData>,
    lang: Res<Lang>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut mishaps: ResMut<Mishaps>,
    mut layout: ResMut<ShopLayout>,
    mut log: ResMut<GameLog>,
    mut rng: ResMut<Rng>,
) {
    let now = clock.elapsed / clock.length.max(1.0);
    let due: Vec<usize> = mishaps
        .planned
        .iter()
        .filter(|(at, _)| *at <= now)
        .map(|(_, i)| *i)
        .collect();
    if due.is_empty() {
        return;
    }
    mishaps.planned.retain(|(at, _)| *at > now);
    for i in due {
        let def = &data.incidents[i];
        layout.mishaps.add(&def.effect);
        mishaps.active.push(i);
        log.push(lang.t(&format!("incident.{}.start", def.id)));
        let base = Vec3::from(def.pos);
        let size = Vec3::from(def.size);
        let marker = commands
            .spawn((
                Marker {
                    incident: i,
                    base,
                    phase: rng.range(0.0, std::f32::consts::TAU),
                },
                Transform::from_translation(base),
                Visibility::default(),
                DespawnOnExit(Phase::Night),
            ))
            .observe(on_marker_click)
            .observe(on_marker_over)
            .observe(on_marker_out)
            .id();
        let model = format!("models/incidents/{}.glb", def.id);
        if asset_exists(&model) {
            commands.spawn((
                WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(model))),
                Transform::default(),
                ChildOf(marker),
            ));
        } else {
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::from_size(size))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: rgb(def.color),
                    perceptual_roughness: 0.8,
                    ..default()
                })),
                Transform::from_xyz(0.0, size.y / 2.0, 0.0),
                NotShadowCaster,
                Pickable::IGNORE,
                ChildOf(marker),
            ));
        }
        // An invisible box a little larger than the spot, so it is easy to click.
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::from_size(size * HITBOX_GROWTH))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::NONE,
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            })),
            Transform::from_xyz(0.0, size.y / 2.0, 0.0),
            NotShadowCaster,
            ChildOf(marker),
        ));
    }
}

/// How much larger than the spot the clickable box is.
const HITBOX_GROWTH: f32 = 1.6;

/// Clicking the spot fixes the mishap (as does its button in the night panel).
fn on_marker_click(
    click: On<PointerClick>,
    markers: Query<&Marker>,
    mut actions: MessageWriter<Action>,
) {
    if click.button != PointerButton::Primary {
        return;
    }
    if let Ok(marker) = markers.get(click.event_target()) {
        actions.write(Action::FixMishap(marker.incident as u32));
    }
}

#[allow(clippy::too_many_arguments)]
fn fix_mishaps(
    mut actions: MessageReader<Action>,
    markers: Query<(Entity, &Marker)>,
    data: Res<GameData>,
    lang: Res<Lang>,
    mut commands: Commands,
    mut mishaps: ResMut<Mishaps>,
    mut layout: ResMut<ShopLayout>,
    mut log: ResMut<GameLog>,
    mut cues: MessageWriter<Cue>,
    mut tip: ResMut<HoverTip>,
    mut report: ResMut<crate::day_cycle::NightReport>,
) {
    for action in actions.read() {
        let Action::FixMishap(i) = action else {
            continue;
        };
        let i = *i as usize;
        let Some(at) = mishaps.active.iter().position(|a| *a == i) else {
            continue;
        };
        mishaps.active.remove(at);
        let def = &data.incidents[i];
        let mut undo = def.effect;
        undo.scale(-1);
        layout.mishaps.add(&undo);
        log.push(lang.t(&format!("incident.{}.fixed", def.id)));
        report.mishaps_fixed += 1;
        cues.write(Cue::Click);
        HoverTip::hide(&mut tip);
        for (entity, marker) in &markers {
            if marker.incident == i {
                commands.entity(entity).despawn();
            }
        }
    }
}

/// The spot that is hovered, for the tooltip.
#[derive(Component)]
struct HoveredMarker;

fn on_marker_over(over: On<PointerOver>, mut commands: Commands) {
    commands.entity(over.event_target()).insert(HoveredMarker);
}

fn on_marker_out(out: On<PointerOut>, mut commands: Commands, mut tip: ResMut<HoverTip>) {
    commands
        .entity(out.event_target())
        .remove::<HoveredMarker>();
    HoverTip::hide(&mut tip);
}

fn marker_tooltip(
    hovered: Query<&Marker, With<HoveredMarker>>,
    data: Res<GameData>,
    lang: Res<Lang>,
    mut tip: ResMut<HoverTip>,
) {
    let Some(marker) = hovered.iter().next() else {
        return;
    };
    let def = &data.incidents[marker.incident];
    let mut t = HoverTip {
        title: lang.t(&format!("incident.{}.name", def.id)),
        ..default()
    };
    t.line(TipKind::Bad, def.effect.changes(&lang));
    t.line(TipKind::Hint, lang.t("incident.fix_hint"));
    HoverTip::show(&mut tip, t);
}

/// The crow hops, the leaves swirl, the drops fall – all in code, the
/// models stay still.
fn sway_markers(
    time: Res<Time>,
    data: Res<GameData>,
    mut markers: Query<(&Marker, &mut Transform)>,
) {
    let t = time.elapsed_secs();
    for (m, mut transform) in &mut markers {
        let p = t + m.phase;
        let (offset, turn) = match data.incidents[m.incident].motion {
            Motion::Hop => (
                Vec3::Y * 0.05 * (p * 3.0).sin().max(0.0),
                0.3 * (p * 0.7).sin(),
            ),
            Motion::Swirl => (
                Vec3::new(
                    0.06 * (p * 1.7).cos(),
                    0.04 * (p * 2.3).sin(),
                    0.06 * (p * 1.7).sin(),
                ),
                p * 1.2,
            ),
            Motion::Drip => (Vec3::Y * 0.004 * (p * 4.0).sin(), 0.0),
        };
        transform.translation = m.base + offset;
        transform.rotation = Quat::from_rotation_y(turn);
    }
}

/// Dawn: whatever was not fixed is put right by morning.
fn clear_mishaps(mut layout: ResMut<ShopLayout>, mut mishaps: ResMut<Mishaps>) {
    layout.mishaps = Atmosphere::default();
    mishaps.planned.clear();
    mishaps.active.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    /// The night panel's button fixes a mishap just like clicking its spot.
    #[test]
    fn a_mishap_is_fixed_from_the_panel() {
        let data = GameData::load();
        let effect = data.incidents[0].effect;
        let mut layout = ShopLayout::new(data.config.grid_cols, data.config.grid_rows);
        layout.mishaps.add(&effect);
        let mut world = World::new();
        world.insert_resource(data);
        world.insert_resource(layout);
        world.insert_resource(Mishaps {
            planned: Vec::new(),
            active: vec![0],
        });
        world.insert_resource(Lang::load("en"));
        world.init_resource::<GameLog>();
        world.init_resource::<HoverTip>();
        world.init_resource::<crate::day_cycle::NightReport>();
        world.init_resource::<Messages<Action>>();
        world.init_resource::<Messages<Cue>>();
        let marker = world
            .spawn(Marker {
                incident: 0,
                base: Vec3::ZERO,
                phase: 0.0,
            })
            .id();
        world
            .resource_mut::<Messages<Action>>()
            .write(Action::FixMishap(0));
        world.run_system_once(fix_mishaps).unwrap();
        assert!(world.resource::<Mishaps>().active.is_empty());
        assert_eq!(
            world.resource::<ShopLayout>().mishaps,
            Atmosphere::default()
        );
        assert_eq!(
            world
                .resource::<crate::day_cycle::NightReport>()
                .mishaps_fixed,
            1
        );
        assert!(world.get_entity(marker).is_err(), "its spot is gone");
    }
}
