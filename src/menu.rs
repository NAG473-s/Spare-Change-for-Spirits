//! Main menu, pause menu (Esc) and the tips box of the first night.

use bevy::prelude::*;

use crate::data::{GameData, asset_exists};
use crate::day_cycle::{NightReport, Phase, Progress};
use crate::ghosts::{Ghost, GhostState, Queue};
use crate::i18n::{AutomaticLanguage, Lang};
use crate::inventory::CratesOrdered;
use crate::register::Register;
use bevy::window::{MonitorSelection, PrimaryWindow, WindowMode};

use crate::save::{
    ActiveProfile, GameState, Knob, MANUAL_SLOTS, NightStart, PROFILES, SaveGame, Settings, Slot,
    can_continue, clear_profile, latest_save, newest_save, read_slot, save_now, write_slot,
};
use crate::shop::ShopLayout;
use crate::title::{LOGO, Opening, TitleLogo};
use crate::ui::{
    Action, BG_PANEL, GameLog, PANEL_EDGE, PanelBuilder, TEXT, UiFont, UiTheme, overlay,
};

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Paused>()
            .init_resource::<LanguageMenu>()
            .init_resource::<SettingsMenu>()
            .init_resource::<SaveMenu>()
            .add_systems(PreUpdate, apply_display)
            .add_systems(Startup, spawn_tip_box)
            .add_systems(OnEnter(Phase::Menu), (leave_pause, build_main_menu).chain())
            .add_systems(
                Update,
                (
                    build_main_menu.run_if(
                        in_state(Phase::Menu).and_then(
                            resource_removed::<Opening>
                                .or_else(resource_changed::<Settings>)
                                .or_else(resource_changed::<UiFont>)
                                .or_else(resource_changed::<LanguageMenu>)
                                .or_else(resource_changed::<SettingsMenu>)
                                .or_else(resource_changed::<SaveMenu>),
                        ),
                    ),
                    main_menu_actions.run_if(in_state(Phase::Menu)),
                    (toggle_pause, pause_actions).run_if(not(in_state(Phase::Menu))),
                    toggle_tips,
                    sync_pause.run_if(
                        resource_changed::<Paused>
                            .or_else(resource_changed::<SettingsMenu>)
                            .or_else(resource_changed::<SaveMenu>)
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

/// The settings page is open (in the main or the pause menu).
#[derive(Resource, Default)]
pub struct SettingsMenu(pub bool);

/// A page about saves is open: the list of saves and the profile choice in
/// the main menu, the save slots in the pause menu.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq)]
pub enum SaveMenu {
    #[default]
    Closed,
    Load,
    NewGame,
    /// Asks before a new game replaces the saves of this profile.
    Confirm(u8),
    Save,
}

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
                min_height: px(0),
                flex_direction: FlexDirection::Column,
                border_radius: BorderRadius::all(px(6)),
                ..default()
            },
            BackgroundColor(BG_PANEL),
            ChildOf(column),
        ))
        .id();
    let panel = theme.panel_body(commands, frame, 0.37, 25.0, 8.0);
    crate::ui::scroll_y(commands, panel);
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
    opening: Option<Res<Opening>>,
    asset_server: Res<AssetServer>,
    font: Res<UiFont>,
    theme: Res<UiTheme>,
    lang: Res<Lang>,
    settings: Res<Settings>,
    language_menu: Res<LanguageMenu>,
    settings_menu: Res<SettingsMenu>,
    save_menu: Res<SaveMenu>,
    data: Res<GameData>,
    existing: Query<Entity, With<MenuRoot>>,
) {
    if opening.is_some() {
        return;
    }
    for e in &existing {
        commands.entity(e).despawn();
    }
    let wide = language_menu.0 || matches!(*save_menu, SaveMenu::Load | SaveMenu::NewGame);
    let width = if wide { 560.0 } else { 386.0 };
    let (layer, panel, has_logo) = title_layout(&mut commands, &asset_server, &theme, width);
    commands
        .entity(layer)
        .insert((MenuRoot, DespawnOnExit(Phase::Menu)));
    let mut ui = PanelBuilder::new(&mut commands, panel, &font);
    let root = ui.root();

    if language_menu.0 {
        ui.heading(&lang.t("menu.choose_language"));
        ui.menu_option(
            root,
            &lang.t("menu.language_auto"),
            Action::SetLanguage(None),
            settings.language.is_none(),
        );
        let row = ui.row();
        for l in &lang.languages {
            let selected = settings.language.as_ref() == Some(&l.code);
            ui.menu_option(
                row,
                &l.name,
                Action::SetLanguage(Some(l.code.clone())),
                selected,
            );
        }
        ui.menu_option(root, &lang.t("menu.back"), Action::CloseLanguages, false);
        return;
    }

    if settings_menu.0 {
        settings_page(&mut ui, &settings, &lang);
        return;
    }
    if !has_logo {
        ui.heading(&lang.t("game.title"));
    }
    let final_night = data.config.final_night();
    match *save_menu {
        SaveMenu::Load => return load_page(&mut ui, &lang, final_night),
        SaveMenu::NewGame => return new_game_page(&mut ui, &lang, final_night),
        SaveMenu::Confirm(profile) => {
            ui.heading(&lang.f("menu.overwrite_title", &[("profile", &profile)]));
            ui.text(&lang.t("menu.overwrite_text"));
            ui.primary(
                &lang.t("menu.overwrite_confirm"),
                Action::ConfirmNewGame(profile),
            );
            ui.menu_option(root, &lang.t("menu.back"), Action::OpenNewGame, false);
            return;
        }
        SaveMenu::Closed | SaveMenu::Save => {}
    }
    ui.text(&lang.t("menu.tagline"));
    ui.rule();
    let any_saves = (1..=PROFILES).any(|p| latest_save(p).is_some());
    if let Some((_, _, save)) = newest_save().filter(|(_, _, s)| can_continue(s, final_night)) {
        ui.primary(
            &lang.f("menu.continue", &[("night", &save.night)]),
            Action::Continue,
        );
        ui.menu_option(root, &lang.t("menu.load_game"), Action::OpenLoad, false);
        ui.menu_option(root, &lang.t("menu.new_game"), Action::OpenNewGame, false);
    } else if any_saves {
        ui.primary(&lang.t("menu.new_game"), Action::OpenNewGame);
        ui.menu_option(root, &lang.t("menu.load_game"), Action::OpenLoad, false);
    } else {
        // The very first game goes straight into profile 1.
        ui.primary(&lang.t("menu.new_game"), Action::NewGameIn(1));
    }
    let language = lang.def().map(|l| l.name.clone()).unwrap_or_default();
    ui.menu_option(
        root,
        &lang.f("menu.language", &[("language", &language)]),
        Action::OpenLanguages,
        false,
    );
    ui.menu_option(root, &lang.t("menu.settings"), Action::OpenSettings, false);
    ui.menu_option(root, &lang.t("menu.quit"), Action::QuitGame, false);
    ui.dim(&lang.f(version_key(), &[("version", &env!("CARGO_PKG_VERSION"))]));
}

/// The version line names the demo only in the demo build (`--features demo`).
fn version_key() -> &'static str {
    if cfg!(feature = "demo") {
        "menu.version"
    } else {
        "menu.version_full"
    }
}

fn leave_pause(mut paused: ResMut<Paused>, mut time: ResMut<Time<Virtual>>) {
    paused.0 = false;
    time.unpause();
}

fn main_menu_actions(
    mut actions: MessageReader<Action>,
    data: Res<GameData>,
    mut lang: ResMut<Lang>,
    automatic: Res<AutomaticLanguage>,
    mut settings: ResMut<Settings>,
    mut language_menu: ResMut<LanguageMenu>,
    mut save_menu: ResMut<SaveMenu>,
    mut profile: ResMut<ActiveProfile>,
    mut state: GameState,
    mut log: ResMut<GameLog>,
    mut next: ResMut<NextState<Phase>>,
    mut exit: MessageWriter<AppExit>,
) {
    for action in actions.read() {
        match action {
            Action::OpenLoad => *save_menu = SaveMenu::Load,
            Action::OpenNewGame => *save_menu = SaveMenu::NewGame,
            Action::CloseSaveMenu => *save_menu = SaveMenu::Closed,
            Action::NewGameIn(p) if latest_save(*p).is_some() => {
                *save_menu = SaveMenu::Confirm(*p);
            }
            Action::NewGameIn(p) | Action::ConfirmNewGame(p) => {
                clear_profile(*p);
                state.reset(&data);
                profile.0 = *p;
                log.clear();
                *save_menu = SaveMenu::Closed;
                next.set(Phase::Evening);
            }
            Action::Continue => {
                if let Some((p, _, save)) = newest_save() {
                    load(&save, p, &data, &lang, &mut state, &mut profile, &mut log);
                    *save_menu = SaveMenu::Closed;
                    next.set(continue_phase(&save));
                }
            }
            Action::LoadSlot(p, slot) => {
                if let Some(save) = read_slot(*p, *slot) {
                    load(&save, *p, &data, &lang, &mut state, &mut profile, &mut log);
                    *save_menu = SaveMenu::Closed;
                    next.set(continue_phase(&save));
                }
            }
            Action::QuitGame => {
                exit.write(AppExit::Success);
            }
            Action::OpenLanguages => language_menu.0 = true,
            Action::CloseLanguages => language_menu.0 = false,
            Action::SetLanguage(code) => {
                *lang = Lang::from_preference(code.as_deref(), &automatic);
                settings.language = code.clone();
                settings.store();
                language_menu.0 = false;
            }
            _ => {}
        }
    }
}

/// Puts a save into the running game.
fn load(
    save: &SaveGame,
    p: u8,
    data: &GameData,
    lang: &Lang,
    state: &mut GameState,
    profile: &mut ActiveProfile,
    log: &mut GameLog,
) {
    state.apply(save, data);
    profile.0 = p;
    log.clear();
    if !save.market_open {
        log.push(lang.f("menu.welcome_back", &[("night", &save.night)]));
    }
}

/// A save continues at the market or at an evening.
fn continue_phase(save: &SaveGame) -> Phase {
    if save.market_open {
        Phase::Day
    } else {
        Phase::Evening
    }
}

fn slot_name(slot: Slot, lang: &Lang) -> String {
    match slot {
        Slot::Auto => lang.t("save.auto"),
        Slot::Manual(n) => lang.f("save.slot", &[("slot", &n)]),
    }
}

/// "Evening of night 3 · money 145 · reputation 12".
fn describe_save(save: &SaveGame, lang: &Lang, final_night: u32) -> String {
    let state = if !crate::save::can_continue(save, final_night) {
        lang.t("save.state_finished")
    } else if save.market_open {
        lang.f("save.state_market", &[("night", &save.night)])
    } else {
        lang.f("save.state_evening", &[("night", &save.night)])
    };
    lang.f(
        "save.summary",
        &[
            ("state", &state),
            ("money", &save.drawer.total()),
            ("reputation", &save.reputation),
        ],
    )
}

/// Main menu: every profile with its saves; a click loads one.
fn load_page(ui: &mut PanelBuilder, lang: &Lang, final_night: u32) {
    ui.heading(&lang.t("menu.load_title"));
    let root = ui.root();
    for p in 1..=PROFILES {
        ui.section(&lang.f("menu.profile", &[("profile", &p)]));
        let mut empty = true;
        for slot in Slot::all() {
            let Some(save) = read_slot(p, slot) else {
                continue;
            };
            empty = false;
            let label = lang.f(
                "save.entry",
                &[
                    ("slot", &slot_name(slot, lang)),
                    ("summary", &describe_save(&save, lang, final_night)),
                ],
            );
            if can_continue(&save, final_night) {
                ui.menu_option(root, &label, Action::LoadSlot(p, slot), false);
            } else {
                ui.dim(&label);
            }
        }
        if empty {
            ui.dim(&lang.t("save.none"));
        }
    }
    ui.menu_option(root, &lang.t("menu.back"), Action::CloseSaveMenu, false);
}

/// Main menu: the profile a new game is played in.
fn new_game_page(ui: &mut PanelBuilder, lang: &Lang, final_night: u32) {
    ui.heading(&lang.t("menu.choose_profile"));
    let root = ui.root();
    for p in 1..=PROFILES {
        let label = match latest_save(p) {
            Some((_, save)) => lang.f(
                "menu.profile_used",
                &[
                    ("profile", &p),
                    ("summary", &describe_save(&save, lang, final_night)),
                ],
            ),
            None => lang.f("menu.profile_empty", &[("profile", &p)]),
        };
        ui.menu_option(root, &label, Action::NewGameIn(p), false);
    }
    ui.menu_option(root, &lang.t("menu.back"), Action::CloseSaveMenu, false);
}

/// Pause menu: the manual slots of the running profile; a click saves.
fn save_page(ui: &mut PanelBuilder, lang: &Lang, profile: u8, phase: Phase, final_night: u32) {
    ui.heading(&lang.f("pause.save_title", &[("profile", &profile)]));
    let root = ui.root();
    for n in 1..=MANUAL_SLOTS {
        let slot = Slot::Manual(n);
        let label = match read_slot(profile, slot) {
            Some(save) => lang.f(
                "save.entry",
                &[
                    ("slot", &slot_name(slot, lang)),
                    ("summary", &describe_save(&save, lang, final_night)),
                ],
            ),
            None => lang.f("save.slot_empty", &[("slot", &n)]),
        };
        ui.menu_option(root, &label, Action::SaveToSlot(n), false);
    }
    if phase == Phase::Night {
        ui.dim(&lang.t("pause.save_hint_night"));
    }
    ui.menu_option(root, &lang.t("menu.back"), Action::CloseSaveMenu, false);
}

fn toggle_tips(
    mut actions: MessageReader<Action>,
    mut settings: ResMut<Settings>,
    mut menu: ResMut<SettingsMenu>,
) {
    for action in actions.read() {
        match action {
            Action::ToggleTips => settings.tips = !settings.tips,
            Action::ToggleSound => settings.sound = !settings.sound,
            Action::ToggleFullscreen => settings.fullscreen = !settings.fullscreen,
            Action::TogglePatientGhosts => settings.patient_ghosts = !settings.patient_ghosts,
            Action::ToggleChangeHint => settings.change_hint = !settings.change_hint,
            Action::Adjust(knob, up) => settings.adjust(*knob, *up),
            Action::OpenSettings => {
                menu.0 = true;
                continue;
            }
            Action::CloseSettings => {
                menu.0 = false;
                continue;
            }
            _ => continue,
        }
        settings.store();
    }
}

/// The settings page: tips and sound, volumes, interface size, fullscreen
/// and the assists.
fn settings_page(ui: &mut PanelBuilder, settings: &Settings, lang: &Lang) {
    ui.heading(&lang.t("settings.title"));
    let root = ui.root();
    let toggles = ui.row();
    ui.menu_option(
        toggles,
        &tips_label(settings, lang),
        Action::ToggleTips,
        settings.tips,
    );
    ui.menu_option(
        toggles,
        &sound_label(settings, lang),
        Action::ToggleSound,
        settings.sound,
    );
    let percent = |v: f32| ((v * 100.0).round() as i32).to_string();
    for (knob, key, value) in [
        (Knob::Music, "settings.music", settings.music),
        (Knob::Effects, "settings.effects", settings.effects),
        (Knob::Narration, "settings.narration", settings.narration),
        (Knob::UiSize, "settings.ui_size", settings.ui_size),
    ] {
        // Label on the left, small step buttons on the right.
        let row = ui.node(
            root,
            Node {
                align_items: AlignItems::Center,
                column_gap: px(4),
                ..default()
            },
        );
        let label = ui.node(
            row,
            (
                Node {
                    flex_grow: 1.0,
                    padding: UiRect::left(px(6)),
                    ..default()
                },
                Pickable::IGNORE,
            ),
        );
        let text = lang.f(key, &[("percent", &percent(value))]);
        ui.add_text(label, &text, false, 14.0, TEXT);
        ui.button_in(row, "−", Action::Adjust(knob, false), false, true);
        ui.button_in(row, "+", Action::Adjust(knob, true), false, true);
    }
    let fullscreen = if settings.fullscreen {
        "settings.fullscreen_on"
    } else {
        "settings.fullscreen_off"
    };
    ui.menu_option(
        root,
        &lang.t(fullscreen),
        Action::ToggleFullscreen,
        settings.fullscreen,
    );

    // Assists: play without time pressure, or with help at the register.
    ui.section(&lang.t("settings.assists"));
    let assists = ui.row();
    let (patient, hint) = (settings.patient_ghosts, settings.change_hint);
    let patient_key = if patient {
        "settings.patient_on"
    } else {
        "settings.patient_off"
    };
    let hint_key = if hint {
        "settings.change_hint_on"
    } else {
        "settings.change_hint_off"
    };
    ui.menu_option(
        assists,
        &lang.t(patient_key),
        Action::TogglePatientGhosts,
        patient,
    );
    ui.menu_option(assists, &lang.t(hint_key), Action::ToggleChangeHint, hint);
    ui.dim(&lang.t("settings.assists_hint"));
    ui.menu_option(root, &lang.t("menu.back"), Action::CloseSettings, false);
}

/// The interface grows with the window (designed for 720 pixels high) and
/// the player's size setting; fullscreen follows the setting.
fn apply_display(
    settings: Res<Settings>,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
    mut scale: ResMut<UiScale>,
    mut applied_mode: Local<Option<bool>>,
) {
    let auto = (window.height() / 720.0).clamp(0.8, 3.0);
    let wanted = auto * settings.ui_size;
    if (scale.0 - wanted).abs() > 0.001 {
        scale.0 = wanted;
    }
    if *applied_mode != Some(settings.fullscreen) {
        *applied_mode = Some(settings.fullscreen);
        window.mode = if settings.fullscreen {
            WindowMode::BorderlessFullscreen(MonitorSelection::Current)
        } else {
            WindowMode::Windowed
        };
    }
}

fn toggle_pause(
    keys: Res<ButtonInput<KeyCode>>,
    mut paused: ResMut<Paused>,
    mut save_menu: ResMut<SaveMenu>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        paused.0 = !paused.0;
        *save_menu = SaveMenu::Closed;
    }
}

fn pause_actions(
    mut actions: MessageReader<Action>,
    phase: Res<State<Phase>>,
    state: GameState,
    data: Res<GameData>,
    lang: Res<Lang>,
    profile: Res<ActiveProfile>,
    night_start: Res<NightStart>,
    mut save_menu: ResMut<SaveMenu>,
    mut log: ResMut<GameLog>,
    mut paused: ResMut<Paused>,
    mut next: ResMut<NextState<Phase>>,
    mut exit: MessageWriter<AppExit>,
) {
    for action in actions.read() {
        let quit_to_menu = match action {
            Action::Resume => {
                paused.0 = false;
                *save_menu = SaveMenu::Closed;
                continue;
            }
            Action::Pause => {
                paused.0 = true;
                *save_menu = SaveMenu::Closed;
                continue;
            }
            Action::OpenSave => {
                *save_menu = SaveMenu::Save;
                continue;
            }
            Action::CloseSaveMenu => {
                *save_menu = SaveMenu::Closed;
                continue;
            }
            Action::SaveToSlot(n) => {
                if let Some(save) = save_now(*phase.get(), &state, &data, &night_start) {
                    let slot = Slot::Manual(*n);
                    write_slot(profile.0, slot, &save);
                    log.push(lang.f("log.saved", &[("slot", &slot_name(slot, &lang))]));
                    // Show the slot with its new content.
                    save_menu.set_changed();
                }
                continue;
            }
            Action::QuitToMenu => true,
            Action::QuitGame => false,
            _ => continue,
        };
        // The evening's purchases and furnishing are kept. A night in
        // progress is not: the game continues at its evening.
        if matches!(phase.get(), Phase::Evening | Phase::Day)
            && let Some(save) = save_now(*phase.get(), &state, &data, &night_start)
        {
            write_slot(profile.0, Slot::Auto, &save);
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
    settings_menu: Res<SettingsMenu>,
    phase: Res<State<Phase>>,
    settings: Res<Settings>,
    save_menu: Res<SaveMenu>,
    profile: Res<ActiveProfile>,
    data: Res<GameData>,
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
    if settings_menu.0 {
        settings_page(&mut ui, &settings, &lang);
        return;
    }
    if *save_menu == SaveMenu::Save {
        save_page(
            &mut ui,
            &lang,
            profile.0,
            *phase.get(),
            data.config.final_night(),
        );
        return;
    }
    ui.heading(&lang.t("pause.title"));
    ui.primary(&lang.t("pause.resume"), Action::Resume);
    let root = ui.root();
    if *phase.get() != Phase::DemoEnd {
        ui.menu_option(root, &lang.t("pause.save"), Action::OpenSave, false);
    }
    let quit_label = match phase.get() {
        Phase::Night => "pause.quit_night",
        _ => "pause.save_quit",
    };
    ui.menu_option(root, &lang.t(quit_label), Action::QuitToMenu, false);
    ui.menu_option(root, &lang.t("menu.settings"), Action::OpenSettings, false);
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
        Phase::Day => "tip.market",
        Phase::Menu | Phase::DemoEnd => return None,
    })
}

fn update_tips(
    mut commands: Commands,
    phase: Res<State<Phase>>,
    settings: Res<Settings>,
    data: Res<GameData>,
    upgrades: Res<crate::upgrades::Upgrades>,
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
        .map(|key| {
            let crates = upgrades.crates_tonight(&data, progress.night);
            lang.f(key, &[("count", &crates)])
        });
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The main menu calls the build a demo only when it is one.
    #[test]
    fn version_line_names_the_demo_only_in_the_demo() {
        let line = Lang::load("en").f(version_key(), &[("version", &"1.2.3")]);
        assert!(line.contains("1.2.3"), "{line}");
        assert_eq!(cfg!(feature = "demo"), line.contains("Demo"), "{line}");
    }
}
