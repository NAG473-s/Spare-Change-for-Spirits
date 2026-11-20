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
            tally_night.before(crate::day_cycle::count_story_tries),
        )
        .add_systems(
            Update,
            (
                unlock_achievements.run_if(
                    resource_changed::<Progress>
                        .or_else(resource_changed::<Drawer>)
                        .or_else(resource_changed::<Upgrades>),
                ),
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
) {
    let done = progress.achievements.len();
    ui.heading(&lang.f(
        "achievements.title",
        &[("done", &done), ("total", &data.achievements.len())],
    ));
    for a in &data.achievements {
        let unlocked = progress.achievements.contains(&a.id);
        let state = if unlocked {
            lang.t("achievements.unlocked")
        } else if let Some((now, goal)) = a.progress(progress) {
            lang.f("achievements.progress", &[("now", &now), ("goal", &goal)])
        } else {
            lang.t("achievements.locked")
        };
        ui.section(&format!(
            "{} – {state}",
            lang.t(&format!("achievement.{}.name", a.id))
        ));
        ui.dim(&lang.t(&format!("achievement.{}.text", a.id)));
    }
    let root = ui.root();
    ui.menu_option(
        root,
        &lang.t("menu.back"),
        crate::ui::Action::CloseSaveMenu,
        false,
    );
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

/// Any goal reached: unlock it, say so, and tell Steam.
fn unlock_achievements(
    data: Res<GameData>,
    mut progress: ResMut<Progress>,
    drawer: Res<Drawer>,
    upgrades: Res<Upgrades>,
    lang: Res<Lang>,
    steam: Res<AutomaticLanguage>,
    mut log: ResMut<GameLog>,
    mut sent: Local<Vec<String>>,
) {
    // A save may hold achievements unlocked while Steam was not running:
    // tell Steam about each one once per session.
    let mut store = false;
    for id in &progress.achievements {
        if !sent.contains(id) {
            sent.push(id.clone());
            steam.unlock_achievement(&id.to_ascii_uppercase());
            store = true;
        }
    }
    if store {
        steam.store_stats();
    }
    let new: Vec<&AchievementDef> = data
        .achievements
        .iter()
        .filter(|a| !progress.achievements.contains(&a.id))
        .filter(|a| a.reached(&data, &progress, &drawer, &upgrades))
        .collect();
    if new.is_empty() {
        return;
    }
    for a in new {
        progress.achievements.push(a.id.clone());
        sent.push(a.id.clone());
        log.push(lang.f(
            "log.achievement",
            &[("name", &lang.t(&format!("achievement.{}.name", a.id)))],
        ));
        steam.unlock_achievement(&a.steam_name());
    }
    steam.store_stats();
}

#[cfg(test)]
mod tests {
    use super::*;

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
