//! Scalable grid/cell world streaming architecture and authored Vertical Slice District
//! ("Inkwell Quarter" — PRD section 3.8).

use engine_core::{Aabb, Collider, CollisionLayer};
use gameplay_paint::{PaintGlyphPuzzle, PaintPlatformAnchor};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const CELL_SIZE_METERS: f32 = 48.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LodLevel {
    Lod0Full,
    Lod1Silhouette,
    Unloaded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaterialKind {
    WhitePlasterWall,
    StoneTrim,
    OrangeTiledRoof,
    InkWindowAccent,
    CheckpointShrine,
    SlashableProp,
}

/// Visual + structural building/rooftop block within a streamed `WorldCell`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldBlock {
    pub id: u64,
    pub bounds: Aabb,
    pub material: MaterialKind,
    pub pitched_roof: bool,
    pub layers: CollisionLayer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: String,
    pub position: Vec3,
    pub cell_id: (i32, i32),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnemySpawn {
    pub id: u64,
    pub position: Vec3,
    pub patrol_a: Vec3,
    pub patrol_b: Vec3,
    pub cell_id: (i32, i32),
}

/// Streamed spatial cell (PRD section 3.15).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldCell {
    pub id: (i32, i32),
    pub district_name: String,
    pub bounds: Aabb,
    pub lod: LodLevel,
    pub blocks: Vec<WorldBlock>,
    pub checkpoints: Vec<Checkpoint>,
    pub enemy_spawns: Vec<EnemySpawn>,
}

/// Manages active cells, LOD transitions, and unified collision proxy extraction.
#[derive(Debug, Clone)]
pub struct WorldStreamer {
    pub cells: HashMap<(i32, i32), WorldCell>,
    pub active_cell: (i32, i32),
    pub lod0_radius: i32,
    pub lod1_radius: i32,
}

impl WorldStreamer {
    pub fn world_to_cell(pos: Vec3) -> (i32, i32) {
        (
            (pos.x / CELL_SIZE_METERS).floor() as i32,
            (pos.z / CELL_SIZE_METERS).floor() as i32,
        )
    }

    /// Builds the playable Vertical Slice District ("Inkwell Quarter") containing:
    /// - Tutorial rooftop & alley
    /// - Training courtyard with wall-run corridor
    /// - 3 linked rooftop traversal loops
    /// - Combat plaza with Ink Wraith spawns
    /// - 3-glyph paint puzzle + authored paint platform anchor
    /// - Tall central Orange Spire landmark tower
    pub fn build_vertical_slice() -> (Self, Vec<PaintPlatformAnchor>, Vec<PaintGlyphPuzzle>) {
        let mut cells = HashMap::new();
        let mut next_id = 1_u64;

        let mut make_block =
            |min: Vec3, max: Vec3, material: MaterialKind, pitched_roof: bool, layers: CollisionLayer| {
                let id = next_id;
                next_id += 1;
                WorldBlock {
                    id,
                    bounds: Aabb::new(min, max),
                    material,
                    pitched_roof,
                    layers,
                }
            };

        let solid_roof = CollisionLayer::STRUCTURAL
            .union(CollisionLayer::CLIMBABLE)
            .union(CollisionLayer::PAINTABLE);
        let wall_run_surface = CollisionLayer::STRUCTURAL
            .union(CollisionLayer::WALL_RUNNABLE)
            .union(CollisionLayer::PAINTABLE);

        // --- Cell (0, 0): Tutorial Rooftop, Alley & Training Courtyard ---
        let mut c00_blocks = Vec::new();
        // Starting white plaster building + bright orange roof deck
        c00_blocks.push(make_block(
            Vec3::new(-10.0, 0.0, -10.0),
            Vec3::new(10.0, 10.0, 10.0),
            MaterialKind::WhitePlasterWall,
            false,
            solid_roof,
        ));
        c00_blocks.push(make_block(
            Vec3::new(-10.5, 10.0, -10.5),
            Vec3::new(10.5, 11.2, 10.5),
            MaterialKind::OrangeTiledRoof,
            false,
            solid_roof,
        ));
        // Parkour Loop 1: Stepped rooftop bridge heading -Z
        c00_blocks.push(make_block(
            Vec3::new(-6.0, 0.0, -24.0),
            Vec3::new(6.0, 11.5, -13.0),
            MaterialKind::WhitePlasterWall,
            false,
            solid_roof,
        ));
        c00_blocks.push(make_block(
            Vec3::new(-6.4, 11.5, -24.4),
            Vec3::new(6.4, 12.6, -12.6),
            MaterialKind::OrangeTiledRoof,
            false,
            solid_roof,
        ));
        // Wall-run billboard flank on +X side
        c00_blocks.push(make_block(
            Vec3::new(6.5, 10.0, -38.0),
            Vec3::new(8.5, 18.5, -22.0),
            MaterialKind::WhitePlasterWall,
            false,
            wall_run_surface,
        ));
        // Second stepped rooftop (mantle test)
        c00_blocks.push(make_block(
            Vec3::new(-7.0, 0.0, -45.0),
            Vec3::new(7.0, 12.8, -34.0),
            MaterialKind::WhitePlasterWall,
            false,
            solid_roof,
        ));
        c00_blocks.push(make_block(
            Vec3::new(-7.4, 12.8, -45.4),
            Vec3::new(7.4, 13.8, -33.6),
            MaterialKind::OrangeTiledRoof,
            false,
            solid_roof,
        ));
        // Decorative pitched-roof flanking houses for dense fantasy skyline
        c00_blocks.push(make_block(
            Vec3::new(-24.0, 0.0, -28.0),
            Vec3::new(-13.0, 14.0, -12.0),
            MaterialKind::WhitePlasterWall,
            false,
            solid_roof,
        ));
        c00_blocks.push(make_block(
            Vec3::new(-24.6, 14.0, -28.6),
            Vec3::new(-12.4, 18.5, -11.4),
            MaterialKind::OrangeTiledRoof,
            true,
            solid_roof,
        ));
        // Breakable training ink lantern prop (slashable, non-structural)
        c00_blocks.push(make_block(
            Vec3::new(0.0, 12.6, -18.0),
            Vec3::new(1.0, 14.4, -17.0),
            MaterialKind::SlashableProp,
            false,
            CollisionLayer::SLASHABLE_PROP,
        ));

        cells.insert(
            (0, 0),
            WorldCell {
                id: (0, 0),
                district_name: "Inkwell Quarter — Tutorial Rooftops".to_string(),
                bounds: Aabb::new(Vec3::new(0.0, -15.0, -48.0), Vec3::new(48.0, 60.0, 48.0)),
                lod: LodLevel::Lod0Full,
                blocks: c00_blocks,
                checkpoints: vec![Checkpoint {
                    id: "cp_tutorial_start".to_string(),
                    position: Vec3::new(0.0, 11.4, 2.0),
                    cell_id: (0, 0),
                }],
                enemy_spawns: vec![],
            },
        );

        // --- Cell (0, -1): Combat Plaza, Paint Puzzle & Landmark Orange Spire ---
        let mut c01_blocks = Vec::new();
        // Combat Plaza rooftop
        c01_blocks.push(make_block(
            Vec3::new(-16.0, 0.0, -76.0),
            Vec3::new(16.0, 12.5, -52.0),
            MaterialKind::WhitePlasterWall,
            false,
            solid_roof,
        ));
        c01_blocks.push(make_block(
            Vec3::new(-16.5, 12.5, -76.5),
            Vec3::new(16.5, 13.5, -51.5),
            MaterialKind::OrangeTiledRoof,
            false,
            solid_roof,
        ));
        // Central Landmark Orange Spire Tower
        c01_blocks.push(make_block(
            Vec3::new(-5.0, 0.0, -94.0),
            Vec3::new(5.0, 34.0, -84.0),
            MaterialKind::WhitePlasterWall,
            false,
            solid_roof,
        ));
        c01_blocks.push(make_block(
            Vec3::new(-5.8, 34.0, -94.8),
            Vec3::new(5.8, 44.0, -83.2),
            MaterialKind::OrangeTiledRoof,
            true,
            solid_roof,
        ));
        // Flanking tall towers for silhouette depth
        c01_blocks.push(make_block(
            Vec3::new(20.0, 0.0, -88.0),
            Vec3::new(30.0, 26.0, -74.0),
            MaterialKind::StoneTrim,
            false,
            solid_roof,
        ));
        c01_blocks.push(make_block(
            Vec3::new(19.4, 26.0, -88.6),
            Vec3::new(30.6, 32.0, -73.4),
            MaterialKind::OrangeTiledRoof,
            true,
            solid_roof,
        ));

        cells.insert(
            (0, -1),
            WorldCell {
                id: (0, -1),
                district_name: "Inkwell Quarter — Combat Plaza & Spire".to_string(),
                bounds: Aabb::new(Vec3::new(-48.0, -15.0, -96.0), Vec3::new(48.0, 60.0, -48.0)),
                lod: LodLevel::Lod0Full,
                blocks: c01_blocks,
                checkpoints: vec![Checkpoint {
                    id: "cp_combat_plaza".to_string(),
                    position: Vec3::new(0.0, 13.7, -54.0),
                    cell_id: (0, -1),
                }],
                enemy_spawns: vec![
                    EnemySpawn {
                        id: 9001,
                        position: Vec3::new(-4.0, 13.6, -64.0),
                        patrol_a: Vec3::new(-8.0, 13.6, -64.0),
                        patrol_b: Vec3::new(4.0, 13.6, -64.0),
                        cell_id: (0, -1),
                    },
                    EnemySpawn {
                        id: 9002,
                        position: Vec3::new(6.0, 13.6, -70.0),
                        patrol_a: Vec3::new(2.0, 13.6, -70.0),
                        patrol_b: Vec3::new(10.0, 13.6, -66.0),
                        cell_id: (0, -1),
                    },
                ],
            },
        );

        // Authored temporary paint platform bridging the gap between Cell (0,0) and Cell (0,-1)
        let anchors = vec![PaintPlatformAnchor {
            id: 5001,
            cell_id: (0, 0),
            bounds: Aabb::new(Vec3::new(-2.5, 13.0, -51.2), Vec3::new(2.5, 13.6, -45.6)),
            active_remaining_secs: 0.0,
            max_duration_secs: 14.0,
        }];

        // 3-glyph ordered paint puzzle on the Combat Plaza
        let puzzles = vec![PaintGlyphPuzzle::new(
            "plaza_spire_seal",
            vec![
                Vec3::new(-10.0, 13.6, -58.0),
                Vec3::new(0.0, 13.6, -68.0),
                Vec3::new(10.0, 13.6, -58.0),
            ],
        )];

        let streamer = Self {
            cells,
            active_cell: (0, 0),
            lod0_radius: 1,
            lod1_radius: 2,
        };
        (streamer, anchors, puzzles)
    }

    /// Updates cell LOD states based on player position.
    pub fn update_streaming(&mut self, player_pos: Vec3) {
        let center = Self::world_to_cell(player_pos);
        self.active_cell = center;
        for (coord, cell) in &mut self.cells {
            let dist = (coord.0 - center.0).abs().max((coord.1 - center.1).abs());
            cell.lod = if dist <= self.lod0_radius {
                LodLevel::Lod0Full
            } else if dist <= self.lod1_radius {
                LodLevel::Lod1Silhouette
            } else {
                LodLevel::Unloaded
            };
        }
    }

    /// Collects active solid/paintable/climbable colliders from all LOD0 cells.
    pub fn collect_colliders(&self) -> Vec<Collider> {
        let mut out = Vec::new();
        for (coord, cell) in &self.cells {
            if cell.lod != LodLevel::Lod0Full {
                continue;
            }
            for block in &cell.blocks {
                out.push(Collider {
                    id: block.id,
                    surface_id: block.id as u32,
                    cell_id: *coord,
                    bounds: block.bounds,
                    layers: block.layers,
                    active: true,
                });
            }
        }
        out
    }

    pub fn active_district_name(&self) -> &str {
        self.cells
            .get(&self.active_cell)
            .map(|c| c.district_name.as_str())
            .unwrap_or("Inkwell Quarter")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertical_slice_streams_cells_and_exports_colliders() {
        let (mut streamer, anchors, puzzles) = WorldStreamer::build_vertical_slice();
        assert_eq!(anchors.len(), 1);
        assert_eq!(puzzles.len(), 1);

        streamer.update_streaming(Vec3::new(0.0, 12.0, 0.0));
        let colliders = streamer.collect_colliders();
        assert!(colliders.len() >= 10, "Should expose active rooftop colliders");

        // Move far away so cells unload cleanly
        streamer.update_streaming(Vec3::new(500.0, 12.0, 500.0));
        assert!(streamer.collect_colliders().is_empty());
    }
}
