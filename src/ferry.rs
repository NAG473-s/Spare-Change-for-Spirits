//! The ferryman: every evening his boat glides up to the harbour quay behind
//! the shop, with one crate of every product he offers on deck.
//! Left-click a crate to buy it, right-click to give one back while he is
//! still here. Hovering a crate opens its lid and shows what it costs, what
//! it sells for and how long it keeps. At midnight he poles away again.

mod rigging;

use std::collections::BTreeMap;

use bevy::light::NotShadowCaster;
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::accounts::Accounts;
use crate::data::{GameData, ProductDef, asset_exists};
use crate::day_cycle::{Phase, Progress};
use crate::economy::{Drawer, Purse};
use crate::fx::Cue;
use crate::harbor::{CART, CART_BED_HEIGHT, HARBOR, RippleMesh, float_hull, spawn_ripple};
use crate::i18n::Lang;
use crate::inventory::{CratesOrdered, Stock};
use crate::rng::Rng;
use crate::save::Settings;
use crate::scene::{LabelKind, world_label};
use crate::shop::{model_or_box, product_model};
use crate::ui::{Action, BG_PANEL, GameLog, Icons, PanelBuilder, TEXT_DIM, UiFont, UiTheme};
use crate::upgrades::Upgrades;

pub struct FerryPlugin;

impl Plugin for FerryPlugin {
    fn build(&self, app: &mut App) {
        install_trade(app);
        app.add_message::<CargoWave>()
            .init_resource::<BoughtTonight>()
            .init_resource::<HoveredCrate>()
            .add_systems(OnEnter(Phase::Evening), spawn_boat)
            .add_systems(OnExit(Phase::Evening), cast_off)
            .add_systems(
                Update,
                (
                    (sail_boat, rigging::follow)
                        .chain()
                        .run_if(crate::menu::shop_unpaused),
                    animate_ferryman,
                    name_ferryman,
                    make_wake.after(sail_boat),
                    (sync_cart, fly_crates)
                        .chain()
                        .run_if(in_state(Phase::Evening)),
                    (animate_crates, update_crate_labels, update_tooltip)
                        .run_if(in_state(Phase::Evening)),
                ),
            );
    }
}

pub(crate) fn install_trade(app: &mut App) {
    app.add_systems(Update, trade_crates.run_if(in_state(Phase::Evening)));
}

/// Crates bought per product this evening (they can be given back).
#[derive(Resource, Default)]
pub struct BoughtTonight(pub BTreeMap<String, u32>);

impl BoughtTonight {
    /// Every unit bought tonight, product by product, for the evening panel.
    pub fn units(&self, data: &GameData) -> Vec<String> {
        self.0
            .iter()
            .flat_map(|(id, n)| {
                let size = data.product(id).map_or(0, |p| p.crate_size);
                std::iter::repeat_n(id.clone(), (size * n) as usize)
            })
            .collect()
    }
}

/// The crate under the mouse pointer.
#[derive(Resource, Default)]
struct HoveredCrate(Option<Entity>);

#[derive(Component)]
struct Boat {
    load: rigging::Load,
    /// Seconds since it appeared, for gliding in.
    age: f32,
    /// Seconds since it cast off, once the shop has opened.
    leaving: Option<f32>,
}

impl Boat {
    /// Moving (being poled), not moored.
    fn moving(&self) -> bool {
        self.age < ARRIVE_SECS || self.leaving.is_some()
    }
}

#[derive(Component)]
struct Ferryman {
    /// Counts down after a sale: he nods.
    nod: f32,
}

#[derive(Component)]
struct FerrymanHead;

#[derive(Component)]
struct FerrymanPole;

/// The lantern on its hook at the bow; it swings with the boat.
#[derive(Component)]
struct SwingingLantern;

#[derive(Component)]
struct Crate {
    id: String,
    spec: &'static ContainerSpec,
    /// Hinge of the lid (rotates about x).
    lid: Entity,
    /// The product shown in the crate.
    item: Entity,
    lid_angle: f32,
    lid_velocity: f32,
    lift: f32,
    /// Counts down after a sale or a return: the lid snaps shut and bounces.
    bump: f32,
}

/// A bought crate on its way from the boat into the shop.
#[derive(Component)]
struct FlyingCrate {
    from: Transform,
    age: f32,
}

#[derive(Component)]
struct ReturningToDeck(String);

/// A purchased crate, including those still in flight to their slot.
#[derive(Component)]
struct CartCrate {
    product: String,
    slot: usize,
    base_height: f32,
}

/// The "×2" / "new" label above a crate.
#[derive(Component)]
struct CrateLabel(Entity);

#[derive(Component)]
struct Tooltip(Entity);

/// Where the boat moors at the harbour quay (centre of the deck).
const MOORING: Vec3 = Vec3::new(HARBOR.x + 0.3, 0.0, 0.25);
const BOAT_SIZE: Vec3 = Vec3::new(7.4, 0.45, 1.7);
const DECK_Y: f32 = 0.05;
const CRATE_SIZE: Vec3 = Vec3::new(0.5, 0.3, 0.42);
const CART_SCALE: f32 = 0.85;

/// Look of a container the ferryman ships products in (`data::CONTAINERS`):
/// its size, whether it is round, and the placeholder colours.
struct ContainerSpec {
    id: &'static str,
    size: Vec3,
    round: bool,
    lid_height: f32,
    opening_rate: f32,
    /// Relative loaded mass for the waterline and unloading response.
    weight: f32,
    wide_angle: f32,
    body: Color,
    lid: Color,
    /// Bands or hoops on the placeholder (brass or iron).
    trim: Option<Color>,
}

const CONTAINER_SPECS: [ContainerSpec; 4] = [
    ContainerSpec {
        id: "crate",
        size: CRATE_SIZE,
        round: false,
        lid_height: 0.05,
        opening_rate: 12.0,
        weight: 1.0,
        wide_angle: 1.88,
        body: Color::srgb(0.5, 0.36, 0.22),
        lid: Color::srgb(0.44, 0.31, 0.19),
        trim: None,
    },
    ContainerSpec {
        id: "chest",
        size: Vec3::new(0.5, 0.28, 0.36),
        round: false,
        lid_height: 0.06,
        opening_rate: 7.8,
        weight: 1.45,
        wide_angle: 1.64,
        body: Color::srgb(0.38, 0.17, 0.13),
        lid: Color::srgb(0.32, 0.14, 0.11),
        trim: Some(Color::srgb(0.75, 0.6, 0.3)),
    },
    ContainerSpec {
        id: "basket",
        size: Vec3::new(0.44, 0.24, 0.44),
        round: true,
        lid_height: 0.05,
        opening_rate: 17.0,
        weight: 0.65,
        wide_angle: 2.06,
        body: Color::srgb(0.72, 0.56, 0.32),
        lid: Color::srgb(0.66, 0.5, 0.28),
        trim: None,
    },
    ContainerSpec {
        id: "keg",
        size: Vec3::new(0.38, 0.34, 0.38),
        round: true,
        lid_height: 0.04,
        opening_rate: 10.0,
        weight: 1.25,
        wide_angle: 1.78,
        body: Color::srgb(0.45, 0.3, 0.18),
        lid: Color::srgb(0.4, 0.27, 0.16),
        trim: Some(Color::srgb(0.25, 0.25, 0.27)),
    },
];

fn container_spec(id: &str) -> &'static ContainerSpec {
    CONTAINER_SPECS
        .iter()
        .find(|c| c.id == id)
        .unwrap_or(&CONTAINER_SPECS[0])
}
/// Resting lid angle in radians; each material has its own fully open angle.
const LID_AJAR: f32 = 1.2;
/// The two rows of crates on deck (z relative to the deck centre).
const ROWS_Z: [f32; 2] = [-0.34, 0.32];
const ARRIVE_SECS: f32 = 3.2;
const ARRIVE_FROM: f32 = -13.0;
const LEAVE_TO: f32 = 16.0;
const FLIGHT_SECS: f32 = 0.9;
const TOOLTIP_WIDTH: f32 = 270.0;
const GREEN: Color = Color::srgb(0.55, 0.85, 0.6);
const RED: Color = Color::srgb(0.95, 0.5, 0.45);

/// What one unit sells for tonight, before the ghost's own pay multiplier.
pub fn sale_price(data: &GameData, id: &str, night: u32) -> u32 {
    data.product(id).map_or(0, |p| {
        (p.price as f32 * data.price_factor(night)).round() as u32
    })
}

/// Value of a full crate sold at tonight's price, minus what it costs.
pub fn crate_profit(data: &GameData, id: &str, night: u32) -> i64 {
    data.product(id).map_or(0, |p| {
        (sale_price(data, id, night) * p.crate_size) as i64 - p.crate_cost as i64
    })
}

/// "+12" / "−3" (with a real minus sign).
pub fn signed(n: i64) -> String {
    match n {
        n if n > 0 => format!("+{n}"),
        n if n < 0 => format!("−{}", -n),
        _ => "0".to_string(),
    }
}

/// Why a crate cannot be bought right now, as a catalog key.
fn blocked(
    data: &GameData,
    crates: &CratesOrdered,
    drawer: &Drawer,
    max_crates: u32,
    id: &str,
) -> Option<&'static str> {
    let p = data.product(id)?;
    if crates.0 >= max_crates {
        Some("ferry.tip_full")
    } else if drawer.total() < p.crate_cost {
        Some("ferry.tip_poor")
    } else {
        None
    }
}

struct CratePreview {
    stock_after: u32,
    money_after: i64,
    fade_after_night: u32,
}

impl CratePreview {
    fn new(product: &ProductDef, count: u32, money: u32, night: u32, fade_bonus: u32) -> Self {
        // A new unit starts at age zero and first ages at the end of this night.
        let shelf_life = product.fade_after.saturating_add(fade_bonus);
        Self {
            stock_after: count.saturating_add(product.crate_size),
            money_after: i64::from(money) - i64::from(product.crate_cost),
            fade_after_night: night.saturating_add(shelf_life.saturating_sub(1)),
        }
    }
}

/// Bundles the asset handles the spawning code needs.
struct Builder<'a, 'w, 's> {
    commands: Commands<'w, 's>,
    asset_server: &'a AssetServer,
    meshes: &'a mut Assets<Mesh>,
    materials: &'a mut Assets<StandardMaterial>,
}

impl Builder<'_, '_, '_> {
    fn part(&mut self, parent: Entity, mesh: Mesh, color: Color, at: Transform) -> Entity {
        let mesh = self.meshes.add(mesh);
        let material = self.materials.add(StandardMaterial {
            base_color: color,
            perceptual_roughness: 0.85,
            ..default()
        });
        self.commands
            .spawn((Mesh3d(mesh), MeshMaterial3d(material), at, ChildOf(parent)))
            .id()
    }

    fn glow(&mut self, parent: Entity, mesh: Mesh, emissive: LinearRgba, at: Transform) -> Entity {
        let mesh = self.meshes.add(mesh);
        let material = self.materials.add(StandardMaterial {
            base_color: Color::WHITE,
            emissive,
            ..default()
        });
        self.commands
            .spawn((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                at,
                NotShadowCaster,
                Pickable::IGNORE,
                ChildOf(parent),
            ))
            .id()
    }

    /// A model from `assets/models/shop/<id>.glb` under `parent`, or `None`.
    fn model(&mut self, parent: Entity, id: &str, at: Transform) -> Option<Entity> {
        let path = format!("models/shop/{id}.glb");
        asset_exists(&path).then(|| {
            let e = model_or_box(
                &mut self.commands,
                self.asset_server,
                self.meshes,
                self.materials,
                &path,
                Vec3::ONE,
                Color::NONE,
            );
            self.commands.entity(e).insert((at, ChildOf(parent)));
            e
        })
    }

    /// A crate: body and a lid on a hinge at the back. Returns (crate, lid hinge).
    /// A container (crate, chest, basket or keg): its body and a lid on a
    /// hinge at the back. Returns (container, lid hinge).
    fn crate_box(&mut self, parent: Entity, at: Transform, container: &str) -> (Entity, Entity) {
        let spec = container_spec(container);
        let size = spec.size;
        let root = self
            .commands
            .spawn((at, Visibility::default(), ChildOf(parent)))
            .id();
        if self.model(root, spec.id, Transform::default()).is_none() {
            let body: Mesh = if spec.round {
                Cylinder::new(size.x / 2.0, size.y).into()
            } else {
                Cuboid::from_size(size).into()
            };
            self.part(
                root,
                body,
                spec.body,
                Transform::from_xyz(0.0, size.y / 2.0, 0.0),
            );
            if let Some(trim) = spec.trim {
                // Two bands or hoops round the body.
                for y in [0.25, 0.75] {
                    let band: Mesh = if spec.round {
                        Cylinder::new(size.x / 2.0 + 0.008, 0.025).into()
                    } else {
                        Cuboid::new(size.x + 0.012, 0.025, size.z + 0.012).into()
                    };
                    self.part(root, band, trim, Transform::from_xyz(0.0, size.y * y, 0.0));
                }
            }
            // Straw on top of the goods.
            let straw: Mesh = if spec.round {
                Cylinder::new(size.x / 2.0 - 0.03, 0.02).into()
            } else {
                Cuboid::new(size.x - 0.06, 0.02, size.z - 0.06).into()
            };
            self.part(
                root,
                straw,
                Color::srgb(0.72, 0.6, 0.35),
                Transform::from_xyz(0.0, size.y + 0.005, 0.0),
            );
        }
        let hinge = self
            .commands
            .spawn((
                Transform::from_xyz(0.0, size.y, -size.z / 2.0),
                Visibility::default(),
                ChildOf(root),
            ))
            .id();
        let lid_id = format!("{}_lid", spec.id);
        if self.model(hinge, &lid_id, Transform::default()).is_none() {
            let lid: Mesh = if spec.round {
                Cylinder::new(size.x / 2.0 + 0.01, spec.lid_height).into()
            } else {
                Cuboid::new(size.x + 0.02, spec.lid_height, size.z + 0.02).into()
            };
            self.part(
                hinge,
                lid,
                spec.lid,
                Transform::from_xyz(0.0, spec.lid_height / 2.0, size.z / 2.0),
            );
        }
        (root, hinge)
    }
}

fn spawn_boat(
    commands: Commands,
    old_boats: Query<Entity, With<Boat>>,
    data: Res<GameData>,
    progress: Res<Progress>,
    font: Res<UiFont>,
    theme: Res<UiTheme>,
    lang: Res<Lang>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let night = progress.calendar(&data);
    let mut b = Builder {
        commands,
        asset_server: &asset_server,
        meshes: &mut meshes,
        materials: &mut materials,
    };
    for old in &old_boats {
        b.commands.entity(old).despawn();
    }
    let boat = b
        .commands
        .spawn((
            Boat {
                age: 0.0,
                leaving: None,
                load: default(),
            },
            Transform::from_translation(MOORING + Vec3::X * ARRIVE_FROM),
            Visibility::default(),
        ))
        .id();

    rigging::spawn(&mut b, boat);

    // The hull: a model, or a long dark box with a raised bow and stern.
    if b.model(boat, "ferry_boat", Transform::from_xyz(0.0, DECK_Y, 0.0))
        .is_none()
    {
        let hull = Color::srgb(0.17, 0.11, 0.08);
        b.part(
            boat,
            Cuboid::from_size(BOAT_SIZE).into(),
            hull,
            Transform::from_xyz(0.0, DECK_Y - BOAT_SIZE.y / 2.0, 0.0),
        );
        for side in [-1.0, 1.0] {
            b.part(
                boat,
                Cuboid::new(0.5, 0.3, BOAT_SIZE.z * 0.8).into(),
                hull,
                Transform::from_xyz(side * (BOAT_SIZE.x / 2.0 + 0.1), DECK_Y + 0.05, 0.0)
                    .with_rotation(Quat::from_rotation_z(side * 0.5)),
            );
        }
        // A rim along both long sides.
        for side in [-1.0, 1.0] {
            b.part(
                boat,
                Cuboid::new(BOAT_SIZE.x, 0.1, 0.08).into(),
                Color::srgb(0.24, 0.16, 0.11),
                Transform::from_xyz(0.0, DECK_Y + 0.05, side * (BOAT_SIZE.z / 2.0 - 0.04)),
            );
        }
    }

    // The swinging lantern on a hooked post at the bow.
    let bow = Vec3::new(BOAT_SIZE.x / 2.0 - 0.25, DECK_Y, -0.55);
    let post_color = Color::srgb(0.12, 0.09, 0.07);
    b.part(
        boat,
        Cylinder::new(0.03, 1.5).into(),
        post_color,
        Transform::from_translation(bow + Vec3::Y * 0.75),
    );
    b.part(
        boat,
        Cuboid::new(0.3, 0.03, 0.03).into(),
        post_color,
        Transform::from_translation(bow + Vec3::new(0.12, 1.48, 0.0)),
    );
    let hook = b
        .commands
        .spawn((
            SwingingLantern,
            Transform::from_translation(bow + Vec3::new(0.25, 1.47, 0.0)),
            Visibility::default(),
            ChildOf(boat),
        ))
        .id();
    let lantern = b.glow(
        hook,
        Cuboid::new(0.13, 0.17, 0.13).into(),
        LinearRgba::rgb(6.0, 3.6, 1.4),
        Transform::from_xyz(0.0, -0.17, 0.0),
    );
    b.commands.entity(lantern).insert(PointLight {
        color: Color::srgb(1.0, 0.75, 0.45),
        intensity: 28_000.0,
        range: 4.3,
        radius: 0.10,
        shadow_maps_enabled: false,
        ..default()
    });

    spawn_ferryman(&mut b, boat);

    // One crate per product on offer, in two rows between him and the bow.
    let offer: Vec<_> = data
        .products
        .iter()
        .filter(|p| p.unlock_night <= night)
        .collect();
    let per_row = offer.len().div_ceil(2).max(1);
    let (start, end) = (-BOAT_SIZE.x / 2.0 + 1.1, BOAT_SIZE.x / 2.0 - 0.75);
    let step = ((end - start) / per_row as f32).min(0.7);
    let first_x = (start + end) / 2.0 - (per_row as f32 - 1.0) * step / 2.0;
    for (i, p) in offer.into_iter().enumerate() {
        let (row, col) = (i / per_row, i % per_row);
        let at = Transform::from_xyz(first_x + col as f32 * step, DECK_Y, ROWS_Z[row]);
        let (item, lid) = b.crate_box(boat, at, &p.container);
        let spec = container_spec(&p.container);
        let product = product_model(
            &mut b.commands,
            b.asset_server,
            b.meshes,
            b.materials,
            &p.id,
            Vec3::Y * spec.size.y,
        );
        b.commands.entity(product).insert(ChildOf(item));
        b.commands
            .entity(item)
            .insert(Crate {
                id: p.id.clone(),
                spec,
                lid,
                item: product,
                lid_angle: 0.0,
                lid_velocity: 0.0,
                lift: -0.25,
                bump: 0.0,
            })
            .observe(on_crate_click)
            .observe(on_crate_over)
            .observe(on_crate_out);
        let new = p.unlock_night == night && night > 1;
        b.commands.spawn((
            world_label(
                &font,
                (
                    CrateLabel(item),
                    Text::new(if new {
                        lang.t("ferry.new")
                    } else {
                        String::new()
                    }),
                ),
                LabelKind::Stock,
                item,
                Vec3::Y * (CRATE_SIZE.y + 0.5),
            ),
            DespawnOnExit(Phase::Evening),
        ));
    }

    // Details of the hovered crate, next to the mouse pointer.
    let frame = b
        .commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: px(TOOLTIP_WIDTH),
                max_width: percent(94),
                flex_direction: FlexDirection::Column,
                border_radius: BorderRadius::all(px(4)),
                ..default()
            },
            BackgroundColor(BG_PANEL),
            GlobalZIndex(150),
            Visibility::Hidden,
            Pickable::IGNORE,
            DespawnOnExit(Phase::Evening),
        ))
        .id();
    let body = theme.panel_body(&mut b.commands, frame, 0.23, 13.0, 4.0);
    b.commands.entity(body).insert(Pickable::IGNORE);
    b.commands.entity(frame).insert(Tooltip(body));
}

/// The ferryman already wears his name.
#[derive(Component)]
struct Named;

/// Height of his name above his feet.
const NAME_HEIGHT: f32 = 1.95;

/// Once his own story is finished (`names_ferryman` in story.ron) the
/// ferryman wears his name above his hood.
fn name_ferryman(
    mut commands: Commands,
    data: Res<GameData>,
    progress: Res<Progress>,
    lang: Res<Lang>,
    font: Res<UiFont>,
    men: Query<Entity, (With<Ferryman>, Without<Named>)>,
) {
    let Some(story) = data
        .stories
        .iter()
        .enumerate()
        .find(|(i, s)| s.names_ferryman && progress.chapter(*i) >= s.chapters.len())
        .map(|(_, s)| s)
    else {
        return;
    };
    for man in &men {
        commands.entity(man).insert(Named);
        commands.spawn((
            world_label(
                &font,
                Text::new(lang.ghost(&story.ghost)),
                LabelKind::Name,
                man,
                Vec3::Y * NAME_HEIGHT,
            ),
            DespawnOnExit(Phase::Evening),
        ));
    }
}

/// A tall hooded figure at the stern with a long pole: a model
/// (`models/shop/ferryman.glb`) or a cloak, a hood and two glowing eyes.
fn spawn_ferryman(b: &mut Builder, boat: Entity) {
    let stern = Vec3::new(-BOAT_SIZE.x / 2.0 + 0.5, DECK_Y, 0.05);
    let root = b
        .commands
        .spawn((
            Ferryman { nod: 0.0 },
            Transform::from_translation(stern),
            Visibility::default(),
            ChildOf(boat),
        ))
        .id();
    let cloak = Color::srgb(0.13, 0.13, 0.18);
    let head = b
        .commands
        .spawn((
            FerrymanHead,
            Transform::from_xyz(0.0, 1.12, 0.0),
            Visibility::default(),
            ChildOf(root),
        ))
        .id();
    if b.model(root, "ferryman", Transform::default()).is_none() {
        b.part(
            root,
            Cone {
                radius: 0.32,
                height: 1.2,
            }
            .into(),
            cloak,
            Transform::from_xyz(0.0, 0.6, 0.0),
        );
        b.part(
            head,
            Sphere::new(0.18).into(),
            cloak,
            Transform::from_xyz(0.0, 0.05, 0.0),
        );
        // The dark opening of the hood, with two pale eyes.
        b.part(
            head,
            Sphere::new(0.12).into(),
            Color::srgb(0.02, 0.02, 0.03),
            Transform::from_xyz(0.0, 0.03, 0.1).with_scale(Vec3::new(1.0, 1.1, 0.5)),
        );
        for x in [-0.045, 0.045] {
            b.glow(
                head,
                Sphere::new(0.022).into(),
                LinearRgba::rgb(1.6, 3.2, 3.0),
                Transform::from_xyz(x, 0.04, 0.155),
            );
        }
    }
    // The pole, held at the hand; it pivots there while he punts.
    let hand = b
        .commands
        .spawn((
            FerrymanPole,
            Transform::from_xyz(0.24, 0.85, 0.12),
            Visibility::default(),
            ChildOf(root),
        ))
        .id();
    b.part(
        hand,
        Cylinder::new(0.022, 2.8).into(),
        Color::srgb(0.32, 0.24, 0.16),
        Transform::from_xyz(0.0, -0.2, 0.0),
    );
}

/// When the shop opens the ferryman poles away to the right.
fn cast_off(mut boats: Query<&mut Boat>, mut hovered: ResMut<HoveredCrate>) {
    for mut boat in &mut boats {
        boat.leaving.get_or_insert(0.0);
    }
    hovered.0 = None;
}

#[derive(Message)]
struct CargoWave {
    spot: Vec2,
    size: f32,
}

/// Glides in from the left, rocks at the landing stage, leaves to the right.
fn sail_boat(
    mut commands: Commands,
    time: Res<Time>,
    data: Res<GameData>,
    bought: Res<BoughtTonight>,
    settings: Res<Settings>,
    mut waves: MessageWriter<CargoWave>,
    mut boats: Query<(Entity, &mut Boat, &mut Transform)>,
) {
    let t = time.elapsed_secs();
    let dt = time.delta_secs();
    for (entity, mut boat, mut transform) in &mut boats {
        let still = settings.reduced_motion;
        boat.age = if still { ARRIVE_SECS } else { boat.age + dt };
        if still && boat.leaving.is_some() {
            commands.entity(entity).despawn();
            continue;
        }
        if boat.leaving.is_none() && (bought.is_changed() || boat.load.new_hull()) {
            let delta = boat.load.weigh(&bought, &data);
            if delta != 0.0 && !still {
                waves.write(CargoWave {
                    spot: transform.translation.xz() + Vec2::new(-0.65, 0.94),
                    size: (0.70 + delta.abs() * 0.5).min(2.2),
                });
            }
        }
        let x = if let Some(leaving) = &mut boat.leaving {
            *leaving += dt;
            // Slowly at first, then faster.
            (*leaving / 4.0).powi(2) * LEAVE_TO
        } else {
            let k = (boat.age / ARRIVE_SECS).min(1.0);
            ARRIVE_FROM * (1.0 - k).powi(3)
        };
        if x > LEAVE_TO {
            commands.entity(entity).despawn();
            continue;
        }
        let at = MOORING + Vec3::X * x;
        *transform = if still {
            Transform::from_translation(at)
        } else {
            float_hull(at, 0.0, BOAT_SIZE.xz() * 0.42, t)
        };
        let (rise, heel) = boat.load.settle(dt, still);
        transform.translation.y += rise;
        transform.rotate_local_x(heel);
    }
}

/// He punts while the boat moves, breathes and looks around when moored,
/// and nods after every sale. The bow lantern swings.
fn animate_ferryman(
    time: Res<Time>,
    settings: Res<Settings>,
    boats: Query<&Boat>,
    mut men: Query<(&mut Ferryman, &mut Transform, &ChildOf)>,
    mut heads: Query<&mut Transform, (With<FerrymanHead>, Without<Ferryman>)>,
    mut poles: Query<
        &mut Transform,
        (With<FerrymanPole>, Without<Ferryman>, Without<FerrymanHead>),
    >,
    mut lanterns: Query<
        &mut Transform,
        (
            With<SwingingLantern>,
            Without<Ferryman>,
            Without<FerrymanHead>,
            Without<FerrymanPole>,
        ),
    >,
) {
    let t = time.elapsed_secs();
    let dt = time.delta_secs();
    if settings.reduced_motion {
        for (mut man, mut at, _) in &mut men {
            man.nod = 0.;
            at.rotation = Quat::IDENTITY;
            at.scale = Vec3::ONE;
        }
        for mut at in &mut heads {
            at.rotation = Quat::IDENTITY;
        }
        for mut at in &mut poles {
            at.rotation = Quat::from_rotation_z(0.12);
        }
        for mut at in &mut lanterns {
            at.rotation = Quat::IDENTITY;
        }
        return;
    }
    let mut moving = false;
    for (mut man, mut transform, child_of) in &mut men {
        moving = boats.get(child_of.parent()).is_ok_and(|b| b.moving());
        man.nod = (man.nod - dt).max(0.0);
        let breathe = 1.0 + 0.02 * (t * 1.5).sin();
        transform.scale = Vec3::new(1.0, breathe, 1.0);
        transform.rotation = if moving {
            Quat::from_rotation_z(0.09 * (t * 2.4).sin())
        } else {
            Quat::from_rotation_y(0.25 * (t * 0.35).sin())
        };
        for mut head in &mut heads {
            let nod = (man.nod * std::f32::consts::TAU * 1.5).sin() * man.nod.min(0.4);
            head.rotation =
                Quat::from_rotation_x(nod) * Quat::from_rotation_z(0.06 * (t * 0.8).sin());
        }
    }
    for mut pole in &mut poles {
        pole.rotation = if moving {
            Quat::from_rotation_z(0.35 * (t * 2.4).sin() + 0.1)
        } else {
            Quat::from_rotation_z(0.12 + 0.02 * (t * 1.2).sin())
        };
    }
    for mut lantern in &mut lanterns {
        let swing = if moving { 0.25 } else { 0.08 };
        lantern.rotation = Quat::from_rotation_z(swing * (t * 1.9).sin());
    }
}

/// Rings on the water around the hull; a wake while it moves.
fn make_wake(
    mut commands: Commands,
    mut cargo: MessageReader<CargoWave>,
    settings: Res<Settings>,
    time: Res<Time>,
    mesh: Option<Res<RippleMesh>>,
    mut rng: ResMut<Rng>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    boats: Query<(&Boat, &Transform)>,
) {
    let Some(mesh) = mesh else { return };
    for wave in cargo.read() {
        if !settings.reduced_motion {
            spawn_ripple(&mut commands, &mesh, &mut materials, wave.spot, wave.size);
        }
    }
    if settings.reduced_motion {
        return;
    }
    let dt = time.delta_secs();
    for (boat, transform) in &boats {
        let rate = if boat.moving() { 3.5 } else { 0.6 };
        if !rng.chance(dt * rate) {
            continue;
        }
        let p = transform.translation;
        let (x, size) = if boat.moving() {
            // Behind the stern.
            (p.x - BOAT_SIZE.x / 2.0 - rng.range(0.0, 0.6), 1.0)
        } else {
            (p.x + rng.range(-BOAT_SIZE.x / 2.0, BOAT_SIZE.x / 2.0), 0.8)
        };
        let side = if rng.chance(0.5) { -1.0 } else { 1.0 };
        let z = p.z + side * (BOAT_SIZE.z / 2.0 + rng.range(0.0, 0.2));
        spawn_ripple(&mut commands, &mesh, &mut materials, Vec2::new(x, z), size);
    }
}

fn on_crate_over(over: On<PointerOver>, mut hovered: ResMut<HoveredCrate>) {
    hovered.0 = Some(over.event_target());
}

fn on_crate_out(out: On<PointerOut>, mut hovered: ResMut<HoveredCrate>) {
    if hovered.0 == Some(out.event_target()) {
        hovered.0 = None;
    }
}

/// A click on a crate on the boat: buy one (left) or give one back (right).
/// The same actions come from the harbour panel's crate buttons.
fn on_crate_click(
    click: On<PointerClick>,
    phase: Res<State<Phase>>,
    crates_q: Query<&Crate>,
    mut actions: MessageWriter<Action>,
) {
    if *phase.get() != Phase::Evening {
        return;
    }
    let Ok(item) = crates_q.get(click.event_target()) else {
        return;
    };
    match click.button {
        PointerButton::Primary => {
            actions.write(Action::BuyCrate(item.id.clone()));
        }
        PointerButton::Secondary => {
            actions.write(Action::ReturnCrate(item.id.clone()));
        }
        PointerButton::Middle => {}
    }
}

/// Buys a crate from the ferryman, or gives one back while it is still full.
#[allow(clippy::too_many_arguments)]
fn trade_crates(
    mut actions: MessageReader<Action>,
    mut crates_q: Query<&mut Crate>,
    mut men: Query<&mut Ferryman>,
    data: Res<GameData>,
    (progress, upgrades): (Res<Progress>, Res<Upgrades>),
    lang: Res<Lang>,
    mut stock: ResMut<Stock>,
    mut drawer: ResMut<Drawer>,
    mut crates: ResMut<CratesOrdered>,
    mut bought: ResMut<BoughtTonight>,
    mut accounts: ResMut<Accounts>,
    mut log: ResMut<GameLog>,
    mut cues: MessageWriter<Cue>,
) {
    for action in actions.read() {
        let (id, buy) = match action {
            Action::BuyCrate(id) => (id.clone(), true),
            Action::ReturnCrate(id) => (id.clone(), false),
            _ => continue,
        };
        let Some(p) = data.product(&id) else { continue };
        let mut bump = || {
            for mut item in &mut crates_q {
                if item.id == id {
                    item.bump = 0.5;
                }
            }
        };
        if buy {
            match blocked(
                &data,
                &crates,
                &drawer,
                upgrades.crates_tonight(&data, progress.calendar(&data)),
                &id,
            ) {
                Some("ferry.tip_full") => log.push(lang.t("log.boat_full")),
                Some(_) => log.push(lang.f(
                    "log.crate_too_expensive",
                    &[("item", &lang.product(&id)), ("cost", &p.crate_cost)],
                )),
                None if drawer.pay(p.crate_cost) => {
                    accounts.ferryman += p.crate_cost;
                    crates.0 += 1;
                    *bought.0.entry(id.clone()).or_default() += 1;
                    stock.add(&id, p.crate_size);
                    bump();
                    for mut man in &mut men {
                        man.nod = 0.8;
                    }
                    cues.write(Cue::Paid);
                    log.push(lang.f(
                        "log.ferry_brings",
                        &[("count", &p.crate_size), ("item", &lang.product(&id))],
                    ));
                }
                None => {}
            }
            continue;
        }
        let Some(n) = bought.0.get_mut(&id) else {
            continue;
        };
        // Only while the crate is still full (nothing of it sold or faded).
        if !stock.remove_new(&id, p.crate_size) {
            continue;
        }
        *n -= 1;
        if *n == 0 {
            bought.0.remove(&id);
        }
        crates.0 = crates.0.saturating_sub(1);
        accounts.ferryman = accounts.ferryman.saturating_sub(p.crate_cost);
        drawer.merge(&Purse::greedy(p.crate_cost));
        bump();
        log.push(lang.f("log.ferry_takes_back", &[("item", &lang.product(&id))]));
    }
}

/// Lids open when a crate can be bought, open wide while it is hovered and
/// snap shut for a moment after a sale; the product rises out of the crate.
fn animate_crates(
    time: Res<Time>,
    settings: Res<Settings>,
    hovered: Res<HoveredCrate>,
    data: Res<GameData>,
    progress: Res<Progress>,
    crates: Res<CratesOrdered>,
    drawer: Res<Drawer>,
    upgrades: Res<Upgrades>,
    mut boxes: Query<(Entity, &mut Crate, &mut Transform)>,
    mut parts: Query<&mut Transform, Without<Crate>>,
) {
    let dt = time.delta_secs();
    for (entity, mut item, mut transform) in &mut boxes {
        item.bump = (item.bump - dt).max(0.0);
        let available = blocked(
            &data,
            &crates,
            &drawer,
            upgrades.crates_tonight(&data, progress.calendar(&data)),
            &item.id,
        )
        .is_none();
        let inspecting = hovered.0 == Some(entity);
        let tapping = !settings.reduced_motion && item.bump > 0.25;
        let (lid, lift) = if !available || tapping {
            (0.0, -0.25)
        } else if inspecting {
            (item.spec.wide_angle, 0.11)
        } else {
            (LID_AJAR, 0.0)
        };
        if settings.reduced_motion {
            item.lid_angle = lid;
            item.lid_velocity = 0.0;
            item.lift = lift;
        } else {
            // A critically damped hinge keeps its momentum on a quick pointer reversal.
            // The closed-form step also settles consistently at low frame rates.
            let rate = item.spec.opening_rate * if lid < item.lid_angle { 1.45 } else { 1.0 };
            let offset = item.lid_angle - lid;
            let impulse = item.lid_velocity + rate * offset;
            let decay = (-rate * dt).exp();
            item.lid_angle = lid + (offset + impulse * dt) * decay;
            item.lid_velocity = (item.lid_velocity - rate * impulse * dt) * decay;
            item.lift += (lift - item.lift) * (1.0 - (-11.0 * dt).exp());
        }
        if let Ok(mut hinge) = parts.get_mut(item.lid) {
            hinge.rotation = Quat::from_rotation_x(-item.lid_angle);
        }
        if let Ok(mut product) = parts.get_mut(item.item) {
            product.translation.y = item.spec.size.y + item.lift;
        }
        // The load stays planted on hover. A traded container settles back onto the deck once.
        let tap = if settings.reduced_motion {
            0.0
        } else {
            (item.bump * std::f32::consts::TAU).sin().powi(2)
        };
        transform.translation.y = DECK_Y + tap * 0.002 * item.spec.opening_rate;
    }
}

/// Four supporting columns, each as high as the closed containers underneath.
fn cart_pose(slot: usize, base_height: f32) -> Transform {
    let spot = Vec3::new(
        if slot.is_multiple_of(2) { -0.27 } else { 0.27 },
        CART_BED_HEIGHT + base_height * CART_SCALE,
        if slot % 4 < 2 { -0.20 } else { 0.20 },
    );
    Transform::from_translation(HARBOR + CART + spot).with_scale(Vec3::splat(CART_SCALE))
}

fn stack_container(heights: &mut [f32; 4], slot: usize, spec: &ContainerSpec) -> f32 {
    let column = &mut heights[slot % 4];
    let base = *column;
    *column += spec.size.y + spec.lid_height;
    base
}

/// Match the cart to the purchases after a sale, refund or evening load.
fn sync_cart(
    commands: Commands,
    bought: Res<BoughtTonight>,
    settings: Res<Settings>,
    phase: Res<State<Phase>>,
    data: Res<GameData>,
    deck: Query<(&Crate, &GlobalTransform)>,
    mut cart: Query<(Entity, &mut CartCrate, &mut Transform, Has<FlyingCrate>)>,
    returns: Query<(Entity, &ReturningToDeck, &Transform), Without<CartCrate>>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !bought.is_changed() && !phase.is_changed() {
        return;
    }
    let mut b = Builder {
        commands,
        asset_server: &asset_server,
        meshes: &mut meshes,
        materials: &mut materials,
    };
    let animate = !phase.is_changed() && !settings.reduced_motion;
    let mut returning: Vec<_> = returns.iter().collect();
    let mut missing = bought.0.clone();
    let mut slot = 0;
    let mut heights = [0.0; 4];
    // Keep the oldest copy of each product first. A return can cancel its
    // latest flight, or remove a landed crate beneath another product.
    let mut current: Vec<_> = cart.iter_mut().collect();
    current.sort_by_key(|(_, c, _, _)| c.slot);
    for (entity, mut cargo, mut at, flying) in current {
        let count = missing.get_mut(&cargo.product).filter(|n| **n > 0);
        let Some(count) = count else {
            if animate && deck.iter().any(|(c, _)| c.id == cargo.product) {
                b.commands.entity(entity).remove::<CartCrate>().insert((
                    ReturningToDeck(cargo.product.clone()),
                    FlyingCrate {
                        from: *at,
                        age: 0.0,
                    },
                ));
            } else {
                b.commands.entity(entity).despawn();
            }
            continue;
        };
        *count -= 1;
        let Some(product) = data.product(&cargo.product) else {
            b.commands.entity(entity).despawn();
            continue;
        };
        cargo.base_height = stack_container(&mut heights, slot, container_spec(&product.container));
        cargo.slot = slot;
        if !flying {
            *at = cart_pose(slot, cargo.base_height);
        }
        slot += 1;
    }
    for (id, count) in missing {
        if count == 0 {
            continue;
        }
        let Some(product) = data.product(&id) else {
            continue;
        };
        // Restored purchases are already stacked when the player arrives.
        let deck_pose = deck
            .iter()
            .find(|(c, _)| c.id == id)
            .map(|(_, at)| at.compute_transform());
        for _ in 0..count {
            let cargo = CartCrate {
                product: id.clone(),
                slot,
                base_height: stack_container(
                    &mut heights,
                    slot,
                    container_spec(&product.container),
                ),
            };
            let target = cart_pose(slot, cargo.base_height);
            let (root, start) = if let Some(i) = returning.iter().position(|(_, p, _)| p.0 == id) {
                let (e, _, at) = returning.swap_remove(i);
                b.commands.entity(e).remove::<ReturningToDeck>();
                (e, Some(*at))
            } else {
                let e = b
                    .commands
                    .spawn((Visibility::default(), DespawnOnExit(Phase::Evening)))
                    .id();
                b.crate_box(e, Transform::default(), &product.container);
                (e, deck_pose)
            };
            let mut entity = b.commands.entity(root);
            entity.insert(cargo);
            if let Some(from) = start.filter(|_| animate) {
                entity.insert((from, FlyingCrate { from, age: 0.0 }));
            } else {
                entity.remove::<FlyingCrate>().insert(target);
            }
            slot += 1;
        }
    }
}

/// A refund uses the same physical container, even if it is turned around in midair.
fn fly_crates(
    mut commands: Commands,
    time: Res<Time>,
    settings: Res<Settings>,
    deck: Query<(&Crate, &GlobalTransform)>,
    mut flying: Query<(
        Entity,
        Option<&CartCrate>,
        Option<&ReturningToDeck>,
        &mut FlyingCrate,
        &mut Transform,
    )>,
) {
    for (entity, cargo, returning, mut flight, mut transform) in &mut flying {
        let target = match (cargo, returning) {
            (Some(cargo), _) => cart_pose(cargo.slot, cargo.base_height),
            (_, Some(ware)) => {
                let Some((_, at)) = deck.iter().find(|(c, _)| c.id == ware.0) else {
                    commands.entity(entity).despawn();
                    continue;
                };
                at.compute_transform()
            }
            _ => continue,
        };
        flight.age += time.delta_secs();
        let k = (flight.age / FLIGHT_SECS).min(1.0);
        if k >= 1.0 || settings.reduced_motion {
            if returning.is_some() {
                commands.entity(entity).despawn();
            } else {
                *transform = target;
                commands.entity(entity).remove::<FlyingCrate>();
            }
            continue;
        }
        let travel = k * k * (3.0 - 2.0 * k);
        let arc = 4.0 * travel * (1.0 - travel);
        let tilt = if returning.is_some() { -arc } else { arc };
        transform.translation =
            flight.from.translation.lerp(target.translation, travel) + Vec3::Y * (arc * 0.95);
        transform.scale = flight.from.scale.lerp(target.scale, travel);
        transform.rotation = flight.from.rotation.slerp(target.rotation, travel)
            * Quat::from_rotation_y(tilt * 0.36)
            * Quat::from_rotation_z(tilt * 0.17);
    }
}

/// "×2" above crates bought tonight, "new" above new products.
fn update_crate_labels(
    bought: Res<BoughtTonight>,
    lang: Res<Lang>,
    data: Res<GameData>,
    progress: Res<Progress>,
    crates: Query<&Crate>,
    mut labels: Query<(Ref<CrateLabel>, &mut Text)>,
) {
    let refresh = bought.is_changed() || lang.is_changed() || progress.is_changed();
    for (label, mut text) in &mut labels {
        if !refresh && !label.is_added() {
            continue;
        }
        let Ok(item) = crates.get(label.0) else {
            continue;
        };
        let n = bought.0.get(&item.id).copied().unwrap_or(0);
        let new = data
            .product(&item.id)
            .is_some_and(|p| p.unlock_night == progress.calendar(&data) && p.unlock_night > 1);
        let label = if n > 0 {
            format!("×{n}")
        } else if new {
            lang.t("ferry.new")
        } else {
            String::new()
        };
        let caption = if progress.pinned_for(&item.id) {
            format!("{} {label}", crate::orders::PIN_MARK)
                .trim_end()
                .to_owned()
        } else {
            label
        };
        if text.0 != caption {
            text.0 = caption;
        }
    }
}

fn update_tooltip(
    mut commands: Commands,
    hovered: Res<HoveredCrate>,
    window: Single<&Window, With<PrimaryWindow>>,
    font: Res<UiFont>,
    icons: Res<Icons>,
    lang: Res<Lang>,
    data: Res<GameData>,
    (progress, upgrades): (Res<Progress>, Res<Upgrades>),
    stock: Res<Stock>,
    drawer: Res<Drawer>,
    crates: Res<CratesOrdered>,
    bought: Res<BoughtTonight>,
    crate_q: Query<&Crate>,
    ui_scale: Res<UiScale>,
    mut tooltip: Single<(&Tooltip, &ComputedNode, &mut Node, &mut Visibility)>,
    mut shown: Local<Option<Entity>>,
) {
    let (tooltip, computed, node, visibility) = &mut *tooltip;
    let item = hovered
        .0
        .and_then(|e| crate_q.get(e).ok().map(|c| (e, c.id.clone())));
    let (Some((hovered_entity, id)), Some(cursor)) = (item, window.cursor_position()) else {
        **visibility = Visibility::Hidden;
        *shown = None;
        return;
    };
    **visibility = Visibility::Inherited;
    // In interface units: window pixels divided by the interface scale.
    let s = ui_scale.0;
    let (cursor, win) = (cursor / s, Vec2::new(window.width(), window.height()) / s);
    let width = TOOLTIP_WIDTH.min(win.x * 0.94);
    let height = computed.size().y * computed.inverse_scale_factor();
    // Keep the tooltip beside the pointer and fit the translated text on screen.
    let x = if cursor.x + width + 26.0 <= win.x {
        cursor.x + 18.0
    } else {
        cursor.x - width - 18.0
    };
    let x = x.clamp(8.0, (win.x - width - 8.0).max(8.0));
    let y = (cursor.y - height * 0.65).clamp(8.0, (win.y - height - 8.0).max(8.0));
    node.left = px(x);
    node.top = px(y);

    let changed = progress.is_changed()
        || stock.is_changed()
        || drawer.is_changed()
        || upgrades.is_changed()
        || crates.is_changed()
        || bought.is_changed()
        || lang.is_changed();
    if *shown == Some(hovered_entity) && !changed {
        return;
    }
    *shown = Some(hovered_entity);
    let Some(p) = data.product(&id) else { return };
    let night = progress.calendar(&data);
    let body = tooltip.0;
    let mut ui = PanelBuilder::new(&mut commands, body, &font);
    let heading = ui.node(
        body,
        (
            Node {
                align_items: AlignItems::Center,
                column_gap: px(8),
                ..default()
            },
            Pickable::IGNORE,
        ),
    );
    if let Some(icon) = icons.product(&id) {
        ui.image_sized(heading, icon, Vec2::splat(32.0));
    }
    ui.display_text(
        heading,
        &lang.product(&id),
        17.0,
        Color::srgb_u8(249, 200, 132),
    );
    if progress.pinned_for(&id) {
        ui.text(&lang.t("order_slips.pinned"));
    }
    ui.rule();
    ui.text(&lang.f(
        "ferry.tip_crate",
        &[("size", &p.crate_size), ("cost", &p.crate_cost)],
    ));
    ui.text(&lang.f(
        "ferry.tip_price",
        &[("price", &sale_price(&data, &id, night))],
    ));
    let profit = crate_profit(&data, &id, night);
    ui.add_text(
        body,
        &lang.f("ferry.tip_profit", &[("profit", &signed(profit))]),
        false,
        14.0,
        if profit >= 0 { GREEN } else { RED },
    );
    let count = stock.count(&id);
    let preview = CratePreview::new(
        p,
        count,
        drawer.total(),
        progress.night,
        upgrades.fade_bonus(&data),
    );
    ui.text(&lang.f(
        "preview.crate_stock",
        &[("before", &count), ("after", &preview.stock_after)],
    ));
    let (key, color) = if preview.money_after < 0 {
        ("preview.crate_short", RED)
    } else {
        ("preview.crate_money", crate::ui::TEXT)
    };
    ui.add_text(
        body,
        &lang.f(key, &[("amount", &preview.money_after.abs())]),
        false,
        14.,
        color,
    );
    ui.text(&lang.f(
        "preview.crate_fades",
        &[("night", &preview.fade_after_night)],
    ));
    if let Some(n) = bought.0.get(&id) {
        ui.text(&lang.f("ferry.tip_bought", &[("count", n)]));
    }
    match blocked(
        &data,
        &crates,
        &drawer,
        upgrades.crates_tonight(&data, night),
        &id,
    ) {
        Some("ferry.tip_poor") => {}
        Some(key) => {
            ui.add_text(body, &lang.t(key), false, 13.0, RED);
        }
        None => {
            ui.add_text(body, &lang.t("ferry.tip_controls"), false, 12.0, TEXT_DIM);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_crate_forecasts_the_actual_balance_stock_and_last_night() {
        let data = GameData::load();
        let product = data.product("lavender").unwrap();
        let mut upgrades = Upgrades::default();
        upgrades.0.insert("herb_sachets".into());
        let bonus = upgrades.fade_bonus(&data);
        let night = 13;
        let mut stock = Stock::default();
        stock.add(&product.id, 2);
        let mut drawer = Drawer(Purse {
            spoons: 1,
            ..default()
        });
        let preview = CratePreview::new(
            product,
            stock.count(&product.id),
            drawer.total(),
            night,
            bonus,
        );
        assert!(drawer.pay(product.crate_cost));
        stock.add(&product.id, product.crate_size);
        assert_eq!(preview.money_after, i64::from(drawer.total()));
        assert_eq!(preview.stock_after, stock.count(&product.id));
        for _ in night..preview.fade_after_night {
            assert!(stock.age_and_fade(&data, bonus).is_empty());
        }
        assert_eq!(
            stock.age_and_fade(&data, bonus),
            [(product.id.clone(), preview.stock_after)]
        );
        assert_eq!(
            CratePreview::new(product, 0, product.crate_cost - 1, night, 0).money_after,
            -1
        );
    }

    #[test]
    fn loaded_cart_and_refunds_match_the_purchased_products() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<WorldAsset>()
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .insert_resource(GameData::load())
            .insert_resource(State::new(Phase::Evening))
            .insert_resource(BoughtTonight(BTreeMap::from([
                ("gear".into(), 1),
                ("tea_tin".into(), 1),
            ])))
            .init_resource::<Settings>()
            .add_systems(Update, sync_cart);
        app.world_mut().spawn((
            Crate {
                id: "gear".into(),
                spec: container_spec("crate"),
                lid: Entity::PLACEHOLDER,
                item: Entity::PLACEHOLDER,
                lid_angle: 0.0,
                lid_velocity: 0.0,
                lift: 0.0,
                bump: 0.0,
            },
            Transform::from_translation(MOORING),
        ));
        let contents = |app: &mut App| {
            let world = app.world_mut();
            let mut cargo = world.query::<(&CartCrate, Has<FlyingCrate>)>();
            let mut rows: Vec<_> = cargo
                .iter(world)
                .map(|(c, flying)| (c.slot, c.product.clone(), flying))
                .collect();
            rows.sort();
            rows
        };
        app.update();
        assert_eq!(
            contents(&mut app),
            [(0, "gear".into(), false), (1, "tea_tin".into(), false)]
        );
        app.world_mut()
            .resource_mut::<BoughtTonight>()
            .0
            .remove("gear");
        app.update();
        assert_eq!(contents(&mut app), [(0, "tea_tin".into(), false)]);
        let returning = {
            let w = app.world_mut();
            w.query_filtered::<Entity, With<ReturningToDeck>>()
                .single(w)
                .unwrap()
        };
        app.world_mut()
            .resource_mut::<BoughtTonight>()
            .0
            .insert("gear".into(), 1);
        app.update();
        assert_eq!(
            contents(&mut app),
            [(0, "tea_tin".into(), false), (1, "gear".into(), true)]
        );
        assert!(app.world().get::<CartCrate>(returning).is_some());
        assert!(app.world().get::<ReturningToDeck>(returning).is_none());
        app.world_mut()
            .resource_mut::<BoughtTonight>()
            .0
            .remove("gear");
        app.update();
        assert_eq!(contents(&mut app), [(0, "tea_tin".into(), false)]);
        app.world_mut().resource_mut::<Settings>().reduced_motion = true;
        app.world_mut()
            .resource_mut::<BoughtTonight>()
            .0
            .insert("gear".into(), 1);
        app.update();
        assert_eq!(
            contents(&mut app),
            [(0, "tea_tin".into(), false), (1, "gear".into(), false)]
        );
    }

    #[test]
    fn mixed_cargo_rests_on_the_actual_lid_below_each_slot() {
        let mut heights = [0.; 4];
        for (slot, id) in ["basket", "keg", "chest", "crate"].into_iter().enumerate() {
            assert_eq!(stack_container(&mut heights, slot, container_spec(id)), 0.0);
        }
        for (slot, below) in [(4, "basket"), (5, "keg"), (6, "chest"), (7, "crate")] {
            let spec = container_spec(below);
            let base = stack_container(&mut heights, slot, container_spec("crate"));
            assert_eq!(base, spec.size.y + spec.lid_height);
            let delta = cart_pose(slot, base).translation.y - cart_pose(slot - 4, 0.).translation.y;
            assert!((delta - base * CART_SCALE).abs() < 0.0001);
        }
    }

    #[test]
    fn crates_are_blocked_by_boat_size_and_budget() {
        let data = GameData::load();
        let p = &data.products[0];
        let max = data.crates_tonight(1);
        let rich = Drawer(Purse::greedy(p.crate_cost * 10));
        assert_eq!(blocked(&data, &CratesOrdered(0), &rich, max, &p.id), None);
        assert_eq!(
            blocked(&data, &CratesOrdered(max), &rich, max, &p.id),
            Some("ferry.tip_full")
        );
        let poor = Drawer(Purse::greedy(p.crate_cost - 1));
        assert_eq!(
            blocked(&data, &CratesOrdered(0), &poor, max, &p.id),
            Some("ferry.tip_poor")
        );
    }

    #[test]
    fn signed_numbers() {
        assert_eq!(signed(5), "+5");
        assert_eq!(signed(-3), "−3");
        assert_eq!(signed(0), "0");
    }

    /// The crate tooltip's numbers: tonight's price is the list price times
    /// the night's price factor, rounded; a crate's profit is that price for
    /// every unit minus what the crate costs.
    #[test]
    fn crate_profit_uses_tonights_price() {
        let data = GameData::load();
        let quiet = 1000;
        let rich = data
            .events
            .iter()
            .find(|e| e.price_factor > 1.0)
            .expect("a night with higher prices")
            .night;
        for p in &data.products {
            assert_eq!(sale_price(&data, &p.id, quiet), p.price);
            let tonight = sale_price(&data, &p.id, rich);
            let expected = (p.price as f32 * data.price_factor(rich)).round() as u32;
            assert_eq!(tonight, expected, "{}", p.id);
            assert!(tonight >= p.price, "{}", p.id);
            assert_eq!(
                crate_profit(&data, &p.id, rich),
                (tonight * p.crate_size) as i64 - p.crate_cost as i64,
                "{}",
                p.id
            );
        }
        assert_eq!(sale_price(&data, "not_a_product", quiet), 0);
        assert_eq!(crate_profit(&data, "not_a_product", quiet), 0);
    }
}
