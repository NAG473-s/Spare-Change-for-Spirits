//! Saving, loading and starting a new game.
//!
//! There are [`PROFILES`] profiles, each a game of its own. A profile has an
//! autosave, written at the start of every evening and night and at dawn,
//! and [`MANUAL_SLOTS`] slots the player saves to from the pause menu at any
//! time; autosaves never overwrite them. A save continues at the market or
//! at an evening: one made during a night continues at the start of that
//! night. Settings live in a separate file so they survive a new game.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::data::GameData;
use crate::day_cycle::{Phase, Progress};
use crate::economy::{Drawer, Purse};
use crate::inventory::{CratesOrdered, Stock};
use crate::loan::{Loan, lend, repay};
use crate::shop::{CellContent, ShopLayout, Storeroom};
use crate::upgrades::Upgrades;

/// Bump when the save format changes incompatibly; older saves are ignored.
pub const SAVE_VERSION: u32 = 4;
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
                autosave_night.after(crate::day_cycle::deliver_parcels),
            )
            // Keeping the shop open after the season: continue at that market.
            .add_systems(OnExit(Phase::DemoEnd), autosave_next_season)
            .add_systems(
                OnEnter(Phase::Morning),
                autosave_dawn
                    .after(repay)
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
    /// The season the shop is in, and the night it began (see `Progress`).
    #[serde(default = "first")]
    pub season: u32,
    #[serde(default = "first")]
    pub season_start: u32,
    pub drawer: Purse,
    pub stock: BTreeMap<String, Vec<u32>>,
    pub cells: Vec<CellContent>,
    /// Furniture bought or given but not placed (decor id → count).
    pub storeroom: BTreeMap<String, u32>,
    pub crates_ordered: u32,
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
        Ok(save) if save.version == SAVE_VERSION => Some(save),
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

/// The newest save of a profile (autosave or manual).
fn latest_in(dir: &Path, profile: u8) -> Option<(Slot, SaveGame, std::time::SystemTime)> {
    Slot::all()
        .filter_map(|slot| {
            let path = slot_path(dir, profile, slot);
            let time = std::fs::metadata(&path).and_then(|m| m.modified()).ok()?;
            Some((slot, read_save_at(&path)?, time))
        })
        .max_by_key(|(_, _, time)| *time)
}

/// The newest save of one profile.
pub fn latest_save(profile: u8) -> Option<(Slot, SaveGame)> {
    latest_in(&save_dir()?, profile).map(|(slot, save, _)| (slot, save))
}

/// The newest save over all profiles: what "Continue" loads.
pub fn newest_save() -> Option<(u8, Slot, SaveGame)> {
    let dir = save_dir()?;
    newest_in(&dir)
}

fn newest_in(dir: &Path) -> Option<(u8, Slot, SaveGame)> {
    (1..=PROFILES)
        .filter_map(|p| latest_in(dir, p).map(|(slot, save, time)| (p, slot, save, time)))
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
            season: self.progress.season,
            season_start: self.progress.season_start,
            drawer: self.drawer.0,
            stock: self.stock.units().clone(),
            cells: self.layout.cells.clone(),
            storeroom: self.storeroom.0.clone(),
            crates_ordered: self.crates.0,
            loan: self.loan.owed,
            upgrades: self.upgrades.0.iter().cloned().collect(),
            market_open,
            finished,
        }
    }

    pub fn apply(&mut self, save: &SaveGame, data: &GameData) {
        *self.progress = Progress {
            night: save.night,
            reputation: save.reputation,
            story: save.story.clone(),
            story_tries: save.story_tries.clone(),
            parcels: save.parcels.clone(),
            season: save.season,
            season_start: save.season_start,
        };
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
        self.loan.owed = 0;
        self.upgrades.0.clear();
    }
}

/// Whether this build can continue a save: always, unless the save is
/// past this build's last night (the end of the demo, played in the demo).
/// Saves never break between the demo and the full game: new fields need
/// `#[serde(default)]` instead of a new `SAVE_VERSION`.
fn first() -> u32 {
    1
}

pub fn can_continue(save: &SaveGame, final_night: u32) -> bool {
    // The full game lasts until the finale, however many nights it takes.
    save.night <= final_night || (!cfg!(feature = "demo") && !save.finished)
}

/// The state at the start of the current night: what a save made during
/// the night stores (the night itself continues from its start).
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
fn autosave_night(state: GameState, profile: Res<ActiveProfile>, mut start: ResMut<NightStart>) {
    let save = state.snapshot(state.progress.night, false, false);
    write_slot(profile.0, Slot::Auto, &save);
    start.0 = Some(save);
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
) -> Option<SaveGame> {
    match phase {
        Phase::Day => Some(state.snapshot(state.progress.night, false, true)),
        Phase::Evening => Some(state.snapshot(state.progress.night, false, false)),
        Phase::Night => night_start.0.clone(),
        Phase::Morning => Some(dawn_snapshot(state, data)),
        Phase::Menu | Phase::DemoEnd => None,
    }
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
            season: 1,
            season_start: 1,
            drawer: Purse::default(),
            stock: BTreeMap::new(),
            cells: Vec::new(),
            storeroom: BTreeMap::new(),
            crates_ordered: 0,
            loan: 0,
            upgrades: Vec::new(),
            market_open: false,
            finished: true,
        };
        assert!(!can_continue(&save, 5), "nothing left in the demo");
        assert!(can_continue(&save, 10), "the full game goes on");
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
            season: 1,
            season_start: 1,
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
            season: 1,
            season_start: 1,
            drawer: Purse::default(),
            stock: BTreeMap::new(),
            cells: Vec::new(),
            storeroom: BTreeMap::new(),
            crates_ordered: 0,
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
    fn the_newest_save_wins_across_profiles_and_slots() {
        let dir = temp_dir("profiles");
        assert!(newest_in(&dir).is_none());
        write_ron(&slot_path(&dir, 1, Slot::Auto), &sample(2));
        std::thread::sleep(std::time::Duration::from_millis(20));
        write_ron(&slot_path(&dir, 2, Slot::Manual(3)), &sample(4));
        let (profile, slot, save) = newest_in(&dir).unwrap();
        assert_eq!((profile, slot, save.night), (2, Slot::Manual(3), 4));
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
