//! Paint traversal, combat marking, and glyph puzzle system (PRD section 3.7).
//!
//! Supports Brush, Platform (authored anchor volumes only), Mark (enemy debuff),
//! Reveal (hidden paths), and Erase modes with bounded per-cell stroke budgets,
//! automatic expiration, and colorblind-safe shape symbols.

use engine_core::{raycast_world, Aabb, Collider, CollisionLayer, Palette};
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Paint tool mode selectable from the radial palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaintMode {
    /// Decorative/route-marker strokes on `PAINTABLE` surfaces (square symbol).
    Brush,
    /// Spawns a temporary solid paint foothold on an authored anchor volume (triangle symbol).
    Platform,
    /// Marks an enemy to amplify Dimensional Slash damage (diamond symbol).
    Mark,
    /// Reveals hidden wall-run or rooftop paths (eye symbol).
    Reveal,
    /// Removes player's own temporary strokes/platforms within aim range.
    Erase,
}

/// Restrained sketchbook pigment colors (always paired with shape symbols for colorblind safety).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaintColor {
    RoofOrange,
    CharcoalInk,
    AccentTeal,
}

impl PaintColor {
    pub fn rgba(self) -> [f32; 4] {
        match self {
            Self::RoofOrange => Palette::ROOF_ORANGE,
            Self::CharcoalInk => Palette::INK,
            Self::AccentTeal => Palette::PAINT_TEAL,
        }
    }
}

/// Persisted or temporary paint decal stroke (PRD section 3.15).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaintStroke {
    pub id: u64,
    pub cell_id: (i32, i32),
    pub surface_id: u32,
    pub position: Vec3,
    pub normal: Vec3,
    pub radius: f32,
    pub color: PaintColor,
    pub mode: PaintMode,
    pub permanent: bool,
    pub remaining_lifetime: f32,
}

/// Authored anchor volume where Platform paint can safely spawn a temporary foothold.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaintPlatformAnchor {
    pub id: u64,
    pub cell_id: (i32, i32),
    pub bounds: Aabb,
    pub active_remaining_secs: f32,
    pub max_duration_secs: f32,
}

/// Ordered paint glyph puzzle (PRD section 3.7: "paint three glyphs in order").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaintGlyphPuzzle {
    pub id: String,
    pub glyph_positions: Vec<Vec3>,
    pub next_index: usize,
    pub solved: bool,
}

impl PaintGlyphPuzzle {
    pub fn new(id: impl Into<String>, glyph_positions: Vec<Vec3>) -> Self {
        Self {
            id: id.into(),
            glyph_positions,
            next_index: 0,
            solved: false,
        }
    }

    /// Checks whether a newly placed stroke activates the next glyph in order.
    pub fn try_activate(&mut self, point: Vec3) -> bool {
        if self.solved || self.next_index >= self.glyph_positions.len() {
            return false;
        }
        let target = self.glyph_positions[self.next_index];
        if point.distance(target) <= 2.2 {
            self.next_index += 1;
            if self.next_index >= self.glyph_positions.len() {
                self.solved = true;
            }
            return true;
        }
        false
    }
}

/// Runtime paint manager enforcing pigment costs, per-cell budgets, and expiration.
#[derive(Debug, Clone)]
pub struct PaintSystem {
    pub active_mode: PaintMode,
    pub active_color: PaintColor,
    pub max_strokes_per_cell: usize,
    pub strokes: Vec<PaintStroke>,
    pub platform_anchors: Vec<PaintPlatformAnchor>,
    pub puzzles: Vec<PaintGlyphPuzzle>,
    pub revealed_secret_paths: bool,
    next_stroke_id: u64,
}

impl Default for PaintSystem {
    fn default() -> Self {
        Self {
            active_mode: PaintMode::Brush,
            active_color: PaintColor::RoofOrange,
            max_strokes_per_cell: 32,
            strokes: Vec::new(),
            platform_anchors: Vec::new(),
            puzzles: Vec::new(),
            revealed_secret_paths: false,
            next_stroke_id: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PaintActionResult {
    None,
    InsufficientPigment,
    StrokePlaced { glyph_activated: bool },
    PlatformActivated(u64),
    EnemyMarked(u64),
    PathsRevealed,
    ErasedCount(usize),
}

impl PaintSystem {
    pub fn cycle_mode(&mut self) {
        self.active_mode = match self.active_mode {
            PaintMode::Brush => PaintMode::Platform,
            PaintMode::Platform => PaintMode::Mark,
            PaintMode::Mark => PaintMode::Reveal,
            PaintMode::Reveal => PaintMode::Erase,
            PaintMode::Erase => PaintMode::Brush,
        };
    }

    pub fn pigment_cost(mode: PaintMode) -> f32 {
        match mode {
            PaintMode::Brush => 6.0,
            PaintMode::Platform => 24.0,
            PaintMode::Mark => 16.0,
            PaintMode::Reveal => 14.0,
            PaintMode::Erase => 0.0,
        }
    }

    /// Fires the active paint tool along the player's aim ray.
    pub fn apply_paint(
        &mut self,
        origin: Vec3,
        look_dir: Vec3,
        colliders: &[Collider],
        pigment: &mut f32,
    ) -> PaintActionResult {
        let cost = Self::pigment_cost(self.active_mode);
        if *pigment < cost {
            return PaintActionResult::InsufficientPigment;
        }

        match self.active_mode {
            PaintMode::Erase => {
                let before = self.strokes.len();
                self.strokes.retain(|s| {
                    s.permanent || (s.position - origin).length() > 8.0
                });
                let erased = before - self.strokes.len();
                PaintActionResult::ErasedCount(erased)
            }
            PaintMode::Mark => {
                if let Some(hit) = raycast_world(
                    colliders,
                    origin,
                    look_dir,
                    22.0,
                    CollisionLayer::SLASHABLE_ENEMY,
                ) {
                    *pigment -= cost;
                    return PaintActionResult::EnemyMarked(hit.collider_id);
                }
                PaintActionResult::None
            }
            PaintMode::Platform => {
                let dir = look_dir.normalize_or_zero();
                for anchor in &mut self.platform_anchors {
                    if anchor.bounds.raycast(origin, dir, 18.0).is_some() {
                        *pigment -= cost;
                        anchor.active_remaining_secs = anchor.max_duration_secs;
                        return PaintActionResult::PlatformActivated(anchor.id);
                    }
                }
                PaintActionResult::None
            }
            PaintMode::Reveal => {
                *pigment -= cost;
                self.revealed_secret_paths = true;
                PaintActionResult::PathsRevealed
            }
            PaintMode::Brush => {
                let hit = match raycast_world(
                    colliders,
                    origin,
                    look_dir,
                    16.0,
                    CollisionLayer::PAINTABLE,
                ) {
                    Some(h) => h,
                    None => return PaintActionResult::None,
                };
                *pigment -= cost;
                self.push_stroke(PaintStroke {
                    id: self.next_stroke_id,
                    cell_id: hit.cell_id,
                    surface_id: hit.surface_id,
                    position: hit.point + hit.normal * 0.02,
                    normal: hit.normal,
                    radius: 0.65,
                    color: self.active_color,
                    mode: PaintMode::Brush,
                    permanent: false,
                    remaining_lifetime: 45.0,
                });
                self.next_stroke_id += 1;

                let mut glyph_activated = false;
                for puzzle in &mut self.puzzles {
                    if puzzle.try_activate(hit.point) {
                        glyph_activated = true;
                    }
                }
                PaintActionResult::StrokePlaced { glyph_activated }
            }
        }
    }

    /// Inserts a stroke while enforcing `max_strokes_per_cell` (evicting oldest temporary stroke).
    pub fn push_stroke(&mut self, stroke: PaintStroke) {
        let cell = stroke.cell_id;
        let count_in_cell = self.strokes.iter().filter(|s| s.cell_id == cell).count();
        if count_in_cell >= self.max_strokes_per_cell {
            if let Some(oldest_idx) = self
                .strokes
                .iter()
                .position(|s| s.cell_id == cell && !s.permanent)
            {
                self.strokes.remove(oldest_idx);
            }
        }
        self.strokes.push(stroke);
    }

    /// Expires temporary strokes and paint platforms cleanly to free memory.
    pub fn tick(&mut self, dt: f32) {
        for s in &mut self.strokes {
            if !s.permanent {
                s.remaining_lifetime -= dt;
            }
        }
        self.strokes.retain(|s| s.permanent || s.remaining_lifetime > 0.0);

        for anchor in &mut self.platform_anchors {
            if anchor.active_remaining_secs > 0.0 {
                anchor.active_remaining_secs = (anchor.active_remaining_secs - dt).max(0.0);
            }
        }
    }

    /// Exports active temporary paint platforms as `PAINT_PLATFORM` colliders.
    pub fn active_platform_colliders(&self) -> Vec<Collider> {
        self.platform_anchors
            .iter()
            .filter(|a| a.active_remaining_secs > 0.0)
            .map(|a| Collider {
                id: a.id,
                surface_id: a.id as u32,
                cell_id: a.cell_id,
                bounds: a.bounds,
                layers: CollisionLayer::PAINT_PLATFORM,
                active: true,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn per_cell_stroke_budget_evicts_oldest_and_expiry_frees_memory() {
        let mut paint = PaintSystem {
            max_strokes_per_cell: 3,
            ..Default::default()
        };
        for i in 1..=5 {
            paint.push_stroke(PaintStroke {
                id: i,
                cell_id: (0, 0),
                surface_id: 1,
                position: Vec3::ZERO,
                normal: Vec3::Y,
                radius: 0.5,
                color: PaintColor::RoofOrange,
                mode: PaintMode::Brush,
                permanent: false,
                remaining_lifetime: 2.0,
            });
        }
        assert_eq!(paint.strokes.len(), 3);
        assert_eq!(paint.strokes[0].id, 3, "Oldest temporary strokes should be evicted");

        paint.tick(2.5);
        assert!(paint.strokes.is_empty(), "Expired temporary strokes must release memory");
    }

    #[test]
    fn ordered_three_glyph_puzzle_solves_in_sequence() {
        let mut puzzle = PaintGlyphPuzzle::new(
            "inkwell_gate",
            vec![
                Vec3::new(0.0, 10.0, 0.0),
                Vec3::new(5.0, 10.0, 0.0),
                Vec3::new(10.0, 10.0, 0.0),
            ],
        );
        // Out of order should not advance
        assert!(!puzzle.try_activate(Vec3::new(10.0, 10.0, 0.0)));
        assert_eq!(puzzle.next_index, 0);

        assert!(puzzle.try_activate(Vec3::new(0.2, 10.0, 0.0)));
        assert!(puzzle.try_activate(Vec3::new(5.0, 10.0, 0.1)));
        assert!(!puzzle.solved);
        assert!(puzzle.try_activate(Vec3::new(10.0, 10.0, 0.0)));
        assert!(puzzle.solved);
    }
}
