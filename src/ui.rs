//! User interface: top bar, side panel per phase, message log.
//!
//! Buttons carry an `ActionButton` component. A global observer turns clicks
//! into `Action` messages that the individual plugins read.

use std::collections::VecDeque;

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui::InteractionDisabled;
use bevy::ui_widgets::{Activate, Button};

use crate::data::{GameData, asset_exists};
use crate::day_cycle::{NightClock, NightReport, Phase, Progress};
use crate::economy::{Coin, Drawer};
use crate::fx::Cue;
use crate::ghosts::{Ghost, GhostState, Queue};
use crate::i18n::Lang;
use crate::inventory::{CratesOrdered, Stock};
use crate::register::Register;
use crate::shop::{Keepsakes, ShopLayout, Tool};

pub(crate) const PANEL_WIDTH: f32 = 380.0;
pub(crate) const BG_PANEL: Color = Color::srgba(0.06, 0.06, 0.1, 0.94);
const TEXT: Color = Color::srgb(0.88, 0.86, 0.8);
pub(crate) const TEXT_DIM: Color = Color::srgb(0.6, 0.6, 0.65);
const ACCENT: Color = Color::srgb(0.95, 0.8, 0.45);
const BTN: Color = Color::srgb(0.17, 0.16, 0.24);
const BTN_HOVER: Color = Color::srgb(0.26, 0.24, 0.36);
const BTN_SELECTED: Color = Color::srgb(0.45, 0.36, 0.18);
const BTN_PRIMARY: Color = Color::srgb(0.22, 0.38, 0.3);
const BTN_DISABLED: Color = Color::srgb(0.1, 0.1, 0.12);

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Action>()
            .init_resource::<UiFont>()
            .init_resource::<GameLog>()
            .add_systems(Startup, spawn_ui)
            // Before Update, so every rebuild this frame already uses the new font.
            .add_systems(PreUpdate, apply_language.run_if(resource_changed::<Lang>))
            .add_observer(on_activate)
            .add_systems(OnEnter(Phase::Morning), build_morning_panel)
            .add_systems(OnEnter(Phase::Menu), clear_for_menu)
            .add_systems(OnEnter(Phase::DemoEnd), build_demo_end_panel)
            .add_systems(
                Update,
                (
                    update_hud,
                    update_log.run_if(resource_changed::<GameLog>),
                    refresh_static_fonts.run_if(resource_changed::<UiFont>),
                    build_morning_panel
                        .run_if(in_state(Phase::Morning).and_then(resource_changed::<UiFont>)),
                    build_demo_end_panel
                        .run_if(in_state(Phase::DemoEnd).and_then(resource_changed::<UiFont>)),
                    button_hover,
                    build_evening_panel.run_if(
                        in_state(Phase::Evening).and_then(
                            state_changed::<Phase>
                                .or_else(resource_changed::<Stock>)
                                .or_else(resource_changed::<Tool>)
                                .or_else(resource_changed::<ShopLayout>)
                                .or_else(resource_changed::<Drawer>)
                                .or_else(resource_changed::<Keepsakes>)
                                .or_else(resource_changed::<UiFont>),
                        ),
                    ),
                    build_night_panel.run_if(
                        in_state(Phase::Night).and_then(
                            state_changed::<Phase>
                                .or_else(resource_changed::<Register>)
                                .or_else(resource_changed::<Queue>)
                                .or_else(resource_changed::<UiFont>)
                                .or_else(ghost_state_changed),
                        ),
                    ),
                ),
            );
    }
}

/// Everything a button can trigger.
#[derive(Message, Clone, Debug, PartialEq)]
pub enum Action {
    Order(String),
    ToolPlace(String),
    ToolStock(String),
    ToolRemove,
    StartNight,
    Offer(String),
    Decline,
    /// Inspect (and reject, if fake) the paid coin with this index.
    CheckCoin(usize),
    AddChange(Coin),
    ResetChange,
    GiveChange,
    KeepSecondPayment,
    RefuseSecondPayment,
    NextEvening,
    NewGame,
    Continue,
    Resume,
    QuitToMenu,
    QuitGame,
    ToggleTips,
    ToggleSound,
    OpenLanguages,
    CloseLanguages,
    SetLanguage(String),
}

#[derive(Component, Clone)]
pub struct ActionButton(pub Action);

#[derive(Component, Clone, Copy)]
struct ButtonColor(Color);

/// Fonts for the current language: Inter first, then the language's own
/// script font (CJK, Thai, Arabic), then every other script font, so that
/// any character – e.g. language names in the language menu – can be drawn.
#[derive(Resource)]
pub struct UiFont {
    pub regular: FontSource,
    pub bold: FontSource,
    inter: Handle<Font>,
    inter_bold: Handle<Font>,
    /// (language code, script font)
    extras: Vec<(String, Handle<Font>)>,
}

impl FromWorld for UiFont {
    fn from_world(world: &mut World) -> Self {
        let code = world.resource::<Lang>().code.clone();
        let languages = world.resource::<Lang>().languages.clone();
        let assets = world.resource::<AssetServer>();
        let extras = languages
            .iter()
            .filter_map(|l| {
                let file = format!("fonts/{}", l.font.as_ref()?);
                asset_exists(&file).then(|| (l.code.clone(), assets.load(file)))
            })
            .collect();
        let mut font = Self {
            regular: FontSource::default(),
            bold: FontSource::default(),
            inter: assets.load("fonts/Inter-Regular.otf"),
            inter_bold: assets.load("fonts/Inter-SemiBold.otf"),
            extras,
        };
        font.set_language(&code);
        font
    }
}

impl UiFont {
    fn chain(&self, base: &Handle<Font>, code: &str) -> FontSource {
        let own = self.extras.iter().filter(|(c, _)| c == code);
        let others = self.extras.iter().filter(|(c, _)| c != code);
        FontSource::list(
            std::iter::once(base.clone()).chain(own.chain(others).map(|(_, h)| h.clone())),
        )
    }

    pub fn set_language(&mut self, code: &str) {
        self.regular = self.chain(&self.inter, code);
        self.bold = self.chain(&self.inter_bold, code);
    }
}

fn apply_language(lang: Res<Lang>, mut font: ResMut<UiFont>) {
    font.set_language(&lang.code);
}

/// Texts that are not rebuilt with the panels and need the new font.
#[derive(Component)]
struct StaticText {
    bold: bool,
}

fn refresh_static_fonts(font: Res<UiFont>, mut texts: Query<(&StaticText, &mut TextFont)>) {
    for (st, mut tf) in &mut texts {
        tf.font = if st.bold {
            font.bold.clone()
        } else {
            font.regular.clone()
        };
    }
}

/// The latest messages, bottom left.
#[derive(Resource, Default)]
pub struct GameLog {
    lines: VecDeque<String>,
}

impl GameLog {
    pub fn clear(&mut self) {
        self.lines.clear();
    }

    pub fn push(&mut self, line: impl Into<String>) {
        let line = line.into();
        info!("{line}");
        self.lines.push_back(line);
        while self.lines.len() > 4 {
            self.lines.pop_front();
        }
    }
}

#[derive(Component)]
struct HudText;

#[derive(Component)]
struct LogText;

#[derive(Component)]
struct SidePanel;

fn spawn_ui(mut commands: Commands, font: Res<UiFont>) {
    // Top bar
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(0),
            height: px(40),
            padding: UiRect::horizontal(px(16)),
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(BG_PANEL),
        children![(
            HudText,
            StaticText { bold: true },
            Text::new(""),
            text_font(&font.bold, 15.0),
            TextColor(TEXT)
        )],
    ));

    // Side panel – its content is rebuilt per phase.
    commands.spawn((
        SidePanel,
        Node {
            position_type: PositionType::Absolute,
            right: px(0),
            top: px(40),
            bottom: px(0),
            width: px(PANEL_WIDTH),
            padding: UiRect::all(px(14)),
            flex_direction: FlexDirection::Column,
            row_gap: px(6),
            overflow: Overflow::clip_y(),
            ..default()
        },
        BackgroundColor(BG_PANEL),
    ));

    // Message log
    commands.spawn((
        LogText,
        StaticText { bold: false },
        Text::new(""),
        text_font(&font.regular, 14.0),
        TextColor(TEXT_DIM),
        Node {
            position_type: PositionType::Absolute,
            left: px(16),
            bottom: px(12),
            right: px(PANEL_WIDTH + 16.0),
            ..default()
        },
        Pickable::IGNORE,
    ));
}

fn on_activate(
    activate: On<Activate>,
    buttons: Query<&ActionButton>,
    mut writer: MessageWriter<Action>,
    mut cues: MessageWriter<Cue>,
) {
    if let Ok(button) = buttons.get(activate.entity) {
        writer.write(button.0.clone());
        cues.write(Cue::Click);
    }
}

fn button_hover(
    mut buttons: Query<
        (
            &Hovered,
            &ButtonColor,
            &mut BackgroundColor,
            Has<InteractionDisabled>,
        ),
        Changed<Hovered>,
    >,
) {
    for (hovered, base, mut bg, disabled) in &mut buttons {
        *bg = if hovered.get() && !disabled && base.0 == BTN {
            BTN_HOVER.into()
        } else {
            base.0.into()
        };
    }
}

fn update_hud(
    lang: Res<Lang>,
    phase: Res<State<Phase>>,
    data: Res<GameData>,
    progress: Res<Progress>,
    clock: Res<NightClock>,
    drawer: Res<Drawer>,
    mut hud: Single<&mut Text, With<HudText>>,
) {
    if *phase.get() == Phase::Menu {
        hud.0.clear();
        return;
    }
    let time = if *phase.get() == Phase::Night {
        format!(" {}", clock.time_string())
    } else {
        String::new()
    };
    hud.0 = lang.f(
        "hud",
        &[
            ("night", &progress.night),
            ("nights", &data.config.nights_in_demo),
            ("phase", &lang.t(phase.get().key())),
            ("time", &time),
            ("drawer", &drawer.describe(&lang)),
            ("total", &drawer.total()),
            ("reputation", &progress.reputation),
        ],
    );
}

/// The menu starts from a clean screen: no side panel, no old messages.
fn clear_for_menu(
    mut commands: Commands,
    panel: Single<Entity, With<SidePanel>>,
    mut log: ResMut<GameLog>,
) {
    commands.entity(*panel).despawn_related::<Children>();
    log.clear();
}

fn update_log(log: Res<GameLog>, mut text: Single<&mut Text, With<LogText>>) {
    text.0 = log.lines.iter().cloned().collect::<Vec<_>>().join("\n");
}

/// Has any ghost changed its state? (So the panel notices, for example,
/// that the front ghost has arrived at the counter.)
fn ghost_state_changed(ghosts: Query<&Ghost>, mut last: Local<Vec<GhostState>>) -> bool {
    // `Changed<Ghost>` would fire every frame (position), so compare states instead.
    let now: Vec<GhostState> = ghosts.iter().map(|g| g.state).collect();
    let changed = *last != now;
    *last = now;
    changed
}

// ---------------------------------------------------------------------------
// Panels

fn build_evening_panel(
    mut commands: Commands,
    panel: Single<Entity, With<SidePanel>>,
    font: Res<UiFont>,
    lang: Res<Lang>,
    data: Res<GameData>,
    stock: Res<Stock>,
    crates: Res<CratesOrdered>,
    tool: Res<Tool>,
    layout: Res<ShopLayout>,
    keepsakes: Res<Keepsakes>,
) {
    let mut ui = PanelBuilder::new(&mut commands, *panel, &font);
    ui.heading(&lang.t("evening.title"));

    ui.section(&lang.f(
        "evening.ferryman",
        &[("count", &crates.0), ("max", &data.config.crates_per_night)],
    ));
    let row = ui.row();
    let boat_full = crates.0 >= data.config.crates_per_night;
    for p in &data.products {
        let label = lang.f(
            "evening.order_button",
            &[
                ("item", &lang.product(&p.id)),
                ("size", &p.crate_size),
                ("cost", &p.crate_cost),
            ],
        );
        ui.button_in(row, &label, Action::Order(p.id.clone()), false, !boat_full);
    }

    ui.section(&lang.t("evening.furnish"));
    let row = ui.row();
    for d in data.decor.iter().filter(|d| !d.keepsake) {
        let selected = *tool == Tool::Place(d.id.clone());
        let label = lang.f(
            "evening.decor_button",
            &[("item", &lang.decor(&d.id)), ("cost", &d.cost)],
        );
        ui.button_in(row, &label, Action::ToolPlace(d.id.clone()), selected, true);
    }
    ui.button_in(
        row,
        &lang.t("evening.remove"),
        Action::ToolRemove,
        *tool == Tool::Remove,
        true,
    );

    let owned: Vec<_> = data
        .decor
        .iter()
        .filter(|d| d.keepsake && keepsakes.count(&d.id) > 0)
        .collect();
    if !owned.is_empty() {
        ui.section(&lang.t("evening.keepsakes"));
        let row = ui.row();
        for d in owned {
            let selected = *tool == Tool::Place(d.id.clone());
            ui.button_in(
                row,
                &lang.decor(&d.id),
                Action::ToolPlace(d.id.clone()),
                selected,
                true,
            );
        }
    }

    ui.section(&lang.t("evening.shelve"));
    let row = ui.row();
    let mut any = false;
    for p in &data.products {
        let n = stock.count(&p.id);
        if n == 0 {
            continue;
        }
        any = true;
        let selected = *tool == Tool::Stock(p.id.clone());
        let label = lang.f(
            "evening.stock_button",
            &[("item", &lang.product(&p.id)), ("count", &n)],
        );
        ui.button_in(row, &label, Action::ToolStock(p.id.clone()), selected, true);
    }
    if !any {
        ui.dim(&lang.t("evening.storeroom_empty"));
    }

    ui.section(&lang.t("evening.atmosphere"));
    ui.text(&layout.atmosphere(&data).describe(&lang));
    ui.dim(&lang.f(
        "evening.display_summary",
        &[
            ("shelves", &layout.shelf_cells(&data).len()),
            ("products", &layout.displayed_products().len()),
        ],
    ));

    ui.spacer();
    ui.primary(&lang.t("evening.open"), Action::StartNight);
}

fn build_night_panel(
    mut commands: Commands,
    panel: Single<Entity, With<SidePanel>>,
    font: Res<UiFont>,
    lang: Res<Lang>,
    data: Res<GameData>,
    stock: Res<Stock>,
    register: Res<Register>,
    queue: Res<Queue>,
    layout: Res<ShopLayout>,
    drawer: Res<Drawer>,
    ghosts: Query<&Ghost>,
) {
    let mut ui = PanelBuilder::new(&mut commands, *panel, &font);
    ui.heading(&lang.t("night.title"));

    if let Some(sale) = &register.sale {
        ui.section(&lang.t("night.register"));
        ui.text(&lang.f("night.item", &[("item", &lang.product(&sale.product))]));
        ui.text(&lang.f("night.price", &[("price", &sale.price)]));
        let paid = sale.paid();
        ui.text(&lang.f(
            "night.paid",
            &[("coins", &paid.describe(&lang)), ("total", &paid.total())],
        ));
        ui.dim(&lang.t("night.inspect_hint"));
        let row = ui.row();
        for (i, c) in sale.coins.iter().enumerate() {
            let label = lang.f(
                "night.coin_chip",
                &[("coin", &c.coin.name(&lang)), ("tell", &lang.t(&c.tell))],
            );
            ui.button_in(row, &label, Action::CheckCoin(i), false, !c.checked);
        }
        if data.config.show_change_due {
            ui.dim(&lang.f("night.change_due", &[("amount", &sale.change_due())]));
        }
        ui.text(&lang.f(
            "night.counted",
            &[
                ("coins", &sale.given.describe(&lang)),
                ("total", &sale.given.total()),
            ],
        ));
        let row = ui.row();
        for coin in Coin::ASCENDING {
            let left = drawer.get(coin) - sale.given.get(coin);
            let label = lang.f(
                "night.add_coin",
                &[
                    ("coin", &coin.name(&lang)),
                    ("value", &coin.value()),
                    ("left", &left),
                ],
            );
            ui.button_in(row, &label, Action::AddChange(coin), false, left > 0);
        }
        let row = ui.row();
        ui.button_in(
            row,
            &lang.t("night.put_back"),
            Action::ResetChange,
            false,
            true,
        );
        ui.spacer();
        ui.primary(&lang.t("night.hand_over"), Action::GiveChange);
        return;
    }

    if let Some(question) = &register.question {
        let who = ghosts
            .get(question.ghost)
            .map(|g| lang.ghost_the(&data.ghosts[g.def].id))
            .unwrap_or_default();
        ui.section(&lang.f("night.pay_again_title", &[("who", &who)]));
        ui.text(&lang.f("night.pay_again_text", &[("amount", &question.amount)]));
        ui.spacer();
        let root = ui.root();
        ui.button_in(
            root,
            &lang.t("night.keep_payment"),
            Action::KeepSecondPayment,
            false,
            true,
        );
        ui.primary(&lang.t("night.tell_truth"), Action::RefuseSecondPayment);
        return;
    }

    let front = queue.front().and_then(|e| ghosts.get(e).ok());
    match front {
        Some(ghost) if ghost.state == GhostState::Waiting => {
            let def = &data.ghosts[ghost.def];
            if ghost.chapter.is_some() {
                ui.section(&lang.f("night.story_says", &[("name", &lang.t("story.name"))]));
            } else {
                ui.section(&lang.f("night.whispers", &[("who", &lang.ghost_the(&def.id))]));
            }
            ui.text(&lang.f("quote", &[("text", &lang.t(&ghost.hint))]));
            ui.section(&lang.t("night.what_hand_over"));
            let row = ui.row();
            let mut any = false;
            for id in layout.displayed_products() {
                let n = stock.count(&id);
                if n == 0 {
                    continue;
                }
                any = true;
                let label = lang.f(
                    "evening.stock_button",
                    &[("item", &lang.product(&id)), ("count", &n)],
                );
                ui.button_in(row, &label, Action::Offer(id.clone()), false, true);
            }
            if !any {
                ui.dim(&lang.t("night.nothing_on_shelves"));
            }
            ui.spacer();
            let root = ui.root();
            ui.button_in(root, &lang.t("night.decline"), Action::Decline, false, true);
        }
        _ => {
            ui.dim(&lang.t("night.quiet"));
            ui.dim(&lang.f("night.in_line", &[("count", &queue.0.len())]));
        }
    }
}

fn build_morning_panel(
    mut commands: Commands,
    panel: Single<Entity, With<SidePanel>>,
    font: Res<UiFont>,
    lang: Res<Lang>,
    data: Res<GameData>,
    report: Res<NightReport>,
    progress: Res<Progress>,
) {
    let mut ui = PanelBuilder::new(&mut commands, *panel, &font);
    ui.heading(&lang.t("morning.title"));
    ui.text(&lang.f(
        "morning.sales",
        &[("sales", &report.sales), ("revenue", &report.revenue)],
    ));
    ui.text(&lang.f(
        "morning.change",
        &[
            ("correct", &report.change_correct),
            ("wrong", &report.change_wrong),
        ],
    ));
    ui.text(&lang.f(
        "morning.unhappy",
        &[("unhappy", &report.unhappy), ("fled", &report.fled)],
    ));
    ui.text(&lang.f(
        "morning.reputation",
        &[
            ("from", &report.reputation_start),
            ("to", &progress.reputation),
        ],
    ));

    if let Some(line) = &report.story_line {
        ui.section(&lang.t("story.name"));
        ui.dim(&lang.f("quote", &[("text", &lang.t(line))]));
    }
    if !report.keepsakes.is_empty() {
        let names: Vec<String> = report.keepsakes.iter().map(|k| lang.decor(k)).collect();
        ui.dim(&lang.f("morning.keepsake", &[("items", &lang.join(&names))]));
    }
    if report.honest + report.dishonest > 0 {
        ui.text(&lang.f(
            "morning.second_payments",
            &[("refused", &report.honest), ("kept", &report.dishonest)],
        ));
    }
    if report.counterfeits_caught > 0 {
        ui.text(&lang.f(
            "morning.counterfeits_spotted",
            &[("count", &report.counterfeits_caught)],
        ));
    }
    if !report.counterfeits_found.is_empty() {
        ui.text(&lang.f(
            "morning.counterfeits_found",
            &[
                ("coins", &report.counterfeits_found.describe(&lang)),
                ("total", &report.counterfeits_found.total()),
            ],
        ));
    }
    if !report.knocked.is_empty() {
        ui.text(&lang.f(
            "morning.knocked",
            &[(
                "items",
                &count_list(&lang, &report.knocked, |p| lang.product(p)),
            )],
        ));
    }

    if !report.redeemed.is_empty() {
        ui.section(&lang.t("morning.redeemed"));
        let names: Vec<String> = report.redeemed.iter().map(|g| lang.ghost(g)).collect();
        ui.dim(&lang.join(&names));
    }
    if !report.faded.is_empty() {
        ui.section(&lang.t("morning.faded"));
        let lines: Vec<String> = report
            .faded
            .iter()
            .map(|(id, n)| lang.f("count.entry", &[("count", n), ("item", &lang.product(id))]))
            .collect();
        ui.dim(&lang.join(&lines));
    }

    ui.spacer();
    let last = progress.night >= data.config.nights_in_demo;
    let label = if last {
        "morning.to_end"
    } else {
        "morning.next"
    };
    ui.primary(&lang.t(label), Action::NextEvening);
}

fn build_demo_end_panel(
    mut commands: Commands,
    panel: Single<Entity, With<SidePanel>>,
    font: Res<UiFont>,
    lang: Res<Lang>,
    data: Res<GameData>,
    progress: Res<Progress>,
) {
    let mut ui = PanelBuilder::new(&mut commands, *panel, &font);
    ui.heading(&lang.t("end.title"));
    ui.text(&lang.f("end.reputation", &[("reputation", &progress.reputation)]));
    let chapters = data.story.chapters.len();
    if progress.story_chapter >= chapters {
        ui.dim(&lang.t("story.ending"));
    } else {
        ui.dim(&lang.f(
            "end.wishes_left",
            &[
                ("name", &lang.t("story.name")),
                ("count", &(chapters - progress.story_chapter)),
            ],
        ));
    }
    ui.dim(&lang.t("end.thanks"));
    ui.spacer();
    ui.primary(&lang.t("end.back"), Action::QuitToMenu);
}

/// "3× Glass Marbles, 1× Rusty Key" – in order of first appearance.
fn count_list(lang: &Lang, ids: &[String], name: impl Fn(&str) -> String) -> String {
    let mut counts: Vec<(&str, u32)> = Vec::new();
    for id in ids {
        match counts.iter_mut().find(|(i, _)| *i == id.as_str()) {
            Some((_, n)) => *n += 1,
            None => counts.push((id.as_str(), 1)),
        }
    }
    let entries: Vec<String> = counts
        .iter()
        .map(|(id, n)| lang.f("count.entry", &[("count", n), ("item", &name(id))]))
        .collect();
    lang.join(&entries)
}

// ---------------------------------------------------------------------------
// Small toolkit for panel content

pub fn text_font(font: &FontSource, size: f32) -> TextFont {
    TextFont {
        font: font.clone(),
        font_size: FontSize::Px(size),
        ..default()
    }
}

pub(crate) struct PanelBuilder<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
    root: Entity,
    font: &'a UiFont,
}

impl<'a, 'w, 's> PanelBuilder<'a, 'w, 's> {
    /// Clears the panel and starts over.
    pub(crate) fn new(commands: &'a mut Commands<'w, 's>, root: Entity, font: &'a UiFont) -> Self {
        commands.entity(root).despawn_related::<Children>();
        Self {
            commands,
            root,
            font,
        }
    }

    pub(crate) fn root(&self) -> Entity {
        self.root
    }

    pub(crate) fn add_text(
        &mut self,
        parent: Entity,
        text: &str,
        bold: bool,
        size: f32,
        color: Color,
    ) {
        let font = if bold {
            &self.font.bold
        } else {
            &self.font.regular
        };
        self.commands.spawn((
            Text::new(text),
            text_font(font, size),
            TextColor(color),
            ChildOf(parent),
        ));
    }

    pub(crate) fn heading(&mut self, text: &str) {
        self.add_text(self.root, text, true, 20.0, ACCENT);
    }

    pub(crate) fn section(&mut self, text: &str) {
        self.commands.spawn((
            Node {
                height: px(6),
                ..default()
            },
            ChildOf(self.root),
        ));
        self.add_text(self.root, text, true, 15.0, TEXT);
    }

    pub(crate) fn text(&mut self, text: &str) {
        self.add_text(self.root, text, false, 14.0, TEXT);
    }

    pub(crate) fn dim(&mut self, text: &str) {
        self.add_text(self.root, text, false, 13.0, TEXT_DIM);
    }

    pub(crate) fn spacer(&mut self) {
        self.commands.spawn((
            Node {
                flex_grow: 1.0,
                ..default()
            },
            ChildOf(self.root),
        ));
    }

    /// Row in which buttons wrap.
    pub(crate) fn row(&mut self) -> Entity {
        self.commands
            .spawn((
                Node {
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: px(4),
                    row_gap: px(4),
                    ..default()
                },
                ChildOf(self.root),
            ))
            .id()
    }

    pub(crate) fn button_in(
        &mut self,
        parent: Entity,
        label: &str,
        action: Action,
        selected: bool,
        enabled: bool,
    ) {
        let color = match (enabled, selected) {
            (false, _) => BTN_DISABLED,
            (true, true) => BTN_SELECTED,
            (true, false) => BTN,
        };
        self.spawn_button(parent, label, action, color, enabled, 12.0);
    }

    pub(crate) fn primary(&mut self, label: &str, action: Action) {
        self.spawn_button(self.root, label, action, BTN_PRIMARY, true, 16.0);
    }

    pub(crate) fn spawn_button(
        &mut self,
        parent: Entity,
        label: &str,
        action: Action,
        color: Color,
        enabled: bool,
        size: f32,
    ) {
        let text_color = if enabled { TEXT } else { TEXT_DIM };
        let mut button = self.commands.spawn((
            Button,
            Hovered::default(),
            ActionButton(action),
            ButtonColor(color),
            Node {
                padding: UiRect::axes(px(8), px(5)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(4)),
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(color),
            BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.08)),
            ChildOf(parent),
            children![(
                Text::new(label),
                text_font(&self.font.regular, size),
                TextColor(text_color),
                Pickable::IGNORE,
            )],
        ));
        if !enabled {
            button.insert(InteractionDisabled);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_list_groups_in_order() {
        let ids: Vec<String> = ["a", "b", "a", "a"].iter().map(|s| s.to_string()).collect();
        let lang = Lang::load("en");
        assert_eq!(count_list(&lang, &ids, |s| s.to_string()), "3× a, 1× b");
    }
}
