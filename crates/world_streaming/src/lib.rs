//! Scalable grid/cell world streaming + REAL sliceable architectural modules,
//! support graph, dynamic falling fragments, and cut persistence (PRD v3.0).

pub mod campaign;
pub use campaign::{atmosphere, level_theme, story_beat, CAMPAIGN_LEVELS};

use engine_core::mesh_slicer::{CutPlane, SliceableMesh};
use engine_core::{Aabb, Collider, CollisionLayer, Palette};
use gameplay_paint::{PaintGlyphPuzzle, PaintPlatformAnchor};
use glam::{Quat, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

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
        // Use authored 120-triangle watertight roof mesh (OBJ), not a 6-tri pyramid.
        let mesh = SliceableMesh::from_obj_scaled(
            include_str!("../../../assets/models/terracotta_roof.obj"), bounds,
            material.exterior_color(),
        ).unwrap_or_else(|_| SliceableMesh::from_pitched_roof(bounds, material.exterior_color()));
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

    /// Rotated world-space AABB. The previous implementation ignored angular
    /// tumble, causing large sliced pieces to clip through buildings.
    pub fn world_bounds(&self) -> Aabb {
        let c = self.bounds.center() + self.offset;
        let h = (self.bounds.max - self.bounds.min) * 0.5;
        let x = self.rotation * Vec3::X;
        let y = self.rotation * Vec3::Y;
        let z = self.rotation * Vec3::Z;
        let extents = x.abs() * h.x + y.abs() * h.y + z.abs() * h.z;
        Aabb::from_center_half_extents(c, extents)
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
    /// Dynamic architectural pieces that struck a surface during the latest physics step.
    pub landed_fragments_this_tick: u32,
    /// Level metadata. A level always regenerates from its original seed.
    pub level_number: u32,
    pub level_seed: u64,
    pub level_start: Vec3,
    pub level_exit: Vec3,
    /// Ordered safe landing targets for deterministic route progress and tuning.
    pub route_roofs: Vec<Vec3>,
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

        // Low-poly architectural detailing gives every house readable scale:
        // door surround, lintel, corner stone and a narrow facade cornice.
        // Decorative pieces have no collision so they cannot snag parkour movement.
        let facade_z = max.z + 0.095;
        let cx = (min.x + max.x) * 0.5;
        if height > 4.6 {
            Self::push_box_module(modules, next_id, cell_id,
                Vec3::new(cx - 0.82, min.y + 0.06, facade_z - 0.04),
                Vec3::new(cx + 0.82, min.y + 2.45, facade_z + 0.12),
                MaterialKind::InkWindowAccent, CollisionLayer::NONE, 0, min.y);
            Self::push_box_module(modules, next_id, cell_id,
                Vec3::new(cx - 1.18, min.y + 2.36, facade_z - 0.08),
                Vec3::new(cx + 1.18, min.y + 2.66, facade_z + 0.18),
                MaterialKind::StoneTrim, CollisionLayer::NONE, 0, min.y);
            Self::push_box_module(modules, next_id, cell_id,
                Vec3::new(min.x - 0.1, max.y - 1.13, max.z),
                Vec3::new(max.x + 0.1, max.y - 0.82, max.z + 0.32),
                MaterialKind::StoneTrim, CollisionLayer::NONE, 0, min.y);
            for xx in [min.x, max.x - 0.24] {
                Self::push_box_module(modules, next_id, cell_id,
                    Vec3::new(xx, min.y, max.z - 0.09),
                    Vec3::new(xx + 0.24, max.y, max.z + 0.12),
                    MaterialKind::StoneTrim, CollisionLayer::NONE, 0, min.y);
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

    /// An authored, watertight tower mesh with 3D parapets, fully cuttable.
    fn push_authored_tower(
        list: &mut Vec<SliceableModule>, next_id: &mut u64,
        cell_id: (i32, i32), bounds: Aabb, layers: CollisionLayer,
        cuts: u8, floor_y: f32,
    ) {
        let id = *next_id;
        *next_id += 1;
        let mat = MaterialKind::StoneTrim;
        let mut piece = SliceableModule::new_box(id, cell_id, bounds, mat, layers, cuts, floor_y);
        if let Ok(mesh) = SliceableMesh::from_obj_scaled(
            include_str!("../../../assets/models/sandstone_tower.obj"), bounds, mat.exterior_color(),
        ) {
            piece.mass = (mesh.approx_volume() * 130.0).clamp(15.0,4000.0);
            piece.mesh = mesh;
        }
        list.push(piece);
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
        Self::push_box_module(&mut c00, &mut next_id, (0, 0), Vec3::new(-10.4, 10.6, -10.4), Vec3::new(10.4, 11.2, 10.4), MaterialKind::SlateRoofAccent, safe_spawn_floor, 0, 0.0);

        // Cut-training wall on the LEFT edge, not in the center of the running lane.
        // The old block (-3.2..2.2) obstructed the camera and ruined the sightline.
        Self::push_box_module(
            &mut c00,
            &mut next_id,
            (0, 0),
            Vec3::new(-8.7, 11.2, -5.2),
            Vec3::new(-5.6, 15.6, -4.2),
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

        // The 8 m gap between the first and second rooftops is now a readable
        // hop / hook decision: two masonry kiosks at different heights, not floating cubes.
        for (x, z, h) in [(-4.2, -29.7, 12.0), (3.4, -31.5, 12.5)] {
            let footprint = Vec3::new(x, 0.0, z);
            Self::push_sketch_house(&mut c00, &mut next_id, (0, 0),
                footprint, Vec3::new(x + 3.5, h, z + 3.0), 0.0,
                MaterialKind::WarmPlasterWall, MaterialKind::OrangeTiledRoof, false, true);
        }

        // DEMO C (Gate 3): Tall modular sliceable tower right on the Courtyard Roof!
        Self::push_authored_tower(
            &mut c00, &mut next_id, (0, 0),
            Aabb::new(Vec3::new(-2.4, 12.4, -22.5), Vec3::new(2.4, 18.8, -18.0)),
            sliceable_arch, 3, 12.4,
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

        // Wall-run facade with a grounded foundation and roof trim, NOT a floating billboard.
        Self::push_box_module(&mut c00, &mut next_id, (0, 0), Vec3::new(10.8, 0.0, -39.0),
            Vec3::new(13.2, 18.0, -23.0), MaterialKind::WarmPlasterWall, sliceable_wallrun, 3, 0.0);
        Self::push_box_module(&mut c00, &mut next_id, (0, 0), Vec3::new(10.4, 18.0, -39.4),
            Vec3::new(13.6, 18.7, -22.6), MaterialKind::OrangeTiledRoof, sliceable_arch, 2, 0.0);

        // Second stepped rooftop (mantle & slide obstacle)
        Self::push_box_module(&mut c00, &mut next_id, (0, 0), Vec3::new(-7.5, 0.0, -45.5), Vec3::new(7.5, 12.8, -33.5), MaterialKind::WhitePlasterWall, sliceable_arch, 2, 0.0);
        Self::push_box_module(&mut c00, &mut next_id, (0, 0), Vec3::new(-8.0, 12.8, -46.0), Vec3::new(8.0, 13.6, -33.0), MaterialKind::OrangeTiledRoof, sliceable_arch, 2, 0.0);
        // Stone archway has TWO supports: no more midair beam.
        Self::push_box_module(&mut c00, &mut next_id, (0, 0), Vec3::new(-7.0, 13.6, -40.2),
            Vec3::new(-5.8, 15.1, -38.8), MaterialKind::StoneTrim, sliceable_arch, 3, 13.6);
        Self::push_box_module(&mut c00, &mut next_id, (0, 0), Vec3::new(5.8, 13.6, -40.2),
            Vec3::new(7.0, 15.1, -38.8), MaterialKind::StoneTrim, sliceable_arch, 3, 13.6);
        // Sliceable stone archway beam (can slide under OR slice in half!)
        Self::push_box_module(&mut c00, &mut next_id, (0, 0), Vec3::new(-7.0, 14.9, -40.2), Vec3::new(7.0, 16.4, -38.8), MaterialKind::StoneTrim, sliceable_arch, 3, 13.6);

        // Coherent neighborhood blocks: 5 lots per side, consistent streets,
        // staggered roof heights, no intersections with the main parkour corridor.
        // Opposite sides alternate height to guide the eye toward the distant spire.
        for (row, z) in [-43.5_f32, -27.5, -11.5, 4.5, 20.5].iter().enumerate() {
            for side in [-1.0_f32, 1.0] {
                let h = 12.8 + ((row * 7 + if side < 0.0 { 3 } else { 0 }) % 5) as f32 * 2.1;
                let min_x = if side < 0.0 { -26.5 } else { 15.0 };
                let max_x = if side < 0.0 { -15.0 } else { 26.5 };
                let wall = if (row + if side < 0.0 { 1 } else { 0 }) % 3 == 0 {
                    MaterialKind::StoneTrim
                } else { MaterialKind::WarmPlasterWall };
                Self::push_sketch_house(&mut c00, &mut next_id, (0, 0),
                    Vec3::new(min_x, 0.0, *z), Vec3::new(max_x, h, *z + 11.8),
                    2.1 + row as f32 * 0.38, wall, MaterialKind::OrangeTiledRoof,
                    row % 2 == 0, true);
            }
        }
        // Background skyline is set back behind a visible street; these also provide
        // longer-range anchors and height layers without overlapping the foreground.
        for (side, row) in [(-1.0_f32, 0usize), (1.0, 1), (-1.0, 2), (1.0, 3)] {
            let z = -40.0 + row as f32 * 20.0;
            let min_x = if side < 0.0 { -44.0 } else { 31.0 };
            Self::push_sketch_house(&mut c00, &mut next_id, (0, 0),
                Vec3::new(min_x, 0.0, z), Vec3::new(min_x + 11.0, 20.0 + row as f32 * 1.6, z + 12.0),
                3.3, MaterialKind::StoneTrim, MaterialKind::OrangeTiledRoof,
                row % 2 == 0, true);
        }

        // Visible, nonblocking shrines show where progress is saved.
        for (x, y, z) in [(0.0, 11.2, 3.5), (0.0, 13.6, -36.0)] {
            Self::push_box_module(&mut c00, &mut next_id, (0, 0),
                Vec3::new(x - 0.65, y, z - 0.65), Vec3::new(x + 0.65, y + 0.07, z + 0.65),
                MaterialKind::CheckpointShrine, CollisionLayer::NONE, 0, 0.0);
        }
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
                }, Checkpoint {
                    id: "cp_rooftop_alley".to_string(),
                    position: Vec3::new(0.0, 13.8, -36.0),
                    cell_id: (0, -1),
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

        // End-of-level beacon in the combat plaza, reachable without fighting.
        Self::push_box_module(&mut c01, &mut next_id, (0, -1),
            Vec3::new(-0.85, 13.5, -69.85), Vec3::new(0.85, 15.8, -68.15),
            MaterialKind::CheckpointShrine, CollisionLayer::NONE, 0, 0.0);
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
            landed_fragments_this_tick: 0,
            level_number: 1,
            level_seed: 0,
            level_start: Vec3::new(0.0, 11.4, 3.0),
            level_exit: Vec3::new(0.0, 13.8, -69.0),
            route_roofs: vec![Vec3::new(0.0, 11.4, 3.0), Vec3::new(0.0, 13.8, -69.0)],
        };
        (streamer, anchors, puzzles)
    }

    /// Build the authored introduction or a seeded, repeatable procedural rooftop course.
    /// Level 2+ roofs are generated on a navigable spine with a bounded horizontal
    /// offset, small jump gaps, and usable side facades for brush-web anchors.
    pub fn build_level(level_number: u32, seed: u64)
        -> (Self, Vec<PaintPlatformAnchor>, Vec<PaintGlyphPuzzle>) {
        if level_number <= 1 { return Self::build_vertical_slice(); }
        Self::build_generated_level(level_number.min(CAMPAIGN_LEVELS), seed)
    }

    /// No network or random crate: this tiny PRNG produces the same district
    /// for the same seed on every Android/desktop build.
    fn seeded_unit(seed: &mut u64) -> f32 {
        *seed ^= *seed >> 12;
        *seed ^= *seed << 25;
        *seed ^= *seed >> 27;
        let n = seed.wrapping_mul(2685821657736338717);
        ((n >> 40) as u32 as f32) / 16_777_216.0
    }

    fn build_generated_level(level_number: u32, seed: u64)
        -> (Self, Vec<PaintPlatformAnchor>, Vec<PaintGlyphPuzzle>) {
        let mut rng = seed ^ (level_number as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let theme = level_theme(level_number);
        let title = theme.title;
        let chapter = theme.chapter;
        let difficulty = theme.difficulty as f32 / 13.0;
        if rng == 0 { rng = 0xA5C3_F15E_2937_BA61; }
        let mut ids = 1u64;
        let mut modules = Vec::new();
        let mut checkpoints = Vec::new();
        let mut enemies = Vec::new();
        let mut paint_anchors = Vec::new();
        let mut route_roofs = Vec::new();
        let floor = CollisionLayer::STRUCTURAL.union(CollisionLayer::CLIMBABLE)
            .union(CollisionLayer::PAINTABLE);
        let sliceable = floor.union(CollisionLayer::SLICEABLE_WORLD);
        let hook_wall = sliceable.union(CollisionLayer::WALL_RUNNABLE);
        // 14-36 platform sequence (~200-600m). Difficulty increases by region,
        // not merely by adding random unreachable skyscrapers.
        let count = (14 + theme.chapter * 3 + theme.act * 2).min(36);
        let checkpoint_stride = if theme.difficulty < 5 { 4 } else { 5 };
        let roof_half_width = (5.1 - difficulty * 1.0).max(4.1);
        let roof_half_depth = (5.2 - difficulty * 0.8).max(4.4);
        let mut x = 0.0f32;
        let mut z = 3.0f32;
        let mut height = 11.2f32;
        let mut last_center = Vec3::new(x, height + 0.2, z);
        let start = last_center;

        for i in 0..count {
            if i > 0 {
                let side_jitter = (Self::seeded_unit(&mut rng) - 0.5) * (3.8 + 4.2 * difficulty);
                x = (x + side_jitter).clamp(-12.0 - 3.0*difficulty, 12.0 + 3.0*difficulty);
                // Tougher courses have longer jumps and occasional web-only traversals.
                // End of each checkpoint sector stays manageable for recovery.
                let web_section = i % 7 == 5 && theme.difficulty >= 3;
                let step = if web_section { 19.6 + difficulty * 2.0 }
                    else { 12.9 + difficulty * 4.3 + Self::seeded_unit(&mut rng) * 1.5 };
                z -= step;
                height = (height + (Self::seeded_unit(&mut rng) - 0.5) * (2.2 + 3.2 * difficulty))
                    .clamp(10.0, 20.0 + 8.0 * difficulty);
            }
            // Four-metre roofs and longer jump gaps reward sprint, air control and
            // brush-web timing; protected checkpoint pads prevent unwinnable restarts.
            let cell = Self::world_to_cell(Vec3::new(x, 0.0, z));
            // Checkpoint floors are protected from slicing to avoid respawn soft-locks.
            // All surrounding façades, walls, towers and ordinary rooftops are cuttable.
            let protected_cp = i == 0 || (i % checkpoint_stride == 0 && i + 1 < count);
            let roof_layers = if protected_cp { floor } else { sliceable };
            let body_layers = if protected_cp { floor } else { sliceable };
            let wall = match chapter {
                2 | 3 | 5 => if i % 3 == 0 { MaterialKind::StoneTrim } else { MaterialKind::WarmPlasterWall },
                4 => if i % 2 == 0 { MaterialKind::SlateRoofAccent } else { MaterialKind::StoneTrim },
                _ => if (i + level_number as usize) % 3 == 0 { MaterialKind::StoneTrim }
                    else { MaterialKind::WarmPlasterWall },
            };
            Self::push_box_module(&mut modules, &mut ids, cell,
                Vec3::new(x - roof_half_width, 0.0, z - roof_half_depth),
                Vec3::new(x + roof_half_width, height - 0.6, z + roof_half_depth),
                wall, body_layers.union(CollisionLayer::WALL_RUNNABLE),
                if protected_cp { 0 } else { 3 }, 0.0);
            Self::push_box_module(&mut modules, &mut ids, cell,
                Vec3::new(x - roof_half_width - 0.28, height - 0.6, z - roof_half_depth - 0.28),
                Vec3::new(x + roof_half_width + 0.28, height, z + roof_half_depth + 0.28),
                if chapter == 3 || i % 4 == 0 { MaterialKind::SlateRoofAccent }
                    else { MaterialKind::OrangeTiledRoof },
                roof_layers, if protected_cp { 0 } else { 3 }, 0.0);
            // Winter produces thin painted snow caps (visual-only so collision stays
            // consistent); the day/night shader tints them with ambient moonlight.
            if chapter == 3 && i % 2 == 0 {
                Self::push_box_module(&mut modules, &mut ids, cell,
                    Vec3::new(x - roof_half_width + 0.45, height + 0.012, z - roof_half_depth + 0.55),
                    Vec3::new(x + roof_half_width - 0.55, height + 0.065, z - roof_half_depth + 1.3),
                    MaterialKind::WhitePlasterWall, CollisionLayer::NONE, 0, height);
            }
            // Decorative roof coping is deliberately non-solid; parkour collision stays clean.
            for side in [-1.0f32, 1.0] {
                Self::push_box_module(&mut modules, &mut ids, cell,
                    Vec3::new(x + side * roof_half_width - 0.18, height, z - roof_half_depth),
                    Vec3::new(x + side * roof_half_width + 0.18, height + 0.38, z + roof_half_depth),
                    MaterialKind::StoneTrim, CollisionLayer::NONE, 0, 0.0);
            }
            // A hookable masonry tower at every second roof; never floating over the path.
            if i > 0 && (i % 2 == 0 || (i % 7 == 5 && theme.difficulty >= 3)) {
                let side = if i % 4 == 0 { 1.0 } else { -1.0 };
                let tx = x + side * (roof_half_width + 4.0);
                Self::push_box_module(&mut modules, &mut ids, cell,
                    Vec3::new(tx - 2.0, 0.0, z - 2.8),
                    Vec3::new(tx + 2.0, height + 8.0, z + 2.8),
                    MaterialKind::StoneTrim, hook_wall, 3, 0.0);
                Self::push_pitched_roof_module(&mut modules, &mut ids, cell,
                    Aabb::new(Vec3::new(tx - 2.5, height + 8.0, z - 3.3),
                        Vec3::new(tx + 2.5, height + 11.0, z + 3.3)),
                    MaterialKind::OrangeTiledRoof, sliceable, 3, 0.0);
            }
            // Unique side skylines per region; none obstruct the playable lane.
            if i % 2 == 1 {
                let side = if i % 4 == 1 { -1.0 } else { 1.0 };
                let bx = x + side * 24.0;
                let bh = height + 3.0 + Self::seeded_unit(&mut rng) * (8.0 + chapter as f32 * 1.7);
                Self::push_sketch_house(&mut modules, &mut ids, cell,
                    Vec3::new(bx - 5.3, 0.0, z - 5.0),
                    Vec3::new(bx + 5.3, bh, z + 5.0), 2.7,
                    if chapter == 2 || chapter == 3 { MaterialKind::StoneTrim } else { MaterialKind::WarmPlasterWall },
                    if chapter == 3 || chapter == 5 { MaterialKind::SlateRoofAccent } else { MaterialKind::OrangeTiledRoof },
                    false, true);
            }
            // Chapter landmarks: architecture changes per place, not merely fog color.
            // All props sit away from the central parkour lane and remain optional to cut.
            if i > 0 && i % 3 == 1 {
                let wing = if i % 2 == 0 { 1.0 } else { -1.0 };
                let lx = x + wing * (roof_half_width + 9.5);
                match chapter {
                    0 => { // Spring canal: low teal water ribbon and narrow stone quay.
                        Self::push_box_module(&mut modules, &mut ids, cell,
                            Vec3::new(lx-3.5, -0.20, z-6.0), Vec3::new(lx+3.5, 0.04, z+6.0),
                            MaterialKind::CheckpointShrine, CollisionLayer::NONE, 0, 0.0);
                    }
                    1 => { // Summer bazaar: suspended orange market awning.
                        Self::push_box_module(&mut modules, &mut ids, cell,
                            Vec3::new(lx-3.0, height-0.4, z-3.6),
                            Vec3::new(lx+3.0, height-0.16, z+3.6),
                            MaterialKind::OrangeTiledRoof, CollisionLayer::NONE, 0, 0.0);
                    }
                    2 => { // Foundry: two charcoal smokestack pillars.
                        for off in [-2.4_f32, 2.4] {
                            Self::push_box_module(&mut modules, &mut ids, cell,
                                Vec3::new(lx+off-0.85, 0.0, z-0.85),
                                Vec3::new(lx+off+0.85, height+13.0, z+0.85),
                                MaterialKind::InkWindowAccent, sliceable, 2, 0.0);
                        }
                    }
                    3 => { // Winter peaks: icy stepped obelisk.
                        Self::push_box_module(&mut modules, &mut ids, cell,
                            Vec3::new(lx-1.6, 0.0, z-1.6), Vec3::new(lx+1.6, height+17.0, z+1.6),
                            MaterialKind::WhitePlasterWall, sliceable, 3, 0.0);
                    }
                    4 => { // Monsoon port: tall shipyard crane mast and narrow crossarm.
                        Self::push_box_module(&mut modules, &mut ids, cell,
                            Vec3::new(lx-0.6, 0.0, z-0.6), Vec3::new(lx+0.6, height+16.0, z+0.6),
                            MaterialKind::StoneTrim, sliceable, 3, 0.0);
                        Self::push_box_module(&mut modules, &mut ids, cell,
                            Vec3::new(lx-4.2, height+14.0, z-0.4),
                            Vec3::new(lx+4.2, height+14.6, z+0.4),
                            MaterialKind::StoneTrim, sliceable, 2, 0.0);
                    }
                    _ => { // Eclipse spires: stacked ink-and-stone signal tower.
                        Self::push_box_module(&mut modules, &mut ids, cell,
                            Vec3::new(lx-1.6, 0.0, z-1.6), Vec3::new(lx+1.6, height+21.0, z+1.6),
                            MaterialKind::InkWindowAccent, sliceable, 3, 0.0);
                        Self::push_box_module(&mut modules, &mut ids, cell,
                            Vec3::new(lx-3.0, height+19.0, z-3.0),
                            Vec3::new(lx+3.0, height+20.2, z+3.0),
                            MaterialKind::OrangeTiledRoof, sliceable, 2, 0.0);
                    }
                }
            }
            let position = Vec3::new(x, height + 0.22, z);
            route_roofs.push(position);
            // Optional brush-painted foothold between alternate rooftop gaps.
            // Old level generator returned zero anchors, disabling Platform paint.
            if i > 0 && (i % 2 == 1 || i % 7 == 5) {
                let mid = (last_center + position) * 0.5;
                paint_anchors.push(PaintPlatformAnchor {
                    id: 5000 + level_number as u64 * 100 + i as u64,
                    cell_id: Self::world_to_cell(mid),
                    bounds: Aabb::new(
                        mid - Vec3::new(1.5, 0.35, 1.5),
                        mid + Vec3::new(1.5, 0.30, 1.5),
                    ),
                    active_remaining_secs: 0.0,
                    max_duration_secs: 14.0,
                });
            }
            if i == 0 || (i % checkpoint_stride == 0 && i + 1 < count) {
                checkpoints.push(Checkpoint {
                    id: format!("level_{level_number}_checkpoint_{i}"), position, cell_id: cell,
                });
                Self::push_box_module(&mut modules, &mut ids, cell,
                    Vec3::new(x - 0.6, height, z - 0.6),
                    Vec3::new(x + 0.6, height + 0.10, z + 0.6),
                    MaterialKind::CheckpointShrine, CollisionLayer::NONE, 0, 0.0);
            }
            if i > 0 && (i + theme.act) % (if theme.difficulty >= 7 { 3 } else { 4 }) == 2 {
                let ex = x + 2.3;
                enemies.push(EnemySpawn {
                    id: 9000 + level_number as u64 * 100 + i as u64,
                    position: Vec3::new(ex, height + 0.12, z),
                    patrol_a: Vec3::new(x - 2.5, height + 0.12, z),
                    patrol_b: Vec3::new(ex, height + 0.12, z), cell_id: cell,
                });
            }
            last_center = position;
        }
        // Exit beacon: stand in the marked ring to finish the level.
        let exit_cell = Self::world_to_cell(last_center);
        Self::push_box_module(&mut modules, &mut ids, exit_cell,
            last_center - Vec3::new(0.85, 0.16, 0.85),
            last_center + Vec3::new(0.85, 2.3, 0.85),
            MaterialKind::CheckpointShrine, CollisionLayer::NONE, 0, 0.0);

        // Place objects into their actual streaming cells (negative Z handled via floor).
        let mut cells: HashMap<(i32, i32), WorldCell> = HashMap::new();
        for mut m in modules {
            let coord = Self::world_to_cell(m.bounds.center());
            m.cell_id = coord;
            let cell = cells.entry(coord).or_insert_with(|| WorldCell {
                id: coord,
                district_name: format!("{} / {title}", theme.region),
                bounds: Aabb::new(Vec3::new(coord.0 as f32 * 48.0, -20.0, coord.1 as f32 * 48.0),
                    Vec3::new((coord.0 + 1) as f32 * 48.0, 65.0, (coord.1 + 1) as f32 * 48.0)),
                lod: LodLevel::Lod0Full,
                modules: Vec::new(), checkpoints: Vec::new(), enemy_spawns: Vec::new(),
            });
            cell.modules.push(m);
        }
        for cp in checkpoints {
            cells.entry(cp.cell_id).or_insert_with(|| WorldCell {
                id: cp.cell_id, district_name: format!("{} / {title}", theme.region),
                bounds: Aabb::new(Vec3::splat(-150.0), Vec3::splat(150.0)),
                lod: LodLevel::Lod0Full, modules: Vec::new(), checkpoints: Vec::new(), enemy_spawns: Vec::new(),
            }).checkpoints.push(cp);
        }
        for enemy in enemies {
            if let Some(cell) = cells.get_mut(&enemy.cell_id) { cell.enemy_spawns.push(enemy); }
        }
        let result = Self {
            cells, active_cell: Self::world_to_cell(start), lod0_radius: 1, lod1_radius: 2,
            next_module_id: ids, cut_history: Vec::new(), dust_puffs: Vec::new(),
            telemetry: WorldCutTelemetry::default(), max_dynamic_bodies: 28,
            landed_fragments_this_tick: 0, level_number, level_seed: seed,
            level_start: start, level_exit: last_center, route_roofs,
        };
        (result, paint_anchors, Vec::new())
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

                // Inverse of renderer transform around the *original* piece pivot.
                // Required for repeat cuts on already-rotating fragments.
                let old_pivot = m.bounds.center();
                let inv = m.rotation.inverse();
                let local_plane = CutPlane {
                    origin: old_pivot + inv * (plane.origin - m.offset - old_pivot),
                    normal: (inv * plane.normal).normalize_or_zero(),
                    forward: (inv * plane.forward).normalize_or_zero(),
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
                                // Child pivot changes after cutting; preserve its existing
                                // rotated world pose without a visible teleport.
                                offset: m.offset + (m.bounds.center() - bounds.center())
                                    + m.rotation * (bounds.center() - m.bounds.center()),
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
        if total_split > 0 {
            // Propagate support loss to roof beams / walls above a severed base.
            self.reconcile_structural_support();
        }
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

    /// Fixed-point support graph: ground-anchored modules support overlapping modules
    /// whose bases touch their tops; anything disconnected becomes falling rubble.
    /// This is an AABB-based structural approximation, not an FEM building solver.
    pub fn reconcile_structural_support(&mut self) {
        for cell in self.cells.values_mut() {
            let mut supported: HashSet<u64> = cell.modules.iter()
                .filter(|m| !m.is_dynamic && m.bounds.min.y + m.offset.y <= m.floor_y + 0.22)
                .map(|m| m.id).collect();
            for _ in 0..cell.modules.len().min(96) {
                let mut changed = false;
                for candidate in &cell.modules {
                    if supported.contains(&candidate.id) || candidate.is_dynamic {
                        continue;
                    }
                    let above = candidate.world_bounds();
                    if cell.modules.iter().any(|base| {
                        if !supported.contains(&base.id) || base.id == candidate.id { return false; }
                        let below = base.world_bounds();
                        let touching = (above.min.y - below.max.y).abs() <= 0.38;
                        let ox = (above.max.x.min(below.max.x) - above.min.x.max(below.min.x)).max(0.0);
                        let oz = (above.max.z.min(below.max.z) - above.min.z.max(below.min.z)).max(0.0);
                        let overlap = ox * oz;
                        let base_area = (above.max.x - above.min.x) * (above.max.z - above.min.z);
                        let wall_attached = candidate.layers == CollisionLayer::NONE
                            && above.min.y >= below.min.y - 0.15
                            && above.max.y <= below.max.y + 0.15
                            && overlap >= base_area * 0.015;
                        (touching && overlap >= base_area * 0.12) || wall_attached
                    }) {
                        changed |= supported.insert(candidate.id);
                    }
                }
                if !changed { break; }
            }
            for m in &mut cell.modules {
                if !m.is_dynamic && !supported.contains(&m.id) {
                    m.is_dynamic = true;
                    m.sleeping = false;
                    m.suspense_timer = 0.09;
                    m.pending_impulse = Vec3::new(0.4, 0.3, -0.25);
                    m.pending_ang_vel = Vec3::new(0.18, 0.12, -0.24);
                    m.cut_glow_timer = m.cut_glow_timer.max(0.20);
                }
            }
        }
    }

    pub fn tick_props(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 1.0 / 30.0);
        let mut active_dyn = 0usize;
        let mut total_mods = 0usize;
        let mut impact_dust = Vec::new();
        let mut impact_count = 0u32;
        for cell in self.cells.values_mut() {
            // Take a snapshot so fragments can hit still-standing roofs, not just floor_y.
            let stationary: Vec<(u64, Aabb)> = cell.modules.iter()
                .filter(|m| !m.is_dynamic && m.layers.intersects(CollisionLayer::STRUCTURAL))
                .map(|m| (m.id, m.world_bounds())).collect();
            for m in &mut cell.modules {
                let before = m.world_bounds();
                let downward = m.velocity.y;
                m.tick_physics(dt);
                if m.is_dynamic && !m.sleeping && m.suspense_timer <= 0.0 {
                    let mut best_top = m.floor_y;
                    let current = m.world_bounds();
                    for (id, support) in &stationary {
                        if *id == m.id || support.max.y > before.min.y + 0.3 { continue; }
                        let x_overlap = current.max.x > support.min.x + 0.08 && current.min.x < support.max.x - 0.08;
                        let z_overlap = current.max.z > support.min.z + 0.08 && current.min.z < support.max.z - 0.08;
                        if x_overlap && z_overlap && before.min.y >= support.max.y - 0.25
                            && current.min.y <= support.max.y + 0.25 {
                            best_top = best_top.max(support.max.y);
                        }
                    }
                    if current.min.y < best_top {
                        m.offset.y += best_top - current.min.y;
                        if m.velocity.y < 0.0 {
                            m.velocity.y = -m.velocity.y * 0.16;
                        }
                        m.velocity.x *= (1.0 - 5.0 * dt).max(0.0);
                        m.velocity.z *= (1.0 - 5.0 * dt).max(0.0);
                        m.angular_vel *= (1.0 - 5.0 * dt).max(0.0);
                    }
                    if downward < -5.0 && current.min.y <= best_top + 0.12 {
                        impact_count += 1;
                        // Collision-driven debris instead of a decorative cut-only puff.
                        let impact = m.world_bounds().center();
                        let count = if downward < -12.0 { 12 } else { 5 };
                        for k in 0..count {
                            let a = k as f32 * 2.3999632;
                            impact_dust.push(CutDustPuff {
                                position: Vec3::new(impact.x, best_top + 0.08, impact.z),
                                velocity: Vec3::new(a.cos() * 2.8, 1.2 + (k % 3) as f32, a.sin() * 2.8),
                                size: 0.14 + (k % 3) as f32 * 0.07,
                                remaining_secs: 0.45 + (k % 3) as f32 * 0.1,
                            });
                        }
                    }
                }
                if m.is_dynamic && !m.sleeping { active_dyn += 1; }
            }
            cell.modules.retain(|m| m.rubble_lifetime.map_or(true, |t| t > 0.0));
            total_mods += cell.modules.len();
        }
        self.landed_fragments_this_tick = impact_count;
        self.telemetry.active_dynamic_bodies = active_dyn;
        self.telemetry.total_modules = total_mods;
        self.dust_puffs.extend(impact_dust);
        // Limit persistent particle count to avoid flooding mobile GPUs after mass cuts.
        if self.dust_puffs.len() > 320 {
            let drop_count = self.dust_puffs.len() - 320;
            self.dust_puffs.drain(0..drop_count);
        }
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
    fn seeded_level_courses_have_reachable_route_and_checkpoints() {
        for level in 2..=5 {
            let (a, _, _) = WorldStreamer::build_level(level, 0xC0FFEE);
            let (b, _, _) = WorldStreamer::build_level(level, 0xC0FFEE);
            assert_eq!(a.level_exit, b.level_exit, "Same seed must reproduce exit position");
            assert!(a.level_exit.z < -75.0, "Every level must have a real route");
            assert!(a.cells.values().flat_map(|c| c.checkpoints.iter()).count() >= 3);
            assert!(a.cells.values().flat_map(|c| c.modules.iter()).any(|m|
                m.layers.contains(CollisionLayer::SLICEABLE_WORLD)));
        }
    }

    #[test]
    fn long_campaign_routes_are_seeded_and_get_more_demanding() {
        let mut previous_distance = 0.0;
        for level in [2, 6, 10, 14, 18, 22, 24] {
            let (w, _, _) = WorldStreamer::build_level(level, 0xDEA0_BEEF);
            let (same, _, _) = WorldStreamer::build_level(level, 0xDEA0_BEEF);
            assert_eq!(w.route_roofs, same.route_roofs);
            assert_eq!(w.level_exit, same.level_exit);
            assert!(w.route_roofs.len() >= 14);
            let distance = (w.level_exit.z - w.level_start.z).abs();
            assert!(distance > previous_distance, "Late chapter should be a longer route");
            previous_distance = distance;
            let count = w.cells.values().flat_map(|c| c.checkpoints.iter()).count();
            assert!(count >= 3, "Long stages require recovery checkpoints");
            assert!(w.route_roofs.windows(2).all(|v| v[1].z < v[0].z));
            if level >= 6 {
                assert!(w.route_roofs.windows(2).any(|v| (v[0].z-v[1].z) > 16.0));
            }
        }
    }

    #[test]
    fn aimed_plane_splits_a_generated_house_and_replay_restores_cut() {
        let (mut world, _, _) = WorldStreamer::build_level(2, 1234);
        let target = world.cells.values().flat_map(|c| c.modules.iter())
            .find(|m| m.layers.contains(CollisionLayer::SLICEABLE_WORLD) &&
                m.material == MaterialKind::OrangeTiledRoof)
            .expect("generated world has a sliceable roof");
        let point = target.bounds.center();
        world.update_streaming(point);
        let cut = CutPlane::new(point, Vec3::Y, Vec3::NEG_Z, 8.0, 12.0);
        assert!(world.apply_world_cut(cut, true) > 0);
        assert_eq!(world.cut_history.len(), 1);
        let (mut replay, _, _) = WorldStreamer::build_level(2, 1234);
        replay.update_streaming(point);
        replay.replay_cut_history(&world.cut_history);
        assert_eq!(replay.cut_history.len(), 1);
    }

    #[test]
    fn gate1_and_gate2_world_cut_splits_wall_and_orange_roof_with_closed_caps_and_gravity() {
        let (mut streamer, _, _) = WorldStreamer::build_vertical_slice();

        // Cut horizontally/diagonally through Demo A freestanding wall AND Demo B wall+roof
        let cut = CutPlane::new(
            Vec3::new(-7.3, 13.5, 2.0),
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
    fn parkour_facade_and_arch_are_grounded_not_hovering() {
        let (world, _, _) = WorldStreamer::build_vertical_slice();
        let cell = &world.cells[&(0, 0)];
        assert!(cell.modules.iter().any(|m| {
            (m.bounds.min.x - 10.8).abs() < 0.01 &&
            (m.bounds.min.z + 39.0).abs() < 0.01 && m.bounds.min.y == 0.0
        }));
        assert!(cell.modules.iter().any(|m| {
            (m.bounds.min.x + 7.0).abs() < 0.01 &&
            (m.bounds.min.y - 13.6).abs() < 0.01 && m.bounds.max.y > 15.0
        }));
    }

    #[test]
    fn starting_runway_is_clear_of_tall_obstacles() {
        let (world, _, _) = WorldStreamer::build_vertical_slice();
        let cell = &world.cells[&(0, 0)];
        assert!(!cell.modules.iter().any(|m| {
            m.bounds.min.y >= 11.2 && m.bounds.max.y > 13.0 &&
            m.bounds.min.x < 2.5 && m.bounds.max.x > -2.5 &&
            m.bounds.min.z < -2.0 && m.bounds.max.z > -7.5
        }), "No tall wall should block the player's starting sightline");
    }

    #[test]
    fn detached_roof_loses_support_after_base_is_removed() {
        let (mut world, _, _) = WorldStreamer::build_vertical_slice();
        let cell = world.cells.get_mut(&(0, 0)).unwrap();
        // Simple authored stack for deterministic support validation.
        let mat = MaterialKind::WarmPlasterWall;
        let layer = CollisionLayer::STRUCTURAL;
        cell.modules.clear();
        cell.modules.push(SliceableModule::new_box(70000, (0,0),
            Aabb::new(Vec3::new(0., 0., 0.), Vec3::new(4., 3., 4.)), mat, layer, 0, 0.));
        cell.modules.push(SliceableModule::new_box(70001, (0,0),
            Aabb::new(Vec3::new(0., 3., 0.), Vec3::new(4., 4., 4.)), mat, layer, 0, 0.));
        world.reconcile_structural_support();
        assert!(!world.cells[&(0, 0)].modules[1].is_dynamic);
        world.cells.get_mut(&(0, 0)).unwrap().modules.remove(0);
        world.reconcile_structural_support();
        assert!(world.cells[&(0, 0)].modules[0].is_dynamic);
    }

    #[test]
    fn rotated_fragment_expands_collision_bounds() {
        let mut fragment = SliceableModule::new_box(9, (0, 0), Aabb::new(
            Vec3::new(-2., 0., -0.2), Vec3::new(2., 1., 0.2)),
            MaterialKind::StoneTrim, CollisionLayer::STRUCTURAL, 0, 0.);
        let before = fragment.world_bounds();
        fragment.rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let after = fragment.world_bounds();
        assert!(after.max.z - after.min.z > before.max.z - before.min.z);
    }

    #[test]
    fn gate3_repeat_cut_splits_already_sliced_piece() {
        let (mut streamer, _, _) = WorldStreamer::build_vertical_slice();
        let cut1 = CutPlane::new(
            Vec3::new(-7.3, 13.4, 2.0),
            Vec3::Y,
            Vec3::NEG_Z,
            12.0,
            5.0,
        );
        assert!(streamer.apply_world_cut(cut1, true) >= 1);

        // Second perpendicular cut on the remaining halves
        let cut2 = CutPlane::new(
            Vec3::new(-7.3, 13.4, 2.0),
            Vec3::X,
            Vec3::NEG_Z,
            12.0,
            5.0,
        );
        assert!(streamer.apply_world_cut(cut2, true) >= 2);
    }
}
