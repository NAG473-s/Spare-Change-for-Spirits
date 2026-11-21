//! A quiet pause after a season's finale, before the accounts: at dawn the
//! view goes out to the quay (for the lighthouse story), where Ida's restored
//! beam sweeps the water, and the finale guest's letter lies waiting. Nothing
//! is timed – the player reads, then continues to the accounts. The letter
//! stays in the book's Letters chapter (`Journal::finale_letters`).
//!
//! Texts: `finale.<guest>.scene` (what the scene shows, in words) and
//! `finale.<guest>.letter`.

use bevy::prelude::*;

use crate::data::GameData;
use crate::day_cycle::{NightReport, Phase, Progress};
use crate::harbor::Location;
use crate::i18n::Lang;
use crate::ui::{Action, PanelBuilder, UiFont, UiTheme};

pub struct FinalePlugin;

impl Plugin for FinalePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FinaleScene>()
            .add_systems(
                OnEnter(Phase::Morning),
                begin.before(crate::accounts::LedgerSystems),
            )
            .add_systems(OnExit(Phase::Morning), end)
            .add_systems(
                Update,
                (
                    show.run_if(
                        resource_changed::<FinaleScene>.or_else(resource_changed::<UiFont>),
                    ),
                    continue_to_accounts,
                )
                    .run_if(in_state(Phase::Morning)),
            );
    }
}

/// The finale guest whose scene is showing, if any.
#[derive(Resource, Default)]
pub struct FinaleScene(pub Option<String>);

/// No finale scene is showing (the accounts may appear).
pub fn no_scene(scene: Res<FinaleScene>) -> bool {
    scene.0.is_none()
}

#[derive(Component)]
struct SceneRoot;

/// Width of the letter at the bottom of the scene.
const LETTER_WIDTH: f32 = 640.0;

/// The finale guest said goodbye tonight: begin the scene at dawn.
fn begin(
    data: Res<GameData>,
    report: Res<NightReport>,
    mut progress: ResMut<Progress>,
    mut scene: ResMut<FinaleScene>,
    mut location: ResMut<Location>,
) {
    let Some(story) = data.stories.iter().find(|s| {
        s.finale_guest
            .as_ref()
            .is_some_and(|g| report.story_lines.iter().any(|(who, _)| who == g))
    }) else {
        return;
    };
    let guest = story
        .finale_guest
        .clone()
        .expect("found by its finale guest");
    // The letter can be read again in the book.
    if !progress.journal.finale_letters.contains(&guest) {
        progress.journal.finale_letters.push(guest.clone());
    }
    // Out to the quay, where the restored lighthouse can be seen.
    if story.lighthouse {
        *location = Location::Harbor;
    }
    scene.0 = Some(guest);
}

fn show(
    mut commands: Commands,
    scene: Res<FinaleScene>,
    font: Res<UiFont>,
    theme: Res<UiTheme>,
    lang: Res<Lang>,
    old: Query<Entity, With<SceneRoot>>,
) {
    for e in &old {
        commands.entity(e).despawn();
    }
    let Some(guest) = &scene.0 else {
        return;
    };
    // At the bottom, so the harbour and the beam stay in view.
    let layer = commands
        .spawn((
            SceneRoot,
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(crate::ui::PANEL_WIDTH),
                bottom: px(24),
                justify_content: JustifyContent::Center,
                ..default()
            },
            GlobalZIndex(90),
            Pickable::IGNORE,
        ))
        .id();
    let frame = commands
        .spawn((
            Node {
                width: px(LETTER_WIDTH),
                max_width: percent(96),
                flex_direction: FlexDirection::Column,
                border_radius: BorderRadius::all(px(6)),
                ..default()
            },
            BackgroundColor(crate::ui::BG_PANEL),
            ChildOf(layer),
        ))
        .id();
    let body = theme.panel_body(&mut commands, frame, 0.36, 22.0, 8.0);
    let mut ui = PanelBuilder::new(&mut commands, body, &font);
    ui.dim(&lang.t(&format!("finale.{guest}.scene")));
    ui.section(&lang.t("letter.title"));
    ui.text(&lang.t(&format!("finale.{guest}.letter")));
    ui.primary(&lang.t("finale.continue"), Action::FinaleContinue);
}

/// The player has read the letter: back to the shop and on to the accounts.
fn continue_to_accounts(
    mut actions: MessageReader<Action>,
    mut scene: ResMut<FinaleScene>,
    mut location: ResMut<Location>,
) {
    for action in actions.read() {
        if *action == Action::FinaleContinue && scene.0.is_some() {
            scene.0 = None;
            *location = Location::Shop;
        }
    }
}

/// Leaving the morning (or quitting to the menu) ends any scene.
fn end(mut scene: ResMut<FinaleScene>, mut location: ResMut<Location>) {
    if scene.0.take().is_some() {
        *location = Location::Shop;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hedda_s_farewell_opens_the_quay_scene_and_continue_ends_it() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = App::new();
        app.add_message::<Action>()
            .insert_resource(GameData::load())
            .insert_resource(Progress::new(0))
            .init_resource::<Location>()
            .init_resource::<FinaleScene>()
            .insert_resource(NightReport::default());
        let world = app.world_mut();
        // An ordinary dawn: no scene, the accounts come at once.
        world.run_system_once(begin).unwrap();
        assert!(world.resource::<FinaleScene>().0.is_none());
        // Hedda said goodbye tonight.
        world
            .resource_mut::<NightReport>()
            .story_lines
            .push(("hedda".into(), "finale.hedda.farewell".into()));
        world.run_system_once(begin).unwrap();
        assert_eq!(world.resource::<FinaleScene>().0.as_deref(), Some("hedda"));
        assert_eq!(*world.resource::<Location>(), Location::Harbor);
        assert_eq!(
            world.resource::<Progress>().journal.finale_letters,
            ["hedda"]
        );
        world.write_message(Action::FinaleContinue);
        world.run_system_once(continue_to_accounts).unwrap();
        assert!(world.resource::<FinaleScene>().0.is_none());
        assert_eq!(*world.resource::<Location>(), Location::Shop);
    }

    #[test]
    fn every_finale_guest_has_a_scene_and_a_letter() {
        let data = GameData::load();
        let lang = Lang::load("en");
        for guest in data.stories.iter().filter_map(|s| s.finale_guest.as_ref()) {
            for key in [
                format!("finale.{guest}.scene"),
                format!("finale.{guest}.letter"),
            ] {
                assert_ne!(lang.t(&key), key, "missing {key}");
            }
        }
    }
}
