//! The shop: a grid with shelves and decor, plus door and counter.
//!
//! In the evening the player picks a tool in the side panel and clicks a cell.

use std::collections::BTreeMap;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::data::{Atmosphere, GameData, asset_exists, rgb};
use crate::day_cycle::Phase;
use crate::economy::Drawer;
use crate::inventory::Stock;
use crate::ui::{Action, GameLog, UiFont};

pub const CELL: f32 = 64.0;
/// Centre of the grid in world coordinates (the side panel sits on the right).
const GRID_CENTER: Vec2 = Vec2::new(-200.0, -20.0);
// Drawn sizes; tools/prepare_sprite.py produces images with these aspect ratios.
const DOOR_SIZE: Vec2 = Vec2::new(64.0, 96.0);
const COUNTER_SIZE: Vec2 = Vec2::new(48.0, 272.0);
const PRODUCT_SIZE: Vec2 = Vec2::new(24.0, 24.0);

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
                    sync_cell_visuals
                        .run_if(resource_changed::<ShopLayout>.or_else(resource_changed::<Stock>)),
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

    pub fn cell_pos(&self, idx: usize) -> Vec2 {
        let x = (idx % self.cols) as f32;
        let y = (idx / self.cols) as f32;
        let origin = GRID_CENTER
            - Vec2::new(self.cols as f32 * CELL, self.rows as f32 * CELL) / 2.0
            + Vec2::splat(CELL / 2.0);
        origin + Vec2::new(x * CELL, y * CELL)
    }

    pub fn door_pos(&self) -> Vec2 {
        Vec2::new(
            GRID_CENTER.x - self.cols as f32 * CELL / 2.0 - 60.0,
            GRID_CENTER.y,
        )
    }

    pub fn counter_pos(&self) -> Vec2 {
        Vec2::new(
            GRID_CENTER.x + self.cols as f32 * CELL / 2.0 + 70.0,
            GRID_CENTER.y,
        )
    }

    /// Spots in front of the counter where ghosts queue.
    /// A long line moves closer together so it stays above the message log.
    pub fn queue_pos(&self, place: usize, len: usize) -> Vec2 {
        let spacing = (300.0 / len.max(1) as f32).min(72.0);
        self.counter_pos() + Vec2::new(-46.0, 100.0 - place as f32 * spacing)
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

    pub fn atmosphere(&self, data: &GameData) -> Atmosphere {
        let mut a = data.config.base_atmosphere;
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

fn spawn_shop(
    mut commands: Commands,
    data: Res<GameData>,
    font: Res<UiFont>,
    asset_server: Res<AssetServer>,
) {
    let layout = ShopLayout::new(data.config.grid_cols, data.config.grid_rows);

    // Back wall behind everything (optional image).
    if asset_exists("sprites/shop/background.png") {
        commands.spawn((
            Sprite {
                image: asset_server.load("sprites/shop/background.png"),
                custom_size: Some(Vec2::new(1280.0, 720.0)),
                ..default()
            },
            Transform::from_xyz(0.0, 0.0, -10.0),
            Pickable::IGNORE,
        ));
    }

    let has_floor = asset_exists("sprites/shop/floor.png");
    for idx in 0..layout.cells.len() {
        let even = (idx % layout.cols + idx / layout.cols).is_multiple_of(2);
        let sprite = if has_floor {
            // A light checker tint keeps the grid readable on a floor texture.
            let tint = if even { 1.0 } else { 0.88 };
            Sprite {
                image: asset_server.load("sprites/shop/floor.png"),
                custom_size: Some(Vec2::splat(CELL - 2.0)),
                color: Color::srgb(tint, tint, tint),
                ..default()
            }
        } else {
            let c = if even { 0.13 } else { 0.11 };
            Sprite::from_color(Color::srgb(c, c, c + 0.03), Vec2::splat(CELL - 2.0))
        };
        commands
            .spawn((
                GridCell(idx),
                sprite,
                Transform::from_translation(layout.cell_pos(idx).extend(0.0)),
                Pickable::default(),
            ))
            .observe(on_cell_click);
    }

    // Door and counter (placeholders until there are images).
    let door = layout.door_pos();
    commands.spawn((
        sprite_for(
            &asset_server,
            "sprites/shop/door.png",
            Color::srgb(0.25, 0.18, 0.12),
            DOOR_SIZE,
        ),
        Transform::from_translation(door.extend(0.0)),
    ));
    let counter = layout.counter_pos();
    commands.spawn((
        sprite_for(
            &asset_server,
            "sprites/shop/counter.png",
            Color::srgb(0.35, 0.24, 0.15),
            COUNTER_SIZE,
        ),
        Transform::from_translation(counter.extend(0.0)),
    ));
    commands.spawn((
        Text2d::new("Counter"),
        small_font(&font, 12.0),
        TextColor(Color::srgb(0.8, 0.75, 0.6)),
        Transform::from_translation(
            (counter + Vec2::new(0.0, COUNTER_SIZE.y / 2.0 + 12.0)).extend(1.0),
        ),
    ));

    commands.insert_resource(layout);
}

pub fn small_font(font: &UiFont, size: f32) -> TextFont {
    TextFont {
        font: font.regular.clone().into(),
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
                    log.push(format!("{} is already in the shop.", def.name));
                    return;
                }
            } else if !drawer.pay(def.cost) {
                log.push(format!(
                    "{} costs {} – not enough in the drawer.",
                    def.name, def.cost
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
                log.push("Products belong on a shelf.");
                return;
            }
            if stock.count(product) == 0 {
                log.push(format!("No {} left in stock.", data.product_name(product)));
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

/// Redraws decor and products whenever the layout changes.
fn sync_cell_visuals(
    mut commands: Commands,
    layout: Res<ShopLayout>,
    data: Res<GameData>,
    font: Res<UiFont>,
    asset_server: Res<AssetServer>,
    stock: Res<Stock>,
    cells: Query<(Entity, &GridCell)>,
) {
    for (entity, GridCell(idx)) in &cells {
        commands.entity(entity).despawn_related::<Children>();
        let content = &layout.cells[*idx];
        let Some(def) = content.decor.as_deref().and_then(|d| data.decor(d)) else {
            continue;
        };

        let sprite = sprite_for(
            &asset_server,
            &format!("sprites/decor/{}.png", def.id),
            rgb(def.color),
            Vec2::splat(CELL - 12.0),
        );
        let label = match content.product.as_deref() {
            Some(p) => format!("{} ×{}", data.product_name(p), stock.count(p)),
            None => def.name.clone(),
        };
        let mut children = vec![
            commands
                .spawn((sprite, Transform::from_xyz(0.0, 0.0, 1.0), Pickable::IGNORE))
                .id(),
            commands
                .spawn((
                    Text2d::new(label),
                    small_font(&font, 10.0),
                    TextColor(Color::WHITE),
                    TextLayout::justify(Justify::Center),
                    bevy::text::TextBounds::new_horizontal(CELL - 4.0),
                    Transform::from_xyz(0.0, -20.0, 3.0),
                    Pickable::IGNORE,
                ))
                .id(),
        ];
        if let Some(p) = content.product.as_deref() {
            let icon = sprite_for(
                &asset_server,
                &format!("sprites/products/{p}.png"),
                Color::srgb(0.85, 0.8, 0.6),
                PRODUCT_SIZE,
            );
            children.push(
                commands
                    .spawn((icon, Transform::from_xyz(0.0, 6.0, 2.0), Pickable::IGNORE))
                    .id(),
            );
        }
        commands.entity(entity).add_children(&children);
    }
}

/// Loads the image if the file exists, otherwise a coloured placeholder.
pub fn sprite_for(asset_server: &AssetServer, path: &str, color: Color, size: Vec2) -> Sprite {
    if asset_exists(path) {
        Sprite {
            image: asset_server.load(path.to_string()),
            custom_size: Some(size),
            ..default()
        }
    } else {
        Sprite::from_color(color, size)
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
