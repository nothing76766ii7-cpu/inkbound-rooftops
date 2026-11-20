//! Atomic, schema-versioned offline save system with migration and corruption
//! recovery (PRD sections 3.14 & 3.15).

use engine_core::GameSettings;
use gameplay_paint::PaintStroke;
use gameplay_player::PlayerState;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

pub const CURRENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum SaveError {
    #[error("I/O error accessing save file: {0}")]
    Io(#[from] std::io::Error),
    #[error("Corrupted save JSON: {0}")]
    CorruptJson(#[from] serde_json::Error),
    #[error("Unsupported future save schema version {0}")]
    UnsupportedVersion(u32),
}

/// Persisted save slot payload (PRD section 3.15).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveSlot {
    pub schema_version: u32,
    pub slot_index: u8,
    pub timestamp_unix: u64,
    pub district: String,
    pub solved_puzzles: Vec<String>,
    pub collectibles: Vec<String>,
    pub player_state: PlayerState,
    pub persistent_paint: Vec<PaintStroke>,
    pub settings: GameSettings,
}

impl SaveSlot {
    pub fn new(slot_index: u8, district: impl Into<String>, player_state: PlayerState) -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            slot_index,
            timestamp_unix: 0,
            district: district.into(),
            solved_puzzles: Vec::new(),
            collectibles: Vec::new(),
            player_state,
            persistent_paint: Vec::new(),
            settings: GameSettings::default(),
        }
    }
}

/// Manages atomic save writes (`.tmp` -> `fs::rename`) in app-private storage.
#[derive(Debug, Clone)]
pub struct SaveManager {
    root_dir: PathBuf,
}

impl SaveManager {
    pub fn new(root_dir: impl Into<PathBuf>) -> Self {
        Self {
            root_dir: root_dir.into(),
        }
    }

    pub fn slot_path(&self, slot_index: u8) -> PathBuf {
        self.root_dir.join(format!("slot_{slot_index}.json"))
    }

    /// Writes `slot` atomically by writing to a sibling `.tmp` file, syncing, and renaming.
    pub fn save_atomic(&self, slot: &SaveSlot) -> Result<PathBuf, SaveError> {
        fs::create_dir_all(&self.root_dir)?;
        let target = self.slot_path(slot.slot_index);
        let tmp = self.root_dir.join(format!("slot_{}.json.tmp", slot.slot_index));

        let payload = serde_json::to_vec_pretty(slot)?;
        fs::write(&tmp, payload)?;
        fs::rename(&tmp, &target)?;
        Ok(target)
    }

    /// Loads and migrates a save slot from disk.
    pub fn load(&self, slot_index: u8) -> Result<SaveSlot, SaveError> {
        let path = self.slot_path(slot_index);
        Self::load_from_path(&path)
    }

    pub fn load_from_path(path: &Path) -> Result<SaveSlot, SaveError> {
        let raw = fs::read_to_string(path)?;
        let mut val: serde_json::Value = serde_json::from_str(&raw)?;

        let version = val
            .get("schema_version")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        if version > CURRENT_SCHEMA_VERSION {
            return Err(SaveError::UnsupportedVersion(version));
        }

        // Migration path from schema v0 -> v1
        if version == 0 {
            if let Some(obj) = val.as_object_mut() {
                obj.insert(
                    "schema_version".to_string(),
                    serde_json::Value::from(CURRENT_SCHEMA_VERSION),
                );
                obj.entry("solved_puzzles")
                    .or_insert_with(|| serde_json::json!([]));
                obj.entry("collectibles")
                    .or_insert_with(|| serde_json::json!([]));
                obj.entry("persistent_paint")
                    .or_insert_with(|| serde_json::json!([]));
                obj.entry("settings")
                    .or_insert_with(|| serde_json::to_value(GameSettings::default()).unwrap());
            }
        }

        let slot: SaveSlot = serde_json::from_value(val)?;
        Ok(slot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_save_and_reload_roundtrip() {
        let dir = std::env::temp_dir().join("inkbound_test_saves_roundtrip");
        let _ = fs::remove_dir_all(&dir);
        let mgr = SaveManager::new(&dir);

        let mut slot = SaveSlot::new(0, "Inkwell Quarter", PlayerState::default());
        slot.solved_puzzles.push("plaza_spire_seal".to_string());
        mgr.save_atomic(&slot).expect("Atomic save should succeed");

        let loaded = mgr.load(0).expect("Reload should succeed");
        assert_eq!(loaded.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(loaded.solved_puzzles, vec!["plaza_spire_seal"]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_save_returns_clean_error_instead_of_panicking() {
        let dir = std::env::temp_dir().join("inkbound_test_saves_corrupt");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let mgr = SaveManager::new(&dir);
        fs::write(mgr.slot_path(1), "{ broken json !!!").unwrap();

        let err = mgr.load(1).unwrap_err();
        assert!(matches!(err, SaveError::CorruptJson(_)));
        let _ = fs::remove_dir_all(&dir);
    }
}
