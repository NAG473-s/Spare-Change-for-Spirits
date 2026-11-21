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
                    refresh_change.after(build_night_panel).run_if(
                        in_state(Phase::Night).and_then(
                            resource_changed::<Register>.or_else(resource_changed::<Drawer>),
                        ),
                    ),
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
                                .or_else(resource_changed::<Stock>)
                                .or_else(resource_changed::<Drawer>)
                                .or_else(resource_changed::<UiFont>)
                                .or_else(resource_changed::<crate::save::Settings>)
                                .or_else(resource_changed::<Progress>)
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
    BookBack,
    /// Close the book, back to where it was opened from.
    BookClose,
    /// Leave the finale scene for the accounts.
    FinaleContinue,
    /// Mend an errand's lost keepsake at the workbench / give it back.
    RepairErrand(usize),
    ReturnErrand(usize),
    /// The campaign's closing sequence: on to the next view, choose the
    /// keepsake for Hedda's ledger (index in `GameData::decor`), credits.
    EndingContinue,
    OpenLedgerChoice,
    ChooseLedgerKeepsake(usize),
    OpenCredits,
    CloseCredits,
    /// Turn to a chapter of the book (its coloured tab).
    BookChapter(crate::book::Chapter),
    GhostProfile(usize),
    /// A keepsake's page in the book (index in `GameData::decor`).
    KeepsakePage(usize),
    RegularHistory(u32, usize),
    OpenOrders(usize),
    PinOrder(usize),
    SkipGuide,
    ResumeGuide,
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
    RemoveChange(Coin),
    ResetChange,
    GiveChange,
    GiveChangeAnyway,
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
    /// Painted currency if installed, otherwise a recognisable silhouette.
    coins: HashMap<Coin, Handle<Image>>,
}

impl Icons {
    pub fn product(&self, id: &str) -> Option<Handle<Image>> {
        self.products.get(id).cloned()
    }

    pub fn decor(&self, id: &str) -> Option<Handle<Image>> {
        self.decor.get(id).cloned()
    }

    pub fn coin(&self, coin: Coin) -> Option<Handle<Image>> {
        self.coins.get(&coin).cloned()
    }
}

impl FromWorld for Icons {
    fn from_world(world: &mut World) -> Self {
        let data = world.resource::<GameData>();
        let product_ids: Vec<String> = data.products.iter().map(|p| p.id.clone()).collect();
        let decor_ids: Vec<String> = data.decor.iter().map(|d| d.id.clone()).collect();
        let assets = world.resource::<AssetServer>().clone();
        let load = |folder: &str, ids: Vec<String>| {
            ids.into_iter()
                .filter_map(|id| {
                    let path = format!("sprites/{folder}/{id}.png");
                    asset_exists(&path).then(|| (id, assets.load(path)))
                })
                .collect()
        };
        let coins = Coin::ASCENDING
            .into_iter()
            .map(|c| {
                let path = format!("sprites/coins/{}.png", c.id());
                let image = if asset_exists(&path) {
                    assets.load(path)
                } else {
                    world
                        .resource_mut::<Assets<Image>>()
                        .add(coin_silhouette(c))
                };
                (c, image)
            })
            .collect();
        Self {
            products: load("products", product_ids),
            decor: load("decor", decor_ids),
            coins,
        }
    }
}

/// A four-hole button, a squat candle with a wick, and a spoon with a long handle.
fn coin_silhouette(coin: Coin) -> Image {
    use bevy::asset::RenderAssetUsages;
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

    const SIDE: u32 = 32;
    let mut pixels = Vec::with_capacity((SIDE * SIDE * 4) as usize);
    for y in 0..SIDE {
        for x in 0..SIDE {
            let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5) - Vec2::splat(SIDE as f32 / 2.);
            let filled = match coin {
                Coin::Button => {
                    p.length_squared() <= 13.0_f32.powi(2)
                        && (p.abs() - Vec2::splat(4.)).length_squared() > 2.5_f32.powi(2)
                }
                Coin::Candle => {
                    (p.x.abs() <= 8. && (-5. ..=12.).contains(&p.y))
                        || (p.x.abs() <= 1.5 && (-12. ..-5.).contains(&p.y))
                }
                Coin::Spoon => {
                    // Bowl and handle share a diagonal centre line.
                    let along = (p.x + p.y) * std::f32::consts::FRAC_1_SQRT_2;
                    let across = (p.x - p.y) * std::f32::consts::FRAC_1_SQRT_2;
                    ((along + 8.) / 7.).powi(2) + (across / 4.5).powi(2) <= 1.
                        || ((-3. ..=15.).contains(&along) && across.abs() <= 1.7)
                }
            };
            pixels.extend_from_slice(&[244, 221, 179, if filled { 255 } else { 0 }]);
        }
    }
    Image::new(
        Extent3d {
            width: SIDE,
            height: SIDE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
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
    // The oldest line fades out at the top instead of ending in a hard cut.
    let frame_top = Color::srgb_u8(51, 36, 42);
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: px(0),
            left: px(0),
            right: px(0),
            height: px(LOG_FADE),
            ..default()
        },
        finish(frame_top, frame_top.with_alpha(0.0)),
        Pickable::IGNORE,
        ChildOf(log_frame),
    ));
}

/// Height of the fade over the oldest log line.
const LOG_FADE: f32 = 26.0;

fn on_activate(
    activate: On<Activate>,
    buttons: Query<&ActionButton>,
    mut writer: MessageWriter<Action>,
    mut cues: MessageWriter<Cue>,
) {
    if let Ok(button) = buttons.get(activate.entity) {
        writer.write(button.0.clone());
        if !matches!(
            button.0,
            Action::CheckCoin(_) | Action::AddChange(_) | Action::RemoveChange(_)
        ) {
            cues.write(Cue::Click);
        }
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
    if rep >= perks.rare_from {
        t.line(TipKind::Good, lang.t("rep_tip.rare"));
    } else {
        t.line(
            TipKind::Text,
            lang.f("rep_tip.rare_from", &[("min", &perks.rare_from)]),
        );
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
    let key = if (cfg!(feature = "demo") || progress.season_has_finale(&data))
        && progress.night <= season_end
    {
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
    crate::errands::market_hint(&mut ui, &lang, &data, &progress);
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
    (market, drawer): (Res<crate::market::MarketDay>, Res<Drawer>),
) {
    let night = progress.calendar(&data);
    let mut ui = PanelBuilder::side(&mut commands, *parts, &font);
    let max_crates = upgrades.crates_tonight(&data, night);
    let bought_units = bought.units(&data);
    let odds = crate::sets::odds_tonight(&data, &layout, &progress, &stock);

    if *location == Location::Harbor {
        ui.heading(&lang.t("harbor.title"));
        if progress.guiding_first_sale() {
            let step = crate::guide::evening_step(&layout, &stock);
            // Already at the harbour: say what to do here.
            let text = if step == "guide_buy" {
                "guide_buy_here"
            } else {
                step
            };
            crate::guide::prompt(&mut ui, &lang, text);
            if step != "guide_buy" {
                ui.button_in(
                    ui.root(),
                    &lang.t("harbor.back"),
                    Action::BackToShop,
                    false,
                    true,
                );
            }
        }
        ui.dim(&lang.t("harbor.intro"));
        let mut pinned = progress
            .orders
            .iter()
            .filter(|o| o.pinned && o.state == crate::orders::OrderState::Open)
            .peekable();
        if pinned.peek().is_some() {
            ui.section(&format!(
                "{} {}",
                crate::orders::PIN_MARK,
                lang.t("order_slips.pinned")
            ));
            for order in pinned {
                ui.text(&order.label(&lang));
            }
        }
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
        evening_visits(&mut ui, &lang, &data, &progress, &odds);
        ui.spacer();
        ui.primary(&lang.t("harbor.back"), Action::BackToShop);
        return;
    }

    ui.heading(&lang.t("evening.title"));
    if progress.guiding_first_sale() {
        let step = crate::guide::evening_step(&layout, &stock);
        crate::guide::prompt(&mut ui, &lang, step);
        match step {
            "guide_buy" => {
                ui.button_in(
                    ui.root(),
                    &lang.t("harbor.go"),
                    Action::GoToHarbor,
                    false,
                    true,
                );
            }
            "guide_shelf" => {
                let row = ui.row();
                for (ware, units) in stock.units().iter().filter(|(_, units)| !units.is_empty()) {
                    let label = lang.f(
                        "evening.stock_button",
                        &[("item", &lang.product(ware)), ("count", &units.len())],
                    );
                    ui.icon_button_in(
                        row,
                        icons.product(ware),
                        &label,
                        Action::ToolStock(ware.clone()),
                        *tool == Tool::Stock(ware.clone()),
                        true,
                    );
                }
            }
            _ => {
                ui.button_in(
                    ui.root(),
                    &lang.t("evening.open"),
                    Action::StartNight,
                    false,
                    true,
                );
            }
        }
    }

    // Hedda's letters: the first waits on the first evening, the others
    // turn up behind the junk her improvements clear.
    let letter = if night == 1 {
        Some(1)
    } else {
        market.evening_letter(progress.night)
    };
    if let Some(n) = letter {
        ui.section(&lang.t("letter.title"));
        ui.text(&lang.t(&format!("letter.{n}")));
    }

    if let Some(event) = data.event(night) {
        ui.section(&lang.f(
            "evening.tonight",
            &[("name", &lang.t(&format!("event.{}.name", event.id)))],
        ));
        event_detail(&mut ui, &lang, event, &progress);
        if event.prepare {
            prepare_section(
                &mut ui,
                &lang,
                &data,
                (&progress, &layout, &storeroom),
                event,
                &odds,
                max_crates,
            );
        }
    }
    evening_visits(&mut ui, &lang, &data, &progress, &odds);

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
    crate::errands::evening_section(&mut ui, &lang, &data, &progress, &layout, &drawer);

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

/// Scheduled callers and the three likeliest regular visitors are shown once
/// each. Several undiscovered identities share one anonymous line.
fn evening_visits(
    ui: &mut PanelBuilder,
    lang: &Lang,
    data: &GameData,
    progress: &Progress,
    odds: &[f32],
) {
    let mut likely: Vec<_> = odds
        .iter()
        .enumerate()
        .filter(|(_, weight)| **weight > 0.)
        .collect();
    likely.sort_by(|a, b| b.1.total_cmp(a.1));
    let orders: Vec<_> = progress
        .orders
        .iter()
        .filter(|o| o.night == progress.night && o.state == crate::orders::OrderState::Open)
        .collect();
    let stories: Vec<_> = data
        .stories
        .iter()
        .enumerate()
        .filter(|(i, _)| progress.story_due(data, *i))
        .collect();
    let expected = orders
        .iter()
        .filter_map(|o| data.ghost_index(&o.ghost))
        .chain(
            stories
                .iter()
                .filter_map(|(_, story)| data.ghost_index(&story.ghost)),
        )
        .chain(likely.into_iter().take(3).map(|(index, _)| index));
    let mut known = Vec::new();
    let mut unfamiliar = false;
    for index in expected {
        if progress.journal.has_seen(&data.ghosts[index].id) {
            if !known.contains(&index) {
                known.push(index);
            }
        } else {
            unfamiliar = true;
        }
    }
    if !orders.is_empty() {
        ui.button_in(
            ui.root(),
            &lang.t("order_slips.title"),
            Action::OpenOrders(0),
            false,
            true,
        );
    }
    if stories
        .iter()
        .any(|(i, _)| progress.parcels.contains(i) && !progress.journal.set_aside.contains(i))
    {
        ui.text(&lang.t("discovery.parcel_waits"));
    }
    if !known.is_empty() || unfamiliar {
        ui.section(&lang.t("evening.expected"));
        if !known.is_empty() {
            let row = ui.row();
            for index in known {
                ui.button_in(
                    row,
                    &lang.ghost(&data.ghosts[index].id),
                    Action::GhostProfile(index),
                    false,
                    true,
                );
            }
        }
        if unfamiliar {
            ui.dim(&lang.t("discovery.unknown_guest"));
        }
    }
    ui.button_in(
        ui.root(),
        &lang.t("discovery.more_in_book"),
        Action::OpenBook,
        false,
        true,
    );
}

/// Ghosts of the forecast whose needs the preparation section explains.
const PREPARE_GUESTS: usize = 4;

/// A night that teaches preparation: before a coin is spent, the forecast,
/// the ferry's crates and the shop's atmosphere side by side, in words – who
/// is likely to come and whether the shop suits them, and what would help.
fn prepare_section(
    ui: &mut PanelBuilder,
    lang: &Lang,
    data: &GameData,
    (progress, layout, storeroom): (&Progress, &ShopLayout, &Storeroom),
    event: &crate::data::NightEvent,
    odds: &[f32],
    max_crates: u32,
) {
    use crate::data::Need;
    let night = progress.calendar(data);
    let atmosphere = layout.atmosphere(data, night);
    ui.section(&lang.t("evening.prepare_title"));
    ui.text(&lang.t(&format!("event.{}.prepare", event.id)));
    ui.text(&lang.f("evening.prepare_ferry", &[("max", &max_crates)]));
    ui.text(&lang.f(
        "evening.prepare_atmosphere",
        &[("atmosphere", &atmosphere.describe(lang))],
    ));
    // The likely guests the player already knows, and whether they would stay.
    let mut likely: Vec<usize> = (0..odds.len()).filter(|&i| odds[i] > 0.0).collect();
    likely.sort_by(|&a, &b| odds[b].total_cmp(&odds[a]));
    let mut missing = Vec::new();
    for i in likely
        .into_iter()
        .filter(|&i| progress.journal.has_seen(&data.ghosts[i].id))
        .take(PREPARE_GUESTS)
    {
        let ghost = &data.ghosts[i];
        for need in &ghost.needs {
            let key = if need.is_met(&atmosphere) {
                "evening.prepare_suits"
            } else {
                if let Need::AtLeast(stat, _) = need
                    && !missing.contains(stat)
                {
                    missing.push(*stat);
                }
                "evening.prepare_turns_round"
            };
            ui.dim(&lang.f(
                key,
                &[
                    ("ghost", &lang.ghost(&ghost.id)),
                    ("need", &need.describe(lang)),
                ],
            ));
        }
    }
    // What would raise what is missing: furniture already owned first,
    // otherwise what the market sells tonight.
    for stat in missing {
        let helps = |d: &&crate::data::DecorDef| d.effect.get(stat) > 0;
        let owned: Vec<_> = data
            .decor
            .iter()
            .filter(helps)
            .filter(|d| storeroom.count(&d.id) > 0)
            .collect();
        let pieces = if owned.is_empty() {
            let mut sold: Vec<_> = data
                .decor
                .iter()
                .filter(helps)
                .filter(|d| !d.keepsake && d.unlock_night <= night)
                .collect();
            sold.sort_by_key(|d| d.cost);
            sold.truncate(2);
            sold
        } else {
            owned
        };
        let names: Vec<String> = pieces
            .iter()
            .map(|d| format!("{} ({})", lang.decor(&d.id), d.effect.changes(lang)))
            .collect();
        if !names.is_empty() {
            ui.dim(&lang.f(
                "evening.prepare_fix",
                &[("stat", &lang.t(stat.key())), ("items", &lang.join(&names))],
            ));
        }
    }
}

fn event_detail(
    ui: &mut PanelBuilder,
    lang: &Lang,
    event: &crate::data::NightEvent,
    progress: &Progress,
) {
    // The prose sometimes names the species attracted by this weather.
    if event
        .ghost_weights
        .iter()
        .all(|(id, _)| progress.journal.has_seen(id))
    {
        ui.dim(&lang.t(&format!("event.{}.text", event.id)));
    } else {
        let change = event.atmosphere.changes(lang);
        if !change.is_empty() {
            ui.dim(&change);
        }
        if event.price_factor != 1.0 || event.crate_bonus != 0 {
            ui.dim(&lang.f(
                "discovery.weather_trade",
                &[
                    ("price", &(event.price_factor * 100.0).round()),
                    ("crates", &crate::ferry::signed(event.crate_bonus.into())),
                ],
            ));
        }
    }
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
    }
}

/// What a complete set brings, in words.
pub fn set_bonus(lang: &Lang, set: &crate::sets::SetDef) -> String {
    let mut parts = Vec::new();
    if !set.ghost_weights.is_empty() {
        parts.push(lang.t("discovery.set_draws"));
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
    guiding: bool,
    front: Option<(Entity, GhostState)>,
    line: usize,
    /// Keep the actual coins and whether the drawer can make change: a
    /// replacement payment can have the same number of coins as the first.
    /// Counting change alone leaves these controls in place.
    sale: Option<(Entity, Vec<crate::register::PaidCoin>, bool)>,
    recount: bool,
    question: bool,
}

/// The line that shows the change counted so far.
#[derive(Component)]
struct CountedLine;

/// Drawer buttons add a coin. A stack in the change tray puts one back.
#[derive(Component, Clone, Copy)]
enum ChangeCoin {
    Drawer(Coin),
    Tray(Coin),
}

#[derive(Component)]
struct CoinPointerTarget(Coin);

fn on_change_coin_click(
    mut click: On<PointerClick>,
    targets: Query<(&ChildOf, &CoinPointerTarget)>,
    controls: Query<Has<InteractionDisabled>, With<ActionButton>>,
    mut actions: MessageWriter<Action>,
    mut commands: Commands,
) {
    click.propagate(false);
    let Ok((parent, CoinPointerTarget(coin))) = targets.get(click.entity) else {
        return;
    };
    let button = parent.parent();
    let Ok(disabled) = controls.get(button) else {
        return;
    };
    match click.button {
        PointerButton::Primary if !disabled => commands.trigger(Activate { entity: button }),
        PointerButton::Secondary => {
            // Even an empty drawer slot can take back a coin already on the tray.
            actions.write(Action::RemoveChange(*coin));
        }
        _ => {}
    }
}

impl ChangeCoin {
    fn label(self, lang: &Lang, drawer: &Drawer, sale: &crate::register::Sale) -> (String, bool) {
        match self {
            Self::Drawer(coin) => {
                let left = drawer.get(coin).saturating_sub(sale.given.get(coin));
                let label = lang.f(
                    "night.add_coin",
                    &[
                        ("coin", &coin.name(lang)),
                        ("value", &coin.value()),
                        ("left", &left),
                    ],
                );
                (label, left > 0)
            }
            Self::Tray(coin) => {
                let count = sale.given.get(coin);
                let label = lang.f(
                    "register.return_one",
                    &[("coin", &coin.name(lang)), ("count", &count)],
                );
                (label, count > 0)
            }
        }
    }
}

fn counted_line(lang: &Lang, sale: &crate::register::Sale) -> String {
    lang.f("register.change_tray", &[("total", &sale.given.total())])
}

/// While change is counted: the counted line and what is left of each coin.
fn refresh_change(
    register: Res<Register>,
    drawer: Res<Drawer>,
    lang: Res<Lang>,
    mut commands: Commands,
    mut lines: Query<&mut Text, With<CountedLine>>,
    mut buttons: Query<(Entity, &ChangeCoin, &Children, &Hovered, &mut Visibility)>,
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
    for (entity, coin, children, hovered, mut visibility) in &mut buttons {
        let (label, enabled) = coin.label(&lang, &drawer, sale);
        if matches!(coin, ChangeCoin::Tray(_)) {
            let wanted = if enabled {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            if *visibility != wanted {
                *visibility = wanted;
            }
        }
        let tone = ButtonTone::option(enabled, false);
        let (surface, trim) = tone.paint(hovered.get() && enabled);
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
        guiding: progress.guiding_first_sale(),
        front,
        line: if waiting { 0 } else { queue.0.len() },
        sale: register.sale.as_ref().map(|sale| {
            (
                sale.ghost,
                sale.coins.clone(),
                drawer.can_give(sale.change_due()),
            )
        }),
        recount: register.sale.as_ref().is_some_and(|s| s.recount),
        question: register.question.is_some(),
    };
    let forced = (shown
        .as_ref()
        .is_some_and(|old| old.guiding != shows.guiding))
        || (waiting && (stock.is_changed() || drawer.is_changed()))
        || font.is_changed()
        || settings.is_changed()
        || phase.is_changed();
    if !forced && shown.as_ref() == Some(&shows) {
        return;
    }
    *shown = Some(shows);
    let mut ui = PanelBuilder::side(&mut commands, *parts, &font);
    ui.heading(&lang.t("night.title"));
    if progress.guiding_first_sale() {
        crate::guide::prompt(
            &mut ui,
            &lang,
            if register.sale.is_some() {
                "guide_change"
            } else {
                "guide_wish"
            },
        );
    }

    if let Some(sale) = &register.sale {
        ui.section(&lang.t("night.register"));
        if let Ok(ghost) = ghosts.get(sale.ghost)
            && let Some(memory) = &ghost.memory
            && let Some(regular) = ghost.regular.and_then(|i| progress.journal.regulars.get(i))
        {
            ui.text(&lang.f(
                "log.story_says",
                &[
                    ("name", &regular.name(&lang)),
                    ("text", &memory.words(&lang)),
                ],
            ));
        }
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
            let button = ui.icon_button_in(
                row,
                icons.coin(c.coin),
                &label,
                Action::CheckCoin(i),
                false,
                !c.checked,
            );
            ui.coin_pointer_target(button, c.coin);
        }
        if settings.change_hint {
            ui.dim(&lang.f("night.change_due", &[("amount", &sale.change_due())]));
        }
        let counted = ui.text(&counted_line(&lang, sale));
        ui.commands.entity(counted).insert(CountedLine);
        ui.dim(&lang.t("register.undo_hint"));
        // Coins from the drawer first, the tray below: the tray's buttons
        // keep their place while hidden, so nothing moves under the pointer
        // when the first coin is laid out.
        for tray in [false, true] {
            let row = ui.row();
            for coin in Coin::ASCENDING {
                let (control, action) = if tray {
                    (ChangeCoin::Tray(coin), Action::RemoveChange(coin))
                } else {
                    (ChangeCoin::Drawer(coin), Action::AddChange(coin))
                };
                let (label, enabled) = control.label(&lang, &drawer, sale);
                let button =
                    ui.icon_button_in(row, icons.coin(coin), &label, action, false, enabled);
                let mut entity = ui.commands.entity(button);
                entity.insert(control);
                if tray && !enabled {
                    entity.insert(Visibility::Hidden);
                }
                ui.coin_pointer_target(button, coin);
            }
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
                &lang.t("register.ask_smaller"),
                Action::NoChange,
                false,
                true,
            );
            if settings.change_hint {
                ui.dim(&lang.f("register.no_change_hint", &[("due", &due)]));
            }
        }
        ui.spacer();
        if sale.recount {
            // Keep the correction beside Hand over, even in a scrolled panel.
            let key = if sale.given.total() < due {
                "register.change_short"
            } else {
                "register.change_over"
            };
            ui.add_text(ui.footer, &lang.t(key), true, 14.0, TEXT);
            let guided_sale = ghosts.get(sale.ghost).is_ok_and(|g| g.first_customer)
                && progress.guiding_first_sale();
            if !guided_sale {
                ui.button_in(
                    ui.footer,
                    &lang.t("register.give_anyway"),
                    Action::GiveChangeAnyway,
                    false,
                    true,
                );
            }
        }
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
            if let Some(regular) = ghost.regular.and_then(|i| progress.journal.regulars.get(i)) {
                ui.section(&lang.f("night.story_says", &[("name", &regular.name(&lang))]));
            } else if ghost.chapter.is_some() {
                ui.section(&lang.f("night.story_says", &[("name", &lang.ghost(&def.id))]));
            } else {
                ui.section(&lang.f("night.whispers", &[("who", &lang.ghost_the(&def.id))]));
            }
            ui.text(&lang.f("quote", &[("text", &ghost.whisper(&lang))]));
            // The ferryman's parcel holds exactly what a story ghost needs:
            // said in words and offered as the main button, so it is not
            // mistaken for an ordinary ware.
            let parcel = ghost
                .chapter
                .is_some_and(|(story, _)| progress.parcels.contains(&story));
            let set_aside = ghost
                .chapter
                .is_some_and(|(story, _)| progress.journal.set_aside.contains(&story));
            if set_aside {
                ui.section(&lang.f(
                    "night.set_aside_waits",
                    &[
                        ("item", &lang.product(&ghost.wish)),
                        ("name", &lang.ghost(&def.id)),
                    ],
                ));
            } else if parcel {
                ui.section(&lang.f("evening.parcel_waits", &[("name", &lang.ghost(&def.id))]));
            }
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
            if !(ghost.first_customer && progress.guiding_first_sale()) {
                ui.button_in(root, &lang.t("night.decline"), Action::Decline, false, true);
            }
            if set_aside {
                ui.primary(
                    &lang.f(
                        "night.give_set_aside",
                        &[("item", &lang.product(&ghost.wish))],
                    ),
                    Action::HandParcel,
                );
            } else if parcel {
                ui.primary(
                    &lang.f("night.parcel", &[("name", &lang.ghost(&def.id))]),
                    Action::HandParcel,
                );
            }
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
        event_detail(&mut ui, &lang, event, &progress);
    }
    for o in progress.orders.iter().filter(|o| o.night == tomorrow.night) {
        ui.text(&lang.f(
            "morning.order",
            &[
                ("who", &o.customer(&lang)),
                ("item", &o.ordered_ware(&lang)),
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
            || (left > 0 && !progress.journal.has_seen(&story.ghost))
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
        // The campaign is complete: the ledger's last page (if the game was
        // closed before it was chosen) and the credits, to see again.
        if progress.journal.campaign_complete {
            let root = ui.root();
            if progress.journal.ledger_keepsake.is_none() {
                ui.button_in(
                    root,
                    &lang.t("ending.ledger_choose"),
                    Action::OpenLedgerChoice,
                    false,
                    true,
                );
            }
            ui.button_in(
                root,
                &lang.t("credits.open"),
                Action::OpenCredits,
                false,
                true,
            );
        }
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
/// The book: like `overlay`, with a fixed header (the chapter tabs) above a
/// page that scrolls on its own. Returns (layer, header, page).
pub(crate) fn book_overlay(
    commands: &mut Commands,
    theme: &UiTheme,
    width: f32,
    height: f32,
) -> (Entity, Entity, Entity) {
    let (layer, body) = overlay(commands, theme, 0.6, width);
    commands
        .entity(body)
        .queue(move |mut entity: EntityWorldMut| {
            if let Some(mut node) = entity.get_mut::<Node>() {
                node.overflow = Overflow::visible();
                node.min_height = px(height);
                node.max_height = px(height);
            }
        });
    let header = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(10),
                flex_shrink: 0.0,
                ..default()
            },
            ChildOf(body),
        ))
        .id();
    let page = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_height: px(0),
                flex_direction: FlexDirection::Column,
                row_gap: px(10),
                ..default()
            },
            ChildOf(body),
        ))
        .id();
    scroll_y(commands, page);
    (layer, header, page)
}

pub(crate) fn scroll_y(commands: &mut Commands, panel: Entity) {
    commands
        .entity(panel)
        .insert(ScrollPosition::default())
        .queue(|mut entity: EntityWorldMut| {
            if let Some(mut node) = entity.get_mut::<Node>() {
                node.overflow = Overflow::scroll_y();
                // Clip at the content, not in the padding: the padding lies on
                // the painted wooden frame, which scrolled text must not cover.
                node.overflow_clip_margin = OverflowClipMargin::content_box();
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
    /// Changes the layout of an entity this builder made (padding, margins).
    pub(crate) fn adjust(
        &mut self,
        entity: Entity,
        f: impl FnOnce(&mut Node) + Send + Sync + 'static,
    ) {
        self.commands
            .entity(entity)
            .entry::<Node>()
            .and_modify(move |mut node| f(&mut node));
    }

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

    // The standard widget turns any pointer button into Activate. Intercept
    // clicks on its full surface (including the one-pixel border) first.
    fn coin_pointer_target(&mut self, button: Entity, coin: Coin) {
        self.commands
            .spawn((
                CoinPointerTarget(coin),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(-1),
                    right: px(-1),
                    top: px(-1),
                    bottom: px(-1),
                    ..default()
                },
                ChildOf(button),
            ))
            .observe(on_change_coin_click);
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
        let label = if matches!(tone, ButtonTone::Selected) {
            format!("✓ {label}")
        } else {
            label.to_owned()
        };
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
    fn expected_visits_link_known_profiles_once_and_group_strangers() {
        #[derive(Resource)]
        struct Odds(Vec<f32>);

        let lang = Lang::load("en");
        let anonymous = lang.t("discovery.unknown_guest");
        let heading = lang.t("evening.expected");
        let parcel = lang.t("discovery.parcel_waits");
        let edwin_name = lang.ghost("edwin");
        let shy_name = lang.ghost("shy");
        let data = GameData::load();
        let edwin = data.ghost_index("edwin").unwrap();
        let shy = data.ghost_index("shy").unwrap();
        let child = data.ghost_index("child").unwrap();
        let vain = data.ghost_index("vain").unwrap();
        let mut odds = vec![0.; data.ghosts.len()];
        for (i, weight) in [(edwin, 9.), (shy, 8.), (child, 7.), (vain, 3.)] {
            odds[i] = weight;
        }
        let mut progress = Progress::new(0);
        progress.parcels.push(
            data.stories
                .iter()
                .position(|s| s.ghost == "edwin")
                .unwrap(),
        );
        progress.orders.push(crate::orders::Order::new(
            "edwin",
            "rain_jar",
            progress.night,
            None,
        ));
        // Known fourth-ranked type still doesn't belong in the three likeliest.
        progress.journal.met("vain", "");
        let mut app = App::new();
        let root = app.world_mut().spawn_empty().id();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Font>()
            .insert_resource(lang)
            .init_resource::<UiFont>()
            .insert_resource(data)
            .insert_resource(progress)
            .insert_resource(Odds(odds))
            .add_systems(
                Update,
                move |mut commands: Commands,
                      font: Res<UiFont>,
                      lang: Res<Lang>,
                      data: Res<GameData>,
                      progress: Res<Progress>,
                      odds: Res<Odds>| {
                    evening_visits(
                        &mut PanelBuilder::new(&mut commands, root, &font),
                        &lang,
                        &data,
                        &progress,
                        &odds.0,
                    );
                },
            );
        let lines = |world: &mut World| {
            world
                .query::<&Text>()
                .iter(world)
                .map(|t| t.0.clone())
                .collect::<Vec<_>>()
        };
        let actions = |world: &mut World| {
            world
                .query::<&ActionButton>()
                .iter(world)
                .map(|b| b.0.clone())
                .collect::<Vec<_>>()
        };
        app.update();
        let texts = lines(app.world_mut());
        assert_eq!(texts.iter().filter(|t| *t == &anonymous).count(), 1);
        assert!(texts.contains(&parcel) && texts.contains(&heading));
        assert!(!texts.contains(&edwin_name) && !texts.contains(&shy_name));
        let buttons = actions(app.world_mut());
        assert!(buttons.contains(&Action::OpenBook) && buttons.contains(&Action::OpenOrders(0)));
        assert!(!buttons.iter().any(|b| matches!(b, Action::GhostProfile(_))));

        let mut progress = app.world_mut().resource_mut::<Progress>();
        progress.journal.met("edwin", "rain_jar");
        progress.journal.met("shy", "");
        app.update();
        let buttons = actions(app.world_mut());
        // One profile link even for a type present in the odds, story and orders.
        assert_eq!(
            buttons
                .iter()
                .filter(|b| **b == Action::GhostProfile(edwin))
                .count(),
            1
        );
        assert!(buttons.contains(&Action::GhostProfile(shy)));
        assert!(!buttons.contains(&Action::GhostProfile(vain)));
        let texts = lines(app.world_mut());
        assert!(
            texts.contains(&edwin_name) && texts.contains(&shy_name) && texts.contains(&anonymous)
        );

        app.world_mut().resource_mut::<Odds>().0.fill(0.);
        app.update();
        let buttons = actions(app.world_mut());
        assert!(buttons.contains(&Action::GhostProfile(edwin)));
        assert!(!buttons.contains(&Action::GhostProfile(shy)));
        assert!(!lines(app.world_mut()).contains(&anonymous));
        app.world_mut().resource_mut::<Progress>().orders.clear();
        app.world_mut().resource_mut::<GameData>().stories.clear();
        app.update();
        assert_eq!(actions(app.world_mut()), [Action::OpenBook]);
        let texts = lines(app.world_mut());
        assert!(
            !texts.contains(&heading) && !texts.contains(&anonymous) && !texts.contains(&parcel)
        );
    }

    #[test]
    fn count_list_groups_in_order() {
        let ids: Vec<String> = ["a", "b", "a", "a"].iter().map(|s| s.to_string()).collect();
        let lang = Lang::load("en");
        assert_eq!(count_list(&lang, &ids, |s| s.to_string()), "3× a, 1× b");
    }

    fn register_panel() -> App {
        use crate::economy::Purse;
        use crate::register::{PaidCoin, Sale};
        use crate::save::Settings;

        let mut app = App::new();
        let body = app.world_mut().spawn_empty().id();
        let footer = app.world_mut().spawn_empty().id();
        let data = GameData::load();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            bevy::ui_widgets::ButtonPlugin,
        ))
        .add_message::<Action>()
        .add_message::<Cue>()
        .add_observer(on_activate)
        .init_asset::<Font>()
        .init_asset::<Image>()
        .insert_resource(SidePanelParts { body, footer })
        .insert_resource(State::new(Phase::Night))
        .insert_resource(Progress::new(0))
        .insert_resource(ShopLayout::new(
            data.config.grid_cols,
            data.config.grid_rows,
        ))
        .insert_resource(data)
        .insert_resource(Lang::load("en"))
        .init_resource::<UiFont>()
        .init_resource::<Icons>()
        .init_resource::<Stock>()
        .init_resource::<Queue>()
        .insert_resource(Drawer(Purse {
            candles: 1,
            ..default()
        }))
        .insert_resource(Settings {
            change_hint: false,
            ..default()
        })
        .insert_resource(Register {
            sale: Some(Sale {
                ghost: Entity::PLACEHOLDER,
                product: "lavender".into(),
                price: 4,
                coins: vec![PaidCoin {
                    coin: Coin::Candle,
                    tell: "tell.genuine.1".into(),
                    fake: false,
                    checked: false,
                }],
                given: Purse::default(),
                recount: false,
                chapter: None,
                from_storeroom: false,
            }),
            question: None,
        })
        .add_systems(Update, (build_night_panel, refresh_change).chain());
        app.update();
        app
    }

    #[test]
    fn the_register_hides_the_due_amount_without_the_assist_and_updates_the_tray_in_place() {
        use crate::save::Settings;
        let mut app = register_panel();
        let has_text = |world: &mut World, prefix: &str| {
            world
                .query::<&Text>()
                .iter(world)
                .any(|t| t.0.starts_with(prefix))
        };
        let world = app.world_mut();
        assert!(has_text(world, "Ask for smaller coins"));
        assert!(!has_text(world, "Change due"));
        assert!(!has_text(world, "The drawer cannot give back exactly"));
        world.resource_mut::<Settings>().change_hint = true;
        app.update();
        let world = app.world_mut();
        assert!(has_text(world, "Change due: 1"));
        assert!(has_text(world, "The drawer cannot give back exactly 1"));
        let tray = world
            .query::<(Entity, &ActionButton)>()
            .iter(world)
            .find(|(_, action)| action.0 == Action::RemoveChange(Coin::Candle))
            .unwrap()
            .0;
        // Hidden but still taking its place, so the buttons above never move.
        assert_eq!(world.get::<Visibility>(tray), Some(&Visibility::Hidden));
        assert_eq!(world.get::<Node>(tray).unwrap().display, Display::Flex);
        world
            .resource_mut::<Register>()
            .sale
            .as_mut()
            .unwrap()
            .given
            .candles = 1;
        app.update();
        let world = app.world_mut();
        assert_eq!(world.get::<Visibility>(tray), Some(&Visibility::Inherited));
        assert!(world.get::<InteractionDisabled>(tray).is_none());
        assert!(has_text(world, "Your change tray · 5"));
        assert!(
            world
                .get::<Children>(tray)
                .unwrap()
                .iter()
                .any(|e| world.get::<ImageNode>(e).is_some())
        );
    }

    #[test]
    fn a_secondary_pointer_click_undoes_one_coin_without_activating_add() {
        use bevy::camera::NormalizedRenderTarget;
        use bevy::picking::{
            backend::HitData,
            events::Pointer,
            pointer::{Location, PointerId},
        };
        use bevy::window::WindowRef;

        let mut app = register_panel();
        let world = app.world_mut();
        let control = |world: &mut World, action: Action| {
            let button = world
                .query::<(Entity, &ActionButton)>()
                .iter(world)
                .find(|(_, a)| a.0 == action)
                .unwrap()
                .0;
            let target = world
                .get::<Children>(button)
                .unwrap()
                .iter()
                .find(|e| world.get::<CoinPointerTarget>(*e).is_some())
                .unwrap();
            (button, target)
        };
        let (slot, drawer_target) = control(world, Action::AddChange(Coin::Candle));
        let (_, offered_target) = control(world, Action::CheckCoin(0));
        let window = world.spawn(Window::default()).id();
        let click = |world: &mut World, target, button| {
            let location = Location {
                target: NormalizedRenderTarget::Window(
                    WindowRef::Entity(window).normalize(Some(window)).unwrap(),
                ),
                position: Vec2::ZERO,
            };
            let pointer = Pointer::new(PointerId::Mouse, location);
            let hit = HitData::new(window, 0., None, None);
            world.trigger(PointerPress {
                entity: target,
                pointer: pointer.clone(),
                button,
                hit: hit.clone(),
                count: 1,
            });
            world.flush();
            world.trigger(PointerClick {
                entity: target,
                pointer: pointer.clone(),
                button,
                hit: hit.clone(),
                duration: default(),
                count: 1,
            });
            world.trigger(PointerRelease {
                entity: target,
                pointer,
                button,
                hit,
            });
            world.flush();
            world
                .resource_mut::<Messages<Action>>()
                .drain()
                .collect::<Vec<_>>()
        };
        assert_eq!(
            click(world, drawer_target, PointerButton::Primary),
            [Action::AddChange(Coin::Candle)]
        );
        assert_eq!(
            click(world, drawer_target, PointerButton::Secondary),
            [Action::RemoveChange(Coin::Candle)]
        );
        world.entity_mut(slot).insert(InteractionDisabled);
        assert!(click(world, drawer_target, PointerButton::Primary).is_empty());
        assert_eq!(
            click(world, drawer_target, PointerButton::Secondary),
            [Action::RemoveChange(Coin::Candle)]
        );
        assert!(click(world, drawer_target, PointerButton::Middle).is_empty());
        // Right-clicking the customer's tender must not accuse it of paying with a fake.
        assert_eq!(
            click(world, offered_target, PointerButton::Secondary),
            [Action::RemoveChange(Coin::Candle)]
        );
        assert_eq!(
            click(world, offered_target, PointerButton::Primary),
            [Action::CheckCoin(0)]
        );
    }
}
