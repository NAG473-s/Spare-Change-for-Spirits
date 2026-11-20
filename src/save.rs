//! Saving, loading and starting a new game.
//!
//! There are [`PROFILES`] profiles, each a game of its own. A profile has an
//! autosave, written at the start of every evening and night and at dawn,
//! and [`MANUAL_SLOTS`] slots the player saves to from the pause menu at any
//! time. A night restarts at its evening, except for the first guided night:
//! its customer and counted change resume at the counter. The settled sale
//! becomes the restart point for the rest of that night. Settings live separately.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::accounts::Accounts;
use crate::data::GameData;
use crate::day_cycle::{Phase, Progress};
use crate::economy::{Drawer, Purse};
use crate::ferry::BoughtTonight;
use crate::inventory::{CratesOrdered, Stock};
use crate::loan::{Loan, lend};
use crate::market::MarketDay;
use crate::shop::{CellContent, ShopLayout, Storeroom};
use crate::upgrades::Upgrades;

/// Bump when the save format changes incompatibly; older saves are ignored.
pub const SAVE_VERSION: u32 = 7;
/// Environment variable to put saves somewhere else (tests, smoke runs).
pub const SAVE_DIR_ENV: &str = "SCFS_SAVE_DIR";

pub struct SavePlugin;

impl Plugin for SavePlugin {
    fn build(&self, app: &mut App) {
        migrate_old_save();
        app.insert_resource(Settings::load())
            .init_resource::<ActiveProfile>()
            .init_resource::<NightStart>()
            .add_systems(OnEnter(Phase::Evening), autosave_evening.after(lend))
            .add_systems(
                OnEnter(Phase::Night),
                autosave_night
                    .after(crate::day_cycle::deliver_parcels)
                    .after(crate::accounts::open_register),
            )
            .add_systems(
                Update,
                save_completed_first_sale
                    .after(crate::register::make_change)
                    .run_if(in_state(Phase::Night)),
            )
            .add_systems(
                PostUpdate,
                autosave_discoveries
                    .run_if(in_state(Phase::Night))
                    .run_if(resource_changed::<Progress>),
            )
            // Keeping the shop open after the season: continue at that market.
            .add_systems(OnExit(Phase::DemoEnd), autosave_next_season)
            .add_systems(
                OnEnter(Phase::Morning),
                autosave_dawn
                    .after(crate::accounts::close_books)
                    .after(crate::inventory::clear_orders)
                    .after(crate::day_cycle::count_story_tries),
            );
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SaveGame {
    pub version: u32,
    /// The night whose evening the game continues at.
    pub night: u32,
    pub reputation: i32,
    /// Next chapter of each story (see `Progress`).
    #[serde(default)]
    pub story: Vec<usize>,
    #[serde(default)]
    pub story_tries: Vec<u32>,
    /// Stories whose ghost has a parcel from the ferryman waiting.
    #[serde(default)]
    pub parcels: Vec<usize>,
    /// Wares ghosts ordered for a coming night.
    #[serde(default)]
    pub orders: Vec<crate::orders::Order>,
    /// Broken things waiting at the workbench, and when it was last used.
    #[serde(default)]
    pub broken: Vec<String>,
    #[serde(default)]
    pub rummaged_on: u32,
    #[serde(default)]
    pub repaired_on: u32,
    /// Counters over the whole game and the achievements unlocked.
    #[serde(default)]
    pub tally: std::collections::BTreeMap<String, u32>,
    #[serde(default)]
    pub achievements: Vec<String>,
    /// The season the shop is in, and the night it began (see `Progress`).
    #[serde(default = "first")]
    pub season: u32,
    #[serde(default = "first")]
    pub season_start: u32,
    /// The ghost book, the regulars and the shop window.
    #[serde(default)]
    pub journal: crate::book::Journal,
    pub drawer: Purse,
    pub stock: BTreeMap<String, Vec<u32>>,
    pub cells: Vec<CellContent>,
    /// Furniture bought or given but not placed (decor id → count).
    pub storeroom: BTreeMap<String, u32>,
    pub crates_ordered: u32,
    /// Receipts for crates that can still be returned this evening.
    #[serde(default)]
    pub bought_crates: BTreeMap<String, u32>,
    #[serde(default)]
    pub accounts: Accounts,
    #[serde(default)]
    pub market: MarketDay,
    /// Still owed to the ferryman (the bankruptcy rule).
    #[serde(default)]
    pub loan: u32,
    /// Shop improvements bought at the market.
    #[serde(default)]
    pub upgrades: Vec<String>,
    /// Saved at dawn: the game continues at the village market.
    #[serde(default)]
    pub market_open: bool,
    /// The last night of the build that wrote it has been played. A save
    /// from the end of the demo can still continue in the full game.
    pub finished: bool,
}

#[derive(Resource, Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// Show the tips box during the first night.
    pub tips: bool,
    /// Play music and sound effects.
    pub sound: bool,
    /// A manual preference. With `None` the game follows Steam or the desktop.
    pub language: Option<String>,
    /// Music, sound effects and narration have independent levels from 0 to 1.
    pub music: f32,
    pub effects: f32,
    pub narration: f32,
    /// Size of the interface on top of the automatic scaling with the
    /// window height (1 = normal).
    pub ui_size: f32,
    pub fullscreen: bool,
    /// Assist: ghosts wait at the counter as long as it takes.
    pub patient_ghosts: bool,
    /// Assist: the register shows how much change is due.
    pub change_hint: bool,
}

/// Steps and limits of the adjustable settings.
pub const VOLUME_STEP: f32 = 0.1;
pub const UI_SIZE_STEP: f32 = 0.1;
/// Up to twice the normal size: body text then reaches Steam's
/// recommended 38 pixels at 1080p.
pub const UI_SIZE_RANGE: (f32, f32) = (0.7, 2.0);

/// A setting the player turns up or down in the settings menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Knob {
    Music,
    Effects,
    Narration,
    UiSize,
}

impl Settings {
    /// Turns a setting one step up (`up`) or down, within its limits.
    pub fn adjust(&mut self, knob: Knob, up: bool) {
        let sign = if up { 1.0 } else { -1.0 };
        let round = |v: f32| (v * 10.0).round() / 10.0;
        match knob {
            Knob::Music => self.music = round((self.music + sign * VOLUME_STEP).clamp(0.0, 1.0)),
            Knob::Effects => {
                self.effects = round((self.effects + sign * VOLUME_STEP).clamp(0.0, 1.0))
            }
            Knob::Narration => {
                self.narration = round((self.narration + sign * VOLUME_STEP).clamp(0.0, 1.0))
            }
            Knob::UiSize => {
                self.ui_size = round(
                    (self.ui_size + sign * UI_SIZE_STEP).clamp(UI_SIZE_RANGE.0, UI_SIZE_RANGE.1),
                )
            }
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            tips: true,
            sound: true,
            language: None,
            music: 0.8,
            effects: 1.0,
            narration: 1.0,
            ui_size: 1.0,
            fullscreen: false,
            patient_ghosts: false,
            change_hint: false,
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        save_dir()
            .and_then(|d| std::fs::read_to_string(d.join("settings.ron")).ok())
            .and_then(|t| ron::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn store(&self) {
        if let Some(dir) = save_dir() {
            write_ron(&dir.join("settings.ron"), self);
        }
    }
}

/// Where saves go: `SCFS_SAVE_DIR`, else the usual per-user data folder.
pub fn save_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os(SAVE_DIR_ENV) {
        return Some(PathBuf::from(dir));
    }
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("APPDATA").map(|p| PathBuf::from(p).join("SpareChangeForSpirits"));
    #[cfg(target_os = "macos")]
    let base = std::env::var_os("HOME")
        .map(|h| PathBuf::from(h).join("Library/Application Support/SpareChangeForSpirits"));
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .map(|p| p.join("spare-change-for-spirits"));
    base
}

/// Reserves the opening for one process, across all profiles and both editions.
/// Reserve it before playback so closing the window during the crossing also counts.
pub fn claim_first_launch() -> bool {
    save_dir().is_some_and(|dir| {
        claim_first_launch_in(&dir).unwrap_or_else(|error| {
            warn!(
                "Cannot record the first launch in {}: {error}",
                dir.display()
            );
            false
        })
    })
}

fn claim_first_launch_in(dir: &Path) -> std::io::Result<bool> {
    // Check before the current session log is opened. Even an older installation
    // that only reached the language picker has a session log.
    if dir.join("settings.ron").is_file()
        || dir.join("save.ron").is_file()
        || (1..=PROFILES).any(|profile| Slot::all().any(|s| slot_path(dir, profile, s).is_file()))
        || std::fs::read_dir(dir.join("logs")).is_ok_and(|entries| {
            entries.flatten().any(|entry| {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                name.starts_with("session-") && name.ends_with(".txt")
            })
        })
    {
        return Ok(false);
    }
    std::fs::create_dir_all(dir)?;
    match std::fs::File::create_new(dir.join("intro_seen")) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        Err(error) => Err(error),
    }
}

/// Number of profiles (separate games).
pub const PROFILES: u8 = 3;
/// Manual save slots per profile.
pub const MANUAL_SLOTS: u8 = 3;

/// Where in a profile a save lives.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Slot {
    Auto,
    /// 1 to [`MANUAL_SLOTS`].
    Manual(u8),
}

impl Slot {
    /// The autosave first, then the manual slots.
    pub fn all() -> impl Iterator<Item = Slot> {
        std::iter::once(Slot::Auto).chain((1..=MANUAL_SLOTS).map(Slot::Manual))
    }

    fn file(self) -> String {
        match self {
            Slot::Auto => "auto.ron".into(),
            Slot::Manual(n) => format!("slot{n}.ron"),
        }
    }
}

/// The profile (1 to [`PROFILES`]) the running game saves to.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActiveProfile(pub u8);

impl Default for ActiveProfile {
    fn default() -> Self {
        Self(1)
    }
}

fn slot_path(dir: &Path, profile: u8, slot: Slot) -> PathBuf {
    dir.join(format!("profile{profile}")).join(slot.file())
}

fn read_save_at(path: &Path) -> Option<SaveGame> {
    let text = std::fs::read_to_string(path).ok()?;
    match ron::from_str::<SaveGame>(&text) {
        Ok(mut save) if (4..=SAVE_VERSION).contains(&save.version) => {
            // Later versions add only defaulted guide, order and ghost-book fields.
            if save.version < 7 {
                save.journal.recover_recorded_sales();
            }
            save.version = SAVE_VERSION;
            Some(save)
        }
        Ok(save) => {
            warn!(
                "ignoring save of version {} at {}",
                save.version,
                path.display()
            );
            None
        }
        Err(e) => {
            warn!("cannot read save {}: {e}", path.display());
            None
        }
    }
}

/// A readable save in this profile and slot.
pub fn read_slot(profile: u8, slot: Slot) -> Option<SaveGame> {
    read_save_at(&slot_path(&save_dir()?, profile, slot))
}

pub fn write_slot(profile: u8, slot: Slot, save: &SaveGame) {
    if let Some(dir) = save_dir() {
        write_ron(&slot_path(&dir, profile, slot), save);
    }
}

/// Readable saves of one profile, including the time each slot was written.
fn profile_saves(
    dir: &Path,
    profile: u8,
) -> impl Iterator<Item = (Slot, SaveGame, std::time::SystemTime)> + '_ {
    Slot::all().filter_map(move |slot| {
        let path = slot_path(dir, profile, slot);
        let time = std::fs::metadata(&path).and_then(|m| m.modified()).ok()?;
        Some((slot, read_save_at(&path)?, time))
    })
}

fn latest_in(dir: &Path, profile: u8) -> Option<(Slot, SaveGame, std::time::SystemTime)> {
    profile_saves(dir, profile).max_by_key(|(_, _, time)| *time)
}

/// The newest save of one profile.
pub fn latest_save(profile: u8) -> Option<(Slot, SaveGame)> {
    latest_in(&save_dir()?, profile).map(|(slot, save, _)| (slot, save))
}

/// The newest playable save over all profiles and slots for "Continue".
pub fn newest_save(demo_nights: u32) -> Option<(u8, Slot, SaveGame)> {
    newest_in(&save_dir()?, demo_nights)
}

fn newest_in(dir: &Path, demo_nights: u32) -> Option<(u8, Slot, SaveGame)> {
    (1..=PROFILES)
        .flat_map(|p| profile_saves(dir, p).map(move |(slot, save, time)| (p, slot, save, time)))
        .filter(|(_, _, save, _)| save.resume_at(demo_nights).is_some())
        .max_by_key(|(.., time)| *time)
        .map(|(p, slot, save, _)| (p, slot, save))
}

/// Removes every save of a profile (before a new game starts in it).
pub fn clear_profile(profile: u8) {
    if let Some(dir) = save_dir() {
        let _ = std::fs::remove_dir_all(dir.join(format!("profile{profile}")));
    }
}

/// Saves from before there were profiles become the autosave of profile 1.
fn migrate_old_save() {
    if let Some(dir) = save_dir() {
        migrate_in(&dir);
    }
}

fn migrate_in(dir: &Path) {
    let old = dir.join("save.ron");
    let new = slot_path(dir, 1, Slot::Auto);
    if old.exists() && !new.exists() {
        let moved = new
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|_| std::fs::rename(&old, &new));
        if let Err(e) = moved {
            warn!("cannot move {} to a profile: {e}", old.display());
        }
    }
}

/// Writes to a temporary file first, so a crash never leaves half a save.
fn write_ron<T: Serialize>(path: &Path, value: &T) {
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = ron::ser::to_string_pretty(value, ron::ser::PrettyConfig::default())?;
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    })();
    if let Err(e) = result {
        warn!("cannot write {}: {e}", path.display());
    }
}

/// Everything a save contains, as live resources.
#[derive(SystemParam)]
pub struct GameState<'w> {
    pub progress: ResMut<'w, Progress>,
    pub drawer: ResMut<'w, Drawer>,
    pub stock: ResMut<'w, Stock>,
    pub layout: ResMut<'w, ShopLayout>,
    pub storeroom: ResMut<'w, Storeroom>,
    pub crates: ResMut<'w, CratesOrdered>,
    pub bought: ResMut<'w, BoughtTonight>,
    pub accounts: ResMut<'w, Accounts>,
    pub market: ResMut<'w, MarketDay>,
    pub loan: ResMut<'w, Loan>,
    pub upgrades: ResMut<'w, Upgrades>,
}

impl GameState<'_> {
    pub fn snapshot(&self, night: u32, finished: bool, market_open: bool) -> SaveGame {
        SaveGame {
            version: SAVE_VERSION,
            night,
            reputation: self.progress.reputation,
            story: self.progress.story.clone(),
            story_tries: self.progress.story_tries.clone(),
            parcels: self.progress.parcels.clone(),
            orders: self.progress.orders.clone(),
            broken: self.progress.broken.clone(),
            rummaged_on: self.progress.rummaged_on,
            repaired_on: self.progress.repaired_on,
            tally: self.progress.tally.clone(),
            achievements: self.progress.achievements.clone(),
            season: self.progress.season,
            season_start: self.progress.season_start,
            journal: self.progress.journal.clone(),
            drawer: self.drawer.0,
            stock: self.stock.units().clone(),
            cells: self.layout.cells.clone(),
            storeroom: self.storeroom.0.clone(),
            crates_ordered: self.crates.0,
            bought_crates: self.bought.0.clone(),
            accounts: self.accounts.clone(),
            market: self.market.clone(),
            loan: self.loan.owed,
            upgrades: self.upgrades.0.iter().cloned().collect(),
            market_open,
            finished,
        }
    }

    pub fn apply(&mut self, save: &SaveGame, night: u32, data: &GameData) {
        *self.progress = Progress {
            night,
            reputation: save.reputation,
            story: save.story.clone(),
            story_tries: save.story_tries.clone(),
            parcels: save.parcels.clone(),
            orders: save.orders.clone(),
            broken: save.broken.clone(),
            rummaged_on: save.rummaged_on,
            repaired_on: save.repaired_on,
            tally: save.tally.clone(),
            achievements: save.achievements.clone(),
            season: save.season,
            season_start: save.season_start,
            journal: save.journal.clone(),
        };
        // Completed chapters prove which wares the customer received even in
        // older profiles. The ferryman's fare was a coin, not a ware.
        for (story, &done) in data.stories.iter().zip(&save.story) {
            for chapter in story
                .chapters
                .iter()
                .take(done)
                .filter(|chapter| !chapter.fare)
            {
                self.progress
                    .journal
                    .note_fulfilled(&story.ghost, &chapter.wish);
            }
        }
        self.drawer.0 = save.drawer;
        *self.stock = Stock::from_units(save.stock.clone());
        let mut layout = ShopLayout::new(data.config.grid_cols, data.config.grid_rows);
        // A save from a different grid size starts with the default furniture.
        if save.cells.len() == layout.cells.len() {
            layout.cells = save.cells.clone();
        }
        *self.layout = layout;
        self.storeroom.0 = save.storeroom.clone();
        self.crates.0 = save.crates_ordered;
        self.bought.0 = save.bought_crates.clone();
        *self.accounts = save.accounts.clone();
        *self.market = save.market.clone();
        self.loan.owed = save.loan;
        self.upgrades.0 = save.upgrades.iter().cloned().collect();
    }

    pub fn reset(&mut self, data: &GameData) {
        *self.progress = Progress::new(data.config.start_reputation);
        self.drawer.0 = data.config.start_coins;
        *self.stock = Stock::default();
        *self.layout = ShopLayout::new(data.config.grid_cols, data.config.grid_rows);
        self.storeroom.0.clear();
        self.crates.0 = 0;
        self.bought.0.clear();
        *self.accounts = Accounts::default();
        *self.market = MarketDay::default();
        self.loan.owed = 0;
        self.upgrades.0.clear();
    }
}

fn first() -> u32 {
    1
}

impl SaveGame {
    /// Dawn saves store the upcoming night. A full-game season finale resumes at
    /// its epilogue so Keep Playing advances the season and calendar exactly once.
    pub fn resume_at(&self, demo_nights: u32) -> Option<(Phase, u32)> {
        if cfg!(feature = "demo") && self.night > demo_nights {
            return None;
        }
        let finished_demo = self.season == 1 && self.night == demo_nights + 1;
        if self.finished && !finished_demo {
            return Some((Phase::DemoEnd, self.night.saturating_sub(1)));
        }
        if self.night == 1 && self.journal.first_sale_checkpoint.is_some() {
            return Some((Phase::Night, self.night));
        }
        let phase = if self.market_open {
            Phase::Day
        } else {
            Phase::Evening
        };
        Some((phase, self.night))
    }
}

/// Resources from the restart point for this night, including the first settled sale if guided.
#[derive(Resource, Default)]
pub struct NightStart(pub Option<SaveGame>);

/// Evening: continue at this evening.
fn autosave_evening(state: GameState, profile: Res<ActiveProfile>) {
    write_slot(
        profile.0,
        Slot::Auto,
        &state.snapshot(state.progress.night, false, false),
    );
}

/// The start of the night: continue at its evening.
fn autosave_night(
    mut state: GameState,
    profile: Res<ActiveProfile>,
    mut start: ResMut<NightStart>,
) {
    let save = state.snapshot(state.progress.night, false, false);
    write_slot(profile.0, Slot::Auto, &save);
    start.0 = Some(save);
    if state.progress.journal.first_sale == crate::guide::FirstSale::Complete {
        state.progress.journal.first_sale_checkpoint = None;
    }
}

/// The guided sale is the first night's new restart point. Subsequent customers replay normally.
fn save_completed_first_sale(
    state: GameState,
    report: Res<crate::day_cycle::NightReport>,
    profile: Res<ActiveProfile>,
    mut start: ResMut<NightStart>,
) {
    use crate::guide::{FirstSale, FirstSaleCheckpoint};
    if state.progress.night != 1
        || state.progress.journal.first_sale != FirstSale::Complete
        || !start
            .0
            .as_ref()
            .is_some_and(|s| s.journal.first_sale == FirstSale::Following)
    {
        return;
    }
    let mut save = state.snapshot(1, false, false);
    save.journal.first_sale_checkpoint =
        Some(FirstSaleCheckpoint::AfterSale(Box::new(report.clone())));
    write_slot(profile.0, Slot::Auto, &save);
    start.0 = Some(save);
}

/// Newly observed ghosts and wishes survive closing the window between nightly saves.
/// The replay point keeps its original counts, wares, coins and story progress.
fn autosave_discoveries(
    progress: Res<Progress>,
    register: Res<crate::register::Register>,
    profile: Res<ActiveProfile>,
    mut start: ResMut<NightStart>,
) {
    let Some(start) = start.0.as_mut() else {
        return;
    };
    if start.journal.remember_discoveries(&progress.journal) {
        write_slot(
            profile.0,
            Slot::Auto,
            &night_snapshot(start, &progress, &register),
        );
    }
}

fn autosave_next_season(state: GameState, profile: Res<ActiveProfile>) {
    // Only when the next season has just begun (not when quitting to the menu).
    let p = &state.progress;
    if p.season > 1 && p.season_start == p.night {
        write_slot(
            profile.0,
            Slot::Auto,
            &state.snapshot(state.progress.night, false, true),
        );
    }
}

/// Dawn: the night is done; continue with the market of the next day.
fn autosave_dawn(state: GameState, data: Res<GameData>, profile: Res<ActiveProfile>) {
    write_slot(profile.0, Slot::Auto, &dawn_snapshot(&state, &data));
}

fn dawn_snapshot(state: &GameState, data: &GameData) -> SaveGame {
    let night = state.progress.night;
    let finished = state.progress.season_over(data);
    state.snapshot(night + 1, finished, !finished)
}

/// What a save made right now contains, in any phase of the game: the
/// market, the evening, the start of a running night, or the next day at
/// dawn. `None` where there is nothing to save (menu, end of the demo).
pub fn save_now(
    phase: Phase,
    state: &GameState,
    data: &GameData,
    night_start: &NightStart,
    register: &crate::register::Register,
) -> Option<SaveGame> {
    match phase {
        Phase::Day => Some(state.snapshot(state.progress.night, false, true)),
        Phase::Evening => Some(state.snapshot(state.progress.night, false, false)),
        Phase::Night => night_start
            .0
            .as_ref()
            .map(|start| night_snapshot(start, &state.progress, register)),
        Phase::Morning => Some(dawn_snapshot(state, data)),
        Phase::Menu | Phase::DemoEnd => None,
    }
}

fn night_snapshot(
    start: &SaveGame,
    progress: &Progress,
    register: &crate::register::Register,
) -> SaveGame {
    use crate::guide::FirstSaleCheckpoint;
    let mut save = start.clone();
    save.journal.remember_discoveries(&progress.journal);
    // Night saves restore the opening stock and promises. Keep pin choices for slips
    // that existed at that point, even if their guest has already collected the ware.
    for (order, live) in save.orders.iter_mut().zip(&progress.orders) {
        order.pinned = live.pinned;
    }
    save.journal.first_sale = progress.journal.first_sale;
    save.journal.first_sale_checkpoint = if progress.guiding_first_sale() {
        Some(if let Some(sale) = &register.sale {
            FirstSaleCheckpoint::Change {
                coins: sale.coins.clone(),
                given: sale.given,
                recount: sale.recount,
            }
        } else {
            progress
                .journal
                .first_sale_checkpoint
                .clone()
                .unwrap_or(FirstSaleCheckpoint::Wish)
        })
    } else {
        start
            .journal
            .first_sale_checkpoint
            .clone()
            .filter(|c| c.settled_report().is_some())
    };
    save
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_end_of_the_demo_continues_in_the_full_game() {
        // Dawn after night 5 of the demo: continue at the evening of night 6.
        let save = SaveGame {
            version: SAVE_VERSION,
            night: 6,
            reputation: 3,
            story: vec![5],
            story_tries: Vec::new(),
            parcels: Vec::new(),
            orders: Vec::new(),
            broken: Vec::new(),
            rummaged_on: 0,
            repaired_on: 0,
            tally: Default::default(),
            achievements: Vec::new(),
            season: 1,
            season_start: 1,
            journal: Default::default(),
            drawer: Purse::default(),
            stock: BTreeMap::new(),
            cells: Vec::new(),
            storeroom: BTreeMap::new(),
            crates_ordered: 0,
            bought_crates: BTreeMap::new(),
            accounts: Accounts::default(),
            market: MarketDay::default(),
            loan: 0,
            upgrades: Vec::new(),
            market_open: false,
            finished: true,
        };
        let expected = if cfg!(feature = "demo") {
            None
        } else {
            Some((Phase::Evening, 6))
        };
        assert_eq!(save.resume_at(5), expected);
    }

    #[test]
    fn a_full_game_finale_resumes_at_its_epilogue_even_if_it_finished_late() {
        let mut save = sample(34);
        save.season = 3;
        save.finished = true;
        let expected = if cfg!(feature = "demo") {
            None
        } else {
            Some((Phase::DemoEnd, 33))
        };
        assert_eq!(save.resume_at(5), expected);
    }

    #[test]
    fn save_round_trips_through_ron() {
        let mut stock = BTreeMap::new();
        stock.insert("marbles".to_string(), vec![0, 1, 2]);
        let save = SaveGame {
            version: SAVE_VERSION,
            night: 3,
            reputation: -2,
            story: vec![2],
            story_tries: Vec::new(),
            parcels: Vec::new(),
            orders: Vec::new(),
            broken: Vec::new(),
            rummaged_on: 0,
            repaired_on: 0,
            tally: Default::default(),
            achievements: Vec::new(),
            season: 1,
            season_start: 1,
            journal: Default::default(),
            drawer: Purse {
                buttons: 4,
                candles: 1,
                spoons: 2,
            },
            stock,
            cells: vec![
                CellContent {
                    decor: Some("shelf".into()),
                    products: vec!["marbles".into()],
                },
                CellContent::default(),
            ],
            storeroom: BTreeMap::from([("umbrella".to_string(), 1)]),
            crates_ordered: 2,
            bought_crates: BTreeMap::from([("marbles".into(), 2)]),
            accounts: Accounts {
                ferryman: 8,
                ..default()
            },
            market: MarketDay::default(),
            loan: 7,
            upgrades: vec!["clear_corner".into()],
            market_open: true,
            finished: false,
        };
        let text = ron::ser::to_string_pretty(&save, ron::ser::PrettyConfig::default()).unwrap();
        let back: SaveGame = ron::from_str(&text).unwrap();
        assert_eq!(back, save);
    }

    fn sample(night: u32) -> SaveGame {
        SaveGame {
            version: SAVE_VERSION,
            night,
            reputation: 0,
            story: vec![0],
            story_tries: Vec::new(),
            parcels: Vec::new(),
            orders: Vec::new(),
            broken: Vec::new(),
            rummaged_on: 0,
            repaired_on: 0,
            tally: Default::default(),
            achievements: Vec::new(),
            season: 1,
            season_start: 1,
            journal: Default::default(),
            drawer: Purse::default(),
            stock: BTreeMap::new(),
            cells: Vec::new(),
            storeroom: BTreeMap::new(),
            crates_ordered: 0,
            bought_crates: BTreeMap::new(),
            accounts: Accounts::default(),
            market: MarketDay::default(),
            loan: 0,
            upgrades: Vec::new(),
            market_open: false,
            finished: false,
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("scfs-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn the_opening_is_once_per_installation_and_skips_existing_players() {
        let fresh = temp_dir("first-launch");
        assert!(claim_first_launch_in(&fresh).unwrap());
        assert!(!claim_first_launch_in(&fresh).unwrap());
        std::fs::remove_dir_all(fresh).unwrap();

        let old = temp_dir("pre-opening");
        write_ron(&old.join("settings.ron"), &Settings::default());
        assert!(!claim_first_launch_in(&old).unwrap());
        std::fs::remove_file(old.join("settings.ron")).unwrap();
        write_ron(&slot_path(&old, 3, Slot::Manual(2)), &sample(4));
        assert!(!claim_first_launch_in(&old).unwrap());
        std::fs::remove_file(slot_path(&old, 3, Slot::Manual(2))).unwrap();
        std::fs::create_dir(old.join("logs")).unwrap();
        std::fs::write(old.join("logs/session-42.txt"), "").unwrap();
        assert!(!claim_first_launch_in(&old).unwrap());
        std::fs::remove_dir_all(old).unwrap();
    }

    #[test]
    fn a_night_rewind_keeps_knowledge_without_replaying_the_sale_or_visit_counts() {
        let mut start = sample(4);
        start.journal.met("shy", "lavender");
        start.journal.served("shy", "lavender", None);
        start
            .orders
            .push(crate::orders::Order::new("shy", "lavender", 4, None));
        let mut live = Progress::new(0);
        live.night = 4;
        live.journal = start.journal.clone();
        live.orders = start.orders.clone();
        live.orders[0].pinned = true;
        live.journal.met("shade", "");
        live.journal.met("shy", "letter_paper");
        live.journal.served("shy", "letter_paper", None);
        let register = crate::register::Register::default();
        let saved = night_snapshot(&start, &live, &register);
        let saved: SaveGame = ron::from_str(&ron::to_string(&saved).unwrap()).unwrap();
        assert!(saved.orders[0].pinned);
        assert_eq!(saved.stock, start.stock);
        assert_eq!(saved.drawer, start.drawer);
        let shy = &saved.journal.book["shy"];
        assert_eq!((shy.met, shy.served), (1, 1));
        assert_eq!(shy.wishes, ["lavender", "letter_paper"]);
        assert_eq!(shy.fulfilled_wishes, ["lavender", "letter_paper"]);
        assert!(saved.journal.has_seen("shade"));
        assert_eq!(saved.journal.book["shade"].met, 0);
        assert_eq!(night_snapshot(&saved, &live, &register), saved);
        let mut replay = saved.journal;
        replay.met("shy", "letter_paper");
        replay.served("shy", "letter_paper", None);
        assert_eq!(replay.book["shy"], live.journal.book["shy"]);
        assert!(!replay.remember_discoveries(&live.journal));
    }

    #[test]
    fn version_six_does_not_guess_which_of_several_wishes_was_fulfilled() {
        let mut old = sample(4);
        old.version = 6;
        for product in ["lavender", "letter_paper"] {
            old.journal.met("shy", product);
        }
        old.journal.book.get_mut("shy").unwrap().served = 1;
        old.journal.met("child", "marbles");
        old.journal.book.get_mut("child").unwrap().served = 1;
        old.journal.regulars.push(ron::from_str(r#"(ghost:"forgetful",name:1,visits:5,favourite:"rain_jar",last_purchase:Some("rusty_key"),encounters:[(night:3,wish:"rusty_key",memory:Purchase("pocket_watch"))])"#).unwrap());
        let mut order = crate::orders::Order::new("cat", "fish_bone", 3, None);
        order.state = crate::orders::OrderState::Collected;
        old.journal.order_history.push(order);
        let dir = temp_dir("known-sales");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("save.ron");
        std::fs::write(
            &path,
            ron::to_string(&old)
                .unwrap()
                .replace(",fulfilled_wishes:[]", ""),
        )
        .unwrap();
        let loaded = read_save_at(&path).unwrap();
        let book = &loaded.journal.book;
        assert!(loaded.journal.has_seen("shy"));
        assert!(book["shy"].fulfilled_wishes.is_empty());
        assert_eq!(book["child"].fulfilled_wishes, ["marbles"]);
        assert_eq!(
            book["forgetful"].fulfilled_wishes,
            ["rain_jar", "pocket_watch", "rusty_key"]
        );
        assert_eq!(book["cat"].fulfilled_wishes, ["fish_bone"]);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn version_four_profiles_load_without_starting_the_guide() {
        let mut original = sample(1);
        original.version = 4;
        original.stock.insert("marbles".into(), vec![0, 0]);
        let old_text = ron::to_string(&original)
            .unwrap()
            .replace("order_history:[],", "")
            .replace("first_sale:Skipped,", "")
            .replace("first_sale_checkpoint:None,", "");
        let dir = temp_dir("old-guide");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("save.ron");
        std::fs::write(&path, old_text).unwrap();
        let loaded = read_save_at(&path).unwrap();
        assert_eq!(loaded.version, SAVE_VERSION);
        assert_eq!(loaded.journal.first_sale, crate::guide::FirstSale::Skipped);
        assert_eq!(loaded.resume_at(5), Some((Phase::Evening, 1)));
        assert_eq!(loaded.stock, original.stock);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn legacy_orders_survive_loading_and_a_restarted_night_keeps_the_pin_with_its_original_stock() {
        use crate::orders::{Order, OrderState};

        let mut old = sample(4);
        old.version = 5;
        old.orders.push(Order::new("shy", "lavender", 4, None));
        old.stock.insert("lavender".into(), vec![0]);
        let mut text = ron::to_string(&old).unwrap();
        text = text
            .replace("order_history:[],", "")
            .replace(",regular_name:None,state:Open,pinned:false", "");
        assert!(!text.contains("order_history:") && !text.contains("pinned:"));
        let dir = temp_dir("order-slips");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("save.ron");
        std::fs::write(&file, text).unwrap();
        let loaded = read_save_at(&file).unwrap();
        assert_eq!(loaded.version, SAVE_VERSION);
        assert_eq!(loaded.orders[0].state, OrderState::Open);
        assert!(!loaded.orders[0].pinned);
        let mut live = Progress::new(0);
        live.night = 4;
        live.orders = loaded.orders.clone();
        live.orders[0].pinned = true;
        live.orders[0].state = OrderState::Collected;
        live.orders.push(Order::new("child", "marbles", 5, None));
        let saved = night_snapshot(&loaded, &live, &crate::register::Register::default());
        write_ron(&file, &saved);
        let reloaded = read_save_at(&file).unwrap();
        assert_eq!(reloaded.orders.len(), 1);
        assert!(reloaded.orders[0].pinned);
        assert_eq!(reloaded.orders[0].state, OrderState::Open);
        assert_eq!(reloaded.stock, old.stock);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_first_night_restarts_at_the_wish_the_skipped_evening_or_the_settled_sale() {
        use crate::guide::{FirstSale, FirstSaleCheckpoint};
        let mut start = sample(1);
        let mut progress = Progress::new(0);
        let register = crate::register::Register::default();
        progress.journal.first_sale = FirstSale::Following;
        let wish = night_snapshot(&start, &progress, &register);
        assert_eq!(wish.resume_at(5), Some((Phase::Night, 1)));
        assert_eq!(
            wish.journal.first_sale_checkpoint,
            Some(FirstSaleCheckpoint::Wish)
        );

        progress.journal.first_sale = FirstSale::Skipped;
        progress.journal.met("shade", "");
        let skipped = night_snapshot(&wish, &progress, &register);
        assert!(skipped.journal.first_sale_checkpoint.is_none());
        assert!(skipped.journal.has_seen("shade"));
        assert_eq!(skipped.resume_at(5), Some((Phase::Evening, 1)));

        let mut report = crate::day_cycle::NightReport::default();
        report.record_sale("marbles", 3, 4, 1);
        progress.journal.first_sale = FirstSale::Complete;
        start.journal.remember_discoveries(&progress.journal);
        start.journal.first_sale = FirstSale::Complete;
        start.journal.first_sale_checkpoint =
            Some(FirstSaleCheckpoint::AfterSale(Box::new(report)));
        let settled = night_snapshot(&start, &progress, &register);
        let restored: SaveGame = ron::from_str(&ron::to_string(&settled).unwrap()).unwrap();
        assert_eq!(restored.resume_at(5), Some((Phase::Night, 1)));
        assert_eq!(restored, start);
    }

    #[test]
    fn the_newest_save_wins_across_profiles_and_slots() {
        let dir = temp_dir("profiles");
        assert!(newest_in(&dir, 5).is_none());
        write_ron(&slot_path(&dir, 1, Slot::Auto), &sample(2));
        std::thread::sleep(std::time::Duration::from_millis(20));
        write_ron(&slot_path(&dir, 2, Slot::Manual(3)), &sample(4));
        let (profile, slot, save) = newest_in(&dir, 5).unwrap();
        assert_eq!((profile, slot, save.night), (2, Slot::Manual(3), 4));
        if cfg!(feature = "demo") {
            write_ron(&slot_path(&dir, 2, Slot::Auto), &sample(6));
            assert_eq!(newest_in(&dir, 5).unwrap().1, Slot::Manual(3));
        }
        let (slot, save, _) = latest_in(&dir, 1).unwrap();
        assert_eq!((slot, save.night), (Slot::Auto, 2));
        assert!(latest_in(&dir, 3).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_old_save_moves_into_profile_one() {
        let dir = temp_dir("migrate");
        write_ron(&dir.join("save.ron"), &sample(3));
        migrate_in(&dir);
        assert!(!dir.join("save.ron").exists());
        let moved = read_save_at(&slot_path(&dir, 1, Slot::Auto)).unwrap();
        assert_eq!(moved.night, 3);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn slots_are_the_autosave_then_the_manual_ones() {
        let slots: Vec<Slot> = Slot::all().collect();
        assert_eq!(slots.len(), 1 + MANUAL_SLOTS as usize);
        assert_eq!(slots[0], Slot::Auto);
        assert_eq!(
            slot_path(Path::new("x"), 2, Slot::Manual(1)),
            Path::new("x/profile2/slot1.ron")
        );
    }

    #[test]
    fn settings_adjust_in_steps_within_limits() {
        let mut settings = Settings::default();
        for _ in 0..5 {
            settings.adjust(Knob::Music, true);
        }
        assert_eq!(settings.music, 1.0);
        settings.adjust(Knob::Effects, false);
        assert_eq!(settings.effects, 0.9);
        settings.adjust(Knob::Narration, false);
        assert_eq!(settings.narration, 0.9);
        for _ in 0..20 {
            settings.adjust(Knob::UiSize, false);
        }
        assert_eq!(settings.ui_size, UI_SIZE_RANGE.0);
        for _ in 0..20 {
            settings.adjust(Knob::UiSize, true);
        }
        assert_eq!(settings.ui_size, UI_SIZE_RANGE.1);
    }

    #[test]
    fn writes_and_reads_through_the_save_dir() {
        let dir = std::env::temp_dir().join(format!("scfs-save-test-{}", std::process::id()));
        // SAFETY: only this test sets the variable; other tests do not read it.
        unsafe { std::env::set_var(SAVE_DIR_ENV, &dir) };
        assert_eq!(save_dir(), Some(dir.clone()));
        let settings = Settings {
            tips: false,
            sound: false,
            language: Some("de".into()),
            music: 0.3,
            effects: 0.6,
            narration: 0.7,
            ui_size: 1.2,
            fullscreen: true,
            patient_ghosts: true,
            change_hint: true,
        };
        settings.store();
        assert_eq!(Settings::load(), settings);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
