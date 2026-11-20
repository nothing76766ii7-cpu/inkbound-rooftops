//! Core deterministic simulation primitives, collision layers, swept-capsule physics,
//! real 3D mesh slicing, palette tokens, and game settings for Inkbound Rooftops (PRD v3.0).

pub mod mesh_slicer;

use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Fixed physics simulation rate (60 Hz) for deterministic traversal across 30/60 FPS devices.
pub const FIXED_DT: f32 = 1.0 / 60.0;

/// Maximum substeps per frame to prevent spiral-of-death after Android resume stalls.
pub const MAX_SIM_SUBSTEPS: u32 = 4;

/// Reference-aligned sketchbook color tokens (linear-approx RGB f32).
pub struct Palette;

impl Palette {
    /// Off-white paper / pale walls (#F5F4EF)
    pub const PAPER: [f32; 4] = [0.961, 0.957, 0.937, 1.0];
    /// Shaded stone / secondary walls (#D8D8D5)
    pub const STONE: [f32; 4] = [0.847, 0.847, 0.835, 1.0];
    /// Charcoal hand-drawn ink outlines (#25262B)
    pub const INK: [f32; 4] = [0.145, 0.149, 0.169, 1.0];
    /// Terracotta / roof orange (#F47B38)
    pub const ROOF_ORANGE: [f32; 4] = [0.957, 0.482, 0.220, 1.0];
    /// Health bar red (#B72B2B)
    pub const HEALTH_RED: [f32; 4] = [0.718, 0.169, 0.169, 1.0];
    /// Muted gray haze / UI secondary (#9C9FA0)
    pub const MUTED_GRAY: [f32; 4] = [0.612, 0.624, 0.627, 1.0];
    /// Paint teal accent (#2A8C82)
    pub const PAINT_TEAL: [f32; 4] = [0.165, 0.549, 0.510, 1.0];
    /// Bright overcast sky / atmospheric fog (#E8E9E7)
    pub const OVERCAST_FOG: [f32; 4] = [0.910, 0.914, 0.906, 1.0];
    /// Fresh interior stone cross-section core for sliced plaster walls (PRD v3.0)
    pub const CUT_CORE_STONE: [f32; 4] = [0.835, 0.796, 0.749, 1.0];
    /// Fresh interior timber/terracotta cross-section core for sliced roofs (PRD v3.0)
    pub const CUT_CORE_TIMBER: [f32; 4] = [0.550, 0.295, 0.176, 1.0];
}

/// Bitmask collision layers supporting PRD v3.0 sliceable architecture,
/// enemies, props, climbable edges, and paint surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CollisionLayer(pub u32);

impl CollisionLayer {
    pub const NONE: Self = Self(0);
    /// Solid architectural geometry (rooftops, walls, towers).
    pub const STRUCTURAL: Self = Self(1 << 0);
    /// Ledges & rooftop edges eligible for vault/mantle/ledge-grab.
    pub const CLIMBABLE: Self = Self(1 << 1);
    /// Vertical walls eligible for wall-running.
    pub const WALL_RUNNABLE: Self = Self(1 << 2);
    /// Tagged architectural surfaces eligible for paint strokes.
    pub const PAINTABLE: Self = Self(1 << 3);
    /// Enemy hitboxes eligible for sword & world-cut slash.
    pub const SLASHABLE_ENEMY: Self = Self(1 << 4);
    /// Designated breakable/cuttable props.
    pub const SLASHABLE_PROP: Self = Self(1 << 5);
    /// Temporary solid paint footholds spawned on authored anchor volumes.
    pub const PAINT_PLATFORM: Self = Self(1 << 6);
    /// Checkpoint shrine trigger volumes.
    pub const CHECKPOINT_TRIGGER: Self = Self(1 << 7);
    /// PRD v3.0 override: architectural module eligible for real 3D plane slicing!
    pub const SLICEABLE_WORLD: Self = Self(1 << 8);

    #[inline]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[inline]
    pub const fn intersects(self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }

    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    #[inline]
    pub const fn is_solid(self) -> bool {
        self.intersects(Self::STRUCTURAL.union(Self::PAINT_PLATFORM))
    }

    /// Enemies, props, and `SLICEABLE_WORLD` architectural modules are all sliceable (PRD v3.0).
    #[inline]
    pub const fn is_slashable(self) -> bool {
        self.intersects(
            Self::SLASHABLE_ENEMY
                .union(Self::SLASHABLE_PROP)
                .union(Self::SLICEABLE_WORLD),
        )
    }
}

/// Axis-aligned bounding box used for broadphase & simplified rooftop collision proxies.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self {
            min: min.min(max),
            max: min.max(max),
        }
    }

    pub fn from_center_half_extents(center: Vec3, half: Vec3) -> Self {
        Self {
            min: center - half,
            max: center + half,
        }
    }

    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    pub fn half_extents(&self) -> Vec3 {
        (self.max - self.min) * 0.5
    }

    pub fn closest_point(&self, point: Vec3) -> Vec3 {
        point.clamp(self.min, self.max)
    }

    pub fn intersects_aabb(&self, other: &Self) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
            && self.min.z <= other.max.z
            && self.max.z >= other.min.z
    }

    /// Slab ray-AABB intersection returning `(distance, hit_normal)` if hit within `[0, max_dist]`.
    pub fn raycast(&self, origin: Vec3, dir: Vec3, max_dist: f32) -> Option<(f32, Vec3)> {
        let mut tmin = 0.0_f32;
        let mut tmax = max_dist;
        let mut hit_normal = Vec3::ZERO;

        for axis in 0..3 {
            let o = origin[axis];
            let d = dir[axis];
            let min_b = self.min[axis];
            let max_b = self.max[axis];

            if d.abs() < 1e-6 {
                if o < min_b || o > max_b {
                    return None;
                }
            } else {
                let inv_d = 1.0 / d;
                let mut t1 = (min_b - o) * inv_d;
                let mut t2 = (max_b - o) * inv_d;
                let mut normal_sign = -1.0_f32;
                if t1 > t2 {
                    std::mem::swap(&mut t1, &mut t2);
                    normal_sign = 1.0;
                }
                if t1 > tmin {
                    tmin = t1;
                    hit_normal = Vec3::ZERO;
                    hit_normal[axis] = normal_sign;
                }
                tmax = tmax.min(t2);
                if tmin > tmax {
                    return None;
                }
            }
        }

        Some((tmin, hit_normal))
    }
}

/// Vertical player collision capsule defined by feet position, radius, and total height.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Capsule {
    pub feet: Vec3,
    pub radius: f32,
    pub height: f32,
}

impl Capsule {
    pub fn new(feet: Vec3, radius: f32, height: f32) -> Self {
        Self {
            feet,
            radius,
            height: height.max(radius * 2.0),
        }
    }

    pub fn bounds(&self) -> Aabb {
        Aabb::new(
            Vec3::new(self.feet.x - self.radius, self.feet.y, self.feet.z - self.radius),
            Vec3::new(
                self.feet.x + self.radius,
                self.feet.y + self.height,
                self.feet.z + self.radius,
            ),
        )
    }

    /// Computes minimal push-out vector separating this capsule from a solid AABB.
    pub fn resolve_aabb(&self, aabb: &Aabb) -> Option<(Vec3, Vec3)> {
        let cap_box = self.bounds();
        if !cap_box.intersects_aabb(aabb) {
            return None;
        }

        let low_y = self.feet.y + self.radius;
        let high_y = self.feet.y + self.height - self.radius;
        let clamped_y = aabb.center().y.clamp(low_y, high_y);
        let sphere_center = Vec3::new(self.feet.x, clamped_y, self.feet.z);
        let closest = aabb.closest_point(sphere_center);
        let delta = sphere_center - closest;
        let dist_sq = delta.length_squared();

        if dist_sq >= self.radius * self.radius {
            return None;
        }

        if dist_sq > 1e-8 {
            let dist = dist_sq.sqrt();
            let normal = delta / dist;
            let penetration = self.radius - dist;
            return Some((normal * penetration, normal));
        }

        let pen_left = (cap_box.max.x - aabb.min.x).abs();
        let pen_right = (aabb.max.x - cap_box.min.x).abs();
        let pen_down = (cap_box.max.y - aabb.min.y).abs();
        let pen_up = (aabb.max.y - cap_box.min.y).abs();
        let pen_back = (cap_box.max.z - aabb.min.z).abs();
        let pen_front = (aabb.max.z - cap_box.min.z).abs();

        let candidates = [
            (pen_up, Vec3::Y),
            (pen_down, Vec3::NEG_Y),
            (pen_right, Vec3::X),
            (pen_left, Vec3::NEG_X),
            (pen_front, Vec3::Z),
            (pen_back, Vec3::NEG_Z),
        ];

        let (best_pen, best_normal) = candidates
            .into_iter()
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .unwrap_or((0.0, Vec3::Y));

        Some((best_normal * best_pen, best_normal))
    }
}

/// World collider proxy used for traversal, raycasts, paint, and combat target checks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collider {
    pub id: u64,
    pub surface_id: u32,
    pub cell_id: (i32, i32),
    pub bounds: Aabb,
    pub layers: CollisionLayer,
    pub active: bool,
}

/// Result of a world raycast against colliders matching a layer mask.
#[derive(Debug, Clone, Copy)]
pub struct RayHit {
    pub collider_id: u64,
    pub surface_id: u32,
    pub cell_id: (i32, i32),
    pub distance: f32,
    pub point: Vec3,
    pub normal: Vec3,
    pub layers: CollisionLayer,
}

/// Casts a ray against a slice of colliders filtered by `mask`.
pub fn raycast_world(
    colliders: &[Collider],
    origin: Vec3,
    dir: Vec3,
    max_dist: f32,
    mask: CollisionLayer,
) -> Option<RayHit> {
    let dir_norm = dir.normalize_or_zero();
    if dir_norm.length_squared() < 0.5 {
        return None;
    }

    let mut best: Option<RayHit> = None;
    for col in colliders {
        if !col.active || !col.layers.intersects(mask) {
            continue;
        }
        let limit = best.map_or(max_dist, |h| h.distance);
        if let Some((dist, normal)) = col.bounds.raycast(origin, dir_norm, limit) {
            best = Some(RayHit {
                collider_id: col.id,
                surface_id: col.surface_id,
                cell_id: col.cell_id,
                distance: dist,
                point: origin + dir_norm * dist,
                normal,
                layers: col.layers,
            });
        }
    }
    best
}

/// Fixed-timestep accumulator clock with spiral-of-death clamping.
#[derive(Debug, Clone)]
pub struct FixedTimestepClock {
    accumulator: f32,
    pub fixed_dt: f32,
    pub max_substeps: u32,
    pub tick_count: u64,
}

impl Default for FixedTimestepClock {
    fn default() -> Self {
        Self {
            accumulator: 0.0,
            fixed_dt: FIXED_DT,
            max_substeps: MAX_SIM_SUBSTEPS,
            tick_count: 0,
        }
    }
}

impl FixedTimestepClock {
    pub fn advance(&mut self, frame_dt: f32) -> (u32, f32) {
        let clamped = frame_dt.clamp(0.0, 0.1);
        self.accumulator += clamped;

        let mut steps = 0;
        while self.accumulator >= self.fixed_dt && steps < self.max_substeps {
            self.accumulator -= self.fixed_dt;
            steps += 1;
            self.tick_count += 1;
        }

        if steps == self.max_substeps && self.accumulator > self.fixed_dt {
            self.accumulator = 0.0;
        }

        let alpha = (self.accumulator / self.fixed_dt).clamp(0.0, 1.0);
        (steps, alpha)
    }
}

/// Graphics quality preset aligned with PRD section 3.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphicsPreset {
    Low,
    Balanced,
    High,
}

/// Complete user settings model (PRD section 3.15).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameSettings {
    pub graphics_preset: GraphicsPreset,
    pub render_scale: f32,
    pub fps_cap: u32,
    pub outline_enabled: bool,
    pub sensitivity_x: f32,
    pub sensitivity_y: f32,
    pub invert_y: bool,
    pub hud_scale: f32,
    pub hud_opacity: f32,
    pub left_handed: bool,
    pub gyro_enabled: bool,
    pub aim_assist: bool,
    pub reduced_motion: bool,
    pub reduced_flashes: bool,
    pub colorblind_symbols: bool,
    pub fov_degrees: f32,
    pub head_bob: f32,
    pub camera_shake: f32,
    pub master_volume: f32,
    pub music_volume: f32,
    pub sfx_volume: f32,
    pub haptics_enabled: bool,
    pub language: String,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            graphics_preset: GraphicsPreset::Balanced,
            render_scale: 0.85,
            fps_cap: 60,
            outline_enabled: true,
            sensitivity_x: 1.0,
            sensitivity_y: 1.0,
            invert_y: false,
            hud_scale: 1.0,
            hud_opacity: 0.85,
            left_handed: false,
            gyro_enabled: false,
            aim_assist: true,
            reduced_motion: false,
            reduced_flashes: false,
            colorblind_symbols: true,
            fov_degrees: 75.0,
            head_bob: 0.6,
            camera_shake: 0.7,
            master_volume: 0.9,
            music_volume: 0.7,
            sfx_volume: 0.9,
            haptics_enabled: true,
            language: "en".to_string(),
        }
    }
}

impl GameSettings {
    pub fn apply_preset(&mut self, preset: GraphicsPreset) {
        self.graphics_preset = preset;
        match preset {
            GraphicsPreset::Low => {
                self.render_scale = 0.70;
                self.fps_cap = 30;
                self.outline_enabled = true;
            }
            GraphicsPreset::Balanced => {
                self.render_scale = 0.85;
                self.fps_cap = 60;
                self.outline_enabled = true;
            }
            GraphicsPreset::High => {
                self.render_scale = 1.0;
                self.fps_cap = 60;
                self.outline_enabled = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v3_sliceable_world_buildings_are_both_solid_and_slashable() {
        let building = CollisionLayer::STRUCTURAL
            .union(CollisionLayer::SLICEABLE_WORLD)
            .union(CollisionLayer::PAINTABLE);
        assert!(building.is_solid());
        assert!(building.is_slashable());

        // Essential spawn floor without SLICEABLE_WORLD remains protected from soft-lock
        let spawn_floor = CollisionLayer::STRUCTURAL;
        assert!(spawn_floor.is_solid());
        assert!(!spawn_floor.is_slashable());
    }

    #[test]
    fn capsule_resolves_vertical_floor_penetration() {
        let floor = Aabb::new(Vec3::new(-10.0, -1.0, -10.0), Vec3::new(10.0, 0.0, 10.0));
        let cap = Capsule::new(Vec3::new(0.0, -0.15, 0.0), 0.35, 1.8);
        let (push, normal) = cap.resolve_aabb(&floor).expect("Should penetrate floor");
        assert!(push.y > 0.14);
        assert!(normal.y > 0.9);
    }

    #[test]
    fn fixed_timestep_clamps_long_android_resume_stall() {
        let mut clock = FixedTimestepClock::default();
        let (steps, alpha) = clock.advance(2.5);
        assert_eq!(steps, MAX_SIM_SUBSTEPS);
        assert!((0.0..=1.0).contains(&alpha));
    }
}
