//! Main menu, pause menu (Esc) and the tips box of the first night.

use bevy::prelude::*;

use crate::data::{GameData, asset_exists};
use crate::day_cycle::{NightReport, Phase, Progress};
use crate::ghosts::{Ghost, GhostState, Queue};
use crate::i18n::Lang;
use crate::inventory::CratesOrdered;
use crate::register::Register;
use crate::save::{GameState, Settings, can_continue, read_save, write_save};
use crate::shop::ShopLayout;
use crate::title::{LOGO, TitleLogo};
use crate::ui::{Action, BG_PANEL, GameLog, PANEL_EDGE, PanelBuilder, UiFont, UiTheme, overlay};

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Paused>()
            .init_resource::<LanguageMenu>()
            .add_systems(Startup, spawn_tip_box)
            .add_systems(OnEnter(Phase::Menu), (leave_pause, build_main_menu).chain())
            .add_systems(
                Update,
                (
                    build_main_menu.run_if(
                        in_state(Phase::Menu).and_then(
                            resource_changed::<Settings>
                                .or_else(resource_changed::<UiFont>)
                                .or_else(resource_changed::<LanguageMenu>),
                        ),
                    ),
                    main_menu_actions.run_if(in_state(Phase::Menu)),
                    (toggle_pause, pause_actions).run_if(not(in_state(Phase::Menu))),
                    toggle_tips,
                    sync_pause.run_if(
                        resource_changed::<Paused>
                            .or_else(resource_changed::<Settings>)
                            .or_else(resource_changed::<UiFont>),
                    ),
                    update_tips,
                ),
            );
    }
}

/// The language list is open in the main menu.
#[derive(Resource, Default)]
pub struct LanguageMenu(pub bool);

/// The game is paused and the pause menu is open.
#[derive(Resource, Default)]
pub struct Paused(pub bool);

#[derive(Component)]
struct MenuRoot;

#[derive(Component)]
struct PauseRoot;

#[derive(Component)]
struct TipBox;

/// Main menu on the title screen: the logo with a panel below it, on the
/// left side of the screen. Returns (layer, panel).
fn title_layout(
    commands: &mut Commands,
    asset_server: &AssetServer,
    theme: &UiTheme,
    width: f32,
) -> (Entity, Entity, bool) {
    let layer = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            GlobalZIndex(100),
            Pickable::IGNORE,
        ))
        .id();
    // The column sits in the left part of the screen, centred under the logo.
    let column = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: percent(5),
                top: px(0),
                bottom: px(0),
                width: px(width.max(LOGO_WIDTH)),
                max_width: percent(90),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: px(14),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(layer),
        ))
        .id();
    let has_logo = asset_exists(LOGO);
    if has_logo {
        commands.spawn((
            TitleLogo,
            ImageNode::new(asset_server.load(LOGO)),
            Node {
                width: px(LOGO_WIDTH),
                max_width: percent(100),
                ..default()
            },
            UiTransform::IDENTITY,
            Pickable::IGNORE,
            ChildOf(column),
        ));
    }
    let frame = commands
        .spawn((
            Node {
                width: px(width),
                max_width: percent(100),
                flex_direction: FlexDirection::Column,
                border_radius: BorderRadius::all(px(6)),
                ..default()
            },
            BackgroundColor(BG_PANEL),
            ChildOf(column),
        ))
        .id();
    let panel = theme.panel_body(commands, frame, 0.37, 25.0, 8.0);
    (layer, panel, has_logo)
}

/// Width of the logo above the main menu.
const LOGO_WIDTH: f32 = 400.0;

fn sound_label(settings: &Settings, lang: &Lang) -> String {
    lang.t(if settings.sound {
        "settings.sound_on"
    } else {
        "settings.sound_off"
    })
}

fn tips_label(settings: &Settings, lang: &Lang) -> String {
    lang.t(if settings.tips {
        "settings.tips_on"
    } else {
        "settings.tips_off"
    })
}

fn build_main_menu(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    font: Res<UiFont>,
    theme: Res<UiTheme>,
    lang: Res<Lang>,
    settings: Res<Settings>,
    language_menu: Res<LanguageMenu>,
    data: Res<GameData>,
    existing: Query<Entity, With<MenuRoot>>,
) {
    for e in &existing {
        commands.entity(e).despawn();
    }
    // First start: let the player pick a language before anything else.
    let choose_language = language_menu.0 || settings.language.is_none();
    let width = if choose_language { 560.0 } else { 386.0 };
    let (layer, panel, has_logo) = title_layout(&mut commands, &asset_server, &theme, width);
    commands
        .entity(layer)
        .insert((MenuRoot, DespawnOnExit(Phase::Menu)));
    let mut ui = PanelBuilder::new(&mut commands, panel, &font);
    let root = ui.root();

    if choose_language {
        ui.heading(&lang.t("menu.choose_language"));
        let row = ui.row();
        for l in &lang.languages {
            let selected = l.code == lang.code;
            ui.menu_option(row, &l.name, Action::SetLanguage(l.code.clone()), selected);
        }
        if settings.language.is_some() {
            ui.menu_option(root, &lang.t("menu.back"), Action::CloseLanguages, false);
        }
        return;
    }

    if !has_logo {
        ui.heading(&lang.t("game.title"));
    }
    ui.text(&lang.t("menu.tagline"));
    ui.rule();
    if let Some(save) = read_save().filter(|s| can_continue(s, data.config.final_night())) {
        ui.primary(
            &lang.f("menu.continue", &[("night", &save.night)]),
            Action::Continue,
        );
        ui.menu_option(root, &lang.t("menu.new_game"), Action::NewGame, false);
    } else {
        ui.primary(&lang.t("menu.new_game"), Action::NewGame);
    }
    let language = lang.def().map(|l| l.name.clone()).unwrap_or_default();
    ui.menu_option(
        root,
        &lang.f("menu.language", &[("language", &language)]),
        Action::OpenLanguages,
        false,
    );
    let toggles = ui.row();
    ui.menu_option(
        toggles,
        &tips_label(&settings, &lang),
        Action::ToggleTips,
        settings.tips,
    );
    ui.menu_option(
        toggles,
        &sound_label(&settings, &lang),
        Action::ToggleSound,
        settings.sound,
    );
    ui.menu_option(root, &lang.t("menu.quit"), Action::QuitGame, false);
    ui.dim(&lang.f("menu.version", &[("version", &env!("CARGO_PKG_VERSION"))]));
}

fn leave_pause(mut paused: ResMut<Paused>, mut time: ResMut<Time<Virtual>>) {
    paused.0 = false;
    time.unpause();
}

fn main_menu_actions(
    mut actions: MessageReader<Action>,
    data: Res<GameData>,
    mut lang: ResMut<Lang>,
    mut settings: ResMut<Settings>,
    mut language_menu: ResMut<LanguageMenu>,
    mut state: GameState,
    mut log: ResMut<GameLog>,
    mut next: ResMut<NextState<Phase>>,
    mut exit: MessageWriter<AppExit>,
) {
    for action in actions.read() {
        match action {
            Action::NewGame => {
                state.reset(&data);
                log.clear();
                next.set(Phase::Evening);
            }
            Action::Continue => {
                if let Some(save) = read_save() {
                    state.apply(&save, &data);
                    log.clear();
                    log.push(lang.f("menu.welcome_back", &[("night", &save.night)]));
                    next.set(Phase::Evening);
                }
            }
            Action::QuitGame => {
                exit.write(AppExit::Success);
            }
            Action::OpenLanguages => language_menu.0 = true,
            Action::CloseLanguages => language_menu.0 = false,
            Action::SetLanguage(code) => {
                *lang = Lang::load(code);
                settings.language = Some(lang.code.clone());
                settings.store();
                language_menu.0 = false;
            }
            _ => {}
        }
    }
}

fn toggle_tips(mut actions: MessageReader<Action>, mut settings: ResMut<Settings>) {
    for action in actions.read() {
        match action {
            Action::ToggleTips => settings.tips = !settings.tips,
            Action::ToggleSound => settings.sound = !settings.sound,
            _ => continue,
        }
        settings.store();
    }
}

fn toggle_pause(keys: Res<ButtonInput<KeyCode>>, mut paused: ResMut<Paused>) {
    if keys.just_pressed(KeyCode::Escape) {
        paused.0 = !paused.0;
    }
}

fn pause_actions(
    mut actions: MessageReader<Action>,
    phase: Res<State<Phase>>,
    state: GameState,
    mut paused: ResMut<Paused>,
    mut next: ResMut<NextState<Phase>>,
    mut exit: MessageWriter<AppExit>,
) {
    for action in actions.read() {
        let quit_to_menu = match action {
            Action::Resume => {
                paused.0 = false;
                continue;
            }
            Action::QuitToMenu => true,
            Action::QuitGame => false,
            _ => continue,
        };
        // The evening's purchases and furnishing are kept. A night in
        // progress is not: the game continues at its evening.
        if *phase.get() == Phase::Evening {
            write_save(&state.snapshot(state.progress.night, false));
        }
        if quit_to_menu {
            paused.0 = false;
            next.set(Phase::Menu);
        } else {
            exit.write(AppExit::Success);
        }
    }
}

fn sync_pause(
    mut commands: Commands,
    paused: Res<Paused>,
    phase: Res<State<Phase>>,
    settings: Res<Settings>,
    font: Res<UiFont>,
    theme: Res<UiTheme>,
    lang: Res<Lang>,
    existing: Query<Entity, With<PauseRoot>>,
    mut time: ResMut<Time<Virtual>>,
) {
    for e in &existing {
        commands.entity(e).despawn();
    }
    if !paused.0 {
        time.unpause();
        return;
    }
    time.pause();
    let (layer, panel) = overlay(&mut commands, &theme, 0.6, 420.0);
    commands.entity(layer).insert(PauseRoot);
    let mut ui = PanelBuilder::new(&mut commands, panel, &font);
    ui.heading(&lang.t("pause.title"));
    ui.primary(&lang.t("pause.resume"), Action::Resume);
    let root = ui.root();
    let quit_label = match phase.get() {
        Phase::Night => "pause.quit_night",
        _ => "pause.save_quit",
    };
    ui.menu_option(root, &lang.t(quit_label), Action::QuitToMenu, false);
    let toggles = ui.row();
    ui.menu_option(
        toggles,
        &tips_label(&settings, &lang),
        Action::ToggleTips,
        settings.tips,
    );
    ui.menu_option(
        toggles,
        &sound_label(&settings, &lang),
        Action::ToggleSound,
        settings.sound,
    );
    ui.menu_option(root, &lang.t("pause.quit_game"), Action::QuitGame, false);
    ui.dim(&lang.t("pause.hint"));
}

fn spawn_tip_box(mut commands: Commands) {
    commands.spawn((
        TipBox,
        Node {
            position_type: PositionType::Absolute,
            left: px(16),
            top: px(52),
            width: px(560),
            padding: UiRect::all(px(10)),
            flex_direction: FlexDirection::Column,
            row_gap: px(6),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(6)),
            display: Display::None,
            ..default()
        },
        BackgroundColor(BG_PANEL.with_alpha(0.96)),
        BorderColor::all(PANEL_EDGE),
    ));
}

/// Catalog key of the tip that fits the current situation, if any.
fn current_tip(
    phase: Phase,
    crates: &CratesOrdered,
    layout: &ShopLayout,
    register: &Register,
    front_waiting: bool,
    report: &NightReport,
) -> Option<&'static str> {
    Some(match phase {
        Phase::Evening if crates.0 == 0 => "tip.order",
        Phase::Evening if layout.displayed_products().is_empty() => "tip.shelve",
        Phase::Evening => "tip.open",
        Phase::Night if register.question.is_some() => "tip.double_pay",
        Phase::Night if register.sale.is_some() => "tip.change",
        Phase::Night if front_waiting => "tip.whisper",
        Phase::Night if report.sales == 0 => "tip.browse",
        Phase::Night => "tip.first_sale",
        Phase::Morning => "tip.morning",
        Phase::Menu | Phase::DemoEnd => return None,
    })
}

fn update_tips(
    mut commands: Commands,
    phase: Res<State<Phase>>,
    settings: Res<Settings>,
    lang: Res<Lang>,
    progress: Res<Progress>,
    crates: Res<CratesOrdered>,
    layout: Res<ShopLayout>,
    register: Res<Register>,
    queue: Res<Queue>,
    report: Res<NightReport>,
    ghosts: Query<&Ghost>,
    font: Res<UiFont>,
    tip_box: Single<(Entity, &mut Node), With<TipBox>>,
    mut shown: Local<Option<String>>,
) {
    let front_waiting = queue
        .front()
        .and_then(|e| ghosts.get(e).ok())
        .is_some_and(|g| g.state == GhostState::Waiting);
    let tip = (settings.tips && progress.night == 1)
        .then(|| {
            current_tip(
                *phase.get(),
                &crates,
                &layout,
                &register,
                front_waiting,
                &report,
            )
        })
        .flatten()
        .map(|key| lang.t(key));
    if *shown == tip {
        return;
    }
    *shown = tip.clone();

    let (entity, mut node) = tip_box.into_inner();
    let Some(text) = tip else {
        node.display = Display::None;
        return;
    };
    node.display = Display::Flex;
    let mut ui = PanelBuilder::new(&mut commands, entity, &font);
    ui.text(&text);
    let root = ui.root();
    ui.button_in(root, &lang.t("tips.hide"), Action::ToggleTips, false, true);
}
