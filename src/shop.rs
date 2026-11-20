//! The shop: a floor grid with shelves and decor, plus door and counter, in
//! the 3D diorama. Every piece is a model from `assets/models/` if it exists,
//! otherwise a coloured placeholder box of the size given in `decor.ron`.
//!
//! In the evening the player picks a tool in the side panel and clicks a cell.

use std::collections::BTreeMap;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use bevy::light::NotShadowCaster;

use bevy::picking::pointer::PointerButton;

use crate::data::{Atmosphere, DecorDef, GameData, asset_exists, rgb};
use crate::day_cycle::{Phase, Progress};
use crate::ferry::sale_price;
use crate::fx::Flicker;
use crate::ghosts::{Ghost, GhostState, Queue};
use crate::harbor::{ShopDoor, on_shop_door_click};
use crate::i18n::Lang;
use crate::inventory::Stock;
use crate::register::Register;
use crate::scene::{Billboard, billboard_material, billboard_mesh, floor, world_label};
use crate::tooltip::{HoverTip, TipKind};
use crate::ui::{Action, GameLog, Icons, UiFont};
use crate::upgrades::Upgrades;

/// One grid cell is one metre.
pub const CELL: f32 = 1.0;
/// Centre of the floor grid (x, z).
const GRID_CENTER: Vec2 = Vec2::new(0.0, 0.2);
// Countertop height, below the till. The loose props rest on this surface.
const COUNTER_SIZE: Vec3 = Vec3::new(4.5, 0.742, 0.8);
const DOOR_SIZE: Vec3 = Vec3::new(1.1, 2.3, 0.2);

pub struct ShopPlugin;

impl Plugin for ShopPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Tool>()
            .init_resource::<Storeroom>()
            .init_resource::<HoveredShelfItem>()
            .init_resource::<Dragging>()
            .add_systems(Startup, spawn_shop)
            .add_systems(OnExit(Phase::Evening), |mut t: ResMut<Tool>| {
                *t = Tool::None
            })
            .add_systems(
                Update,
                (
                    select_tool.run_if(in_state(Phase::Evening)),
                    follow_drag,
                    clear_sold_out.run_if(resource_changed::<Stock>),
                    sync_cell_visuals.run_if(
                        resource_changed::<ShopLayout>
                            .or_else(resource_changed::<Stock>)
                            .or_else(resource_changed::<UiFont>),
                    ),
                    translate_counter.run_if(resource_changed::<UiFont>),
                    shelf_tooltip,
                ),
            )
            // A rebuilt shelf loses its hovered item.
            .add_systems(
                PostUpdate,
                (|mut h: ResMut<HoveredShelfItem>, items: Query<(), With<ShelfItem>>| {
                    if h.0.is_some_and(|e| items.get(e).is_err()) {
                        h.0 = None;
                    }
                })
                .run_if(resource_changed::<ShopLayout>),
            );
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct CellContent {
    pub decor: Option<String>,
    /// Shelves only: the products displayed there, one per slot.
    #[serde(default)]
    pub products: Vec<String>,
}

#[derive(Resource)]
pub struct ShopLayout {
    pub cols: usize,
    pub rows: usize,
    pub cells: Vec<CellContent>,
    /// Tonight's mishaps that are not fixed yet (`incidents.rs`).
    pub mishaps: Atmosphere,
}

impl ShopLayout {
    pub fn new(cols: usize, rows: usize) -> Self {
        let mut layout = Self {
            cols,
            rows,
            cells: vec![CellContent::default(); cols * rows],
            mishaps: Atmosphere::default(),
        };
        // Starting furniture: two shelves and a candle.
        layout.cells[cols + 2].decor = Some("shelf".into());
        layout.cells[cols + 4].decor = Some("shelf".into());
        layout.cells[2 * cols + 3].decor = Some("candle".into());
        layout
    }

    /// Floor position (x, z) of a cell. Row 0 is at the front, near the camera.
    pub fn cell_pos(&self, idx: usize) -> Vec2 {
        let col = (idx % self.cols) as f32;
        let row = (idx / self.cols) as f32;
        let first = GRID_CENTER
            + Vec2::new(
                -(self.cols as f32 - 1.0) * CELL / 2.0,
                (self.rows as f32 - 1.0) * CELL / 2.0,
            );
        first + Vec2::new(col * CELL, -row * CELL)
    }

    /// Where ghosts appear: in front of the door in the left wall.
    pub fn door_pos(&self) -> Vec2 {
        Vec2::new(
            GRID_CENTER.x - self.cols as f32 * CELL / 2.0 - 1.0,
            GRID_CENTER.y,
        )
    }

    /// Centre of the counter along the right side.
    pub fn counter_pos(&self) -> Vec2 {
        Vec2::new(
            GRID_CENTER.x + self.cols as f32 * CELL / 2.0 + 1.3,
            GRID_CENTER.y,
        )
    }

    /// Spots where ghosts queue: the first right at the counter, the others
    /// in a line along the front of the shop, so the camera sees them side by
    /// side. A long line moves closer together.
    pub fn queue_pos(&self, place: usize, len: usize) -> Vec2 {
        let counter = self.counter_pos();
        if place == 0 {
            return counter + Vec2::new(-0.95, 0.9);
        }
        let spacing = (8.0 / len.max(1) as f32).min(0.95);
        let front = GRID_CENTER.y + self.rows as f32 * CELL / 2.0 + 0.4;
        Vec2::new(counter.x - 0.95 - place as f32 * spacing, front)
    }

    pub fn shelf_cells(&self, data: &GameData) -> Vec<usize> {
        (0..self.cells.len())
            .filter(|i| {
                self.cells[*i]
                    .decor
                    .as_deref()
                    .and_then(|d| data.decor(d))
                    .is_some_and(|d| d.is_shelf())
            })
            .collect()
    }

    /// All products currently on a shelf (without duplicates).
    pub fn displayed_products(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for p in self.cells.iter().flat_map(|c| &c.products) {
            if !out.contains(p) {
                out.push(p.clone());
            }
        }
        out
    }

    /// Atmosphere tonight: base, every decor item and the night's event.
    pub fn atmosphere(&self, data: &GameData, night: u32) -> Atmosphere {
        let mut a = data.config.base_atmosphere;
        if let Some(e) = data.event(night) {
            a.add(&e.atmosphere);
        }
        a.add(&self.mishaps);
        for c in &self.cells {
            if let Some(d) = c.decor.as_deref().and_then(|d| data.decor(d)) {
                a.add(&d.effect);
            }
        }
        a
    }
}

/// Furniture the player owns but has not placed: bought at the market or
/// left by Edwin as a keepsake (decor id → count).
#[derive(Resource, Default)]
pub struct Storeroom(pub BTreeMap<String, u32>);

impl Storeroom {
    pub fn count(&self, id: &str) -> u32 {
        self.0.get(id).copied().unwrap_or(0)
    }

    pub fn give(&mut self, id: &str) {
        *self.0.entry(id.to_string()).or_default() += 1;
    }

    /// Takes one out of the box; `false` if there is none.
    pub fn take(&mut self, id: &str) -> bool {
        match self.0.get_mut(id) {
            Some(n) if *n > 0 => {
                *n -= 1;
                true
            }
            _ => false,
        }
    }
}

/// Clears a cell. The furniture that stood there goes back into the
/// storeroom; its products stay in stock.
fn clear_cell(layout: &mut ShopLayout, idx: usize, storeroom: &mut Storeroom) {
    if let Some(d) = &layout.cells[idx].decor {
        storeroom.give(d);
    }
    layout.cells[idx] = CellContent::default();
}

/// Current tool in the evening.
#[derive(Resource, Default, Clone, PartialEq, Debug)]
pub enum Tool {
    #[default]
    None,
    Place(String),
    Stock(String),
    Remove,
}

#[derive(Component)]
pub struct GridCell(pub usize);

/// A piece of furniture being dragged to another cell (evening, no tool
/// picked): where it comes from and the cell under the pointer.
#[derive(Resource, Default)]
struct Dragging {
    from: Option<usize>,
    over: Option<usize>,
}

/// Marks the target cell while dragging.
#[derive(Component)]
struct DragMarker;

/// Where a dragged piece's parts sat before the drag.
#[derive(Component)]
struct DragBase(Vec3);

/// How high a dragged piece floats above the floor.
const DRAG_LIFT: f32 = 0.15;
const DRAG_OK: Color = Color::srgba(0.45, 0.85, 0.5, 0.35);
const DRAG_BLOCKED: Color = Color::srgba(0.95, 0.4, 0.35, 0.35);

/// Grab a piece of furniture: it lifts and follows the pointer cell by cell.
#[allow(clippy::too_many_arguments)]
fn on_drag_start(
    drag: On<PointerDragStart>,
    mut commands: Commands,
    phase: Res<State<Phase>>,
    tool: Res<Tool>,
    layout: Res<ShopLayout>,
    cells: Query<&GridCell>,
    children: Query<&Children>,
    transforms: Query<&Transform>,
    mut dragging: ResMut<Dragging>,
) {
    if *phase.get() != Phase::Evening
        || *tool != Tool::None
        || drag.button != PointerButton::Primary
    {
        return;
    }
    let Ok(GridCell(idx)) = cells.get(drag.event_target()) else {
        return;
    };
    if layout.cells[*idx].decor.is_none() {
        return;
    }
    dragging.from = Some(*idx);
    dragging.over = Some(*idx);
    // The dragged parts must not catch the pointer, or the cell under them
    // would never be found.
    if let Ok(parts) = children.get(drag.event_target()) {
        for part in parts.iter() {
            if let Ok(t) = transforms.get(part) {
                commands.entity(part).insert(DragBase(t.translation));
            }
        }
    }
    for part in children.iter_descendants(drag.event_target()) {
        commands.entity(part).insert(Pickable::IGNORE);
    }
}

fn on_drag_over(
    over: On<PointerDragOver>,
    cells: Query<&GridCell>,
    mut dragging: ResMut<Dragging>,
) {
    if dragging.from.is_some()
        && let Ok(GridCell(idx)) = cells.get(over.event_target())
    {
        dragging.over = Some(*idx);
    }
}

/// Let go: move the piece (with its wares) to the cell, or swap it with the
/// piece standing there. Old junk blocks the cell.
#[allow(clippy::too_many_arguments)]
fn on_drag_end(
    _end: On<PointerDragEnd>,
    mut commands: Commands,
    data: Res<GameData>,
    upgrades: Res<Upgrades>,
    lang: Res<Lang>,
    markers: Query<Entity, With<DragMarker>>,
    mut layout: ResMut<ShopLayout>,
    mut dragging: ResMut<Dragging>,
    mut log: ResMut<GameLog>,
) {
    let Some(from) = dragging.from.take() else {
        return;
    };
    let over = dragging.over.take();
    for marker in &markers {
        commands.entity(marker).despawn();
    }
    match over {
        Some(to) if to != from && upgrades.usable(&data, &layout, to) => {
            layout.cells.swap(from, to);
        }
        Some(to) if to != from => {
            log.push(lang.t("log.cell_blocked"));
            layout.set_changed();
        }
        // Back where it was: rebuild, so it sits down again.
        _ => layout.set_changed(),
    }
}

/// The dragged piece hovers over the cell under the pointer, which is
/// marked green (free or swappable) or red (old junk).
#[allow(clippy::too_many_arguments)]
fn follow_drag(
    mut commands: Commands,
    dragging: Res<Dragging>,
    layout: Res<ShopLayout>,
    data: Res<GameData>,
    upgrades: Res<Upgrades>,
    mut parts: Query<(&DragBase, &mut Transform), Without<DragMarker>>,
    mut markers: Query<(&mut Transform, &MeshMaterial3d<StandardMaterial>), With<DragMarker>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let (Some(from), Some(over)) = (dragging.from, dragging.over) else {
        return;
    };
    let shift = layout.cell_pos(over) - layout.cell_pos(from);
    for (base, mut transform) in &mut parts {
        transform.translation = base.0 + Vec3::new(shift.x, DRAG_LIFT, shift.y);
    }
    let color = if upgrades.usable(&data, &layout, over) {
        DRAG_OK
    } else {
        DRAG_BLOCKED
    };
    let at = floor(layout.cell_pos(over), 0.02);
    match markers.single_mut() {
        Ok((mut transform, material)) => {
            transform.translation = at;
            if let Some(mut m) = materials.get_mut(&material.0) {
                m.base_color = color;
            }
        }
        Err(_) => {
            commands.spawn((
                DragMarker,
                Mesh3d(meshes.add(Plane3d::default().mesh().size(CELL * 0.95, CELL * 0.95))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: color,
                    alpha_mode: AlphaMode::Blend,
                    unlit: true,
                    ..default()
                })),
                Transform::from_translation(at),
                Pickable::IGNORE,
                NotShadowCaster,
            ));
        }
    }
}

/// Model if `assets/<path>` exists, otherwise a placeholder box; either way
/// standing on its origin.
pub(crate) fn model_or_box(
    commands: &mut Commands,
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    path: &str,
    size: Vec3,
    color: Color,
) -> Entity {
    if asset_exists(path) {
        commands
            .spawn((
                WorldAssetRoot(
                    asset_server.load(GltfAssetLabel::Scene(0).from_asset(path.to_string())),
                ),
                Transform::default(),
            ))
            .id()
    } else {
        commands
            .spawn((
                Mesh3d(meshes.add(Cuboid::from_size(size))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: color,
                    perceptual_roughness: 0.8,
                    ..default()
                })),
                Transform::from_xyz(0.0, size.y / 2.0, 0.0),
            ))
            .id()
    }
}

fn spawn_shop(
    mut commands: Commands,
    lang: Res<Lang>,
    data: Res<GameData>,
    font: Res<UiFont>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let layout = ShopLayout::new(data.config.grid_cols, data.config.grid_rows);

    // Floor tiles with a light checker tint so the grid stays readable.
    let floor_texture =
        asset_exists("sprites/shop/floor.png").then(|| asset_server.load("sprites/shop/floor.png"));
    let tile_material = |materials: &mut Assets<StandardMaterial>, tint: f32| {
        materials.add(StandardMaterial {
            base_color: if floor_texture.is_some() {
                Color::srgb(tint, tint, tint)
            } else {
                Color::srgb(0.3 * tint, 0.22 * tint, 0.17 * tint)
            },
            base_color_texture: floor_texture.clone(),
            perceptual_roughness: 0.85,
            ..default()
        })
    };
    let light_tile = tile_material(&mut materials, 1.0);
    let dark_tile = tile_material(&mut materials, 0.82);
    let tile_mesh = meshes.add(Plane3d::default().mesh().size(CELL * 0.97, CELL * 0.97));
    for idx in 0..layout.cells.len() {
        let even = (idx % layout.cols + idx / layout.cols).is_multiple_of(2);
        commands
            .spawn((
                GridCell(idx),
                Mesh3d(tile_mesh.clone()),
                MeshMaterial3d(if even {
                    light_tile.clone()
                } else {
                    dark_tile.clone()
                }),
                Transform::from_translation(floor(layout.cell_pos(idx), 0.0)),
                Pickable::default(),
            ))
            .observe(on_cell_click)
            .observe(on_drag_start)
            .observe(on_drag_over)
            .observe(on_drag_end);
    }

    // The door stands in the left wall, its front facing into the shop (+x).
    let door = model_or_box(
        &mut commands,
        &asset_server,
        &mut meshes,
        &mut materials,
        "models/shop/door.glb",
        DOOR_SIZE,
        Color::srgb(0.3, 0.2, 0.13),
    );
    commands
        .entity(door)
        .insert(ShopDoor)
        .observe(on_shop_door_click);
    let door_spot = Vec2::new(crate::scene::ROOM_MIN_X + 0.1, layout.door_pos().y);
    commands.entity(door).insert(
        Transform::from_translation(floor(door_spot, 0.0))
            .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)),
    );
    if !asset_exists("models/shop/door.glb") {
        commands.entity(door).insert(
            Transform::from_translation(floor(door_spot, DOOR_SIZE.y / 2.0))
                .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)),
        );
    }

    // The counter runs along the right side, customer side facing left (-x).
    let counter = model_or_box(
        &mut commands,
        &asset_server,
        &mut meshes,
        &mut materials,
        "models/shop/counter.glb",
        COUNTER_SIZE,
        Color::srgb(0.38, 0.26, 0.17),
    );
    let y = if asset_exists("models/shop/counter.glb") {
        0.0
    } else {
        COUNTER_SIZE.y / 2.0
    };
    commands.entity(counter).insert((
        CounterModel,
        Transform::from_translation(floor(layout.counter_pos(), y))
            .with_rotation(Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2)),
    ));
    commands.spawn((
        CounterLabel,
        world_label(
            &font,
            lang.t("shop.counter"),
            12.0,
            counter,
            Vec3::new(0.0, 1.3 - y, 0.0),
        ),
    ));

    commands.insert_resource(layout);
}

#[derive(Component)]
struct CounterModel;

#[derive(Component)]
struct CounterLabel;

/// The counter label follows the language (and its font).
fn translate_counter(
    lang: Res<Lang>,
    font: Res<UiFont>,
    mut label: Single<(&mut Text, &mut TextFont), With<CounterLabel>>,
) {
    label.0.0 = lang.t("shop.counter");
    *label.1 = small_font(&font, 12.0);
}

pub fn small_font(font: &UiFont, size: f32) -> TextFont {
    TextFont {
        font: font.regular.clone(),
        font_size: FontSize::Px(size),
        ..default()
    }
}

fn on_cell_click(
    click: On<PointerClick>,
    phase: Res<State<Phase>>,
    cells: Query<&GridCell>,
    tool: Res<Tool>,
    data: Res<GameData>,
    stock: Res<Stock>,
    mut layout: ResMut<ShopLayout>,
    mut storeroom: ResMut<Storeroom>,
    mut log: ResMut<GameLog>,
    lang: Res<Lang>,
    upgrades: Res<Upgrades>,
) {
    if *phase.get() != Phase::Evening {
        return;
    }
    let Ok(GridCell(idx)) = cells.get(click.entity) else {
        return;
    };
    let idx = *idx;
    if matches!(*tool, Tool::Place(_)) && !upgrades.usable(&data, &layout, idx) {
        log.push(lang.t("log.cell_blocked"));
        return;
    }
    match &*tool {
        Tool::None => {}
        Tool::Remove => {
            clear_cell(&mut layout, idx, &mut storeroom);
        }
        Tool::Place(id) => {
            if layout.cells[idx].decor.as_deref() == Some(id.as_str()) {
                return;
            }
            if !storeroom.take(id) {
                log.push(lang.f("log.none_in_storeroom", &[("item", &lang.decor(id))]));
                return;
            }
            clear_cell(&mut layout, idx, &mut storeroom);
            layout.cells[idx] = CellContent {
                decor: Some(id.clone()),
                products: Vec::new(),
            };
        }
        Tool::Stock(product) => {
            let Some(def) = layout.cells[idx]
                .decor
                .as_deref()
                .and_then(|d| data.decor(d))
                .filter(|d| d.is_shelf())
            else {
                log.push(lang.t("log.products_on_shelf"));
                return;
            };
            if stock.count(product) == 0 {
                log.push(lang.f("log.out_of_stock", &[("item", &lang.product(product))]));
                return;
            }
            let cell = &mut layout.cells[idx];
            if cell.products.contains(product) {
                return;
            }
            if cell.products.len() >= def.slots.len() {
                log.push(lang.t("log.shelf_full"));
                return;
            }
            cell.products.push(product.clone());
        }
    }
}

/// A product on a shelf: hover it for details, click it to take it off the
/// shelf (evening) or to hand it to the ghost waiting at the counter (night).
#[derive(Component)]
pub struct ShelfItem {
    cell: usize,
    product: String,
    #[allow(dead_code)]
    slot: usize,
}

/// The shelf item under the mouse pointer.
#[derive(Resource, Default)]
struct HoveredShelfItem(Option<Entity>);

fn on_shelf_item_over(over: On<PointerOver>, mut hovered: ResMut<HoveredShelfItem>) {
    hovered.0 = Some(over.event_target());
}

fn on_shelf_item_out(out: On<PointerOut>, mut hovered: ResMut<HoveredShelfItem>) {
    if hovered.0 == Some(out.event_target()) {
        hovered.0 = None;
    }
}

fn on_shelf_item_click(
    mut click: On<PointerClick>,
    phase: Res<State<Phase>>,
    tool: Res<Tool>,
    items: Query<&ShelfItem>,
    mut layout: ResMut<ShopLayout>,
    mut actions: MessageWriter<Action>,
) {
    if click.button != PointerButton::Primary {
        return;
    }
    let Ok(item) = items.get(click.event_target()) else {
        return;
    };
    match phase.get() {
        // With the furnishing tools the click is meant for the cell.
        Phase::Evening if matches!(*tool, Tool::None | Tool::Stock(_)) => {
            click.propagate(false);
            layout.cells[item.cell]
                .products
                .retain(|p| *p != item.product);
        }
        Phase::Night => {
            click.propagate(false);
            actions.write(Action::Offer(item.product.clone()));
        }
        _ => {}
    }
}

/// Details of the hovered shelf item: price, stock, when it fades, and what
/// a click does right now.
fn shelf_tooltip(
    hovered: Res<HoveredShelfItem>,
    items: Query<&ShelfItem>,
    phase: Res<State<Phase>>,
    data: Res<GameData>,
    lang: Res<Lang>,
    stock: Res<Stock>,
    icons: Res<Icons>,
    progress: Res<Progress>,
    register: Res<Register>,
    ghosts: Query<&Ghost>,
    queue: Res<Queue>,
    upgrades: Res<Upgrades>,
    mut tip: ResMut<HoverTip>,
    mut shown: Local<bool>,
) {
    let Some(item) = hovered.0.and_then(|e| items.get(e).ok()) else {
        if *shown {
            HoverTip::hide(&mut tip);
            *shown = false;
        }
        return;
    };
    let Some(p) = data.product(&item.product) else {
        return;
    };
    let mut t = HoverTip {
        icon: icons.product(&p.id),
        title: lang.product(&p.id),
        lines: Vec::new(),
    };
    t.line(
        TipKind::Text,
        lang.f(
            "ferry.tip_price",
            &[("price", &sale_price(&data, &p.id, progress.calendar(&data)))],
        ),
    );
    t.line(
        TipKind::Text,
        lang.f("ferry.tip_stock", &[("count", &stock.count(&p.id))]),
    );
    // The oldest unit is the next to fade.
    if let Some(oldest) = stock.units().get(&p.id).and_then(|ages| ages.iter().max()) {
        let lasts = p.fade_after + upgrades.fade_bonus(&data);
        let left = lasts.saturating_sub(*oldest);
        let kind = if left <= 1 {
            TipKind::Bad
        } else {
            TipKind::Text
        };
        t.line(kind, lang.f("shelf.tip_fades", &[("nights", &left)]));
    }
    let hint = match phase.get() {
        Phase::Evening => lang.t("shelf.tip_evening"),
        Phase::Night => {
            let waiting = queue
                .front()
                .and_then(|e| ghosts.get(e).ok())
                .is_some_and(|g| g.state == GhostState::Waiting);
            if waiting && register.sale.is_none() {
                lang.t("shelf.tip_night")
            } else {
                lang.t("shelf.tip_no_ghost")
            }
        }
        _ => String::new(),
    };
    if !hint.is_empty() {
        t.line(TipKind::Hint, hint);
    }
    HoverTip::show(&mut tip, t);
    *shown = true;
}

fn select_tool(mut actions: MessageReader<Action>, mut tool: ResMut<Tool>) {
    for action in actions.read() {
        let new = match action {
            Action::ToolPlace(id) => Tool::Place(id.clone()),
            Action::ToolStock(id) => Tool::Stock(id.clone()),
            Action::ToolRemove => Tool::Remove,
            _ => continue,
        };
        // Clicking the same tool again puts it away.
        *tool = if *tool == new { Tool::None } else { new };
    }
}

/// A label that belongs to a grid cell (rebuilt with the cell).
#[derive(Component)]
struct CellLabel;

/// Wares that are sold out (or have faded) leave the shelves, so no empty
/// "×0" slot stays behind; the slot is free for something else.
fn clear_sold_out(stock: Res<Stock>, mut layout: ResMut<ShopLayout>) {
    let sold_out = |p: &String| stock.count(p) == 0;
    if !layout.cells.iter().any(|c| c.products.iter().any(sold_out)) {
        return;
    }
    for cell in &mut layout.cells {
        cell.products.retain(|p| !sold_out(p));
    }
}

/// Redraws decor, products, lights and labels whenever the layout changes.
fn sync_cell_visuals(
    mut commands: Commands,
    layout: Res<ShopLayout>,
    data: Res<GameData>,
    font: Res<UiFont>,
    asset_server: Res<AssetServer>,
    stock: Res<Stock>,
    lang: Res<Lang>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    cells: Query<(Entity, &GridCell)>,
    labels: Query<Entity, With<CellLabel>>,
) {
    for label in &labels {
        commands.entity(label).despawn();
    }
    for (entity, GridCell(idx)) in &cells {
        commands.entity(entity).despawn_related::<Children>();
        let content = &layout.cells[*idx];
        let Some(def) = content.decor.as_deref().and_then(|d| data.decor(d)) else {
            continue;
        };
        let model_path = format!("models/decor/{}.glb", def.id);
        let has_model = asset_exists(&model_path);
        let size = Vec3::new(def.size.0, def.size.1, def.size.2);
        let visual = model_or_box(
            &mut commands,
            &asset_server,
            &mut meshes,
            &mut materials,
            &model_path,
            size,
            rgb(def.color),
        );
        let mut children = vec![visual];
        // Models from files cannot be hit by the pointer (their meshes live
        // on the graphics card only), so an invisible box of the furniture's
        // size catches it – for grabbing and moving it.
        children.push(
            commands
                .spawn((
                    Mesh3d(meshes.add(Cuboid::from_size(size))),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color: Color::NONE,
                        alpha_mode: AlphaMode::Blend,
                        unlit: true,
                        ..default()
                    })),
                    Transform::from_xyz(0.0, size.y / 2.0, 0.0),
                    NotShadowCaster,
                ))
                .id(),
        );

        if def.effect.light > 0 {
            children.push(spawn_light(
                &mut commands,
                &mut meshes,
                &mut materials,
                def,
                has_model,
                *idx,
            ));
        }

        for (slot, p) in content.products.iter().enumerate() {
            let Some(&(x, y, z)) = def.slots.get(slot) else {
                continue;
            };
            let item = product_model(
                &mut commands,
                &asset_server,
                &mut meshes,
                &mut materials,
                p,
                Vec3::new(x, y, z),
            );
            // An invisible box a little bigger than the product and towards
            // the front, so the pointer finds it before the shelf boards.
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.34, 0.32, 0.34))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::NONE,
                    alpha_mode: AlphaMode::Blend,
                    unlit: true,
                    ..default()
                })),
                Transform::from_xyz(0.0, 0.16, 0.12),
                NotShadowCaster,
                ChildOf(item),
            ));
            commands
                .entity(item)
                .insert(ShelfItem {
                    cell: *idx,
                    slot,
                    product: p.clone(),
                })
                .observe(on_shelf_item_click)
                .observe(on_shelf_item_over)
                .observe(on_shelf_item_out);
            children.push(item);
        }

        commands.entity(entity).add_children(&children);

        // Shelves say how many they hold (and what, while the product has no
        // model to show it); placeholders say what they are.
        let text = if content.products.is_empty() {
            (!has_model).then(|| lang.decor(&def.id))
        } else {
            let counts: Vec<String> = content
                .products
                .iter()
                .map(|p| {
                    if asset_exists(&format!("models/products/{p}.glb")) {
                        format!("×{}", stock.count(p))
                    } else {
                        lang.f(
                            "evening.stock_button",
                            &[("item", &lang.product(p)), ("count", &stock.count(p))],
                        )
                    }
                })
                .collect();
            Some(counts.join(" · "))
        };
        if let Some(text) = text {
            commands.spawn((
                CellLabel,
                world_label(
                    &font,
                    text,
                    11.0,
                    entity,
                    Vec3::new(0.0, size.y + 0.15, 0.0),
                ),
            ));
        }
    }
}

/// A warm, flickering point light (plus a little flame for placeholders).
fn spawn_light(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    def: &DecorDef,
    has_model: bool,
    idx: usize,
) -> Entity {
    // Several lamps can stand side by side. Give each a local pool and
    // reserve the wider reach for brighter furniture such as the oil lamp.
    let strength = def.effect.light as f32;
    let intensity = 54_000.0 * strength.sqrt();
    // Models put the flame at light_height; on a placeholder box it sits on top.
    let height = if has_model {
        def.light_height
    } else {
        def.size.1 + 0.08
    };
    let light = commands
        .spawn((
            PointLight {
                color: Color::srgb(1.0, 0.72, 0.42),
                intensity,
                range: 2.8 + 0.85 * strength,
                radius: 0.10,
                shadow_maps_enabled: false,
                ..default()
            },
            Flicker {
                base: intensity,
                phase: idx as f32 * 1.7,
            },
            Transform::from_xyz(0.0, height, 0.0),
        ))
        .id();
    if !has_model {
        let flame = commands
            .spawn((
                Mesh3d(meshes.add(Sphere::new(0.06))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(1.0, 0.8, 0.4),
                    emissive: LinearRgba::rgb(8.0, 4.0, 1.2),
                    ..default()
                })),
                NotShadowCaster,
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(light).add_child(flame);
    }
    light
}

/// A product standing at `spot`: its model, or its picture as a small
/// billboard (also used on the ferryman's crates).
/// Products are shown a little larger than life, so they read well on the
/// shelves from the camera (the models keep their real size).
pub const PRODUCT_DISPLAY_SCALE: f32 = 1.35;

pub(crate) fn product_model(
    commands: &mut Commands,
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    id: &str,
    spot: Vec3,
) -> Entity {
    let model = format!("models/products/{id}.glb");
    if asset_exists(&model) {
        commands
            .spawn((
                WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(model))),
                Transform::from_translation(spot).with_scale(Vec3::splat(PRODUCT_DISPLAY_SCALE)),
            ))
            .id()
    } else {
        commands
            .spawn((
                Mesh3d(billboard_mesh(
                    meshes,
                    Vec2::splat(0.38 * PRODUCT_DISPLAY_SCALE),
                )),
                MeshMaterial3d(billboard_material(
                    materials,
                    asset_server,
                    &format!("sprites/products/{id}.png"),
                    Color::srgb(0.85, 0.8, 0.6),
                )),
                Transform::from_translation(spot + Vec3::new(0.0, 0.0, 0.2)),
                Billboard,
                NotShadowCaster,
            ))
            .id()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sold_out_wares_leave_the_shelves() {
        let mut app = App::new();
        let mut layout = ShopLayout::new(4, 4);
        layout.cells[0].decor = Some("shelf".into());
        layout.cells[0].products = vec!["tea_tin".into(), "marbles".into()];
        let mut stock = Stock::default();
        stock.add("marbles", 2);
        app.insert_resource(layout).insert_resource(stock);
        app.add_systems(Update, clear_sold_out);
        app.update();
        let layout = app.world().resource::<ShopLayout>();
        assert_eq!(layout.cells[0].products, vec!["marbles".to_string()]);
    }

    #[test]
    fn furniture_goes_back_into_the_storeroom_when_replaced() {
        let mut layout = ShopLayout::new(4, 4);
        let mut storeroom = Storeroom::default();
        storeroom.give("umbrella");
        assert!(storeroom.take("umbrella"));
        assert!(!storeroom.take("umbrella"));
        layout.cells[0].decor = Some("umbrella".into());
        clear_cell(&mut layout, 0, &mut storeroom);
        assert_eq!(storeroom.count("umbrella"), 1);
        // Bought furniture comes back too, and its products stay in stock.
        layout.cells[1] = CellContent {
            decor: Some("shelf".into()),
            products: vec!["marbles".into()],
        };
        clear_cell(&mut layout, 1, &mut storeroom);
        assert_eq!(storeroom.count("shelf"), 1);
        assert!(layout.cells[1].products.is_empty());
    }
}
