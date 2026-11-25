//! The harbour behind the shop: in the evening the player steps out through
//! the shop door to the quay, where the ferryman moors with his crates. A
//! diorama of its own, built far away from the shop; the camera moves there
//! and back with a short fade.
//!
//! Dark water, a quay with the harbour master's hut, a lighthouse whose beam
//! sweeps the water, a jetty with rowboats, the village behind, a lamp post,
//! barrels, mooring posts, a bell, a hand cart for the crates, a signpost
//! back to the shop, paper lanterns drifting past, mist over the water and
//! rings spreading around whatever floats there.

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::light::NotShadowCaster;
use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::data::GameData;
use crate::data::asset_exists;
use crate::day_cycle::Phase;
use crate::day_cycle::Progress;
use crate::fx::Flicker;
use crate::i18n::Lang;
use crate::rng::Rng;
use crate::scene::{
    Billboard, CAMERA_POS, CAMERA_TARGET, MainCamera, SHOP_CAMERA_POS, SHOP_CAMERA_TARGET,
};
use crate::sign::{
    SignReading,
    paint::{Motif, on_board},
    return_post,
};
use crate::title::spawn_fade;
use crate::ui::{Action, GameLog};

pub struct HarborPlugin;

impl Plugin for HarborPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Location>()
            .add_systems(Startup, spawn_harbor)
            .add_systems(PostStartup, spawn_door_boards)
            .add_systems(OnExit(Phase::Evening), |mut location: ResMut<Location>| {
                *location = Location::Shop;
            })
            .add_systems(
                Update,
                (
                    walk.run_if(in_state(Phase::Evening)),
                    update_door_boards,
                    way_hints,
                    move_camera.run_if(resource_changed::<Location>),
                    drift_lanterns,
                    drift_mist,
                    spread_ripples,
                    ring_bell,
                    bob_rowboats,
                    move_waves.run_if(resource_equals(Location::Harbor)),
                    sweep_beam,
                    light_lighthouse,
                ),
            );
    }
}

/// Where the player is: in the shop, or out at the harbour (evenings only).
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Location {
    #[default]
    Shop,
    Harbor,
    /// The village market (by day).
    Market,
}

/// The harbour's origin in the world. Its local coordinates match the
/// shop's, so the same camera angle frames both.
pub const HARBOR: Vec3 = Vec3::new(-80.0, 0.0, 0.0);
/// Mean water level, before the swell is applied.
pub const WATER_Y: f32 = -0.12;
/// Front edge of the quay (local z): water from here towards the camera.
const QUAY_EDGE: f32 = -0.7;
pub(crate) const MOORING_POSTS: [Vec3; 3] = [
    Vec3::new(-4.6, 0.0, QUAY_EDGE - 0.12),
    Vec3::new(-0.8, 0.0, QUAY_EDGE - 0.12),
    Vec3::new(3.6, 0.0, QUAY_EDGE - 0.12),
];
const QUAY_PAVING: &str = "sprites/harbor/quay_stone.png";
/// The hand cart on the quay, where bought crates are stacked (local).
pub const CART: Vec3 = Vec3::new(-2.3, 0.0, -2.3);
pub const CART_BED_HEIGHT: f32 = 0.42;
/// The signpost back to the shop (local).
const SIGNPOST: Vec3 = Vec3::new(-5.0, 0.0, -1.5);
/// Lanterns and mist wrap around between these x positions (local).
const WRAP: (f32, f32) = (-11.0, 13.0);

/// The shop door: in the evening, clicking it leads out to the harbour.
#[derive(Component)]
pub struct ShopDoor;

#[derive(Component)]
struct Rowboat {
    phase: f32,
    base: Vec3,
    turn: f32,
}

/// Pivot of the lighthouse beam; it turns slowly.
#[derive(Component)]
struct LighthouseBeam;

/// Part of the lighthouse that glows: the model (and every mesh in it), the
/// placeholder lamp glass or the halo around the lamp.
#[derive(Component)]
struct LighthouseTower;

/// The spotlight of the lighthouse beam.
#[derive(Component)]
struct BeamLight;

/// What a lighthouse material looked like when it was loaded: the repairs
/// scale from there.
#[derive(Component, Clone, Copy)]
struct LitAtFull {
    emissive: LinearRgba,
    base: Color,
}

/// How bright the lamp glass looks while the lighthouse is dark.
const DARK_GLASS: f32 = 0.3;

/// How visible the mist in the beam is at full light.
const BEAM_MIST_ALPHA: f32 = 0.19;
/// From this share of its repairs on, the lighthouse beam turns again.
const BEAM_FROM: f32 = 0.4;
/// The beam's spotlight when the lighthouse is fully repaired.
const BEAM_INTENSITY: f32 = 430_000.0;

/// How far the lighthouse is repaired, from 0 (dark) to 1 (shining): the
/// story marked `lighthouse` in story.ron lights it chapter by chapter.
/// The demo ends before the first repair.
pub fn lighthouse_level(data: &GameData, progress: &Progress) -> f32 {
    data.stories
        .iter()
        .enumerate()
        .find(|(_, s)| s.lighthouse)
        .map_or(0.0, |(i, s)| {
            progress.chapter(i).min(s.chapters.len()) as f32 / s.chapters.len() as f32
        })
}

/// The lamp glows as brightly as the lighthouse is repaired; the beam turns
/// only once the clockwork runs again.
#[allow(clippy::type_complexity)]
fn light_lighthouse(
    mut commands: Commands,
    data: Res<GameData>,
    progress: Res<Progress>,
    towers: Query<Entity, With<LighthouseTower>>,
    children: Query<&Children>,
    parts: Query<(&MeshMaterial3d<StandardMaterial>, Option<&LitAtFull>)>,
    mist: Query<&MeshMaterial3d<StandardMaterial>, With<BeamMist>>,
    mut pivot: Query<&mut Visibility, With<LighthouseBeam>>,
    mut spot: Query<&mut SpotLight, With<BeamLight>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut shown: Local<Option<f32>>,
) {
    let lamp = lighthouse_level(&data, &progress);
    let beam = if lamp >= BEAM_FROM { lamp } else { 0.0 };
    // The model loads later than the scene: catch its meshes as they come.
    let mut fresh = false;
    for tower in &towers {
        for e in std::iter::once(tower).chain(children.iter_descendants(tower)) {
            let Ok((handle, lit)) = parts.get(e) else {
                continue;
            };
            if lit.is_some() && *shown == Some(lamp) {
                continue;
            }
            let Some(mut m) = materials.get_mut(&handle.0) else {
                continue;
            };
            let full = *lit.unwrap_or(&LitAtFull {
                emissive: m.emissive,
                base: m.base_color,
            });
            if lit.is_none() {
                commands.entity(e).insert(full);
                fresh = true;
            }
            m.emissive = full.emissive * lamp;
            if m.alpha_mode == AlphaMode::Add {
                // A halo fades out.
                m.base_color = full.base.with_alpha(full.base.alpha() * lamp);
            } else if full.emissive != LinearRgba::BLACK {
                // Unlit lamp glass turns dull; the rest of the tower keeps its paint.
                let dull = DARK_GLASS + (1.0 - DARK_GLASS) * lamp;
                let c = full.base.to_linear();
                m.base_color = Color::LinearRgba(LinearRgba::new(
                    c.red * dull,
                    c.green * dull,
                    c.blue * dull,
                    c.alpha,
                ));
            }
        }
    }
    if *shown == Some(lamp) && !fresh {
        return;
    }
    *shown = Some(lamp);
    for handle in &mist {
        if let Some(mut m) = materials.get_mut(&handle.0) {
            m.base_color.set_alpha(BEAM_MIST_ALPHA * beam);
        }
    }
    for mut v in &mut pivot {
        *v = if beam > 0.0 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    for mut s in &mut spot {
        s.intensity = BEAM_INTENSITY * beam;
    }
}

/// The visible light scattered by mist along the spotlight's axis.
#[derive(Component)]
struct BeamMist;

#[derive(Component)]
struct Lantern {
    speed: f32,
    phase: f32,
}

#[derive(Component)]
struct Mist {
    speed: f32,
    phase: f32,
    alpha: f32,
}

#[derive(Component)]
struct Bell;

/// A ring on the water that grows and fades.
#[derive(Component)]
pub struct Ripple {
    age: f32,
    life: f32,
    size: f32,
}

/// Shared mesh for ripples (a thin flat ring of radius 1).
#[derive(Resource)]
pub struct RippleMesh(Handle<Mesh>);

/// Spawning helper: a plain mesh with a matt colour at a harbour position.
struct Kit<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
    asset_server: &'a AssetServer,
    meshes: &'a mut Assets<Mesh>,
    materials: &'a mut Assets<StandardMaterial>,
}

impl Kit<'_, '_, '_> {
    /// The model `assets/models/harbor/<id>.glb` at a harbour position, or
    /// `None` while it has not been made (the caller builds a placeholder).
    fn model(&mut self, id: &str, at: Transform) -> Option<Entity> {
        let path = format!("models/harbor/{id}.glb");
        asset_exists(&path).then(|| {
            self.commands
                .spawn((
                    WorldAssetRoot(
                        self.asset_server
                            .load(GltfAssetLabel::Scene(0).from_asset(path)),
                    ),
                    at.with_translation(HARBOR + at.translation),
                ))
                .id()
        })
    }

    /// Like `model`, but as a child of a moving entity (local transform).
    fn model_in(&mut self, parent: Entity, id: &str, at: Transform) -> bool {
        let Some(e) = self.model(id, at) else {
            return false;
        };
        self.commands.entity(e).insert((at, ChildOf(parent)));
        true
    }

    /// A warm point light at a harbour position (the lamp itself is part of
    /// a model, or drawn by the caller).
    fn light(&mut self, at: Vec3, intensity: f32, range: f32, shadows: bool) -> Entity {
        self.commands
            .spawn((
                PointLight {
                    color: Color::srgb(1.0, 0.75, 0.45),
                    intensity,
                    range,
                    radius: 0.14,
                    shadow_maps_enabled: shadows,
                    ..default()
                },
                Transform::from_translation(HARBOR + at),
            ))
            .id()
    }

    fn add(&mut self, mesh: impl Into<Mesh>, color: Color, at: Transform) -> Entity {
        let mesh = self.meshes.add(mesh.into());
        let material = self.materials.add(StandardMaterial {
            base_color: color,
            perceptual_roughness: 0.9,
            ..default()
        });
        self.commands
            .spawn((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                at.with_translation(HARBOR + at.translation),
                Pickable::IGNORE,
            ))
            .id()
    }

    fn glow(&mut self, mesh: impl Into<Mesh>, emissive: LinearRgba, at: Vec3) -> Entity {
        let mesh = self.meshes.add(mesh.into());
        let material = self.materials.add(StandardMaterial {
            base_color: Color::WHITE,
            emissive,
            ..default()
        });
        self.commands
            .spawn((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::from_translation(HARBOR + at),
                NotShadowCaster,
                Pickable::IGNORE,
            ))
            .id()
    }
}

fn spawn_harbor(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut rng: ResMut<Rng>,
) {
    // The grid carries the swell. The scrolling normal map adds fine ripples
    // that break the moonlight and lamplight into smaller glints.
    let water = Plane3d::default()
        .mesh()
        .size(44.0, 16.0)
        .subdivisions_x(95)
        .subdivisions_z(35)
        .build()
        .with_generated_tangents()
        .expect("a plane has normals and UVs");
    let water = meshes.add(water);
    // A painted texture (`sprites/harbor/water.png`, and optionally its
    // normal map `water_normal.png`) or a calm dark surface with computed
    // waves; either drifts slowly (`move_waves`).
    let painted =
        asset_exists(WATER_TEXTURE).then(|| repeating(&asset_server, WATER_TEXTURE, true));
    let normals = if asset_exists(WATER_NORMALS) {
        repeating(&asset_server, WATER_NORMALS, false)
    } else {
        images.add(wave_normals(128))
    };
    commands.spawn((
        Mesh3d(water),
        Water,
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: if painted.is_some() {
                Color::WHITE
            } else {
                Color::srgb(0.045, 0.16, 0.20)
            },
            base_color_texture: painted,
            // The open sky gives the water a cool undertone between the
            // warmer reflections from the low evening sun and the lamps.
            emissive: LinearRgba::rgb(0.006, 0.024, 0.041),
            perceptual_roughness: 0.31,
            reflectance: 0.32,
            normal_map_texture: Some(normals),
            uv_transform: water_uv(0.0),
            ..default()
        })),
        Transform::from_translation(HARBOR + Vec3::new(1.0, WATER_Y, QUAY_EDGE + 8.0)),
        NotShadowCaster,
        Pickable::IGNORE,
    ));

    let mut kit = Kit {
        commands: &mut commands,
        asset_server: &asset_server,
        meshes: &mut meshes,
        materials: &mut materials,
    };

    // The quay: dark stone reaching back to the village, a wooden
    // boardwalk along the edge and a low wall behind.
    let quay_back = -9.0;
    let quay_depth = QUAY_EDGE - quay_back;
    kit.add(
        Cuboid::new(30.0, 0.7, quay_depth),
        Color::srgb(0.2, 0.2, 0.23),
        Transform::from_xyz(1.0, -0.35, QUAY_EDGE - quay_depth / 2.0),
    );
    if asset_exists(QUAY_PAVING) {
        let top = kit
            .meshes
            .add(Plane3d::default().mesh().size(30.0, quay_depth));
        let stone = kit.materials.add(StandardMaterial {
            base_color_texture: Some(kit.asset_server.load(QUAY_PAVING)),
            perceptual_roughness: 0.76,
            ..default()
        });
        kit.commands.spawn((
            Mesh3d(top),
            MeshMaterial3d(stone),
            Transform::from_translation(
                HARBOR + Vec3::new(1.0, 0.002, QUAY_EDGE - quay_depth / 2.0),
            ),
            Pickable::IGNORE,
        ));
    }
    for i in 0..36 {
        let x = -7.2 + i as f32 * 0.42;
        let tilt = rng.range(-0.03, 0.03);
        let shade = if i % 2 == 0 { 0.3 } else { 0.26 };
        kit.add(
            Cuboid::new(0.38, 0.06, 0.9),
            Color::srgb(shade, shade * 0.7, shade * 0.47),
            Transform::from_xyz(x, 0.03 + rng.range(-0.01, 0.01), QUAY_EDGE - 0.45)
                .with_rotation(Quat::from_rotation_x(tilt)),
        );
    }
    kit.add(
        Cuboid::new(30.0, 0.5, 0.35),
        Color::srgb(0.25, 0.25, 0.28),
        Transform::from_xyz(1.0, 0.25, -4.6),
    );

    // The village behind the harbour: dark houses with a few lit windows.
    for (i, (x, z, w, h, lit)) in [
        (-3.8, -7.6, 2.0, 1.7, true),
        (-1.3, -8.0, 1.7, 2.1, false),
        (1.0, -7.7, 2.2, 1.6, true),
        (3.6, -8.2, 1.8, 2.0, true),
    ]
    .into_iter()
    .enumerate()
    {
        let size = Transform::from_xyz(x, 0.0, z).with_scale(Vec3::new(w / 2.2, h / 2.4, 1.0));
        // Each house its own model if there is one, else the shared one.
        if kit
            .model(&format!("village_house_{}", i + 1), size)
            .is_some()
            || kit.model("village_house", size).is_some()
        {
            continue;
        }
        let wall = Color::srgb(0.14, 0.13, 0.17);
        kit.add(
            Cuboid::new(w, h, 1.8),
            wall,
            Transform::from_xyz(x, h / 2.0, z),
        );
        // A pitched roof from two slanted boards.
        for side in [-1.0, 1.0] {
            kit.add(
                Cuboid::new(w * 0.62, 0.08, 2.0),
                Color::srgb(0.1, 0.09, 0.12),
                Transform::from_xyz(x + side * w * 0.24, h + 0.32, z)
                    .with_rotation(Quat::from_rotation_z(-side * 0.6)),
            );
        }
        if lit {
            kit.glow(
                Cuboid::new(0.4, 0.45, 0.05),
                LinearRgba::rgb(0.9, 0.55, 0.2),
                Vec3::new(x + w * 0.2, h * 0.6, z + 0.92),
            );
        }
    }

    // The harbour master's hut, with a lit window and a lantern.
    let hut = Vec3::new(3.0, 0.0, -2.6);
    let hut_lamp = hut + Vec3::new(-0.1, 1.75, 1.0);
    if kit
        .model("harbour_hut", Transform::from_translation(hut))
        .is_some()
    {
        kit.light(hut_lamp, 60_000.0, 4.0, false);
    } else {
        kit.add(
            Cuboid::new(2.6, 2.0, 1.8),
            Color::srgb(0.27, 0.19, 0.13),
            Transform::from_translation(hut + Vec3::Y * 1.0),
        );
        for side in [-1.0, 1.0] {
            kit.add(
                Cuboid::new(1.65, 0.1, 2.1),
                Color::srgb(0.16, 0.12, 0.1),
                Transform::from_translation(hut + Vec3::new(side * 0.62, 2.35, 0.0))
                    .with_rotation(Quat::from_rotation_z(-side * 0.55)),
            );
        }
        kit.add(
            Cuboid::new(0.7, 1.4, 0.06),
            Color::srgb(0.18, 0.12, 0.08),
            Transform::from_translation(hut + Vec3::new(-0.6, 0.7, 0.91)),
        );
        kit.glow(
            Cuboid::new(0.6, 0.5, 0.06),
            LinearRgba::rgb(1.2, 0.7, 0.28),
            hut + Vec3::new(0.55, 1.25, 0.92),
        );
        kit.glow(Sphere::new(0.08), LinearRgba::rgb(5.0, 3.0, 1.2), hut_lamp);
        kit.light(hut_lamp, 60_000.0, 4.0, false);
    }
    // Barrels and nets beside it.
    let barrel = Color::srgb(0.33, 0.22, 0.13);
    for (x, z, y) in [(5.0, -2.2, 0.0), (5.6, -2.5, 0.0), (5.3, -2.35, 0.75)] {
        let turn = Quat::from_rotation_y(x * 2.0);
        if kit
            .model("barrel", Transform::from_xyz(x, y, z).with_rotation(turn))
            .is_some()
        {
            continue;
        }
        kit.add(
            Cylinder::new(0.3, 0.72),
            barrel,
            Transform::from_xyz(x, y + 0.36, z),
        );
    }

    // Lamp post at the quay.
    let lamp = Vec3::new(0.6, 0.0, QUAY_EDGE - 0.55);
    let iron = Color::srgb(0.1, 0.1, 0.12);
    if kit
        .model("lamp_post", Transform::from_translation(lamp))
        .is_none()
    {
        kit.add(
            Cylinder::new(0.06, 2.6),
            iron,
            Transform::from_translation(lamp + Vec3::Y * 1.3),
        );
        kit.add(
            Cuboid::new(0.3, 0.04, 0.3),
            iron,
            Transform::from_translation(lamp + Vec3::Y * 2.62),
        );
        kit.glow(
            Cuboid::new(0.22, 0.3, 0.22),
            LinearRgba::rgb(6.0, 3.8, 1.5),
            lamp + Vec3::Y * 2.45,
        );
    }
    // The model's metal housing casts shadows. Put the light just below its
    // sill and in front of the post so the pool reaches the planks and ferry.
    kit.light(lamp + Vec3::new(0.0, 2.20, 0.14), 88_000.0, 7.5, true);

    // A small lighthouse at the end of the harbour wall; its beam sweeps
    // over the water.
    let tower = Vec3::new(-6.0, 0.0, -3.6);
    if let Some(model) = kit.model("lighthouse", Transform::from_translation(tower)) {
        kit.commands.entity(model).insert(LighthouseTower);
    } else {
        for (i, (r, h)) in [(0.55, 1.2), (0.45, 1.2), (0.38, 1.0)]
            .into_iter()
            .enumerate()
        {
            let y = [0.6, 1.8, 2.9][i];
            let color = if i % 2 == 0 {
                Color::srgb(0.8, 0.78, 0.74)
            } else {
                Color::srgb(0.55, 0.2, 0.18)
            };
            kit.add(
                Cylinder::new(r, h),
                color,
                Transform::from_translation(tower + Vec3::Y * y),
            );
        }
        let glass = kit.glow(
            Cylinder::new(0.3, 0.45),
            LinearRgba::rgb(6.0, 5.0, 2.5),
            tower + Vec3::Y * 3.62,
        );
        kit.commands.entity(glass).insert(LighthouseTower);
        kit.add(
            Cone {
                radius: 0.42,
                height: 0.45,
            },
            Color::srgb(0.12, 0.12, 0.14),
            Transform::from_translation(tower + Vec3::Y * 4.07),
        );
    }
    let beam_pivot = kit
        .commands
        .spawn((
            LighthouseBeam,
            Transform::from_translation(HARBOR + tower + Vec3::Y * 3.62),
            Visibility::default(),
        ))
        .id();
    let beam_mat = kit.materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.89, 0.66, BEAM_MIST_ALPHA),
        alpha_mode: AlphaMode::Add,
        unlit: true,
        cull_mode: None,
        ..default()
    });
    kit.commands.spawn((
        BeamMist,
        Mesh3d(kit.meshes.add(beam_ribbon())),
        MeshMaterial3d(beam_mat),
        Transform::default().looking_to(BEAM_DIRECTION, Vec3::Y),
        NotShadowCaster,
        Pickable::IGNORE,
        ChildOf(beam_pivot),
    ));
    kit.commands.spawn((
        SpotLight {
            color: Color::srgb(1.0, 0.92, 0.7),
            intensity: BEAM_INTENSITY,
            range: BEAM_RANGE,
            outer_angle: BEAM_ANGLE,
            inner_angle: BEAM_ANGLE * 0.45,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::default().looking_to(BEAM_DIRECTION, Vec3::Y),
        ChildOf(beam_pivot),
        BeamLight,
    ));

    // A wooden jetty on posts on the right, with a rowboat tied to it.
    let jetty_x = 5.9;
    let jetty = Transform::from_xyz(jetty_x, 0.06, QUAY_EDGE + 1.9);
    if kit.model("jetty", jetty).is_none() {
        for i in 0..9 {
            let z = QUAY_EDGE + 0.2 + i as f32 * 0.42;
            kit.add(
                Cuboid::new(1.0, 0.06, 0.38),
                Color::srgb(0.29, 0.2, 0.13),
                Transform::from_xyz(jetty_x, 0.03, z)
                    .with_rotation(Quat::from_rotation_z(rng.range(-0.03, 0.03))),
            );
        }
        for z in [QUAY_EDGE + 1.2, QUAY_EDGE + 3.4] {
            for side in [-0.45, 0.45] {
                kit.add(
                    Cylinder::new(0.06, 0.7),
                    Color::srgb(0.18, 0.13, 0.09),
                    Transform::from_xyz(jetty_x + side, -0.2, z),
                );
            }
        }
    }

    // Rowboats moored here and there; they bob on the water.
    for (x, z, turn) in [(4.7, 1.9, 0.08), (-5.6, 2.2, -0.35)] {
        let boat = kit
            .commands
            .spawn((
                Rowboat {
                    phase: rng.range(0.0, 6.3),
                    base: HARBOR + Vec3::new(x, WATER_Y, z),
                    turn,
                },
                Transform::from_translation(HARBOR + Vec3::new(x, WATER_Y, z)),
                Visibility::default(),
            ))
            .id();
        if kit.model_in(boat, "rowboat", Transform::from_xyz(0.0, -0.1, 0.0)) {
            continue;
        }
        let hull = kit.meshes.add(Cuboid::new(0.7, 0.25, 1.9));
        let seat = kit.meshes.add(Cuboid::new(0.6, 0.05, 0.22));
        let hull_mat = kit.materials.add(Color::srgb(0.32, 0.24, 0.17));
        let seat_mat = kit.materials.add(Color::srgb(0.22, 0.16, 0.11));
        kit.commands.spawn((
            Mesh3d(hull),
            MeshMaterial3d(hull_mat),
            Transform::from_xyz(0.0, 0.08, 0.0),
            Pickable::IGNORE,
            ChildOf(boat),
        ));
        kit.commands.spawn((
            Mesh3d(seat),
            MeshMaterial3d(seat_mat),
            Transform::from_xyz(0.0, 0.22, 0.2),
            Pickable::IGNORE,
            ChildOf(boat),
        ));
    }

    // Mooring posts with coils of rope along the edge.
    for base in MOORING_POSTS {
        if kit
            .model("mooring_post", Transform::from_translation(base))
            .is_some()
        {
            continue;
        }
        kit.add(
            Cylinder::new(0.09, 0.55),
            Color::srgb(0.2, 0.14, 0.1),
            Transform::from_translation(base + Vec3::Y * 0.28),
        );
        kit.add(
            Torus::new(0.07, 0.12),
            Color::srgb(0.55, 0.47, 0.32),
            Transform::from_translation(base + Vec3::Y * 0.35),
        );
    }

    // The ferryman's bell on a post beside the mooring.
    let bell_post = Vec3::new(-6.3, 0.0, QUAY_EDGE - 0.4);
    if kit
        .model("bell_post", Transform::from_translation(bell_post))
        .is_none()
    {
        kit.add(
            Cylinder::new(0.05, 1.9),
            Color::srgb(0.2, 0.14, 0.1),
            Transform::from_translation(bell_post + Vec3::Y * 0.95),
        );
        kit.add(
            Cuboid::new(0.45, 0.05, 0.05),
            Color::srgb(0.2, 0.14, 0.1),
            Transform::from_translation(bell_post + Vec3::new(0.2, 1.88, 0.0)),
        );
    }
    // The bell hangs from the end of the arm and swings about its hook.
    let hook = bell_post + Vec3::new(0.38, 1.85, 0.0);
    let bell = match kit.model("ferry_bell", Transform::from_translation(hook)) {
        Some(bell) => bell,
        None => kit.add(
            Cone {
                radius: 0.12,
                height: 0.2,
            },
            Color::srgb(0.7, 0.55, 0.28),
            Transform::from_translation(hook - Vec3::Y * 0.11),
        ),
    };
    kit.commands.entity(bell).insert(Bell);

    // The hand cart that takes bought crates up to the shop.
    let wood = Color::srgb(0.36, 0.25, 0.16);
    if kit
        .model("hand_cart", Transform::from_translation(CART))
        .is_none()
    {
        kit.add(
            Cuboid::new(1.2, 0.08, 0.8),
            wood,
            Transform::from_translation(CART + Vec3::Y * (CART_BED_HEIGHT - 0.04)),
        );
        for side in [-1.0, 1.0] {
            kit.add(
                Cylinder::new(0.28, 0.06),
                Color::srgb(0.22, 0.15, 0.1),
                Transform::from_translation(CART + Vec3::new(0.0, 0.28, side * 0.45))
                    .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            );
        }
        kit.add(
            Cuboid::new(0.9, 0.05, 0.05),
            wood,
            Transform::from_translation(CART + Vec3::new(-1.0, 0.55, 0.0))
                .with_rotation(Quat::from_rotation_z(0.35)),
        );
    }

    // The signpost back to the shop: click it to go back.
    let sign = return_post(
        kit.commands,
        kit.asset_server,
        kit.meshes,
        kit.materials,
        HARBOR + SIGNPOST,
        "harbor.back",
    );
    kit.commands.entity(sign).observe(on_signpost_click);

    // Paper lanterns drifting slowly across the water.
    let lantern_model = asset_exists("models/harbor/paper_lantern.glb");
    let lantern_mesh = meshes.add(Cuboid::new(0.14, 0.18, 0.14));
    let float_mesh = meshes.add(Cylinder::new(0.12, 0.04));
    let float_mat = materials.add(Color::srgb(0.18, 0.14, 0.1));
    for i in 0..7 {
        let warm = lantern_model || i % 3 != 2;
        let glow = if warm {
            LinearRgba::rgb(4.0, 2.2, 0.8)
        } else {
            LinearRgba::rgb(1.2, 2.4, 3.6)
        };
        let lantern = commands
            .spawn((
                Lantern {
                    speed: rng.range(0.1, 0.22),
                    phase: rng.range(0.0, 6.3),
                },
                Transform::from_translation(
                    HARBOR + Vec3::new(rng.range(WRAP.0, WRAP.1), WATER_Y, rng.range(2.6, 5.0)),
                ),
                Visibility::default(),
                Pickable::IGNORE,
            ))
            .id();
        // Moving, short-range lights put real highlights on the swell as the
        // paper rafts drift. Their shared model has amber paper.
        const LANTERN_LUMENS: f32 = 2_200.0;
        commands.spawn((
            PointLight {
                color: if warm {
                    Color::srgb(1.0, 0.72, 0.41)
                } else {
                    Color::srgb(0.55, 0.78, 1.0)
                },
                intensity: LANTERN_LUMENS,
                radius: 0.09,
                range: 1.65,
                ..default()
            },
            Flicker {
                base: LANTERN_LUMENS,
                phase: i as f32 * 2.4,
            },
            Transform::from_xyz(0.0, 0.18, 0.0),
            ChildOf(lantern),
        ));
        if lantern_model {
            commands.spawn((
                WorldAssetRoot(
                    asset_server.load(
                        GltfAssetLabel::Scene(0).from_asset("models/harbor/paper_lantern.glb"),
                    ),
                ),
                Transform::default(),
                ChildOf(lantern),
            ));
            continue;
        }
        commands.spawn((
            Mesh3d(float_mesh.clone()),
            MeshMaterial3d(float_mat.clone()),
            Transform::from_xyz(0.0, 0.02, 0.0),
            NotShadowCaster,
            Pickable::IGNORE,
            ChildOf(lantern),
        ));
        commands.spawn((
            Mesh3d(lantern_mesh.clone()),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(1.0, 0.85, 0.6),
                emissive: glow,
                ..default()
            })),
            Transform::from_xyz(0.0, 0.13, 0.0),
            NotShadowCaster,
            Pickable::IGNORE,
            ChildOf(lantern),
        ));
    }

    // Mist: soft round puffs that always face the camera.
    let puff = images.add(soft_puff(64));
    let mist_mesh = meshes.add(Rectangle::new(3.4, 1.2));
    for _ in 0..9 {
        let alpha = rng.range(0.06, 0.13);
        commands.spawn((
            Mist {
                speed: rng.range(0.05, 0.12),
                phase: rng.range(0.0, 6.3),
                alpha,
            },
            Mesh3d(mist_mesh.clone()),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgba(0.75, 0.82, 0.95, alpha),
                base_color_texture: Some(puff.clone()),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            })),
            Transform::from_translation(
                HARBOR
                    + Vec3::new(
                        rng.range(WRAP.0, WRAP.1),
                        WATER_Y + 0.35,
                        rng.range(1.6, 6.5),
                    ),
            ),
            Billboard,
            NotShadowCaster,
            Pickable::IGNORE,
        ));
    }

    // Soft halos around the lamps, so the light seems to hang in the mist.
    let glow = if asset_exists(GLOW_TEXTURE) {
        asset_server.load(GLOW_TEXTURE)
    } else {
        images.add(soft_puff(64))
    };
    let lamp_post = Vec3::new(0.6, 2.45, QUAY_EDGE - 0.55);
    let hut_lamp = Vec3::new(2.9, 1.75, -1.6);
    let lighthouse_lamp = Vec3::new(-6.0, 3.62, -3.6);
    for (pos, size, color) in [
        (lamp_post, 1.6, Color::srgba(1.0, 0.7, 0.35, 0.35)),
        (hut_lamp, 0.9, Color::srgba(1.0, 0.7, 0.35, 0.3)),
        (lighthouse_lamp, 2.4, Color::srgba(1.0, 0.92, 0.65, 0.3)),
    ] {
        let halo = commands.spawn((
            Mesh3d(meshes.add(Rectangle::new(size, size))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: color,
                base_color_texture: Some(glow.clone()),
                alpha_mode: AlphaMode::Add,
                unlit: true,
                ..default()
            })),
            Transform::from_translation(HARBOR + pos),
            Billboard,
            NotShadowCaster,
            Pickable::IGNORE,
        ));
        if pos == lighthouse_lamp {
            let mut halo = halo;
            halo.insert(LighthouseTower);
        }
    }

    commands.insert_resource(RippleMesh(meshes.add(Annulus::new(0.92, 1.0))));
}

/// Where a board beside the shop door leads.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Way {
    /// Out to the ferryman, in the evening.
    Harbor,
    /// To the village market, in the morning after the accounts.
    Market,
}

/// One of the two boards beside the shop door (the board itself and its
/// lettering both carry it).
#[derive(Component)]
struct DoorBoard(Way);

/// The boards hang on the wall beside the door, one above the other; the
/// game writes on them. `assets/models/shop/way_board.glb`, otherwise a
/// plain plank. Origin: the centre of the board's back.
const WAY_BOARD_MODEL: &str = "models/shop/way_board.glb";
const WAY_BOARD_SIZE: Vec3 = Vec3::new(0.8, 0.24, 0.039);
const WALL_BOARD_SCALE: Vec3 = Vec3::new(2.17, 1.85, 1.0);
/// From the door's centre towards the back of its wall. The nearest tip is
/// 10 cm from its jamb. The backs sit flush against the plaster.
const WALL_BOARD_OFFSET: Vec3 = Vec3::new(1.52, 0.0, -0.084);
const WAY_BOARDS: [(Way, &str, f32); 2] = [
    (Way::Harbor, "harbor.door", 1.96),
    (Way::Market, "shop.to_market", 1.40),
];

/// What a board does right now: go there, or explain when it can be used.
fn way_action(way: Way, phase: Phase, season_over: bool) -> Action {
    match way {
        Way::Harbor if phase == Phase::Evening => Action::GoToHarbor,
        Way::Market if phase == Phase::Morning && !season_over => Action::NextEvening,
        _ => Action::WayHint(way),
    }
}

fn spawn_door_boards(
    mut commands: Commands,
    door: Single<&Transform, With<ShopDoor>>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mounting = door.translation + door.rotation * WALL_BOARD_OFFSET;
    for (way, key, height) in WAY_BOARDS {
        let board = commands
            .spawn((
                DoorBoard(way),
                SignReading(key),
                Transform::from_translation(mounting.with_y(height))
                    .with_rotation(door.rotation)
                    .with_scale(WALL_BOARD_SCALE),
                Visibility::default(),
            ))
            .observe(on_door_board_click)
            .id();
        if asset_exists(WAY_BOARD_MODEL) {
            commands.spawn((
                WorldAssetRoot(
                    asset_server.load(GltfAssetLabel::Scene(0).from_asset(WAY_BOARD_MODEL)),
                ),
                // Both arrowheads point towards the exit. The lettering keeps
                // its own orientation on the face.
                Transform::from_scale(Vec3::new(-1.0, 1.0, 1.0)),
                ChildOf(board),
            ));
        } else {
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::from_size(WAY_BOARD_SIZE))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.42, 0.29, 0.19),
                    perceptual_roughness: 0.85,
                    ..default()
                })),
                Transform::from_xyz(0.0, 0.0, WAY_BOARD_SIZE.z / 2.0),
                ChildOf(board),
            ));
        }
        let motif = match way {
            Way::Harbor => Motif::Anchor,
            Way::Market => Motif::Awning,
        };
        let ink = on_board(
            &mut commands,
            board,
            key,
            motif,
            Transform::from_xyz(0.026, 0.0, 0.041),
            Vec2::new(0.66, 0.205),
        );
        commands.entity(ink).insert(DoorBoard(way));
    }
}

fn on_door_board_click(
    click: On<PointerClick>,
    boards: Query<&DoorBoard>,
    phase: Res<State<Phase>>,
    progress: Res<Progress>,
    data: Res<GameData>,
    mut actions: MessageWriter<Action>,
) {
    if click.button != PointerButton::Primary {
        return;
    }
    if let Ok(DoorBoard(way)) = boards.get(click.event_target()) {
        actions.write(way_action(*way, *phase.get(), progress.season_over(&data)));
    }
}

/// Weathered paint is quieter while a route is closed. Clicking explains the hours.
fn update_door_boards(
    phase: Res<State<Phase>>,
    progress: Res<Progress>,
    data: Res<GameData>,
    boards: Query<(&DoorBoard, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (DoorBoard(way), paint) in &boards {
        let open = !matches!(
            way_action(*way, *phase.get(), progress.season_over(&data)),
            Action::WayHint(_)
        );
        let wanted = if open {
            Color::WHITE
        } else {
            Color::srgb(0.78, 0.74, 0.73)
        };
        if materials.get(paint).is_some_and(|m| m.base_color != wanted)
            && let Some(mut material) = materials.get_mut(paint)
        {
            material.base_color = wanted;
        }
    }
}

/// A closed way says when it opens.
fn way_hints(mut actions: MessageReader<Action>, lang: Res<Lang>, mut log: ResMut<GameLog>) {
    for action in actions.read() {
        if let Action::WayHint(way) = action {
            log.push(lang.t(match way {
                Way::Harbor => "log.harbor_evening",
                Way::Market => "log.market_morning",
            }));
        }
    }
}

/// Clicking the shop door in the evening goes out to the harbour.
pub fn on_shop_door_click(
    click: On<PointerClick>,
    phase: Res<State<Phase>>,
    mut actions: MessageWriter<Action>,
) {
    if click.button == PointerButton::Primary && *phase.get() == Phase::Evening {
        actions.write(Action::GoToHarbor);
    }
}

fn on_signpost_click(click: On<PointerClick>, mut actions: MessageWriter<Action>) {
    if click.button == PointerButton::Primary {
        actions.write(Action::BackToShop);
    }
}

fn walk(mut actions: MessageReader<Action>, mut location: ResMut<Location>) {
    for action in actions.read() {
        match action {
            Action::GoToHarbor => *location = Location::Harbor,
            Action::BackToShop => *location = Location::Shop,
            _ => {}
        }
    }
}

/// Same camera angle in both places; a short fade hides the jump.
fn move_camera(
    mut commands: Commands,
    location: Res<Location>,
    mut camera: Single<(&mut Transform, &mut Camera), With<MainCamera>>,
    mut first: Local<bool>,
) {
    let (pos, target) = match *location {
        Location::Shop => (SHOP_CAMERA_POS, SHOP_CAMERA_TARGET),
        Location::Harbor => (CAMERA_POS + HARBOR, CAMERA_TARGET + HARBOR),
        Location::Market => (
            CAMERA_POS + crate::market::MARKET,
            CAMERA_TARGET + crate::market::MARKET,
        ),
    };
    let (transform, camera) = &mut *camera;
    **transform = Transform::from_translation(pos).looking_at(target, Vec3::Y);
    camera.clear_color = if *location == Location::Harbor {
        ClearColorConfig::Custom(Color::srgb(0.075, 0.105, 0.17))
    } else {
        ClearColorConfig::Default
    };
    // No fade at startup.
    if *first {
        spawn_fade(&mut commands, 0.6);
    }
    *first = true;
}

const WATER_TEXTURE: &str = "sprites/harbor/water.png";
const WATER_NORMALS: &str = "sprites/harbor/water_normal.png";
/// Soft glow around lamps (a painted one, or a computed soft disc).
const GLOW_TEXTURE: &str = "sprites/fx/glow.png";

/// An image that tiles (repeats) across a surface.
pub(crate) fn repeating(asset_server: &AssetServer, path: &str, srgb: bool) -> Handle<Image> {
    asset_server
        .load_builder()
        .with_settings(move |s: &mut bevy::image::ImageLoaderSettings| {
            s.is_srgb = srgb;
            s.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                address_mode_u: ImageAddressMode::Repeat,
                address_mode_v: ImageAddressMode::Repeat,
                ..ImageSamplerDescriptor::linear()
            });
        })
        .load(path.to_string())
}

/// The harbour water; its waves drift.
#[derive(Component)]
struct Water;

/// How often the wave texture repeats across the water (x, z).
const WAVE_TILES: Vec2 = Vec2::new(5.5, 5.2);

fn water_uv(t: f32) -> bevy::math::Affine2 {
    bevy::math::Affine2::from_scale_angle_translation(
        WAVE_TILES,
        0.0,
        Vec2::new(t * 0.018, t * 0.011),
    )
}

/// A seamless normal map of small overlapping waves.
fn wave_normals(size: u32) -> Image {
    // Many small waves in different directions; whole-number frequencies
    // keep the texture seamless.
    let waves = [
        (Vec2::new(1.0, 2.0), 1.0, 0.3),
        (Vec2::new(3.0, -1.0), 0.7, 1.7),
        (Vec2::new(-2.0, 3.0), 0.6, 4.1),
        (Vec2::new(4.0, 3.0), 0.45, 2.2),
        (Vec2::new(-5.0, -2.0), 0.35, 5.3),
        (Vec2::new(2.0, -6.0), 0.3, 0.9),
        (Vec2::new(-7.0, 4.0), 0.22, 3.4),
        (Vec2::new(8.0, 5.0), 0.18, 1.1),
        (Vec2::new(-3.0, -9.0), 0.15, 2.8),
        (Vec2::new(11.0, -4.0), 0.12, 4.6),
        (Vec2::new(-10.0, 9.0), 0.1, 0.4),
        (Vec2::new(13.0, 7.0), 0.08, 3.9),
    ];
    let strength = 0.004;
    let mut data = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let p = Vec2::new(x as f32, y as f32) / size as f32;
            // Gradient of the summed sines.
            let mut grad = Vec2::ZERO;
            for (k, amp, phase) in waves {
                let a = std::f32::consts::TAU * k.dot(p) + phase;
                grad += k * std::f32::consts::TAU * amp * a.cos();
            }
            let n = Vec3::new(-grad.x * strength, -grad.y * strength, 1.0).normalize();
            let byte = |v: f32| ((v * 0.5 + 0.5) * 255.0).round() as u8;
            data.extend_from_slice(&[byte(n.x), byte(n.y), byte(n.z), 255]);
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        ..ImageSamplerDescriptor::linear()
    });
    image
}

// Wave vector (radians per metre), amplitude (metres), angular speed and phase.
const SWELL: [(Vec2, f32, f32, f32); 3] = [
    (Vec2::new(0.60, 1.12), 0.036, 1.05, 0.4),
    (Vec2::new(-1.28, 0.52), 0.022, 1.39, 2.3),
    (Vec2::new(0.83, -2.10), 0.011, 1.85, 4.7),
];

/// Height above the mean level and slope at a world-space (x, z).
pub(crate) fn swell(at: Vec2, t: f32) -> (f32, Vec2) {
    let local = at - HARBOR.xz();
    let mut height = 0.0;
    let mut slope = Vec2::ZERO;
    for (wave, amplitude, speed, phase) in SWELL {
        let (s, c) = (wave.dot(local) - speed * t + phase).sin_cos();
        height += amplitude * s;
        slope += wave * (amplitude * c);
    }
    (height, slope)
}

fn surface_normal(slope: Vec2) -> Vec3 {
    Vec3::new(-slope.x, 1.0, -slope.y).normalize()
}

/// A hull rests across several crests. Sample opposite sides in its own
/// heading so a short wave cannot tip the whole ferry like a small raft.
pub(crate) fn float_hull(at: Vec3, yaw: f32, half_span: Vec2, t: f32) -> Transform {
    let heading = Quat::from_rotation_y(yaw);
    let x = (heading * Vec3::X).xz() * half_span.x;
    let z = (heading * Vec3::Z).xz() * half_span.y;
    let [left, right, back, front] = [-x, x, -z, z].map(|offset| swell(at.xz() + offset, t).0);
    let slope = Vec2::new(right - left, front - back) / (2.0 * half_span);
    let heave = (left + right + back + front) * 0.25;
    Transform::from_translation(at + Vec3::Y * heave)
        .with_rotation(heading * Quat::from_rotation_arc(Vec3::Y, surface_normal(slope)))
}

fn move_waves(
    time: Res<Time>,
    water: Single<(&Mesh3d, &MeshMaterial3d<StandardMaterial>, &Transform), With<Water>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if time.delta().is_zero() {
        return;
    }
    let (mesh, material, transform) = *water;
    let t = time.elapsed_secs();
    if let Some(mut m) = materials.get_mut(&material.0) {
        m.uv_transform = water_uv(t);
    }
    let Some(mut mesh) = meshes.get_mut(&mesh.0) else {
        return;
    };
    use VertexAttributeValues::{Float32x3, Float32x4};
    let Ok(
        [
            Some((_, Float32x3(positions))),
            Some((_, Float32x3(normals))),
            Some((_, Float32x4(tangents))),
        ],
    ) = mesh.try_disjoint_attributes_mut([
        &Mesh::ATTRIBUTE_POSITION.id,
        &Mesh::ATTRIBUTE_NORMAL.id,
        &Mesh::ATTRIBUTE_TANGENT.id,
    ])
    else {
        return;
    };
    for ((p, n), tangent) in positions.iter_mut().zip(normals).zip(tangents) {
        let (height, slope) = swell(transform.translation.xz() + Vec2::new(p[0], p[2]), t);
        p[1] = height;
        *n = surface_normal(slope).to_array();
        let along_u = Vec3::new(1.0, slope.x, 0.0).normalize();
        *tangent = along_u.extend(tangent[3]).to_array();
    }
}

/// A white disc that fades out towards its edge.
pub(crate) fn soft_puff(size: u32) -> Image {
    let mut data = Vec::with_capacity((size * size * 4) as usize);
    let c = (size as f32 - 1.0) / 2.0;
    for y in 0..size {
        for x in 0..size {
            let d = Vec2::new(x as f32 - c, y as f32 - c).length() / c;
            let a = (1.0 - d).clamp(0.0, 1.0).powf(1.6);
            data.extend_from_slice(&[255, 255, 255, (a * 255.0) as u8]);
        }
    }
    Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

fn wrap(x: f32) -> f32 {
    if x > HARBOR.x + WRAP.1 {
        HARBOR.x + WRAP.0
    } else {
        x
    }
}

fn drift_lanterns(time: Res<Time>, mut lanterns: Query<(&Lantern, &mut Transform)>) {
    let t = time.elapsed_secs();
    let dt = time.delta_secs();
    for (lantern, mut transform) in &mut lanterns {
        transform.translation.x = wrap(transform.translation.x + lantern.speed * dt);
        let (height, slope) = swell(transform.translation.xz(), t);
        transform.translation.y = WATER_Y + height;
        transform.rotation = Quat::from_rotation_arc(Vec3::Y, surface_normal(slope))
            * Quat::from_rotation_y(t * 0.2 + lantern.phase);
    }
}

fn drift_mist(
    time: Res<Time>,
    mut mists: Query<(&Mist, &mut Transform, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let t = time.elapsed_secs();
    let dt = time.delta_secs();
    for (mist, mut transform, material) in &mut mists {
        transform.translation.x = wrap(transform.translation.x + mist.speed * dt);
        // Breathes in and out.
        let a = mist.alpha * (0.6 + 0.4 * (t * 0.3 + mist.phase).sin());
        if let Some(mut m) = materials.get_mut(&material.0) {
            m.base_color.set_alpha(a);
        }
    }
}

fn bob_rowboats(time: Res<Time>, mut boats: Query<(&Rowboat, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (boat, mut transform) in &mut boats {
        let yaw = boat.turn + 0.05 * (t * 0.4 + boat.phase).sin();
        *transform = float_hull(boat.base, yaw, Vec2::new(0.3, 0.8), t);
    }
}

// The light reaches the water about ten metres from the tower. The mist
// follows the same direction and opening angle as the spotlight.
const BEAM_DIRECTION: Vec3 = Vec3::new(1.0, -0.36, 0.0);
const BEAM_ANGLE: f32 = 0.16;
const BEAM_RANGE: f32 = 18.0;

fn beam_ribbon() -> Mesh {
    let across = [-1.0_f32, -0.65, -0.30, 0.0, 0.30, 0.65, 1.0];
    let mut positions = Vec::new();
    let mut colors = Vec::new();
    let mut indices = Vec::new();
    for (row, (fraction, density)) in [
        (0.0, 0.45),
        (0.08, 1.0),
        (0.36, 0.72),
        (0.65, 0.30),
        (0.88, 0.0),
    ]
    .into_iter()
    .enumerate()
    {
        let distance = BEAM_RANGE * fraction;
        let radius = 0.06 + distance * BEAM_ANGLE.tan();
        for (col, u) in across.into_iter().enumerate() {
            positions.push([radius * u, 0.0, -distance]);
            colors.push([1.0, 1.0, 1.0, density * (1.0 - u * u).powi(3)]);
            if row > 0 && col > 0 {
                let i = (row * across.len() + col) as u32;
                let above = i - across.len() as u32;
                indices.extend([above - 1, above, i, above - 1, i, i - 1]);
            }
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        vec![[0.0, 1.0, 0.0]; positions.len()],
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(indices))
}

fn sweep_beam(
    time: Res<Time>,
    mut pivot: Single<&mut Transform, (With<LighthouseBeam>, Without<BeamMist>)>,
    mut mist: Single<&mut Transform, (With<BeamMist>, Without<LighthouseBeam>)>,
) {
    pivot.rotation = Quat::from_rotation_y(time.elapsed_secs() * 0.6);
    let to_camera = HARBOR + CAMERA_POS - pivot.translation;
    mist.look_to(BEAM_DIRECTION, pivot.rotation.inverse() * to_camera);
}

/// The bell sways in the night wind.
fn ring_bell(time: Res<Time>, mut bells: Query<&mut Transform, With<Bell>>) {
    let t = time.elapsed_secs();
    for mut bell in &mut bells {
        bell.rotation = Quat::from_rotation_z(0.12 * (t * 1.3).sin() + 0.05 * (t * 3.1).sin());
    }
}

/// Spawns a ring at `pos` on the water (world x, z) that spreads and fades.
pub fn spawn_ripple(
    commands: &mut Commands,
    mesh: &RippleMesh,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec2,
    size: f32,
) {
    commands.spawn((
        Ripple {
            age: 0.0,
            life: 2.4,
            size,
        },
        Mesh3d(mesh.0.clone()),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(0.7, 0.8, 0.95, 0.0),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        })),
        Transform::from_xyz(pos.x, WATER_Y + 0.01, pos.y)
            .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))
            .with_scale(Vec3::splat(0.1)),
        NotShadowCaster,
        Pickable::IGNORE,
    ));
}

/// Rings grow and fade; the drifting lanterns leave some behind too.
fn spread_ripples(
    mut commands: Commands,
    time: Res<Time>,
    mesh: Option<Res<RippleMesh>>,
    mut rng: ResMut<Rng>,
    mut ripples: Query<(
        Entity,
        &mut Ripple,
        &mut Transform,
        &MeshMaterial3d<StandardMaterial>,
    )>,
    lanterns: Query<&Transform, (With<Lantern>, Without<Ripple>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let dt = time.delta_secs();
    for (entity, mut ripple, mut transform, material) in &mut ripples {
        ripple.age += dt;
        let k = ripple.age / ripple.life;
        if k >= 1.0 {
            materials.remove(&material.0);
            commands.entity(entity).despawn();
            continue;
        }
        transform.scale = Vec3::splat(0.1 + ripple.size * k);
        let (height, slope) = swell(transform.translation.xz(), time.elapsed_secs());
        transform.translation.y = WATER_Y + height + 0.012;
        transform.rotation = Quat::from_rotation_arc(Vec3::Y, surface_normal(slope))
            * Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
        if let Some(mut m) = materials.get_mut(&material.0) {
            m.base_color.set_alpha(0.1 * (1.0 - k) * (k * 8.0).min(1.0));
        }
    }
    let Some(mesh) = mesh else { return };
    for lantern in &lanterns {
        if rng.chance(dt * 0.25) {
            let p = lantern.translation;
            spawn_ripple(
                &mut commands,
                &mesh,
                &mut materials,
                Vec2::new(p.x, p.z),
                0.5,
            );
        }
    }
}
