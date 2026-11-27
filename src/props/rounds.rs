//! Loose goods, working cloth, light and traces follow the residents' hands and feet.
use super::residents::{Job, Resident, ease, walk};
use crate::{
    data::GameData,
    day_cycle::{Phase, Progress},
    harbor::{HARBOR, Location, WATER_Y},
    save::Settings,
    weather::{Weather, WeatherNow},
};
use bevy::{prelude::*, world_serialization::WorldInstanceReady};

const SHOULDER: Vec3 = Vec3::new(0.191, 1.028, -0.004);
const PRINTS: usize = 16;
const PRINT_LIFE: f32 = 5.0;

pub(super) fn install(app: &mut App) {
    app.add_systems(Startup, setup).add_systems(
        Update,
        (tools, fabrics, footprints)
            .after(walk)
            .run_if(crate::menu::shop_unpaused)
            .run_if(not(in_state(Phase::Menu))),
    );
}

/// Local position of a point on the moving right arm.
pub(super) fn hand(rest: Vec3, angle: f32) -> Vec3 {
    SHOULDER + Quat::from_rotation_x(angle) * (rest - SHOULDER)
}

pub(super) struct Art<'a, 'w, 's> {
    pub commands: &'a mut Commands<'w, 's>,
    pub assets: &'a AssetServer,
    pub meshes: &'a mut Assets<Mesh>,
    pub materials: &'a mut Assets<StandardMaterial>,
}

impl Art<'_, '_, '_> {
    pub(super) fn piece(&mut self, id: &str, size: Vec3, color: Color) -> Entity {
        let root = self
            .commands
            .spawn((Transform::default(), Visibility::Hidden, Pickable::IGNORE))
            .id();
        let path = super::prop_model_path(id);
        let part = crate::shop::model_or_box(
            self.commands,
            self.assets,
            self.meshes,
            self.materials,
            &path,
            size,
            color,
        );
        self.commands
            .entity(part)
            .insert((ChildOf(root), Pickable::IGNORE))
            .observe(crate::scene::ignore_scene_picking);
        root
    }
}

#[derive(Component)]
enum Tool {
    Loaf { index: usize, boards: [Vec3; 2] },
    StallLoaf { index: usize, board: Vec3 },
    Cover,
    Letter,
    Flame,
    Reflection,
}

#[derive(Component)]
struct Print {
    index: usize,
    life: f32,
}

fn setup(
    mut commands: Commands,
    data: Res<GameData>,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut art = Art {
        commands: &mut commands,
        assets: &assets,
        meshes: &mut meshes,
        materials: &mut materials,
    };
    let board = |id| data.props.iter().find(|p| p.id == id).map(super::world_pos);
    if let (Some(oven), Some(stall)) = (board("bread_board"), board("bread_stall")) {
        for index in 0..2 {
            for tool in [
                Tool::Loaf {
                    index,
                    boards: [oven, stall],
                },
                Tool::StallLoaf {
                    index,
                    board: stall,
                },
            ] {
                let id = art.piece(
                    "round_loaf",
                    Vec3::new(0.31, 0.17, 0.17),
                    Color::srgb(0.78, 0.56, 0.32),
                );
                art.commands.entity(id).insert(tool);
            }
        }
    }
    for (id, size, color, tool) in [
        (
            "basket_waxcloth",
            Vec3::new(0.43, 0.13, 0.34),
            Color::srgb(0.70, 0.53, 0.30),
            Tool::Cover,
        ),
        (
            "round_letter",
            Vec3::new(0.33, 0.23, 0.03),
            Color::srgb(0.93, 0.85, 0.69),
            Tool::Letter,
        ),
        (
            "keeper_reflection",
            Vec3::new(0.52, 0.01, 0.94),
            Color::srgb(0.47, 0.44, 0.31),
            Tool::Reflection,
        ),
    ] {
        let entity = art.piece(id, size, color);
        art.commands.entity(entity).insert(tool);
    }
    art.commands.spawn((
        Tool::Flame,
        PointLight {
            color: Color::srgb(1., 0.66, 0.29),
            intensity: 24_000.,
            range: 3.6,
            radius: 0.08,
            shadow_maps_enabled: false,
            ..default()
        },
        Visibility::Hidden,
        Transform::default(),
        Pickable::IGNORE,
    ));
    let winter = data.stories.iter().any(|s| s.snow)
        || (1..=data.config.demo_nights).any(|n| {
            data.event(n)
                .is_some_and(|e| matches!(e.weather, Some(Weather::Snow | Weather::Blizzard)))
        });
    if cfg!(feature = "demo") && !winter {
        return;
    }
    for index in 0..PRINTS {
        let id = art.piece(
            "boot_print",
            Vec3::new(0.11, 0.013, 0.20),
            Color::srgb(0.52, 0.58, 0.64),
        );
        art.commands.entity(id).insert(Print { index, life: 0. });
    }
}

fn visible(on: bool) -> Visibility {
    if on {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    }
}

/// Loaves pass through the hand in sequence. Goods on a board stay put while the baker turns.
fn bread(boards: [Vec3; 2], index: usize, pose: &super::residents::Pose) -> Transform {
    let side = index as f32;
    let parcel = pose.at.transform_point(hand(
        Vec3::new(0.175 + 0.135 * side, 0.768, 0.165 + 0.035 * side),
        pose.arm,
    ));
    let board = |n: usize| boards[n] + Vec3::new((side - 0.5) * 0.36, 0.758, 0.);
    let progress = ease((pose.stop - 0.20 - 0.24 * side) / 0.21);
    let (from, to, k) = if pose.returning {
        // Empty basket on the way back. The next batch waits on the bakehouse board.
        (board(0), parcel, progress)
    } else {
        (parcel, board(1), progress)
    };
    let arc = Vec3::Y * (0.12 * (std::f32::consts::PI * k).sin());
    let in_basket = if pose.returning { k } else { 1. - k };
    Transform::from_translation(from.lerp(to, k) + arc)
        .with_rotation(Quat::IDENTITY.slerp(pose.at.rotation, in_basket))
        .with_scale(Vec3::splat(1. - 0.44 * in_basket))
}

fn tools(
    time: Res<Time>,
    location: Res<Location>,
    weather: Res<WeatherNow>,
    residents: Query<&Resident>,
    mail: Res<super::mail::Round>,
    mut tools: Query<(&Tool, &mut Transform, &mut Visibility)>,
) {
    let wet = matches!(weather.0, Some(Weather::Rain | Weather::Storm));
    for (tool, mut at, mut shown) in &mut tools {
        let (job, site) = match tool {
            Tool::Loaf { .. } | Tool::StallLoaf { .. } | Tool::Cover => {
                (Job::Bread, Location::Market)
            }
            Tool::Letter => (Job::Post, Location::Market),
            _ => (Job::Lamp, Location::Harbor),
        };
        if *location != site {
            shown.set_if_neq(Visibility::Hidden);
            continue;
        }
        let Some(resident) = residents.iter().find(|r| r.job == job) else {
            continue;
        };
        let pose = resident.pose();
        let attached = |p| {
            Transform::from_translation(pose.at.transform_point(hand(p, pose.arm)))
                .with_rotation(pose.at.rotation * Quat::from_rotation_x(pose.arm))
        };
        let mut on = true;
        *at = match tool {
            Tool::Loaf { index, boards } => {
                let mut at = bread(*boards, *index, &pose);
                if pose.returning {
                    at.scale *= ease(pose.travel / 0.25);
                }
                at
            }
            Tool::StallLoaf { index, board } => {
                let available = if pose.returning {
                    1.
                } else {
                    1. - ease((pose.travel - 0.20 - 0.15 * (*index as f32)) / 0.28)
                };
                Transform::from_translation(
                    *board + Vec3::new((*index as f32 - 0.5) * 0.36, 0.758, 0.),
                )
                .with_scale(Vec3::splat(available))
            }
            Tool::Cover => {
                on = wet;
                let open = ease((pose.stop - 0.06) / 0.13) * (1. - ease((pose.stop - 0.66) / 0.17));
                let fold = Quat::from_rotation_x(-1.38 * open);
                let hem = Vec3::new(0., 0., 0.16);
                attached(Vec3::new(0.246, 0.810, 0.01))
                    .mul_transform(Transform::from_translation(fold * hem).with_rotation(fold))
            }
            Tool::Letter => {
                on = !mail.carrying_delivery();
                attached(Vec3::new(0.290, 0.86, 0.183)).with_scale(Vec3::splat(0.63))
            }
            Tool::Flame => attached(Vec3::new(0.309, 0.641, 0.173)),
            Tool::Reflection => {
                let strength = if pose.returning {
                    0.
                } else {
                    (std::f32::consts::PI * pose.stop).sin().max(0.)
                };
                on = strength > 0.015;
                // The water is in front of the pier end. Keep the hat and lantern clear of the boards.
                let water = Vec2::new(
                    pose.at.translation.x - HARBOR.x,
                    pose.at.translation.z - HARBOR.z + 1.15,
                );
                let surface = crate::harbor::swell(HARBOR.xz() + water, time.elapsed_secs()).0;
                Transform::from_translation(
                    HARBOR + Vec3::new(water.x, WATER_Y + surface + 0.025, water.y),
                )
                .with_scale(Vec3::new(strength, 1., 0.68 + 0.32 * strength))
            }
        };
        shown.set_if_neq(visible(on));
    }
}

#[derive(Component)]
pub(super) enum Fabric {
    Linen,
    Net,
    Stitch,
}

pub(super) fn bind_fabric(
    ready: On<WorldInstanceReady>,
    children: Query<&Children>,
    nodes: Query<&Name, Without<Mesh3d>>,
    mut commands: Commands,
) {
    for entity in children.iter_descendants(ready.entity) {
        let Ok(name) = nodes.get(entity) else {
            continue;
        };
        let part = match name.as_str() {
            "Drying linen" => Fabric::Linen,
            "Net mesh" => Fabric::Net,
            "Loose stitch" => Fabric::Stitch,
            _ => continue,
        };
        commands.entity(entity).insert(part);
    }
}

fn fabrics(
    time: Res<Time>,
    location: Res<Location>,
    weather: Res<WeatherNow>,
    settings: Res<Settings>,
    residents: Query<&Resident>,
    mut parts: Query<(&Fabric, &mut Transform)>,
) {
    let wet = matches!(weather.0, Some(Weather::Rain | Weather::Storm));
    let pose = residents
        .iter()
        .find(|r| r.job == Job::Net)
        .map(Resident::pose);
    for (part, mut at) in &mut parts {
        let scale = match part {
            Fabric::Linen if *location == Location::Market => {
                if wet {
                    Vec3::new(1., 0.16, 0.50)
                } else {
                    Vec3::ONE
                }
            }
            Fabric::Net | Fabric::Stitch if *location == Location::Harbor => {
                let pull = pose.as_ref().filter(|p| !p.returning).map_or(0., |p| {
                    ease((p.stop - 0.14) / 0.40) * (1. - ease((p.stop - 0.80) / 0.20))
                });
                match part {
                    Fabric::Net => {
                        Vec3::new(0.85 + 0.15 * pull, 1. - 0.115 * pull, 1. - 0.46 * pull)
                    }
                    _ => Vec3::new(0.76 + 0.24 * pull, 1.26 - 0.72 * pull, 1.15 - 0.62 * pull),
                }
            }
            _ => continue,
        };
        let blend = if settings.reduced_motion {
            1.
        } else {
            1. - (-7. * time.delta_secs()).exp()
        };
        at.scale = at.scale.lerp(scale, blend);
    }
}

#[derive(Default)]
struct Trail {
    last: Option<Vec3>,
    next: usize,
}

fn footprints(
    time: Res<Time>,
    settings: Res<Settings>,
    location: Res<Location>,
    weather: Res<WeatherNow>,
    data: Res<GameData>,
    progress: Res<Progress>,
    residents: Query<&Resident>,
    mut trail: Local<Trail>,
    mut prints: Query<(&mut Print, &mut Transform, &mut Visibility)>,
) {
    let active = *location == Location::Market
        && !settings.reduced_motion
        && (matches!(weather.0, Some(Weather::Snow | Weather::Blizzard))
            || crate::winter::snowing(&data, &progress));
    let at = residents
        .iter()
        .find(|r| r.job == Job::Post)
        .map(|r| r.pose().at);
    let footfall = at.filter(|at| {
        active
            && trail
                .last
                .is_none_or(|p| p.distance(at.translation) >= 0.32)
    });
    if !active {
        trail.last = None;
    }
    for (mut print, mut at, mut visibility) in &mut prints {
        print.life = if active {
            (print.life - time.delta_secs()).max(0.)
        } else {
            0.
        };
        if let Some(foot) = footfall
            && trail.next == print.index
        {
            let side = if print.index % 2 == 0 { -0.086 } else { 0.086 };
            *at = Transform::from_translation(foot.transform_point(Vec3::new(side, 0.034, 0.)))
                .with_rotation(foot.rotation);
            print.life = PRINT_LIFE;
        }
        at.scale = Vec3::splat(ease(print.life / 1.2));
        visibility.set_if_neq(visible(print.life > 0.));
    }
    if let Some(foot) = footfall {
        trail.next = (trail.next + 1) % PRINTS;
        trail.last = Some(foot.translation);
    }
}

/// The replacement art can be removed without losing the working pivots.
pub(super) fn fallback_fabric(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    root: Entity,
    linen: bool,
) {
    let mut sheets = Vec::new();
    let mut block = |parent, pos, size, color: Color, rotation| {
        commands
            .spawn((
                Mesh3d(meshes.add(Cuboid::from_size(size))),
                MeshMaterial3d(materials.add(color)),
                Transform::from_translation(pos).with_rotation(Quat::from_rotation_z(rotation)),
                Pickable::IGNORE,
                ChildOf(parent),
            ))
            .id()
    };
    if linen {
        for (x, color) in [
            (-0.56, Color::srgb(0.29, 0.49, 0.51)),
            (0., Color::srgb(0.70, 0.47, 0.34)),
            (0.54, Color::srgb(0.49, 0.41, 0.54)),
        ] {
            let pivot = block(
                root,
                Vec3::new(x, 1.79, 0.94),
                Vec3::splat(0.025),
                color,
                0.,
            );
            block(
                pivot,
                Vec3::new(0., -0.24, 0.),
                Vec3::new(0.38, 0.49, 0.03),
                color,
                0.,
            );
            sheets.push(pivot);
        }
    } else {
        let wood = Color::srgb(0.44, 0.32, 0.24);
        let flax = Color::srgb(0.74, 0.65, 0.47);
        for x in [-0.59, 0.59] {
            block(
                root,
                Vec3::new(x, 0.57, 0.),
                Vec3::new(0.08, 1.14, 0.10),
                wood,
                0.,
            );
        }
        block(
            root,
            Vec3::new(0., 1.1, 0.),
            Vec3::new(1.32, 0.08, 0.10),
            wood,
            0.,
        );
        let pivot = block(
            root,
            Vec3::new(-0.49, 1.07, 0.06),
            Vec3::splat(0.025),
            wood,
            0.,
        );
        for i in 0..4 {
            for slope in [-0.34, 0.34] {
                block(
                    pivot,
                    Vec3::new(0.16 + i as f32 * 0.22, -0.30, 0.),
                    Vec3::new(0.023, 0.64, 0.025),
                    flax,
                    slope,
                );
            }
        }
        commands.entity(pivot).insert(Fabric::Net);
    }
    for pivot in sheets {
        commands.entity(pivot).insert(Fabric::Linen);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loaves_change_hands_one_at_a_time_and_reach_the_stall() {
        let data = GameData::load();
        let prop = data.props.iter().find(|p| p.id == "bread_carrier").unwrap();
        let route = prop.walk.unwrap();
        let mut baker = Resident::new(prop, route, super::super::world_pos(prop));
        let boards = ["bread_board", "bread_stall"]
            .map(|id| super::super::world_pos(data.props.iter().find(|p| p.id == id).unwrap()));
        let stops = route.travel + route.pause;
        for (returning, source) in [(true, 0), (false, 1)] {
            let at = if returning {
                stops + route.travel
            } else {
                route.travel
            };
            let on_board =
                |i: usize| boards[source] + Vec3::new((i as f32 - 0.5) * 0.36, 0.758, 0.);
            baker.elapsed = at + route.pause * 0.43;
            let pose = baker.pose();
            assert_eq!(
                bread(boards, 0, &pose).translation.distance(on_board(0)) < 0.001,
                !returning
            );
            assert_eq!(
                bread(boards, 1, &pose).translation.distance(on_board(1)) < 0.001,
                returning
            );
            baker.elapsed = at + route.pause * 0.76;
            assert_eq!(
                bread(boards, 1, &baker.pose())
                    .translation
                    .distance(on_board(1))
                    < 0.001,
                !returning
            );
        }
    }
}
