//! Scalable grid/cell world streaming architecture and dense medieval-sketchbook
//! Vertical Slice District ("Inkwell Quarter" — PRD v2.0 sections 3.8 & 4.6).

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
    WarmPlasterWall,
    StoneTrim,
    OrangeTiledRoof,
    SlateRoofAccent,
    InkWindowAccent,
    ChimneyStone,
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

/// Sliceable wooden/ink crate that splits into two colliding halves when cut (PRD 4.3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SliceableCrate {
    pub id: u64,
    pub cell_id: (i32, i32),
    pub position: Vec3,
    pub half_extents: Vec3,
    pub intact: bool,
    /// When sliced, two halves separate with velocity impulses and clean up after timer.
    pub half_a_pos: Vec3,
    pub half_b_pos: Vec3,
    pub half_a_vel: Vec3,
    pub half_b_vel: Vec3,
    pub cleanup_timer: f32,
}

impl SliceableCrate {
    pub fn new(id: u64, cell_id: (i32, i32), position: Vec3) -> Self {
        Self {
            id,
            cell_id,
            position,
            half_extents: Vec3::new(0.65, 0.75, 0.65),
            intact: true,
            half_a_pos: position,
            half_b_pos: position,
            half_a_vel: Vec3::ZERO,
            half_b_vel: Vec3::ZERO,
            cleanup_timer: 0.0,
        }
    }

    pub fn collider(&self) -> Collider {
        Collider {
            id: self.id,
            surface_id: self.id as u32,
            cell_id: self.cell_id,
            bounds: Aabb::from_center_half_extents(
                self.position + Vec3::new(0.0, self.half_extents.y, 0.0),
                self.half_extents,
            ),
            layers: CollisionLayer::SLASHABLE_PROP,
            active: self.intact,
        }
    }

    pub fn slice(&mut self, impulse: Vec3) {
        if !self.intact {
            return;
        }
        self.intact = false;
        let center = self.position + Vec3::new(0.0, self.half_extents.y, 0.0);
        let side = impulse.cross(Vec3::Y).normalize_or_zero();
        let sep = if side.length_squared() > 0.1 { side } else { Vec3::X };
        self.half_a_pos = center - sep * 0.28;
        self.half_b_pos = center + sep * 0.28;
        self.half_a_vel = -sep * 3.8 + Vec3::Y * 4.2;
        self.half_b_vel = sep * 3.8 + Vec3::Y * 5.0;
        self.cleanup_timer = 8.0;
    }

    pub fn tick(&mut self, dt: f32) {
        if self.intact || self.cleanup_timer <= 0.0 {
            return;
        }
        self.cleanup_timer = (self.cleanup_timer - dt).max(0.0);
        self.half_a_vel.y -= 18.0 * dt;
        self.half_b_vel.y -= 18.0 * dt;
        self.half_a_pos += self.half_a_vel * dt;
        self.half_b_pos += self.half_b_vel * dt;
        let floor_y = self.position.y + 0.35;
        if self.half_a_pos.y < floor_y {
            self.half_a_pos.y = floor_y;
            self.half_a_vel *= 0.4;
        }
        if self.half_b_pos.y < floor_y {
            self.half_b_pos.y = floor_y;
            self.half_b_vel *= 0.4;
        }
    }
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

/// Manages active cells, LOD transitions, sliceable crates, and unified collision proxies.
#[derive(Debug, Clone)]
pub struct WorldStreamer {
    pub cells: HashMap<(i32, i32), WorldCell>,
    pub crates: Vec<SliceableCrate>,
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

    /// Helper to stamp a stylized medieval sketchbook house (plaster body + eaves + pitched roof
    /// + dark recessed windows + chimney) into a cell's block list.
    #[allow(clippy::too_many_arguments)]
    fn push_sketch_house(
        blocks: &mut Vec<WorldBlock>,
        next_id: &mut u64,
        min: Vec3,
        max: Vec3,
        roof_height: f32,
        wall_mat: MaterialKind,
        roof_mat: MaterialKind,
        add_chimney: bool,
        layers: CollisionLayer,
    ) {
        let mut add = |bmin: Vec3, bmax: Vec3, mat: MaterialKind, pitched: bool, lyr: CollisionLayer| {
            let id = *next_id;
            *next_id += 1;
            blocks.push(WorldBlock {
                id,
                bounds: Aabb::new(bmin, bmax),
                material: mat,
                pitched_roof: pitched,
                layers: lyr,
            });
        };

        // Main plaster building body
        add(min, max, wall_mat, false, layers);

        // Dark charcoal recessed windows on +Z and -Z facades (visual detail, non-colliding)
        let width = max.x - min.x;
        let height = max.y - min.y;
        let cols = ((width / 3.4).floor() as i32).clamp(1, 4);
        let rows = ((height / 3.8).floor() as i32).clamp(1, 3);
        for r in 0..rows {
            let wy = min.y + 2.2 + r as f32 * 3.4;
            if wy + 1.4 > max.y - 0.8 {
                continue;
            }
            for c in 0..cols {
                let wx = min.x + (c as f32 + 0.5) * (width / cols as f32);
                // Front facade window
                add(
                    Vec3::new(wx - 0.45, wy, max.z - 0.08),
                    Vec3::new(wx + 0.45, wy + 1.35, max.z + 0.08),
                    MaterialKind::InkWindowAccent,
                    false,
                    CollisionLayer::NONE,
                );
                // Back facade window
                add(
                    Vec3::new(wx - 0.45, wy, min.z - 0.08),
                    Vec3::new(wx + 0.45, wy + 1.35, min.z + 0.08),
                    MaterialKind::InkWindowAccent,
                    false,
                    CollisionLayer::NONE,
                );
            }
        }

        // Overhanging roof eaves trim
        let eave = 0.55;
        add(
            Vec3::new(min.x - eave, max.y - 0.25, min.z - eave),
            Vec3::new(max.x + eave, max.y + 0.35, max.z + eave),
            roof_mat,
            false,
            layers,
        );

        // Pitched terracotta/orange roof pyramid
        if roof_height > 0.2 {
            add(
                Vec3::new(min.x - eave, max.y + 0.35, min.z - eave),
                Vec3::new(max.x + eave, max.y + 0.35 + roof_height, max.z + eave),
                roof_mat,
                true,
                layers,
            );
        }

        // Optional brick/stone chimney silhouette
        if add_chimney {
            let cx = max.x - 1.6;
            let cz = (min.z + max.z) * 0.5;
            add(
                Vec3::new(cx - 0.55, max.y + 0.2, cz - 0.55),
                Vec3::new(cx + 0.55, max.y + roof_height * 0.85 + 1.6, cz + 0.55),
                MaterialKind::ChimneyStone,
                false,
                CollisionLayer::STRUCTURAL,
            );
        }
    }

    /// Push a single world block to list and increment the next ID counter
    #[inline]
    fn push_block(
        list: &mut Vec<WorldBlock>,
        next_id: &mut u64,
        min: Vec3,
        max: Vec3,
        mat: MaterialKind,
        pitched: bool,
        layers: CollisionLayer,
    ) {
        let id = *next_id;
        *next_id += 1;
        list.push(WorldBlock {
            id,
            bounds: Aabb::new(min, max),
            material: mat,
            pitched_roof: pitched,
            layers,
        });
    }

    /// Builds the upgraded Vertical Slice District ("Inkwell Quarter") with dense
    /// reference-aligned architecture, 3 target Ink Wraiths, and 2 sliceable crates.
    pub fn build_vertical_slice() -> (Self, Vec<PaintPlatformAnchor>, Vec<PaintGlyphPuzzle>) {
        let mut cells = HashMap::new();
        let mut next_id = 1_u64;

        let solid_roof = CollisionLayer::STRUCTURAL
            .union(CollisionLayer::CLIMBABLE)
            .union(CollisionLayer::PAINTABLE);
        let wall_run_surface = CollisionLayer::STRUCTURAL
            .union(CollisionLayer::WALL_RUNNABLE)
            .union(CollisionLayer::PAINTABLE);

        // --- Cell (0, 0): Starting Terrace, Courtyard, Wall-Run Alley & Flanking Skyline ---
        let mut c00 = Vec::new();
        // Starting rooftop terrace with stone parapet walls (like 1000003870.mp4)
        Self::push_block(&mut c00, &mut next_id, Vec3::new(-9.0, 0.0, -9.0), Vec3::new(9.0, 10.6, 9.0), MaterialKind::WhitePlasterWall, false, solid_roof);
        Self::push_block(&mut c00, &mut next_id, Vec3::new(-9.4, 10.6, -9.4), Vec3::new(9.4, 11.2, 9.4), MaterialKind::OrangeTiledRoof, false, solid_roof);
        // Low stone balustrade/parapet flanks on starting terrace
        Self::push_block(&mut c00, &mut next_id, Vec3::new(-9.2, 11.2, -7.0), Vec3::new(-8.4, 12.2, 7.0), MaterialKind::StoneTrim, false, solid_roof);
        Self::push_block(&mut c00, &mut next_id, Vec3::new(8.4, 11.2, -7.0), Vec3::new(9.2, 12.2, 7.0), MaterialKind::StoneTrim, false, solid_roof);

        // Rooftop Bridge 1 -> Courtyard Roof
        Self::push_block(&mut c00, &mut next_id, Vec3::new(-6.0, 0.0, -25.0), Vec3::new(6.0, 11.6, -12.5), MaterialKind::WarmPlasterWall, false, solid_roof);
        Self::push_block(&mut c00, &mut next_id, Vec3::new(-6.5, 11.6, -25.5), Vec3::new(6.5, 12.4, -12.0), MaterialKind::OrangeTiledRoof, false, solid_roof);

        // Tall Wall-Run Billboard Wall on +X flank with crenellations
        Self::push_block(&mut c00, &mut next_id, Vec3::new(6.8, 10.0, -39.0), Vec3::new(9.2, 19.5, -22.0), MaterialKind::WhitePlasterWall, false, wall_run_surface);
        for i in 0..5 {
            let z0 = -38.5 + i as f32 * 3.4;
            Self::push_block(&mut c00, &mut next_id, Vec3::new(6.8, 19.5, z0), Vec3::new(9.2, 20.6, z0 + 1.8), MaterialKind::StoneTrim, false, CollisionLayer::STRUCTURAL);
        }

        // Second stepped rooftop (mantle & slide obstacle)
        Self::push_block(&mut c00, &mut next_id, Vec3::new(-7.5, 0.0, -45.5), Vec3::new(7.5, 12.8, -33.5), MaterialKind::WhitePlasterWall, false, solid_roof);
        Self::push_block(&mut c00, &mut next_id, Vec3::new(-8.0, 12.8, -46.0), Vec3::new(8.0, 13.6, -33.0), MaterialKind::OrangeTiledRoof, false, solid_roof);
        // Overhead wooden/stone archway beam to practice sliding under (PRD 4.4)
        Self::push_block(&mut c00, &mut next_id, Vec3::new(-7.0, 14.9, -40.2), Vec3::new(7.0, 16.4, -38.8), MaterialKind::StoneTrim, false, CollisionLayer::STRUCTURAL);

        // Dense flanking medieval houses with windows, eaves, pitched orange roofs & chimneys
        Self::push_sketch_house(&mut c00, &mut next_id, Vec3::new(-23.0, 0.0, -14.0), Vec3::new(-11.5, 15.5, 4.0), 5.2, MaterialKind::WhitePlasterWall, MaterialKind::OrangeTiledRoof, true, solid_roof);
        Self::push_sketch_house(&mut c00, &mut next_id, Vec3::new(11.5, 0.0, -16.0), Vec3::new(24.0, 16.5, 2.0), 5.8, MaterialKind::WarmPlasterWall, MaterialKind::OrangeTiledRoof, true, solid_roof);
        Self::push_sketch_house(&mut c00, &mut next_id, Vec3::new(-24.0, 0.0, -36.0), Vec3::new(-11.0, 17.0, -18.0), 6.0, MaterialKind::WarmPlasterWall, MaterialKind::OrangeTiledRoof, true, solid_roof);
        Self::push_sketch_house(&mut c00, &mut next_id, Vec3::new(12.0, 0.0, -42.0), Vec3::new(25.0, 18.0, -24.0), 6.2, MaterialKind::WhitePlasterWall, MaterialKind::SlateRoofAccent, true, solid_roof);

        cells.insert(
            (0, 0),
            WorldCell {
                id: (0, 0),
                district_name: "Inkwell Quarter — Tutorial Rooftops".to_string(),
                bounds: Aabb::new(Vec3::new(-48.0, -15.0, -48.0), Vec3::new(48.0, 60.0, 48.0)),
                lod: LodLevel::Lod0Full,
                blocks: c00,
                checkpoints: vec![Checkpoint {
                    id: "cp_tutorial_start".to_string(),
                    position: Vec3::new(0.0, 11.4, 3.0),
                    cell_id: (0, 0),
                }],
                enemy_spawns: vec![
                    // Immediate training dummy / Ink Wraith on starting rooftop for instant slash testing (PRD 4.3)
                    EnemySpawn {
                        id: 9000,
                        position: Vec3::new(0.0, 11.2, -4.5),
                        patrol_a: Vec3::new(-2.0, 11.2, -4.5),
                        patrol_b: Vec3::new(2.0, 11.2, -4.5),
                        cell_id: (0, 0),
                    },
                ],
            },
        );

        // --- Cell (0, -1): Combat Plaza, Paint Puzzle & Tall Spire Skyline ---
        let mut c01 = Vec::new();
        Self::push_block(&mut c01, &mut next_id, Vec3::new(-16.0, 0.0, -76.0), Vec3::new(16.0, 12.6, -52.0), MaterialKind::WhitePlasterWall, false, solid_roof);
        Self::push_block(&mut c01, &mut next_id, Vec3::new(-16.6, 12.6, -76.6), Vec3::new(16.6, 13.5, -51.4), MaterialKind::OrangeTiledRoof, false, solid_roof);

        // Central Landmark Orange Spire Tower
        Self::push_sketch_house(&mut c01, &mut next_id, Vec3::new(-5.5, 0.0, -94.0), Vec3::new(5.5, 34.0, -83.0), 12.0, MaterialKind::WhitePlasterWall, MaterialKind::OrangeTiledRoof, false, solid_roof);
        // Layered flanking towers for rich skyline silhouette
        Self::push_sketch_house(&mut c01, &mut next_id, Vec3::new(-28.0, 0.0, -86.0), Vec3::new(-16.0, 24.0, -70.0), 7.5, MaterialKind::WarmPlasterWall, MaterialKind::OrangeTiledRoof, true, solid_roof);
        Self::push_sketch_house(&mut c01, &mut next_id, Vec3::new(18.0, 0.0, -88.0), Vec3::new(30.0, 27.0, -72.0), 8.5, MaterialKind::WhitePlasterWall, MaterialKind::OrangeTiledRoof, true, solid_roof);
        Self::push_sketch_house(&mut c01, &mut next_id, Vec3::new(-18.0, 0.0, -112.0), Vec3::new(-6.0, 30.0, -98.0), 9.0, MaterialKind::StoneTrim, MaterialKind::OrangeTiledRoof, false, solid_roof);
        Self::push_sketch_house(&mut c01, &mut next_id, Vec3::new(10.0, 0.0, -114.0), Vec3::new(22.0, 32.0, -100.0), 9.5, MaterialKind::StoneTrim, MaterialKind::SlateRoofAccent, false, solid_roof);

        cells.insert(
            (0, -1),
            WorldCell {
                id: (0, -1),
                district_name: "Inkwell Quarter — Combat Plaza & Spire".to_string(),
                bounds: Aabb::new(Vec3::new(-48.0, -15.0, -96.0), Vec3::new(48.0, 60.0, -48.0)),
                lod: LodLevel::Lod0Full,
                blocks: c01,
                checkpoints: vec![Checkpoint {
                    id: "cp_combat_plaza".to_string(),
                    position: Vec3::new(0.0, 13.7, -54.0),
                    cell_id: (0, -1),
                }],
                enemy_spawns: vec![
                    EnemySpawn {
                        id: 9001,
                        position: Vec3::new(-4.0, 13.5, -63.0),
                        patrol_a: Vec3::new(-8.0, 13.5, -63.0),
                        patrol_b: Vec3::new(3.0, 13.5, -63.0),
                        cell_id: (0, -1),
                    },
                    EnemySpawn {
                        id: 9002,
                        position: Vec3::new(6.0, 13.5, -69.0),
                        patrol_a: Vec3::new(2.0, 13.5, -69.0),
                        patrol_b: Vec3::new(10.0, 13.5, -65.0),
                        cell_id: (0, -1),
                    },
                ],
            },
        );

        // Two sliceable crates (one right on the starting rooftop, one on the courtyard bridge)
        let crates = vec![
            SliceableCrate::new(7001, (0, 0), Vec3::new(3.2, 11.2, -3.0)),
            SliceableCrate::new(7002, (0, 0), Vec3::new(0.0, 12.4, -18.0)),
        ];

        // Authored temporary paint platform bridging Cell (0,0) and Cell (0,-1)
        let anchors = vec![PaintPlatformAnchor {
            id: 5001,
            cell_id: (0, 0),
            bounds: Aabb::new(Vec3::new(-2.8, 13.0, -51.4), Vec3::new(2.8, 13.6, -45.6)),
            active_remaining_secs: 0.0,
            max_duration_secs: 14.0,
        }];

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
            crates,
            active_cell: (0, 0),
            lod0_radius: 1,
            lod1_radius: 2,
        };
        (streamer, anchors, puzzles)
    }

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

    pub fn tick_props(&mut self, dt: f32) {
        for c in &mut self.crates {
            c.tick(dt);
        }
    }

    pub fn slice_crate(&mut self, id: u64, impulse: Vec3) -> bool {
        if let Some(c) = self.crates.iter_mut().find(|c| c.id == id && c.intact) {
            c.slice(impulse);
            return true;
        }
        false
    }

    pub fn collect_colliders(&self) -> Vec<Collider> {
        let mut out = Vec::new();
        for (coord, cell) in &self.cells {
            if cell.lod != LodLevel::Lod0Full {
                continue;
            }
            for block in &cell.blocks {
                if block.layers == CollisionLayer::NONE {
                    continue;
                }
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
        for c in &self.crates {
            if c.intact {
                out.push(c.collider());
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
    fn sliceable_crate_splits_into_halves_and_deactivates_intact_collider() {
        let (mut streamer, _, _) = WorldStreamer::build_vertical_slice();
        assert!(streamer.crates.len() >= 2);
        let id = streamer.crates[0].id;
        assert!(streamer.slice_crate(id, Vec3::NEG_Z));
        assert!(!streamer.crates[0].intact);
        streamer.tick_props(0.2);
        assert!(streamer.crates[0].half_a_pos.distance(streamer.crates[0].half_b_pos) > 0.5);
    }
}
