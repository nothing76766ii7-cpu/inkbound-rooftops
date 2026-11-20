//! Atomic, schema-versioned offline save system with migration, cut-history persistence
//! (PRD v3.0 Gate 9), and corruption recovery.

use engine_core::mesh_slicer::CutPlane;
use engine_core::GameSettings;
use gameplay_paint::PaintStroke;
use gameplay_player::PlayerState;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

pub const CURRENT_SCHEMA_VERSION: u32 = 4;
fn initial_level() -> u32 { 1 }
fn initial_seed() -> u64 { 0xA11C_E50D_1234_BEEF }
fn initial_day_phase() -> f32 { 0.37 }

#[derive(Debug, Error)]
pub enum SaveError {
    #[error("I/O error accessing save file: {0}")]
    Io(#[from] std::io::Error),
    #[error("Corrupted save JSON: {0}")]
    CorruptJson(#[from] serde_json::Error),
    #[error("Unsupported future save schema version {0}")]
    UnsupportedVersion(u32),
}

/// Persisted save slot payload (includes deterministic world-cut history).
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
    #[serde(default)]
    pub world_cuts: Vec<CutPlane>,
    pub settings: GameSettings,
    #[serde(default = "initial_level")]
    pub level_number: u32,
    #[serde(default = "initial_seed")]
    pub world_seed: u64,
    /// How many cuts had occurred at the latest activated checkpoint.
    #[serde(default)]
    pub checkpoint_cut_count: usize,
    /// In-game normalized clock; old saves safely begin in daylight.
    #[serde(default = "initial_day_phase")]
    pub day_phase: f32,
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
            world_cuts: Vec::new(),
            settings: GameSettings::default(),
            level_number: 1,
            world_seed: initial_seed(),
            checkpoint_cut_count: 0,
            day_phase: initial_day_phase(),
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

    pub fn save_atomic(&self, slot: &SaveSlot) -> Result<PathBuf, SaveError> {
        fs::create_dir_all(&self.root_dir)?;
        let target = self.slot_path(slot.slot_index);
        let tmp = self.root_dir.join(format!("slot_{}.json.tmp", slot.slot_index));

        let payload = serde_json::to_vec_pretty(slot)?;
        fs::write(&tmp, payload)?;
        fs::rename(&tmp, &target)?;
        Ok(target)
    }

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

        // Migrate v0/v1 -> v2 (adds world_cuts array)
        if version < CURRENT_SCHEMA_VERSION {
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
                obj.entry("world_cuts")
                    .or_insert_with(|| serde_json::json!([]));
                obj.entry("level_number")
                    .or_insert_with(|| serde_json::json!(1));
                obj.entry("world_seed")
                    .or_insert_with(|| serde_json::json!(initial_seed()));
                obj.entry("checkpoint_cut_count")
                    .or_insert_with(|| serde_json::json!(0));
                obj.entry("day_phase")
                    .or_insert_with(|| serde_json::json!(initial_day_phase()));
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
    use glam::Vec3;

    #[test]
    fn atomic_save_and_reload_preserves_world_cuts() {
        let dir = std::env::temp_dir().join("inkbound_test_saves_v3_cuts");
        let _ = fs::remove_dir_all(&dir);
        let mgr = SaveManager::new(&dir);

        let mut slot = SaveSlot::new(0, "Inkwell Quarter", PlayerState::default());
        slot.world_cuts.push(CutPlane::new(
            Vec3::new(0.0, 13.0, 0.0),
            Vec3::Y,
            Vec3::NEG_Z,
            18.0,
            6.0,
        ));
        mgr.save_atomic(&slot).expect("Atomic save should succeed");

        let loaded = mgr.load(0).expect("Reload should succeed");
        assert_eq!(loaded.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(loaded.world_cuts.len(), 1);
        let _ = fs::remove_dir_all(&dir);
    }
}
