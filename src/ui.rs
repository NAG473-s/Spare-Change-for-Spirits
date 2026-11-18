//! User interface: top bar, side panel per phase, message log.
//!
//! Buttons carry an `ActionButton` component. A global observer turns clicks
//! into `Action` messages that the individual plugins read.

use std::collections::VecDeque;

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui::InteractionDisabled;
use bevy::ui_widgets::{Activate, Button};

use crate::data::GameData;
use crate::day_cycle::{NightClock, NightReport, Phase, Progress};
use crate::economy::{Coin, Drawer};
use crate::fx::Cue;
use crate::ghosts::{Ghost, GhostState, Queue};
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
            .add_observer(on_activate)
            .add_systems(OnEnter(Phase::Morning), build_morning_panel)
            .add_systems(OnEnter(Phase::Menu), clear_for_menu)
            .add_systems(OnEnter(Phase::DemoEnd), build_demo_end_panel)
            .add_systems(
                Update,
                (
                    update_hud,
                    update_log.run_if(resource_changed::<GameLog>),
                    button_hover,
                    build_evening_panel.run_if(
                        in_state(Phase::Evening).and_then(
                            state_changed::<Phase>
                                .or_else(resource_changed::<Stock>)
                                .or_else(resource_changed::<Tool>)
                                .or_else(resource_changed::<ShopLayout>)
                                .or_else(resource_changed::<Drawer>)
                                .or_else(resource_changed::<Keepsakes>),
                        ),
                    ),
                    build_night_panel.run_if(
                        in_state(Phase::Night).and_then(
                            state_changed::<Phase>
                                .or_else(resource_changed::<Register>)
                                .or_else(resource_changed::<Queue>)
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
}

#[derive(Component, Clone)]
pub struct ActionButton(pub Action);

#[derive(Component, Clone, Copy)]
struct ButtonColor(Color);

#[derive(Resource)]
pub struct UiFont {
    pub regular: Handle<Font>,
    pub bold: Handle<Font>,
}

impl FromWorld for UiFont {
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<AssetServer>();
        Self {
            regular: assets.load("fonts/Inter-Regular.otf"),
            bold: assets.load("fonts/Inter-SemiBold.otf"),
        }
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
    hud.0 = format!(
        "Night {}/{}  ·  {}{}  ·  Drawer: {} (= {})  ·  Reputation {}",
        progress.night,
        data.config.nights_in_demo,
        phase.get().label(),
        time,
        drawer.describe(),
        drawer.total(),
        progress.reputation
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
    data: Res<GameData>,
    stock: Res<Stock>,
    crates: Res<CratesOrdered>,
    tool: Res<Tool>,
    layout: Res<ShopLayout>,
    keepsakes: Res<Keepsakes>,
) {
    let mut ui = PanelBuilder::new(&mut commands, *panel, &font);
    ui.heading("Evening – Preparation");

    ui.section(&format!(
        "Ferryman · crates {}/{}",
        crates.0, data.config.crates_per_night
    ));
    let row = ui.row();
    let boat_full = crates.0 >= data.config.crates_per_night;
    for p in &data.products {
        let label = format!("{} ({}× · {})", p.name, p.crate_size, p.crate_cost);
        ui.button_in(row, &label, Action::Order(p.id.clone()), false, !boat_full);
    }

    ui.section("Furnish (pick a tool, then click a cell)");
    let row = ui.row();
    for d in data.decor.iter().filter(|d| !d.keepsake) {
        let selected = *tool == Tool::Place(d.id.clone());
        ui.button_in(
            row,
            &format!("{} ({})", d.name, d.cost),
            Action::ToolPlace(d.id.clone()),
            selected,
            true,
        );
    }
    ui.button_in(
        row,
        "Remove",
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
        ui.section("Keepsakes (free to place)");
        let row = ui.row();
        for d in owned {
            let selected = *tool == Tool::Place(d.id.clone());
            ui.button_in(
                row,
                &d.name,
                Action::ToolPlace(d.id.clone()),
                selected,
                true,
            );
        }
    }

    ui.section("Put products on shelves");
    let row = ui.row();
    let mut any = false;
    for p in &data.products {
        let n = stock.count(&p.id);
        if n == 0 {
            continue;
        }
        any = true;
        let selected = *tool == Tool::Stock(p.id.clone());
        ui.button_in(
            row,
            &format!("{} ×{}", p.name, n),
            Action::ToolStock(p.id.clone()),
            selected,
            true,
        );
    }
    if !any {
        ui.dim("The storeroom is empty. Order something from the ferryman.");
    }

    ui.section("Atmosphere");
    ui.text(&layout.atmosphere(&data).describe());
    let displayed = layout.displayed_products();
    ui.dim(&format!(
        "{} shelf/shelves, {} different products on display.",
        layout.shelf_cells(&data).len(),
        displayed.len()
    ));

    ui.spacer();
    ui.primary("Open the shop  →", Action::StartNight);
}

fn build_night_panel(
    mut commands: Commands,
    panel: Single<Entity, With<SidePanel>>,
    font: Res<UiFont>,
    data: Res<GameData>,
    stock: Res<Stock>,
    register: Res<Register>,
    queue: Res<Queue>,
    layout: Res<ShopLayout>,
    drawer: Res<Drawer>,
    ghosts: Query<&Ghost>,
) {
    let mut ui = PanelBuilder::new(&mut commands, *panel, &font);
    ui.heading("Night – open");

    if let Some(sale) = &register.sale {
        ui.section("Register");
        ui.text(&format!("Item: {}", data.product_name(&sale.product)));
        ui.text(&format!("Price: {}", sale.price));
        let paid = sale.paid();
        ui.text(&format!("Paid: {} (= {})", paid.describe(), paid.total()));
        ui.dim("Click a coin to inspect it. Doubting a genuine coin offends the customer.");
        let row = ui.row();
        for (i, c) in sale.coins.iter().enumerate() {
            ui.button_in(
                row,
                &format!("{} · {}", c.coin.name(1), c.tell),
                Action::CheckCoin(i),
                false,
                !c.checked,
            );
        }
        if data.config.show_change_due {
            ui.dim(&format!("Change due: {}", sale.change_due()));
        }
        ui.text(&format!(
            "Counted out: {} (= {})",
            sale.given.describe(),
            sale.given.total()
        ));
        let row = ui.row();
        for coin in Coin::ASCENDING {
            let left = drawer.get(coin) - sale.given.get(coin);
            ui.button_in(
                row,
                &format!("+ {} ({}) · {} in drawer", coin.name(1), coin.value(), left),
                Action::AddChange(coin),
                false,
                left > 0,
            );
        }
        let row = ui.row();
        ui.button_in(row, "Put back", Action::ResetChange, false, true);
        ui.spacer();
        ui.primary("Hand over", Action::GiveChange);
        return;
    }

    if let Some(question) = &register.question {
        let name = ghosts
            .get(question.ghost)
            .map(|g| data.ghosts[g.def].the_cap())
            .unwrap_or_default();
        ui.section(&format!("{name} wants to pay again"));
        ui.text(&format!(
            "It has already paid, but it holds out {} once more.",
            question.amount
        ));
        ui.spacer();
        ui.button_in(
            ui.root(),
            "Keep the second payment",
            Action::KeepSecondPayment,
            false,
            true,
        );
        ui.primary("Tell it the truth", Action::RefuseSecondPayment);
        return;
    }

    let front = queue.front().and_then(|e| ghosts.get(e).ok());
    match front {
        Some(ghost) if ghost.state == GhostState::Waiting => {
            let def = &data.ghosts[ghost.def];
            if ghost.chapter.is_some() {
                ui.section(&format!("{} says:", data.story.name));
            } else {
                ui.section(&format!("{} whispers:", def.the_cap()));
            }
            ui.text(&format!("\u{201c}{}\u{201d}", ghost.hint));
            ui.section("What do you hand over?");
            let row = ui.row();
            let mut any = false;
            for id in layout.displayed_products() {
                let n = stock.count(&id);
                if n == 0 {
                    continue;
                }
                any = true;
                ui.button_in(
                    row,
                    &format!("{} ×{}", data.product_name(&id), n),
                    Action::Offer(id.clone()),
                    false,
                    true,
                );
            }
            if !any {
                ui.dim("Nothing on the shelves …");
            }
            ui.spacer();
            ui.button_in(
                ui.root(),
                "Apologise (decline)",
                Action::Decline,
                false,
                true,
            );
        }
        _ => {
            ui.dim("The shop is quiet. Candles flicker.");
            ui.dim(&format!("Ghosts in line: {}", queue.0.len()));
        }
    }
}

fn build_morning_panel(
    mut commands: Commands,
    panel: Single<Entity, With<SidePanel>>,
    font: Res<UiFont>,
    data: Res<GameData>,
    report: Res<NightReport>,
    progress: Res<Progress>,
) {
    let mut ui = PanelBuilder::new(&mut commands, *panel, &font);
    ui.heading("Dawn – Accounts");
    ui.text(&format!(
        "Sales: {} · Revenue: {}",
        report.sales, report.revenue
    ));
    ui.text(&format!(
        "Change correct: {} · wrong: {}",
        report.change_correct, report.change_wrong
    ));
    ui.text(&format!(
        "Left unhappy: {} · Fled: {}",
        report.unhappy, report.fled
    ));
    ui.text(&format!(
        "Reputation: {} → {}",
        report.reputation_start, progress.reputation
    ));

    if let Some(line) = &report.story_line {
        ui.section(&data.story.name);
        ui.dim(&format!("\u{201c}{line}\u{201d}"));
    }
    if !report.keepsakes.is_empty() {
        let names: Vec<&str> = report
            .keepsakes
            .iter()
            .map(|k| data.decor(k).map_or(k.as_str(), |d| d.name.as_str()))
            .collect();
        ui.dim(&format!("Keepsake received: {}", names.join(", ")));
    }
    if report.honest + report.dishonest > 0 {
        ui.text(&format!(
            "Second payments refused: {} · kept: {}",
            report.honest, report.dishonest
        ));
    }
    if report.counterfeits_caught > 0 {
        ui.text(&format!(
            "Counterfeits spotted: {}",
            report.counterfeits_caught
        ));
    }
    if !report.counterfeits_found.is_empty() {
        ui.text(&format!(
            "Counterfeits found in the drawer: {} (−{})",
            report.counterfeits_found.describe(),
            report.counterfeits_found.total()
        ));
    }
    if !report.knocked.is_empty() {
        ui.text(&format!(
            "Knocked off the shelves: {}",
            count_list(&report.knocked, |p| data.product_name(p))
        ));
    }

    if !report.redeemed.is_empty() {
        ui.section("Passed into the light");
        ui.dim(&report.redeemed.join(", "));
    }
    if !report.faded.is_empty() {
        ui.section("Faded items");
        let lines: Vec<String> = report
            .faded
            .iter()
            .map(|(id, n)| format!("{n}× {}", data.product_name(id)))
            .collect();
        ui.dim(&lines.join(", "));
    }

    ui.spacer();
    let last = progress.night >= data.config.nights_in_demo;
    ui.primary(
        if last {
            "To the end of the demo  →"
        } else {
            "Next evening  →"
        },
        Action::NextEvening,
    );
}

fn build_demo_end_panel(
    mut commands: Commands,
    panel: Single<Entity, With<SidePanel>>,
    font: Res<UiFont>,
    data: Res<GameData>,
    progress: Res<Progress>,
) {
    let mut ui = PanelBuilder::new(&mut commands, *panel, &font);
    ui.heading("End of the demo");
    ui.text(&format!(
        "Your reputation among the spirits: {}",
        progress.reputation
    ));
    let story = &data.story;
    if progress.story_chapter >= story.chapters.len() {
        ui.dim(&story.ending);
    } else {
        ui.dim(&format!(
            "{} still has {} wish(es) left. Perhaps next time.",
            story.name,
            story.chapters.len() - progress.story_chapter
        ));
    }
    ui.dim("Thanks for playing! Wishlist Spare Change for Spirits on Steam.");
    ui.spacer();
    ui.primary("Back to the menu", Action::QuitToMenu);
}

/// "3× Glass Marbles, 1× Rusty Key" – in order of first appearance.
fn count_list<'a>(ids: &'a [String], name: impl Fn(&'a str) -> &'a str) -> String {
    let mut counts: Vec<(&str, u32)> = Vec::new();
    for id in ids {
        match counts.iter_mut().find(|(i, _)| *i == id.as_str()) {
            Some((_, n)) => *n += 1,
            None => counts.push((id.as_str(), 1)),
        }
    }
    counts
        .iter()
        .map(|(id, n)| format!("{n}× {}", name(id)))
        .collect::<Vec<_>>()
        .join(", ")
}

// ---------------------------------------------------------------------------
// Small toolkit for panel content

pub fn text_font(font: &Handle<Font>, size: f32) -> TextFont {
    TextFont {
        font: font.clone().into(),
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
        assert_eq!(count_list(&ids, |s| s), "3× a, 1× b");
    }
}
