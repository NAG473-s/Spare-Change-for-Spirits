//! Saving, loading and starting a new game.
//!
//! The game autosaves at the start of every evening and night and at dawn.
//! A save always continues at an evening. Settings (tips on/off) live in a
//! separate file so they survive a new game.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::data::GameData;
use crate::day_cycle::{Phase, Progress, start_morning};
use crate::economy::{Drawer, Purse};
use crate::inventory::{CratesOrdered, Stock};
use crate::shop::{CellContent, Keepsakes, ShopLayout};

/// Bump when the save format changes incompatibly; older saves are ignored.
pub const SAVE_VERSION: u32 = 1;
/// Environment variable to put saves somewhere else (tests, smoke runs).
pub const SAVE_DIR_ENV: &str = "SCFS_SAVE_DIR";

pub struct SavePlugin;

impl Plugin for SavePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Settings::load())
            .add_systems(OnEnter(Phase::Evening), autosave_evening)
            .add_systems(OnEnter(Phase::Night), autosave_evening)
            .add_systems(OnEnter(Phase::Morning), autosave_dawn.after(start_morning));
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SaveGame {
    pub version: u32,
    /// The night whose evening the game continues at.
    pub night: u32,
    pub reputation: i32,
    pub story_chapter: usize,
    pub drawer: Purse,
    pub stock: BTreeMap<String, Vec<u32>>,
    pub cells: Vec<CellContent>,
    pub keepsakes: BTreeMap<String, u32>,
    pub crates_ordered: u32,
    /// The demo has been played to the end; there is nothing to continue.
    pub finished: bool,
}

#[derive(Resource, Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// Show the tips box during the first night.
    pub tips: bool,
    /// Play music and sound effects.
    pub sound: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            tips: true,
            sound: true,
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

/// The save to continue, if there is a readable one of this version.
pub fn read_save() -> Option<SaveGame> {
    let path = save_dir()?.join("save.ron");
    let text = std::fs::read_to_string(&path).ok()?;
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

pub fn write_save(save: &SaveGame) {
    if let Some(dir) = save_dir() {
        write_ron(&dir.join("save.ron"), save);
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
    pub keepsakes: ResMut<'w, Keepsakes>,
    pub crates: ResMut<'w, CratesOrdered>,
}

impl GameState<'_> {
    pub fn snapshot(&self, night: u32, finished: bool) -> SaveGame {
        SaveGame {
            version: SAVE_VERSION,
            night,
            reputation: self.progress.reputation,
            story_chapter: self.progress.story_chapter,
            drawer: self.drawer.0,
            stock: self.stock.units().clone(),
            cells: self.layout.cells.clone(),
            keepsakes: self.keepsakes.0.clone(),
            crates_ordered: self.crates.0,
            finished,
        }
    }

    pub fn apply(&mut self, save: &SaveGame, data: &GameData) {
        *self.progress = Progress {
            night: save.night,
            reputation: save.reputation,
            story_chapter: save.story_chapter,
        };
        self.drawer.0 = save.drawer;
        *self.stock = Stock::from_units(save.stock.clone());
        let mut layout = ShopLayout::new(data.config.grid_cols, data.config.grid_rows);
        // A save from a different grid size starts with the default furniture.
        if save.cells.len() == layout.cells.len() {
            layout.cells = save.cells.clone();
        }
        *self.layout = layout;
        self.keepsakes.0 = save.keepsakes.clone();
        self.crates.0 = save.crates_ordered;
    }

    pub fn reset(&mut self, data: &GameData) {
        *self.progress = Progress {
            night: 1,
            reputation: data.config.start_reputation,
            story_chapter: 0,
        };
        self.drawer.0 = data.config.start_coins;
        *self.stock = Stock::default();
        *self.layout = ShopLayout::new(data.config.grid_cols, data.config.grid_rows);
        self.keepsakes.0.clear();
        self.crates.0 = 0;
    }
}

/// Evening and the start of the night: continue at this evening.
fn autosave_evening(state: GameState) {
    write_save(&state.snapshot(state.progress.night, false));
}

/// Dawn: the night is done, continue at the next evening.
fn autosave_dawn(state: GameState, data: Res<GameData>) {
    let night = state.progress.night;
    let finished = night >= data.config.nights_in_demo;
    write_save(&state.snapshot(night + 1, finished));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_round_trips_through_ron() {
        let mut stock = BTreeMap::new();
        stock.insert("marbles".to_string(), vec![0, 1, 2]);
        let save = SaveGame {
            version: SAVE_VERSION,
            night: 3,
            reputation: -2,
            story_chapter: 2,
            drawer: Purse {
                buttons: 4,
                candles: 1,
                spoons: 2,
            },
            stock,
            cells: vec![
                CellContent {
                    decor: Some("shelf".into()),
                    product: Some("marbles".into()),
                },
                CellContent::default(),
            ],
            keepsakes: BTreeMap::from([("umbrella".to_string(), 1)]),
            crates_ordered: 2,
            finished: false,
        };
        let text = ron::ser::to_string_pretty(&save, ron::ser::PrettyConfig::default()).unwrap();
        let back: SaveGame = ron::from_str(&text).unwrap();
        assert_eq!(back, save);
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
        };
        settings.store();
        assert_eq!(Settings::load(), settings);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
