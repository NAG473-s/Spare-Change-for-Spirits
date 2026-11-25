//! The day's accounts: what went into and out of the cash drawer between
//! dusk and dawn, shown as a ledger in the morning together with how the
//! night went.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::data::{GameData, asset_exists};
use crate::day_cycle::{NightReport, Phase, Progress};
use crate::economy::Drawer;
use crate::ferry::signed;
use crate::ghosts::Ghost;
use crate::i18n::Lang;
use crate::inventory::Stock;
use crate::loan::{Loan, repay};
use crate::ui::{Action, Icons, PanelBuilder, UiFont, UiTheme, overlay};
use crate::upgrades::Upgrades;

pub struct AccountsPlugin;

impl Plugin for AccountsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Accounts>()
            .add_systems(OnEnter(Phase::Day), open_books)
            .add_systems(OnEnter(Phase::Evening), open_books)
            .add_systems(OnEnter(Phase::Night), open_register)
            .add_systems(
                OnEnter(Phase::Morning),
                (
                    close_books,
                    // After a finale, the scene comes first (`finale.rs`).
                    build_ledger
                        .run_if(crate::finale::no_scene.and_then(crate::ending::no_sequence)),
                )
                    .chain()
                    .after(repay)
                    .in_set(LedgerSystems),
            )
            .add_systems(
                Update,
                (
                    count_visitors.run_if(in_state(Phase::Night)),
                    build_ledger.run_if(
                        in_state(Phase::Morning)
                            .and_then(
                                resource_changed::<UiFont>
                                    .or_else(resource_changed::<crate::finale::FinaleScene>)
                                    .or_else(resource_changed::<crate::ending::Ending>),
                            )
                            .and_then(crate::finale::no_scene)
                            .and_then(crate::ending::no_sequence),
                    ),
                ),
            );
    }
}

/// The dawn systems that close the books and show the ledger.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct LedgerSystems;

/// Money in and out of the drawer for the current day (dusk to dawn).
#[derive(Resource, Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Accounts {
    /// The night these books are for (they open at the market, or in the
    /// evening when there is no market that day).
    pub night: u32,
    /// In the drawer when the day began.
    pub opening: u32,
    /// Lent by the ferryman this evening (the bankruptcy rule).
    pub borrowed: u32,
    /// Paid back to the ferryman at dawn.
    pub repaid: u32,
    /// Paid to the ferryman (crates given back are subtracted again).
    pub ferryman: u32,
    /// Paid for furniture at the market (minus what was sold back).
    pub furniture: u32,
    /// Other market spending: single wares (the Reaper's pieces, the winter
    /// and spring traders' goods), the hourglass, the raven's change fees.
    pub market: u32,
    /// Shop improvements bought at the market.
    pub upgrades: u32,
    /// Spent repairing broken wares in the evening.
    pub repairs: u32,
    /// Coins given at the counter for a ghost's crossing (the ferryman's
    /// fare): paid out of the drawer at night, but not part of the takings.
    pub fares: u32,
    /// In the drawer when the shop opened.
    pub night_start: u32,
    /// What the night brought in: the drawer at dawn (before counterfeits
    /// turned to dust) minus the drawer at opening time.
    pub takings: i64,
    /// Value of counterfeit coins that turned to dust at dawn.
    pub dust: u32,
    /// In the drawer now, at dawn.
    pub closing: u32,
    /// Wares in stock, valued at what they cost, when the day began and at
    /// dawn (after fading): crates bought turn coins into wares, so the
    /// first night's purchases show as an investment, not only a loss.
    pub wares_opening: u32,
    pub wares_closing: u32,
}

impl Accounts {
    /// Profit (or loss, if negative) of the whole day.
    pub fn profit(&self) -> i64 {
        self.closing as i64 - self.opening as i64
    }

    /// The profit plus what the wares in stock gained (or lost) in value.
    pub fn result_with_wares(&self) -> i64 {
        self.profit() + self.wares_closing as i64 - self.wares_opening as i64
    }

    /// Fills in the night's figures from the drawer at dawn (after the
    /// counterfeits turned to dust and the ferryman was paid back).
    pub fn close(&mut self, closing: u32, dust: u32) {
        self.closing = closing;
        self.dust = dust;
        self.takings =
            i64::from(closing) + i64::from(dust) + i64::from(self.repaid) + i64::from(self.fares)
                - i64::from(self.night_start);
    }
}

#[derive(Component)]
struct Ledger;

/// Opens the day's books once: at the market, or in the evening when the
/// day had no market (the first one, or a game continued in the evening).
pub(crate) fn open_books(
    drawer: Res<Drawer>,
    progress: Res<Progress>,
    stock: Res<Stock>,
    data: Res<GameData>,
    mut accounts: ResMut<Accounts>,
) {
    if accounts.night == progress.night {
        return;
    }
    *accounts = Accounts {
        night: progress.night,
        opening: drawer.total(),
        night_start: drawer.total(),
        wares_opening: wares_value(&stock, &data),
        ..default()
    };
}

/// What the wares in stock cost: each unit at its crate's price divided by
/// the units in the crate.
pub fn wares_value(stock: &Stock, data: &GameData) -> u32 {
    let value: f32 = stock
        .units()
        .iter()
        .filter_map(|(id, ages)| {
            let p = data.product(id)?;
            Some(p.crate_cost as f32 / p.crate_size.max(1) as f32 * ages.len() as f32)
        })
        .sum();
    value.round() as u32
}

pub(crate) fn open_register(
    drawer: Res<Drawer>,
    mut accounts: ResMut<Accounts>,
    progress: Res<Progress>,
) {
    if !progress
        .journal
        .first_sale_checkpoint
        .as_ref()
        .is_some_and(|c| c.settled_report().is_some())
    {
        accounts.night_start = drawer.total();
    }
}

/// Runs after `start_morning` has removed the counterfeits from the drawer
/// and the ferryman has taken back what was owed.
pub(crate) fn close_books(
    drawer: Res<Drawer>,
    report: Res<NightReport>,
    stock: Res<Stock>,
    data: Res<GameData>,
    mut accounts: ResMut<Accounts>,
) {
    accounts.close(drawer.total(), report.counterfeits_found.total());
    accounts.wares_closing = wares_value(&stock, &data);
}

fn count_visitors(added: Query<(), Added<Ghost>>, mut report: ResMut<NightReport>) {
    let n = added.iter().count() as u32;
    if n > 0 {
        report.visitors += n;
    }
}

const TEXT: Color = Color::srgb(0.22, 0.14, 0.12);
const TEXT_DIM: Color = Color::srgb(0.34, 0.24, 0.19);
const GREEN: Color = Color::srgb(0.16, 0.35, 0.23);
const RED: Color = Color::srgb(0.55, 0.18, 0.20);
const RULE: Color = Color::srgba(0.46, 0.26, 0.13, 0.32);
const AMOUNT_WIDTH: f32 = 70.0;

fn paper(left: bool) -> BackgroundGradient {
    let (start, end) = if left {
        (Color::srgb_u8(205, 175, 130), Color::srgb_u8(181, 147, 111))
    } else {
        (Color::srgb_u8(181, 147, 111), Color::srgb_u8(205, 175, 130))
    };
    let cream = Color::srgb_u8(233, 211, 171);
    BackgroundGradient(vec![
        LinearGradient::to_right(vec![
            ColorStop::new(start, percent(0)),
            ColorStop::new(cream, percent(16)),
            ColorStop::new(cream, percent(84)),
            ColorStop::new(end, percent(100)),
        ])
        .into(),
    ])
}

/// The ledger keeps the side panel beside it only with this much room.
const LEDGER_MIN_WIDTH: f32 = 600.0;

fn build_ledger(
    mut commands: Commands,
    font: Res<UiFont>,
    theme: Res<UiTheme>,
    lang: Res<Lang>,
    data: Res<GameData>,
    icons: Res<Icons>,
    accounts: Res<Accounts>,
    report: Res<NightReport>,
    progress: Res<Progress>,
    stock: Res<Stock>,
    loan: Res<Loan>,
    upgrades: Res<Upgrades>,
    asset_server: Res<AssetServer>,
    existing: Query<Entity, With<Ledger>>,
    (window, ui_scale): (
        Single<&Window, With<bevy::window::PrimaryWindow>>,
        Res<UiScale>,
    ),
) {
    for e in &existing {
        commands.entity(e).despawn();
    }
    // A large interface size on a narrow screen leaves too little room
    // beside the side panel: the ledger then covers the whole width and its
    // two pages stand one above the other.
    let free = window.width() / ui_scale.0 - crate::ui::PANEL_WIDTH;
    let narrow = free < LEDGER_MIN_WIDTH;
    let (layer, panel) = overlay(&mut commands, &theme, 0.55, 860.0);
    commands
        .entity(layer)
        .insert((Ledger, DespawnOnExit(Phase::Morning), Pickable::IGNORE))
        // Centred over the shop, leaving the side panel (tips, tomorrow)
        // and the log beside it readable.
        .entry::<Node>()
        .and_modify(move |mut node| {
            if !narrow {
                node.right = px(crate::ui::PANEL_WIDTH);
            }
        });
    let mut ui = PanelBuilder::new(&mut commands, panel, &font);
    let root = ui.root();
    ui.heading(&lang.f("accounts.title", &[("night", &progress.night)]));

    let columns = ui.node(
        root,
        Node {
            column_gap: px(5),
            row_gap: px(5),
            flex_direction: if narrow {
                FlexDirection::Column
            } else {
                FlexDirection::Row
            },
            min_height: px(0),
            flex_shrink: if narrow { 0.0 } else { 1.0 },
            padding: UiRect::all(px(3)),
            border_radius: BorderRadius::all(px(7)),
            ..default()
        },
    );
    ui.tint(columns, Color::srgb_u8(79, 46, 39));

    // Left: the cash drawer, line by line.
    let left = ui.node(
        columns,
        (
            Node {
                width: percent(if narrow { 100.0 } else { 45.0 }),
                min_height: px(0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                row_gap: px(4),
                padding: UiRect::all(px(14)),
                border_radius: BorderRadius::all(px(4)),
                overflow: if narrow {
                    Overflow::visible()
                } else {
                    Overflow::scroll_y()
                },
                overflow_clip_margin: OverflowClipMargin::content_box(),
                ..default()
            },
            paper(true),
            ScrollPosition::default(),
        ),
    );
    ui.display_text(left, &lang.t("accounts.cash"), 18.0, TEXT);
    let line = |ui: &mut PanelBuilder, label: &str, amount: String, color: Color, indent: bool| {
        let row = ui.node(
            left,
            (
                Node {
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::left(px(if indent { 12.0 } else { 0.0 })),
                    border: UiRect::bottom(px(1)),
                    ..default()
                },
                BorderColor::all(RULE),
            ),
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
    if a.borrowed > 0 {
        line(
            &mut ui,
            &lang.t("accounts.borrowed"),
            signed(a.borrowed as i64),
            money(a.borrowed as i64),
            false,
        );
    }
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
    if a.upgrades > 0 {
        line(
            &mut ui,
            &lang.t("accounts.upgrades"),
            signed(-(a.upgrades as i64)),
            money(-(a.upgrades as i64)),
            false,
        );
    }
    if a.market > 0 {
        line(
            &mut ui,
            &lang.t("accounts.market"),
            signed(-(a.market as i64)),
            money(-(a.market as i64)),
            false,
        );
    }
    if a.repairs > 0 {
        let spent = -i64::from(a.repairs);
        line(
            &mut ui,
            &lang.decor(crate::workbench::WORKBENCH),
            signed(spent),
            money(spent),
            false,
        );
    }
    if a.fares > 0 {
        let given = -i64::from(a.fares);
        line(
            &mut ui,
            &lang.t("accounts.fare"),
            signed(given),
            money(given),
            false,
        );
    }
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
    if report.tips > 0 {
        line(
            &mut ui,
            &lang.t("accounts.tips"),
            signed(report.tips as i64),
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
    if a.repaid > 0 {
        line(
            &mut ui,
            &lang.t("accounts.repaid"),
            signed(-(a.repaid as i64)),
            money(-(a.repaid as i64)),
            false,
        );
    }
    let rule = ui.node(
        left,
        Node {
            height: px(1),
            flex_shrink: 0.0,
            margin: UiRect::vertical(px(3)),
            ..default()
        },
    );
    ui.tint(rule, RULE.with_alpha(0.8));
    line(
        &mut ui,
        &lang.t("accounts.closing"),
        a.closing.to_string(),
        TEXT,
        false,
    );
    let profit = a.profit();
    let key = if profit < 0 {
        "accounts.loss"
    } else {
        "accounts.profit"
    };
    line(&mut ui, &lang.t(key), signed(profit), money(profit), false);
    line(
        &mut ui,
        &lang.t("accounts.wares"),
        a.wares_closing.to_string(),
        TEXT_DIM,
        true,
    );
    let result = a.result_with_wares();
    line(
        &mut ui,
        &lang.t("accounts.result_with_wares"),
        signed(result),
        money(result),
        false,
    );
    if loan.owed > 0 {
        let owed = lang.f("accounts.owed", &[("owed", &loan.owed)]);
        ui.add_text(left, &owed, false, 12.0, RED);
    }

    // Right: how the night went.
    let right = ui.node(
        columns,
        (
            Node {
                flex_grow: 1.0,
                flex_shrink: if narrow { 0.0 } else { 1.0 },
                min_height: px(0),
                min_width: px(0),
                flex_direction: FlexDirection::Column,
                row_gap: px(5),
                padding: UiRect::all(px(14)),
                border_radius: BorderRadius::all(px(4)),
                overflow: if narrow {
                    Overflow::visible()
                } else {
                    Overflow::scroll_y()
                },
                overflow_clip_margin: OverflowClipMargin::content_box(),
                ..default()
            },
            paper(false),
            ScrollPosition::default(),
        ),
    );
    ui.display_text(right, &lang.t("accounts.night"), 18.0, TEXT);
    let text = |ui: &mut PanelBuilder, s: String| ui.add_text(right, &s, false, 13.0, TEXT);
    if progress.night == 1 {
        for note in first_morning_notes(&report, &stock, &lang) {
            text(&mut ui, note);
        }
    }
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
    for (ghost, line) in &report.story_lines {
        ui.add_text(right, &lang.ghost(ghost), true, 13.0, TEXT);
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
                ui.tint(chip, RULE.with_alpha(0.14));
                if let Some(image) = image {
                    ui.image_sized(chip, image, size);
                }
                ui.add_text(chip, &label, false, 12.0, TEXT);
            }
        };
    let icon = Vec2::splat(22.0);
    let product_chips = |counts: Vec<(&str, u32)>| -> Vec<(Option<Handle<Image>>, Vec2, String)> {
        counts
            .into_iter()
            .map(|(id, n)| {
                let label = lang.f("count.entry", &[("count", &n), ("item", &lang.product(id))]);
                (icons.product(id), icon, label)
            })
            .collect()
    };
    chips(
        &mut ui,
        &lang.t("accounts.sold"),
        product_chips(counted_wares(
            report.receipts.iter().map(|r| r.product.as_str()),
        )),
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
        product_chips(counted_wares(report.knocked.iter().map(String::as_str))),
    );
    chips(
        &mut ui,
        &lang.t("morning.stolen_title"),
        product_chips(counted_wares(report.stolen.iter().map(String::as_str))),
    );
    let faded = report
        .faded
        .iter()
        .map(|(id, n)| (id.as_str(), *n))
        .collect();
    chips(&mut ui, &lang.t("morning.faded"), product_chips(faded));
    let fading = fading_next_dawn(&stock, &data, upgrades.fade_bonus(&data));
    chips(
        &mut ui,
        &lang.t("accounts.fading_soon"),
        product_chips(counted_wares(fading.iter().map(String::as_str))),
    );

    // Footer.
    let last = progress.season_over(&data);
    if progress.night == 1 && !last {
        ui.text(&lang.t("accounts.first_market"));
    }
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
    let label = if last && cfg!(feature = "demo") {
        "morning.to_end"
    } else if last {
        "morning.to_season_end"
    } else {
        "morning.next"
    };
    ui.primary(&lang.t(label), Action::NextEvening);
}

fn first_morning_notes(report: &NightReport, stock: &Stock, lang: &Lang) -> [String; 2] {
    let sale = match report.receipts.first() {
        Some(receipt) => lang.f(
            "accounts.first_sale",
            &[
                ("item", &lang.product(&receipt.product)),
                ("price", &receipt.price),
                ("paid", &receipt.paid),
                ("given", &receipt.given),
            ],
        ),
        None => lang.t("accounts.no_first_sale"),
    };
    let kept: Vec<_> = stock
        .units()
        .iter()
        .filter(|(_, ages)| !ages.is_empty())
        .map(|(id, ages)| {
            lang.f(
                "count.entry",
                &[("count", &ages.len()), ("item", &lang.product(id))],
            )
        })
        .collect();
    let stock_note = if kept.is_empty() {
        lang.t("accounts.stock_empty")
    } else {
        lang.f("accounts.stock_kept", &[("items", &lang.join(&kept))])
    };
    [sale, stock_note]
}

/// Counts repeated ids without disturbing the order they appear in the ledger.
fn counted_wares<'a>(ids: impl Iterator<Item = &'a str>) -> Vec<(&'a str, u32)> {
    let mut counts = Vec::new();
    for id in ids {
        match counts.iter_mut().find(|(item, _)| *item == id) {
            Some((_, count)) => *count += 1,
            None => counts.push((id, 1)),
        }
    }
    counts
}

fn money(n: i64) -> Color {
    match n {
        n if n > 0 => GREEN,
        n if n < 0 => RED,
        _ => TEXT,
    }
}

/// Units that will fade at the next dawn if they are not sold tonight.
pub fn fading_next_dawn(stock: &Stock, data: &GameData, bonus: u32) -> Vec<String> {
    stock
        .units()
        .keys()
        .flat_map(|id| std::iter::repeat_n(id.clone(), stock.fading_next_dawn(id, data, bonus)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn first_morning_reports_the_first_receipt_and_the_wares_that_survived_dawn() {
        let data = GameData::load();
        let lang = Lang::load("en");
        let mut report = NightReport::default();
        // The first customer received less change than was due.
        report.record_sale("music_box", 17, 25, 6);
        report.record_sale("rain_jar", 5, 5, 0);
        let last_night = data.product("lavender").unwrap().fade_after - 1;
        let mut stock = Stock::from_units(BTreeMap::from([
            ("lavender".into(), vec![last_night]),
            ("rain_jar".into(), vec![0, 0]),
        ]));
        report.faded = stock.age_and_fade(&data, 0);
        let [receipt, kept] = first_morning_notes(&report, &stock, &lang);
        assert!(receipt.contains(&lang.product("music_box")));
        for amount in ["17", "25", "6"] {
            assert!(receipt.contains(amount));
        }
        assert!(!receipt.contains(&lang.product("rain_jar")));
        assert!(kept.contains(&lang.f(
            "count.entry",
            &[("count", &2), ("item", &lang.product("rain_jar"))],
        )));
        assert!(!kept.contains(&lang.product("lavender")));
        assert_eq!(report.faded, [("lavender".to_owned(), 1)]);
        assert_eq!((report.sales, report.revenue), (2, 22));
    }

    #[test]
    fn first_morning_handles_no_sales_and_empty_stock() {
        let lang = Lang::load("en");
        let stock = Stock::from_units(BTreeMap::from([("lavender".into(), vec![])]));
        assert_eq!(
            first_morning_notes(&NightReport::default(), &stock, &lang),
            [
                lang.t("accounts.no_first_sale"),
                lang.t("accounts.stock_empty")
            ]
        );
    }

    #[test]
    fn an_empty_opening_drawer_still_opens_the_books_only_once() {
        let mut app = App::new();
        app.insert_resource(GameData::load())
            .insert_resource(Progress::new(0))
            .init_resource::<Drawer>()
            .init_resource::<Stock>()
            .init_resource::<Accounts>()
            .add_systems(Update, open_books);
        app.update();
        app.world_mut().resource_mut::<Accounts>().repairs = 3;
        app.update();
        assert_eq!(app.world().resource::<Accounts>().repairs, 3);
        app.world_mut().resource_mut::<Progress>().night += 1;
        app.update();
        assert_eq!(app.world().resource::<Accounts>().repairs, 0);
    }

    #[test]
    fn wares_bought_count_as_an_investment() {
        let data = GameData::load();
        let p = &data.products[0];
        let mut stock = Stock::default();
        stock.add(&p.id, p.crate_size);
        // One crate bought with the drawer, nothing sold yet.
        let accounts = Accounts {
            opening: 100,
            closing: 100 - p.crate_cost,
            wares_opening: 0,
            wares_closing: wares_value(&stock, &data),
            ..default()
        };
        assert_eq!(accounts.profit(), -(p.crate_cost as i64));
        assert_eq!(accounts.result_with_wares(), 0);
    }
    #[test]
    fn the_books_balance() {
        // 100 in the morning, 6 for a rug, 2 in change fees, 20 to the
        // ferryman: 72 when the shop opens.
        let mut a = Accounts {
            opening: 100,
            ferryman: 20,
            furniture: 6,
            market: 2,
            night_start: 72,
            ..default()
        };
        // Dawn: 95 in the drawer after a counterfeit worth 5 turned to dust.
        a.close(95, 5);
        assert_eq!(a.takings, 28);
        assert_eq!(
            a.opening as i64 - a.ferryman as i64 - a.furniture as i64 - a.market as i64 + a.takings
                - a.dust as i64,
            a.closing as i64
        );
        assert_eq!(a.profit(), -5);
    }

    /// A coin given for a crossing has its own line and does not lower the
    /// takings: 50 at opening, 8 taken in sales, a candle stub (5) given.
    #[test]
    fn a_fare_is_booked_apart_from_the_takings() {
        let mut a = Accounts {
            opening: 50,
            night_start: 50,
            fares: 5,
            ..default()
        };
        a.close(53, 0);
        assert_eq!(a.takings, 8);
        assert_eq!(
            a.opening as i64 - a.fares as i64 + a.takings,
            a.closing as i64
        );
    }

    #[test]
    fn units_about_to_fade_are_listed() {
        let data = GameData::load();
        let p = data.product("lavender").expect("lavender exists");
        let stock = Stock::from_units(BTreeMap::from([(
            "lavender".to_string(),
            vec![p.fade_after - 1, 0, p.fade_after - 1],
        )]));
        assert_eq!(fading_next_dawn(&stock, &data, 0).len(), 2);
    }
}
