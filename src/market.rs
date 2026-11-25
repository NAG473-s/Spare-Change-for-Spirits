//! The village market, open by day between the morning accounts and the
//! evening (from the second day on). Seven traders of the other side sell
//! from their stalls (`assets/data/market.ron`): furniture as small samples
//! on the table, the raven changes coins, the Reaper sells rare single
//! pieces and an hourglass that makes the coming night longer.
//!
//! Bought furniture waits in the storeroom until it is placed in the evening.
//! Left-click a ware to buy it, right-click to sell furniture back the same
//! day. Like the harbour, the market is a diorama of its own away from the
//! shop; the camera moves there for the day.

mod basket;

use bevy::light::NotShadowCaster;
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;

use crate::accounts::Accounts;
use crate::data::{Exchange, GameData, Ware, asset_exists, rgb};
use crate::day_cycle::{Phase, Progress};
use crate::economy::{Coin, Drawer, Purse};
use crate::fx::Cue;
use crate::harbor::{Location, repeating};
use crate::i18n::Lang;
use crate::inventory::Stock;
use crate::rng::Rng;
use crate::save::Settings;
use crate::scene::{LabelKind, billboard_material, billboard_mesh, world_label};
use crate::shop::{Storeroom, model_or_box, product_model};
use crate::sign::return_post;
use crate::tooltip::{HoverTip, TipKind};
use crate::ui::{Action, GameLog, Icons, PanelBuilder, UiFont};
use crate::upgrades::Upgrades;

pub struct MarketPlugin;

impl Plugin for MarketPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<basket::Trade>()
            .init_resource::<MarketDay>()
            .init_resource::<HoveredWare>()
            .add_systems(Startup, spawn_market)
            .add_systems(OnEnter(Phase::Day), (open_market, spawn_vendor_plaques))
            .add_systems(OnExit(Phase::Day), close_market)
            .add_systems(
                Update,
                (
                    (animate_vendors, sway_bunting)
                        .run_if(resource_equals(Location::Market))
                        .run_if(crate::menu::shop_unpaused),
                    (
                        basket::handoffs,
                        basket::arrange,
                        mark_sold.run_if(resource_changed::<MarketDay>),
                    )
                        .chain()
                        .after(buy_from_list)
                        .run_if(in_state(Phase::Day))
                        .run_if(crate::menu::shop_unpaused),
                    (ware_tooltip, animate_wares, buy_from_list).run_if(in_state(Phase::Day)),
                ),
            );
    }
}

/// The market's origin in the world (its local coordinates match the shop's,
/// so the same camera angle frames it).
pub const MARKET: Vec3 = Vec3::new(-160.0, 0.0, 0.0);
/// The stall's table (w, h, d); wares stand on it.
const TABLE: Vec3 = Vec3::new(1.7, 0.85, 0.7);
/// At the front rail, towards the trader's end of the stall.
const PLAQUE_OFFSET: Vec3 = Vec3::new(-0.65, 0.51, 0.48);
/// Furniture is shown as a small sample: its largest side, in metres.
const SAMPLE_SIZE: f32 = 0.5;
/// A rolled-up plan stands for improvements without a model of their own
/// (clearing the floor): `assets/models/market/plan.glb`.
const PLAN_MODEL: &str = "models/market/plan.glb";
const PLAN_SIZE: Vec3 = Vec3::new(0.4, 0.12, 0.12);
const PLAN_COLOR: Color = Color::srgb(0.88, 0.8, 0.62);
/// The Reaper's hourglass: price, and how much longer the night gets.
pub const HOURGLASS_PRICE: u32 = 12;
const HOURGLASS_BONUS: f32 = 1.25;
/// The raven keeps this many buttons when he changes a coin.
const CHANGE_FEE: u32 = 1;
/// Where the signpost back to the shop stands (local).
const SIGNPOST: Vec3 = Vec3::new(-3.40, 0.0, 4.65);
/// Where a trader stands: beside the stall's table, so the awning does not
/// hide them (the raven sits on a perch on the other side).
const TRADER_SPOT: Vec3 = Vec3::new(-1.15, 0.0, 0.05);
const PERCH_SPOT: Vec3 = Vec3::new(1.1, 0.0, -0.05);
/// The misty sky of a market day, and the night sky it replaces.
const DAY_SKY: Color = Color::srgb(0.5, 0.56, 0.64);
const NIGHT_SKY: Color = Color::srgb(0.04, 0.04, 0.07);
const SQUARE_PAVING: &str = "sprites/market/square.png";
const COBBLES: &str = "sprites/market/cobbles.png";

/// The offers and purchases for one market day, kept when the player saves.
#[derive(Resource, Default, Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MarketDay {
    night: u32,
    /// A letter found behind the cleared junk, read the following evening.
    letter: Option<u32>,
    /// Furniture bought today (it may be sold back until the evening).
    bought: Vec<String>,
    hourglass: bool,
    /// Single pieces already sold today (the Reaper's and the seasonal ones).
    rare_sold: Vec<(usize, String)>,
    /// Today's wares of each trader (index in `market.ron`) and whether each
    /// is a special offer. Empty for a trader who is not here today.
    offers: Vec<Vec<(WareKind, bool)>>,
    /// The trader whose wares the side panel lists, if any.
    #[serde(skip)]
    pub stall: Option<usize>,
}

#[derive(Resource, Default)]
struct HoveredWare(Option<Entity>);

#[derive(Component)]
struct Stall(usize);

/// A trader behind the stall: nods after a sale and moves a little.
#[derive(Component)]
struct VendorFigure {
    vendor: usize,
    kind: FigureKind,
    nod: f32,
    phase: f32,
}

impl FigureKind {
    fn spot(self) -> Vec3 {
        match self {
            FigureKind::Bird => PERCH_SPOT,
            _ => TRADER_SPOT,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FigureKind {
    Standing,
    /// A will-o'-the-wisp floats and glows.
    Wisp,
    /// The raven hops and turns its head.
    Bird,
}

#[derive(Component)]
struct WareItem {
    vendor: usize,
    kind: WareKind,
    /// Today's special offer: cheaper.
    special: bool,
}

#[derive(Clone, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
enum WareKind {
    Decor(String),
    Exchange(Exchange),
    Hourglass,
    Rare(String),
    /// One piece of a product from a seasonal trader, every day.
    Single(String),
    /// A shop improvement; it leaves the table once bought.
    Upgrade(String),
}

/// The visible part of a ware; it lifts and grows a little when hovered,
/// while the ware's hover box stays where it is.
#[derive(Component)]
struct WareVisual;

#[derive(Component)]
struct SoldTicket;

/// Size of a ware's hover box (its width is its place on the table).
const HIT_HEIGHT: f32 = 0.7;
const HIT_DEPTH: f32 = 0.5;

/// A pennant on the bunting; it flutters.
#[derive(Component)]
struct Pennant {
    phase: f32,
}

/// Spawning helper with the asset collections.
struct Kit<'a, 'w, 's> {
    commands: Commands<'w, 's>,
    asset_server: &'a AssetServer,
    meshes: &'a mut Assets<Mesh>,
    materials: &'a mut Assets<StandardMaterial>,
}

impl Kit<'_, '_, '_> {
    fn part(
        &mut self,
        parent: Entity,
        mesh: impl Into<Mesh>,
        color: Color,
        at: Transform,
    ) -> Entity {
        let mesh = self.meshes.add(mesh.into());
        let material = self.materials.add(StandardMaterial {
            base_color: color,
            perceptual_roughness: 0.85,
            ..default()
        });
        self.commands
            .spawn((Mesh3d(mesh), MeshMaterial3d(material), at, ChildOf(parent)))
            .id()
    }

    fn glow(&mut self, parent: Entity, radius: f32, emissive: LinearRgba, at: Vec3) -> Entity {
        let mesh = self.meshes.add(Sphere::new(radius));
        let material = self.materials.add(StandardMaterial {
            base_color: Color::WHITE,
            emissive,
            ..default()
        });
        self.commands
            .spawn((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::from_translation(at),
                NotShadowCaster,
                ChildOf(parent),
            ))
            .id()
    }

    /// `assets/<path>` under `parent` if it exists.
    fn model(&mut self, parent: Entity, path: &str, at: Transform) -> bool {
        if !asset_exists(path) {
            return false;
        }
        self.commands.spawn((
            WorldAssetRoot(
                self.asset_server
                    .load(GltfAssetLabel::Scene(0).from_asset(path.to_string())),
            ),
            at,
            ChildOf(parent),
        ));
        true
    }

    fn node(&mut self, parent: Option<Entity>, at: Transform) -> Entity {
        let mut e = self.commands.spawn((at, Visibility::default()));
        if let Some(parent) = parent {
            e.insert(ChildOf(parent));
        }
        e.id()
    }
}

fn spawn_market(
    commands: Commands,
    data: Res<GameData>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rng: ResMut<Rng>,
) {
    let mut k = Kit {
        commands,
        asset_server: &asset_server,
        meshes: &mut meshes,
        materials: &mut materials,
    };
    let root = k.node(None, Transform::from_translation(MARKET));

    // Cobbled square: a painted texture or plain grey stone.
    let (cobbles, repeat) = if asset_exists(SQUARE_PAVING) {
        (Some(k.asset_server.load(SQUARE_PAVING)), Vec2::ONE)
    } else {
        (
            asset_exists(COBBLES).then(|| repeating(k.asset_server, COBBLES, true)),
            Vec2::new(8.0, 5.0),
        )
    };
    let ground = k.meshes.add(Plane3d::default().mesh().size(32.0, 20.0));
    let ground_mat = k.materials.add(StandardMaterial {
        base_color: if cobbles.is_some() {
            Color::WHITE
        } else {
            Color::srgb(0.36, 0.34, 0.33)
        },
        base_color_texture: cobbles,
        uv_transform: bevy::math::Affine2::from_scale(repeat),
        perceptual_roughness: 0.9,
        ..default()
    });
    k.commands.spawn((
        Mesh3d(ground),
        MeshMaterial3d(ground_mat),
        Transform::from_xyz(1.0, 0.0, -1.0),
        ChildOf(root),
    ));

    // Houses round the square (the village's models, or plain blocks).
    for (i, (x, z, w, h)) in [
        (-5.0, -6.0, 2.2, 2.2),
        (-2.2, -6.6, 1.8, 2.6),
        (0.6, -6.2, 2.4, 2.0),
        (3.4, -6.7, 2.0, 2.5),
        (6.0, -6.1, 2.2, 2.1),
    ]
    .into_iter()
    .enumerate()
    {
        let at = Transform::from_xyz(x, 0.0, z).with_scale(Vec3::new(w / 2.2, h / 2.4, 1.0));
        let house = k.node(Some(root), at);
        let variant = format!("models/harbor/village_house_{}.glb", i % 4 + 1);
        if !k.model(house, &variant, Transform::default())
            && !k.model(
                house,
                "models/harbor/village_house.glb",
                Transform::default(),
            )
        {
            k.part(
                house,
                Cuboid::new(2.2, 2.4, 1.8),
                Color::srgb(0.55, 0.5, 0.48),
                Transform::from_xyz(0.0, 1.2, 0.0),
            );
            for side in [-1.0, 1.0] {
                k.part(
                    house,
                    Cuboid::new(1.4, 0.1, 2.0),
                    Color::srgb(0.35, 0.22, 0.2),
                    Transform::from_xyz(side * 0.52, 2.72, 0.0)
                        .with_rotation(Quat::from_rotation_z(-side * 0.6)),
                );
            }
        }
    }

    // A string of pennants across the back of the square.
    let colors: Vec<Color> = data.vendors.iter().map(|v| rgb(v.color)).collect();
    let (from, to) = (Vec3::new(-6.8, 3.1, -3.6), Vec3::new(7.0, 3.1, -3.6));
    let count = 30;
    for i in 0..count {
        let t = i as f32 / (count - 1) as f32;
        let sag = 0.55 * 4.0 * t * (1.0 - t);
        let pos = from.lerp(to, t) - Vec3::Y * sag;
        let pennant = k.node(Some(root), Transform::from_translation(pos));
        k.commands.entity(pennant).insert(Pennant {
            phase: i as f32 * 0.7 + rng.range(0.0, 1.0),
        });
        let color = colors
            .get(i % colors.len().max(1))
            .copied()
            .unwrap_or(Color::WHITE);
        k.part(
            pennant,
            Triangle3d::new(
                Vec3::new(-0.13, 0.0, 0.0),
                Vec3::new(0.13, 0.0, 0.0),
                Vec3::new(0.0, -0.3, 0.0),
            ),
            color,
            Transform::default(),
        );
    }

    // The way back to the shop.
    let sign = return_post(
        &mut k.commands,
        k.asset_server,
        k.meshes,
        k.materials,
        SIGNPOST,
        "market.back",
    );
    k.commands
        .entity(sign)
        .insert(ChildOf(root))
        .observe(on_signpost_click);

    // The stalls and their traders.
    for (i, v) in data.vendors.iter().enumerate() {
        if cfg!(feature = "demo") && v.from_night > data.config.final_night() {
            continue;
        }
        let at = Transform::from_xyz(v.pos.0, 0.0, v.pos.1)
            .with_rotation(Quat::from_rotation_y(v.turn.to_radians()));
        let stall = k.node(Some(root), at);
        k.commands.entity(stall).insert(Stall(i));
        let path = format!("models/market/stall_{}.glb", v.id);
        if !k.model(stall, &path, Transform::default()) {
            spawn_stall_placeholder(&mut k, stall, rgb(v.color));
        }
        let kind = match v.id.as_str() {
            "candlemaker" => FigureKind::Wisp,
            "raven" => FigureKind::Bird,
            _ => FigureKind::Standing,
        };
        let figure = k.node(Some(stall), Transform::from_translation(kind.spot()));
        k.commands.entity(figure).insert(VendorFigure {
            vendor: i,
            kind,
            nod: 0.0,
            phase: i as f32 * 1.3,
        });
        let path = format!("models/market/vendor_{}.glb", v.id);
        if !k.model(figure, &path, Transform::default()) {
            spawn_vendor_placeholder(&mut k, figure, &v.id);
        }
        if kind == FigureKind::Wisp {
            k.commands.entity(figure).with_child((
                PointLight {
                    color: Color::srgb(0.6, 0.95, 1.0),
                    intensity: 15_000.0,
                    range: 2.8,
                    radius: 0.18,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_xyz(0.0, 1.3, 0.0),
            ));
        }
    }
}

fn spawn_vendor_plaques(
    mut commands: Commands,
    stalls: Query<(Entity, &Stall)>,
    data: Res<GameData>,
    lang: Res<Lang>,
    font: Res<UiFont>,
    progress: Res<Progress>,
) {
    for (stall, Stall(index)) in &stalls {
        if !data.vendors[*index].here(&data, progress.calendar(&data)) {
            continue;
        }
        let key = format!("vendor.{}.name", data.vendors[*index].id);
        commands.spawn((
            world_label(
                &font,
                Text::new(lang.t(&key)),
                LabelKind::Sign,
                stall,
                PLAQUE_OFFSET,
            ),
            DespawnOnExit(Phase::Day),
        ));
    }
}

/// A table, two posts and an awning in the trader's colour.
fn spawn_stall_placeholder(k: &mut Kit, stall: Entity, color: Color) {
    let wood = Color::srgb(0.42, 0.3, 0.2);
    k.part(
        stall,
        Cuboid::from_size(TABLE),
        wood,
        Transform::from_xyz(0.0, TABLE.y / 2.0, 0.0),
    );
    for x in [-0.82, 0.82] {
        k.part(
            stall,
            Cylinder::new(0.05, 2.35),
            wood,
            Transform::from_xyz(x, 1.17, -0.5),
        );
    }
    k.part(
        stall,
        Cuboid::new(1.95, 0.05, 1.15),
        color,
        Transform::from_xyz(0.0, 2.25, -0.05).with_rotation(Quat::from_rotation_x(0.25)),
    );
    // A paler fringe along the front of the awning.
    k.part(
        stall,
        Cuboid::new(1.95, 0.18, 0.03),
        color.mix(&Color::WHITE, 0.5),
        Transform::from_xyz(0.0, 2.05, 0.5),
    );
}

/// A simple figure for each trader until its model exists.
fn spawn_vendor_placeholder(k: &mut Kit, figure: Entity, id: &str) {
    let at = |x: f32, y: f32, z: f32| Transform::from_xyz(x, y, z);
    let eyes = |k: &mut Kit, y: f32, z: f32, glow: LinearRgba| {
        for x in [-0.045, 0.045] {
            k.glow(figure, 0.018, glow, Vec3::new(x, y, z));
        }
    };
    match id {
        // A friendly skeleton in a leather apron.
        "carpenter" => {
            let bone = Color::srgb(0.9, 0.88, 0.8);
            k.part(
                figure,
                Capsule3d::new(0.17, 0.75),
                Color::srgb(0.45, 0.3, 0.2),
                at(0.0, 0.62, 0.0),
            );
            k.part(figure, Sphere::new(0.16), bone, at(0.0, 1.32, 0.0));
            eyes(k, 1.34, 0.13, LinearRgba::rgb(0.3, 0.3, 0.3));
        }
        // A will-o'-the-wisp: a floating, glowing ball of light.
        "candlemaker" => {
            k.glow(
                figure,
                0.2,
                LinearRgba::rgb(1.2, 3.0, 3.2),
                Vec3::new(0.0, 1.3, 0.0),
            );
        }
        // A witch in green with a pointed hat.
        "herbalist" => {
            let robe = Color::srgb(0.2, 0.35, 0.25);
            k.part(
                figure,
                Cone {
                    radius: 0.3,
                    height: 1.1,
                },
                robe,
                at(0.0, 0.55, 0.0),
            );
            k.part(
                figure,
                Sphere::new(0.14),
                Color::srgb(0.6, 0.75, 0.55),
                at(0.0, 1.2, 0.0),
            );
            k.part(
                figure,
                Cone {
                    radius: 0.2,
                    height: 0.45,
                },
                Color::srgb(0.12, 0.12, 0.15),
                at(0.0, 1.5, 0.0),
            );
            eyes(k, 1.22, 0.12, LinearRgba::rgb(1.5, 1.2, 0.3));
        }
        // A pale vampire in a black cape with a red collar.
        "glazier" => {
            k.part(
                figure,
                Cone {
                    radius: 0.32,
                    height: 1.3,
                },
                Color::srgb(0.08, 0.07, 0.1),
                at(0.0, 0.65, 0.0),
            );
            k.part(
                figure,
                Cuboid::new(0.4, 0.2, 0.05),
                Color::srgb(0.6, 0.1, 0.15),
                at(0.0, 1.25, -0.06),
            );
            k.part(
                figure,
                Sphere::new(0.14),
                Color::srgb(0.85, 0.85, 0.9),
                at(0.0, 1.42, 0.0),
            );
            eyes(k, 1.44, 0.12, LinearRgba::rgb(2.0, 0.2, 0.2));
        }
        // A shaggy werewolf with pointed ears.
        "weaver" => {
            let fur = Color::srgb(0.4, 0.3, 0.22);
            k.part(figure, Capsule3d::new(0.22, 0.7), fur, at(0.0, 0.65, 0.0));
            k.part(figure, Sphere::new(0.17), fur, at(0.0, 1.35, 0.02));
            for x in [-0.09, 0.09] {
                k.part(
                    figure,
                    Cone {
                        radius: 0.05,
                        height: 0.14,
                    },
                    fur,
                    at(x, 1.52, 0.0),
                );
            }
            eyes(k, 1.38, 0.15, LinearRgba::rgb(2.0, 1.6, 0.3));
        }
        // A big raven on a perch beside the table.
        "raven" => {
            let black = Color::srgb(0.06, 0.06, 0.08);
            k.part(
                figure,
                Cylinder::new(0.03, 1.0),
                Color::srgb(0.35, 0.25, 0.17),
                at(0.0, 0.5, 0.0),
            );
            k.part(
                figure,
                Sphere::new(0.13),
                black,
                at(0.0, 1.12, 0.0).with_scale(Vec3::new(0.9, 1.0, 1.3)),
            );
            k.part(figure, Sphere::new(0.08), black, at(0.0, 1.28, 0.1));
            k.part(
                figure,
                Cone {
                    radius: 0.025,
                    height: 0.1,
                },
                Color::srgb(0.25, 0.25, 0.25),
                at(0.0, 1.27, 0.2)
                    .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            );
            eyes(k, 1.3, 0.16, LinearRgba::rgb(0.8, 0.8, 1.0));
        }
        // The Reaper: a tall black hood, glowing eyes and a scythe.
        "reaper" => {
            let cloak = Color::srgb(0.06, 0.05, 0.08);
            k.part(
                figure,
                Cone {
                    radius: 0.36,
                    height: 1.5,
                },
                cloak,
                at(0.0, 0.75, 0.0),
            );
            k.part(figure, Sphere::new(0.18), cloak, at(0.0, 1.55, 0.0));
            eyes(k, 1.56, 0.15, LinearRgba::rgb(0.6, 2.4, 2.0));
            let wood = Color::srgb(0.3, 0.22, 0.15);
            k.part(figure, Cylinder::new(0.025, 2.1), wood, at(0.38, 1.05, 0.0));
            k.part(
                figure,
                Cuboid::new(0.6, 0.06, 0.02),
                Color::srgb(0.7, 0.72, 0.75),
                at(0.15, 2.05, 0.0).with_rotation(Quat::from_rotation_z(-0.3)),
            );
        }
        _ => {
            k.part(
                figure,
                Capsule3d::new(0.2, 0.8),
                Color::srgb(0.5, 0.5, 0.55),
                at(0.0, 0.7, 0.0),
            );
        }
    }
}

impl MarketDay {
    pub fn evening_letter(&self, night: u32) -> Option<u32> {
        self.letter.filter(|_| self.night == night)
    }

    pub fn night_length_factor(&self, night: u32) -> f32 {
        if self.night == night && self.hourglass {
            HOURGLASS_BONUS
        } else {
            1.0
        }
    }

    /// Draw fresh stock only when entering a new market day.
    fn prepare(
        &mut self,
        data: &GameData,
        progress: &Progress,
        upgrades: &Upgrades,
        rng: &mut Rng,
    ) {
        let night = progress.calendar(data);
        if self.night == progress.night {
            // The full catalog appends seasonal traders after those in the demo.
            // A saved trading day keeps its offers and purchases in either build.
            let added = data.vendors.get(self.offers.len()..).unwrap_or_default();
            let removed = self.offers.get(data.vendors.len()..).unwrap_or_default();
            if added.iter().all(|v| !v.here(data, night)) && removed.iter().all(Vec::is_empty) {
                self.offers.resize_with(data.vendors.len(), Vec::new);
                return;
            }
        }
        *self = Self {
            night: progress.night,
            ..default()
        };
        // The Reaper's single pieces: different products every day.
        let mut pool: Vec<String> = data
            .products
            .iter()
            .filter(|p| p.unlock_night <= night)
            .map(|p| p.id.clone())
            .collect();
        let mut rare = Vec::new();
        let wanted = data
            .vendors
            .iter()
            .flat_map(|v| &v.wares)
            .map(|w| match w {
                Ware::RarePieces(n) => *n as usize,
                _ => 0,
            })
            .sum::<usize>();
        while rare.len() < wanted && !pool.is_empty() {
            rare.push(pool.swap_remove(rng.index(pool.len())));
        }
        let mut rare = rare.into_iter();
        for vendor in &data.vendors {
            if !vendor.here(data, night) {
                self.offers.push(Vec::new());
                continue;
            }
            let mut kinds = Vec::new();
            for ware in &vendor.wares {
                match ware {
                    Ware::Decor(id) => {
                        if data.decor(id).is_some_and(|d| d.unlock_night <= night) {
                            kinds.push(WareKind::Decor(id.clone()));
                        }
                    }
                    Ware::Exchange(e) => kinds.push(WareKind::Exchange(*e)),
                    Ware::Hourglass => kinds.push(WareKind::Hourglass),
                    Ware::RarePieces(n) => {
                        for _ in 0..*n {
                            if let Some(id) = rare.next() {
                                kinds.push(WareKind::Rare(id));
                            }
                        }
                    }
                    Ware::Single(id) => {
                        if data.product(id).is_some_and(|p| p.unlock_night <= night) {
                            kinds.push(WareKind::Single(id.clone()));
                        }
                    }
                    Ware::Upgrade(id) => {
                        if data.upgrade(id).is_some_and(|u| upgrades.offered(u, night)) {
                            kinds.push(WareKind::Upgrade(id.clone()));
                        }
                    }
                }
            }
            // Today's special offers: furniture or an improvement, a new pick each day.
            let mut offers: Vec<(WareKind, bool)> = kinds.into_iter().map(|k| (k, false)).collect();
            let mut candidates: Vec<usize> = offers
                .iter()
                .enumerate()
                .filter(|(_, (k, _))| matches!(k, WareKind::Decor(_) | WareKind::Upgrade(_)))
                .map(|(n, _)| n)
                .collect();
            // A market day of the free-play week brings more specials.
            let extra = data.event(night).map_or(0, |e| e.market_specials);
            for _ in 0..vendor.specials + extra {
                if candidates.is_empty() {
                    break;
                }
                let n = candidates.swap_remove(rng.index(candidates.len()));
                offers[n].1 = true;
            }
            self.offers.push(offers);
        }
    }
}

/// Opens the market for the day: the camera moves there and today's wares
/// go on the tables.
fn open_market(
    mut commands: Commands,
    data: Res<GameData>,
    progress: Res<Progress>,
    asset_server: Res<AssetServer>,
    stalls: Query<(Entity, &Stall)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut day: ResMut<MarketDay>,
    mut location: ResMut<Location>,
    mut rng: ResMut<Rng>,
    mut sky: ResMut<ClearColor>,
    upgrades: Res<Upgrades>,
) {
    *location = Location::Market;
    sky.0 = DAY_SKY;
    let night = progress.calendar(&data);
    day.prepare(&data, &progress, &upgrades, &mut rng);

    let mut k = Kit {
        commands: commands.reborrow(),
        asset_server: &asset_server,
        meshes: &mut meshes,
        materials: &mut materials,
    };
    basket::spawn(&mut k);
    for (stall, Stall(i)) in &stalls {
        let vendor = &data.vendors[*i];
        // A seasonal trader's stall stands only in its season.
        let here = vendor.here(&data, night);
        k.commands.entity(stall).insert(if here {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
        if !here {
            continue;
        }
        let kinds = &day.offers[*i];
        let n = kinds.len().max(1) as f32;
        let step = ((TABLE.x - 0.3) / n).min(0.5);
        // One fixed, invisible hover box per ware, as wide as its place on
        // the table: the ware itself lifts and grows when hovered, and must
        // not slip away from under the pointer or onto its neighbour.
        let hit_mesh = k
            .meshes
            .add(Cuboid::new(step * 0.94, HIT_HEIGHT, HIT_DEPTH));
        let hit_material = k.materials.add(StandardMaterial {
            base_color: Color::NONE,
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        });
        for (j, (kind, special)) in kinds.iter().enumerate() {
            let x = (j as f32 - (n - 1.0) / 2.0) * step;
            let base = Vec3::new(x, TABLE.y, 0.08);
            let item = k
                .commands
                .spawn((
                    WareItem {
                        vendor: *i,
                        kind: kind.clone(),
                        special: *special,
                    },
                    Transform::from_translation(base),
                    Visibility::default(),
                    ChildOf(stall),
                    DespawnOnExit(Phase::Day),
                ))
                .observe(on_ware_click)
                .observe(|over: On<PointerOver>, mut h: ResMut<HoveredWare>| {
                    h.0 = Some(over.event_target());
                })
                .observe(|out: On<PointerOut>, mut h: ResMut<HoveredWare>| {
                    if h.0 == Some(out.event_target()) {
                        h.0 = None;
                    }
                })
                .id();
            k.commands.spawn((
                Mesh3d(hit_mesh.clone()),
                MeshMaterial3d(hit_material.clone()),
                Transform::from_xyz(0.0, HIT_HEIGHT / 2.0 - 0.02, 0.0),
                NotShadowCaster,
                ChildOf(item),
            ));
            let visual = k.node(Some(item), Transform::default());
            k.commands.entity(visual).insert(WareVisual);
            spawn_ware_visual(&mut k, &data, visual, kind);
            if matches!(
                kind,
                WareKind::Rare(_) | WareKind::Single(_) | WareKind::Hourglass
            ) {
                let tag = model_or_box(
                    &mut k.commands,
                    k.asset_server,
                    k.meshes,
                    k.materials,
                    "models/market/sold_ticket.glb",
                    Vec3::new(0.30, 0.224, 0.16),
                    Color::srgb(0.78, 0.65, 0.44),
                );
                k.commands.entity(tag).insert((
                    SoldTicket,
                    ChildOf(item),
                    Visibility::Hidden,
                    Pickable::IGNORE,
                ));
            }
            if *special {
                spawn_special_tag(&mut k, item);
            }
        }
    }
}

/// A little brass tag over today's special offer. Model (optional):
/// `assets/models/market/special_tag.glb`, origin at the bottom centre.
const SPECIAL_TAG: &str = "models/market/special_tag.glb";
const SPECIAL_TAG_SIZE: Vec3 = Vec3::new(0.14, 0.1, 0.02);
const SPECIAL_TAG_HEIGHT: f32 = 0.55;

fn spawn_special_tag(k: &mut Kit, item: Entity) {
    let at = Transform::from_xyz(0.0, SPECIAL_TAG_HEIGHT, 0.0);
    if !k.model(item, SPECIAL_TAG, at) {
        let tag = model_or_box(
            &mut k.commands,
            k.asset_server,
            k.meshes,
            k.materials,
            SPECIAL_TAG,
            SPECIAL_TAG_SIZE,
            Color::srgb(0.85, 0.68, 0.3),
        );
        k.commands.entity(tag).insert((at, ChildOf(item)));
    }
}

/// What a ware looks like on the table.
fn spawn_ware_visual(k: &mut Kit, data: &GameData, item: Entity, kind: &WareKind) {
    match kind {
        WareKind::Decor(id) => {
            let Some(def) = data.decor(id) else { return };
            let size = Vec3::new(def.size.0, def.size.1, def.size.2);
            let scale = (SAMPLE_SIZE / size.max_element()).min(1.0);
            let sample = k.node(Some(item), Transform::from_scale(Vec3::splat(scale)));
            let path = format!("models/decor/{id}.glb");
            let visual = model_or_box(
                &mut k.commands,
                k.asset_server,
                k.meshes,
                k.materials,
                &path,
                size,
                rgb(def.color),
            );
            k.commands.entity(visual).insert(ChildOf(sample));
        }
        WareKind::Upgrade(id) => {
            // The improvement in miniature, or a rolled-up plan of the work.
            let Some(def) = data.upgrade(id) else { return };
            let (path, size, color) = match def.model {
                Some(m) => (
                    format!("models/shop/{id}.glb"),
                    Vec3::new(m.size.0, m.size.1, m.size.2),
                    rgb(m.color),
                ),
                None => (PLAN_MODEL.to_string(), PLAN_SIZE, PLAN_COLOR),
            };
            let scale = (SAMPLE_SIZE / size.max_element()).min(1.0);
            let sample = k.node(Some(item), Transform::from_scale(Vec3::splat(scale)));
            let visual = model_or_box(
                &mut k.commands,
                k.asset_server,
                k.meshes,
                k.materials,
                &path,
                size,
                color,
            );
            k.commands.entity(visual).insert(ChildOf(sample));
        }
        WareKind::Rare(id) | WareKind::Single(id) => {
            let visual = product_model(
                &mut k.commands,
                k.asset_server,
                k.meshes,
                k.materials,
                id,
                Vec3::ZERO,
            );
            k.commands.entity(visual).insert(ChildOf(item));
        }
        WareKind::Exchange(e) => {
            // The coin the raven takes, as a little picture.
            let coin = exchange_gives(*e).0;
            let path = format!("sprites/coins/{}.png", coin.id());
            let mesh = billboard_mesh(k.meshes, Vec2::splat(0.24));
            let material = billboard_material(
                k.materials,
                k.asset_server,
                &path,
                Color::srgb(0.8, 0.7, 0.5),
            );
            k.commands.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::from_xyz(0.0, 0.02, 0.0),
                crate::scene::Billboard,
                NotShadowCaster,
                ChildOf(item),
            ));
        }
        WareKind::Hourglass => {
            if !k.model(item, "models/market/hourglass.glb", Transform::default()) {
                let brass = Color::srgb(0.7, 0.55, 0.3);
                k.part(
                    item,
                    Cylinder::new(0.09, 0.02),
                    brass,
                    Transform::from_xyz(0.0, 0.01, 0.0),
                );
                k.part(
                    item,
                    Cylinder::new(0.09, 0.02),
                    brass,
                    Transform::from_xyz(0.0, 0.29, 0.0),
                );
                for (y, flip) in [(0.08, 0.0), (0.22, std::f32::consts::PI)] {
                    k.part(
                        item,
                        Cone {
                            radius: 0.07,
                            height: 0.12,
                        },
                        Color::srgba(0.8, 0.9, 1.0, 0.6),
                        Transform::from_xyz(0.0, y, 0.0).with_rotation(Quat::from_rotation_x(flip)),
                    );
                }
                k.glow(
                    item,
                    0.03,
                    LinearRgba::rgb(1.6, 1.3, 0.6),
                    Vec3::new(0.0, 0.06, 0.0),
                );
            }
        }
    }
}

fn close_market(
    mut location: ResMut<Location>,
    mut hovered: ResMut<HoveredWare>,
    mut tip: ResMut<HoverTip>,
    mut sky: ResMut<ClearColor>,
) {
    *location = Location::Shop;
    sky.0 = NIGHT_SKY;
    hovered.0 = None;
    HoverTip::hide(&mut tip);
}

fn on_signpost_click(
    click: On<PointerClick>,
    phase: Res<State<Phase>>,
    mut actions: MessageWriter<Action>,
) {
    if click.button == PointerButton::Primary && *phase.get() == Phase::Day {
        actions.write(Action::EndDay);
    }
}

/// (coin given, coins received, fee kept) of an exchange.
fn exchange_gives(e: Exchange) -> (Coin, Purse, u32) {
    let mut get = Purse::default();
    match e {
        Exchange::SpoonToCandles => {
            get.add(Coin::Candle, 4);
            get.add(Coin::Button, 5 - CHANGE_FEE);
            (Coin::Spoon, get, CHANGE_FEE)
        }
        Exchange::CandleToButtons => {
            get.add(Coin::Button, 5 - CHANGE_FEE);
            (Coin::Candle, get, CHANGE_FEE)
        }
        Exchange::CandlesToSpoon => {
            get.add(Coin::Spoon, 1);
            (Coin::Candle, get, 0)
        }
    }
}

/// How many of the given coin an exchange takes.
fn exchange_takes(e: Exchange) -> u32 {
    match e {
        Exchange::CandlesToSpoon => 5,
        _ => 1,
    }
}

/// Price of a ware (0 for exchanges, which cost a fee instead).
fn price(data: &GameData, kind: &WareKind, special: bool) -> u32 {
    let full = full_price(data, kind);
    if special {
        ((full as f32 * (1.0 - data.config.special_discount)).round() as u32).max(1)
    } else {
        full
    }
}

fn full_price(data: &GameData, kind: &WareKind) -> u32 {
    match kind {
        WareKind::Decor(id) => data.decor(id).map_or(0, |d| d.cost),
        WareKind::Rare(id) | WareKind::Single(id) => data.product(id).map_or(0, |p| p.price),
        WareKind::Hourglass => HOURGLASS_PRICE,
        WareKind::Exchange(_) => 0,
        WareKind::Upgrade(id) => data.upgrade(id).map_or(0, |u| u.cost),
    }
}

/// Why a ware cannot be bought right now, as a catalog key.
fn blocked(
    data: &GameData,
    day: &MarketDay,
    drawer: &Drawer,
    vendor: usize,
    kind: &WareKind,
    special: bool,
    reputation: i32,
) -> Option<&'static str> {
    if basket::sold_piece(day, vendor, kind) {
        return Some("market.tip_sold_out");
    }
    match kind {
        // The Reaper's rare pieces are for shops the spirits speak well of.
        WareKind::Rare(_) if reputation < data.config.reputation_perks.rare_from => {
            Some("market.tip_reputation")
        }
        WareKind::Exchange(e) => {
            let (coin, _, _) = exchange_gives(*e);
            (drawer.get(coin) < exchange_takes(*e)).then_some("market.no_coins")
        }
        _ => (drawer.total() < price(data, kind, special)).then_some("market.tip_poor"),
    }
}

/// Everything a purchase at the market touches.
#[derive(bevy::ecs::system::SystemParam)]
struct Shop<'w, 's> {
    commands: Commands<'w, 's>,
    upgrades: ResMut<'w, Upgrades>,
    items: Query<'w, 's, (Entity, &'static WareItem, &'static GlobalTransform)>,
    data: Res<'w, GameData>,
    lang: Res<'w, Lang>,
    drawer: ResMut<'w, Drawer>,
    storeroom: ResMut<'w, Storeroom>,
    stock: ResMut<'w, Stock>,
    accounts: ResMut<'w, Accounts>,
    day: ResMut<'w, MarketDay>,
    log: ResMut<'w, GameLog>,
    cues: MessageWriter<'w, Cue>,
    figures: Query<'w, 's, &'static mut VendorFigure>,
    trades: MessageWriter<'w, basket::Trade>,
    progress: Res<'w, Progress>,
}

impl Shop<'_, '_> {
    /// Buys one ware of a trader; false if it cannot be bought now.
    fn buy(&mut self, vendor: usize, kind: &WareKind, special: bool) -> bool {
        let data = &self.data;
        let lang = &self.lang;
        if blocked(
            data,
            &self.day,
            &self.drawer,
            vendor,
            kind,
            special,
            self.progress.reputation,
        )
        .is_some()
        {
            return false;
        }
        let name = lang.t(&format!("vendor.{}.name", data.vendors[vendor].id));
        let cost = price(data, kind, special);
        match kind {
            WareKind::Decor(id) => {
                if !self.drawer.pay(cost) {
                    return false;
                }
                self.accounts.furniture += cost;
                self.storeroom.give(id);
                self.day.bought.push(id.clone());
                self.log.push(lang.f(
                    "log.market_bought",
                    &[("item", &lang.decor(id)), ("vendor", &name)],
                ));
            }
            WareKind::Rare(id) | WareKind::Single(id) => {
                if !self.drawer.pay(cost) {
                    return false;
                }
                self.accounts.market += cost;
                self.stock.add(id, 1);
                self.day.rare_sold.push((vendor, id.clone()));
                self.log.push(lang.f(
                    "log.market_bought",
                    &[("item", &lang.product(id)), ("vendor", &name)],
                ));
            }
            WareKind::Hourglass => {
                if !self.drawer.pay(cost) {
                    return false;
                }
                self.accounts.market += cost;
                self.day.hourglass = true;
                self.log.push(lang.t("log.hourglass"));
            }
            WareKind::Exchange(e) => {
                let (coin, get, fee) = exchange_gives(*e);
                let mut give = Purse::default();
                give.add(coin, exchange_takes(*e));
                if !self.drawer.remove(&give) {
                    return false;
                }
                self.drawer.merge(&get);
                self.accounts.market += fee;
                self.log.push(lang.f("log.exchanged", &[("vendor", &name)]));
            }
            WareKind::Upgrade(id) => {
                if !self.drawer.pay(cost) {
                    return false;
                }
                self.accounts.upgrades += cost;
                self.upgrades.0.insert(id.clone());
                self.log.push(lang.f(
                    "log.market_bought",
                    &[
                        ("item", &lang.t(&format!("upgrade.{id}.name"))),
                        ("vendor", &name),
                    ],
                ));
                // Clearing Hedda's junk turns up one of her letters.
                if let Some(n) = data.upgrade(id).and_then(|u| u.letter) {
                    self.day.letter = Some(n);
                    self.log.push(lang.t("log.letter_found"));
                }
                // Done for good: it leaves the table and the list.
                for (entity, item, _) in &self.items {
                    if item.vendor == vendor && item.kind == *kind {
                        self.commands.entity(entity).despawn();
                    }
                }
                if let Some(offers) = self.day.offers.get_mut(vendor) {
                    offers.retain(|(k, _)| k != kind);
                }
            }
        }
        self.sold(vendor, kind, true);
        true
    }

    /// Furniture bought today can go back while the market is open.
    fn sell_back(&mut self, vendor: usize, kind: &WareKind, special: bool) -> bool {
        let WareKind::Decor(id) = kind else {
            return false;
        };
        let Some(pos) = self.day.bought.iter().position(|b| b == id) else {
            return false;
        };
        if !self.storeroom.take(id) {
            return false;
        }
        self.day.bought.remove(pos);
        let cost = price(&self.data, kind, special);
        self.drawer.merge(&Purse::greedy(cost));
        self.accounts.furniture = self.accounts.furniture.saturating_sub(cost);
        let name = self
            .lang
            .t(&format!("vendor.{}.name", self.data.vendors[vendor].id));
        self.log.push(self.lang.f(
            "log.market_sold_back",
            &[("item", &self.lang.decor(id)), ("vendor", &name)],
        ));
        self.sold(vendor, kind, false);
        true
    }

    /// The trader nods after a sale.
    fn sold(&mut self, vendor: usize, kind: &WareKind, bought: bool) {
        if basket::tangible(kind)
            && let Some((_, _, at)) = self
                .items
                .iter()
                .find(|(_, item, _)| item.vendor == vendor && &item.kind == kind)
        {
            let count = basket::count(&self.day);
            self.trades.write(basket::Trade {
                slot: if bought { count - 1 } else { count },
                sample: at.translation(),
                bought,
            });
        }
        self.cues.write(Cue::Paid);
        for mut figure in &mut self.figures {
            if figure.vendor == vendor {
                figure.nod = 0.8;
            }
        }
    }
}

fn on_ware_click(click: On<PointerClick>, mut shop: Shop) {
    let Ok((_, item, _)) = shop.items.get(click.event_target()) else {
        return;
    };
    let (vendor, kind, special) = (item.vendor, item.kind.clone(), item.special);
    match click.button {
        PointerButton::Primary => {
            shop.buy(vendor, &kind, special);
        }
        PointerButton::Secondary => {
            shop.sell_back(vendor, &kind, special);
        }
        PointerButton::Middle => {}
    }
}

/// The side panel lists a trader's wares; buying from the list works like
/// clicking the ware on the table.
fn buy_from_list(mut actions: MessageReader<Action>, mut shop: Shop) {
    for action in actions.read() {
        match action {
            Action::OpenStall(stall) => shop.day.stall = *stall,
            Action::MarketBuy(vendor, n) => {
                let Some((kind, special)) = shop
                    .day
                    .offers
                    .get(*vendor)
                    .and_then(|o| o.get(*n))
                    .cloned()
                else {
                    continue;
                };
                shop.buy(*vendor, &kind, special);
            }
            _ => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
/// The side panel's view of one trader: every ware of the day with its
/// price, today's special marked in words, and a button to buy it.
pub(crate) fn stall_panel(
    ui: &mut PanelBuilder,
    lang: &Lang,
    data: &GameData,
    day: &MarketDay,
    drawer: &Drawer,
    icons: &Icons,
    reputation: i32,
    vendor: usize,
) {
    let v = &data.vendors[vendor];
    ui.heading(&lang.t(&format!("vendor.{}.name", v.id)));
    ui.dim(&lang.t(&format!("vendor.{}.text", v.id)));
    let root = ui.root();
    let percent = (data.config.special_discount * 100.0).round() as u32;
    for (n, (kind, special)) in day.offers.get(vendor).into_iter().flatten().enumerate() {
        let (name, icon) = match kind {
            WareKind::Decor(id) => (lang.decor(id), icons.decor(id)),
            WareKind::Rare(id) | WareKind::Single(id) => (lang.product(id), icons.product(id)),
            WareKind::Hourglass => (lang.t("item.hourglass.name"), None),
            WareKind::Upgrade(id) => (lang.t(&format!("upgrade.{id}.name")), None),
            WareKind::Exchange(e) => {
                let key = exchange_key(*e);
                (
                    lang.t(&format!("exchange.{key}.name")),
                    icons.coin(exchange_gives(*e).0),
                )
            }
        };
        let cost = price(data, kind, *special);
        let label = match kind {
            WareKind::Exchange(_) => name,
            _ if *special => lang.f(
                "market.offer_special",
                &[("item", &name), ("price", &cost), ("percent", &percent)],
            ),
            _ => lang.f("market.offer", &[("item", &name), ("price", &cost)]),
        };
        let ok = blocked(data, day, drawer, vendor, kind, *special, reputation).is_none();
        ui.icon_button_in(root, icon, &label, Action::MarketBuy(vendor, n), false, ok);
    }
    ui.dim(&lang.t("market.list_hint"));
    ui.menu_option(
        root,
        &lang.t("market.all_traders"),
        Action::OpenStall(None),
        false,
    );
}

fn exchange_key(e: Exchange) -> &'static str {
    match e {
        Exchange::SpoonToCandles => "spoon_to_candles",
        Exchange::CandleToButtons => "candle_to_buttons",
        Exchange::CandlesToSpoon => "candles_to_spoon",
    }
}

fn ware_tooltip(
    hovered: Res<HoveredWare>,
    items: Query<&WareItem>,
    data: Res<GameData>,
    lang: Res<Lang>,
    icons: Res<Icons>,
    drawer: Res<Drawer>,
    storeroom: Res<Storeroom>,
    stock: Res<Stock>,
    day: Res<MarketDay>,
    mut tip: ResMut<HoverTip>,
    mut shown: Local<bool>,
    progress: Res<Progress>,
) {
    let Some(item) = hovered.0.and_then(|e| items.get(e).ok()) else {
        if *shown {
            HoverTip::hide(&mut tip);
            *shown = false;
        }
        return;
    };
    *shown = true;
    let mut t = HoverTip::default();
    let cost = price(&data, &item.kind, item.special);
    match &item.kind {
        WareKind::Decor(id) => {
            let Some(def) = data.decor(id) else { return };
            t.icon = icons.decor(id);
            t.title = lang.decor(id);
            t.line(
                TipKind::Text,
                lang.f("market.tip_price", &[("price", &cost)]),
            );
            t.line(
                TipKind::Text,
                lang.f(
                    "market.tip_atmosphere",
                    &[("effect", &def.effect.describe(&lang))],
                ),
            );
            if def.is_shelf() {
                t.line(
                    TipKind::Good,
                    lang.f("market.tip_slots", &[("count", &def.slots.len())]),
                );
            }
            t.line(
                TipKind::Text,
                lang.f("market.tip_owned", &[("count", &storeroom.count(id))]),
            );
            for set in data.sets.iter().filter(|s| s.decor.contains(id)) {
                t.line(
                    TipKind::Good,
                    lang.f(
                        "sets.part_of",
                        &[
                            ("name", &lang.t(&format!("set.{}.name", set.id))),
                            ("bonus", &crate::ui::set_bonus(&lang, set)),
                        ],
                    ),
                );
            }
        }
        WareKind::Rare(id) | WareKind::Single(id) => {
            t.icon = icons.product(id);
            t.title = lang.product(id);
            t.line(TipKind::Text, lang.t("market.rare_text"));
            t.line(
                TipKind::Text,
                lang.f("market.tip_price", &[("price", &cost)]),
            );
            t.line(
                TipKind::Text,
                lang.f("ferry.tip_stock", &[("count", &stock.count(id))]),
            );
        }
        WareKind::Hourglass => {
            t.title = lang.t("item.hourglass.name");
            t.line(TipKind::Text, lang.t("item.hourglass.text"));
            t.line(
                TipKind::Text,
                lang.f("market.tip_price", &[("price", &cost)]),
            );
        }
        WareKind::Upgrade(id) => {
            t.title = lang.t(&format!("upgrade.{id}.name"));
            t.line(TipKind::Text, lang.t(&format!("upgrade.{id}.text")));
            t.line(
                TipKind::Text,
                lang.f("market.tip_price", &[("price", &cost)]),
            );
            t.line(TipKind::Good, lang.t("market.tip_upgrade"));
        }
        WareKind::Exchange(e) => {
            let key = exchange_key(*e);
            t.icon = icons.coin(exchange_gives(*e).0);
            t.title = lang.t(&format!("exchange.{key}.name"));
            t.line(TipKind::Text, lang.t(&format!("exchange.{key}.text")));
        }
    }
    if item.special {
        let percent = (data.config.special_discount * 100.0).round() as u32;
        t.line(
            TipKind::Good,
            lang.f("market.tip_special", &[("percent", &percent)]),
        );
    }
    match blocked(
        &data,
        &day,
        &drawer,
        item.vendor,
        &item.kind,
        item.special,
        progress.reputation,
    ) {
        Some(key) => {
            let min = data.config.reputation_perks.rare_from;
            t.line(TipKind::Bad, lang.f(key, &[("min", &min)]));
        }
        None => {
            let key = if matches!(item.kind, WareKind::Decor(_)) {
                "market.tip_controls"
            } else {
                "market.tip_buy"
            };
            t.line(TipKind::Hint, lang.t(key));
        }
    }
    HoverTip::show(&mut tip, t);
}

/// A sold single piece leaves its sealed ticket and keeps its original tooltip hitbox.
fn mark_sold(
    day: Res<MarketDay>,
    items: Query<&WareItem>,
    mut displays: Query<
        (&ChildOf, &mut Visibility, Has<SoldTicket>),
        Or<(With<WareVisual>, With<SoldTicket>)>,
    >,
) {
    for (parent, mut visibility, ticket) in &mut displays {
        if let Ok(item) = items.get(parent.parent()) {
            let sold = basket::sold_piece(&day, item.vendor, &item.kind);
            visibility.set_if_neq(if sold == ticket {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            });
        }
    }
}

/// The hovered ware lifts a little.
fn animate_wares(
    time: Res<Time>,
    hovered: Res<HoveredWare>,
    settings: Res<Settings>,
    mut visuals: Query<(&ChildOf, &mut Transform), With<WareVisual>>,
) {
    let dt = time.delta_secs();
    for (parent, mut transform) in &mut visuals {
        let is_hovered = hovered.0 == Some(parent.parent());
        let lift = if is_hovered { 0.06 } else { 0.0 };
        let target = Vec3::Y * lift;
        let now = transform.translation;
        transform.translation = if settings.reduced_motion {
            target
        } else {
            now.lerp(target, 1.0 - (-dt * 10.0).exp())
        };
        let scale = if is_hovered { 1.12 } else { 1.0 };
        transform.scale = Vec3::splat(scale);
    }
}

/// Traders breathe and sway, nod after a sale; the wisp floats and the
/// raven bobs and looks around.
fn animate_vendors(
    time: Res<Time>,
    settings: Res<Settings>,
    mut figures: Query<(&mut VendorFigure, &mut Transform)>,
) {
    let t = time.elapsed_secs();
    let dt = time.delta_secs();
    for (mut f, mut transform) in &mut figures {
        if settings.reduced_motion {
            f.nod = 0.0;
            transform.set_if_neq(Transform::from_translation(f.kind.spot()));
            continue;
        }
        f.nod = (f.nod - dt).max(0.0);
        let nod = (f.nod * std::f32::consts::TAU * 1.5).sin() * f.nod.min(0.4) * 0.5;
        let p = f.phase;
        let base = f.kind.spot();
        match f.kind {
            FigureKind::Standing => {
                transform.translation = base;
                transform.rotation =
                    Quat::from_rotation_z(0.04 * (t * 0.9 + p).sin()) * Quat::from_rotation_x(nod);
                transform.scale = Vec3::new(1.0, 1.0 + 0.015 * (t * 1.5 + p).sin(), 1.0);
            }
            FigureKind::Wisp => {
                transform.translation = base + Vec3::Y * (0.12 * (t * 1.4 + p).sin() + nod * 0.3);
                transform.scale = Vec3::splat(1.0 + 0.08 * (t * 3.0 + p).sin());
            }
            FigureKind::Bird => {
                let hop = ((t * 0.7 + p).sin().max(0.0)).powi(12) * 0.08;
                transform.translation = base + Vec3::Y * hop;
                transform.rotation =
                    Quat::from_rotation_y(0.6 * (t * 0.5 + p).sin()) * Quat::from_rotation_x(nod);
            }
        }
    }
}

fn sway_bunting(
    time: Res<Time>,
    settings: Res<Settings>,
    mut pennants: Query<(&Pennant, &mut Transform)>,
) {
    let t = time.elapsed_secs();
    for (p, mut transform) in &mut pennants {
        transform.rotation = if settings.reduced_motion {
            Quat::IDENTITY
        } else {
            Quat::from_rotation_x(0.35 * (t * 2.1 + p.phase).sin())
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traders_can_sell_their_own_copy_of_the_same_product() {
        let data = GameData::load();
        let reaper = data.vendors.iter().position(|v| v.id == "reaper").unwrap();
        let frost = data.vendors.iter().position(|v| v.id == "frost").unwrap();
        let day = MarketDay {
            rare_sold: vec![(reaper, "snow_globe".into())],
            ..default()
        };
        let drawer = Drawer(Purse::greedy(100));
        let rare = WareKind::Rare("snow_globe".into());
        let single = WareKind::Single("snow_globe".into());
        assert_eq!(
            blocked(&data, &day, &drawer, reaper, &rare, false, 100),
            Some("market.tip_sold_out")
        );
        assert_eq!(
            blocked(&data, &day, &drawer, frost, &single, false, 100),
            None
        );
    }

    #[test]
    fn resuming_a_market_keeps_its_offers_receipts_and_hourglass() {
        let data = GameData::load();
        let mut progress = Progress::new(0);
        progress.night = 4;
        let upgrades = Upgrades::default();
        let mut day = MarketDay::default();
        day.prepare(&data, &progress, &upgrades, &mut Rng::new(17));
        day.bought.push("shelf".into());
        day.letter = Some(2);
        day.hourglass = true;
        let mut resumed: MarketDay = ron::from_str(&ron::to_string(&day).unwrap()).unwrap();
        let mut rng = Rng::new(29);
        resumed.prepare(&data, &progress, &upgrades, &mut rng);
        assert_eq!(resumed, day);
        assert_eq!(resumed.evening_letter(4), Some(2));
        assert_eq!(resumed.evening_letter(5), None);
        assert_eq!(resumed.night_length_factor(4), HOURGLASS_BONUS);
        progress.night += 1;
        assert_eq!(resumed.night_length_factor(5), 1.0);
        resumed.prepare(&data, &progress, &upgrades, &mut rng);
        assert!(resumed.bought.is_empty());
        assert!(resumed.letter.is_none());
        assert!(!resumed.hourglass);
    }

    #[test]
    fn a_market_day_survives_crossing_between_the_demo_and_full_catalogs() {
        let mut full = GameData::load();
        full.vendors = ron::from_str(include_str!("../assets/data/market.ron")).unwrap();
        let mut demo = GameData::load();
        let limit = demo.config.demo_nights;
        demo.vendors.retain(|v| v.from_night <= limit);
        let count = demo.vendors.len();
        assert!(count < full.vendors.len());
        assert!(
            demo.vendors
                .iter()
                .zip(&full.vendors)
                .all(|(d, f)| d.id == f.id)
        );

        let mut progress = Progress::new(0);
        progress.night = 4;
        let upgrades = Upgrades::default();
        let mut rng = Rng::new(29);
        let mut day = MarketDay::default();
        day.prepare(&demo, &progress, &upgrades, &mut rng);
        day.bought.push("mirror".into());
        day.letter = Some(3);
        day.hourglass = true;
        let reaper = demo.vendors.iter().position(|v| v.id == "reaper").unwrap();
        let sold = day.offers[reaper]
            .iter()
            .find_map(|(ware, _)| match ware {
                WareKind::Rare(id) => Some(id.clone()),
                _ => None,
            })
            .unwrap();
        day.rare_sold.push((reaper, sold));

        let mut imported: MarketDay = ron::from_str(&ron::to_string(&day).unwrap()).unwrap();
        imported.prepare(&full, &progress, &upgrades, &mut rng);
        assert_eq!(imported.offers.len(), full.vendors.len());
        assert!(imported.offers[count..].iter().all(Vec::is_empty));
        let mut expected = day.clone();
        expected.offers.resize_with(full.vendors.len(), Vec::new);
        assert_eq!(imported, expected);

        // Early saves from older builds also carry the unoccupied seasonal stalls.
        let mut older: MarketDay = ron::from_str(&ron::to_string(&imported).unwrap()).unwrap();
        older.prepare(&demo, &progress, &upgrades, &mut rng);
        assert_eq!(older, day);
    }

    #[test]
    fn exchanges_keep_the_value_minus_the_fee() {
        for e in [
            Exchange::SpoonToCandles,
            Exchange::CandleToButtons,
            Exchange::CandlesToSpoon,
        ] {
            let (coin, get, fee) = exchange_gives(e);
            let given = coin.value() * exchange_takes(e);
            assert_eq!(get.total() + fee, given, "{e:?}");
        }
    }

    #[test]
    fn every_market_ware_has_a_price_or_is_an_exchange() {
        let data = GameData::load();
        for v in &data.vendors {
            for w in &v.wares {
                if let Ware::Decor(id) = w {
                    assert!(
                        price(&data, &WareKind::Decor(id.clone()), false) > 0,
                        "{id} is free"
                    );
                }
            }
        }
    }
}
