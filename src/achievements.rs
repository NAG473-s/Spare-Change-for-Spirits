//! Achievements of the full game (`assets/data/achievements.ron`). The demo
//! has none. The game counts what matters (sales, exact change, repairs …)
//! in each profile's save, unlocks an achievement once its goal is reached,
//! keeps the list in the save and – when the game runs through Steam – tells
//! Steam (`steam_api` loaded at launch, see `i18n/steam.rs`).
//!
//! Steamworks needs each achievement under its API name: the id in upper
//! case (`first_sale` → `FIRST_SALE`), see docs/STEAM.md.

use bevy::prelude::*;
use serde::Deserialize;

use crate::data::GameData;
use crate::day_cycle::{NightReport, Phase, Progress};
use crate::economy::Drawer;
use crate::i18n::{AutomaticLanguage, Lang};
use crate::ui::GameLog;
use crate::upgrades::Upgrades;

pub struct AchievementsPlugin;

impl Plugin for AchievementsPlugin {
    fn build(&self, app: &mut App) {
        // The demo has no achievements.
        if cfg!(feature = "demo") {
            return;
        }
        app.add_systems(
            OnEnter(Phase::Morning),
            // Include unlocks from this night in the dawn autosave.
            (tally_night, unlock_achievements)
                .chain()
                .after(crate::loan::repay)
                .before(crate::day_cycle::count_story_tries),
        )
        .add_systems(
            Update,
            (
                unlock_achievements.run_if(
                    resource_changed::<Progress>
                        .or_else(resource_changed::<Drawer>)
                        .or_else(resource_changed::<Upgrades>),
                ),
                sync_steam
                    .after(unlock_achievements)
                    .run_if(resource_changed::<Progress>),
                |steam: Res<AutomaticLanguage>| steam.run_callbacks(),
            ),
        );
    }
}

/// One achievement. Texts: `achievement.<id>.name`, `achievement.<id>.text`.
/// Steam icon: see docs/ART_PROMPTS.md.
#[derive(Deserialize, Clone, Debug)]
pub struct AchievementDef {
    pub id: String,
    pub goal: Goal,
    #[serde(default)]
    pub reveal_after: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub enum Goal {
    /// A counter in the save (`Progress::tally`) reaches this many.
    Count(Stat, u32),
    /// The story of this ghost is finished.
    Story(String),
    /// The reputation reaches this.
    Reputation(i32),
    /// This much money in the drawer at once (button units).
    Drawer(u32),
    /// All of Hedda's old junk is cleared.
    JunkCleared,
}

/// What the game counts over a whole game.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stat {
    Sales,
    ExactChange,
    IntoTheLight,
    Honest,
    Counterfeits,
    Tips,
    Orders,
    Mishaps,
    Repairs,
}

impl Stat {
    pub fn key(self) -> &'static str {
        match self {
            Stat::Sales => "sales",
            Stat::ExactChange => "exact_change",
            Stat::IntoTheLight => "into_the_light",
            Stat::Honest => "honest",
            Stat::Counterfeits => "counterfeits",
            Stat::Tips => "tips",
            Stat::Orders => "orders",
            Stat::Mishaps => "mishaps",
            Stat::Repairs => "repairs",
        }
    }
}

impl AchievementDef {
    fn visible_in_menu(&self, progress: &Progress) -> bool {
        let guest = match &self.goal {
            Goal::Story(ghost) => Some(ghost.as_str()),
            _ => self.reveal_after.as_deref(),
        };
        guest.is_none_or(|id| progress.journal.has_seen(id))
            || progress.achievements.contains(&self.id)
    }

    /// The Steamworks API name.
    pub fn steam_name(&self) -> String {
        self.id.to_ascii_uppercase()
    }

    /// How far the goal is: (now, goal) for counting goals.
    pub fn progress(&self, progress: &Progress) -> Option<(u32, u32)> {
        match &self.goal {
            Goal::Count(stat, n) => Some((progress.tally(*stat).min(*n), *n)),
            _ => None,
        }
    }

    pub fn reached(
        &self,
        data: &GameData,
        progress: &Progress,
        drawer: &Drawer,
        upgrades: &Upgrades,
    ) -> bool {
        match &self.goal {
            Goal::Count(stat, n) => progress.tally(*stat) >= *n,
            Goal::Story(ghost) => data
                .stories
                .iter()
                .enumerate()
                .any(|(i, s)| &s.ghost == ghost && progress.chapter(i) >= s.chapters.len()),
            Goal::Reputation(r) => progress.reputation >= *r,
            Goal::Drawer(n) => drawer.total() >= *n,
            Goal::JunkCleared => {
                upgrades.area(data) == (data.config.grid_cols, data.config.grid_rows)
            }
        }
    }
}

/// The achievements page of the pause menu: each one with its state in
/// words (not by colour alone) and, for counters, how far it is.
pub fn achievements_page(
    ui: &mut crate::ui::PanelBuilder,
    lang: &Lang,
    data: &GameData,
    progress: &Progress,
    asset_server: &AssetServer,
) {
    let done = progress.achievements.len();
    ui.heading(&lang.f(
        "achievements.title",
        &[("done", &done), ("total", &data.achievements.len())],
    ));
    // Back at the top as well: the list is long and scrolls.
    let root = ui.root();
    ui.menu_option(
        root,
        &lang.t("menu.back"),
        crate::ui::Action::CloseSaveMenu,
        false,
    );
    for a in data
        .achievements
        .iter()
        .filter(|a| a.visible_in_menu(progress))
    {
        let unlocked = progress.achievements.contains(&a.id);
        let state = if unlocked {
            lang.t("achievements.unlocked")
        } else if let Some((now, goal)) = a.progress(progress) {
            lang.f("achievements.progress", &[("now", &now), ("goal", &goal)])
        } else {
            lang.t("achievements.locked")
        };
        // The icon (grey while locked) beside the name, the state in words
        // and what to do.
        let root = ui.root();
        let row = ui.node(
            root,
            Node {
                column_gap: px(10),
                align_items: AlignItems::Center,
                margin: UiRect::top(px(6)),
                ..default()
            },
        );
        let icon = icon_path(&a.id, unlocked);
        if crate::data::asset_exists(&icon) {
            ui.image_sized(row, asset_server.load(icon), Vec2::splat(ICON_SIZE));
        }
        let words = ui.node(
            row,
            Node {
                flex_direction: FlexDirection::Column,
                flex_shrink: 1.0,
                row_gap: px(2),
                ..default()
            },
        );
        ui.add_text(
            words,
            &format!(
                "{} – {state}",
                lang.t(&format!("achievement.{}.name", a.id))
            ),
            true,
            15.0,
            crate::ui::TEXT,
        );
        ui.add_text(
            words,
            &lang.t(&format!("achievement.{}.text", a.id)),
            false,
            13.0,
            crate::ui::TEXT_DIM,
        );
    }
    let root = ui.root();
    ui.menu_option(
        root,
        &lang.t("menu.back"),
        crate::ui::Action::CloseSaveMenu,
        false,
    );
}

/// Size of an achievement's icon on the page (interface pixels).
const ICON_SIZE: f32 = 48.0;

/// The in-game icon: `assets/sprites/achievements/<id>.png`, or
/// `<id>_locked.png` while locked (made from Steam's artwork by
/// `tools/achievement_icons.py`).
pub fn icon_path(id: &str, unlocked: bool) -> String {
    if unlocked {
        format!("sprites/achievements/{id}.png")
    } else {
        format!("sprites/achievements/{id}_locked.png")
    }
}

/// The achievements of all profiles together, for the main menu: unlocked
/// in any profile, and the best count of each.
pub fn across_profiles() -> Progress {
    let mut all = Progress::new(0);
    for p in 1..=crate::save::PROFILES {
        let Some((_, save)) = crate::save::latest_save(p) else {
            continue;
        };
        for id in save.achievements {
            if !all.achievements.contains(&id) {
                all.achievements.push(id);
            }
        }
        for (k, v) in save.tally {
            let best = all.tally.entry(k).or_default();
            *best = (*best).max(v);
        }
    }
    all
}

/// At dawn, what tonight brought is added to the counters.
fn tally_night(mut progress: ResMut<Progress>, report: Res<NightReport>) {
    progress.count(Stat::Sales, report.sales);
    progress.count(Stat::ExactChange, report.change_correct);
    progress.count(Stat::IntoTheLight, report.redeemed.len() as u32);
    progress.count(Stat::Honest, report.honest);
    progress.count(Stat::Counterfeits, report.counterfeits_caught);
    progress.count(Stat::Tips, report.tips_given);
    progress.count(Stat::Orders, report.orders_fulfilled);
    progress.count(Stat::Mishaps, report.mishaps_fixed);
}

/// Record newly reached goals in the profile and the log.
fn unlock_achievements(
    data: Res<GameData>,
    mut progress: ResMut<Progress>,
    drawer: Res<Drawer>,
    upgrades: Res<Upgrades>,
    lang: Res<Lang>,
    mut log: ResMut<GameLog>,
) {
    for a in &data.achievements {
        if progress.achievements.contains(&a.id) || !a.reached(&data, &progress, &drawer, &upgrades)
        {
            continue;
        }
        progress.achievements.push(a.id.clone());
        log.push(lang.f(
            "log.achievement",
            &[("name", &lang.t(&format!("achievement.{}.name", a.id)))],
        ));
    }
}

/// Report each unlock once per session, including ones earned playing offline.
fn sync_steam(
    data: Res<GameData>,
    progress: Res<Progress>,
    steam: Res<AutomaticLanguage>,
    mut sent: Local<Vec<String>>,
) {
    let mut store = false;
    for a in &data.achievements {
        if progress.achievements.contains(&a.id) && !sent.contains(&a.id) {
            sent.push(a.id.clone());
            steam.unlock_achievement(&a.steam_name());
            store = true;
        }
    }
    if store {
        steam.store_stats();
    }
}

#[cfg(all(test, not(feature = "demo")))]
mod tests {
    use super::*;

    #[test]
    fn a_story_achievement_only_names_a_seen_guest_or_an_earned_achievement() {
        let data = GameData::load();
        let achievement = data
            .achievements
            .iter()
            .find(|a| a.id == "lighthouse")
            .unwrap();
        let honest = data.achievements.iter().find(|a| a.id == "honest").unwrap();
        let mut profile = Progress::new(0);
        assert!(!achievement.visible_in_menu(&profile));
        assert!(!honest.visible_in_menu(&profile));
        profile.journal.met("ida", "");
        assert!(achievement.visible_in_menu(&profile));
        assert!(!honest.visible_in_menu(&profile));
        profile.journal.met("forgetful", "");
        assert!(honest.visible_in_menu(&profile));
        let mut legacy = Progress::new(0);
        legacy.achievements.push("lighthouse".into());
        assert!(achievement.visible_in_menu(&legacy));
    }

    #[test]
    fn first_sale_is_unlocked_before_leaving_the_dawn_schedule() {
        let mut app = App::new();
        app.add_plugins(AchievementsPlugin)
            .insert_resource(GameData::load())
            .insert_resource(Lang::load("en"))
            .insert_resource(Progress::new(0))
            .insert_resource(NightReport {
                sales: 1,
                ..default()
            })
            .init_resource::<GameLog>()
            .init_resource::<Drawer>()
            .init_resource::<Upgrades>();
        app.world_mut().run_schedule(OnEnter(Phase::Morning));
        assert!(
            app.world()
                .resource::<Progress>()
                .achievements
                .iter()
                .any(|id| id == "first_sale")
        );
    }

    #[test]
    fn goals_are_reached_by_counts_and_stories() {
        let data = GameData::load();
        let mut progress = Progress::new(0);
        let drawer = Drawer::default();
        let upgrades = Upgrades::default();
        let first = data
            .achievements
            .iter()
            .find(|a| a.id == "first_sale")
            .unwrap();
        assert!(!first.reached(&data, &progress, &drawer, &upgrades));
        progress.count(Stat::Sales, 1);
        assert!(first.reached(&data, &progress, &drawer, &upgrades));
        assert_eq!(first.steam_name(), "FIRST_SALE");
        let edwin = data.achievements.iter().find(|a| a.id == "edwin").unwrap();
        assert!(!edwin.reached(&data, &progress, &drawer, &upgrades));
        let i = data
            .stories
            .iter()
            .position(|s| s.ghost == "edwin")
            .unwrap();
        for _ in 0..data.stories[i].chapters.len() {
            progress.finish_chapter(i);
        }
        assert!(edwin.reached(&data, &progress, &drawer, &upgrades));
    }
}
