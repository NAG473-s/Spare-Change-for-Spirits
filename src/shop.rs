//! The shop: a floor grid with shelves and decor, plus door and counter, in
//! the 3D diorama. Every piece is a model from `assets/models/` if it exists,
//! otherwise a coloured placeholder box of the size given in `decor.ron`.
//!
//! In the evening the player picks a tool in the side panel and clicks a cell.

use std::collections::BTreeMap;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use bevy::light::NotShadowCaster;

use crate::data::{Atmosphere, DecorDef, GameData, asset_exists, rgb};
use crate::day_cycle::Phase;
use crate::economy::Drawer;
use crate::fx::Flicker;
use crate::i18n::Lang;
use crate::inventory::Stock;
use crate::scene::{Billboard, billboard_material, billboard_mesh, floor, world_label};
use crate::ui::{Action, GameLog, UiFont};

/// One grid cell is one metre.
pub const CELL: f32 = 1.0;
/// Centre of the floor grid (x, z).
const GRID_CENTER: Vec2 = Vec2::new(0.0, 0.2);
/// Height of the shelf board where products stand.
const SHELF_BOARD: f32 = 0.75;
const COUNTER_SIZE: Vec3 = Vec3::new(4.5, 1.0, 0.8);
const DOOR_SIZE: Vec3 = Vec3::new(1.1, 2.3, 0.2);

pub struct ShopPlugin;

impl Plugin for ShopPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Tool>()
            .init_resource::<Keepsakes>()
            .add_systems(Startup, spawn_shop)
            .add_systems(OnExit(Phase::Evening), |mut t: ResMut<Tool>| {
                *t = Tool::None
            })
            .add_systems(
                Update,
                (
                    select_tool.run_if(in_state(Phase::Evening)),
                    sync_cell_visuals.run_if(
                        resource_changed::<ShopLayout>
                            .or_else(resource_changed::<Stock>)
                            .or_else(resource_changed::<UiFont>),
                    ),
                    translate_counter.run_if(resource_changed::<UiFont>),
                ),
            );
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct CellContent {
    pub decor: Option<String>,
    /// Shelves only: which product is displayed there.
    pub product: Option<String>,
}

#[derive(Resource)]
pub struct ShopLayout {
    pub cols: usize,
    pub rows: usize,
    pub cells: Vec<CellContent>,
}

impl ShopLayout {
    pub fn new(cols: usize, rows: usize) -> Self {
        let mut layout = Self {
            cols,
            rows,
            cells: vec![CellContent::default(); cols * rows],
        };
        // Starting furniture: two shelves and a candle.
        layout.cells[cols + 2].decor = Some("shelf".into());
        layout.cells[cols + 4].decor = Some("shelf".into());
        layout.cells[3 * cols + 3].decor = Some("candle".into());
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
                    .is_some_and(|d| d.is_shelf)
            })
            .collect()
    }

    /// All products currently on a shelf (without duplicates).
    pub fn displayed_products(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for c in &self.cells {
            if let Some(p) = &c.product
                && !out.contains(p)
            {
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
        for c in &self.cells {
            if let Some(d) = c.decor.as_deref().and_then(|d| data.decor(d)) {
                a.add(&d.effect);
            }
        }
        a
    }
}

/// Keepsakes the player owns but has not placed yet (decor id → count).
#[derive(Resource, Default)]
pub struct Keepsakes(pub BTreeMap<String, u32>);

impl Keepsakes {
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

/// Clears a cell. A keepsake that stood there goes back into the box.
fn clear_cell(layout: &mut ShopLayout, idx: usize, data: &GameData, keepsakes: &mut Keepsakes) {
    if let Some(d) = layout.cells[idx]
        .decor
        .as_deref()
        .and_then(|d| data.decor(d))
        && d.keepsake
    {
        keepsakes.give(&d.id);
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

/// Model if `assets/<path>` exists, otherwise a placeholder box; either way
/// standing on its origin.
fn model_or_box(
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
            .observe(on_cell_click);
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
            Vec3::new(0.0, COUNTER_SIZE.y + 0.3 - y, 0.0),
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
    mut drawer: ResMut<Drawer>,
    mut keepsakes: ResMut<Keepsakes>,
    mut log: ResMut<GameLog>,
    lang: Res<Lang>,
) {
    if *phase.get() != Phase::Evening {
        return;
    }
    let Ok(GridCell(idx)) = cells.get(click.entity) else {
        return;
    };
    let idx = *idx;
    match &*tool {
        Tool::None => {}
        Tool::Remove => {
            clear_cell(&mut layout, idx, &data, &mut keepsakes);
        }
        Tool::Place(id) => {
            let Some(def) = data.decor(id) else { return };
            if layout.cells[idx].decor.as_deref() == Some(id.as_str()) {
                return;
            }
            if def.keepsake {
                if !keepsakes.take(id) {
                    log.push(lang.f("log.keepsake_placed", &[("item", &lang.decor(id))]));
                    return;
                }
            } else if !drawer.pay(def.cost) {
                log.push(lang.f(
                    "log.decor_too_expensive",
                    &[("item", &lang.decor(id)), ("cost", &def.cost)],
                ));
                return;
            }
            clear_cell(&mut layout, idx, &data, &mut keepsakes);
            layout.cells[idx] = CellContent {
                decor: Some(id.clone()),
                product: None,
            };
        }
        Tool::Stock(product) => {
            let is_shelf = layout.cells[idx]
                .decor
                .as_deref()
                .and_then(|d| data.decor(d))
                .is_some_and(|d| d.is_shelf);
            if !is_shelf {
                log.push(lang.t("log.products_on_shelf"));
                return;
            }
            if stock.count(product) == 0 {
                log.push(lang.f("log.out_of_stock", &[("item", &lang.product(product))]));
                return;
            }
            layout.cells[idx].product = Some(product.clone());
        }
    }
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

        if let Some(p) = content.product.as_deref() {
            children.push(spawn_product(
                &mut commands,
                &asset_server,
                &mut meshes,
                &mut materials,
                p,
            ));
        }

        commands.entity(entity).add_children(&children);

        // Shelves say what they hold; placeholders say what they are.
        let text = match content.product.as_deref() {
            Some(p) => Some(lang.f(
                "evening.stock_button",
                &[("item", &lang.product(p)), ("count", &stock.count(p))],
            )),
            None if !has_model => Some(lang.decor(&def.id)),
            None => None,
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
    let intensity = 180_000.0 * def.effect.light as f32;
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
                range: 5.0 + 1.5 * def.effect.light as f32,
                radius: 0.05,
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

/// The product on a shelf: its model, or its picture as a small billboard.
fn spawn_product(
    commands: &mut Commands,
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    id: &str,
) -> Entity {
    let model = format!("models/products/{id}.glb");
    let spot = Vec3::new(0.0, SHELF_BOARD, 0.08);
    if asset_exists(&model) {
        commands
            .spawn((
                WorldAssetRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(model))),
                Transform::from_translation(spot),
            ))
            .id()
    } else {
        commands
            .spawn((
                Mesh3d(billboard_mesh(meshes, Vec2::splat(0.38))),
                MeshMaterial3d(billboard_material(
                    materials,
                    asset_server,
                    &format!("sprites/products/{id}.png"),
                    Color::srgb(0.85, 0.8, 0.6),
                )),
                Transform::from_translation(spot + Vec3::new(0.0, 0.0, 0.2)),
                Billboard,
                NotShadowCaster,
                Pickable::IGNORE,
            ))
            .id()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keepsakes_go_back_into_the_box_when_replaced() {
        let data = GameData::load();
        let mut layout = ShopLayout::new(4, 4);
        let mut keepsakes = Keepsakes::default();
        keepsakes.give("umbrella");
        assert!(keepsakes.take("umbrella"));
        assert!(!keepsakes.take("umbrella"));
        layout.cells[0].decor = Some("umbrella".into());
        clear_cell(&mut layout, 0, &data, &mut keepsakes);
        assert_eq!(keepsakes.count("umbrella"), 1);
        // Bought decor is simply gone.
        layout.cells[1].decor = Some("candle".into());
        clear_cell(&mut layout, 1, &data, &mut keepsakes);
        assert_eq!(keepsakes.count("candle"), 0);
    }
}
