//! The village crab apples and the cut branches in the shop follow Anselm's winter and Mira's thaw.
use super::village::Breeze;
use crate::data::{GameData, asset_exists};
use crate::day_cycle::{Phase, Progress};
use crate::harbor::{HARBOR, Location};
use crate::market::MARKET;
use crate::save::Settings;
use crate::shop::entrance::{self, DoorSwing};
use crate::{weather::WeatherNow, winter};
use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;

const MARKET_TREE: Vec3 = Vec3::new(3.67, 0., 5.66);
const QUAY_TREE: Vec3 = Vec3::new(-3.76, 0., -4.04);
const PITCHER: Vec3 = Vec3::new(-4.56, 0., -2.45);
const NECK: Vec3 = Vec3::new(0., 0.715, 0.);
const GRAFTS: [(Vec3, f32, f32, u8); 5] = [
    // Socket in the trunk, heading, length and order in which the bough opens.
    (Vec3::new(-0.36, 1.05, 0.08), 3.02, 1.04, 2),
    (Vec3::new(0.43, 1.18, 0.15), -0.37, 1.00, 1),
    (Vec3::new(-0.14, 1.49, -0.17), 1.95, 0.94, 4),
    (Vec3::new(0.13, 1.63, -0.18), 0.72, 0.86, 5),
    (Vec3::new(0.03, 1.78, 0.02), -1.78, 0.79, 3),
];
const SHED_INTERVAL: f32 = 6.8;
const FALL_TIME: f32 = 3.3;

pub(super) fn install(app: &mut App) {
    app.add_systems(Startup, plant)
        .add_systems(Update, dress.run_if(resource_changed::<Progress>))
        .add_systems(
            Update,
            move_branches
                .after(entrance::animate)
                .run_if(crate::menu::shop_unpaused)
                .run_if(not(in_state(Phase::Menu))),
        );
}

#[derive(Clone, Copy, PartialEq)]
enum Season {
    Autumn,
    Winter,
    Spring(f32),
}

impl Season {
    fn current(data: &GameData, progress: &Progress) -> Self {
        let flowers = winter::blossom_level(data, progress);
        if flowers > 0. {
            Self::Spring(flowers)
        } else if winter::snowing(data, progress) {
            Self::Winter
        } else {
            Self::Autumn
        }
    }
}

#[derive(Component, Clone, Copy)]
enum Cover {
    Russet,
    Snow,
    Flowers(u8),
}

impl Cover {
    fn shows(self, season: Season) -> bool {
        match (self, season) {
            (Self::Russet, Season::Autumn) | (Self::Snow, Season::Winter) => true,
            (Self::Flowers(rank), Season::Spring(level)) => {
                level >= rank as f32 / GRAFTS.len() as f32
            }
            _ => false,
        }
    }
}

#[derive(Component)]
struct Bough {
    site: Location,
    rest: Quat,
    offset: f32,
}

#[derive(Component)]
struct Falling {
    site: Location,
    delay: f32,
}

// All instances share their models or their primitive stand-ins. The wood stays
// at the same graft when its foliage changes or the models finish loading.
enum Art {
    Scene(Handle<WorldAsset>),
    Shape(Handle<Mesh>, Handle<StandardMaterial>),
}

impl Art {
    fn load(
        assets: &AssetServer,
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<StandardMaterial>,
        path: &'static str,
        color: Color,
        fallback: impl FnOnce() -> Mesh,
    ) -> Self {
        if asset_exists(path) {
            Self::Scene(assets.load(GltfAssetLabel::Scene(0).from_asset(path)))
        } else {
            Self::Shape(meshes.add(fallback()), materials.add(color))
        }
    }

    fn spawn(&self, commands: &mut Commands, parent: Entity) -> Entity {
        let mut e = commands.spawn((Transform::default(), Pickable::IGNORE, ChildOf(parent)));
        match self {
            Self::Scene(scene) => {
                e.insert(WorldAssetRoot(scene.clone()))
                    .observe(ignore_meshes);
            }
            Self::Shape(mesh, material) => {
                e.insert((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone())));
            }
        }
        e.id()
    }
}

/// The camera can see a trader or a furnishing square through the leaves.
fn ignore_meshes(
    ready: On<WorldInstanceReady>,
    children: Query<&Children>,
    mut commands: Commands,
) {
    for child in children.iter_descendants(ready.entity) {
        commands.entity(child).insert(Pickable::IGNORE);
    }
}

fn oval(centre: Vec3, radii: Vec3) -> Mesh {
    Sphere::new(1.)
        .mesh()
        .ico(1)
        .unwrap()
        .transformed_by(Transform::from_translation(centre).with_scale(radii))
}

fn limb_between(from: Vec3, to: Vec3, radius: f32) -> Mesh {
    let span = to - from;
    Mesh::from(Cylinder::new(radius, span.length())).transformed_by(
        Transform::from_translation((to + from) * 0.5)
            .with_rotation(Quat::from_rotation_arc(Vec3::Y, span.normalize())),
    )
}

fn plant(
    mut commands: Commands,
    data: Res<GameData>,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut art = |path, color: (u8, u8, u8), fallback: fn() -> Mesh| {
        Art::load(
            &assets,
            &mut meshes,
            &mut materials,
            path,
            Color::srgb_u8(color.0, color.1, color.2),
            fallback,
        )
    };
    let trunk = art("models/harbor/apple_trunk.glb", (110, 83, 70), || {
        let mut wood = Mesh::from(ConicalFrustum {
            radius_bottom: 0.27,
            radius_top: 0.10,
            height: 1.8,
        })
        .translated_by(Vec3::Y * 0.9);
        for (socket, _, _, _) in GRAFTS {
            wood.merge(&limb_between(Vec3::Y * (socket.y - 0.39), socket, 0.065))
                .expect("the trunk and its forks have the same mesh attributes");
        }
        wood
    });
    let twig = art("models/harbor/apple_bough.glb", (145, 109, 82), || {
        limb_between(Vec3::ZERO, Vec3::new(1.1, 0.8, 0.13), 0.038)
    });
    let jug = art("models/shop/apple_jug.glb", (85, 136, 131), || {
        Mesh::from(ConicalFrustum {
            radius_bottom: 0.24,
            radius_top: 0.14,
            height: NECK.y,
        })
        .translated_by(Vec3::Y * NECK.y * 0.5)
    });
    let russet = art("models/harbor/apple_autumn.glb", (199, 146, 77), || {
        oval(Vec3::new(0.70, 0.62, 0.05), Vec3::new(0.60, 0.31, 0.40))
    });
    let leaf = art("models/harbor/apple_leaf.glb", (202, 145, 68), || {
        oval(Vec3::ZERO, Vec3::new(0.09, 0.012, 0.035))
    });
    let last = data.config.final_night();
    let snow = data
        .stories
        .iter()
        .any(|s| s.snow && s.start_night + s.chapters.len() as u32 - 1 <= last)
        .then(|| {
            art("models/harbor/apple_snow.glb", (222, 229, 222), || {
                oval(Vec3::new(0.70, 0.67, 0.05), Vec3::new(0.51, 0.055, 0.30))
            })
        });
    let has_spring = data
        .stories
        .iter()
        .any(|s| s.blossom && s.start_night <= last);
    let flowers = has_spring.then(|| {
        art("models/harbor/apple_blossom.glb", (244, 207, 197), || {
            oval(Vec3::new(0.71, 0.69, 0.05), Vec3::new(0.55, 0.28, 0.35))
        })
    });
    let petal = has_spring.then(|| {
        art("models/harbor/apple_petal.glb", (252, 221, 208), || {
            oval(Vec3::ZERO, Vec3::new(0.05, 0.012, 0.03))
        })
    });

    for (site, pos, turn, size) in [
        (Location::Market, MARKET + MARKET_TREE, -0.20, 1.04),
        (Location::Harbor, HARBOR + QUAY_TREE, 0.48, 1.17),
        (Location::Shop, PITCHER, -0.17, 1.0),
    ] {
        let outside = site != Location::Shop;
        let root = commands
            .spawn((
                Transform::from_translation(pos)
                    .with_rotation(Quat::from_rotation_y(turn))
                    .with_scale(Vec3::splat(size)),
                Visibility::default(),
            ))
            .id();
        let support = if outside { &trunk } else { &jug };
        support.spawn(&mut commands, root);
        for (socket, heading, length, rank) in GRAFTS {
            if !outside && rank % 2 == 0 {
                continue;
            }
            let (base, lift, length) = if outside {
                (socket, 0., length)
            } else {
                (NECK + socket.with_y(0.) * 0.11, 0.49, length * 0.73)
            };
            // In the jug the tallest cutting fans out to the left of its two neighbours.
            let heading = if !outside && rank == 5 { 2.92 } else { heading };
            let rest = Quat::from_rotation_y(heading) * Quat::from_rotation_z(lift);
            let limb = commands
                .spawn((
                    Bough {
                        site,
                        rest,
                        offset: heading + size,
                    },
                    Transform::from_translation(base)
                        .with_rotation(rest)
                        .with_scale(Vec3::splat(length)),
                    Visibility::default(),
                    ChildOf(root),
                ))
                .id();
            twig.spawn(&mut commands, limb);
            let mut cover = |part: &Art, marker| {
                let e = part.spawn(&mut commands, limb);
                commands.entity(e).insert((marker, Visibility::Hidden));
            };
            cover(&russet, Cover::Russet);
            if let Some(snow) = &snow
                && outside
            {
                cover(snow, Cover::Snow);
            }
            if let Some(flowers) = &flowers {
                cover(flowers, Cover::Flowers(rank));
            }
        }
        if outside {
            let drift = commands
                .spawn((
                    Falling {
                        site,
                        delay: if site == Location::Market { 0. } else { 3.7 },
                    },
                    Transform::default(),
                    Visibility::Hidden,
                    ChildOf(root),
                ))
                .id();
            for (art, coat) in [
                (Some(&leaf), Cover::Russet),
                (petal.as_ref(), Cover::Flowers(1)),
            ] {
                if let Some(art) = art {
                    let e = art.spawn(&mut commands, drift);
                    commands.entity(e).insert((coat, Visibility::Hidden));
                }
            }
        }
    }
}

fn dress(
    data: Res<GameData>,
    progress: Res<Progress>,
    mut covers: Query<(&Cover, &mut Visibility)>,
) {
    let season = Season::current(&data, &progress);
    for (cover, mut shown) in &mut covers {
        shown.set_if_neq(if cover.shows(season) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
}

fn move_branches(
    site: Res<Location>,
    wind: Res<Breeze>,
    weather: Res<WeatherNow>,
    settings: Res<Settings>,
    doors: Query<&DoorSwing>,
    mut boughs: Query<(&Bough, &mut Transform)>,
    mut drifting: Query<(&Falling, &mut Visibility, &mut Transform), Without<Bough>>,
) {
    let time = wind.elapsed_secs();
    let strength = match *site {
        Location::Shop => doors
            .iter()
            .next()
            .map_or(0., |door| 0.19 * door.aperture()),
        _ if weather.stormy() => 0.22,
        _ => 0.115,
    };
    for (bough, mut at) in &mut boughs {
        if bough.site == *site {
            let bend = if settings.reduced_motion {
                0.
            } else {
                strength * wind.gust(bough.offset)
            };
            let twist = bend * 0.34 * wind.flutter(bough.offset);
            let rotation = Quat::from_rotation_z(-bend) * bough.rest * Quat::from_rotation_x(twist);
            if rotation != at.rotation {
                at.rotation = rotation;
            }
        }
    }
    for (fall, mut shown, mut at) in &mut drifting {
        let age = (time + fall.delay).rem_euclid(SHED_INTERVAL);
        if fall.site != *site || settings.reduced_motion || age >= FALL_TIME {
            shown.set_if_neq(Visibility::Hidden);
            continue;
        }
        shown.set_if_neq(Visibility::Inherited);
        let t = age / FALL_TIME;
        let wave = (t * 7.8).sin();
        // Starts among the opening right-hand boughs and settles at the foot of the tree.
        at.translation = Vec3::new(
            1.18 + t * (0.65 + 2.4 * strength) + 0.17 * wave,
            1.94 * (1. - t) + 0.08 * wave,
            0.35 + 0.34 * t + 0.13 * (t * 10.).sin(),
        );
        at.rotation = Quat::from_euler(EulerRot::YXZ, t * 5., wave * 0.94, t * 3.1);
        at.scale = Vec3::splat((t * 9.).min(1.) * ((1. - t) * 6.).min(1.));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(feature = "demo"))]
    #[test]
    fn loading_an_earlier_chapter_restores_its_branches() {
        let data = GameData::load();
        let clock = data.stories.iter().position(|story| story.snow).unwrap();
        let gardener = data.stories.iter().position(|story| story.blossom).unwrap();
        let mut progress = Progress::new(0);
        progress.story.resize(data.stories.len(), 0);
        let mut app = App::new();
        app.add_systems(Update, dress.run_if(resource_changed::<Progress>));
        let covers = [
            Cover::Russet,
            Cover::Snow,
            Cover::Flowers(1),
            Cover::Flowers(5),
        ]
        .map(|cover| app.world_mut().spawn((cover, Visibility::Hidden)).id());
        let winter = data.stories[clock].chapters.len();
        let spring = data.stories[gardener].chapters.len();
        app.insert_resource(data);
        for (mended, flowering, expected) in [
            (0, 0, [true, false, false, false]),
            (winter, 0, [false, true, false, false]),
            (winter, 1, [false, false, true, false]),
            (winter, spring, [false, false, true, true]),
            (winter, 0, [false, true, false, false]),
        ] {
            progress.story[clock] = mended;
            progress.story[gardener] = flowering;
            app.insert_resource(progress.clone());
            app.update();
            let visible =
                covers.map(|e| *app.world().get::<Visibility>(e).unwrap() == Visibility::Inherited);
            assert_eq!(visible, expected);
        }
    }

    #[test]
    fn reduced_motion_parks_the_bough_and_hides_the_falling_leaf() {
        let mut app = App::new();
        app.init_resource::<Settings>()
            .insert_resource(Location::Market)
            .init_resource::<Breeze>()
            .init_resource::<WeatherNow>()
            .add_systems(Update, move_branches);
        let rest = Quat::from_rotation_y(0.65);
        let bough = app
            .world_mut()
            .spawn((
                Bough {
                    site: Location::Market,
                    rest,
                    offset: 0.3,
                },
                Transform::from_rotation(rest),
            ))
            .id();
        let leaf = app
            .world_mut()
            .spawn((
                Falling {
                    site: Location::Market,
                    delay: 1.0,
                },
                Transform::default(),
                Visibility::Hidden,
            ))
            .id();
        app.update();
        assert_ne!(app.world().get::<Transform>(bough).unwrap().rotation, rest);
        assert_eq!(
            *app.world().get::<Visibility>(leaf).unwrap(),
            Visibility::Inherited
        );
        app.world_mut().resource_mut::<Settings>().reduced_motion = true;
        app.update();
        assert_eq!(app.world().get::<Transform>(bough).unwrap().rotation, rest);
        assert_eq!(
            *app.world().get::<Visibility>(leaf).unwrap(),
            Visibility::Hidden
        );
    }
}
