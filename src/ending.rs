//! The end of the campaign: the dawn after the last season's finale (the
//! ferryman's own crossing). The view follows Kasimir's last fare down to
//! the quay, where the restored lighthouse shines, then goes to the market
//! with the working tower clock and the flowering window boxes. The player
//! chooses one keepsake for the last page of Hedda's ledger – the choice and
//! the campaign's completion are saved at once – and the credits follow.
//! They can be skipped, and opened again from the settings. Afterwards the
//! accounts, the epilogue and free play continue in the same profile.
//!
//! Nothing is timed. Closing the game at any point keeps the ending: the
//! dawn save already counts the season as finished, so the game continues
//! at the epilogue, which offers the ledger choice if it is still open.

use bevy::prelude::*;
use serde::Deserialize;

use crate::data::{GameData, asset_exists};
use crate::day_cycle::{NightReport, Phase, Progress};
use crate::harbor::Location;
use crate::i18n::Lang;
use crate::shop::{ShopLayout, Storeroom};
use crate::ui::{Action, PanelBuilder, UiFont, UiTheme};

pub struct EndingPlugin;

impl Plugin for EndingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Ending>()
            .init_resource::<CreditsOpen>()
            .add_systems(
                OnEnter(Phase::Morning),
                begin.before(crate::accounts::LedgerSystems),
            )
            .add_systems(OnExit(Phase::Morning), leave)
            .add_systems(Update, (act, show).chain());
    }
}

/// Where the closing sequence is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Quay,
    Market,
    Ledger,
    Credits,
}

/// The closing sequence, while it runs.
#[derive(Resource, Default)]
pub struct Ending(pub Option<Step>);

/// The credits are open (from the ending or from the settings).
#[derive(Resource, Default)]
pub struct CreditsOpen(pub bool);

/// One section of the credits (`assets/data/credits.ron`).
#[derive(Deserialize, Clone, Debug)]
pub struct CreditSection {
    /// `credits.<heading>` in the catalogs.
    pub heading: String,
    /// Names, titles and licences, the same in every language.
    #[serde(default)]
    pub lines: Vec<String>,
    /// A translated line (catalog key), after the names.
    #[serde(default)]
    pub lines_key: Option<String>,
}

#[derive(Component)]
struct EndingRoot;

/// Width of the caption and of the ledger page.
const CAPTION_WIDTH: f32 = 640.0;

/// The story whose finale ends the campaign: the last one with a finale.
pub fn campaign_finale(data: &GameData) -> Option<usize> {
    data.stories
        .iter()
        .enumerate()
        .filter(|(_, s)| s.finale)
        .max_by_key(|(_, s)| s.start_night)
        .map(|(i, _)| i)
}

/// Keepsakes the player owns, for the ledger choice (decor indices).
fn owned_keepsakes(data: &GameData, layout: &ShopLayout, storeroom: &Storeroom) -> Vec<usize> {
    data.decor
        .iter()
        .enumerate()
        .filter(|(_, d)| {
            d.keepsake
                && (storeroom.count(&d.id) > 0
                    || layout
                        .cells
                        .iter()
                        .any(|c| c.decor.as_deref() == Some(&d.id)))
        })
        .map(|(i, _)| i)
        .collect()
}

/// The campaign's last finale was done tonight: the sequence begins.
fn begin(
    data: Res<GameData>,
    report: Res<NightReport>,
    mut progress: ResMut<Progress>,
    mut ending: ResMut<Ending>,
    mut location: ResMut<Location>,
) {
    let Some(story) = campaign_finale(&data) else {
        return;
    };
    let done = progress.chapter(story) >= data.stories[story].chapters.len();
    if !report.story_done.contains(&story) || !done {
        return;
    }
    progress.journal.campaign_complete = true;
    *location = Location::Harbor;
    ending.0 = Some(Step::Quay);
}

/// Continue through the steps; choose the ledger's keepsake; credits.
fn act(
    mut commands: Commands,
    mut actions: MessageReader<Action>,
    data: Res<GameData>,
    mut progress: ResMut<Progress>,
    mut ending: ResMut<Ending>,
    mut credits: ResMut<CreditsOpen>,
    mut location: ResMut<Location>,
    phase: Res<State<Phase>>,
) {
    for action in actions.read() {
        match action {
            Action::EndingContinue => match ending.0 {
                Some(Step::Quay) => {
                    *location = Location::Market;
                    ending.0 = Some(Step::Market);
                }
                Some(Step::Market) => {
                    *location = Location::Shop;
                    ending.0 = Some(Step::Ledger);
                }
                _ => {}
            },
            Action::OpenLedgerChoice => ending.0 = Some(Step::Ledger),
            Action::ChooseLedgerKeepsake(i) => {
                let Some(d) = data.decor.get(*i).filter(|d| d.keepsake) else {
                    continue;
                };
                progress.journal.ledger_keepsake = Some(d.id.clone());
                // Saved before the credits: closing the game now keeps it.
                if *phase.get() == Phase::Morning {
                    commands.run_system_cached(crate::save::autosave_dawn);
                }
                if ending.0 == Some(Step::Ledger) && *phase.get() == Phase::Morning {
                    ending.0 = Some(Step::Credits);
                    credits.0 = true;
                } else {
                    ending.0 = None;
                }
            }
            Action::OpenCredits => credits.0 = true,
            Action::CloseCredits => {
                credits.0 = false;
                if ending.0 == Some(Step::Credits) {
                    ending.0 = None;
                }
            }
            _ => {}
        }
    }
}

/// Builds whatever the sequence shows now: a caption over the quay or the
/// market, the ledger page, or the credits.
#[allow(clippy::too_many_arguments)]
fn show(
    mut commands: Commands,
    ending: Res<Ending>,
    credits: Res<CreditsOpen>,
    data: Res<GameData>,
    progress: Res<Progress>,
    lang: Res<Lang>,
    font: Res<UiFont>,
    theme: Res<UiTheme>,
    shop: (Res<ShopLayout>, Res<Storeroom>, Res<AssetServer>),
    old: Query<Entity, With<EndingRoot>>,
) {
    if !(ending.is_changed() || credits.is_changed() || lang.is_changed() || font.is_changed()) {
        return;
    }
    for e in &old {
        commands.entity(e).despawn();
    }
    if credits.0 {
        credits_page(&mut commands, &theme, &font, &lang, &data);
        return;
    }
    match ending.0 {
        Some(step @ (Step::Quay | Step::Market)) => {
            let key = if step == Step::Quay {
                "ending.quay"
            } else {
                "ending.market"
            };
            caption(&mut commands, &theme, &font, &lang, key);
        }
        Some(Step::Ledger) => {
            let (layout, storeroom, assets) = &shop;
            ledger_page(
                &mut commands,
                &theme,
                &font,
                &lang,
                &data,
                &progress,
                owned_keepsakes(&data, layout, storeroom),
                assets,
            );
        }
        Some(Step::Credits) | None => {}
    }
}

fn caption(commands: &mut Commands, theme: &UiTheme, font: &UiFont, lang: &Lang, key: &str) {
    let layer = commands
        .spawn((
            EndingRoot,
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
                width: px(CAPTION_WIDTH),
                max_width: percent(96),
                flex_direction: FlexDirection::Column,
                border_radius: BorderRadius::all(px(6)),
                ..default()
            },
            BackgroundColor(crate::ui::BG_PANEL),
            ChildOf(layer),
        ))
        .id();
    let body = theme.panel_body(commands, frame, 0.36, 22.0, 8.0);
    let mut ui = PanelBuilder::new(commands, body, font);
    ui.text(&lang.t(key));
    ui.primary(&lang.t("ending.continue"), Action::EndingContinue);
}

#[allow(clippy::too_many_arguments)]
fn ledger_page(
    commands: &mut Commands,
    theme: &UiTheme,
    font: &UiFont,
    lang: &Lang,
    data: &GameData,
    progress: &Progress,
    owned: Vec<usize>,
    assets: &AssetServer,
) {
    let (layer, panel) = crate::ui::overlay(commands, theme, 0.55, CAPTION_WIDTH);
    commands.entity(layer).insert(EndingRoot);
    let mut ui = PanelBuilder::new(commands, panel, font);
    ui.heading(&lang.t("ending.ledger_title"));
    ui.text(&lang.t("ending.ledger_text"));
    let row = ui.row();
    for i in owned {
        let d = &data.decor[i];
        let icon = format!("sprites/decor/{}.png", d.id);
        let icon = asset_exists(&icon).then(|| assets.load(icon));
        let chosen = progress.journal.ledger_keepsake.as_deref() == Some(&d.id);
        ui.icon_button_in(
            row,
            icon,
            &lang.decor(&d.id),
            Action::ChooseLedgerKeepsake(i),
            chosen,
            true,
        );
    }
}

fn credits_page(
    commands: &mut Commands,
    theme: &UiTheme,
    font: &UiFont,
    lang: &Lang,
    data: &GameData,
) {
    let (layer, panel) = crate::ui::overlay(commands, theme, 0.85, CAPTION_WIDTH);
    commands
        .entity(layer)
        .insert((EndingRoot, GlobalZIndex(300)));
    let mut ui = PanelBuilder::new(commands, panel, font);
    ui.heading(&lang.t("credits.title"));
    let root = ui.root();
    ui.menu_option(root, &lang.t("credits.close"), Action::CloseCredits, false);
    for section in &data.credits {
        ui.section(&lang.t(&format!("credits.{}", section.heading)));
        for line in &section.lines {
            ui.text(line);
        }
        if let Some(key) = &section.lines_key {
            ui.dim(&lang.t(key));
        }
    }
    let root = ui.root();
    ui.menu_option(root, &lang.t("credits.close"), Action::CloseCredits, false);
}

/// Leaving the morning (or quitting) ends the sequence; the ending itself is
/// kept in the save.
fn leave(
    mut ending: ResMut<Ending>,
    mut credits: ResMut<CreditsOpen>,
    mut location: ResMut<Location>,
) {
    if ending.0.take().is_some() {
        credits.0 = false;
        *location = Location::Shop;
    }
}

/// No closing sequence is showing (the accounts may appear).
pub fn no_sequence(ending: Res<Ending>) -> bool {
    ending.0.is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ferryman_s_crossing_ends_the_campaign_and_every_text_exists() {
        let data = GameData::load();
        let lang = Lang::load("en");
        let story = campaign_finale(&data).unwrap();
        assert_eq!(data.stories[story].ghost, "kasimir");
        for key in [
            "ending.quay",
            "ending.market",
            "ending.continue",
            "ending.ledger_title",
            "ending.ledger_text",
            "ending.ledger_page",
            "ending.ledger_choose",
            "credits.title",
            "credits.close",
            "credits.open",
        ] {
            assert_ne!(lang.t(key), key, "missing {key}");
        }
        for section in &data.credits {
            let key = format!("credits.{}", section.heading);
            assert_ne!(lang.t(&key), key, "missing {key}");
            if let Some(key) = &section.lines_key {
                assert_ne!(lang.t(key), *key, "missing {key}");
            }
        }
    }

    #[test]
    fn the_sequence_starts_only_after_the_campaign_finale() {
        use bevy::ecs::system::RunSystemOnce;
        let data = GameData::load();
        let story = campaign_finale(&data).unwrap();
        let chapters = data.stories[story].chapters.len();
        let mut world = World::new();
        world.insert_resource(data);
        world.insert_resource(Progress::new(0));
        world.insert_resource(NightReport::default());
        world.init_resource::<Ending>();
        world.init_resource::<Location>();
        world.run_system_once(begin).unwrap();
        assert!(world.resource::<Ending>().0.is_none());
        for _ in 0..chapters {
            world.resource_mut::<Progress>().finish_chapter(story);
        }
        world.resource_mut::<NightReport>().story_done.push(story);
        world.run_system_once(begin).unwrap();
        assert_eq!(world.resource::<Ending>().0, Some(Step::Quay));
        assert_eq!(*world.resource::<Location>(), Location::Harbor);
        assert!(world.resource::<Progress>().journal.campaign_complete);
    }
}
