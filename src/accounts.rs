//! The day's accounts: what went into and out of the cash drawer between
//! dusk and dawn, shown as a ledger in the morning together with how the
//! night went.

use bevy::prelude::*;

use crate::data::{GameData, asset_exists};
use crate::day_cycle::{NightReport, Phase, Progress, start_morning};
use crate::economy::Drawer;
use crate::ferry::signed;
use crate::ghosts::Ghost;
use crate::i18n::Lang;
use crate::inventory::Stock;
use crate::ui::{Action, Icons, PanelBuilder, TEXT, TEXT_DIM, UiFont, overlay};

pub struct AccountsPlugin;

impl Plugin for AccountsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Accounts>()
            .add_systems(OnEnter(Phase::Evening), open_books)
            .add_systems(
                OnEnter(Phase::Night),
                |drawer: Res<Drawer>, mut a: ResMut<Accounts>| {
                    a.night_start = drawer.total();
                },
            )
            .add_systems(
                OnEnter(Phase::Morning),
                (close_books, build_ledger).chain().after(start_morning),
            )
            .add_systems(
                Update,
                (
                    count_visitors.run_if(in_state(Phase::Night)),
                    build_ledger
                        .run_if(in_state(Phase::Morning).and_then(resource_changed::<UiFont>)),
                ),
            );
    }
}

/// Money in and out of the drawer for the current day (dusk to dawn).
#[derive(Resource, Default, Debug, Clone, PartialEq)]
pub struct Accounts {
    /// In the drawer when the evening began.
    pub opening: u32,
    /// Paid to the ferryman (crates given back are subtracted again).
    pub ferryman: u32,
    /// Paid for furniture.
    pub furniture: u32,
    /// In the drawer when the shop opened.
    pub night_start: u32,
    /// What the night brought in: the drawer at dawn (before counterfeits
    /// turned to dust) minus the drawer at opening time.
    pub takings: i64,
    /// Value of counterfeit coins that turned to dust at dawn.
    pub dust: u32,
    /// In the drawer now, at dawn.
    pub closing: u32,
}

impl Accounts {
    /// Profit (or loss, if negative) of the whole day.
    pub fn profit(&self) -> i64 {
        self.closing as i64 - self.opening as i64
    }

    /// Fills in the night's figures from the drawer at dawn.
    pub fn close(&mut self, closing: u32, dust: u32) {
        self.closing = closing;
        self.dust = dust;
        self.takings = (closing + dust) as i64 - self.night_start as i64;
    }
}

#[derive(Component)]
struct Ledger;

fn open_books(drawer: Res<Drawer>, mut accounts: ResMut<Accounts>) {
    *accounts = Accounts {
        opening: drawer.total(),
        night_start: drawer.total(),
        ..default()
    };
}

/// Runs after `start_morning` has removed the counterfeits from the drawer.
fn close_books(drawer: Res<Drawer>, report: Res<NightReport>, mut accounts: ResMut<Accounts>) {
    accounts.close(drawer.total(), report.counterfeits_found.total());
}

fn count_visitors(added: Query<(), Added<Ghost>>, mut report: ResMut<NightReport>) {
    let n = added.iter().count() as u32;
    if n > 0 {
        report.visitors += n;
    }
}

const GREEN: Color = Color::srgb(0.55, 0.85, 0.6);
const RED: Color = Color::srgb(0.95, 0.5, 0.45);
const GOLD: Color = Color::srgb(0.95, 0.8, 0.45);
const LEFT_WIDTH: f32 = 330.0;
const AMOUNT_WIDTH: f32 = 70.0;

fn build_ledger(
    mut commands: Commands,
    font: Res<UiFont>,
    lang: Res<Lang>,
    data: Res<GameData>,
    icons: Res<Icons>,
    accounts: Res<Accounts>,
    report: Res<NightReport>,
    progress: Res<Progress>,
    stock: Res<Stock>,
    asset_server: Res<AssetServer>,
    existing: Query<Entity, With<Ledger>>,
) {
    for e in &existing {
        commands.entity(e).despawn();
    }
    let (layer, panel) = overlay(&mut commands, 0.55, 860.0);
    commands
        .entity(layer)
        .insert((Ledger, DespawnOnExit(Phase::Morning)));
    let mut ui = PanelBuilder::new(&mut commands, panel, &font);
    let root = ui.root();
    ui.heading(&lang.f("accounts.title", &[("night", &progress.night)]));

    let columns = ui.node(
        root,
        Node {
            column_gap: px(28),
            min_height: px(0),
            flex_shrink: 1.0,
            ..default()
        },
    );

    // Left: the cash drawer, line by line.
    let left = ui.node(
        columns,
        Node {
            width: px(LEFT_WIDTH),
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Column,
            row_gap: px(5),
            ..default()
        },
    );
    ui.add_text(left, &lang.t("accounts.cash"), true, 15.0, TEXT);
    let line = |ui: &mut PanelBuilder, label: &str, amount: String, color: Color, indent: bool| {
        let row = ui.node(
            left,
            Node {
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::left(px(if indent { 16.0 } else { 0.0 })),
                ..default()
            },
        );
        let size = if indent { 12.0 } else { 14.0 };
        let label_color = if indent { TEXT_DIM } else { TEXT };
        ui.add_text(row, label, false, size, label_color);
        ui.cell(row, &amount, AMOUNT_WIDTH, JustifyContent::End, color);
    };
    let a = &*accounts;
    line(
        &mut ui,
        &lang.t("accounts.opening"),
        a.opening.to_string(),
        TEXT,
        false,
    );
    line(
        &mut ui,
        &lang.t("accounts.ferryman"),
        signed(-(a.ferryman as i64)),
        money(-(a.ferryman as i64)),
        false,
    );
    line(
        &mut ui,
        &lang.t("accounts.furniture"),
        signed(-(a.furniture as i64)),
        money(-(a.furniture as i64)),
        false,
    );
    line(
        &mut ui,
        &lang.t("accounts.takings"),
        signed(a.takings),
        money(a.takings),
        false,
    );
    line(
        &mut ui,
        &lang.f("accounts.sales", &[("count", &report.sales)]),
        signed(report.revenue as i64),
        TEXT_DIM,
        true,
    );
    if report.change_kept != 0 {
        line(
            &mut ui,
            &lang.t("accounts.change"),
            signed(report.change_kept),
            TEXT_DIM,
            true,
        );
    }
    if report.second_payments_kept > 0 {
        line(
            &mut ui,
            &lang.t("accounts.kept_payments"),
            signed(report.second_payments_kept as i64),
            TEXT_DIM,
            true,
        );
    }
    if a.dust > 0 {
        line(
            &mut ui,
            &lang.t("accounts.dust"),
            signed(-(a.dust as i64)),
            RED,
            false,
        );
    }
    ui.node(
        left,
        Node {
            height: px(1),
            margin: UiRect::vertical(px(3)),
            ..default()
        },
    );
    let rule = ui.node(
        left,
        Node {
            height: px(1),
            ..default()
        },
    );
    ui.tint(rule, Color::srgba(1.0, 1.0, 1.0, 0.25));
    line(
        &mut ui,
        &lang.t("accounts.closing"),
        a.closing.to_string(),
        GOLD,
        false,
    );
    let profit = a.profit();
    let key = if profit < 0 {
        "accounts.loss"
    } else {
        "accounts.profit"
    };
    line(&mut ui, &lang.t(key), signed(profit), money(profit), false);

    // Right: how the night went.
    let right = ui.node(
        columns,
        Node {
            flex_grow: 1.0,
            flex_shrink: 1.0,
            min_height: px(0),
            flex_direction: FlexDirection::Column,
            row_gap: px(5),
            overflow: Overflow::scroll_y(),
            ..default()
        },
    );
    ui.add_text(right, &lang.t("accounts.night"), true, 15.0, TEXT);
    let text = |ui: &mut PanelBuilder, s: String| ui.add_text(right, &s, false, 13.0, TEXT);
    text(
        &mut ui,
        lang.f(
            "accounts.customers",
            &[
                ("visitors", &report.visitors),
                ("served", &report.sales),
                ("fled", &report.fled),
                ("unhappy", &report.unhappy),
            ],
        ),
    );
    text(
        &mut ui,
        lang.f(
            "morning.change",
            &[
                ("correct", &report.change_correct),
                ("wrong", &report.change_wrong),
            ],
        ),
    );
    text(
        &mut ui,
        lang.f(
            "morning.reputation",
            &[
                ("from", &report.reputation_start),
                ("to", &progress.reputation),
            ],
        ),
    );
    if report.honest + report.dishonest > 0 {
        text(
            &mut ui,
            lang.f(
                "morning.second_payments",
                &[("refused", &report.honest), ("kept", &report.dishonest)],
            ),
        );
    }
    if report.counterfeits_caught > 0 {
        text(
            &mut ui,
            lang.f(
                "morning.counterfeits_spotted",
                &[("count", &report.counterfeits_caught)],
            ),
        );
    }
    if let Some(line) = &report.story_line {
        ui.add_text(right, &lang.t("story.name"), true, 13.0, TEXT);
        ui.add_text(
            right,
            &lang.f("quote", &[("text", &lang.t(line))]),
            false,
            12.0,
            TEXT_DIM,
        );
    }
    if !report.keepsakes.is_empty() {
        let names: Vec<String> = report.keepsakes.iter().map(|k| lang.decor(k)).collect();
        text(
            &mut ui,
            lang.f("morning.keepsake", &[("items", &lang.join(&names))]),
        );
    }

    // Chips: a small picture and a label.
    let chips =
        |ui: &mut PanelBuilder, title: &str, items: Vec<(Option<Handle<Image>>, Vec2, String)>| {
            if items.is_empty() {
                return;
            }
            ui.add_text(right, title, true, 13.0, TEXT);
            let row = ui.node(
                right,
                Node {
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: px(6),
                    row_gap: px(4),
                    ..default()
                },
            );
            for (image, size, label) in items {
                let chip = ui.node(
                    row,
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: px(4),
                        padding: UiRect::new(px(3), px(7), px(2), px(2)),
                        border_radius: BorderRadius::all(px(4)),
                        ..default()
                    },
                );
                ui.tint(chip, Color::srgba(1.0, 1.0, 1.0, 0.06));
                if let Some(image) = image {
                    ui.image_sized(chip, image, size);
                }
                ui.add_text(chip, &label, false, 12.0, TEXT);
            }
        };
    let icon = Vec2::splat(22.0);
    let product_chips = |ids: &[String]| -> Vec<(Option<Handle<Image>>, Vec2, String)> {
        let mut counts: Vec<(String, u32)> = Vec::new();
        for id in ids {
            match counts.iter_mut().find(|(i, _)| i == id) {
                Some((_, n)) => *n += 1,
                None => counts.push((id.clone(), 1)),
            }
        }
        counts
            .into_iter()
            .map(|(id, n)| {
                let label = lang.f(
                    "count.entry",
                    &[("count", &n), ("item", &lang.product(&id))],
                );
                (icons.product(&id), icon, label)
            })
            .collect()
    };
    chips(
        &mut ui,
        &lang.t("accounts.sold"),
        product_chips(&report.sold),
    );
    let ghosts: Vec<_> = report
        .redeemed
        .iter()
        .map(|g| {
            let path = format!("sprites/ghosts/{g}.png");
            let image = asset_exists(&path).then(|| asset_server.load(path));
            (image, Vec2::new(18.0, 27.0), lang.ghost(g))
        })
        .collect();
    chips(&mut ui, &lang.t("morning.redeemed"), ghosts);
    chips(
        &mut ui,
        &lang.t("morning.knocked_title"),
        product_chips(&report.knocked),
    );
    let faded: Vec<String> = report
        .faded
        .iter()
        .flat_map(|(id, n)| std::iter::repeat_n(id.clone(), *n as usize))
        .collect();
    chips(&mut ui, &lang.t("morning.faded"), product_chips(&faded));
    chips(
        &mut ui,
        &lang.t("accounts.fading_soon"),
        product_chips(&fading_next_dawn(&stock, &data)),
    );

    // Footer.
    let footer = ui.node(
        root,
        Node {
            justify_content: JustifyContent::End,
            flex_shrink: 0.0,
            margin: UiRect::top(px(6)),
            ..default()
        },
    );
    let mut ui = ui.with_footer(footer);
    let last = progress.night >= data.config.nights_in_demo;
    let label = if last {
        "morning.to_end"
    } else {
        "morning.next"
    };
    ui.primary(&lang.t(label), Action::NextEvening);
}

fn money(n: i64) -> Color {
    match n {
        n if n > 0 => GREEN,
        n if n < 0 => RED,
        _ => TEXT,
    }
}

/// Units that will fade at the next dawn if they are not sold tonight.
pub fn fading_next_dawn(stock: &Stock, data: &GameData) -> Vec<String> {
    stock
        .units()
        .iter()
        .flat_map(|(id, ages)| {
            let limit = data.product(id).map_or(u32::MAX, |p| p.fade_after);
            let n = ages.iter().filter(|age| **age + 1 >= limit).count();
            std::iter::repeat_n(id.clone(), n)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn the_books_balance() {
        // 100 at dusk, 20 to the ferryman, 8 for a candle: 72 when opening.
        let mut a = Accounts {
            opening: 100,
            ferryman: 20,
            furniture: 8,
            night_start: 72,
            ..default()
        };
        // Dawn: 95 in the drawer after a counterfeit worth 5 turned to dust.
        a.close(95, 5);
        assert_eq!(a.takings, 28);
        assert_eq!(
            a.opening as i64 - a.ferryman as i64 - a.furniture as i64 + a.takings - a.dust as i64,
            a.closing as i64
        );
        assert_eq!(a.profit(), -5);
    }

    #[test]
    fn units_about_to_fade_are_listed() {
        let data = GameData::load();
        let p = data.product("lavender").expect("lavender exists");
        let stock = Stock::from_units(BTreeMap::from([(
            "lavender".to_string(),
            vec![p.fade_after - 1, 0, p.fade_after - 1],
        )]));
        assert_eq!(fading_next_dawn(&stock, &data).len(), 2);
    }
}
