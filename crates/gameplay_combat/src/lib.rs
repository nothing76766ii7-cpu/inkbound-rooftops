//! Sword combat and Dimensional Slash system (PRD section 3.6).
//!
//! Enforces strict world-safety rules: static `STRUCTURAL` buildings are NEVER
//! sliced or collision-compromised. Only `SLASHABLE_ENEMY` and `SLASHABLE_PROP`
//! targets receive damage/split effects, while a visual rift plane delivers the
//! dramatic spatial-cut aesthetic.

use engine_core::{raycast_world, Collider, CollisionLayer};
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Data-driven slash configuration (PRD section 3.15).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlashConfig {
    pub light_damage: f32,
    pub heavy_damage: f32,
    pub dimensional_damage: f32,
    pub paint_mark_multiplier: f32,
    pub melee_range: f32,
    pub dimensional_range: f32,
    pub charge_seconds: f32,
    pub light_cooldown: f32,
    pub heavy_cooldown: f32,
    pub dimensional_cooldown: f32,
    pub dimensional_energy_cost: f32,
    pub allowed_target_layers: CollisionLayer,
}

impl Default for SlashConfig {
    fn default() -> Self {
        Self {
            light_damage: 25.0,
            heavy_damage: 45.0,
            dimensional_damage: 90.0,
            paint_mark_multiplier: 1.75,
            melee_range: 3.4,
            dimensional_range: 28.0,
            charge_seconds: 0.45,
            light_cooldown: 0.30,
            heavy_cooldown: 0.60,
            dimensional_cooldown: 3.5,
            dimensional_energy_cost: 40.0,
            allowed_target_layers: CollisionLayer::SLASHABLE_ENEMY
                .union(CollisionLayer::SLASHABLE_PROP),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttackKind {
    Light,
    Heavy,
    Dimensional,
}

/// Active sword & dimensional-slash runtime state.
#[derive(Debug, Clone)]
pub struct CombatState {
    pub slash_energy: f32,
    pub max_slash_energy: f32,
    pub melee_cooldown: f32,
    pub dimensional_cooldown: f32,
    pub swing_anim_timer: f32,
    pub swing_kind: Option<AttackKind>,
    pub hit_stop_timer: f32,
    /// Visual intensity `[0.0, 1.0]` for the screen-space dimensional rift shader.
    pub dimensional_rift_intensity: f32,
    pub rift_plane_angle: f32,
    pub charge_ratio: f32,
}

impl Default for CombatState {
    fn default() -> Self {
        Self {
            slash_energy: 100.0,
            max_slash_energy: 100.0,
            melee_cooldown: 0.0,
            dimensional_cooldown: 0.0,
            swing_anim_timer: 0.0,
            swing_kind: None,
            hit_stop_timer: 0.0,
            dimensional_rift_intensity: 0.0,
            rift_plane_angle: 0.25,
            charge_ratio: 0.0,
        }
    }
}

/// Outcome of an executed sword or dimensional slash.
#[derive(Debug, Clone)]
pub struct SlashHitResult {
    pub collider_id: u64,
    pub kind: AttackKind,
    pub damage: f32,
    pub hit_point: Vec3,
    pub knockback: Vec3,
    pub triggers_split_vfx: bool,
}

impl CombatState {
    pub fn tick(&mut self, dt: f32, charging: bool, charge_secs: f32, cfg: &SlashConfig) {
        self.melee_cooldown = (self.melee_cooldown - dt).max(0.0);
        self.dimensional_cooldown = (self.dimensional_cooldown - dt).max(0.0);
        self.swing_anim_timer = (self.swing_anim_timer - dt).max(0.0);
        self.hit_stop_timer = (self.hit_stop_timer - dt).max(0.0);
        self.dimensional_rift_intensity = (self.dimensional_rift_intensity - dt * 2.4).max(0.0);
        self.slash_energy = (self.slash_energy + 10.0 * dt).min(self.max_slash_energy);
        self.charge_ratio = if charging {
            (charge_secs / cfg.charge_seconds).clamp(0.0, 1.0)
        } else {
            0.0
        };
    }

    /// Executes a Light, Heavy, or Dimensional slash and returns all valid hits.
    ///
    /// Line-of-sight is checked against `STRUCTURAL` walls so attacks never clip
    /// through buildings, and `STRUCTURAL` colliders are strictly excluded from damage.
    pub fn perform_slash(
        &mut self,
        kind: AttackKind,
        origin: Vec3,
        look_dir: Vec3,
        colliders: &[Collider],
        marked_enemy_ids: &[u64],
        cfg: &SlashConfig,
    ) -> Option<SlashHitResult> {
        let (range, base_damage, cooldown) = match kind {
            AttackKind::Light => {
                if self.melee_cooldown > 0.0 {
                    return None;
                }
                (cfg.melee_range, cfg.light_damage, cfg.light_cooldown)
            }
            AttackKind::Heavy => {
                if self.melee_cooldown > 0.0 {
                    return None;
                }
                (cfg.melee_range * 1.15, cfg.heavy_damage, cfg.heavy_cooldown)
            }
            AttackKind::Dimensional => {
                if self.dimensional_cooldown > 0.0 || self.slash_energy < cfg.dimensional_energy_cost
                {
                    return None;
                }
                self.slash_energy -= cfg.dimensional_energy_cost;
                self.dimensional_cooldown = cfg.dimensional_cooldown;
                self.dimensional_rift_intensity = 1.0;
                self.rift_plane_angle = 0.22;
                (
                    cfg.dimensional_range,
                    cfg.dimensional_damage,
                    cfg.light_cooldown,
                )
            }
        };

        self.melee_cooldown = cooldown;
        self.swing_anim_timer = match kind {
            AttackKind::Light => 0.24,
            AttackKind::Heavy => 0.38,
            AttackKind::Dimensional => 0.50,
        };
        self.swing_kind = Some(kind);

        // Enforce line-of-sight: buildings block slash reach without being damaged.
        let wall_limit = raycast_world(
            colliders,
            origin,
            look_dir,
            range,
            CollisionLayer::STRUCTURAL,
        )
        .map_or(range, |h| h.distance);

        // Query strictly valid non-structural targets within unobstructed reach.
        let target_hit = raycast_world(
            colliders,
            origin,
            look_dir,
            wall_limit,
            cfg.allowed_target_layers,
        )?;

        if !target_hit.layers.is_slashable() {
            return None;
        }

        let is_marked = marked_enemy_ids.contains(&target_hit.collider_id);
        let final_damage = if is_marked {
            base_damage * cfg.paint_mark_multiplier
        } else {
            base_damage
        };

        self.hit_stop_timer = match kind {
            AttackKind::Light => 0.03,
            AttackKind::Heavy => 0.06,
            AttackKind::Dimensional => 0.11,
        };

        let knockback_strength = match kind {
            AttackKind::Light => 4.0,
            AttackKind::Heavy => 8.0,
            AttackKind::Dimensional => 14.0,
        };

        Some(SlashHitResult {
            collider_id: target_hit.collider_id,
            kind,
            damage: final_damage,
            hit_point: target_hit.point,
            knockback: look_dir.normalize_or_zero() * knockback_strength + Vec3::Y * 2.5,
            triggers_split_vfx: kind == AttackKind::Dimensional,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_core::Aabb;

    #[test]
    fn dimensional_slash_never_damages_structural_buildings() {
        let cfg = SlashConfig::default();
        let mut combat = CombatState::default();
        let building = Collider {
            id: 100,
            surface_id: 1,
            cell_id: (0, 0),
            bounds: Aabb::from_center_half_extents(Vec3::new(0.0, 2.0, -8.0), Vec3::splat(3.0)),
            layers: CollisionLayer::STRUCTURAL,
            active: true,
        };

        let hit = combat.perform_slash(
            AttackKind::Dimensional,
            Vec3::new(0.0, 2.0, 0.0),
            Vec3::NEG_Z,
            &[building],
            &[],
            &cfg,
        );
        assert!(hit.is_none(), "Structural buildings must remain immune to dimensional slash");
        assert!(combat.dimensional_rift_intensity > 0.9, "Visual rift VFX still plays");
    }

    #[test]
    fn paint_marked_enemy_takes_amplified_dimensional_slash_damage() {
        let cfg = SlashConfig::default();
        let mut combat = CombatState::default();
        let wraith = Collider {
            id: 201,
            surface_id: 201,
            cell_id: (0, 0),
            bounds: Aabb::from_center_half_extents(Vec3::new(0.0, 1.5, -10.0), Vec3::new(0.6, 1.0, 0.6)),
            layers: CollisionLayer::SLASHABLE_ENEMY,
            active: true,
        };

        let hit = combat
            .perform_slash(
                AttackKind::Dimensional,
                Vec3::new(0.0, 1.5, 0.0),
                Vec3::NEG_Z,
                &[wraith],
                &[201],
                &cfg,
            )
            .expect("Should hit Ink Wraith");
        assert!(hit.triggers_split_vfx);
        assert!(
            (hit.damage - cfg.dimensional_damage * cfg.paint_mark_multiplier).abs() < 1e-3
        );
    }
}
