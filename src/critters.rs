//! Animals that make the scenes feel alive: a cat asleep on the counter, and
//! at the harbour a cat on the barrels swishing its tail, a gull on a mooring
//! post, an owl on the lighthouse turning its head, bats circling the
//! lighthouse, fish jumping out of the water and moths around the lamp.
//!
//! Each body part is a model from `assets/models/critters/<id>.glb` (the
//! pivot of a moving part is its origin), or a few primitives until the
//! models exist. All movement is done here.

use std::f32::consts::{PI, TAU};

use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use crate::data::asset_exists;
use crate::harbor::{HARBOR, RippleMesh, WATER_Y, spawn_ripple};
use crate::rng::Rng;

pub struct CrittersPlugin;

impl Plugin for CrittersPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_critters).add_systems(
            Update,
            (
                breathe,
                swish_tails,
                look_around,
                hop_gulls,
                fly_bats,
                jump_fish,
                flutter_moths,
            ),
        );
    }
}

/// Where the critters sit (shop metres for the shop cat, harbour metres for
/// the rest). Kept together so they can be moved with the props.
const SHOP_CAT: Vec3 = Vec3::new(6.3, 1.0, 1.55);
const HARBOR_CAT: Vec3 = Vec3::new(5.3, 1.47, -2.35);
const GULL: Vec3 = Vec3::new(3.6, 0.6, -0.82);
const OWL: Vec3 = Vec3::new(-5.7, 3.28, -3.3);
const LIGHTHOUSE_TOP: Vec3 = Vec3::new(-6.0, 3.9, -3.6);
const LAMP: Vec3 = Vec3::new(0.6, 2.45, -1.25);
const BATS: usize = 3;
const MOTHS: usize = 7;

#[derive(Component)]
struct Breathing {
    phase: f32,
}

#[derive(Component)]
struct Tail {
    phase: f32,
}

/// A head (or a whole bird) that turns to look around now and then.
#[derive(Component)]
struct Looking {
    target: f32,
    wait: f32,
}

#[derive(Component)]
struct Gull {
    base: Vec3,
    hop: f32,
    wait: f32,
}

#[derive(Component)]
struct Bat {
    radius: f32,
    height: f32,
    speed: f32,
    phase: f32,
}

#[derive(Component)]
struct Wing {
    side: f32,
}

#[derive(Component)]
struct Fish {
    from: Vec3,
    to: Vec3,
    age: f32,
    wait: f32,
}

#[derive(Component)]
struct Moth {
    phase: f32,
    speed: f32,
}

/// Spawning helper: a model part or a placeholder primitive.
struct Parts<'a, 'w, 's> {
    commands: Commands<'w, 's>,
    asset_server: &'a AssetServer,
    meshes: &'a mut Assets<Mesh>,
    materials: &'a mut Assets<StandardMaterial>,
}

impl Parts<'_, '_, '_> {
    /// `models/critters/<id>.glb` as a child of `parent`, if it exists.
    fn model(&mut self, parent: Entity, id: &str) -> bool {
        let path = format!("models/critters/{id}.glb");
        if !asset_exists(&path) {
            return false;
        }
        self.commands.spawn((
            WorldAssetRoot(
                self.asset_server
                    .load(GltfAssetLabel::Scene(0).from_asset(path)),
            ),
            Transform::default(),
            ChildOf(parent),
        ));
        true
    }

    fn shape(&mut self, parent: Entity, mesh: Mesh, color: Color, at: Transform) {
        let mesh = self.meshes.add(mesh);
        let material = self.materials.add(StandardMaterial {
            base_color: color,
            perceptual_roughness: 0.9,
            ..default()
        });
        self.commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            at,
            Pickable::IGNORE,
            ChildOf(parent),
        ));
    }

    fn glow(&mut self, parent: Entity, radius: f32, emissive: LinearRgba, at: Vec3) {
        let mesh = self.meshes.add(Sphere::new(radius));
        let material = self.materials.add(StandardMaterial {
            base_color: Color::WHITE,
            emissive,
            ..default()
        });
        self.commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_translation(at),
            NotShadowCaster,
            Pickable::IGNORE,
            ChildOf(parent),
        ));
    }

    fn node(&mut self, parent: Option<Entity>, at: Transform, extra: impl Bundle) -> Entity {
        let mut e = self
            .commands
            .spawn((at, Visibility::default(), Pickable::IGNORE, extra));
        if let Some(parent) = parent {
            e.insert(ChildOf(parent));
        }
        e.id()
    }
}

fn spawn_critters(
    commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rng: ResMut<Rng>,
) {
    let mut p = Parts {
        commands,
        asset_server: &asset_server,
        meshes: &mut meshes,
        materials: &mut materials,
    };
    let black = Color::srgb(0.08, 0.08, 0.1);
    let ginger = Color::srgb(0.75, 0.45, 0.2);

    // The shop cat, curled up asleep on the counter.
    let cat = p.node(
        None,
        Transform::from_translation(SHOP_CAT).with_rotation(Quat::from_rotation_y(0.4)),
        Breathing { phase: 0.0 },
    );
    if !p.model(cat, "cat_sleeping") {
        p.shape(
            cat,
            Sphere::new(0.16).into(),
            ginger,
            Transform::from_xyz(0.0, 0.1, 0.0).with_scale(Vec3::new(1.3, 0.65, 1.0)),
        );
        p.shape(
            cat,
            Sphere::new(0.08).into(),
            ginger,
            Transform::from_xyz(0.17, 0.1, 0.06),
        );
    }

    // The harbour cat on the barrels, its tail hanging down and swishing.
    let cat = p.node(
        None,
        Transform::from_translation(HARBOR + HARBOR_CAT).with_rotation(Quat::from_rotation_y(-0.3)),
        Breathing { phase: 1.3 },
    );
    if !p.model(cat, "cat_sitting") {
        p.shape(
            cat,
            Sphere::new(0.13).into(),
            black,
            Transform::from_xyz(0.0, 0.13, 0.0).with_scale(Vec3::new(0.9, 1.2, 1.0)),
        );
        p.shape(
            cat,
            Sphere::new(0.08).into(),
            black,
            Transform::from_xyz(0.0, 0.33, 0.04),
        );
        for x in [-0.045, 0.045] {
            p.glow(
                cat,
                0.012,
                LinearRgba::rgb(2.0, 2.6, 0.6),
                Vec3::new(x, 0.34, 0.115),
            );
        }
    }
    let tail = p.node(
        Some(cat),
        Transform::from_xyz(0.0, 0.05, -0.1),
        Tail { phase: 0.0 },
    );
    if !p.model(tail, "cat_tail") {
        p.shape(
            tail,
            Capsule3d::new(0.02, 0.28).into(),
            black,
            Transform::from_xyz(0.0, -0.15, -0.02).with_rotation(Quat::from_rotation_x(0.3)),
        );
    }

    // A gull dozing on a mooring post, hopping round now and then.
    let gull = p.node(
        None,
        Transform::from_translation(HARBOR + GULL),
        Gull {
            base: HARBOR + GULL,
            hop: 0.0,
            wait: 3.0,
        },
    );
    if !p.model(gull, "seagull") {
        let white = Color::srgb(0.9, 0.9, 0.88);
        p.shape(
            gull,
            Sphere::new(0.1).into(),
            white,
            Transform::from_xyz(0.0, 0.1, 0.0).with_scale(Vec3::new(0.8, 0.8, 1.4)),
        );
        p.shape(
            gull,
            Sphere::new(0.055).into(),
            white,
            Transform::from_xyz(0.0, 0.2, 0.1),
        );
        p.shape(
            gull,
            Cone {
                radius: 0.015,
                height: 0.07,
            }
            .into(),
            Color::srgb(0.9, 0.7, 0.2),
            Transform::from_xyz(0.0, 0.2, 0.17).with_rotation(Quat::from_rotation_x(PI / 2.0)),
        );
    }

    // An owl on the lighthouse gallery; its head turns.
    let owl = p.node(None, Transform::from_translation(HARBOR + OWL), ());
    if !p.model(owl, "owl_body") {
        p.shape(
            owl,
            Sphere::new(0.12).into(),
            Color::srgb(0.45, 0.35, 0.25),
            Transform::from_xyz(0.0, 0.12, 0.0).with_scale(Vec3::new(1.0, 1.3, 0.9)),
        );
    }
    let head = p.node(
        Some(owl),
        Transform::from_xyz(0.0, 0.27, 0.0),
        Looking {
            target: 0.0,
            wait: 2.0,
        },
    );
    if !p.model(head, "owl_head") {
        p.shape(
            head,
            Sphere::new(0.09).into(),
            Color::srgb(0.5, 0.4, 0.3),
            Transform::from_xyz(0.0, 0.05, 0.0),
        );
        for x in [-0.035, 0.035] {
            p.glow(
                head,
                0.018,
                LinearRgba::rgb(3.0, 2.2, 0.4),
                Vec3::new(x, 0.06, 0.08),
            );
        }
    }

    // Bats circling the lighthouse lamp.
    for i in 0..BATS {
        let bat = p.node(
            None,
            Transform::from_translation(HARBOR + LIGHTHOUSE_TOP),
            Bat {
                radius: rng.range(1.2, 2.4),
                height: rng.range(0.2, 1.1),
                speed: rng.range(0.7, 1.2) * if i % 2 == 0 { 1.0 } else { -1.0 },
                phase: rng.range(0.0, TAU),
            },
        );
        if !p.model(bat, "bat_body") {
            p.shape(bat, Sphere::new(0.035).into(), black, Transform::default());
        }
        for side in [-1.0, 1.0] {
            let wing = p.node(
                Some(bat),
                Transform::from_xyz(side * 0.025, 0.0, 0.0).with_scale(Vec3::new(side, 1.0, 1.0)),
                Wing { side },
            );
            if !p.model(wing, "bat_wing") {
                p.shape(
                    wing,
                    Cuboid::new(0.11, 0.008, 0.06).into(),
                    black,
                    Transform::from_xyz(0.055, 0.0, 0.0),
                );
            }
        }
    }

    // A fish that leaps out of the water from time to time.
    let fish = p.node(
        None,
        Transform::from_translation(HARBOR + Vec3::new(0.0, -2.0, 3.0)),
        Fish {
            from: Vec3::ZERO,
            to: Vec3::ZERO,
            age: 1.0,
            wait: 4.0,
        },
    );
    if !p.model(fish, "fish") {
        p.shape(
            fish,
            Sphere::new(0.07).into(),
            Color::srgb(0.6, 0.68, 0.72),
            Transform::from_scale(Vec3::new(0.5, 0.7, 1.8)),
        );
    }

    // Moths around the lamp post: tiny pale specks, no model needed.
    for i in 0..MOTHS {
        let moth = p.node(
            None,
            Transform::from_translation(HARBOR + LAMP),
            Moth {
                phase: i as f32 * 1.7 + rng.range(0.0, 1.0),
                speed: rng.range(1.5, 3.0),
            },
        );
        p.glow(moth, 0.015, LinearRgba::rgb(1.2, 1.1, 0.9), Vec3::ZERO);
    }
}

fn breathe(time: Res<Time>, mut bodies: Query<(&Breathing, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (b, mut transform) in &mut bodies {
        let s = 1.0 + 0.04 * (t * 1.4 + b.phase).sin();
        transform.scale = Vec3::new(1.0, s, 1.0 + (s - 1.0) * 0.5);
    }
}

fn swish_tails(time: Res<Time>, mut tails: Query<(&Tail, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (tail, mut transform) in &mut tails {
        // Lazy swishes with a quicker flick every few seconds.
        let flick = ((t * 0.5 + tail.phase).sin().max(0.0)).powi(6);
        let angle = 0.35 * (t * 1.3 + tail.phase).sin() + 0.5 * flick * (t * 9.0).sin();
        transform.rotation = Quat::from_rotation_z(angle);
    }
}

fn look_around(
    time: Res<Time>,
    mut rng: ResMut<Rng>,
    mut heads: Query<(&mut Looking, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (mut look, mut transform) in &mut heads {
        look.wait -= dt;
        if look.wait <= 0.0 {
            look.target = rng.range(-1.6, 1.6);
            look.wait = rng.range(1.5, 5.0);
        }
        let (current, _, _) = transform.rotation.to_euler(EulerRot::YXZ);
        let next = current + (look.target - current) * (dt * 4.0).min(1.0);
        transform.rotation = Quat::from_rotation_y(next);
    }
}

fn hop_gulls(time: Res<Time>, mut rng: ResMut<Rng>, mut gulls: Query<(&mut Gull, &mut Transform)>) {
    let dt = time.delta_secs();
    for (mut gull, mut transform) in &mut gulls {
        gull.wait -= dt;
        if gull.wait <= 0.0 && gull.hop <= 0.0 {
            gull.hop = 0.35;
            gull.wait = rng.range(3.0, 9.0);
            // Turn to a new direction with the hop.
            let turn = rng.range(-PI, PI);
            transform.rotation = Quat::from_rotation_y(turn);
        }
        let mut y = 0.0;
        if gull.hop > 0.0 {
            gull.hop -= dt;
            let k = 1.0 - (gull.hop / 0.35).max(0.0);
            y = 4.0 * k * (1.0 - k) * 0.12;
        }
        transform.translation = gull.base + Vec3::Y * y;
    }
}

fn fly_bats(
    time: Res<Time>,
    mut bats: Query<(&Bat, &mut Transform), Without<Wing>>,
    mut wings: Query<(&Wing, &mut Transform), Without<Bat>>,
) {
    let t = time.elapsed_secs();
    for (bat, mut transform) in &mut bats {
        let a = t * bat.speed + bat.phase;
        // A wobbly circle around the lamp, dipping up and down.
        let r = bat.radius * (1.0 + 0.2 * (a * 2.3).sin());
        let offset = Vec3::new(a.cos() * r, bat.height + 0.3 * (a * 3.1).sin(), a.sin() * r);
        transform.translation = HARBOR + LIGHTHOUSE_TOP + offset;
        // Face along the flight direction.
        let heading = if bat.speed > 0.0 { -a } else { PI - a };
        transform.rotation = Quat::from_rotation_y(heading);
    }
    for (wing, mut transform) in &mut wings {
        let flap = 0.9 * (t * 18.0).sin();
        transform.rotation = Quat::from_rotation_z(wing.side * flap);
    }
}

fn jump_fish(
    mut commands: Commands,
    time: Res<Time>,
    mut rng: ResMut<Rng>,
    mesh: Option<Res<RippleMesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut fish: Query<(&mut Fish, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (mut f, mut transform) in &mut fish {
        if f.age >= 1.0 {
            f.wait -= dt;
            transform.translation.y = WATER_Y - 2.0;
            if f.wait > 0.0 {
                continue;
            }
            let start = HARBOR + Vec3::new(rng.range(-6.0, 5.0), WATER_Y, rng.range(1.8, 4.5));
            let dir = if rng.chance(0.5) { 1.0 } else { -1.0 };
            f.from = start;
            f.to = start + Vec3::new(dir * rng.range(0.6, 1.0), 0.0, 0.0);
            f.age = 0.0;
            f.wait = rng.range(4.0, 11.0);
            if let Some(mesh) = &mesh {
                spawn_ripple(
                    &mut commands,
                    mesh,
                    &mut materials,
                    Vec2::new(start.x, start.z),
                    0.6,
                );
            }
        }
        f.age += dt / 0.8;
        let k = f.age.min(1.0);
        let arc = 4.0 * k * (1.0 - k) * 0.45;
        transform.translation = f.from.lerp(f.to, k) + Vec3::Y * arc;
        let dir = (f.to - f.from).normalize_or_zero();
        // Nose up on the way out, down on the way back in.
        let pitch = (1.0 - 2.0 * k) * 0.9;
        transform.rotation =
            Quat::from_rotation_y(dir.x.atan2(dir.z)) * Quat::from_rotation_x(-pitch);
        if f.age >= 1.0
            && let Some(mesh) = &mesh
        {
            spawn_ripple(
                &mut commands,
                mesh,
                &mut materials,
                Vec2::new(f.to.x, f.to.z),
                0.7,
            );
        }
    }
}

fn flutter_moths(time: Res<Time>, mut moths: Query<(&Moth, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (moth, mut transform) in &mut moths {
        let a = t * moth.speed + moth.phase;
        let offset = Vec3::new(
            0.25 * a.cos() + 0.06 * (a * 4.7).sin(),
            0.12 * (a * 1.9).sin() + 0.05 * (a * 6.1).cos(),
            0.25 * a.sin() + 0.06 * (a * 5.3).cos(),
        );
        transform.translation = HARBOR + LAMP + offset;
    }
}
