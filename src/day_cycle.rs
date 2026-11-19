//! Daily cycle: evening (preparation) → night (open) → morning (accounts).

use bevy::prelude::*;

use crate::data::GameData;
use crate::economy::{Coin, Drawer, Purse};
use crate::ghosts::{Ghost, GhostState, Queue};
use crate::i18n::Lang;
use crate::inventory::Stock;
use crate::register::Register;
use crate::ui::{Action, GameLog};

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Phase {
    #[default]
    Menu,
    /// The village market, before the evening (from the second day on).
    Day,
    Evening,
    Night,
    Morning,
    DemoEnd,
}

impl Phase {
    /// Catalog key of the phase name.
    pub fn key(self) -> &'static str {
        match self {
            Phase::Menu => "phase.menu",
            Phase::Day => "phase.day",
            Phase::Evening => "phase.evening",
            Phase::Night => "phase.night",
            Phase::Morning => "phase.morning",
            Phase::DemoEnd => "phase.demo_end",
        }
    }
}

pub struct DayCyclePlugin;

impl Plugin for DayCyclePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<Phase>()
            .init_resource::<NightClock>()
            .init_resource::<NightReport>()
            .init_resource::<NightBonus>()
            .add_systems(OnEnter(Phase::Night), start_night)
            .add_systems(OnEnter(Phase::Morning), start_morning)
            .add_systems(
                Update,
                (
                    (tick_clock, close_at_dawn)
                        .chain()
                        .run_if(in_state(Phase::Night)),
                    next_evening.run_if(in_state(Phase::Morning)),
                    end_day.run_if(in_state(Phase::Day)),
                ),
            );
    }
}

/// Running game state across all nights.
#[derive(Resource)]
pub struct Progress {
    pub night: u32,
    pub reputation: i32,
    /// Index of the next story chapter; equal to the chapter count when finished.
    pub story_chapter: usize,
}

#[derive(Resource, Default)]
pub struct NightClock {
    pub elapsed: f32,
    pub length: f32,
}

impl NightClock {
    pub fn is_over(&self) -> bool {
        self.elapsed >= self.length
    }

    /// In-game clock: 00:00 to 05:00.
    pub fn time_string(&self) -> String {
        let minutes = (self.elapsed / self.length.max(1.0) * 300.0).min(300.0) as u32;
        format!("{:02}:{:02}", minutes / 60, minutes % 60)
    }
}

/// Statistics of one night, shown in the morning.
#[derive(Resource, Default)]
pub struct NightReport {
    pub sales: u32,
    pub revenue: u32,
    pub change_correct: u32,
    pub change_wrong: u32,
    pub unhappy: u32,
    pub fled: u32,
    pub redeemed: Vec<String>,
    pub faded: Vec<(String, u32)>,
    pub reputation_start: i32,
    /// Ids of ghosts that passed into the light tonight are in `redeemed`.
    /// Items the poltergeist knocked off the shelves (product ids).
    pub knocked: Vec<String>,
    /// Counterfeit coins spotted at the register.
    pub counterfeits_caught: u32,
    /// Counterfeit coins accepted without noticing.
    pub counterfeits_taken: Vec<Coin>,
    /// Counterfeit coins found in the drawer in the morning (removed from it).
    pub counterfeits_found: Purse,
    /// Double payments refused / kept.
    pub honest: u32,
    pub dishonest: u32,
    /// The story ghost has come tonight.
    pub story_arrived: bool,
    /// Catalog key of what the story ghost said tonight when served.
    pub story_line: Option<String>,
    /// Keepsakes received tonight (decor ids).
    pub keepsakes: Vec<String>,
    /// Ghosts that came in (or tried to) tonight.
    pub visitors: u32,
    /// Product id of every unit sold tonight.
    pub sold: Vec<String>,
    /// Change due minus change given, summed over all sales: positive when
    /// too little change was handed out (the drawer keeps it).
    pub change_kept: i64,
    /// Value of second payments that were kept.
    pub second_payments_kept: u32,
}

/// Makes the coming night longer (the Reaper's hourglass): a factor on its
/// length, used up when the night begins.
#[derive(Resource)]
pub struct NightBonus(pub f32);

impl Default for NightBonus {
    fn default() -> Self {
        Self(1.0)
    }
}

fn start_night(
    data: Res<GameData>,
    progress: Res<Progress>,
    mut bonus: ResMut<NightBonus>,
    mut clock: ResMut<NightClock>,
    mut report: ResMut<NightReport>,
    mut log: ResMut<GameLog>,
    lang: Res<Lang>,
) {
    *clock = NightClock {
        elapsed: 0.0,
        length: data.config.night_length_secs * bonus.0,
    };
    bonus.0 = 1.0;
    *report = NightReport {
        reputation_start: progress.reputation,
        ..default()
    };
    log.push(lang.t("log.midnight"));
}

fn tick_clock(time: Res<Time>, mut clock: ResMut<NightClock>) {
    clock.elapsed += time.delta_secs();
}

/// At dawn all ghosts leave. Once the shop is empty, morning begins.
fn close_at_dawn(
    clock: Res<NightClock>,
    register: Res<Register>,
    mut queue: ResMut<Queue>,
    mut ghosts: Query<(Entity, &mut Ghost)>,
    mut next: ResMut<NextState<Phase>>,
) {
    if !clock.is_over() {
        return;
    }
    for (entity, mut ghost) in &mut ghosts {
        let busy = register.is_busy_with(entity);
        if !busy
            && matches!(
                ghost.state,
                GhostState::Entering
                    | GhostState::Browsing
                    | GhostState::ToCounter
                    | GhostState::Waiting
            )
        {
            ghost.state = GhostState::Leaving;
            queue.remove(entity);
        }
    }
    if ghosts.is_empty() {
        next.set(Phase::Morning);
    }
}

pub fn start_morning(
    data: Res<GameData>,
    mut stock: ResMut<Stock>,
    mut drawer: ResMut<Drawer>,
    mut report: ResMut<NightReport>,
) {
    report.faded = stock.age_and_fade(&data);
    report.counterfeits_found = remove_counterfeits(&mut drawer.0, &report.counterfeits_taken);
}

/// Counterfeit coins that are still in the drawer turn to dust at dawn.
/// Those already handed out as change are gone anyway.
pub fn remove_counterfeits(drawer: &mut Purse, taken: &[Coin]) -> Purse {
    let mut found = Purse::default();
    for coin in taken {
        let mut one = Purse::default();
        one.add(*coin, 1);
        if drawer.remove(&one) {
            found.add(*coin, 1);
        }
    }
    found
}

fn next_evening(
    mut actions: MessageReader<Action>,
    data: Res<GameData>,
    mut progress: ResMut<Progress>,
    mut next: ResMut<NextState<Phase>>,
) {
    if !actions.read().any(|a| *a == Action::NextEvening) {
        return;
    }
    if progress.night >= data.config.final_night() {
        next.set(Phase::DemoEnd);
    } else {
        progress.night += 1;
        next.set(Phase::Day);
    }
}

/// Leaving the market: back to the shop for the evening.
fn end_day(mut actions: MessageReader<Action>, mut next: ResMut<NextState<Phase>>) {
    if actions.read().any(|a| *a == Action::EndDay) {
        next.set(Phase::Evening);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counterfeits_are_removed_only_if_still_in_drawer() {
        let mut drawer = Purse {
            buttons: 3,
            candles: 0,
            spoons: 1,
        };
        let found = remove_counterfeits(&mut drawer, &[Coin::Spoon, Coin::Candle]);
        assert_eq!(
            found,
            Purse {
                buttons: 0,
                candles: 0,
                spoons: 1
            }
        );
        assert_eq!(
            drawer,
            Purse {
                buttons: 3,
                candles: 0,
                spoons: 0
            }
        );
    }
}
