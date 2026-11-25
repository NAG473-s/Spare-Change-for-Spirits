//! The old cargo on the locked floor, and the item actually found at the workbench.
use bevy::light::NotShadowCaster;
use bevy::picking::hover::Hovered;
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use serde::Deserialize;

use super::Upgrades;
use crate::data::{GameData, asset_exists, rgb};
use crate::day_cycle::{Phase, Progress};
use crate::harbor::Location;
use crate::i18n::Lang;
use crate::menu::Paused;
use crate::save::Settings;
use crate::scene::floor;
use crate::shop::{ShopLayout, Tool, bind_floor_target, model_or_box, product_model};
use crate::tooltip::{HoverTip, TipKind};
use crate::ui::Action;
use crate::workbench::{Rummaged, has_workbench, use_workbench};

#[derive(Deserialize, Clone, Debug)]
pub struct PileDef {
    model: String,
    size: (f32, f32, f32),
    color: (f32, f32, f32),
    #[serde(default)]
    lid: Option<LidDef>,
}

#[derive(Deserialize, Clone, Debug)]
struct LidDef {
    size: (f32, f32, f32),
    /// Degrees the hinge travels when searched.
    angle: f32,
}

#[derive(Component)]
struct Pile {
    cell: usize,
    rim: f32,
    hinge: Option<(Entity, f32)>,
    target: Entity,
}

#[derive(Component)]
struct Hinge;
#[derive(Component)]
struct SearchTarget;

#[derive(Resource)]
struct HitVolume {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

impl FromWorld for HitVolume {
    fn from_world(world: &mut World) -> Self {
        Self {
            mesh: world
                .resource_mut::<Assets<Mesh>>()
                .add(Cuboid::from_size(Vec3::ONE)),
            material: world
                .resource_mut::<Assets<StandardMaterial>>()
                .add(StandardMaterial {
                    base_color: Color::NONE,
                    alpha_mode: AlphaMode::Blend,
                    unlit: true,
                    ..default()
                }),
        }
    }
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<HitVolume>()
        .add_systems(
            Update,
            (
                sync_piles
                    .run_if(resource_changed::<Upgrades>.or_else(resource_added::<ShopLayout>)),
                search_targets.after(sync_piles),
                (
                    show_find.after(use_workbench).after(sync_piles),
                    animate_find,
                )
                    .chain()
                    .run_if(in_state(Phase::Evening))
                    .run_if(crate::menu::shop_unpaused),
            ),
        )
        .add_systems(
            OnExit(Phase::Evening),
            |mut hinges: Query<&mut Transform, With<Hinge>>| {
                for mut at in &mut hinges {
                    at.rotation = Quat::IDENTITY;
                }
            },
        );
}

/// Offset alternating rows and the start of each repeated group. Cells keep
/// their cargo when the season or the unlocked area changes, without using gameplay RNG.
fn arrangement(cell: usize, cols: usize, count: usize) -> (usize, f32) {
    let (x, row) = (cell % cols, cell / cols);
    let variant = (x + x / count + 2 * row + row.div_ceil(2)) % count;
    let angle = (((x * 17 + row * 11) % 13) as f32 - 6.0) * 0.032;
    (variant, angle)
}

fn sync_piles(
    mut commands: Commands,
    data: Res<GameData>,
    layout: Res<ShopLayout>,
    upgrades: Res<Upgrades>,
    assets: Res<AssetServer>,
    hit: Res<HitVolume>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    piles: Query<(Entity, &Pile)>,
) {
    let (width, depth) = upgrades.area(&data);
    let blocked = |cell: usize| cell % layout.cols >= width || cell / layout.cols >= depth;
    let mut occupied = vec![false; layout.cells.len()];
    for (entity, pile) in &piles {
        if pile.cell < occupied.len() && blocked(pile.cell) {
            occupied[pile.cell] = true;
        } else {
            commands.entity(entity).despawn();
        }
    }
    let variants = &data.workbench.piles;
    if variants.is_empty() {
        return;
    }
    for (cell, present) in occupied.into_iter().enumerate() {
        if present || !blocked(cell) {
            continue;
        }
        let (variant, angle) = arrangement(cell, layout.cols, variants.len());
        let def = &variants[variant];
        let size = Vec3::from(def.size);
        let root = commands
            .spawn((
                Transform::from_translation(floor(layout.cell_pos(cell), 0.0))
                    .with_rotation(Quat::from_rotation_y(angle)),
                Visibility::default(),
                Pickable::IGNORE,
            ))
            .id();
        let body = model_or_box(
            &mut commands,
            &assets,
            &mut meshes,
            &mut materials,
            &format!("models/shop/{}.glb", def.model),
            size,
            rgb(def.color),
        );
        commands
            .entity(body)
            .insert((ChildOf(root), Pickable::IGNORE));
        let hinge = def.lid.as_ref().map(|lid| {
            let hinge = commands
                .spawn((
                    Hinge,
                    Transform::from_xyz(0.0, size.y, -size.z / 2.0),
                    Visibility::default(),
                    ChildOf(root),
                ))
                .id();
            let path = format!("models/shop/{}_lid.glb", def.model);
            let model = model_or_box(
                &mut commands,
                &assets,
                &mut meshes,
                &mut materials,
                &path,
                lid.size.into(),
                rgb(def.color),
            );
            if !asset_exists(&path) {
                // The placeholder cuboid is centred. The painted lids are authored on the hinge.
                commands.entity(model).insert(Transform::from_xyz(
                    0.0,
                    lid.size.1 / 2.0,
                    lid.size.2 / 2.0,
                ));
            }
            commands
                .entity(model)
                .insert((ChildOf(hinge), Pickable::IGNORE));
            (hinge, lid.angle.to_radians())
        });
        let lid_size = def.lid.as_ref().map_or(Vec3::ZERO, |lid| lid.size.into());
        let bounds = Vec3::new(
            size.x.max(lid_size.x),
            size.y + lid_size.y,
            size.z.max(lid_size.z),
        );
        let target = commands
            .spawn((
                SearchTarget,
                Mesh3d(hit.mesh.clone()),
                MeshMaterial3d(hit.material.clone()),
                NotShadowCaster,
                Pickable::IGNORE,
                Visibility::Hidden,
                Transform::from_xyz(0.0, bounds.y / 2.0, 0.0).with_scale(bounds),
                ChildOf(root),
            ))
            .id();
        bind_floor_target(&mut commands, root, cell);
        // Clicks on the painted mesh and on the simpler hit volume bubble to the same pile.
        commands
            .entity(root)
            .insert(Pile {
                cell,
                rim: size.y,
                hinge,
                target,
            })
            .observe(click_pile);
    }
}

fn click_pile(
    mut click: On<PointerClick>,
    piles: Query<&Pile>,
    targets: Query<&Pickable, With<SearchTarget>>,
    tool: Res<Tool>,
    mut actions: MessageWriter<Action>,
) {
    if click.button == PointerButton::Primary
        && *tool == Tool::None
        && let Ok(pile) = piles.get(click.event_target())
        && let Ok(pick) = targets.get(pile.target)
        && pick.is_hoverable
    {
        click.propagate(false);
        actions.write(Action::Rummage(Some(pile.cell)));
    }
}

fn search_targets(
    phase: Res<State<Phase>>,
    location: Res<Location>,
    paused: Res<Paused>,
    tool: Res<Tool>,
    layout: Res<ShopLayout>,
    progress: Res<Progress>,
    lang: Res<Lang>,
    mut tip: ResMut<HoverTip>,
    mut previous: Local<Option<HoverTip>>,
    piles: Query<&Hovered, With<Pile>>,
    mut targets: Query<(&mut Pickable, &mut Visibility), With<SearchTarget>>,
) {
    let editing = *phase.get() == Phase::Evening && *location == Location::Shop && !paused.0;
    let ready = editing
        && *tool == Tool::None
        && progress.rummaged_on != progress.night
        && has_workbench(&layout);
    let target_active = ready || (editing && *tool != Tool::None);
    for (mut pick, mut visible) in &mut targets {
        if pick.is_hoverable != target_active {
            *pick = if target_active {
                Pickable::default()
            } else {
                Pickable::IGNORE
            };
            *visible = if target_active {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
    }
    if ready && piles.iter().any(Hovered::get) {
        if previous.is_none() || lang.is_changed() {
            let mut hint = HoverTip {
                title: lang.t("workbench.rummage"),
                ..default()
            };
            hint.line(TipKind::Hint, lang.t("workbench.hint"));
            *previous = Some(hint);
        }
        if let Some(hint) = previous.as_ref()
            && *tip != *hint
        {
            HoverTip::show(&mut tip, hint.clone());
        }
    } else if let Some(hint) = previous.take()
        && *tip == hint
    {
        HoverTip::hide(&mut tip);
    }
}

#[derive(Component)]
struct Find {
    age: f32,
    rim: f32,
    hinge: Option<(Entity, f32)>,
    item: Entity,
}

const FIND_SECONDS: f32 = 2.9;
const ITEM_RISE: f32 = 0.27;

fn show_find(
    mut finds: MessageReader<Rummaged>,
    mut commands: Commands,
    layout: Res<ShopLayout>,
    progress: Res<Progress>,
    piles: Query<(Entity, &Pile)>,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for found in finds.read() {
        // The panel favours a hinged case near the front. A direct click always uses that pile.
        let pile = match found.cell {
            Some(cell) => piles.iter().find(|(_, pile)| pile.cell == cell),
            None => piles.iter().min_by_key(|(_, pile)| {
                (
                    pile.hinge.is_none(),
                    pile.cell / layout.cols,
                    (pile.cell % layout.cols + progress.night as usize % layout.cols) % layout.cols,
                )
            }),
        };
        let (at, rim, hinge) = match pile {
            Some((_, pile)) => (Transform::default(), pile.rim, pile.hinge),
            // When the whole shop is clear the ferryman can leave a find by the door.
            None => (
                Transform::from_translation(floor(layout.door_pos(), 0.0)),
                0.08,
                None,
            ),
        };
        let root = commands
            .spawn((
                at,
                Visibility::default(),
                Pickable::IGNORE,
                DespawnOnExit(Phase::Evening),
            ))
            .id();
        if let Some((parent, _)) = pile {
            commands.entity(root).insert(ChildOf(parent));
        }
        let item = commands
            .spawn((
                Transform::default(),
                Visibility::Hidden,
                Pickable::IGNORE,
                ChildOf(root),
            ))
            .id();
        let model = product_model(
            &mut commands,
            &assets,
            &mut meshes,
            &mut materials,
            &found.product,
            Vec3::ZERO,
        );
        commands
            .entity(model)
            .insert((ChildOf(item), Pickable::IGNORE));
        commands.entity(root).insert(Find {
            age: 0.0,
            rim,
            hinge,
            item,
        });
    }
}

fn ease(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn animate_find(
    mut commands: Commands,
    time: Res<Time>,
    settings: Res<Settings>,
    mut finds: Query<(Entity, &mut Find)>,
    mut transforms: Query<&mut Transform>,
    mut visibility: Query<&mut Visibility>,
) {
    for (entity, mut find) in &mut finds {
        find.age += time.delta_secs();
        let finished = find.age >= FIND_SECONDS;
        let (open, rise) = if finished {
            (0.0, 0.0)
        } else if settings.reduced_motion {
            (1.0, 1.0)
        } else {
            (
                ease(find.age / 0.46) * ease((FIND_SECONDS - find.age) / 0.47),
                ease((find.age - 0.22) / 0.60) * ease((FIND_SECONDS - 0.40 - find.age) / 0.45),
            )
        };
        if let Some((hinge, angle)) = find.hinge
            && let Ok(mut at) = transforms.get_mut(hinge)
        {
            at.rotation = Quat::from_rotation_x(-angle * open);
        }
        if finished {
            commands.entity(entity).despawn();
            continue;
        }
        if let Ok(mut at) = transforms.get_mut(find.item) {
            at.translation = Vec3::Y * (find.rim - 0.13 + (ITEM_RISE + 0.13) * rise);
            at.rotation = Quat::from_rotation_y(-0.2 + 0.45 * rise);
            at.scale = Vec3::splat(0.55 + 0.67 * rise);
        }
        if let Ok(mut vis) = visibility.get_mut(find.item) {
            *vis = if rise > 0.18 {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    #[test]
    fn the_starting_floor_contains_every_silhouette_without_identical_neighbours() {
        let data = GameData::load();
        let layout = ShopLayout::new(data.config.grid_cols, data.config.grid_rows);
        let count = data.workbench.piles.len();
        let mut on_floor = std::collections::BTreeSet::new();
        let upgrades = Upgrades::default();
        for cell in 0..layout.cells.len() {
            if upgrades.usable(&data, &layout, cell) {
                continue;
            }
            let (variant, yaw) = arrangement(cell, layout.cols, count);
            on_floor.insert(variant);
            assert!(yaw.abs() < 0.21);
            if cell % layout.cols > 0 {
                assert_ne!(variant, arrangement(cell - 1, layout.cols, count).0);
            }
            if cell >= layout.cols {
                assert_ne!(
                    variant,
                    arrangement(cell - layout.cols, layout.cols, count).0
                );
            }
        }
        assert_eq!(on_floor.len(), count);
    }

    #[test]
    fn search_uses_the_selected_case_and_records_the_find_once_even_in_reduced_motion() {
        use bevy::camera::NormalizedRenderTarget;
        use bevy::picking::{
            backend::HitData,
            events::Pointer,
            pointer::{Location as PointerLocation, PointerId},
        };
        use bevy::window::WindowRef;
        let data = GameData::load();
        let mut layout = ShopLayout::new(data.config.grid_cols, data.config.grid_rows);
        layout.cells[0].decor = Some(crate::workbench::WORKBENCH.into());
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<WorldAsset>()
            .init_asset::<Mesh>()
            .init_asset::<Image>()
            .init_asset::<StandardMaterial>()
            .init_resource::<HitVolume>()
            .init_resource::<Upgrades>()
            .init_resource::<Tool>()
            .init_resource::<Location>()
            .init_resource::<Paused>()
            .init_resource::<HoverTip>()
            .insert_resource(State::new(Phase::Evening))
            .init_resource::<crate::economy::Drawer>()
            .init_resource::<crate::inventory::Stock>()
            .init_resource::<crate::accounts::Accounts>()
            .init_resource::<crate::ui::GameLog>()
            .insert_resource(Settings {
                reduced_motion: true,
                ..default()
            })
            .insert_resource(Progress::new(0))
            .insert_resource(crate::rng::Rng::new(3))
            .insert_resource(Lang::load("en"))
            .insert_resource(layout)
            .insert_resource(data)
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                160,
            )))
            .add_message::<Action>()
            .add_message::<Rummaged>()
            .add_systems(
                Update,
                (
                    sync_piles,
                    search_targets,
                    use_workbench,
                    show_find,
                    animate_find,
                )
                    .chain(),
            );
        app.update();
        let world = app.world_mut();
        let (chest, hinge, opening) = world
            .query::<(Entity, &Pile)>()
            .iter(world)
            .filter_map(|(e, p)| p.hinge.map(|(h, a)| (e, h, a)))
            .next()
            .unwrap();
        world.write_message(Action::Rummage(Some(0)));
        app.update();
        assert_eq!(app.world().resource::<Progress>().rummaged_on, 0);
        // The painted lid can sit in front of the hit volume. Its click must reach the case.
        let world = app.world_mut();
        let window = world.spawn(Window::default()).id();
        let lid = world.get::<Children>(hinge).unwrap()[0];
        world.trigger(PointerClick {
            entity: lid,
            pointer: Pointer::new(
                PointerId::Mouse,
                PointerLocation {
                    target: NormalizedRenderTarget::Window(
                        WindowRef::Entity(window).normalize(Some(window)).unwrap(),
                    ),
                    position: Vec2::ZERO,
                },
            ),
            button: PointerButton::Primary,
            hit: HitData::new(window, 0.0, None, None),
            duration: Default::default(),
            count: 1,
        });
        app.update();
        assert_eq!(app.world().resource::<Progress>().broken.len(), 1);
        app.world_mut().write_message(Action::Rummage(None));
        app.update();
        let world = app.world_mut();
        assert_eq!(world.resource::<Progress>().broken.len(), 1);
        let (find, parent) = world.query::<(&Find, &ChildOf)>().single(world).unwrap();
        assert_eq!(parent.parent(), chest);
        let item = find.item;
        let pose = *world.get::<Transform>(item).unwrap();
        assert_eq!(
            *world.get::<Visibility>(item).unwrap(),
            Visibility::Inherited
        );
        assert_eq!(
            world.get::<Transform>(hinge).unwrap().rotation,
            Quat::from_rotation_x(-opening)
        );
        app.update();
        assert_eq!(app.world().get::<Transform>(item), Some(&pose));
        for _ in 0..20 {
            app.update();
        }
        assert!(app.world().get_entity(item).is_err());
        assert_eq!(
            app.world().get::<Transform>(hinge).unwrap().rotation,
            Quat::IDENTITY
        );
        // Another improvement keeps the existing pile, including its hinge.
        app.world_mut()
            .resource_mut::<Upgrades>()
            .0
            .insert("waiting_bench".into());
        app.update();
        assert!(app.world().get::<Pile>(chest).is_some());
        // Once the carpenter clears the entire floor the hit volumes go with it.
        app.world_mut()
            .resource_mut::<Upgrades>()
            .0
            .extend(["clear_corner", "back_room", "knock_through"].map(String::from));
        app.update();
        let world = app.world_mut();
        assert_eq!(world.query::<&SearchTarget>().iter(world).count(), 0);
        assert!(world.get_entity(hinge).is_err());
    }
}
