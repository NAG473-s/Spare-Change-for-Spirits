//! Daily cycle: evening (preparation) → night (open) → morning (accounts).

use bevy::prelude::*;

use crate::data::GameData;
use crate::ghosts::{Ghost, GhostState, Queue};
use crate::inventory::Stock;
use crate::register::Register;
use crate::ui::{Action, GameLog};

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Phase {
    #[default]
    Evening,
    Night,
    Morning,
    DemoEnd,
}

impl Phase {
    pub fn label(self) -> &'static str {
        match self {
            Phase::Evening => "Evening",
            Phase::Night => "Open",
            Phase::Morning => "Dawn",
            Phase::DemoEnd => "End of demo",
        }
    }
}

pub struct DayCyclePlugin;

impl Plugin for DayCyclePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<Phase>()
            .init_resource::<NightClock>()
            .init_resource::<NightReport>()
            .add_systems(OnEnter(Phase::Night), start_night)
            .add_systems(OnEnter(Phase::Morning), start_morning)
            .add_systems(
                Update,
                (
                    open_shop.run_if(in_state(Phase::Evening)),
                    (tick_clock, close_at_dawn).chain().run_if(in_state(Phase::Night)),
                    next_evening.run_if(in_state(Phase::Morning)),
                ),
            );
    }
}

/// Running game state across all nights.
#[derive(Resource)]
pub struct Progress {
    pub night: u32,
    pub reputation: i32,
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
}

fn open_shop(mut actions: MessageReader<Action>, mut next: ResMut<NextState<Phase>>) {
    if actions.read().any(|a| *a == Action::StartNight) {
        next.set(Phase::Night);
    }
}

fn start_night(
    data: Res<GameData>,
    progress: Res<Progress>,
    mut clock: ResMut<NightClock>,
    mut report: ResMut<NightReport>,
    mut log: ResMut<GameLog>,
) {
    *clock = NightClock {
        elapsed: 0.0,
        length: data.config.night_length_secs,
    };
    *report = NightReport {
        reputation_start: progress.reputation,
        ..default()
    };
    log.push("Midnight. The shop bell rings, though no one touched the door.");
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
        let busy = register.sale.as_ref().is_some_and(|s| s.ghost == entity);
        if !busy && matches!(ghost.state, GhostState::Entering | GhostState::Browsing | GhostState::ToCounter | GhostState::Waiting) {
            ghost.state = GhostState::Leaving;
            queue.remove(entity);
        }
    }
    if ghosts.is_empty() {
        next.set(Phase::Morning);
    }
}

fn start_morning(
    data: Res<GameData>,
    mut stock: ResMut<Stock>,
    mut report: ResMut<NightReport>,
) {
    report.faded = stock.age_and_fade(&data);
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
    if progress.night >= data.config.nights_in_demo {
        next.set(Phase::DemoEnd);
    } else {
        progress.night += 1;
        next.set(Phase::Evening);
    }
}
