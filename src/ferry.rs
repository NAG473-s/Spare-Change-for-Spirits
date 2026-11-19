//! The ferryman: every evening his boat glides up to the harbour quay behind
//! the shop, with one crate of every product he offers on deck.
//! Left-click a crate to buy it, right-click to give one back while he is
//! still here. Hovering a crate opens its lid and shows what it costs, what
//! it sells for and how long it keeps. At midnight he poles away again.

use std::collections::BTreeMap;

use bevy::light::NotShadowCaster;
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::accounts::Accounts;
use crate::data::{GameData, asset_exists};
use crate::day_cycle::{Phase, Progress};
use crate::economy::{Drawer, Purse};
use crate::fx::Cue;
use crate::harbor::{CART, HARBOR, RippleMesh, spawn_ripple};
use crate::i18n::Lang;
use crate::inventory::{CratesOrdered, Stock};
use crate::rng::Rng;
use crate::scene::world_label;
use crate::shop::{model_or_box, product_model};
use crate::ui::{GameLog, PanelBuilder, TEXT, TEXT_DIM, UiFont};

pub struct FerryPlugin;

impl Plugin for FerryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BoughtTonight>()
            .init_resource::<HoveredCrate>()
            .add_systems(OnEnter(Phase::Evening), spawn_boat)
            .add_systems(OnExit(Phase::Evening), cast_off)
            .add_systems(
                Update,
                (
                    sail_boat,
                    animate_ferryman,
                    make_wake,
                    fly_crates,
                    (animate_crates, update_crate_labels, update_tooltip)
                        .run_if(in_state(Phase::Evening)),
                ),
            );
    }
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
    /// Hinge of the lid (rotates about x).
    lid: Entity,
    /// The product shown in the crate.
    item: Entity,
    lid_angle: f32,
    lift: f32,
    /// Counts down after a sale or a return: the lid snaps shut and bounces.
    bump: f32,
}

/// A bought crate on its way from the boat into the shop.
#[derive(Component)]
struct FlyingCrate {
    from: Vec3,
    to: Vec3,
    age: f32,
}

/// A bought crate waiting on the hand cart (its place in the stack).
#[derive(Component)]
struct CartCrate(usize);

/// The "×2" / "new" label above a crate.
#[derive(Component)]
struct CrateLabel(Entity);

#[derive(Component)]
struct Tooltip;

/// Where the boat moors at the harbour quay (centre of the deck).
const MOORING: Vec3 = Vec3::new(HARBOR.x + 0.3, 0.0, 0.25);
const BOAT_SIZE: Vec3 = Vec3::new(7.4, 0.45, 1.7);
const DECK_Y: f32 = 0.05;
const CRATE_SIZE: Vec3 = Vec3::new(0.5, 0.3, 0.42);
const LID_THICKNESS: f32 = 0.05;
/// Lid angles (radians, 0 = shut): waiting, hovered.
const LID_AJAR: f32 = 1.2;
const LID_WIDE: f32 = 1.95;
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
    night: u32,
    id: &str,
) -> Option<&'static str> {
    let p = data.product(id)?;
    if crates.0 >= data.crates_tonight(night) {
        Some("ferry.tip_full")
    } else if drawer.total() < p.crate_cost {
        Some("ferry.tip_poor")
    } else {
        None
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
    fn crate_box(&mut self, parent: Entity, at: Transform) -> (Entity, Entity) {
        let root = self
            .commands
            .spawn((at, Visibility::default(), ChildOf(parent)))
            .id();
        let wood = Color::srgb(0.5, 0.36, 0.22);
        if self.model(root, "crate", Transform::default()).is_none() {
            self.part(
                root,
                Cuboid::from_size(CRATE_SIZE).into(),
                wood,
                Transform::from_xyz(0.0, CRATE_SIZE.y / 2.0, 0.0),
            );
            // Straw on top of the goods.
            self.part(
                root,
                Cuboid::new(CRATE_SIZE.x - 0.06, 0.02, CRATE_SIZE.z - 0.06).into(),
                Color::srgb(0.72, 0.6, 0.35),
                Transform::from_xyz(0.0, CRATE_SIZE.y + 0.005, 0.0),
            );
        }
        let hinge = self
            .commands
            .spawn((
                Transform::from_xyz(0.0, CRATE_SIZE.y, -CRATE_SIZE.z / 2.0),
                Visibility::default(),
                ChildOf(root),
            ))
            .id();
        if self
            .model(hinge, "crate_lid", Transform::default())
            .is_none()
        {
            self.part(
                hinge,
                Cuboid::new(CRATE_SIZE.x + 0.02, LID_THICKNESS, CRATE_SIZE.z + 0.02).into(),
                Color::srgb(0.44, 0.31, 0.19),
                Transform::from_xyz(0.0, LID_THICKNESS / 2.0, CRATE_SIZE.z / 2.0),
            );
        }
        (root, hinge)
    }
}

fn spawn_boat(
    commands: Commands,
    data: Res<GameData>,
    progress: Res<Progress>,
    font: Res<UiFont>,
    lang: Res<Lang>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let night = progress.night;
    let mut b = Builder {
        commands,
        asset_server: &asset_server,
        meshes: &mut meshes,
        materials: &mut materials,
    };
    let boat = b
        .commands
        .spawn((
            Boat {
                age: 0.0,
                leaving: None,
            },
            Transform::from_translation(MOORING + Vec3::X * ARRIVE_FROM),
            Visibility::default(),
        ))
        .id();

    // The hull: a model, or a long dark box with a raised bow and stern.
    if b.model(boat, "ferry_boat", Transform::default()).is_none() {
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
        intensity: 90_000.0,
        range: 5.0,
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
        let (item, lid) = b.crate_box(boat, at);
        let product = product_model(
            &mut b.commands,
            b.asset_server,
            b.meshes,
            b.materials,
            &p.id,
            Vec3::Y * CRATE_SIZE.y,
        );
        b.commands.entity(product).insert(ChildOf(item));
        b.commands
            .entity(item)
            .insert(Crate {
                id: p.id.clone(),
                lid,
                item: product,
                lid_angle: 0.0,
                lift: -0.25,
                bump: 0.0,
            })
            .observe(on_crate_click)
            .observe(on_crate_over)
            .observe(on_crate_out);
        let new = p.unlock_night == night && night > 1;
        b.commands.spawn((
            CrateLabel(item),
            world_label(
                &font,
                if new {
                    lang.t("ferry.new")
                } else {
                    String::new()
                },
                12.0,
                item,
                Vec3::Y * (CRATE_SIZE.y + 0.5),
            ),
            DespawnOnExit(Phase::Evening),
        ));
    }

    // Details of the hovered crate, next to the mouse pointer.
    b.commands.spawn((
        Tooltip,
        Node {
            position_type: PositionType::Absolute,
            width: px(TOOLTIP_WIDTH),
            padding: UiRect::all(px(10)),
            flex_direction: FlexDirection::Column,
            row_gap: px(4),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(6)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.05, 0.05, 0.09, 0.95)),
        BorderColor::all(Color::srgba(1.0, 0.85, 0.55, 0.25)),
        GlobalZIndex(150),
        Visibility::Hidden,
        Pickable::IGNORE,
        DespawnOnExit(Phase::Evening),
    ));
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
fn cast_off(
    mut boats: Query<&mut Boat>,
    mut bought: ResMut<BoughtTonight>,
    mut hovered: ResMut<HoveredCrate>,
) {
    for mut boat in &mut boats {
        boat.leaving.get_or_insert(0.0);
    }
    bought.0.clear();
    hovered.0 = None;
}

/// Glides in from the left, rocks at the landing stage, leaves to the right.
fn sail_boat(
    mut commands: Commands,
    time: Res<Time>,
    mut boats: Query<(Entity, &mut Boat, &mut Transform)>,
) {
    let t = time.elapsed_secs();
    let dt = time.delta_secs();
    for (entity, mut boat, mut transform) in &mut boats {
        boat.age += dt;
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
        transform.translation = MOORING + Vec3::new(x, 0.03 * (t * 1.3).sin(), 0.0);
        transform.rotation = Quat::from_rotation_z(0.012 * (t * 0.9).sin())
            * Quat::from_rotation_x(0.01 * (t * 1.1).sin());
    }
}

/// He punts while the boat moves, breathes and looks around when moored,
/// and nods after every sale. The bow lantern swings.
fn animate_ferryman(
    time: Res<Time>,
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
    time: Res<Time>,
    mesh: Option<Res<RippleMesh>>,
    mut rng: ResMut<Rng>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    boats: Query<(&Boat, &Transform)>,
) {
    let Some(mesh) = mesh else { return };
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

fn on_crate_click(
    click: On<PointerClick>,
    phase: Res<State<Phase>>,
    mut crates_q: Query<(&mut Crate, &GlobalTransform)>,
    mut men: Query<&mut Ferryman>,
    data: Res<GameData>,
    progress: Res<Progress>,
    lang: Res<Lang>,
    mut stock: ResMut<Stock>,
    mut drawer: ResMut<Drawer>,
    mut crates: ResMut<CratesOrdered>,
    mut bought: ResMut<BoughtTonight>,
    mut accounts: ResMut<Accounts>,
    mut log: ResMut<GameLog>,
    mut cues: MessageWriter<Cue>,
    cart: Query<(Entity, &CartCrate)>,
    mut commands: Commands,
) {
    if *phase.get() != Phase::Evening {
        return;
    }
    let Ok((mut item, global)) = crates_q.get_mut(click.event_target()) else {
        return;
    };
    let id = item.id.clone();
    let Some(p) = data.product(&id) else { return };
    match click.button {
        PointerButton::Primary => match blocked(&data, &crates, &drawer, progress.night, &id) {
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
                item.bump = 0.5;
                for mut man in &mut men {
                    man.nod = 0.8;
                }
                cues.write(Cue::Paid);
                log.push(lang.f(
                    "log.ferry_brings",
                    &[("count", &p.crate_size), ("item", &lang.product(&id))],
                ));
                // A full crate is lifted onto the hand cart.
                commands.spawn((
                    FlyingCrate {
                        from: global.translation(),
                        to: HARBOR + CART + Vec3::Y * 0.5,
                        age: 0.0,
                    },
                    Transform::from_translation(global.translation()),
                    Visibility::default(),
                    DespawnOnExit(Phase::Evening),
                ));
            }
            None => {}
        },
        PointerButton::Secondary => {
            let Some(n) = bought.0.get_mut(&id) else {
                return;
            };
            // Only while the crate is still full (nothing of it sold or faded).
            if !stock.remove_new(&id, p.crate_size) {
                return;
            }
            *n -= 1;
            if *n == 0 {
                bought.0.remove(&id);
            }
            crates.0 = crates.0.saturating_sub(1);
            accounts.ferryman = accounts.ferryman.saturating_sub(p.crate_cost);
            drawer.merge(&Purse::greedy(p.crate_cost));
            item.bump = 0.5;
            // The top crate leaves the hand cart again.
            if let Some((top, _)) = cart.iter().max_by_key(|(_, c)| c.0) {
                commands.entity(top).despawn();
            }
            log.push(lang.f("log.ferry_takes_back", &[("item", &lang.product(&id))]));
        }
        PointerButton::Middle => {}
    }
}

/// Lids open when a crate can be bought, open wide while it is hovered and
/// snap shut for a moment after a sale; the product rises out of the crate.
fn animate_crates(
    time: Res<Time>,
    hovered: Res<HoveredCrate>,
    data: Res<GameData>,
    progress: Res<Progress>,
    crates: Res<CratesOrdered>,
    drawer: Res<Drawer>,
    mut boxes: Query<(Entity, &mut Crate, &mut Transform)>,
    mut parts: Query<&mut Transform, Without<Crate>>,
) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    for (entity, mut item, mut transform) in &mut boxes {
        item.bump = (item.bump - dt).max(0.0);
        let available = blocked(&data, &crates, &drawer, progress.night, &item.id).is_none();
        let is_hovered = hovered.0 == Some(entity);
        let (lid, lift) = if item.bump > 0.25 || !available {
            (0.0, -0.25)
        } else if is_hovered {
            (LID_WIDE, 0.1 + 0.02 * (t * 5.0).sin())
        } else {
            (LID_AJAR, 0.0)
        };
        // Snappy when shutting, a little slower when opening.
        let rate = if lid < item.lid_angle { 18.0 } else { 7.0 };
        item.lid_angle += (lid - item.lid_angle) * (dt * rate).min(1.0);
        item.lift += (lift - item.lift) * (dt * 9.0).min(1.0);
        if let Ok(mut hinge) = parts.get_mut(item.lid) {
            hinge.rotation = Quat::from_rotation_x(-item.lid_angle);
        }
        if let Ok(mut product) = parts.get_mut(item.item) {
            product.translation.y = CRATE_SIZE.y + item.lift;
        }
        // A hop when hovered, a bounce after a sale.
        let bounce = (item.bump * 12.0).sin().abs() * item.bump * 0.12;
        let hop = if is_hovered { 0.04 } else { 0.0 };
        transform.translation.y +=
            (DECK_Y + hop + bounce - transform.translation.y) * (dt * 12.0).min(1.0);
    }
}

/// A bought crate flies in an arc onto the hand cart and stays there.
fn fly_crates(
    mut commands: Commands,
    time: Res<Time>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut flying: Query<(Entity, &mut FlyingCrate, &mut Transform, Option<&Children>)>,
    cart: Query<&CartCrate>,
) {
    let mut stacked = cart.iter().map(|c| c.0 + 1).max().unwrap_or(0);
    let dt = time.delta_secs();
    for (entity, mut flight, mut transform, children) in &mut flying {
        if children.is_none() {
            // First frame: give it a body (a shut crate).
            let mut b = Builder {
                commands: commands.reborrow(),
                asset_server: &asset_server,
                meshes: &mut meshes,
                materials: &mut materials,
            };
            b.crate_box(entity, Transform::default());
        }
        flight.age += dt;
        let k = (flight.age / FLIGHT_SECS).min(1.0);
        let arc = 4.0 * k * (1.0 - k) * 1.2;
        transform.translation = flight.from.lerp(flight.to, k) + Vec3::Y * arc;
        transform.rotation = Quat::from_rotation_y(k * 2.0) * Quat::from_rotation_z(k * 0.6);
        transform.scale = Vec3::splat(1.0 - 0.15 * k);
        if k >= 1.0 {
            // Two by two on the cart, then a second layer.
            let slot = stacked;
            stacked += 1;
            let spot = Vec3::new(
                if slot % 2 == 0 { -0.27 } else { 0.27 },
                0.46 + (slot / 4) as f32 * CRATE_SIZE.y * 0.85,
                if slot % 4 < 2 { -0.18 } else { 0.18 },
            );
            *transform =
                Transform::from_translation(HARBOR + CART + spot).with_scale(Vec3::splat(0.85));
            commands
                .entity(entity)
                .remove::<FlyingCrate>()
                .insert((CartCrate(slot), DespawnOnExit(Phase::Evening)));
        }
    }
}

/// "×2" above crates bought tonight, "new" above new products.
fn update_crate_labels(
    bought: Res<BoughtTonight>,
    lang: Res<Lang>,
    data: Res<GameData>,
    progress: Res<Progress>,
    crates: Query<&Crate>,
    mut labels: Query<(&CrateLabel, &mut Text)>,
) {
    if !bought.is_changed() && !lang.is_changed() {
        return;
    }
    for (label, mut text) in &mut labels {
        let Ok(item) = crates.get(label.0) else {
            continue;
        };
        let n = bought.0.get(&item.id).copied().unwrap_or(0);
        let new = data
            .product(&item.id)
            .is_some_and(|p| p.unlock_night == progress.night && progress.night > 1);
        text.0 = if n > 0 {
            format!("×{n}")
        } else if new {
            lang.t("ferry.new")
        } else {
            String::new()
        };
    }
}

fn update_tooltip(
    mut commands: Commands,
    hovered: Res<HoveredCrate>,
    window: Single<&Window, With<PrimaryWindow>>,
    font: Res<UiFont>,
    lang: Res<Lang>,
    data: Res<GameData>,
    progress: Res<Progress>,
    stock: Res<Stock>,
    drawer: Res<Drawer>,
    crates: Res<CratesOrdered>,
    bought: Res<BoughtTonight>,
    crate_q: Query<&Crate>,
    mut tooltip: Single<(Entity, &mut Node, &mut Visibility), With<Tooltip>>,
    mut shown: Local<Option<Entity>>,
) {
    let (entity, node, visibility) = &mut *tooltip;
    let item = hovered
        .0
        .and_then(|e| crate_q.get(e).ok().map(|c| (e, c.id.clone())));
    let (Some((hovered_entity, id)), Some(cursor)) = (item, window.cursor_position()) else {
        **visibility = Visibility::Hidden;
        *shown = None;
        return;
    };
    **visibility = Visibility::Inherited;
    // Above and next to the pointer, kept inside the window.
    let x = (cursor.x + 18.0).min(window.width() - TOOLTIP_WIDTH - 8.0);
    let y = (cursor.y - 190.0).max(8.0);
    node.left = px(x);
    node.top = px(y);

    let changed = stock.is_changed()
        || drawer.is_changed()
        || crates.is_changed()
        || bought.is_changed()
        || lang.is_changed();
    if *shown == Some(hovered_entity) && !changed {
        return;
    }
    *shown = Some(hovered_entity);
    let Some(p) = data.product(&id) else { return };
    let night = progress.night;
    let mut ui = PanelBuilder::new(&mut commands, *entity, &font);
    ui.add_text(*entity, &lang.product(&id), true, 15.0, TEXT);
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
        *entity,
        &lang.f("ferry.tip_profit", &[("profit", &signed(profit))]),
        false,
        14.0,
        if profit >= 0 { GREEN } else { RED },
    );
    ui.text(&lang.f("ferry.tip_lasts", &[("nights", &p.fade_after)]));
    ui.text(&lang.f("ferry.tip_stock", &[("count", &stock.count(&id))]));
    if let Some(n) = bought.0.get(&id) {
        ui.text(&lang.f("ferry.tip_bought", &[("count", n)]));
    }
    match blocked(&data, &crates, &drawer, night, &id) {
        Some(key) => ui.add_text(*entity, &lang.t(key), false, 13.0, RED),
        None => ui.add_text(
            *entity,
            &lang.t("ferry.tip_controls"),
            false,
            12.0,
            TEXT_DIM,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crates_are_blocked_by_boat_size_and_budget() {
        let data = GameData::load();
        let p = &data.products[0];
        let max = data.crates_tonight(1);
        let rich = Drawer(Purse::greedy(p.crate_cost * 10));
        assert_eq!(blocked(&data, &CratesOrdered(0), &rich, 1, &p.id), None);
        assert_eq!(
            blocked(&data, &CratesOrdered(max), &rich, 1, &p.id),
            Some("ferry.tip_full")
        );
        let poor = Drawer(Purse::greedy(p.crate_cost - 1));
        assert_eq!(
            blocked(&data, &CratesOrdered(0), &poor, 1, &p.id),
            Some("ferry.tip_poor")
        );
    }

    #[test]
    fn signed_numbers() {
        assert_eq!(signed(5), "+5");
        assert_eq!(signed(-3), "−3");
        assert_eq!(signed(0), "0");
    }
}
