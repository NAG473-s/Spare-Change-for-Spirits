//! Chalk corners and an enamel symbol identify the square under a furnishing tool.
use super::{
    CELL, Dragging, GridCell, HoveredShelfItem, ShelfItem, ShopLayout, Storeroom, Tool,
    room_on_shelf,
};
use crate::data::{GameData, asset_exists};
use crate::day_cycle::Phase;
use crate::harbor::Location;
use crate::inventory::Stock;
use crate::menu::Paused;
use crate::scene::{Billboard, billboard_mesh, floor};
use crate::upgrades::Upgrades;
use bevy::asset::RenderAssetUsages;
use bevy::light::NotShadowCaster;
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

#[derive(Clone, Copy, PartialEq, Debug)]
enum Mark {
    Place,
    Swap,
    Blocked,
    Remove,
}

impl Mark {
    fn for_cell(
        data: &GameData,
        layout: &ShopLayout,
        upgrades: &Upgrades,
        room: &Storeroom,
        stock: &Stock,
        tool: &Tool,
        drag: &Dragging,
        idx: usize,
    ) -> Option<Self> {
        let cell = &layout.cells[idx];
        if let Some(from) = drag.from {
            return Some(if !upgrades.usable(data, layout, idx) {
                Self::Blocked
            } else if from != idx && cell.decor.is_some() {
                Self::Swap
            } else {
                Self::Place
            });
        }
        match tool {
            Tool::Place(id) if cell.decor.as_ref() == Some(id) => None,
            Tool::Place(id) if !upgrades.usable(data, layout, idx) || room.count(id) == 0 => {
                Some(Self::Blocked)
            }
            Tool::Place(_) if cell.decor.is_some() => Some(Self::Swap),
            Tool::Place(_) => Some(Self::Place),
            Tool::Remove if cell.decor.is_some() => Some(Self::Remove),
            Tool::Stock(id) => Some(if stock.count(id) > 0 && room_on_shelf(data, cell, id) {
                Self::Place
            } else {
                Self::Blocked
            }),
            _ => None,
        }
    }
}

#[derive(Component)]
pub(super) struct Guide;

#[derive(Component)]
pub(super) struct Badge([Handle<StandardMaterial>; 4]);

fn fallback(mask: [u8; 7]) -> Image {
    let pixels: Vec<u8> = mask
        .iter()
        .flat_map(|row| {
            (0..7).flat_map(move |x| {
                if row & (64 >> x) != 0 {
                    [255, 236, 191, 255]
                } else {
                    [57, 43, 46, 255]
                }
            })
        })
        .collect();
    Image::new(
        Extent3d {
            width: 7,
            height: 7,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

pub(super) fn spawn(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let path = "sprites/shop/placement_corners.png";
    let corners = asset_exists(path);
    let floor_material = materials.add(StandardMaterial {
        base_color: if corners {
            Color::WHITE
        } else {
            Color::srgba(0.87, 0.74, 0.49, 0.22)
        },
        base_color_texture: corners.then(|| assets.load(path)),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    let guide = commands
        .spawn((
            Guide,
            Mesh3d(meshes.add(Plane3d::default().mesh().size(CELL * 1.03, CELL * 1.03))),
            MeshMaterial3d(floor_material),
            Visibility::Hidden,
            Transform::default(),
            Pickable::IGNORE,
            NotShadowCaster,
        ))
        .id();
    let faces = [
        ("place", [0, 1, 2, 68, 40, 16, 0]),
        ("swap", [4, 126, 4, 0, 16, 63, 16]),
        ("blocked", [0, 34, 20, 8, 20, 34, 0]),
        ("remove", [0, 0, 0, 62, 0, 0, 0]),
    ]
    .map(|(id, mask)| {
        let path = format!("sprites/shop/placement_{id}.png");
        let image = if asset_exists(&path) {
            assets.load(path)
        } else {
            images.add(fallback(mask))
        };
        materials.add(StandardMaterial {
            base_color_texture: Some(image),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        })
    });
    commands.spawn((
        Mesh3d(billboard_mesh(&mut meshes, Vec2::splat(0.42))),
        MeshMaterial3d(faces[0].clone()),
        Badge(faces),
        Billboard,
        Transform::from_xyz(-0.25, 0.18, 0.55),
        Pickable::IGNORE,
        NotShadowCaster,
        ChildOf(guide),
    ));
}

pub(super) fn update(
    phase: Res<State<Phase>>,
    location: Res<Location>,
    paused: Res<Paused>,
    tool: Res<Tool>,
    drag: Res<Dragging>,
    data: Res<GameData>,
    layout: Res<ShopLayout>,
    upgrades: Res<Upgrades>,
    room: Res<Storeroom>,
    stock: Res<Stock>,
    cells: Query<(&GridCell, &Hovered)>,
    shelf_hover: Res<HoveredShelfItem>,
    wares: Query<&ShelfItem>,
    mut guide: Single<(&mut Transform, &mut Visibility), With<Guide>>,
    mut badge: Single<
        (
            &Badge,
            &mut MeshMaterial3d<StandardMaterial>,
            &mut Transform,
        ),
        Without<Guide>,
    >,
) {
    let mark = if *phase.get() != Phase::Evening || *location != Location::Shop || paused.0 {
        None
    } else {
        let index = if drag.from.is_some() {
            drag.over
        } else {
            cells.iter().find(|(_, h)| h.get()).map(|(cell, _)| cell.0)
        };
        index.and_then(|i| {
            // Clicking an existing shelf sample with the stock tool takes that sample down.
            let over_sample = matches!(*tool, Tool::Stock(_))
                && shelf_hover
                    .0
                    .and_then(|e| wares.get(e).ok())
                    .is_some_and(|s| s.cell == i);
            let mark = if over_sample {
                Some(Mark::Remove)
            } else {
                Mark::for_cell(&data, &layout, &upgrades, &room, &stock, &tool, &drag, i)
            };
            mark.map(|m| (i, m))
        })
    };
    let (at, vis) = &mut *guide;
    if let Some((idx, mark)) = mark {
        at.set_if_neq(Transform::from_translation(floor(
            layout.cell_pos(idx),
            0.024,
        )));
        vis.set_if_neq(Visibility::Inherited);
        let (faces, mat, badge_pose) = &mut *badge;
        // Lift the sign clear of a blocking case. Keep it to the left of the pointer tooltip.
        let height = if mark == Mark::Blocked && !upgrades.usable(&data, &layout, idx) {
            1.0
        } else {
            0.18
        };
        if badge_pose.translation.y != height {
            badge_pose.translation.y = height;
        }
        mat.set_if_neq(MeshMaterial3d(faces.0[mark as usize].clone()));
    } else {
        vis.set_if_neq(Visibility::Hidden);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn moving_and_furnishing_distinguish_empty_occupied_and_locked_squares() {
        let data = GameData::load();
        let layout = ShopLayout::new(data.config.grid_cols, data.config.grid_rows);
        let upgrades = Upgrades::default();
        let mut room = Storeroom::default();
        room.give("mirror");
        let stock = Stock::default();
        let empty =
            super::super::free_cell_for(&data, &layout, &upgrades, &Tool::Place("mirror".into()))
                .unwrap();
        let occupied = layout
            .cells
            .iter()
            .position(|c| c.decor.as_deref() == Some("shelf"))
            .unwrap();
        let locked = (0..layout.cells.len())
            .find(|i| !upgrades.usable(&data, &layout, *i))
            .unwrap();
        let select = |tool, drag: &Dragging, idx| {
            Mark::for_cell(&data, &layout, &upgrades, &room, &stock, &tool, drag, idx)
        };
        let drag = Dragging {
            from: Some(occupied),
            over: Some(empty),
        };
        assert_eq!(select(Tool::None, &drag, empty), Some(Mark::Place));
        let idle = Dragging::default();
        assert_eq!(
            select(Tool::Place("mirror".into()), &idle, occupied),
            Some(Mark::Swap)
        );
        assert_eq!(select(Tool::None, &drag, locked), Some(Mark::Blocked));
        assert_eq!(select(Tool::Remove, &idle, occupied), Some(Mark::Remove));
        assert_eq!(select(Tool::Remove, &idle, empty), None);
    }
}
