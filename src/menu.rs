//! Main menu, pause menu (Esc) and the tips box of the first night.

use bevy::prelude::*;

use crate::data::GameData;
use crate::day_cycle::{NightReport, Phase, Progress};
use crate::ghosts::{Ghost, GhostState, Queue};
use crate::inventory::CratesOrdered;
use crate::register::Register;
use crate::save::{GameState, Settings, read_save, write_save};
use crate::shop::ShopLayout;
use crate::ui::{Action, BG_PANEL, GameLog, PanelBuilder, TEXT_DIM, UiFont};

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Paused>()
            .add_systems(Startup, spawn_tip_box)
            .add_systems(OnEnter(Phase::Menu), (leave_pause, build_main_menu).chain())
            .add_systems(
                Update,
                (
                    build_main_menu
                        .run_if(in_state(Phase::Menu).and_then(resource_changed::<Settings>)),
                    main_menu_actions.run_if(in_state(Phase::Menu)),
                    (toggle_pause, pause_actions).run_if(not(in_state(Phase::Menu))),
                    toggle_tips,
                    sync_pause
                        .run_if(resource_changed::<Paused>.or_else(resource_changed::<Settings>)),
                    update_tips,
                ),
            );
    }
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

/// Full-screen dimmed layer with a centred panel on top of everything.
/// Returns (layer, panel).
fn overlay(commands: &mut Commands, dim: f32) -> (Entity, Entity) {
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
            BackgroundColor(Color::srgba(0.02, 0.02, 0.05, dim)),
            GlobalZIndex(100),
        ))
        .id();
    let panel = commands
        .spawn((
            Node {
                width: px(420),
                padding: UiRect::all(px(24)),
                flex_direction: FlexDirection::Column,
                row_gap: px(10),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(8)),
                ..default()
            },
            BackgroundColor(BG_PANEL),
            BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.1)),
            ChildOf(layer),
        ))
        .id();
    (layer, panel)
}

fn tips_label(settings: &Settings) -> &'static str {
    if settings.tips {
        "Tips: on"
    } else {
        "Tips: off"
    }
}

fn build_main_menu(
    mut commands: Commands,
    font: Res<UiFont>,
    settings: Res<Settings>,
    existing: Query<Entity, With<MenuRoot>>,
) {
    for e in &existing {
        commands.entity(e).despawn();
    }
    let (layer, panel) = overlay(&mut commands, 0.97);
    commands
        .entity(layer)
        .insert((MenuRoot, DespawnOnExit(Phase::Menu)));

    let mut ui = PanelBuilder::new(&mut commands, panel, &font);
    ui.heading("Spare Change for Spirits");
    ui.dim(&format!("Demo · version {}", env!("CARGO_PKG_VERSION")));
    ui.text("A little shop that only opens between midnight and dawn.");
    let root = ui.root();
    if let Some(save) = read_save().filter(|s| !s.finished) {
        ui.primary(
            &format!("Continue – night {}", save.night),
            Action::Continue,
        );
        ui.button_in(root, "New game", Action::NewGame, false, true);
    } else {
        ui.primary("New game", Action::NewGame);
    }
    ui.button_in(root, tips_label(&settings), Action::ToggleTips, false, true);
    ui.button_in(root, "Quit", Action::QuitGame, false, true);
}

fn leave_pause(mut paused: ResMut<Paused>, mut time: ResMut<Time<Virtual>>) {
    paused.0 = false;
    time.unpause();
}

fn main_menu_actions(
    mut actions: MessageReader<Action>,
    data: Res<GameData>,
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
                    log.push(format!("Welcome back. Evening of night {}.", save.night));
                    next.set(Phase::Evening);
                }
            }
            Action::QuitGame => {
                exit.write(AppExit::Success);
            }
            _ => {}
        }
    }
}

fn toggle_tips(mut actions: MessageReader<Action>, mut settings: ResMut<Settings>) {
    for action in actions.read() {
        if *action == Action::ToggleTips {
            settings.tips = !settings.tips;
            settings.store();
        }
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
    let (layer, panel) = overlay(&mut commands, 0.6);
    commands.entity(layer).insert(PauseRoot);
    let mut ui = PanelBuilder::new(&mut commands, panel, &font);
    ui.heading("Paused");
    ui.primary("Resume", Action::Resume);
    let root = ui.root();
    let quit_label = match phase.get() {
        Phase::Night => "Quit to menu (this night restarts)",
        _ => "Save and quit to menu",
    };
    ui.button_in(root, quit_label, Action::QuitToMenu, false, true);
    ui.button_in(root, tips_label(&settings), Action::ToggleTips, false, true);
    ui.button_in(root, "Quit game", Action::QuitGame, false, true);
    ui.dim("Press Esc to resume.");
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
        BackgroundColor(Color::srgba(0.14, 0.11, 0.24, 0.94)),
        BorderColor::all(TEXT_DIM.with_alpha(0.4)),
    ));
}

/// Which tip fits the current situation, if any.
fn current_tip<'a>(
    data: &'a GameData,
    phase: Phase,
    crates: &CratesOrdered,
    layout: &ShopLayout,
    register: &Register,
    front_waiting: bool,
    report: &NightReport,
) -> Option<&'a str> {
    let tips = &data.tips;
    let tip = match phase {
        Phase::Evening if crates.0 == 0 => &tips.order,
        Phase::Evening if layout.displayed_products().is_empty() => &tips.shelve,
        Phase::Evening => &tips.open,
        Phase::Night if register.question.is_some() => &tips.double_pay,
        Phase::Night if register.sale.is_some() => &tips.change,
        Phase::Night if front_waiting => &tips.whisper,
        Phase::Night if report.sales == 0 => &tips.browse,
        Phase::Night => &tips.first_sale,
        Phase::Morning => &tips.morning,
        Phase::Menu | Phase::DemoEnd => return None,
    };
    Some(tip.as_str())
}

fn update_tips(
    mut commands: Commands,
    phase: Res<State<Phase>>,
    settings: Res<Settings>,
    data: Res<GameData>,
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
                &data,
                *phase.get(),
                &crates,
                &layout,
                &register,
                front_waiting,
                &report,
            )
        })
        .flatten()
        .map(str::to_string);
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
    ui.button_in(root, "Hide tips", Action::ToggleTips, false, true);
}
