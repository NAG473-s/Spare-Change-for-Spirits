//! User interface: top bar, side panel per phase, message log.
//!
//! Buttons carry an `ActionButton` component. A global observer turns clicks
//! into `Action` messages that the individual plugins read.

use std::collections::{HashMap, VecDeque};

use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::picking::hover::{HoverMap, Hovered};
use bevy::prelude::*;
use bevy::ui::InteractionDisabled;
use bevy::ui::widget::NodeImageMode;
use bevy::ui_widgets::{Activate, Button};

use crate::data::{GameData, asset_exists};
use crate::day_cycle::{NightClock, Phase, Progress};
use crate::economy::{Coin, Drawer};
use crate::ferry::BoughtTonight;
use crate::fx::Cue;
use crate::ghosts::{Ghost, GhostState, Queue};
use crate::harbor::Location;
use crate::i18n::Lang;
use crate::inventory::{CratesOrdered, Stock};
use crate::register::Register;
use crate::save::{Knob, Slot};
use crate::shop::{ShopLayout, Storeroom, Tool};
use crate::upgrades::Upgrades;

pub(crate) const PANEL_WIDTH: f32 = 380.0;
/// Height of the message log below the side panel.
pub(crate) const LOG_HEIGHT: f32 = 118.0;
pub(crate) const BG_PANEL: Color = Color::srgb(0.11, 0.08, 0.12);
pub(crate) const PANEL_EDGE: Color = Color::srgb(0.39, 0.27, 0.19);
pub(crate) const TEXT: Color = Color::srgb(0.95, 0.89, 0.79);
pub(crate) const TEXT_DIM: Color = Color::srgb(0.76, 0.68, 0.61);
const ACCENT: Color = Color::srgb(0.98, 0.78, 0.49);
const INK: Color = Color::srgb(0.13, 0.08, 0.06);
/// Size of the pictures on product and decor buttons.
const ICON_SIZE: f32 = 24.0;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Action>()
            .init_resource::<UiFont>()
            .init_resource::<GameLog>()
            .init_resource::<Icons>()
            .init_resource::<UiTheme>()
            .add_systems(Startup, spawn_ui)
            // Before Update, so every rebuild this frame already uses the new font.
            .add_systems(PreUpdate, apply_language.run_if(resource_changed::<Lang>))
            .add_observer(on_activate)
            .add_observer(on_scroll)
            .add_systems(
                StateTransition,
                scroll_panel_to_top.run_if(state_changed::<Phase>),
            )
            .add_systems(OnEnter(Phase::Morning), build_morning_panel)
            .add_systems(OnEnter(Phase::Day), build_market_panel)
            .add_systems(OnEnter(Phase::Menu), clear_for_menu)
            .add_systems(OnEnter(Phase::DemoEnd), build_demo_end_panel)
            .add_systems(
                Update,
                (
                    show_more_below,
                    update_hud,
                    update_coin_bar,
                    send_scroll_events,
                    update_log.run_if(resource_changed::<GameLog>),
                    refresh_static_fonts.run_if(resource_changed::<UiFont>),
                    build_morning_panel.run_if(
                        in_state(Phase::Morning).and_then(
                            resource_changed::<UiFont>
                                .or_else(resource_changed::<crate::save::Settings>),
                        ),
                    ),
                    build_demo_end_panel
                        .run_if(in_state(Phase::DemoEnd).and_then(resource_changed::<UiFont>)),
                    button_hover,
                    reputation_tooltip,
                    refresh_change.run_if(in_state(Phase::Night).and_then(
                        resource_changed::<Register>.or_else(resource_changed::<Drawer>),
                    )),
                    build_market_panel.run_if(
                        in_state(Phase::Day).and_then(
                            resource_changed::<Storeroom>
                                .or_else(resource_changed::<UiFont>)
                                .or_else(resource_changed::<crate::market::MarketDay>)
                                .or_else(resource_changed::<Drawer>),
                        ),
                    ),
                    build_evening_panel.run_if(
                        in_state(Phase::Evening).and_then(
                            state_changed::<Phase>
                                .or_else(resource_changed::<Stock>)
                                .or_else(resource_changed::<Tool>)
                                .or_else(resource_changed::<ShopLayout>)
                                .or_else(resource_changed::<Drawer>)
                                .or_else(resource_changed::<Storeroom>)
                                .or_else(resource_changed::<Progress>)
                                .or_else(resource_changed::<BoughtTonight>)
                                .or_else(resource_changed::<Location>)
                                .or_else(resource_changed::<UiFont>),
                        ),
                    ),
                    build_night_panel.run_if(
                        in_state(Phase::Night).and_then(
                            state_changed::<Phase>
                                .or_else(resource_changed::<Register>)
                                .or_else(resource_changed::<Queue>)
                                .or_else(resource_changed::<UiFont>)
                                .or_else(resource_changed::<crate::save::Settings>)
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
    ToolPlace(String),
    ToolStock(String),
    ToolRemove,
    StartNight,
    Offer(String),
    /// Hand the ferryman's parcel to the story ghost at the counter.
    HandParcel,
    /// The drawer cannot give the change: ask the ghost for smaller coins.
    NoChange,
    /// The wish is not in stock: the ghost orders it for tomorrow.
    OrderForTomorrow,
    /// Give the ferryman his fare: one coin from the drawer.
    GiveFare,
    /// The achievements page of the pause menu.
    OpenAchievements,
    /// The ghost book (pauses the game).
    OpenBook,
    /// Put a ware in the shop window, or empty it.
    Showcase(Option<String>),
    /// List a trader's wares in the side panel (`None`: all traders).
    OpenStall(Option<usize>),
    /// Buy the n-th ware of the day from a trader's list.
    MarketBuy(usize, usize),
    /// Search the old junk for something broken (once an evening).
    Rummage,
    /// Repair a broken thing at the workbench.
    Repair(String),
    /// The season is over: keep the shop open anyway.
    KeepPlaying,
    Decline,
    /// Inspect (and reject, if fake) the paid coin with this index.
    CheckCoin(usize),
    AddChange(Coin),
    ResetChange,
    GiveChange,
    KeepSecondPayment,
    RefuseSecondPayment,
    NextEvening,
    /// Out through the shop door to the harbour, and back (evening).
    GoToHarbor,
    BackToShop,
    /// Leave the village market for the evening in the shop.
    EndDay,
    /// A board beside the shop door whose way is closed now: say when it
    /// opens.
    WayHint(crate::harbor::Way),
    /// Main menu: newest save over all profiles.
    Continue,
    /// Main menu pages: the list of saves, and the profile for a new game.
    OpenLoad,
    OpenNewGame,
    CloseSaveMenu,
    /// Start a new game in this profile (asks first if it has saves).
    NewGameIn(u8),
    /// Confirmed: delete the profile's saves and start a new game in it.
    ConfirmNewGame(u8),
    LoadSlot(u8, Slot),
    /// Pause menu: the manual save slots of the running profile.
    OpenSave,
    SaveToSlot(u8),
    /// The menu button in the top bar: pause without the keyboard.
    Pause,
    Resume,
    QuitToMenu,
    QuitGame,
    ToggleTips,
    ToggleSound,
    /// The settings page in the main and pause menus.
    OpenSettings,
    CloseSettings,
    /// Turn a setting one step up (`true`) or down.
    Adjust(Knob, bool),
    ToggleFullscreen,
    TogglePatientGhosts,
    ToggleChangeHint,
    OpenLanguages,
    CloseLanguages,
    SetLanguage(Option<String>),
}

#[derive(Component, Clone)]
pub struct ActionButton(pub Action);

/// Hand-painted walnut and brass used by the shop panels.
#[derive(Resource)]
pub(crate) struct UiTheme(Option<Handle<Image>>);

impl FromWorld for UiTheme {
    fn from_world(world: &mut World) -> Self {
        const FRAME: &str = "sprites/ui/ledger_frame.png";
        let image = asset_exists(FRAME).then(|| world.resource::<AssetServer>().load(FRAME));
        Self(image)
    }
}

impl UiTheme {
    pub(crate) fn frame(&self, corner_scale: f32) -> Option<ImageNode> {
        let image = self.0.as_ref()?;
        let slicer = TextureSlicer {
            // The leaf ornaments fit inside the corners of the 512 px painting.
            border: BorderRect::all(86.0),
            center_scale_mode: SliceScaleMode::Stretch,
            sides_scale_mode: SliceScaleMode::Stretch,
            max_corner_scale: corner_scale,
        };
        Some(ImageNode::new(image.clone()).with_mode(NodeImageMode::Sliced(slicer)))
    }

    /// Give the artwork the whole frame. UI images use the content box,
    /// so the padding belongs on a separate layout node inside it.
    pub(crate) fn panel_body(
        &self,
        commands: &mut Commands,
        frame: Entity,
        corner_scale: f32,
        padding: f32,
        gap: f32,
    ) -> Entity {
        if let Some(image) = self.frame(corner_scale) {
            commands.entity(frame).insert(image);
        }
        commands
            .spawn((
                Node {
                    min_height: px(0),
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(px(padding)),
                    row_gap: px(gap),
                    ..default()
                },
                ChildOf(frame),
            ))
            .id()
    }
}

pub(crate) fn finish(top: Color, bottom: Color) -> BackgroundGradient {
    BackgroundGradient(vec![
        LinearGradient::to_bottom(vec![
            ColorStop::new(top, percent(0)),
            ColorStop::new(bottom, percent(100)),
        ])
        .into(),
    ])
}

#[derive(Component, Clone, Copy)]
pub(crate) enum ButtonTone {
    Walnut,
    Selected,
    Brass,
    Disabled,
}

impl ButtonTone {
    fn option(enabled: bool, selected: bool) -> Self {
        match (enabled, selected) {
            (false, _) => Self::Disabled,
            (true, true) => Self::Selected,
            (true, false) => Self::Walnut,
        }
    }

    pub(crate) fn paint(self, hovered: bool) -> (BackgroundGradient, BorderColor) {
        let (top, bottom, trim) = match (self, hovered) {
            (Self::Walnut, false) => ((60, 43, 46), (40, 28, 35), (103, 72, 57)),
            (Self::Walnut, true) => ((92, 62, 52), (57, 39, 42), (177, 127, 77)),
            (Self::Selected, false) => ((53, 84, 79), (33, 54, 53), (166, 129, 79)),
            (Self::Selected, true) => ((67, 97, 87), (42, 65, 62), (213, 165, 99)),
            (Self::Brass, false) => ((236, 193, 127), (181, 122, 72), (248, 210, 151)),
            (Self::Brass, true) => ((252, 218, 159), (202, 148, 87), (255, 227, 174)),
            (Self::Disabled, _) => ((48, 39, 44), (37, 29, 37), (72, 56, 50)),
        };
        let rgb = |(r, g, b)| Color::srgb_u8(r, g, b);
        (finish(rgb(top), rgb(bottom)), BorderColor::all(rgb(trim)))
    }
}

/// Small pictures for product and decor buttons: `sprites/products/<id>.png`
/// and `sprites/decor/<id>.png`, where they exist.
#[derive(Resource)]
pub struct Icons {
    products: HashMap<String, Handle<Image>>,
    decor: HashMap<String, Handle<Image>>,
    /// `sprites/coins/<coin>.png`, in the order of `Coin::ASCENDING`.
    coins: Vec<Option<Handle<Image>>>,
}

impl Icons {
    pub fn product(&self, id: &str) -> Option<Handle<Image>> {
        self.products.get(id).cloned()
    }

    pub fn decor(&self, id: &str) -> Option<Handle<Image>> {
        self.decor.get(id).cloned()
    }

    pub fn coin(&self, coin: Coin) -> Option<Handle<Image>> {
        let i = Coin::ASCENDING.iter().position(|c| *c == coin)?;
        self.coins.get(i).cloned().flatten()
    }
}

impl FromWorld for Icons {
    fn from_world(world: &mut World) -> Self {
        let data = world.resource::<GameData>();
        let product_ids: Vec<String> = data.products.iter().map(|p| p.id.clone()).collect();
        let decor_ids: Vec<String> = data.decor.iter().map(|d| d.id.clone()).collect();
        let assets = world.resource::<AssetServer>();
        let load = |folder: &str, ids: Vec<String>| {
            ids.into_iter()
                .filter_map(|id| {
                    let path = format!("sprites/{folder}/{id}.png");
                    asset_exists(&path).then(|| (id, assets.load(path)))
                })
                .collect()
        };
        let coins = Coin::ASCENDING
            .iter()
            .map(|c| {
                let path = format!("sprites/coins/{}.png", c.id());
                asset_exists(&path).then(|| assets.load(path))
            })
            .collect();
        Self {
            products: load("products", product_ids),
            decor: load("decor", decor_ids),
            coins,
        }
    }
}

/// Fonts for the current language: Inter first, then the language's own
/// script font (CJK, Thai, Arabic), then every other script font, so that
/// any character – e.g. language names in the language menu – can be drawn.
#[derive(Resource)]
pub struct UiFont {
    pub regular: FontSource,
    pub bold: FontSource,
    pub display: FontSource,
    inter: Handle<Font>,
    inter_bold: Handle<Font>,
    serif_bold: Handle<Font>,
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
            display: FontSource::default(),
            inter: assets.load("fonts/Inter-Regular.otf"),
            inter_bold: assets.load("fonts/Inter-SemiBold.otf"),
            serif_bold: assets.load("fonts/NotoSerif-Bold.ttf"),
            extras,
        };
        font.set_language(&code);
        font
    }
}

impl UiFont {
    fn chain(&self, bases: &[Handle<Font>], code: &str) -> FontSource {
        let own = self.extras.iter().filter(|(c, _)| c == code);
        let others = self.extras.iter().filter(|(c, _)| c != code);
        FontSource::list(
            bases
                .iter()
                .cloned()
                .chain(own.chain(others).map(|(_, h)| h.clone())),
        )
    }

    pub fn set_language(&mut self, code: &str) {
        self.regular = self.chain(std::slice::from_ref(&self.inter), code);
        self.bold = self.chain(std::slice::from_ref(&self.inter_bold), code);
        self.display = self.chain(&[self.serif_bold.clone(), self.inter_bold.clone()], code);
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
    /// Every line since the playtest log last took them.
    new: Vec<String>,
}

impl GameLog {
    pub fn clear(&mut self) {
        self.lines.clear();
    }

    /// The lines pushed since the last call (for the playtest log).
    pub fn take_new(&mut self) -> Vec<String> {
        std::mem::take(&mut self.new)
    }

    pub fn push(&mut self, line: impl Into<String>) {
        let line = line.into();
        info!("{line}");
        self.new.push(line.clone());
        self.lines.push_back(line);
        while self.lines.len() > 4 {
            self.lines.pop_front();
        }
    }
}

#[derive(Component)]
struct HudText;

/// Text of the menu button in the top bar.
#[derive(Component)]
struct MenuButtonLabel;

#[derive(Component)]
struct LogText;

#[derive(Component)]
struct SidePanel;

/// The two parts of the side panel.
#[derive(Resource, Clone, Copy)]
pub(crate) struct SidePanelParts {
    body: Entity,
    footer: Entity,
}

const SCROLL_LINE: f32 = 24.0;
/// How far a click on the "more below" arrow scrolls the side panel.
const MORE_SCROLL: f32 = 140.0;

/// The arrow under the side panel's body that says more is hidden below.
#[derive(Component)]
struct MoreBelow;

fn show_more_below(
    parts: Option<Res<SidePanelParts>>,
    bodies: Query<(&ComputedNode, &ScrollPosition)>,
    mut more: Single<&mut Node, With<MoreBelow>>,
) {
    let Some(parts) = parts else { return };
    let Ok((computed, scroll)) = bodies.get(parts.body) else {
        return;
    };
    let hidden = (computed.content_size().y - computed.size().y) * computed.inverse_scale_factor()
        - scroll.y;
    let display = if hidden > 4.0 {
        Display::Flex
    } else {
        Display::None
    };
    if more.display != display {
        more.display = display;
    }
}

fn scroll_panel_on(
    _click: On<PointerClick>,
    parts: Res<SidePanelParts>,
    mut bodies: Query<(&mut ScrollPosition, &ComputedNode)>,
) {
    if let Ok((mut scroll, computed)) = bodies.get_mut(parts.body) {
        let max = (computed.content_size().y - computed.size().y) * computed.inverse_scale_factor();
        scroll.y = (scroll.y + MORE_SCROLL).min(max.max(0.0));
    }
}

/// Mouse wheel scrolls whatever scrollable UI node is under the pointer.
fn send_scroll_events(
    mut wheel: MessageReader<MouseWheel>,
    hover_map: Res<HoverMap>,
    mut commands: Commands,
) {
    for event in wheel.read() {
        let mut delta = -Vec2::new(event.x, event.y);
        if event.unit == MouseScrollUnit::Line {
            delta *= SCROLL_LINE;
        }
        for pointer_map in hover_map.values() {
            for entity in pointer_map.keys().copied() {
                commands.trigger(Scroll { entity, delta });
            }
        }
    }
}

#[derive(EntityEvent, Debug)]
#[entity_event(propagate, auto_propagate)]
struct Scroll {
    entity: Entity,
    delta: Vec2,
}

fn on_scroll(
    mut scroll: On<Scroll>,
    mut nodes: Query<(&mut ScrollPosition, &Node, &ComputedNode)>,
) {
    let Ok((mut position, node, computed)) = nodes.get_mut(scroll.entity) else {
        return;
    };
    if node.overflow.y != OverflowAxis::Scroll || scroll.delta.y == 0.0 {
        return;
    }
    let max = ((computed.content_size() - computed.size()) * computed.inverse_scale_factor()).y;
    position.y = (position.y + scroll.delta.y).clamp(0.0, max.max(0.0));
    scroll.propagate(false);
}

/// A new phase starts at the top of the side panel.
fn scroll_panel_to_top(
    parts: Option<Res<SidePanelParts>>,
    mut positions: Query<&mut ScrollPosition>,
) {
    if let Some(parts) = parts
        && let Ok(mut p) = positions.get_mut(parts.body)
    {
        p.y = 0.0;
    }
}

fn spawn_ui(mut commands: Commands, font: Res<UiFont>, icons: Res<Icons>, theme: Res<UiTheme>) {
    // Top bar
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                height: px(40),
                padding: UiRect::horizontal(px(16)),
                border: UiRect::bottom(px(1)),
                align_items: AlignItems::Center,
                ..default()
            },
            finish(Color::srgb_u8(51, 36, 42), Color::srgb_u8(28, 21, 29)),
            BorderColor::all(PANEL_EDGE),
            children![
                (
                    HudText,
                    Hovered::default(),
                    StaticText { bold: true },
                    Text::new(""),
                    text_font(&font.bold, 15.0),
                    TextColor(TEXT),
                    Node {
                        flex_grow: 1.0,
                        ..default()
                    },
                ),
                coin_bar(&font, &icons),
            ],
        ))
        .with_children(|bar| {
            // Pauses with the mouse alone (Esc does the same).
            let (surface, trim) = ButtonTone::Walnut.paint(false);
            bar.spawn((
                Button,
                Hovered::default(),
                ActionButton(Action::Pause),
                ButtonTone::Walnut,
                Node {
                    margin: UiRect::left(px(12)),
                    padding: UiRect::axes(px(10), px(4)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(3)),
                    ..default()
                },
                surface,
                trim,
                children![(
                    MenuButtonLabel,
                    StaticText { bold: false },
                    Text::new(""),
                    text_font(&font.regular, 13.0),
                    TextColor(TEXT),
                    Pickable::IGNORE,
                )],
            ));
        });

    // Side panel: a scrolling body (rebuilt per phase) and a fixed footer for
    // the main button, so it can never be pushed off screen.
    let body = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_height: px(0),
                flex_direction: FlexDirection::Column,
                row_gap: px(6),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            ScrollPosition::default(),
        ))
        .id();
    let footer = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(6),
            padding: UiRect::top(px(8)),
            border: UiRect::top(px(1)),
            ..default()
        })
        .insert(BorderColor::all(PANEL_EDGE))
        .id();
    let frame = commands
        .spawn((
            SidePanel,
            Node {
                position_type: PositionType::Absolute,
                right: px(0),
                top: px(40),
                bottom: px(LOG_HEIGHT),
                width: px(PANEL_WIDTH),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(BG_PANEL),
        ))
        .id();
    // "There is more below": shown while the body hides content under its
    // lower edge; a click scrolls on.
    let more = commands
        .spawn((
            MoreBelow,
            Node {
                justify_content: JustifyContent::Center,
                padding: UiRect::vertical(px(1)),
                display: Display::None,
                ..default()
            },
            children![(
                Text::new("↓"),
                text_font(&font.bold, 15.0),
                TextColor(TEXT_DIM),
                Pickable::IGNORE,
            )],
        ))
        .observe(scroll_panel_on)
        .id();
    let content = theme.panel_body(&mut commands, frame, 0.27, 16.0, 0.0);
    commands.entity(content).add_children(&[body, more, footer]);
    commands.insert_resource(SidePanelParts { body, footer });

    // Message log: in the right column below the side panel, so it never
    // covers the shop. The newest line is at the bottom; older ones are cut
    // off at the top when they do not fit.
    let log_frame = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: px(0),
                bottom: px(0),
                width: px(PANEL_WIDTH),
                height: px(LOG_HEIGHT),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::End,
                padding: UiRect::axes(px(14), px(8)),
                border: UiRect::top(px(1)),
                overflow: Overflow::clip(),
                ..default()
            },
            finish(Color::srgb_u8(51, 36, 42), Color::srgb_u8(28, 21, 29)),
            BorderColor::all(PANEL_EDGE),
            Pickable::IGNORE,
        ))
        .id();
    commands.spawn((
        LogText,
        StaticText { bold: false },
        Text::new(""),
        text_font(&font.regular, 12.0),
        TextColor(TEXT_DIM),
        Node {
            flex_shrink: 0.0,
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(log_frame),
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
            &ButtonTone,
            &mut BackgroundGradient,
            &mut BorderColor,
            Has<InteractionDisabled>,
        ),
        Changed<Hovered>,
    >,
) {
    for (hovered, tone, mut bg, mut edge, disabled) in &mut buttons {
        (*bg, *edge) = tone.paint(hovered.get() && !disabled);
    }
}

/// Hovering the top bar explains what the reputation does right now.
fn reputation_tooltip(
    hud: Single<&Hovered, With<HudText>>,
    progress: Res<Progress>,
    data: Res<GameData>,
    lang: Res<Lang>,
    mut tip: ResMut<crate::tooltip::HoverTip>,
    mut shown: Local<bool>,
) {
    use crate::tooltip::{HoverTip, TipKind};
    if !hud.get() {
        if *shown {
            HoverTip::hide(&mut tip);
            *shown = false;
        }
        return;
    }
    *shown = true;
    let rep = progress.reputation;
    let mut t = HoverTip {
        title: lang.f("rep_tip.title", &[("reputation", &rep)]),
        ..default()
    };
    // The factor is on the time between ghosts: its inverse is how often they come.
    let percent = ((1.0 / data.config.reputation_spawn_factor(rep) - 1.0) * 100.0).round() as i32;
    match percent {
        p if p > 0 => t.line(TipKind::Good, lang.f("rep_tip.more", &[("percent", &p)])),
        p if p < 0 => t.line(TipKind::Bad, lang.f("rep_tip.fewer", &[("percent", &-p)])),
        _ => t.line(TipKind::Text, lang.t("rep_tip.same")),
    };
    let perks = data.config.reputation_perks;
    let chance = (perks.tip_chance(rep) * 100.0).round() as u32;
    if chance > 0 {
        t.line(
            TipKind::Good,
            lang.f("rep_tip.tips", &[("percent", &chance)]),
        );
    } else {
        t.line(
            TipKind::Text,
            lang.f("rep_tip.tips_from", &[("min", &perks.tip_from)]),
        );
    }
    for g in data
        .ghosts
        .iter()
        .filter(|g| g.unlock_night <= progress.calendar(&data))
    {
        let Some(min) = g.min_reputation else {
            continue;
        };
        let name = lang.ghost(&g.id);
        if rep >= min {
            t.line(TipKind::Good, lang.f("rep_tip.guest", &[("ghost", &name)]));
        } else {
            t.line(
                TipKind::Text,
                lang.f("rep_tip.guest_from", &[("ghost", &name), ("min", &min)]),
            );
        }
    }
    if rep >= perks.rare_from {
        t.line(TipKind::Good, lang.t("rep_tip.rare"));
    } else {
        t.line(
            TipKind::Text,
            lang.f("rep_tip.rare_from", &[("min", &perks.rare_from)]),
        );
    }
    if progress.calendar(&data) >= data.config.thief.from_night && rep < data.config.thief.below {
        t.line(TipKind::Bad, lang.t("rep_tip.thief"));
    }
    t.line(TipKind::Hint, lang.t("rep_tip.earn"));
    HoverTip::show(&mut tip, t);
}

fn update_hud(
    lang: Res<Lang>,
    phase: Res<State<Phase>>,
    data: Res<GameData>,
    progress: Res<Progress>,
    clock: Res<NightClock>,
    mut hud: Single<&mut Text, With<HudText>>,
    mut menu: Single<&mut Text, (With<MenuButtonLabel>, Without<HudText>)>,
) {
    let label = lang.t("hud.menu");
    if menu.0 != label {
        menu.0 = label;
    }
    if *phase.get() == Phase::Menu {
        hud.0.clear();
        return;
    }
    let time = if *phase.get() == Phase::Night {
        format!(" {}", clock.time_string())
    } else {
        String::new()
    };
    // Past the last night (the story still running, or after the season)
    // the night count has no end.
    // A season with a finale counts up to its last night; past it (the
    // finale still open), or in a season without a story, the count is open.
    let season_end = if cfg!(feature = "demo") {
        data.config.final_night()
    } else {
        progress.season_end(&data)
    };
    let key = if progress.season_has_finale(&data) && progress.night <= season_end {
        "hud"
    } else {
        "hud_open"
    };
    hud.0 = lang.f(
        key,
        &[
            ("night", &progress.night),
            ("nights", &season_end),
            ("phase", &lang.t(phase.get().key())),
            ("time", &time),
            ("reputation", &progress.reputation),
        ],
    );
}

/// Count of one coin in the coin bar.
#[derive(Component)]
struct CoinCount(Coin);

/// Total value in the coin bar.
#[derive(Component)]
struct CoinTotal;

#[derive(Component)]
struct CoinBar;

/// The cash drawer at a glance, top right: a picture and the count of each
/// coin, then the total. Visible in every phase of the game.
fn coin_bar(font: &UiFont, icons: &Icons) -> impl Bundle {
    let coin = |c: Coin| {
        (
            Node {
                align_items: AlignItems::Center,
                column_gap: px(4),
                ..default()
            },
            children![
                (
                    ImageNode::new(icons.coin(c).unwrap_or_default()),
                    Node {
                        width: px(24),
                        height: px(24),
                        ..default()
                    },
                ),
                (
                    CoinCount(c),
                    StaticText { bold: true },
                    Text::new("0"),
                    text_font(&font.bold, 15.0),
                    TextColor(TEXT),
                ),
            ],
        )
    };
    (
        CoinBar,
        Node {
            align_items: AlignItems::Center,
            column_gap: px(14),
            padding: UiRect::axes(px(12), px(3)),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(6)),
            ..default()
        },
        BackgroundColor(Color::srgb_u8(35, 25, 31)),
        BorderColor::all(PANEL_EDGE),
        children![
            coin(Coin::Spoon),
            coin(Coin::Candle),
            coin(Coin::Button),
            (
                CoinTotal,
                StaticText { bold: true },
                Text::new(""),
                text_font(&font.bold, 15.0),
                TextColor(ACCENT),
            ),
        ],
    )
}

/// Keeps the coin bar up to date; the total flashes green when money comes
/// in and red when it goes out.
fn update_coin_bar(
    time: Res<Time<Real>>,
    drawer: Res<Drawer>,
    phase: Res<State<Phase>>,
    mut bar: Single<&mut Visibility, With<CoinBar>>,
    mut counts: Query<(&CoinCount, &mut Text), Without<CoinTotal>>,
    mut total: Single<(&mut Text, &mut TextColor), With<CoinTotal>>,
    mut last: Local<Option<u32>>,
    mut flash: Local<(f32, bool)>,
) {
    **bar = if *phase.get() == Phase::Menu {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    let now = drawer.total();
    if drawer.is_changed() {
        for (count, mut text) in &mut counts {
            text.0 = drawer.get(count.0).to_string();
        }
        total.0.0 = format!("= {now}");
        if let Some(before) = *last
            && before != now
        {
            *flash = (1.0, now > before);
        }
        *last = Some(now);
    }
    flash.0 = (flash.0 - time.delta_secs() * 1.5).max(0.0);
    let to = if flash.1 {
        Color::srgb(0.55, 0.95, 0.6)
    } else {
        Color::srgb(1.0, 0.5, 0.45)
    };
    total.1.0 = ACCENT.mix(&to, flash.0);
}

/// The menu starts from a clean screen: no side panel, no old messages.
/// (Runs once before Startup too, when the panel does not exist yet.)
fn clear_for_menu(
    mut commands: Commands,
    parts: Option<Res<SidePanelParts>>,
    mut log: ResMut<GameLog>,
) {
    let Some(parts) = parts else { return };
    commands.entity(parts.body).despawn_related::<Children>();
    commands.entity(parts.footer).despawn_related::<Children>();
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

/// The side panel at the village market: who sells what, the storeroom,
/// and the way back to the shop.
#[allow(clippy::too_many_arguments)]
fn build_market_panel(
    mut commands: Commands,
    parts: Res<SidePanelParts>,
    font: Res<UiFont>,
    lang: Res<Lang>,
    data: Res<GameData>,
    storeroom: Res<Storeroom>,
    day: Res<crate::market::MarketDay>,
    drawer: Res<Drawer>,
    icons: Res<Icons>,
    progress: Res<Progress>,
) {
    let mut ui = PanelBuilder::side(&mut commands, *parts, &font);
    // One trader's wares, or the overview of all traders here today.
    if let Some(stall) = day.stall {
        crate::market::stall_panel(
            &mut ui,
            &lang,
            &data,
            &day,
            &drawer,
            &icons,
            progress.reputation,
            stall,
        );
        ui.spacer();
        ui.primary(&lang.t("market.back"), Action::EndDay);
        return;
    }
    ui.heading(&lang.t("market.title"));
    ui.dim(&lang.t("market.intro"));
    ui.section(&lang.t("market.vendors"));
    let calendar = progress.calendar(&data);
    let root = ui.root();
    for (i, v) in data.vendors.iter().enumerate() {
        if !v.in_season(calendar) {
            continue;
        }
        ui.menu_option(
            root,
            &lang.f(
                "market.show_wares",
                &[("vendor", &lang.t(&format!("vendor.{}.name", v.id)))],
            ),
            Action::OpenStall(Some(i)),
            false,
        );
        ui.dim(&lang.t(&format!("vendor.{}.text", v.id)));
    }
    ui.section(&lang.t("market.storeroom"));
    let owned: Vec<String> = data
        .decor
        .iter()
        .filter(|d| storeroom.count(&d.id) > 0)
        .map(|d| {
            lang.f(
                "count.entry",
                &[
                    ("count", &storeroom.count(&d.id)),
                    ("item", &lang.decor(&d.id)),
                ],
            )
        })
        .collect();
    if owned.is_empty() {
        ui.dim(&lang.t("market.storeroom_empty"));
    } else {
        ui.text(&lang.join(&owned));
    }
    ui.dim(&lang.t("market.hint"));
    ui.spacer();
    ui.primary(&lang.t("market.back"), Action::EndDay);
}

fn build_evening_panel(
    mut commands: Commands,
    parts: Res<SidePanelParts>,
    font: Res<UiFont>,
    lang: Res<Lang>,
    data: Res<GameData>,
    stock: Res<Stock>,
    crates: Res<CratesOrdered>,
    tool: Res<Tool>,
    layout: Res<ShopLayout>,
    storeroom: Res<Storeroom>,
    progress: Res<Progress>,
    icons: Res<Icons>,
    bought: Res<BoughtTonight>,
    location: Res<Location>,
    upgrades: Res<Upgrades>,
    (letter, drawer): (Res<crate::market::FoundLetter>, Res<Drawer>),
) {
    let night = progress.calendar(&data);
    let mut ui = PanelBuilder::side(&mut commands, *parts, &font);
    let max_crates = upgrades.crates_tonight(&data, night);
    let bought_units = bought.units(&data);

    if *location == Location::Harbor {
        ui.heading(&lang.t("harbor.title"));
        ui.dim(&lang.t("harbor.intro"));
        ui.section(&lang.f(
            "evening.ferryman",
            &[("count", &crates.0), ("max", &max_crates)],
        ));
        ui.dim(&lang.t("ferry.tip_controls"));
        if !bought_units.is_empty() {
            ui.text(&lang.f(
                "evening.bought",
                &[(
                    "items",
                    &count_list(&lang, &bought_units, |p| lang.product(p)),
                )],
            ));
        }
        let odds = crate::sets::odds_tonight(&data, &layout, &progress, &stock);
        expected_guests(&mut ui, &lang, &data, &stock, &progress, &layout, &odds);
        ui.section(&lang.t("evening.shelve"));
        let in_stock: Vec<String> = data
            .products
            .iter()
            .flat_map(|p| std::iter::repeat_n(p.id.clone(), stock.count(&p.id) as usize))
            .collect();
        if in_stock.is_empty() {
            ui.dim(&lang.t("evening.storeroom_empty"));
        } else {
            ui.text(&count_list(&lang, &in_stock, |p| lang.product(p)));
        }
        ui.spacer();
        ui.primary(&lang.t("harbor.back"), Action::BackToShop);
        return;
    }

    ui.heading(&lang.t("evening.title"));

    // Hedda's letters: the first waits on the first evening, the others
    // turn up behind the junk her improvements clear.
    let letter = if night == 1 { Some(1) } else { letter.0 };
    if let Some(n) = letter {
        ui.section(&lang.t("letter.title"));
        ui.text(&lang.t(&format!("letter.{n}")));
    }

    if let Some(event) = data.event(night) {
        ui.section(&lang.f(
            "evening.tonight",
            &[("name", &lang.t(&format!("event.{}.name", event.id)))],
        ));
        ui.dim(&lang.t(&format!("event.{}.text", event.id)));
    }
    if night > 1 {
        let (products, decor, ghosts) = data.unlocked_on(night);
        let names: Vec<String> = ghosts
            .iter()
            .map(|g| lang.ghost(g))
            .chain(products.iter().map(|p| lang.product(p)))
            .chain(decor.iter().map(|d| lang.decor(d)))
            .collect();
        if !names.is_empty() {
            ui.dim(&lang.f("evening.new_tonight", &[("items", &lang.join(&names))]));
        }
    }

    let odds = crate::sets::odds_tonight(&data, &layout, &progress, &stock);
    expected_guests(&mut ui, &lang, &data, &stock, &progress, &layout, &odds);
    let root = ui.root();
    ui.button_in(root, &lang.t("book.open"), Action::OpenBook, false, true);

    ui.section(&lang.f(
        "evening.ferryman",
        &[("count", &crates.0), ("max", &max_crates)],
    ));
    ui.dim(&lang.t("evening.ferry_hint"));
    let root = ui.root();
    ui.button_in(root, &lang.t("harbor.go"), Action::GoToHarbor, false, true);
    if !bought_units.is_empty() {
        ui.text(&lang.f(
            "evening.bought",
            &[(
                "items",
                &count_list(&lang, &bought_units, |p| lang.product(p)),
            )],
        ));
    }

    // Furniture from the storeroom (bought at the market, or keepsakes).
    ui.section(&lang.t("evening.furnish"));
    let owned: Vec<_> = data
        .decor
        .iter()
        .filter(|d| storeroom.count(&d.id) > 0)
        .collect();
    let row = ui.row();
    for d in &owned {
        let selected = *tool == Tool::Place(d.id.clone());
        let label = lang.f(
            "evening.stock_button",
            &[
                ("item", &lang.decor(&d.id)),
                ("count", &storeroom.count(&d.id)),
            ],
        );
        ui.icon_button_in(
            row,
            icons.decor(&d.id),
            &label,
            Action::ToolPlace(d.id.clone()),
            selected,
            true,
        );
    }
    ui.button_in(
        row,
        &lang.t("evening.remove"),
        Action::ToolRemove,
        *tool == Tool::Remove,
        true,
    );
    if owned.is_empty() {
        ui.dim(&lang.t("evening.no_furniture"));
    }
    ui.dim(&lang.t("evening.drag_hint"));
    // Part of the floor is still blocked by old junk.
    if upgrades.area(&data) != (data.config.grid_cols, data.config.grid_rows) {
        ui.dim(&lang.t("evening.junk"));
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
        ui.icon_button_in(
            row,
            icons.product(&p.id),
            &label,
            Action::ToolStock(p.id.clone()),
            selected,
            true,
        );
    }
    if !any {
        ui.dim(&lang.t("evening.storeroom_empty"));
    }

    showcase_section(&mut ui, &lang, &data, &stock, &progress, &icons);
    sets_section(&mut ui, &lang, &data, &layout, &storeroom);

    workbench_section(
        &mut ui, &lang, &data, &layout, &progress, &drawer, &upgrades,
    );

    ui.section(&lang.t("evening.atmosphere"));
    ui.text(&layout.atmosphere(&data, night).describe(&lang));
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

/// How many ghost types the evening forecast lists.
const EXPECTED_GUESTS: usize = 3;

/// The ghost types most likely to come tonight, how likely, and what they
/// ask for – so the player knows what to order and put on the shelves.
fn expected_guests(
    ui: &mut PanelBuilder,
    lang: &Lang,
    data: &GameData,
    stock: &Stock,
    progress: &Progress,
    layout: &ShopLayout,
    odds: &[f32],
) {
    let night = progress.calendar(data);
    let total: f32 = odds.iter().sum();
    if total <= 0.0 {
        return;
    }
    let mut likely: Vec<usize> = (0..odds.len()).filter(|&i| odds[i] > 0.0).collect();
    likely.sort_by(|&a, &b| odds[b].total_cmp(&odds[a]));
    ui.section(&lang.t("evening.expected"));
    // Orders for tonight: these ghosts come for sure.
    for o in progress.orders.iter().filter(|o| o.night == progress.night) {
        let item = match stock.count(&o.product) {
            0 => lang.product(&o.product),
            n => lang.f(
                "evening.stock_button",
                &[("item", &lang.product(&o.product)), ("count", &n)],
            ),
        };
        ui.text(&lang.f(
            "evening.order",
            &[("who", &lang.ghost(&o.ghost)), ("item", &item)],
        ));
    }
    // Story ghosts come every night until they have what they need: their
    // wish is announced, and the ferryman's parcel is mentioned.
    for (i, story) in data.stories.iter().enumerate() {
        if !progress.story_due(data, i) {
            continue;
        }
        let name = lang.ghost(&story.ghost);
        let chapter = progress.chapter(i);
        let mut wish = lang.t(&format!("story.{}.{}.intro", story.ghost, chapter + 1));
        if progress.asks_plainly(i) {
            let c = &story.chapters[chapter];
            let plainly = if c.fare {
                lang.t("story.fare_plainly")
            } else {
                lang.f("story.plainly", &[("item", &lang.product(&c.wish))])
            };
            wish = format!("{wish} {plainly}");
        }
        ui.text(&lang.f(
            "evening.story_comes",
            &[
                ("name", &name),
                ("text", &lang.f("quote", &[("text", &wish)])),
            ],
        ));
        if progress.parcels.contains(&i) {
            ui.dim(&lang.f("evening.parcel_waits", &[("name", &name)]));
        }
    }
    for &i in likely.iter().take(EXPECTED_GUESTS) {
        let ghost = &data.ghosts[i];
        let wishes: Vec<String> = data
            .wishes_tonight(ghost, night)
            .into_iter()
            .map(|w| match stock.count(w) {
                0 => lang.product(w),
                n => lang.f(
                    "evening.stock_button",
                    &[("item", &lang.product(w)), ("count", &n)],
                ),
            })
            .collect();
        let percent = (odds[i] / total * 100.0).round() as u32;
        ui.text(&lang.f(
            "evening.expected_line",
            &[
                ("ghost", &lang.ghost(&ghost.id)),
                ("percent", &percent),
                ("wishes", &lang.join(&wishes)),
            ],
        ));
    }
    // Ghosts the atmosphere will send away again, and why.
    let atmosphere = layout.atmosphere(data, night);
    let away: Vec<String> = data
        .ghosts
        .iter()
        .enumerate()
        .filter(|(i, _)| odds[*i] > 0.0)
        .filter_map(|(_, g)| {
            g.unmet_need(&atmosphere)
                .map(|n| format!("{} ({})", lang.ghost(&g.id), n.complaint(lang)))
        })
        .collect();
    if !away.is_empty() {
        ui.dim(&lang.f("evening.turn_away", &[("list", &lang.join(&away))]));
    }
    ui.dim(&lang.t("evening.expected_hint"));
}

/// The shop window: one ware from stock draws the ghosts who wish for it.
fn showcase_section(
    ui: &mut PanelBuilder,
    lang: &Lang,
    data: &GameData,
    stock: &Stock,
    progress: &Progress,
    icons: &Icons,
) {
    ui.section(&lang.t("showcase.title"));
    let chosen = progress.journal.showcase.as_deref();
    match chosen {
        Some(w) if stock.count(w) > 0 => {
            ui.text(&lang.f("showcase.shows", &[("item", &lang.product(w))]))
        }
        Some(w) => ui.text(&lang.f("showcase.sold_out", &[("item", &lang.product(w))])),
        None => ui.text(&lang.t("showcase.empty")),
    };
    let row = ui.row();
    for p in data.products.iter().filter(|p| stock.count(&p.id) > 0) {
        ui.icon_button_in(
            row,
            icons.product(&p.id),
            &lang.product(&p.id),
            Action::Showcase(Some(p.id.clone())),
            chosen == Some(p.id.as_str()),
            true,
        );
    }
    if chosen.is_some() {
        ui.button_in(
            row,
            &lang.t("showcase.clear"),
            Action::Showcase(None),
            false,
            true,
        );
    }
    ui.dim(&lang.t("showcase.hint"));
}

/// Furniture sets the player has started: how far each is, and what it
/// brings once complete (in words, not by colour).
fn sets_section(
    ui: &mut PanelBuilder,
    lang: &Lang,
    data: &GameData,
    layout: &ShopLayout,
    storeroom: &Storeroom,
) {
    let started: Vec<_> = data
        .sets
        .iter()
        .filter(|s| s.started(layout, storeroom))
        .collect();
    if started.is_empty() {
        return;
    }
    ui.section(&lang.t("sets.title"));
    for set in started {
        let name = lang.t(&format!("set.{}.name", set.id));
        let pieces: Vec<String> = set.decor.iter().map(|d| lang.decor(d)).collect();
        let bonus = set_bonus(lang, set);
        if set.complete(layout) {
            ui.text(&lang.f("sets.complete", &[("name", &name), ("bonus", &bonus)]));
        } else {
            ui.text(&lang.f(
                "sets.progress",
                &[
                    ("name", &name),
                    ("placed", &set.placed(layout)),
                    ("total", &set.decor.len()),
                    ("bonus", &bonus),
                ],
            ));
        }
        ui.dim(&lang.f("sets.pieces", &[("items", &lang.join(&pieces))]));
    }
}

/// What a complete set brings, in words.
pub fn set_bonus(lang: &Lang, set: &crate::sets::SetDef) -> String {
    let ghosts: Vec<String> = set
        .ghost_weights
        .iter()
        .map(|(g, _)| lang.ghost(g))
        .collect();
    let mut parts = Vec::new();
    if !ghosts.is_empty() {
        parts.push(lang.f("sets.draws", &[("ghosts", &lang.join(&ghosts))]));
    }
    let atmosphere = set.atmosphere.changes(lang);
    if !atmosphere.is_empty() {
        parts.push(atmosphere);
    }
    lang.join(&parts)
}

/// The workbench: search the junk, repair one broken thing an evening.
fn workbench_section(
    ui: &mut PanelBuilder,
    lang: &Lang,
    data: &GameData,
    layout: &ShopLayout,
    progress: &Progress,
    drawer: &Drawer,
    upgrades: &Upgrades,
) {
    use crate::workbench::has_workbench;
    let junk_left = upgrades.area(data) != (data.config.grid_cols, data.config.grid_rows);
    if !has_workbench(layout) {
        if junk_left {
            ui.dim(&lang.t("workbench.buy_hint"));
        }
        return;
    }
    ui.section(&lang.t("workbench.title"));
    let row = ui.row();
    let rummaged = progress.rummaged_on == progress.night;
    ui.button_in(
        row,
        &lang.t("workbench.rummage"),
        Action::Rummage,
        false,
        !rummaged,
    );
    let repaired = progress.repaired_on == progress.night;
    for b in &progress.broken {
        let cost = data.workbench.cost(b).unwrap_or(0);
        let label = lang.f(
            "workbench.repair",
            &[("item", &lang.product(b)), ("cost", &cost)],
        );
        ui.button_in(
            row,
            &label,
            Action::Repair(b.clone()),
            false,
            !repaired && drawer.total() >= cost,
        );
    }
    if rummaged && repaired {
        ui.dim(&lang.t("workbench.done"));
    } else {
        ui.dim(&lang.t("workbench.hint"));
    }
}

/// What the night panel shows besides the register: the ghost at the
/// front and its state, and the length of the line while nobody waits.
#[derive(PartialEq)]
struct NightPanelShows {
    front: Option<(Entity, GhostState)>,
    line: usize,
    /// The sale at the register (its ghost and which coins were checked) and
    /// whether a ghost is asking about a second payment. Counting change does
    /// not rebuild the panel: `refresh_change` updates it in place, so a click
    /// is never lost to a button that was just replaced.
    sale: Option<(Entity, Vec<bool>)>,
    question: bool,
}

/// The line that shows the change counted so far.
#[derive(Component)]
struct CountedLine;

/// A button that adds one coin of this kind to the change.
#[derive(Component)]
struct ChangeCoin(Coin);

fn counted_line(lang: &Lang, sale: &crate::register::Sale) -> String {
    lang.f(
        "night.counted",
        &[
            ("coins", &sale.given.describe(lang)),
            ("total", &sale.given.total()),
        ],
    )
}

/// While change is counted: the counted line and what is left of each coin.
fn refresh_change(
    register: Res<Register>,
    drawer: Res<Drawer>,
    lang: Res<Lang>,
    mut commands: Commands,
    mut lines: Query<&mut Text, With<CountedLine>>,
    buttons: Query<(Entity, &ChangeCoin, &Children)>,
    mut texts: Query<&mut Text, Without<CountedLine>>,
    mut colors: Query<&mut TextColor>,
    mut images: Query<&mut ImageNode>,
) {
    let Some(sale) = &register.sale else {
        return;
    };
    for mut line in &mut lines {
        let want = counted_line(&lang, sale);
        if line.0 != want {
            line.0 = want;
        }
    }
    for (entity, ChangeCoin(coin), children) in &buttons {
        let left = drawer.get(*coin) - sale.given.get(*coin);
        let label = lang.f(
            "night.add_coin",
            &[
                ("coin", &coin.name(&lang)),
                ("value", &coin.value()),
                ("left", &left),
            ],
        );
        let enabled = left > 0;
        let tone = ButtonTone::option(enabled, false);
        let (surface, trim) = tone.paint(false);
        let mut button = commands.entity(entity);
        button.insert((tone, surface, trim));
        if enabled {
            button.remove::<InteractionDisabled>();
        } else {
            button.insert(InteractionDisabled);
        }
        for child in children.iter() {
            if let Ok(mut text) = texts.get_mut(child)
                && text.0 != label
            {
                text.0 = label.clone();
            }
            if let Ok(mut color) = colors.get_mut(child) {
                color.0 = if enabled { TEXT } else { TEXT_DIM };
            }
            if let Ok(mut image) = images.get_mut(child) {
                image.color = Color::srgba(1.0, 1.0, 1.0, if enabled { 1.0 } else { 0.4 });
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn build_night_panel(
    mut commands: Commands,
    parts: Res<SidePanelParts>,
    font: Res<UiFont>,
    lang: Res<Lang>,
    data: Res<GameData>,
    stock: Res<Stock>,
    register: Res<Register>,
    queue: Res<Queue>,
    layout: Res<ShopLayout>,
    drawer: Res<Drawer>,
    icons: Res<Icons>,
    settings: Res<crate::save::Settings>,
    ghosts: Query<&Ghost>,
    phase: Res<State<Phase>>,
    progress: Res<Progress>,
    mut shown: Local<Option<NightPanelShows>>,
) {
    // Rebuild only when something this panel shows has changed. Rebuilding
    // recreates the buttons, and a click whose press and release land on
    // different button entities is lost – often at a low frame rate, when
    // other ghosts keep changing state.
    let front = queue
        .front()
        .and_then(|e| ghosts.get(e).ok().map(|g| (e, g.state)));
    let waiting = front.is_some_and(|(_, state)| state == GhostState::Waiting);
    let shows = NightPanelShows {
        front,
        line: if waiting { 0 } else { queue.0.len() },
        sale: register
            .sale
            .as_ref()
            .map(|s| (s.ghost, s.coins.iter().map(|c| c.checked).collect())),
        question: register.question.is_some(),
    };
    let forced =
        stock.is_changed() || font.is_changed() || settings.is_changed() || phase.is_changed();
    if !forced && shown.as_ref() == Some(&shows) {
        return;
    }
    *shown = Some(shows);
    let mut ui = PanelBuilder::side(&mut commands, *parts, &font);
    ui.heading(&lang.t("night.title"));

    // Marked after building: the counted line and the change buttons are
    // updated in place while change is counted (`refresh_change`).
    let mut commands_later: Vec<(Entity, Option<Coin>)> = Vec::new();
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
            ui.icon_button_in(
                row,
                icons.coin(c.coin),
                &label,
                Action::CheckCoin(i),
                false,
                !c.checked,
            );
        }
        if settings.change_hint {
            ui.dim(&lang.f("night.change_due", &[("amount", &sale.change_due())]));
        }
        let counted = ui.text(&counted_line(&lang, sale));
        commands_later.push((counted, None));
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
            let button = ui.icon_button_in(
                row,
                icons.coin(coin),
                &label,
                Action::AddChange(coin),
                false,
                left > 0,
            );
            commands_later.push((button, Some(coin)));
        }
        let row = ui.row();
        ui.button_in(
            row,
            &lang.t("night.put_back"),
            Action::ResetChange,
            false,
            true,
        );
        // Only when the drawer really cannot give the change exactly.
        let due = sale.change_due();
        if !drawer.can_give(due) {
            ui.button_in(
                row,
                &lang.t("register.no_change"),
                Action::NoChange,
                false,
                true,
            );
            ui.dim(&lang.f("register.no_change_hint", &[("due", &due)]));
        }
        ui.spacer();
        ui.primary(&lang.t("night.hand_over"), Action::GiveChange);
        for (entity, coin) in commands_later {
            match coin {
                Some(coin) => commands.entity(entity).insert(ChangeCoin(coin)),
                None => commands.entity(entity).insert(CountedLine),
            };
        }
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
                ui.section(&lang.f("night.story_says", &[("name", &lang.ghost(&def.id))]));
            } else {
                ui.section(&lang.f("night.whispers", &[("who", &lang.ghost_the(&def.id))]));
            }
            ui.text(&lang.f("quote", &[("text", &ghost.whisper(&lang))]));
            ui.section(&lang.t("night.what_hand_over"));
            let row = ui.row();
            let mut any = false;
            // The ferryman's fare: a coin, not a ware.
            if crate::register::asks_fare(ghost, &data) {
                match Coin::DESCENDING.into_iter().find(|c| drawer.get(*c) > 0) {
                    Some(coin) => {
                        ui.button_in(
                            row,
                            &lang.f("night.give_fare", &[("coin", &coin.name(&lang))]),
                            Action::GiveFare,
                            false,
                            true,
                        );
                    }
                    None => ui.dim(&lang.t("night.fare_empty")),
                }
            }
            // The ferryman's parcel holds exactly what a story ghost needs.
            let parcel = ghost
                .chapter
                .is_some_and(|(story, _)| progress.parcels.contains(&story));
            if parcel {
                ui.button_in(
                    row,
                    &lang.f("night.parcel", &[("name", &lang.ghost(&def.id))]),
                    Action::HandParcel,
                    false,
                    true,
                );
            }
            // Its wish was not on display (or it would have brought it), so
            // everything in the storeroom is offered: the wares on display
            // first, then the rest.
            let mut wares = layout.displayed_products();
            for id in stock.units().keys() {
                if !wares.contains(id) {
                    wares.push(id.clone());
                }
            }
            for id in wares {
                let n = stock.count(&id);
                if n == 0 {
                    continue;
                }
                any = true;
                let label = lang.f(
                    "evening.stock_button",
                    &[("item", &lang.product(&id)), ("count", &n)],
                );
                ui.icon_button_in(
                    row,
                    icons.product(&id),
                    &label,
                    Action::Offer(id.clone()),
                    false,
                    true,
                );
            }
            if any {
                ui.dim(&lang.t("night.storeroom_hint"));
            } else if !parcel && !crate::register::asks_fare(ghost, &data) {
                ui.dim(&lang.t("night.nothing_on_shelves"));
            }
            ui.spacer();
            let root = ui.root();
            if crate::register::can_order(
                ghost,
                &stock,
                &progress,
                data.config.orders.max_per_night,
            ) {
                ui.button_in(
                    root,
                    &lang.t("night.order"),
                    Action::OrderForTomorrow,
                    false,
                    true,
                );
                ui.dim(&lang.f(
                    "night.order_hint",
                    &[(
                        "percent",
                        &((data.config.orders.price_bonus * 100.0).round() as u32),
                    )],
                ));
            }
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
    parts: Res<SidePanelParts>,
    font: Res<UiFont>,
    lang: Res<Lang>,
    data: Res<GameData>,
    progress: Res<Progress>,
    settings: Res<crate::save::Settings>,
    report: Res<crate::day_cycle::NightReport>,
) {
    // The accounts are shown in the middle of the screen (`accounts.rs`);
    // beside them: the first night's tip, and what tomorrow brings.
    let mut ui = PanelBuilder::side(&mut commands, *parts, &font);
    ui.heading(&lang.t("morning.title"));
    if settings.tips && progress.night == 1 {
        ui.text(&lang.t("tip.morning"));
        let root = ui.root();
        ui.button_in(root, &lang.t("tips.hide"), Action::ToggleTips, false, true);
    }
    if report.apprentice_served > 0 {
        ui.text(&lang.f(
            "morning.apprentice",
            &[("count", &report.apprentice_served)],
        ));
    }
    if progress.season_over(&data) {
        return;
    }
    let mut tomorrow = progress.clone();
    tomorrow.night += 1;
    let calendar = tomorrow.calendar(&data);
    if let Some(event) = data.event(calendar) {
        ui.section(&lang.f(
            "morning.tomorrow",
            &[("name", &lang.t(&format!("event.{}.name", event.id)))],
        ));
        ui.dim(&lang.t(&format!("event.{}.text", event.id)));
    }
    for o in progress.orders.iter().filter(|o| o.night == tomorrow.night) {
        ui.text(&lang.f(
            "morning.order",
            &[
                ("who", &lang.ghost(&o.ghost)),
                ("item", &lang.product(&o.product)),
            ],
        ));
    }
}

fn build_demo_end_panel(
    mut commands: Commands,
    parts: Res<SidePanelParts>,
    font: Res<UiFont>,
    lang: Res<Lang>,
    data: Res<GameData>,
    progress: Res<Progress>,
) {
    let mut ui = PanelBuilder::side(&mut commands, *parts, &font);
    let demo = cfg!(feature = "demo");
    ui.heading(&lang.t(if demo { "end.title" } else { "end.title_full" }));
    ui.text(&lang.f("end.reputation", &[("reputation", &progress.reputation)]));
    for (i, story) in data.stories.iter().enumerate() {
        let left = story.chapters.len().saturating_sub(progress.chapter(i));
        // This season's stories only.
        if data.season_of(story.start_night) != progress.season
            || progress.calendar(&data) < story.start_night
        {
            continue;
        }
        if left == 0 {
            ui.dim(&lang.t(&format!("story.{}.ending", story.ghost)));
        } else {
            ui.dim(&lang.f(
                "end.wishes_left",
                &[("name", &lang.ghost(&story.ghost)), ("count", &left)],
            ));
        }
    }
    if demo {
        ui.text(&lang.f("end.carry_over", &[("night", &(progress.night + 1))]));
        ui.dim(&lang.t("end.thanks"));
    } else {
        ui.text(&lang.t(&format!("season.{}.epilogue", progress.season)));
        ui.dim(&lang.t("end.thanks_full"));
        // On to a season with its own story, or simply keep the shop open.
        let next_story = data
            .stories
            .iter()
            .any(|s| data.season_of(s.start_night) == progress.season + 1);
        let label = if next_story {
            "end.next_season"
        } else {
            "end.keep_playing"
        };
        let root = ui.root();
        ui.button_in(root, &lang.t(label), Action::KeepPlaying, false, true);
    }
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

/// Full-screen dimmed layer with a centred panel on top of the game
/// (pause menu, ferryman's boat, accounts). Returns (layer, panel).
pub(crate) fn overlay(
    commands: &mut Commands,
    theme: &UiTheme,
    dim: f32,
    width: f32,
) -> (Entity, Entity) {
    let layer = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.06, 0.035, 0.06, dim)),
            GlobalZIndex(100),
        ))
        .id();
    let frame = commands
        .spawn((
            Node {
                width: px(width),
                max_width: percent(96),
                max_height: percent(94),
                flex_direction: FlexDirection::Column,
                border_radius: BorderRadius::all(px(6)),
                ..default()
            },
            BackgroundColor(BG_PANEL),
            ChildOf(layer),
        ))
        .id();
    let panel = theme.panel_body(commands, frame, 0.36, 24.0, 10.0);
    scroll_y(commands, panel);
    (layer, panel)
}

/// Lets a panel body scroll when it is taller than the window (a large
/// interface size on a small screen), so every button stays reachable.
pub(crate) fn scroll_y(commands: &mut Commands, panel: Entity) {
    commands
        .entity(panel)
        .insert(ScrollPosition::default())
        .queue(|mut entity: EntityWorldMut| {
            if let Some(mut node) = entity.get_mut::<Node>() {
                node.overflow = Overflow::scroll_y();
            }
        });
}

pub(crate) struct PanelBuilder<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
    root: Entity,
    /// Where `primary` buttons go (the side panel's fixed footer, or `root`).
    footer: Entity,
    font: &'a UiFont,
}

impl<'a, 'w, 's> PanelBuilder<'a, 'w, 's> {
    /// Clears the panel and starts over.
    pub(crate) fn new(commands: &'a mut Commands<'w, 's>, root: Entity, font: &'a UiFont) -> Self {
        commands.entity(root).despawn_related::<Children>();
        Self {
            commands,
            root,
            footer: root,
            font,
        }
    }

    /// Clears the side panel: content goes into the scrolling body,
    /// primary buttons into the fixed footer.
    fn side(commands: &'a mut Commands<'w, 's>, parts: SidePanelParts, font: &'a UiFont) -> Self {
        commands.entity(parts.body).despawn_related::<Children>();
        commands.entity(parts.footer).despawn_related::<Children>();
        Self {
            commands,
            root: parts.body,
            footer: parts.footer,
            font,
        }
    }

    pub(crate) fn root(&self) -> Entity {
        self.root
    }

    /// A plain layout node under `parent`.
    pub(crate) fn node(&mut self, parent: Entity, bundle: impl Bundle) -> Entity {
        self.commands.spawn((bundle, ChildOf(parent))).id()
    }

    /// A background colour for a node built here.
    pub(crate) fn tint(&mut self, entity: Entity, color: Color) {
        self.commands.entity(entity).insert(BackgroundColor(color));
    }

    /// A picture of any size under `parent`.
    pub(crate) fn image_sized(&mut self, parent: Entity, image: Handle<Image>, size: Vec2) {
        self.commands.spawn((
            ImageNode::new(image),
            Node {
                width: px(size.x),
                height: px(size.y),
                flex_shrink: 0.0,
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(parent),
        ));
    }

    /// Text under `parent`, in a box of a fixed width (table cells).
    pub(crate) fn cell(
        &mut self,
        parent: Entity,
        text: &str,
        width: f32,
        justify: JustifyContent,
        color: Color,
    ) {
        let cell = self.node(
            parent,
            Node {
                width: px(width),
                flex_shrink: 0.0,
                justify_content: justify,
                ..default()
            },
        );
        self.add_text(cell, text, false, 13.0, color);
    }

    /// Lets `primary` buttons go into `footer` instead of the root.
    pub(crate) fn with_footer(mut self, footer: Entity) -> Self {
        self.footer = footer;
        self
    }

    pub(crate) fn add_text(
        &mut self,
        parent: Entity,
        text: &str,
        bold: bool,
        size: f32,
        color: Color,
    ) -> Entity {
        let font = if bold {
            &self.font.bold
        } else {
            &self.font.regular
        };
        self.commands
            .spawn((
                Text::new(text),
                text_font(font, size),
                TextColor(color),
                Pickable::IGNORE,
                ChildOf(parent),
            ))
            .id()
    }

    pub(crate) fn display_text(&mut self, parent: Entity, text: &str, size: f32, color: Color) {
        self.commands.spawn((
            Text::new(text),
            text_font(&self.font.display, size),
            TextColor(color),
            Pickable::IGNORE,
            ChildOf(parent),
        ));
    }

    pub(crate) fn heading(&mut self, text: &str) {
        self.display_text(self.root, text, 23.0, ACCENT);
        self.rule();
    }

    pub(crate) fn rule(&mut self) {
        self.commands.spawn((
            Node {
                height: px(1),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundGradient(vec![
                LinearGradient::to_right(vec![
                    ColorStop::new(PANEL_EDGE, percent(0)),
                    ColorStop::new(ACCENT.with_alpha(0.45), percent(48)),
                    ColorStop::new(PANEL_EDGE, percent(100)),
                ])
                .into(),
            ]),
            Pickable::IGNORE,
            ChildOf(self.root),
        ));
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

    pub(crate) fn text(&mut self, text: &str) -> Entity {
        self.add_text(self.root, text, false, 14.0, TEXT)
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
        self.icon_button_in(parent, None, label, action, selected, enabled);
    }

    /// Like `button_in`, with a small picture in front of the label.
    pub(crate) fn icon_button_in(
        &mut self,
        parent: Entity,
        icon: Option<Handle<Image>>,
        label: &str,
        action: Action,
        selected: bool,
        enabled: bool,
    ) -> Entity {
        self.spawn_button(
            parent,
            icon,
            label,
            action,
            ButtonTone::option(enabled, selected),
            12.0,
            false,
        )
    }

    /// Full-width menu rows can share equal space for related settings.
    pub(crate) fn menu_option(
        &mut self,
        parent: Entity,
        label: &str,
        action: Action,
        selected: bool,
    ) {
        self.spawn_button(
            parent,
            None,
            label,
            action,
            ButtonTone::option(true, selected),
            14.0,
            true,
        );
    }

    pub(crate) fn primary(&mut self, label: &str, action: Action) {
        self.spawn_button(
            self.footer,
            None,
            label,
            action,
            ButtonTone::Brass,
            16.0,
            false,
        );
    }

    fn spawn_button(
        &mut self,
        parent: Entity,
        icon: Option<Handle<Image>>,
        label: &str,
        action: Action,
        tone: ButtonTone,
        size: f32,
        grow: bool,
    ) -> Entity {
        let enabled = !matches!(tone, ButtonTone::Disabled);
        let (label_font, text_color) = match tone {
            ButtonTone::Brass => (&self.font.display, INK),
            ButtonTone::Disabled => (&self.font.regular, TEXT_DIM),
            _ => (&self.font.regular, TEXT),
        };
        let padding = if icon.is_some() {
            UiRect::new(px(4), px(8), px(2), px(2))
        } else {
            UiRect::axes(px(8), px(if size >= 14.0 { 7 } else { 5 }))
        };
        let (surface, trim) = tone.paint(false);
        let mut button = self.commands.spawn((
            Button,
            Hovered::default(),
            ActionButton(action),
            tone,
            Node {
                padding,
                max_width: percent(100),
                flex_grow: if grow { 1.0 } else { 0.0 },
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(3)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                column_gap: px(5),
                ..default()
            },
            surface,
            trim,
            ChildOf(parent),
        ));
        let id = button.id();
        if let Some(image) = icon {
            let tint = Color::srgba(1.0, 1.0, 1.0, if enabled { 1.0 } else { 0.4 });
            button.commands().spawn((
                ImageNode::new(image).with_color(tint),
                Node {
                    width: px(ICON_SIZE),
                    height: px(ICON_SIZE),
                    flex_shrink: 0.0,
                    ..default()
                },
                Pickable::IGNORE,
                ChildOf(id),
            ));
        }
        button.commands().spawn((
            Text::new(label),
            text_font(label_font, size),
            TextColor(text_color),
            Pickable::IGNORE,
            ChildOf(id),
        ));
        if !enabled {
            button.insert(InteractionDisabled);
        }
        id
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
