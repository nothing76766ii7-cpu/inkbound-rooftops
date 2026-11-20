//! Scalable grid/cell world streaming + REAL sliceable architectural modules,
//! support graph, dynamic falling fragments, and cut persistence (PRD v3.0).

use engine_core::mesh_slicer::{CutPlane, SliceableMesh};
use engine_core::{Aabb, Collider, CollisionLayer, Palette};
use gameplay_paint::{PaintGlyphPuzzle, PaintPlatformAnchor};
use glam::{Quat, Vec3};
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

impl MaterialKind {
    pub fn exterior_color(self) -> [f32; 4] {
        match self {
            Self::WhitePlasterWall => Palette::PAPER,
            Self::WarmPlasterWall => [0.935, 0.918, 0.885, 1.0],
            Self::StoneTrim => Palette::STONE,
            Self::OrangeTiledRoof => Palette::ROOF_ORANGE,
            Self::SlateRoofAccent => [0.62, 0.56, 0.54, 1.0],
            Self::InkWindowAccent => [0.19, 0.20, 0.23, 1.0],
            Self::ChimneyStone => [0.76, 0.74, 0.72, 1.0],
            Self::CheckpointShrine => Palette::PAINT_TEAL,
            Self::SlashableProp => [0.78, 0.54, 0.34, 1.0],
        }
    }

    /// Fresh cross-section core color exposed when a blade slices through this material (PRD v3.0).
    pub fn cut_core_color(self) -> [f32; 4] {
        match self {
            Self::OrangeTiledRoof | Self::SlateRoofAccent | Self::SlashableProp => {
                Palette::CUT_CORE_TIMBER
            }
            _ => Palette::CUT_CORE_STONE,
        }
    }
}

/// Real sliceable architectural module (wall span, roof panel, parapet, tower section, crate).
/// Supports arbitrary plane slicing, closed caps, repeat cuts, support-graph detachment,
/// delayed suspense beat, and dynamic falling physics (PRD v3.0 Gates 1–3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SliceableModule {
    pub id: u64,
    pub cell_id: (i32, i32),
    pub material: MaterialKind,
    pub mesh: SliceableMesh,
    pub bounds: Aabb,
    pub layers: CollisionLayer,
    /// Remaining allowed repeat cuts on this piece (Gate 3).
    pub remaining_cuts: u8,
    /// Ground/roof elevation beneath this piece for dynamic settling.
    pub floor_y: f32,
    /// Dynamic rigid-body state once detached/unsupported.
    pub is_dynamic: bool,
    pub offset: Vec3,
    pub velocity: Vec3,
    pub rotation: Quat,
    pub angular_vel: Vec3,
    pub mass: f32,
    /// Brief suspense beat (`0.14s`) before cut halves separate (PRD v3.0 Final Principle).
    pub suspense_timer: f32,
    pub pending_impulse: Vec3,
    pub pending_ang_vel: Vec3,
    /// Luminous glowing cut-seam timer (`0.65s`).
    pub cut_glow_timer: f32,
    pub sleeping: bool,
    /// Optional lifetime for tiny rubble fragments (None = permanent architectural piece).
    pub rubble_lifetime: Option<f32>,
}

impl SliceableModule {
    pub fn new_box(
        id: u64,
        cell_id: (i32, i32),
        bounds: Aabb,
        material: MaterialKind,
        layers: CollisionLayer,
        remaining_cuts: u8,
        floor_y: f32,
    ) -> Self {
        let mesh = SliceableMesh::from_box(bounds, material.exterior_color());
        let mass = (mesh.approx_volume() * 120.0).clamp(15.0, 4000.0);
        Self {
            id,
            cell_id,
            material,
            mesh,
            bounds,
            layers,
            remaining_cuts,
            floor_y,
            is_dynamic: false,
            offset: Vec3::ZERO,
            velocity: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            angular_vel: Vec3::ZERO,
            mass,
            suspense_timer: 0.0,
            pending_impulse: Vec3::ZERO,
            pending_ang_vel: Vec3::ZERO,
            cut_glow_timer: 0.0,
            sleeping: false,
            rubble_lifetime: None,
        }
    }

    pub fn new_pitched_roof(
        id: u64,
        cell_id: (i32, i32),
        bounds: Aabb,
        material: MaterialKind,
        layers: CollisionLayer,
        remaining_cuts: u8,
        floor_y: f32,
    ) -> Self {
        let mesh = SliceableMesh::from_pitched_roof(bounds, material.exterior_color());
        let mass = (mesh.approx_volume() * 80.0).clamp(15.0, 2500.0);
        Self {
            id,
            cell_id,
            material,
            mesh,
            bounds,
            layers,
            remaining_cuts,
            floor_y,
            is_dynamic: false,
            offset: Vec3::ZERO,
            velocity: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            angular_vel: Vec3::ZERO,
            mass,
            suspense_timer: 0.0,
            pending_impulse: Vec3::ZERO,
            pending_ang_vel: Vec3::ZERO,
            cut_glow_timer: 0.0,
            sleeping: false,
            rubble_lifetime: None,
        }
    }

    /// World-space AABB accounting for dynamic offset after detachment.
    pub fn world_bounds(&self) -> Aabb {
        Aabb::new(self.bounds.min + self.offset, self.bounds.max + self.offset)
    }

    pub fn collider(&self) -> Collider {
        Collider {
            id: self.id,
            surface_id: self.id as u32,
            cell_id: self.cell_id,
            bounds: self.world_bounds(),
            layers: self.layers,
            active: self.rubble_lifetime.map_or(true, |t| t > 0.0),
        }
    }

    pub fn tick_physics(&mut self, dt: f32) {
        self.cut_glow_timer = (self.cut_glow_timer - dt).max(0.0);

        if let Some(ref mut life) = self.rubble_lifetime {
            *life = (*life - dt).max(0.0);
        }

        if self.suspense_timer > 0.0 {
            self.suspense_timer = (self.suspense_timer - dt).max(0.0);
            if self.suspense_timer <= 0.0 && self.is_dynamic {
                self.velocity = self.pending_impulse;
                self.angular_vel = self.pending_ang_vel;
            }
            return;
        }

        if !self.is_dynamic || self.sleeping {
            return;
        }

        // Gravity, linear integration, and angular tumble
        self.velocity.y -= 19.5 * dt;
        self.offset += self.velocity * dt;

        let ang_speed = self.angular_vel.length();
        if ang_speed > 1e-4 {
            let dq = Quat::from_axis_angle(self.angular_vel / ang_speed, ang_speed * dt);
            self.rotation = (dq * self.rotation).normalize();
        }

        // Floor / roof settling with friction & sleep threshold
        let wb = self.world_bounds();
        if wb.min.y <= self.floor_y {
            let push_y = self.floor_y - wb.min.y;
            self.offset.y += push_y;
            if self.velocity.y < 0.0 {
                self.velocity.y = -self.velocity.y * 0.18;
            }
            self.velocity.x *= (1.0 - 5.0 * dt).max(0.0);
            self.velocity.z *= (1.0 - 5.0 * dt).max(0.0);
            self.angular_vel *= (1.0 - 6.0 * dt).max(0.0);

            if self.velocity.length() < 0.15 && self.angular_vel.length() < 0.15 {
                self.velocity = Vec3::ZERO;
                self.angular_vel = Vec3::ZERO;
                self.sleeping = true;
            }
        }
    }
}

/// Dust particle puff spawned along real architectural cut seams.
#[derive(Debug, Clone)]
pub struct CutDustPuff {
    pub position: Vec3,
    pub velocity: Vec3,
    pub size: f32,
    pub remaining_secs: f32,
}

/// Telemetry counters for PRD v3.0 debug overlay.
#[derive(Debug, Clone, Default)]
pub struct WorldCutTelemetry {
    pub successful_cuts: u32,
    pub failed_caps: u32,
    pub active_dynamic_bodies: usize,
    pub total_modules: usize,
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

/// Streamed spatial cell holding sliceable architectural modules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldCell {
    pub id: (i32, i32),
    pub district_name: String,
    pub bounds: Aabb,
    pub lod: LodLevel,
    pub modules: Vec<SliceableModule>,
    pub checkpoints: Vec<Checkpoint>,
    pub enemy_spawns: Vec<EnemySpawn>,
}

/// Manages active cells, real architectural mesh slicing, dynamic falling pieces,
/// cut-event history for save persistence, and dust VFX.
#[derive(Debug, Clone)]
pub struct WorldStreamer {
    pub cells: HashMap<(i32, i32), WorldCell>,
    pub active_cell: (i32, i32),
    pub lod0_radius: i32,
    pub lod1_radius: i32,
    pub next_module_id: u64,
    pub cut_history: Vec<CutPlane>,
    pub dust_puffs: Vec<CutDustPuff>,
    pub telemetry: WorldCutTelemetry,
    pub max_dynamic_bodies: usize,
}

impl WorldStreamer {
    pub fn world_to_cell(pos: Vec3) -> (i32, i32) {
        (
            (pos.x / CELL_SIZE_METERS).floor() as i32,
            (pos.z / CELL_SIZE_METERS).floor() as i32,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn push_sketch_house(
        modules: &mut Vec<SliceableModule>,
        next_id: &mut u64,
        cell_id: (i32, i32),
        min: Vec3,
        max: Vec3,
        roof_height: f32,
        wall_mat: MaterialKind,
        roof_mat: MaterialKind,
        add_chimney: bool,
        sliceable: bool,
    ) {
        let base_layers = CollisionLayer::STRUCTURAL
            .union(CollisionLayer::CLIMBABLE)
            .union(CollisionLayer::PAINTABLE);
        let layers = if sliceable {
            base_layers.union(CollisionLayer::SLICEABLE_WORLD)
        } else {
            base_layers
        };
        let cuts = if sliceable { 3 } else { 0 };

        // Main plaster building body
        Self::push_box_module(
            modules,
            next_id,
            cell_id,
            min,
            max,
            wall_mat,
            layers,
            cuts,
            min.y,
        );

        // Recessed dark charcoal windows on +Z and -Z facades
        let width = max.x - min.x;
        let height = max.y - min.y;
        let cols = ((width / 3.4).floor() as i32).clamp(1, 3);
        let rows = ((height / 3.8).floor() as i32).clamp(1, 3);
        for r in 0..rows {
            let wy = min.y + 2.2 + r as f32 * 3.4;
            if wy + 1.4 > max.y - 0.8 {
                continue;
            }
            for c in 0..cols {
                let wx = min.x + (c as f32 + 0.5) * (width / cols as f32);
                Self::push_box_module(
                    modules,
                    next_id,
                    cell_id,
                    Vec3::new(wx - 0.45, wy, max.z - 0.08),
                    Vec3::new(wx + 0.45, wy + 1.35, max.z + 0.08),
                    MaterialKind::InkWindowAccent,
                    CollisionLayer::NONE,
                    0,
                    min.y,
                );
                Self::push_box_module(
                    modules,
                    next_id,
                    cell_id,
                    Vec3::new(wx - 0.45, wy, min.z - 0.08),
                    Vec3::new(wx + 0.45, wy + 1.35, min.z + 0.08),
                    MaterialKind::InkWindowAccent,
                    CollisionLayer::NONE,
                    0,
                    min.y,
                );
            }
        }

        // Overhanging roof eaves
        let eave = 0.55;
        Self::push_box_module(
            modules,
            next_id,
            cell_id,
            Vec3::new(min.x - eave, max.y - 0.25, min.z - eave),
            Vec3::new(max.x + eave, max.y + 0.35, max.z + eave),
            roof_mat,
            layers,
            cuts,
            min.y,
        );

        // Pitched terracotta/orange roof pyramid (sliceable!)
        if roof_height > 0.2 {
            Self::push_pitched_roof_module(
                modules,
                next_id,
                cell_id,
                Aabb::new(
                    Vec3::new(min.x - eave, max.y + 0.35, min.z - eave),
                    Vec3::new(max.x + eave, max.y + 0.35 + roof_height, max.z + eave),
                ),
                roof_mat,
                layers,
                cuts,
                min.y,
            );
        }

        if add_chimney {
            let cx = max.x - 1.6;
            let cz = (min.z + max.z) * 0.5;
            Self::push_box_module(
                modules,
                next_id,
                cell_id,
                Vec3::new(cx - 0.55, max.y + 0.2, cz - 0.55),
                Vec3::new(cx + 0.55, max.y + roof_height * 0.85 + 1.6, cz + 0.55),
                MaterialKind::ChimneyStone,
                layers,
                cuts,
                min.y,
            );
        }
    }

    /// Push a sliceable box module and increment next_id
    #[inline]
    fn push_box_module(
        list: &mut Vec<SliceableModule>,
        next_id: &mut u64,
        cell_id: (i32, i32),
        min: Vec3,
        max: Vec3,
        mat: MaterialKind,
        layers: CollisionLayer,
        cuts: u8,
        floor_y: f32,
    ) {
        let id = *next_id;
        *next_id += 1;
        list.push(SliceableModule::new_box(
            id,
            cell_id,
            Aabb::new(min, max),
            mat,
            layers,
            cuts,
            floor_y,
        ));
    }

    /// Push a pitched roof module and increment next_id
    #[inline]
    fn push_pitched_roof_module(
        list: &mut Vec<SliceableModule>,
        next_id: &mut u64,
        cell_id: (i32, i32),
        bounds: Aabb,
        mat: MaterialKind,
        layers: CollisionLayer,
        cuts: u8,
        floor_y: f32,
    ) {
        let id = *next_id;
        *next_id += 1;
        list.push(SliceableModule::new_pitched_roof(
            id,
            cell_id,
            bounds,
            mat,
            layers,
            cuts,
            floor_y,
        ));
    }

    /// Builds the PRD v3.0 Vertical Slice District ("Inkwell Quarter") featuring the
    /// 3 reproducible world-slicing demonstrations right from spawn:
    /// - Demo A: Freestanding sliceable white plaster wall right in front of spawn (`z = -4.8`)
    /// - Demo B: Sliceable orange-roofed corner house (`x = -6.5, z = -10.5`) spanning wall + roof
    /// - Demo C: Tall stacked sliceable tower (`z = -24.0`) whose upper half & orange spire topple
    pub fn build_vertical_slice() -> (Self, Vec<PaintPlatformAnchor>, Vec<PaintGlyphPuzzle>) {
        let mut cells = HashMap::new();
        let mut next_id = 1_u64;

        let safe_spawn_floor = CollisionLayer::STRUCTURAL
            .union(CollisionLayer::CLIMBABLE)
            .union(CollisionLayer::PAINTABLE);
        let sliceable_arch = safe_spawn_floor.union(CollisionLayer::SLICEABLE_WORLD);
        let sliceable_wallrun = sliceable_arch.union(CollisionLayer::WALL_RUNNABLE);
        let sliceable_prop = CollisionLayer::SLASHABLE_PROP.union(CollisionLayer::SLICEABLE_WORLD);

        // --- Cell (0, 0): Starting Terrace + Demos A, B, C + Parkour Alley ---
        let mut c00 = Vec::new();
        // Essential spawn terrace floor (explicitly kept intact to prevent spawn soft-lock)
        Self::push_box_module(&mut c00, &mut next_id, (0, 0), Vec3::new(-10.0, 0.0, -10.0), Vec3::new(10.0, 10.6, 10.0), MaterialKind::WhitePlasterWall, safe_spawn_floor, 0, 0.0);
        Self::push_box_module(&mut c00, &mut next_id, (0, 0), Vec3::new(-10.4, 10.6, -10.4), Vec3::new(10.4, 11.2, 10.4), MaterialKind::OrangeTiledRoof, safe_spawn_floor, 0, 0.0);

        // DEMO A (Gate 1): Freestanding sliceable white plaster wall right in front of the player!
        Self::push_box_module(
            &mut c00,
            &mut next_id,
            (0, 0),
            Vec3::new(-3.2, 11.2, -5.2),
            Vec3::new(2.2, 15.6, -4.2),
            MaterialKind::WhitePlasterWall,
            sliceable_arch,
            3,
            11.2,
        );

        // Sliceable wooden/ink crate right beside Demo A for quick melee/cut testing
        Self::push_box_module(
            &mut c00,
            &mut next_id,
            (0, 0),
            Vec3::new(3.5, 11.2, -4.4),
            Vec3::new(4.9, 12.7, -3.0),
            MaterialKind::SlashableProp,
            sliceable_prop,
            2,
            11.2,
        );

        // DEMO B (Gate 2): Sliceable white plaster wall + bright orange roof overhang on left flank!
        Self::push_box_module(
            &mut c00,
            &mut next_id,
            (0, 0),
            Vec3::new(-9.6, 11.2, -9.2),
            Vec3::new(-5.8, 14.6, -4.6),
            MaterialKind::WarmPlasterWall,
            sliceable_arch,
            3,
            11.2,
        );
        Self::push_pitched_roof_module(
            &mut c00,
            &mut next_id,
            (0, 0),
            Aabb::new(Vec3::new(-10.1, 14.6, -9.7), Vec3::new(-5.3, 17.4, -4.1)),
            MaterialKind::OrangeTiledRoof,
            sliceable_arch,
            3,
            11.2,
        );

        // Rooftop Bridge 1 -> Courtyard Roof
        Self::push_box_module(&mut c00, &mut next_id, (0, 0), Vec3::new(-6.5, 0.0, -25.0), Vec3::new(6.5, 11.6, -12.5), MaterialKind::WarmPlasterWall, sliceable_arch, 2, 0.0);
        Self::push_box_module(&mut c00, &mut next_id, (0, 0), Vec3::new(-7.0, 11.6, -25.5), Vec3::new(7.0, 12.4, -12.0), MaterialKind::OrangeTiledRoof, sliceable_arch, 2, 0.0);

        // DEMO C (Gate 3): Tall modular sliceable tower right on the Courtyard Roof!
        Self::push_box_module(
            &mut c00,
            &mut next_id,
            (0, 0),
            Vec3::new(-2.4, 12.4, -22.5),
            Vec3::new(2.4, 18.8, -18.0),
            MaterialKind::WhitePlasterWall,
            sliceable_arch,
            3,
            12.4,
        );
        Self::push_pitched_roof_module(
            &mut c00,
            &mut next_id,
            (0, 0),
            Aabb::new(Vec3::new(-2.9, 18.8, -23.0), Vec3::new(2.9, 23.2, -17.5)),
            MaterialKind::OrangeTiledRoof,
            sliceable_arch,
            3,
            12.4,
        );

        // Tall Sliceable Wall-Run Billboard Wall on +X flank
        Self::push_box_module(&mut c00, &mut next_id, (0, 0), Vec3::new(7.0, 10.0, -39.0), Vec3::new(9.2, 19.5, -22.0), MaterialKind::WhitePlasterWall, sliceable_wallrun, 3, 10.0);

        // Second stepped rooftop (mantle & slide obstacle)
        Self::push_box_module(&mut c00, &mut next_id, (0, 0), Vec3::new(-7.5, 0.0, -45.5), Vec3::new(7.5, 12.8, -33.5), MaterialKind::WhitePlasterWall, sliceable_arch, 2, 0.0);
        Self::push_box_module(&mut c00, &mut next_id, (0, 0), Vec3::new(-8.0, 12.8, -46.0), Vec3::new(8.0, 13.6, -33.0), MaterialKind::OrangeTiledRoof, sliceable_arch, 2, 0.0);
        // Sliceable stone archway beam (can slide under OR slice in half!)
        Self::push_box_module(&mut c00, &mut next_id, (0, 0), Vec3::new(-7.0, 14.9, -40.2), Vec3::new(7.0, 16.4, -38.8), MaterialKind::StoneTrim, sliceable_arch, 3, 13.6);

        // Dense flanking medieval sketchbook houses (all sliceable!)
        Self::push_sketch_house(&mut c00, &mut next_id, (0, 0), Vec3::new(-24.0, 0.0, -14.0), Vec3::new(-12.0, 15.5, 4.0), 5.2, MaterialKind::WhitePlasterWall, MaterialKind::OrangeTiledRoof, true, true);
        Self::push_sketch_house(&mut c00, &mut next_id, (0, 0), Vec3::new(12.0, 0.0, -16.0), Vec3::new(24.5, 16.5, 2.0), 5.8, MaterialKind::WarmPlasterWall, MaterialKind::OrangeTiledRoof, true, true);
        Self::push_sketch_house(&mut c00, &mut next_id, (0, 0), Vec3::new(-24.5, 0.0, -36.0), Vec3::new(-11.5, 17.0, -18.0), 6.0, MaterialKind::WarmPlasterWall, MaterialKind::OrangeTiledRoof, true, true);
        Self::push_sketch_house(&mut c00, &mut next_id, (0, 0), Vec3::new(12.0, 0.0, -42.0), Vec3::new(25.0, 18.0, -24.0), 6.2, MaterialKind::WhitePlasterWall, MaterialKind::SlateRoofAccent, true, true);

        cells.insert(
            (0, 0),
            WorldCell {
                id: (0, 0),
                district_name: "Inkwell Quarter — Slicing Terrace & Alley".to_string(),
                bounds: Aabb::new(Vec3::new(-48.0, -15.0, -48.0), Vec3::new(48.0, 60.0, 48.0)),
                lod: LodLevel::Lod0Full,
                modules: c00,
                checkpoints: vec![Checkpoint {
                    id: "cp_tutorial_start".to_string(),
                    position: Vec3::new(0.0, 11.4, 3.5),
                    cell_id: (0, 0),
                }],
                enemy_spawns: vec![EnemySpawn {
                    id: 9000,
                    position: Vec3::new(4.8, 11.2, -6.5),
                    patrol_a: Vec3::new(3.2, 11.2, -6.5),
                    patrol_b: Vec3::new(6.8, 11.2, -6.5),
                    cell_id: (0, 0),
                }],
            },
        );

        // --- Cell (0, -1): Combat Plaza, Paint Puzzle & Tall Spire Skyline ---
        let mut c01 = Vec::new();
        Self::push_box_module(&mut c01, &mut next_id, (0, -1), Vec3::new(-16.0, 0.0, -76.0), Vec3::new(16.0, 12.6, -52.0), MaterialKind::WhitePlasterWall, safe_spawn_floor, 0, 0.0);
        Self::push_box_module(&mut c01, &mut next_id, (0, -1), Vec3::new(-16.6, 12.6, -76.6), Vec3::new(16.6, 13.5, -51.4), MaterialKind::OrangeTiledRoof, safe_spawn_floor, 0, 0.0);

        // Tall sliceable Landmark Orange Spire Tower & flanking skyline
        Self::push_sketch_house(&mut c01, &mut next_id, (0, -1), Vec3::new(-5.5, 0.0, -94.0), Vec3::new(5.5, 34.0, -83.0), 12.0, MaterialKind::WhitePlasterWall, MaterialKind::OrangeTiledRoof, false, true);
        Self::push_sketch_house(&mut c01, &mut next_id, (0, -1), Vec3::new(-28.0, 0.0, -86.0), Vec3::new(-16.0, 24.0, -70.0), 7.5, MaterialKind::WarmPlasterWall, MaterialKind::OrangeTiledRoof, true, true);
        Self::push_sketch_house(&mut c01, &mut next_id, (0, -1), Vec3::new(18.0, 0.0, -88.0), Vec3::new(30.0, 27.0, -72.0), 8.5, MaterialKind::WhitePlasterWall, MaterialKind::OrangeTiledRoof, true, true);
        Self::push_sketch_house(&mut c01, &mut next_id, (0, -1), Vec3::new(-18.0, 0.0, -112.0), Vec3::new(-6.0, 30.0, -98.0), 9.0, MaterialKind::StoneTrim, MaterialKind::OrangeTiledRoof, false, false);
        Self::push_sketch_house(&mut c01, &mut next_id, (0, -1), Vec3::new(10.0, 0.0, -114.0), Vec3::new(22.0, 32.0, -100.0), 9.5, MaterialKind::StoneTrim, MaterialKind::SlateRoofAccent, false, false);

        cells.insert(
            (0, -1),
            WorldCell {
                id: (0, -1),
                district_name: "Inkwell Quarter — Combat Plaza & Spire".to_string(),
                bounds: Aabb::new(Vec3::new(-48.0, -15.0, -96.0), Vec3::new(48.0, 60.0, -48.0)),
                lod: LodLevel::Lod0Full,
                modules: c01,
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
            active_cell: (0, 0),
            lod0_radius: 1,
            lod1_radius: 2,
            next_module_id: next_id,
            cut_history: Vec::new(),
            dust_puffs: Vec::new(),
            telemetry: WorldCutTelemetry::default(),
            max_dynamic_bodies: 28,
        };
        (streamer, anchors, puzzles)
    }

    /// Executes a REAL 3D plane slice across every intersected `SLICEABLE_WORLD` module
    /// in active cells (PRD v3.0 Gates 1, 2, 3).
    ///
    /// Returns the number of architectural modules physically split into capped halves.
    pub fn apply_world_cut(&mut self, plane: CutPlane, record_history: bool) -> usize {
        let mut total_split = 0usize;
        let mut next_id = self.next_module_id;
        let mut spawned_dust = Vec::new();

        for cell in self.cells.values_mut() {
            if cell.lod != LodLevel::Lod0Full {
                continue;
            }
            let mut replacement_list = Vec::with_capacity(cell.modules.len() + 8);
            let old_modules = std::mem::take(&mut cell.modules);

            for m in old_modules {
                let wb = m.world_bounds();
                let eligible = m.layers.contains(CollisionLayer::SLICEABLE_WORLD)
                    && m.remaining_cuts > 0
                    && plane.intersects_aabb(&wb);

                if !eligible {
                    replacement_list.push(m);
                    continue;
                }

                // Transform cut plane into module's local space if it has already moved
                let local_plane = CutPlane {
                    origin: plane.origin - m.offset,
                    normal: plane.normal,
                    forward: plane.forward,
                    range: plane.range,
                    half_width: plane.half_width,
                    half_height: plane.half_height,
                };

                let cap_col = m.material.cut_core_color();
                match m.mesh.slice(&local_plane, cap_col) {
                    Some((pos_mesh, neg_mesh)) => {
                        let Some(pos_b) = pos_mesh.compute_bounds() else {
                            self.telemetry.failed_caps += 1;
                            replacement_list.push(m);
                            continue;
                        };
                        let Some(neg_b) = neg_mesh.compute_bounds() else {
                            self.telemetry.failed_caps += 1;
                            replacement_list.push(m);
                            continue;
                        };

                        let pos_c = pos_mesh.centroid();
                        let neg_c = neg_mesh.centroid();

                        // Support-graph rule: the higher / non-grounded half detaches and falls!
                        let pos_is_upper = pos_c.y >= neg_c.y;
                        let pos_dynamic = m.is_dynamic || pos_is_upper || (pos_b.min.y + m.offset.y) > m.floor_y + 0.35;
                        let neg_dynamic = m.is_dynamic || !pos_is_upper && (neg_b.min.y + m.offset.y) > m.floor_y + 0.35;

                        let impulse_dir = plane.normal.normalize_or_zero();
                        let forward_push = plane.forward.normalize_or_zero() * 2.2;

                        let make_child = |id: u64,
                                          mesh: SliceableMesh,
                                          bounds: Aabb,
                                          is_dyn: bool,
                                          sign: f32| {
                            let vol = mesh.approx_volume();
                            let rubble = if vol < 0.8 { Some(12.0) } else { None };
                            SliceableModule {
                                id,
                                cell_id: m.cell_id,
                                material: m.material,
                                mesh,
                                bounds,
                                layers: m.layers,
                                remaining_cuts: m.remaining_cuts.saturating_sub(1),
                                floor_y: m.floor_y,
                                is_dynamic: is_dyn,
                                offset: m.offset,
                                velocity: Vec3::ZERO,
                                rotation: m.rotation,
                                angular_vel: Vec3::ZERO,
                                mass: (vol * 110.0).clamp(10.0, 3000.0),
                                // Brief 0.14s suspense beat before cut halves open & slide!
                                suspense_timer: 0.14,
                                pending_impulse: if is_dyn {
                                    impulse_dir * (2.6 * sign) + forward_push + Vec3::Y * 1.4
                                } else {
                                    Vec3::ZERO
                                },
                                pending_ang_vel: if is_dyn {
                                    Vec3::new(0.45 * sign, 0.25, -0.55 * sign)
                                } else {
                                    Vec3::ZERO
                                },
                                cut_glow_timer: 0.65,
                                sleeping: false,
                                rubble_lifetime: rubble,
                            }
                        };

                        let id_a = next_id;
                        let id_b = next_id + 1;
                        next_id += 2;

                        replacement_list.push(make_child(id_a, pos_mesh, pos_b, pos_dynamic, 1.0));
                        replacement_list.push(make_child(id_b, neg_mesh, neg_b, neg_dynamic, -1.0));

                        // Spawn cut-seam dust puffs
                        let seam_mid = (pos_c + neg_c) * 0.5 + m.offset;
                        for k in 0..8 {
                            let a = (k as f32 / 8.0) * std::f32::consts::TAU;
                            spawned_dust.push(CutDustPuff {
                                position: seam_mid + Vec3::new(a.cos() * 0.6, (k % 3) as f32 * 0.25, a.sin() * 0.6),
                                velocity: Vec3::new(a.cos() * 2.2, 1.8 + (k % 2) as f32, a.sin() * 2.2),
                                size: 0.22,
                                remaining_secs: 0.75,
                            });
                        }

                        total_split += 1;
                        self.telemetry.successful_cuts += 1;
                    }
                    None => {
                        replacement_list.push(m);
                    }
                }
            }

            cell.modules = replacement_list;
        }

        self.next_module_id = next_id;
        self.dust_puffs.extend(spawned_dust);
        if total_split > 0 && record_history {
            self.cut_history.push(plane);
        }
        total_split
    }

    /// Replays persisted cut events on reload (PRD v3.0 Gate 9).
    pub fn replay_cut_history(&mut self, cuts: &[CutPlane]) {
        for &plane in cuts {
            self.apply_world_cut(plane, true);
        }
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
        let mut active_dyn = 0usize;
        let mut total_mods = 0usize;
        for cell in self.cells.values_mut() {
            for m in &mut cell.modules {
                m.tick_physics(dt);
                if m.is_dynamic && !m.sleeping {
                    active_dyn += 1;
                }
            }
            // Evict expired tiny rubble
            cell.modules
                .retain(|m| m.rubble_lifetime.map_or(true, |t| t > 0.0));
            total_mods += cell.modules.len();
        }
        self.telemetry.active_dynamic_bodies = active_dyn;
        self.telemetry.total_modules = total_mods;

        for p in &mut self.dust_puffs {
            p.position += p.velocity * dt;
            p.velocity *= (1.0 - 2.5 * dt).max(0.0);
            p.remaining_secs -= dt;
        }
        self.dust_puffs.retain(|p| p.remaining_secs > 0.0);
    }

    pub fn collect_colliders(&self) -> Vec<Collider> {
        let mut out = Vec::new();
        for cell in self.cells.values() {
            if cell.lod != LodLevel::Lod0Full {
                continue;
            }
            for m in &cell.modules {
                if m.layers == CollisionLayer::NONE {
                    continue;
                }
                out.push(m.collider());
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
    fn gate1_and_gate2_world_cut_splits_wall_and_orange_roof_with_closed_caps_and_gravity() {
        let (mut streamer, _, _) = WorldStreamer::build_vertical_slice();

        // Cut horizontally/diagonally through Demo A freestanding wall AND Demo B wall+roof
        let cut = CutPlane::new(
            Vec3::new(0.0, 13.5, 2.0),
            Vec3::new(0.20, 0.98, 0.0),
            Vec3::NEG_Z,
            20.0,
            12.0,
        );
        let split_count = streamer.apply_world_cut(cut, true);
        assert!(split_count >= 2, "Expected both Demo A wall and Demo B roof/wall to split");

        // Advance past 0.14s suspense beat + 1.0s of gravity
        for _ in 0..80 {
            streamer.tick_props(1.0 / 60.0);
        }
        assert!(
            streamer.cells[&(0, 0)]
                .modules
                .iter()
                .any(|m| m.is_dynamic && m.offset.length() > 0.2),
            "Detached upper half must physically separate and fall"
        );
    }

    #[test]
    fn gate3_repeat_cut_splits_already_sliced_piece() {
        let (mut streamer, _, _) = WorldStreamer::build_vertical_slice();
        let cut1 = CutPlane::new(
            Vec3::new(0.0, 13.4, 2.0),
            Vec3::Y,
            Vec3::NEG_Z,
            12.0,
            5.0,
        );
        assert!(streamer.apply_world_cut(cut1, true) >= 1);

        // Second perpendicular cut on the remaining halves
        let cut2 = CutPlane::new(
            Vec3::new(-0.5, 13.4, 2.0),
            Vec3::X,
            Vec3::NEG_Z,
            12.0,
            5.0,
        );
        assert!(streamer.apply_world_cut(cut2, true) >= 2);
    }
}
