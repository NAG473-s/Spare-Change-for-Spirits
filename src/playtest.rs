//! Playtest logs, so real games can be compared with the balancing
//! simulation (`--simulate`). Written next to the saves, in `logs/`:
//!
//! - `nights.csv`: one row per night played, with the simulation's figures
//!   (money, reputation, visitors, sales, declined wishes, …), how long each
//!   ghost spent at the counter, and how long the market, the evening and
//!   the night took (paused time does not count).
//! - `session-<unix time>.txt`: every game message of one session, with the
//!   night, the phase and the clock. The newest [`KEEP_SESSIONS`] are kept.
//!
//! Nothing leaves the computer; `tools/playtest_report.py` sums the CSV up.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use bevy::prelude::*;

use crate::accounts::{Accounts, close_books};
use crate::day_cycle::{NightClock, NightReport, Phase, Progress};
use crate::ghosts::{Ghost, GhostState, Queue};
use crate::inventory::CratesOrdered;
use crate::loan::Loan;
use crate::save::{ActiveProfile, Settings, save_dir};
use crate::ui::GameLog;

/// Session logs kept; older ones are deleted at start.
const KEEP_SESSIONS: usize = 20;

pub struct PlaytestPlugin;

impl Plugin for PlaytestPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Playtest::start())
            .add_systems(OnEnter(Phase::Night), note_crates)
            .add_systems(
                OnEnter(Phase::Morning),
                (end_counter_turn, write_night).chain().after(close_books),
            )
            .add_systems(
                Update,
                (
                    time_phases,
                    track_counter.run_if(in_state(Phase::Night)),
                    mirror_log,
                ),
            );
    }
}

#[derive(Resource, Default)]
pub struct Playtest {
    dir: Option<PathBuf>,
    session: u64,
    /// Seconds spent in each phase since the last night was written.
    market_secs: f32,
    evening_secs: f32,
    night_secs: f32,
    /// Crates ordered for the night (the count is reset at dawn).
    crates: u32,
    /// The ghost at the front of the counter, and since when.
    counter: Option<(Entity, f32)>,
    /// Seconds each ghost spent at the front of the counter tonight.
    counter_secs: Vec<f32>,
    last_phase: Option<Phase>,
}

impl Playtest {
    fn start() -> Self {
        let session = unix_time();
        let dir = save_dir().map(|d| d.join("logs"));
        if let Some(dir) = &dir {
            if let Err(e) = std::fs::create_dir_all(dir) {
                warn!("cannot create {}: {e}", dir.display());
            }
            prune_sessions(dir, KEEP_SESSIONS);
            let build = if cfg!(feature = "demo") {
                "demo"
            } else {
                "full game"
            };
            append(
                &dir.join(format!("session-{session}.txt")),
                &format!(
                    "Spare Change for Spirits {} ({build}), session {session}\n",
                    env!("CARGO_PKG_VERSION")
                ),
            );
        }
        Self {
            dir,
            session,
            ..default()
        }
    }

    fn session_file(&self) -> Option<PathBuf> {
        self.dir
            .as_ref()
            .map(|d| d.join(format!("session-{}.txt", self.session)))
    }
}

fn unix_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn append(path: &Path, text: &str) {
    let result = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut f| f.write_all(text.as_bytes()));
    if let Err(e) = result {
        warn!("cannot write {}: {e}", path.display());
    }
}

/// Deletes the oldest session logs so that `keep - 1` remain (room for the
/// new one).
fn prune_sessions(dir: &Path, keep: usize) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut sessions: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("session-") && n.ends_with(".txt"))
        })
        .collect();
    sessions.sort();
    let excess = (sessions.len() + 1).saturating_sub(keep);
    for old in sessions.into_iter().take(excess) {
        let _ = std::fs::remove_file(old);
    }
}

/// Real time spent per phase (paused time does not count), and a heading
/// in the session log when the phase changes.
fn time_phases(
    time: Res<Time>,
    phase: Res<State<Phase>>,
    progress: Res<Progress>,
    mut log: ResMut<Playtest>,
) {
    let dt = time.delta_secs();
    match phase.get() {
        Phase::Day => log.market_secs += dt,
        Phase::Evening => log.evening_secs += dt,
        Phase::Night => log.night_secs += dt,
        _ => {}
    }
    if log.last_phase != Some(*phase.get()) {
        log.last_phase = Some(*phase.get());
        if let Some(path) = log.session_file() {
            append(
                &path,
                &format!("\n== {:?}, night {} ==\n", phase.get(), progress.night),
            );
        }
    }
}

fn note_crates(crates: Res<CratesOrdered>, mut log: ResMut<Playtest>) {
    log.crates = crates.0;
    log.counter_secs.clear();
    log.counter = None;
}

/// How long each ghost spends at the front of the counter: from the moment
/// it waits there until it is served, turned away or gives up.
fn track_counter(
    time: Res<Time>,
    queue: Res<Queue>,
    ghosts: Query<&Ghost>,
    mut log: ResMut<Playtest>,
) {
    let now = time.elapsed_secs();
    let front = queue.front().filter(|e| {
        ghosts
            .get(*e)
            .is_ok_and(|g| matches!(g.state, GhostState::Waiting | GhostState::AtRegister))
    });
    let current = log.counter.map(|(e, _)| e);
    if front == current {
        return;
    }
    if let Some((_, since)) = log.counter {
        log.counter_secs.push(now - since);
    }
    log.counter = front.map(|e| (e, now));
}

fn end_counter_turn(time: Res<Time>, mut log: ResMut<Playtest>) {
    if let Some((_, since)) = log.counter.take() {
        let now = time.elapsed_secs();
        log.counter_secs.push(now - since);
    }
}

const CSV_HEADER: &str = "unix_time,session,version,build,profile,night,money,opening,\
ferryman,furniture,market,takings,borrowed,repaid,owed,reputation,reputation_start,visitors,\
sales,revenue,declined,wrong_item,fled,unhappy,faded,knocked,crates,change_correct,change_wrong,\
counterfeits_caught,counterfeits_taken,honest,dishonest,counter_turns,counter_avg_secs,\
counter_max_secs,market_secs,evening_secs,night_secs,patient_ghosts,change_hint\n";

/// Dawn: one row in `nights.csv`.
#[allow(clippy::too_many_arguments)]
fn write_night(
    accounts: Res<Accounts>,
    report: Res<NightReport>,
    progress: Res<Progress>,
    loan: Res<Loan>,
    profile: Res<ActiveProfile>,
    settings: Res<Settings>,
    clock: Res<NightClock>,
    mut log: ResMut<Playtest>,
) {
    let Some(dir) = log.dir.clone() else {
        return;
    };
    let turns = &log.counter_secs;
    let avg = if turns.is_empty() {
        0.0
    } else {
        turns.iter().sum::<f32>() / turns.len() as f32
    };
    let max = turns.iter().copied().fold(0.0, f32::max);
    let faded: u32 = report.faded.iter().map(|(_, n)| n).sum();
    let build = if cfg!(feature = "demo") {
        "demo"
    } else {
        "full"
    };
    let row = [
        unix_time().to_string(),
        log.session.to_string(),
        env!("CARGO_PKG_VERSION").to_string(),
        build.to_string(),
        profile.0.to_string(),
        progress.night.to_string(),
        accounts.closing.to_string(),
        accounts.opening.to_string(),
        accounts.ferryman.to_string(),
        accounts.furniture.to_string(),
        accounts.market.to_string(),
        accounts.takings.to_string(),
        accounts.borrowed.to_string(),
        accounts.repaid.to_string(),
        loan.owed.to_string(),
        progress.reputation.to_string(),
        report.reputation_start.to_string(),
        report.visitors.to_string(),
        report.sales.to_string(),
        report.revenue.to_string(),
        report.declined.to_string(),
        report.wrong_item.to_string(),
        report.fled.to_string(),
        report.unhappy.to_string(),
        faded.to_string(),
        report.knocked.len().to_string(),
        log.crates.to_string(),
        report.change_correct.to_string(),
        report.change_wrong.to_string(),
        report.counterfeits_caught.to_string(),
        report.counterfeits_taken.len().to_string(),
        report.honest.to_string(),
        report.dishonest.to_string(),
        turns.len().to_string(),
        format!("{avg:.1}"),
        format!("{max:.1}"),
        format!("{:.0}", log.market_secs),
        format!("{:.0}", log.evening_secs),
        format!("{:.0}", log.night_secs.max(clock.elapsed)),
        settings.patient_ghosts.to_string(),
        settings.change_hint.to_string(),
    ]
    .join(",");
    let path = dir.join("nights.csv");
    if !path.exists() {
        append(&path, CSV_HEADER);
    }
    append(&path, &(row + "\n"));
    log.market_secs = 0.0;
    log.evening_secs = 0.0;
    log.night_secs = 0.0;
}

/// Every game message goes into the session log as well.
fn mirror_log(
    mut game_log: ResMut<GameLog>,
    phase: Res<State<Phase>>,
    progress: Res<Progress>,
    clock: Res<NightClock>,
    log: Res<Playtest>,
) {
    let lines = game_log.bypass_change_detection().take_new();
    if lines.is_empty() {
        return;
    }
    let Some(path) = log.session_file() else {
        return;
    };
    let when = match phase.get() {
        Phase::Night => format!("night {} {}", progress.night, clock.time_string()),
        other => format!("night {} {:?}", progress.night, other),
    };
    let text: String = lines.iter().map(|l| format!("[{when}] {l}\n")).collect();
    append(&path, &text);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_sessions_are_pruned() {
        let dir = std::env::temp_dir().join(format!("scfs-logs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for n in 10..15 {
            std::fs::write(dir.join(format!("session-{n}.txt")), "x").unwrap();
        }
        std::fs::write(dir.join("nights.csv"), "x").unwrap();
        prune_sessions(&dir, 3);
        let mut left: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        left.sort();
        assert_eq!(left, ["nights.csv", "session-13.txt", "session-14.txt"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_csv_header_has_one_name_per_column() {
        let columns = CSV_HEADER.trim().split(',').count();
        assert_eq!(columns, 41);
    }
}
