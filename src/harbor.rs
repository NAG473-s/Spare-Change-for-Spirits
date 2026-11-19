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
use bevy::light::NotShadowCaster;
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::data::asset_exists;
use crate::day_cycle::Phase;
use crate::i18n::Lang;
use crate::rng::Rng;
use crate::scene::{Billboard, CAMERA_POS, CAMERA_TARGET, MainCamera, world_label};
use crate::title::spawn_fade;
use crate::ui::{Action, PANEL_EDGE, UiFont, finish, text_font};

pub struct HarborPlugin;

impl Plugin for HarborPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Location>()
            .add_systems(Startup, spawn_harbor)
            .add_systems(OnEnter(Phase::Evening), label_shop_door)
            .add_systems(OnExit(Phase::Evening), |mut location: ResMut<Location>| {
                *location = Location::Shop;
            })
            .add_systems(
                Update,
                (
                    walk.run_if(in_state(Phase::Evening)),
                    translate_wayfinders.run_if(resource_changed::<UiFont>),
                    move_camera.run_if(resource_changed::<Location>),
                    drift_lanterns,
                    drift_mist,
                    spread_ripples,
                    ring_bell,
                    bob_rowboats,
                    sweep_beam,
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
}

/// The harbour's origin in the world. Its local coordinates match the
/// shop's, so the same camera angle frames both.
pub const HARBOR: Vec3 = Vec3::new(-80.0, 0.0, 0.0);
/// Height of the water surface.
pub const WATER_Y: f32 = -0.12;
/// Front edge of the quay (local z): water from here towards the camera.
const QUAY_EDGE: f32 = -0.7;
/// The hand cart on the quay, where bought crates are stacked (local).
pub const CART: Vec3 = Vec3::new(-2.3, 0.0, -2.3);
/// The signpost back to the shop (local).
const SIGNPOST: Vec3 = Vec3::new(-5.0, 0.0, -1.5);
/// Lanterns and mist wrap around between these x positions (local).
const WRAP: (f32, f32) = (-11.0, 13.0);

/// The shop door: in the evening, clicking it leads out to the harbour.
#[derive(Component)]
pub struct ShopDoor;

/// The signpost on the quay: clicking it leads back into the shop.
#[derive(Component)]
struct Signpost;

#[derive(Component)]
struct Wayfinder(&'static str);

#[derive(Component)]
struct Rowboat {
    phase: f32,
    base: Vec3,
    turn: f32,
}

/// Pivot of the lighthouse beam; it turns slowly.
#[derive(Component)]
struct LighthouseBeam;

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
    font: Res<UiFont>,
    lang: Res<Lang>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut rng: ResMut<Rng>,
) {
    // Water: dark and glossy, so the moon and the lamps glint in it.
    let water = meshes.add(Plane3d::default().mesh().size(44.0, 16.0));
    commands.spawn((
        Mesh3d(water),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.03, 0.06, 0.09),
            // Glossy, but not so smooth that the moon becomes a glaring spot.
            perceptual_roughness: 0.38,
            reflectance: 0.3,
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
    for (x, z, w, h, lit) in [
        (-3.8, -7.6, 2.0, 1.7, true),
        (-1.3, -8.0, 1.7, 2.1, false),
        (1.0, -7.7, 2.2, 1.6, true),
        (3.6, -8.2, 1.8, 2.0, true),
    ] {
        let size = Transform::from_xyz(x, 0.0, z).with_scale(Vec3::new(w / 2.2, h / 2.4, 1.0));
        if kit.model("village_house", size).is_some() {
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
        .is_some()
    {
        kit.light(lamp + Vec3::Y * 2.45, 220_000.0, 9.0, true);
    } else {
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
        let light = kit.glow(
            Cuboid::new(0.22, 0.3, 0.22),
            LinearRgba::rgb(6.0, 3.8, 1.5),
            lamp + Vec3::Y * 2.45,
        );
        kit.commands.entity(light).insert(PointLight {
            color: Color::srgb(1.0, 0.75, 0.45),
            intensity: 220_000.0,
            range: 9.0,
            shadow_maps_enabled: true,
            ..default()
        });
    }

    // A small lighthouse at the end of the harbour wall; its beam sweeps
    // over the water.
    let tower = Vec3::new(-6.0, 0.0, -3.6);
    if kit
        .model("lighthouse", Transform::from_translation(tower))
        .is_none()
    {
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
        kit.glow(
            Cylinder::new(0.3, 0.45),
            LinearRgba::rgb(6.0, 5.0, 2.5),
            tower + Vec3::Y * 3.62,
        );
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
    let beam_mesh = kit.meshes.add(
        Cone {
            radius: 1.6,
            height: 9.0,
        }
        .mesh()
        .build()
        // Tip at the lamp, opening outwards along -y before turning.
        .translated_by(Vec3::Y * -4.5),
    );
    let beam_mat = kit.materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.95, 0.7, 0.07),
        alpha_mode: AlphaMode::Add,
        unlit: true,
        cull_mode: None,
        ..default()
    });
    kit.commands.spawn((
        Mesh3d(beam_mesh),
        MeshMaterial3d(beam_mat),
        // Lie the cone down so it points sideways, slightly downwards.
        Transform::from_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2 + 0.12)),
        NotShadowCaster,
        Pickable::IGNORE,
        ChildOf(beam_pivot),
    ));
    kit.commands.spawn((
        SpotLight {
            color: Color::srgb(1.0, 0.92, 0.7),
            intensity: 120_000.0,
            range: 18.0,
            outer_angle: 0.22,
            inner_angle: 0.12,
            shadow_maps_enabled: false,
            ..default()
        },
        // Spot lights shine along -z; aim it out and a little down.
        Transform::from_rotation(
            Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2) * Quat::from_rotation_x(-0.2),
        ),
        ChildOf(beam_pivot),
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
    for x in [-4.6, -0.8, 3.6] {
        let base = Vec3::new(x, 0.0, QUAY_EDGE - 0.12);
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
            Transform::from_translation(CART + Vec3::Y * 0.42),
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
    let sign = kit
        .commands
        .spawn((
            Signpost,
            Transform::from_translation(HARBOR + SIGNPOST),
            Visibility::default(),
        ))
        .observe(on_signpost_click)
        .id();
    if !kit.model_in(sign, "signpost", Transform::default()) {
        let post = kit.meshes.add(Cylinder::new(0.06, 1.9));
        let board = kit.meshes.add(Cuboid::new(0.8, 0.22, 0.05));
        let sign_mat = kit.materials.add(Color::srgb(0.4, 0.28, 0.18));
        kit.commands.spawn((
            Mesh3d(post),
            MeshMaterial3d(sign_mat.clone()),
            Transform::from_xyz(0.0, 0.95, 0.0),
            ChildOf(sign),
        ));
        for (y, turn) in [(1.6, 0.15), (1.3, -0.1)] {
            kit.commands.spawn((
                Mesh3d(board.clone()),
                MeshMaterial3d(sign_mat.clone()),
                Transform::from_xyz(0.25, y, 0.05).with_rotation(Quat::from_rotation_y(turn)),
                ChildOf(sign),
            ));
        }
    }
    spawn_wayfinder(
        &mut kit.commands,
        &font,
        &lang,
        "harbor.back",
        sign,
        Vec3::new(0.22, 2.35, 0.0),
    );

    // Paper lanterns drifting slowly across the water.
    let lantern_model = asset_exists("models/harbor/paper_lantern.glb");
    let lantern_mesh = meshes.add(Cuboid::new(0.14, 0.18, 0.14));
    let float_mesh = meshes.add(Cylinder::new(0.12, 0.04));
    let float_mat = materials.add(Color::srgb(0.18, 0.14, 0.1));
    for i in 0..7 {
        let glow = if i % 3 != 2 {
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

    commands.insert_resource(RippleMesh(meshes.add(Annulus::new(0.92, 1.0))));
}

/// "Harbour" above the shop door in the evening.
fn label_shop_door(
    mut commands: Commands,
    font: Res<UiFont>,
    lang: Res<Lang>,
    door: Option<Single<Entity, With<ShopDoor>>>,
) {
    let Some(door) = door else { return };
    let label = spawn_wayfinder(
        &mut commands,
        &font,
        &lang,
        "harbor.door",
        *door,
        Vec3::new(1.1, 2.5, 0.0),
    );
    commands.entity(label).insert(DespawnOnExit(Phase::Evening));
}

fn spawn_wayfinder(
    commands: &mut Commands,
    font: &UiFont,
    lang: &Lang,
    key: &'static str,
    target: Entity,
    offset: Vec3,
) -> Entity {
    commands
        .spawn(world_label(font, lang.t(key), 13.0, target, offset))
        .insert((
            Wayfinder(key),
            text_font(&font.display, 13.0),
            TextShadow {
                offset: Vec2::splat(1.0),
                ..default()
            },
            Node {
                position_type: PositionType::Absolute,
                width: px(crate::scene::LABEL_WIDTH),
                padding: UiRect::axes(px(5), px(3)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(4)),
                ..default()
            },
            BorderColor::all(PANEL_EDGE),
            finish(Color::srgb_u8(77, 50, 38), Color::srgb_u8(44, 29, 29)),
        ))
        .id()
}

fn translate_wayfinders(
    lang: Res<Lang>,
    font: Res<UiFont>,
    mut labels: Query<(&Wayfinder, &mut Text, &mut TextFont)>,
) {
    for (wayfinder, mut text, mut face) in &mut labels {
        text.0 = lang.t(wayfinder.0);
        face.font = font.display.clone();
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
    mut camera: Single<&mut Transform, With<MainCamera>>,
    mut first: Local<bool>,
) {
    let offset = match *location {
        Location::Shop => Vec3::ZERO,
        Location::Harbor => HARBOR,
    };
    **camera = Transform::from_translation(CAMERA_POS + offset)
        .looking_at(CAMERA_TARGET + offset, Vec3::Y);
    // No fade at startup.
    if *first {
        spawn_fade(&mut commands, 0.6);
    }
    *first = true;
}

/// A white disc that fades out towards its edge.
fn soft_puff(size: u32) -> Image {
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
        transform.translation.y = WATER_Y + 0.02 * (t * 1.4 + lantern.phase).sin();
        transform.rotation = Quat::from_rotation_z(0.06 * (t * 1.1 + lantern.phase).sin())
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
        let p = boat.phase;
        transform.translation = boat.base + Vec3::Y * 0.03 * (t * 1.2 + p).sin();
        transform.rotation = Quat::from_rotation_y(boat.turn + 0.05 * (t * 0.4 + p).sin())
            * Quat::from_rotation_z(0.05 * (t * 1.0 + p).sin())
            * Quat::from_rotation_x(0.03 * (t * 1.3 + p).sin());
    }
}

/// The lighthouse beam sweeps round and round.
fn sweep_beam(time: Res<Time>, mut beams: Query<&mut Transform, With<LighthouseBeam>>) {
    for mut beam in &mut beams {
        beam.rotation = Quat::from_rotation_y(time.elapsed_secs() * 0.6);
    }
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
