//! The shop: a grid with shelves and decor, plus door and counter.
//!
//! In the evening the player picks a tool in the side panel and clicks a cell.

use bevy::prelude::*;

use crate::data::{Atmosphere, GameData, asset_exists, rgb};
use crate::day_cycle::Phase;
use crate::economy::Drawer;
use crate::ui::{Action, GameLog, UiFont};

pub const CELL: f32 = 64.0;
/// Centre of the grid in world coordinates (the side panel sits on the right).
const GRID_CENTER: Vec2 = Vec2::new(-200.0, -20.0);

pub struct ShopPlugin;

impl Plugin for ShopPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Tool>()
            .add_systems(Startup, spawn_shop)
            .add_systems(OnExit(Phase::Evening), |mut t: ResMut<Tool>| {
                *t = Tool::None
            })
            .add_systems(
                Update,
                (
                    select_tool.run_if(in_state(Phase::Evening)),
                    sync_cell_visuals.run_if(resource_changed::<ShopLayout>),
                ),
            );
    }
}

#[derive(Clone, Default, Debug)]
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
    pub fn queue_pos(&self, place: usize) -> Vec2 {
        self.counter_pos() + Vec2::new(-46.0, 100.0 - place as f32 * 72.0)
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

    for idx in 0..layout.cells.len() {
        let checker = if (idx % layout.cols + idx / layout.cols).is_multiple_of(2) {
            0.13
        } else {
            0.11
        };
        commands
            .spawn((
                GridCell(idx),
                Sprite::from_color(
                    Color::srgb(checker, checker, checker + 0.03),
                    Vec2::splat(CELL - 2.0),
                ),
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
            Vec2::new(50.0, 110.0),
        ),
        Transform::from_translation(door.extend(0.0)),
    ));
    let counter = layout.counter_pos();
    commands.spawn((
        sprite_for(
            &asset_server,
            "sprites/shop/counter.png",
            Color::srgb(0.35, 0.24, 0.15),
            Vec2::new(40.0, 260.0),
        ),
        Transform::from_translation(counter.extend(0.0)),
    ));
    commands.spawn((
        Text2d::new("Counter"),
        small_font(&font, 12.0),
        TextColor(Color::srgb(0.8, 0.75, 0.6)),
        Transform::from_translation((counter + Vec2::new(0.0, 145.0)).extend(1.0)),
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
    stock: Res<crate::inventory::Stock>,
    mut layout: ResMut<ShopLayout>,
    mut drawer: ResMut<Drawer>,
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
            layout.cells[idx] = CellContent::default();
        }
        Tool::Place(id) => {
            let Some(def) = data.decor(id) else { return };
            if layout.cells[idx].decor.as_deref() == Some(id.as_str()) {
                return;
            }
            if !drawer.pay(def.cost) {
                log.push(format!(
                    "{} costs {} – not enough in the drawer.",
                    def.name, def.cost
                ));
                return;
            }
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
            Some(p) => data.product_name(p).to_string(),
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
                Vec2::splat(22.0),
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
