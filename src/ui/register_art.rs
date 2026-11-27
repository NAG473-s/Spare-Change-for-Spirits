//! The ware, amount and three pockets on the register. The text and controls still carry every count.
use super::{Action, CountedLine, Icons, PanelBuilder, TEXT, TEXT_DIM, counted_line};
use crate::economy::Coin;
use crate::i18n::Lang;
use crate::register::{Register, Sale};
use crate::save::Settings;
use bevy::prelude::*;

const TRAY_W: f32 = 128.0;
const TRAY_H: f32 = 48.0;
const TOKEN: f32 = 22.0;
const MOVE_TIME: f32 = 0.32;
const LAYERS: usize = 3;

pub(super) const TRAY_ART: &str = "sprites/ui/change_tray.png";

pub(super) fn receipt(ui: &mut PanelBuilder, icons: &Icons, sale: &Sale, lang: &Lang) {
    // The product portrait shares a line with the two existing receipt fields.
    let card = ui.node(
        ui.root(),
        (
            Node {
                align_items: AlignItems::Center,
                column_gap: px(9),
                padding: UiRect::axes(px(8), px(4)),
                border: UiRect::left(px(2)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::srgb(0.13, 0.105, 0.12)),
            BorderColor::all(Color::srgb(0.67, 0.49, 0.29)),
            Pickable::IGNORE,
        ),
    );
    if let Some(icon) = icons.product(&sale.product) {
        ui.image_sized(card, icon, Vec2::splat(38.0));
    }
    let lines = ui.node(
        card,
        (
            Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                min_width: px(0),
                ..default()
            },
            Pickable::IGNORE,
        ),
    );
    ui.add_text(
        lines,
        &lang.f("night.item", &[("item", &lang.product(&sale.product))]),
        true,
        15.0,
        TEXT,
    );
    ui.add_text(
        lines,
        &lang.f("night.price", &[("price", &sale.price)]),
        false,
        14.0,
        TEXT_DIM,
    );
}

#[derive(Component)]
pub(super) struct Pocket {
    coin: Coin,
    count: u32,
    still: [Entity; LAYERS],
    moving: Entity,
    badge: Entity,
    stroke: Option<Stroke>,
}

struct Stroke {
    adding: bool,
    top: usize,
    age: f32,
}

fn visible_count(count: u32) -> usize {
    count.min(LAYERS as u32) as usize
}

fn slot(layer: usize) -> Vec2 {
    Vec2::new(8.0 + layer as f32 * 2.6, 15.0 - layer as f32 * 2.2)
}

fn token(
    ui: &mut PanelBuilder,
    pocket: Entity,
    image: &Handle<Image>,
    layer: usize,
    visible: bool,
) -> Entity {
    let at = slot(layer);
    ui.node(
        pocket,
        (
            Node {
                position_type: PositionType::Absolute,
                width: px(TOKEN),
                height: px(TOKEN),
                left: px(at.x),
                top: px(at.y),
                ..default()
            },
            ImageNode::new(image.clone()),
            if visible {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            },
            Pickable::IGNORE,
        ),
    )
}

pub(super) fn change_tray(
    ui: &mut PanelBuilder,
    parent: Entity,
    icons: &Icons,
    sale: &Sale,
    lang: &Lang,
    show_due: bool,
) {
    let summary = ui.node(
        parent,
        (
            Node {
                width: percent(46),
                min_width: px(TRAY_W),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Start,
                row_gap: px(2),
                ..default()
            },
            Pickable::IGNORE,
        ),
    );
    if show_due {
        ui.add_text(
            summary,
            &lang.f("night.change_due", &[("amount", &sale.change_due())]),
            false,
            13.0,
            TEXT_DIM,
        );
    }
    let line = ui.add_text(summary, &counted_line(lang, sale), true, 14.0, TEXT);
    ui.commands.entity(line).insert(CountedLine);
    let tray = ui.node(
        summary,
        (
            Node {
                width: px(TRAY_W),
                height: px(TRAY_H),
                flex_shrink: 0.0,
                ..default()
            },
            Pickable::IGNORE,
        ),
    );
    if let Some(art) = &icons.tray {
        ui.commands.entity(tray).insert(ImageNode::new(art.clone()));
    } else {
        ui.commands
            .entity(tray)
            .insert(BackgroundColor(Color::srgb(0.19, 0.32, 0.31)));
    }
    for (index, coin) in Coin::ASCENDING.into_iter().enumerate() {
        let Some(image) = icons.coin(coin) else {
            continue;
        };
        let pocket = ui.node(
            tray,
            Node {
                position_type: PositionType::Absolute,
                // Even in a right-to-left catalog the pockets follow the painted dividers.
                left: px(9.0 + 35.9 * index as f32),
                width: px(36),
                height: px(TRAY_H),
                ..default()
            },
        );
        let count = sale.given.get(coin);
        let still = std::array::from_fn(|layer| {
            token(ui, pocket, &image, layer, layer < visible_count(count))
        });
        let moving = token(ui, pocket, &image, 0, false);
        let badge = ui.add_text(pocket, &count.to_string(), true, 11.0, TEXT);
        ui.commands.entity(badge).insert((
            Node {
                position_type: PositionType::Absolute,
                right: px(-1),
                bottom: px(4),
                padding: UiRect::horizontal(px(2)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.12, 0.16, 0.17)),
            if count > LAYERS as u32 {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            },
        ));
        ui.commands
            .entity(pocket)
            .observe(return_from_pocket)
            .insert(Pocket {
                coin,
                count,
                still,
                moving,
                badge,
                stroke: None,
            });
    }
}

fn return_from_pocket(
    mut click: On<PointerClick>,
    pockets: Query<&Pocket>,
    mut actions: MessageWriter<Action>,
) {
    if matches!(
        click.button,
        PointerButton::Primary | PointerButton::Secondary
    ) && let Ok(pocket) = pockets.get(click.entity)
        && pocket.count > 0
    {
        click.propagate(false);
        actions.write(Action::RemoveChange(pocket.coin));
    }
}

/// Animate only a change the register accepted. Rebuilt or restored panels start at the saved count.
pub(super) fn animate(
    time: Res<Time>,
    register: Res<Register>,
    settings: Res<Settings>,
    mut pockets: Query<&mut Pocket>,
    mut pieces: Query<(&mut Node, &mut ImageNode, &mut Visibility), Without<Text>>,
    mut badges: Query<(&mut Text, &mut Visibility), Without<ImageNode>>,
) {
    let Some(sale) = &register.sale else { return };
    for mut pocket in &mut pockets {
        let count = sale.given.get(pocket.coin);
        if count != pocket.count {
            pocket.stroke = Some(Stroke {
                adding: count > pocket.count,
                top: visible_count(pocket.count.max(count)) - 1,
                age: 0.0,
            });
            pocket.count = count;
            if let Ok((mut text, mut visible)) = badges.get_mut(pocket.badge) {
                text.0 = count.to_string();
                *visible = if count > LAYERS as u32 {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
        }
        let Some(stroke) = &mut pocket.stroke else {
            continue;
        };
        stroke.age += time.delta_secs();
        let t = if settings.reduced_motion {
            1.0
        } else {
            (stroke.age / MOVE_TIME).min(1.0)
        };
        let k = t * t * (3.0 - 2.0 * t);
        let (adding, top) = (stroke.adding, stroke.top);
        let shown = visible_count(count).saturating_sub(usize::from(adding && t < 1.0));
        for (layer, piece) in pocket.still.iter().enumerate() {
            if let Ok((_, _, mut vis)) = pieces.get_mut(*piece) {
                *vis = if layer < shown {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
        }
        if let Ok((mut node, mut image, mut vis)) = pieces.get_mut(pocket.moving) {
            if t >= 1.0 {
                *vis = Visibility::Hidden;
            } else {
                let flight = if adding { 1.0 - k } else { k };
                let travel = Vec2::new(if adding { -8.0 } else { 9.0 }, -12.0);
                let at = slot(top) + travel * flight;
                node.left = px(at.x);
                node.top = px(at.y);
                image.color = Color::WHITE.with_alpha(if adding { 1.0 } else { 1.0 - k });
                *vis = Visibility::Inherited;
            }
        }
        if t >= 1.0 {
            pocket.stroke = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    #[test]
    fn pocket_counts_survive_quick_undo_and_settle_when_motion_is_disabled() {
        let mut app = super::super::tests::register_panel();
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            80,
        )));
        let world = app.world_mut();
        let pocket = world
            .query::<&Pocket>()
            .iter(world)
            .find(|p| p.coin == Coin::Candle)
            .unwrap();
        let (tokens, motion, badge) = (pocket.still, pocket.moving, pocket.badge);
        world.resource_mut::<crate::economy::Drawer>().0.candles = 7;
        world
            .resource_mut::<Register>()
            .sale
            .as_mut()
            .unwrap()
            .given
            .candles = 5;
        app.update();
        let shown =
            |world: &World, e| *world.get::<Visibility>(e).unwrap() == Visibility::Inherited;
        assert!(shown(app.world(), motion) && shown(app.world(), badge));
        assert_eq!(app.world().get::<Text>(badge).unwrap().0, "5");
        for _ in 0..5 {
            app.update();
        }
        assert!(tokens.iter().all(|e| shown(app.world(), *e)) && !shown(app.world(), motion));
        app.world_mut()
            .resource_mut::<Register>()
            .sale
            .as_mut()
            .unwrap()
            .given
            .candles = 2;
        app.update();
        assert!(
            shown(app.world(), motion)
                && !shown(app.world(), tokens[2])
                && !shown(app.world(), badge)
        );
        // Turning the preference on midway through the return settles it immediately.
        app.world_mut().resource_mut::<Settings>().reduced_motion = true;
        app.update();
        // The preferences rebuild the panel. Read the new pocket at its settled count.
        let world = app.world_mut();
        let pocket = world
            .query::<&Pocket>()
            .iter(world)
            .find(|p| p.coin == Coin::Candle)
            .unwrap();
        let (tokens, motion) = (pocket.still, pocket.moving);
        assert!(!shown(world, motion) && shown(world, tokens[1]) && !shown(world, tokens[2]));
        world
            .resource_mut::<Register>()
            .sale
            .as_mut()
            .unwrap()
            .given
            .candles = 0;
        app.update();
        assert!(!shown(app.world(), motion) && tokens.iter().all(|e| !shown(app.world(), *e)));
    }
}
