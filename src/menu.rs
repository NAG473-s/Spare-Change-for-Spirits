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
    clear_profile, latest_save, newest_save, read_slot, save_now, write_slot,
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
            .init_resource::<BookOrigin>()
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
                    (toggle_pause, pause_actions)
                        .run_if(not(in_state(Phase::Menu)))
                        .run_if(crate::story_film::outside_film),
                    toggle_tips,
                    sync_pause.run_if(crate::story_film::outside_film).run_if(
                        resource_changed::<Paused>
                            .or_else(resource_changed::<SettingsMenu>)
                            .or_else(resource_changed::<SaveMenu>)
                            .or_else(order_page_changed)
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
    /// The achievements of the full game (pause menu).
    Achievements,
    /// The ghost book (pause menu).
    Book,
    GhostProfile(usize),
    /// Hedda's letters.
    Letters,
    /// The keepsakes, and one keepsake's page (index in `GameData::decor`).
    Keepsakes,
    Keepsake(usize),
    RegularHistory(u32, usize),
    FirstSale,
    Orders(usize),
}

impl SaveMenu {
    /// The book chapter this page belongs to, if it is a book page. The
    /// achievements count as one only inside a game (`in_game`).
    fn chapter(self, in_game: bool) -> Option<crate::book::Chapter> {
        use crate::book::Chapter;
        Some(match self {
            SaveMenu::Book | SaveMenu::GhostProfile(_) | SaveMenu::RegularHistory(_, _) => {
                Chapter::Ghosts
            }
            SaveMenu::Keepsakes | SaveMenu::Keepsake(_) => Chapter::Keepsakes,
            SaveMenu::Letters => Chapter::Letters,
            SaveMenu::Orders(_) => Chapter::Orders,
            SaveMenu::FirstSale => Chapter::Guide,
            SaveMenu::Achievements if in_game => Chapter::Achievements,
            _ => return None,
        })
    }

    /// The first page of a chapter.
    fn of_chapter(chapter: crate::book::Chapter) -> Self {
        use crate::book::Chapter;
        match chapter {
            Chapter::Ghosts => SaveMenu::Book,
            Chapter::Keepsakes => SaveMenu::Keepsakes,
            Chapter::Letters => SaveMenu::Letters,
            Chapter::Orders => SaveMenu::Orders(0),
            Chapter::Guide => SaveMenu::FirstSale,
            Chapter::Achievements => SaveMenu::Achievements,
        }
    }
}

/// The game is paused and the pause menu is open.
#[derive(Resource, Default)]
pub struct Paused(pub bool);

/// Where the book was opened. Closing it returns there: to the running shop
/// or harbour view when it was opened from play, to Pause otherwise. `entry`
/// is the page it was opened at; Back on that page closes the book too.
#[derive(Resource, Default)]
struct BookOrigin {
    entry: Option<SaveMenu>,
    from_play: bool,
}

impl BookOrigin {
    fn open(&mut self, page: SaveMenu, paused: &mut Paused, menu: &mut SaveMenu) {
        if !paused.0 {
            self.entry = Some(page);
            self.from_play = true;
        } else if *menu == SaveMenu::Closed {
            self.entry = Some(page);
            self.from_play = false;
        }
        paused.0 = true;
        *menu = page;
    }

    fn close(&mut self, paused: &mut Paused, menu: &mut SaveMenu) {
        *menu = SaveMenu::Closed;
        if self.from_play {
            paused.0 = false;
        }
        self.entry = None;
        self.from_play = false;
    }

    fn back(
        &mut self,
        paused: &mut Paused,
        menu: &mut SaveMenu,
        journal: &crate::book::Journal,
        data: &GameData,
    ) -> bool {
        if !paused.0 {
            return false;
        }
        if self.entry == Some(*menu) {
            self.close(paused, menu);
            return true;
        }
        match book_parent(*menu, journal, data) {
            Some(SaveMenu::Closed) => {
                self.close(paused, menu);
                true
            }
            Some(parent) => {
                *menu = parent;
                true
            }
            None => false,
        }
    }
}

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

/// One page of the book, below its tabs.
fn book_chapter_page(
    ui: &mut PanelBuilder,
    save_menu: &SaveMenu,
    lang: &Lang,
    data: &GameData,
    progress: &Progress,
    shop: &(
        Res<ShopLayout>,
        Res<crate::shop::Storeroom>,
        Res<crate::upgrades::Upgrades>,
        Res<AssetServer>,
        Res<crate::inventory::Stock>,
    ),
    phase: Phase,
) {
    let (layout, storeroom, upgrades, assets, stock) = &shop;
    match *save_menu {
        SaveMenu::Achievements => {
            crate::achievements::achievements_page(ui, lang, data, progress, assets, None)
        }
        SaveMenu::Orders(page) => crate::orders::book_page(ui, lang, progress, stock, page),
        SaveMenu::FirstSale => crate::guide::book_page(ui, lang, progress, phase),
        SaveMenu::RegularHistory(name, page) => {
            crate::book::regular_history_page(ui, lang, &progress.journal, name, page)
        }
        SaveMenu::GhostProfile(index) => {
            crate::book::ghost_profile(ui, lang, data, &progress.journal, index, assets)
        }
        SaveMenu::Letters => crate::book::letters_page(ui, lang, data, &progress.journal, upgrades),
        SaveMenu::Keepsakes => {
            crate::book::keepsakes_page(ui, lang, data, (layout, storeroom), assets)
        }
        SaveMenu::Keepsake(index) => {
            crate::book::keepsake_page(ui, lang, data, progress, (layout, storeroom), index, assets)
        }
        _ => crate::book::book_page(ui, lang, data, &progress.journal),
    }
    // A short page leaves room below; this takes it, so buttons keep
    // their height instead of stretching into it.
    let root = ui.root();
    ui.node(
        root,
        Node {
            flex_grow: BOOK_FILLER,
            ..default()
        },
    );
}

/// Width of the book (all its chapters).
const BOOK_WIDTH: f32 = 640.0;
/// Height of the book's page (it scrolls when a chapter is longer).
const BOOK_HEIGHT: f32 = 520.0;
/// Far more than all buttons' growth together, so the filler takes the room.
const BOOK_FILLER: f32 = 10_000.0;

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
    let demo_nights = data.config.demo_nights;
    match *save_menu {
        SaveMenu::Load => return load_page(&mut ui, &lang, demo_nights),
        SaveMenu::NewGame => return new_game_page(&mut ui, &lang, demo_nights),
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
        // All profiles together: unlocked in any of them.
        SaveMenu::Achievements => {
            let all = crate::achievements::across_profiles();
            return crate::achievements::achievements_page(
                &mut ui,
                &lang,
                &data,
                &all,
                &asset_server,
                Some(Action::CloseSaveMenu),
            );
        }
        SaveMenu::RegularHistory(_, _) => {}
        SaveMenu::Closed
        | SaveMenu::Save
        | SaveMenu::Book
        | SaveMenu::GhostProfile(_)
        | SaveMenu::Letters
        | SaveMenu::Keepsakes
        | SaveMenu::Keepsake(_)
        | SaveMenu::FirstSale
        | SaveMenu::Orders(_) => {}
    }
    ui.text(&lang.t("menu.tagline"));
    ui.rule();
    let any_saves = (1..=PROFILES).any(|p| latest_save(p).is_some());
    if let Some((_, night)) =
        newest_save(demo_nights).and_then(|(_, _, s)| s.resume_at(demo_nights))
    {
        ui.primary(
            &lang.f("menu.continue", &[("night", &night)]),
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
    // The demo has no achievements.
    if !cfg!(feature = "demo") {
        ui.menu_option(
            root,
            &lang.t("pause.achievements"),
            Action::OpenAchievements,
            false,
        );
    }
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
            Action::OpenAchievements => *save_menu = SaveMenu::Achievements,
            Action::NewGameIn(p) if latest_save(*p).is_some() => {
                *save_menu = SaveMenu::Confirm(*p);
            }
            Action::NewGameIn(p) | Action::ConfirmNewGame(p) => {
                clear_profile(*p);
                state.reset(&data);
                if settings.tips {
                    state.progress.journal.first_sale = crate::guide::FirstSale::Following;
                }
                profile.0 = *p;
                log.clear();
                *save_menu = SaveMenu::Closed;
                next.set(Phase::Evening);
            }
            Action::Continue | Action::LoadSlot(_, _) => {
                let selected = match action {
                    Action::LoadSlot(p, slot) => read_slot(*p, *slot).map(|s| (*p, s)),
                    _ => newest_save(data.config.demo_nights).map(|(p, _, s)| (p, s)),
                };
                if let Some((p, save)) = selected
                    && let Some((phase, night)) = save.resume_at(data.config.demo_nights)
                {
                    state.apply(&save, night, &data);
                    profile.0 = p;
                    log.clear();
                    if phase == Phase::Evening {
                        log.push(lang.f("menu.welcome_back", &[("night", &night)]));
                    }
                    *save_menu = SaveMenu::Closed;
                    next.set(phase);
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

fn slot_name(slot: Slot, lang: &Lang) -> String {
    match slot {
        Slot::Auto => lang.t("save.auto"),
        Slot::Manual(n) => lang.f("save.slot", &[("slot", &n)]),
    }
}

/// "Evening of night 3 · money 145 · reputation 12".
fn describe_save(save: &SaveGame, lang: &Lang, demo_nights: u32) -> String {
    let state = match save.resume_at(demo_nights) {
        None => lang.t("save.state_finished"),
        Some((Phase::DemoEnd, _)) => lang.t("end.title_full"),
        Some((Phase::Night, _)) => lang.t("guide_title"),
        Some((Phase::Day, night)) => lang.f("save.state_market", &[("night", &night)]),
        Some((_, night)) => lang.f("save.state_evening", &[("night", &night)]),
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
fn load_page(ui: &mut PanelBuilder, lang: &Lang, demo_nights: u32) {
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
                    ("summary", &describe_save(&save, lang, demo_nights)),
                ],
            );
            if save.resume_at(demo_nights).is_some() {
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
fn new_game_page(ui: &mut PanelBuilder, lang: &Lang, demo_nights: u32) {
    ui.heading(&lang.t("menu.choose_profile"));
    let root = ui.root();
    for p in 1..=PROFILES {
        let label = match latest_save(p) {
            Some((_, save)) => lang.f(
                "menu.profile_used",
                &[
                    ("profile", &p),
                    ("summary", &describe_save(&save, lang, demo_nights)),
                ],
            ),
            None => lang.f("menu.profile_empty", &[("profile", &p)]),
        };
        ui.menu_option(root, &label, Action::NewGameIn(p), false);
    }
    ui.menu_option(root, &lang.t("menu.back"), Action::CloseSaveMenu, false);
}

/// Pause menu: the manual slots of the running profile; a click saves.
fn save_page(
    ui: &mut PanelBuilder,
    lang: &Lang,
    profile: u8,
    restarts_at_evening: bool,
    demo_nights: u32,
) {
    ui.heading(&lang.f("pause.save_title", &[("profile", &profile)]));
    let root = ui.root();
    for n in 1..=MANUAL_SLOTS {
        let slot = Slot::Manual(n);
        let label = match read_slot(profile, slot) {
            Some(save) => lang.f(
                "save.entry",
                &[
                    ("slot", &slot_name(slot, lang)),
                    ("summary", &describe_save(&save, lang, demo_nights)),
                ],
            ),
            None => lang.f("save.slot_empty", &[("slot", &n)]),
        };
        ui.menu_option(root, &label, Action::SaveToSlot(n), false);
    }
    if restarts_at_evening {
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
            Action::ToggleHighContrast => settings.high_contrast = !settings.high_contrast,
            Action::ToggleReducedMotion => settings.reduced_motion = !settings.reduced_motion,
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
    // Seeing and moving: solid panels, and calmer scenes.
    ui.section(&lang.t("settings.display"));
    let display = ui.row();
    let (contrast, calm) = (settings.high_contrast, settings.reduced_motion);
    let contrast_key = if contrast {
        "settings.contrast_on"
    } else {
        "settings.contrast_off"
    };
    let motion_key = if calm {
        "settings.motion_on"
    } else {
        "settings.motion_off"
    };
    ui.menu_option(
        display,
        &lang.t(contrast_key),
        Action::ToggleHighContrast,
        contrast,
    );
    ui.menu_option(
        display,
        &lang.t(motion_key),
        Action::ToggleReducedMotion,
        calm,
    );
    ui.dim(&lang.t("settings.display_hint"));
    ui.dim(&lang.t("settings.keyboard_hint"));
    ui.menu_option(root, &lang.t("credits.open"), Action::OpenCredits, false);
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
    gamepads: Query<&Gamepad>,
    mut paused: ResMut<Paused>,
    mut save_menu: ResMut<SaveMenu>,
    mut settings_menu: ResMut<SettingsMenu>,
    progress: Res<Progress>,
    data: Res<GameData>,
    mut origin: ResMut<BookOrigin>,
) {
    if keys.just_pressed(KeyCode::Escape) || crate::gamepad::start_pressed(&gamepads) {
        if origin.back(&mut paused, &mut save_menu, &progress.journal, &data) {
            return;
        }
        paused.0 = !paused.0;
        *save_menu = SaveMenu::Closed;
        settings_menu.0 = false;
    }
}

fn book_parent(
    page: SaveMenu,
    journal: &crate::book::Journal,
    data: &GameData,
) -> Option<SaveMenu> {
    match page {
        // A chapter's first page: Back closes the book.
        SaveMenu::Book
        | SaveMenu::Keepsakes
        | SaveMenu::Letters
        | SaveMenu::FirstSale
        | SaveMenu::Achievements
        | SaveMenu::Orders(0) => Some(SaveMenu::Closed),
        SaveMenu::GhostProfile(_) => Some(SaveMenu::Book),
        SaveMenu::Keepsake(_) => Some(SaveMenu::Keepsakes),
        SaveMenu::Orders(page) => Some(SaveMenu::Orders(page - 1)),
        SaveMenu::RegularHistory(name, 0) => Some(
            journal
                .regulars
                .iter()
                .find(|r| r.name == name && journal.has_seen(&r.ghost))
                .and_then(|r| data.ghost_index(&r.ghost))
                .map_or(SaveMenu::Book, SaveMenu::GhostProfile),
        ),
        SaveMenu::RegularHistory(name, page) => Some(SaveMenu::RegularHistory(name, page - 1)),
        _ => None,
    }
}

fn pause_actions(
    mut actions: MessageReader<Action>,
    phase: Res<State<Phase>>,
    mut state: GameState,
    data: Res<GameData>,
    lang: Res<Lang>,
    profile: Res<ActiveProfile>,
    (night_start, register): (Res<NightStart>, Res<Register>),
    mut save_menu: ResMut<SaveMenu>,
    mut log: ResMut<GameLog>,
    mut paused: ResMut<Paused>,
    mut origin: ResMut<BookOrigin>,
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
            Action::OpenOrders(page) => {
                origin.open(SaveMenu::Orders(*page), &mut paused, &mut save_menu);
                continue;
            }
            Action::PinOrder(index) if matches!(*save_menu, SaveMenu::Orders(_)) => {
                if let Some(order) = state.progress.orders.get_mut(*index)
                    && order.state == crate::orders::OrderState::Open
                {
                    order.pinned = !order.pinned;
                    save_menu.set_changed();
                    if let Some(save) =
                        save_now(*phase.get(), &state, &data, &night_start, &register)
                    {
                        write_slot(profile.0, Slot::Auto, &save);
                    }
                }
                continue;
            }
            Action::SkipGuide if state.progress.guiding_first_sale() => {
                state.progress.journal.first_sale = crate::guide::FirstSale::Skipped;
                state.progress.journal.first_sale_checkpoint = None;
                if let Some(save) = save_now(*phase.get(), &state, &data, &night_start, &register) {
                    write_slot(profile.0, Slot::Auto, &save);
                }
                continue;
            }
            Action::ResumeGuide if *phase.get() == Phase::Evening && state.progress.night == 1 => {
                state.progress.journal.first_sale = crate::guide::FirstSale::Following;
                paused.0 = false;
                *save_menu = SaveMenu::Closed;
                continue;
            }
            Action::OpenSave => {
                *save_menu = SaveMenu::Save;
                continue;
            }
            Action::OpenAchievements => {
                *save_menu = SaveMenu::Achievements;
                continue;
            }
            // From the pause menu or straight from the evening panel.
            Action::RegularHistory(name, page) => {
                if state
                    .progress
                    .journal
                    .regulars
                    .iter()
                    .any(|r| r.name == *name && state.progress.journal.has_seen(&r.ghost))
                {
                    *save_menu = SaveMenu::RegularHistory(*name, *page);
                }
                continue;
            }
            Action::GhostProfile(index) => {
                if data
                    .ghosts
                    .get(*index)
                    .is_some_and(|g| state.progress.journal.has_seen(&g.id))
                {
                    origin.open(SaveMenu::GhostProfile(*index), &mut paused, &mut save_menu);
                }
                continue;
            }
            Action::BookChapter(chapter) => {
                origin.open(SaveMenu::of_chapter(*chapter), &mut paused, &mut save_menu);
                continue;
            }
            Action::KeepsakePage(index) => {
                origin.open(SaveMenu::Keepsake(*index), &mut paused, &mut save_menu);
                continue;
            }
            Action::BookClose => {
                origin.close(&mut paused, &mut save_menu);
                continue;
            }
            Action::OpenBook => {
                origin.open(SaveMenu::Book, &mut paused, &mut save_menu);
                continue;
            }
            Action::BookBack => {
                origin.back(&mut paused, &mut save_menu, &state.progress.journal, &data);
                continue;
            }
            Action::CloseSaveMenu => {
                *save_menu = SaveMenu::Closed;
                continue;
            }
            Action::SaveToSlot(n) => {
                if let Some(save) = save_now(*phase.get(), &state, &data, &night_start, &register) {
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
        // Store the current preparation or the restart point for this night.
        if matches!(phase.get(), Phase::Evening | Phase::Day | Phase::Night)
            && let Some(save) = save_now(*phase.get(), &state, &data, &night_start, &register)
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

fn order_page_changed(
    menu: Res<SaveMenu>,
    progress: Res<Progress>,
    stock: Res<crate::inventory::Stock>,
) -> bool {
    matches!(*menu, SaveMenu::Orders(_)) && (progress.is_changed() || stock.is_changed())
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
    progress: Res<Progress>,
    shop: (
        Res<ShopLayout>,
        Res<crate::shop::Storeroom>,
        Res<crate::upgrades::Upgrades>,
        Res<AssetServer>,
        Res<crate::inventory::Stock>,
    ),
    gamepads: Query<&Gamepad>,
) {
    for e in &existing {
        commands.entity(e).despawn();
    }
    if !paused.0 {
        time.unpause();
        return;
    }
    time.pause();
    // Book pages share one wider overlay with the chapter tabs on top.
    let chapter = (!settings_menu.0)
        .then(|| save_menu.chapter(true))
        .flatten();
    if let Some(chapter) = chapter {
        let (layer, header, page) =
            crate::ui::book_overlay(&mut commands, &theme, BOOK_WIDTH, BOOK_HEIGHT);
        commands.entity(layer).insert(PauseRoot);
        // The tabs stay at the top; only the page below them scrolls.
        crate::book::tabs(
            &mut PanelBuilder::new(&mut commands, header, &font),
            &lang,
            chapter,
        );
        let mut ui = PanelBuilder::new(&mut commands, page, &font);
        book_chapter_page(
            &mut ui,
            &save_menu,
            &lang,
            &data,
            &progress,
            &shop,
            *phase.get(),
        );
        return;
    }
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
            *phase.get() == Phase::Night
                && !(progress.night == 1
                    && progress.journal.first_sale != crate::guide::FirstSale::Skipped),
            data.config.demo_nights,
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
        Phase::Night if !progress.guiding_first_sale() => "pause.quit_night",
        _ => "pause.save_quit",
    };
    ui.menu_option(root, &lang.t(quit_label), Action::QuitToMenu, false);
    // The achievements are a chapter of the book.
    ui.menu_option(root, &lang.t("book.open"), Action::OpenBook, false);
    ui.menu_option(root, &lang.t("menu.settings"), Action::OpenSettings, false);
    ui.menu_option(root, &lang.t("pause.quit_game"), Action::QuitGame, false);
    ui.dim(&lang.t("pause.hint"));
    if !gamepads.is_empty() {
        ui.dim(&lang.t("pause.controller"));
    }
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
        // The accounts cover this corner at dawn: the side panel shows the tip.
        Phase::Morning => return None,
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
    let tip = (settings.tips && progress.night == 1 && !progress.guiding_first_sale())
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
            let crates = upgrades.crates_tonight(&data, progress.calendar(&data));
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

    #[test]
    fn back_returns_to_the_running_view_only_at_its_book_entry_point() {
        let data = GameData::load();
        let mut journal = crate::book::Journal::default();
        journal.met("shy", "lavender");
        journal.regulars.push(
            ron::from_str(r#"(ghost:"shy", name:1, visits:4, favourite:"lavender")"#).unwrap(),
        );
        let profile = SaveMenu::GhostProfile(data.ghost_index("shy").unwrap());
        let mut origin = BookOrigin::default();
        let mut paused = Paused::default();
        let mut page = SaveMenu::Closed;

        // The expected guest in the evening panel opens its profile directly.
        origin.open(profile, &mut paused, &mut page);
        assert!(paused.0 && page == profile);
        page = SaveMenu::RegularHistory(1, 0);
        assert!(origin.back(&mut paused, &mut page, &journal, &data));
        assert!(paused.0 && page == profile);
        assert!(origin.back(&mut paused, &mut page, &journal, &data));
        assert!(!paused.0 && page == SaveMenu::Closed);

        // The same links work from an index opened during play or from Pause.
        for from_pause in [false, true] {
            paused.0 = from_pause;
            page = SaveMenu::Closed;
            origin.open(SaveMenu::Book, &mut paused, &mut page);
            origin.open(profile, &mut paused, &mut page);
            assert!(origin.back(&mut paused, &mut page, &journal, &data));
            assert!(paused.0 && page == SaveMenu::Book);
            // Turning to another chapter keeps where the book was opened.
            origin.open(SaveMenu::Orders(1), &mut paused, &mut page);
            assert!(origin.back(&mut paused, &mut page, &journal, &data));
            assert!(paused.0 && page == SaveMenu::Orders(0));
            // Back on a chapter's first page closes the book.
            assert!(origin.back(&mut paused, &mut page, &journal, &data));
            assert!(page == SaveMenu::Closed);
            assert_eq!(paused.0, from_pause);
        }

        // Close goes straight back, from any page.
        paused.0 = false;
        page = SaveMenu::Closed;
        origin.open(SaveMenu::Keepsakes, &mut paused, &mut page);
        origin.open(SaveMenu::Keepsake(3), &mut paused, &mut page);
        origin.close(&mut paused, &mut page);
        assert!(!paused.0 && page == SaveMenu::Closed);
    }

    #[test]
    fn every_book_page_belongs_to_a_chapter_with_a_first_page() {
        use crate::book::Chapter;
        for page in [
            SaveMenu::Book,
            SaveMenu::GhostProfile(0),
            SaveMenu::RegularHistory(1, 2),
            SaveMenu::Keepsakes,
            SaveMenu::Keepsake(0),
            SaveMenu::Letters,
            SaveMenu::Orders(2),
            SaveMenu::FirstSale,
        ] {
            let chapter = page.chapter(true).unwrap();
            assert_eq!(SaveMenu::of_chapter(chapter).chapter(true), Some(chapter));
        }
        // The main menu's achievements are no book page; in a game they are.
        assert_eq!(SaveMenu::Achievements.chapter(false), None);
        assert_eq!(
            SaveMenu::Achievements.chapter(true),
            Some(Chapter::Achievements)
        );
    }

    #[test]
    fn back_from_a_regulars_history_retraces_the_profile_and_index() {
        let data = GameData::load();
        let mut journal = crate::book::Journal::default();
        journal.regulars.push(
            ron::from_str(r#"(ghost:"shy", name:1, visits:4, favourite:"lavender")"#).unwrap(),
        );
        assert!(
            book_parent(SaveMenu::RegularHistory(1, 0), &journal, &data) == Some(SaveMenu::Book)
        );
        journal.met("shy", "lavender");
        let ghost = SaveMenu::GhostProfile(data.ghost_index("shy").unwrap());
        assert!(
            book_parent(SaveMenu::RegularHistory(1, 1), &journal, &data)
                == Some(SaveMenu::RegularHistory(1, 0))
        );
        assert!(book_parent(SaveMenu::RegularHistory(1, 0), &journal, &data) == Some(ghost));
        assert!(book_parent(ghost, &journal, &data) == Some(SaveMenu::Book));
        assert!(book_parent(SaveMenu::Book, &journal, &data) == Some(SaveMenu::Closed));
    }

    /// The main menu calls the build a demo only when it is one.
    #[test]
    fn version_line_names_the_demo_only_in_the_demo() {
        let line = Lang::load("en").f(version_key(), &[("version", &"1.2.3")]);
        assert!(line.contains("1.2.3"), "{line}");
        assert_eq!(cfg!(feature = "demo"), line.contains("Demo"), "{line}");
    }
}
