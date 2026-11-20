//! Sword combat, deterministic attack state machine, swept blade-arc hit detection,
//! and real 3D World-Cut `CutPlane` commitment (PRD v3.0).

use engine_core::mesh_slicer::CutPlane;
use engine_core::{raycast_world, Aabb, Collider, CollisionLayer};
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Data-driven slash configuration (PRD v3.0).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlashConfig {
    pub light_damage: f32,
    pub heavy_damage: f32,
    pub dimensional_damage: f32,
    pub paint_mark_multiplier: f32,
    pub melee_reach: f32,
    pub blade_sweep_radius: f32,
    pub aim_assist_cone_cos: f32,
    pub dimensional_range: f32,
    pub dimensional_speed: f32,
    pub dimensional_width: f32,
    pub dimensional_cooldown: f32,
    pub dimensional_energy_cost: f32,
    pub allowed_target_layers: CollisionLayer,
}

impl Default for SlashConfig {
    fn default() -> Self {
        Self {
            light_damage: 30.0,
            heavy_damage: 58.0,
            dimensional_damage: 95.0,
            paint_mark_multiplier: 1.75,
            melee_reach: 3.8,
            blade_sweep_radius: 0.75,
            aim_assist_cone_cos: 0.76,
            dimensional_range: 34.0,
            dimensional_speed: 44.0,
            dimensional_width: 6.5,
            dimensional_cooldown: 1.4,
            dimensional_energy_cost: 25.0,
            allowed_target_layers: CollisionLayer::SLASHABLE_ENEMY
                .union(CollisionLayer::SLASHABLE_PROP),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttackKind {
    LightA,
    LightB,
    LightC,
    Heavy,
    Dimensional,
}

/// Deterministic attack state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttackState {
    Idle,
    Windup,
    Active,
    Recovery,
    Charged,
    Interrupted,
}

/// Visible 3D slash crescent ribbon segment emitted from animated blade-tip trajectory.
#[derive(Debug, Clone)]
pub struct SlashTrailSegment {
    pub base: Vec3,
    pub tip: Vec3,
    pub remaining_secs: f32,
    pub max_secs: f32,
    pub is_dimensional: bool,
}

/// Impact spark burst (enemy hit, wall cut, or melee clang).
#[derive(Debug, Clone)]
pub struct ImpactSpark {
    pub position: Vec3,
    pub velocity: Vec3,
    pub remaining_secs: f32,
    pub is_orange_accent: bool,
}

/// Traveling 3D dimensional slash crescent with swept-volume enemy hit detection.
#[derive(Debug, Clone)]
pub struct DimensionalRiftWave {
    pub origin: Vec3,
    pub position: Vec3,
    pub direction: Vec3,
    pub right: Vec3,
    pub traveled: f32,
    pub max_range: f32,
    pub width: f32,
    pub hit_ids: Vec<u64>,
}

#[derive(Debug, Clone)]
pub struct SlashHitResult {
    pub collider_id: u64,
    pub kind: AttackKind,
    pub damage: f32,
    pub hit_point: Vec3,
    pub knockback: Vec3,
    pub triggers_split_vfx: bool,
    pub is_prop: bool,
}

/// Per-tick combat events, including any committed real 3D `CutPlane` for world slicing!
#[derive(Debug, Clone, Default)]
pub struct CombatStepOutput {
    pub hits: Vec<SlashHitResult>,
    pub swing_started: Option<AttackKind>,
    pub wall_clang_point: Option<Vec3>,
    pub dimensional_launched: bool,
    /// Real 3D world-slicing plane committed on active frame of Dimensional/Heavy cut.
    pub committed_world_cut: Option<CutPlane>,
}

/// Authoritative combat runtime state.
#[derive(Debug, Clone)]
pub struct CombatState {
    pub state: AttackState,
    pub current_attack: Option<AttackKind>,
    pub buffered_attack: Option<AttackKind>,
    pub combo_step: u8,
    pub state_timer: f32,
    pub state_duration: f32,
    pub slash_energy: f32,
    pub max_slash_energy: f32,
    pub dimensional_cooldown: f32,
    pub hit_stop_timer: f32,
    pub dimensional_rift_intensity: f32,
    pub rift_plane_angle: f32,
    pub charge_ratio: f32,
    /// Active aiming-plane preview state while holding World-Cut button (PRD v3.0).
    pub world_cut_preview_active: bool,
    pub world_cut_angle: f32,
    pub hit_ids_this_swing: Vec<u64>,
    pub prev_blade_tip: Option<Vec3>,
    pub active_trails: Vec<SlashTrailSegment>,
    pub active_sparks: Vec<ImpactSpark>,
    pub active_rifts: Vec<DimensionalRiftWave>,
}

impl Default for CombatState {
    fn default() -> Self {
        Self {
            state: AttackState::Idle,
            current_attack: None,
            buffered_attack: None,
            combo_step: 0,
            state_timer: 0.0,
            state_duration: 0.0,
            slash_energy: 100.0,
            max_slash_energy: 100.0,
            dimensional_cooldown: 0.0,
            hit_stop_timer: 0.0,
            dimensional_rift_intensity: 0.0,
            rift_plane_angle: 0.24,
            charge_ratio: 0.0,
            world_cut_preview_active: false,
            world_cut_angle: 0.24,
            hit_ids_this_swing: Vec::new(),
            prev_blade_tip: None,
            active_trails: Vec::new(),
            active_sparks: Vec::new(),
            active_rifts: Vec::new(),
        }
    }
}

impl CombatState {
    pub fn request_attack(
        &mut self,
        kind: AttackKind,
        cut_angle: f32,
        paused: bool,
        cfg: &SlashConfig,
    ) -> bool {
        if paused || self.state == AttackState::Interrupted {
            return false;
        }
        if kind == AttackKind::Dimensional
            && (self.dimensional_cooldown > 0.0 || self.slash_energy < cfg.dimensional_energy_cost)
        {
            return false;
        }

        self.world_cut_angle = cut_angle;

        let resolved = match kind {
            AttackKind::LightA => match self.combo_step % 3 {
                0 => AttackKind::LightA,
                1 => AttackKind::LightB,
                _ => AttackKind::LightC,
            },
            other => other,
        };

        match self.state {
            AttackState::Idle | AttackState::Charged => {
                self.start_windup(resolved, cfg);
                true
            }
            AttackState::Recovery => {
                self.buffered_attack = Some(resolved);
                true
            }
            AttackState::Windup | AttackState::Active | AttackState::Interrupted => false,
        }
    }

    fn start_windup(&mut self, kind: AttackKind, cfg: &SlashConfig) {
        self.current_attack = Some(kind);
        self.state = AttackState::Windup;
        self.hit_ids_this_swing.clear();
        self.prev_blade_tip = None;

        // Readable anticipation timings (PRD v3.0: 80-180ms)
        let windup = match kind {
            AttackKind::LightA | AttackKind::LightB | AttackKind::LightC => 0.085,
            AttackKind::Heavy => 0.16,
            AttackKind::Dimensional => {
                self.slash_energy = (self.slash_energy - cfg.dimensional_energy_cost).max(0.0);
                self.dimensional_cooldown = cfg.dimensional_cooldown;
                0.11
            }
        };
        self.state_timer = windup;
        self.state_duration = windup;
    }

    pub fn phase_progress(&self) -> f32 {
        if self.state_duration <= 1e-5 {
            return 0.0;
        }
        (1.0 - self.state_timer / self.state_duration).clamp(0.0, 1.0)
    }

    /// Computes world-space `CutPlane` from camera origin, look direction, and chosen cut angle.
    pub fn build_cut_plane(eye: Vec3, look_dir: Vec3, angle: f32, range: f32, half_width: f32) -> CutPlane {
        let fwd = look_dir.normalize_or_zero();
        let right = fwd.cross(Vec3::Y).normalize_or_zero();
        let up = right.cross(fwd).normalize_or_zero();
        // Plane normal perpendicular to both `fwd` and the tilted slash line
        let normal = (up * angle.cos() - right * angle.sin()).normalize_or_zero();
        CutPlane::new(eye + fwd * 0.5, normal, fwd, range, half_width)
    }

    /// Computes world-space `(blade_base, blade_tip)` along the current swing arc.
    pub fn current_blade_segment(&self, eye: Vec3, look_dir: Vec3) -> (Vec3, Vec3) {
        let fwd = look_dir.normalize_or_zero();
        let right = fwd.cross(Vec3::Y).normalize_or_zero();
        let up = right.cross(fwd).normalize_or_zero();

        let p = self.phase_progress();
        let (yaw_off, pitch_off, reach) = match (self.state, self.current_attack) {
            (AttackState::Windup, Some(AttackKind::LightA)) => (0.85 * p, 0.35 * p, 1.5),
            (AttackState::Active, Some(AttackKind::LightA)) => (0.85 - 1.85 * p, 0.35 - 0.70 * p, 3.0),
            (AttackState::Active, Some(AttackKind::LightB)) => (-0.95 + 1.90 * p, -0.25 + 0.65 * p, 3.0),
            (AttackState::Active, Some(AttackKind::LightC)) => (0.95 - 1.95 * p, 0.05, 3.2),
            (AttackState::Active, Some(AttackKind::Heavy)) => (0.15 - 0.30 * p, 1.05 - 2.05 * p, 3.4),
            (AttackState::Active, Some(AttackKind::Dimensional)) => (1.10 - 2.20 * p, 0.35 - 0.70 * p, 3.8),
            _ => (0.35, -0.18, 1.2),
        };

        let swing_dir = (fwd + right * yaw_off + up * pitch_off).normalize_or_zero();
        let base = eye + fwd * 0.45 + right * 0.22 - up * 0.14;
        let tip = base + swing_dir * reach;
        (base, tip)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn step(
        &mut self,
        dt: f32,
        eye: Vec3,
        look_dir: Vec3,
        charging: bool,
        charge_secs: f32,
        world_cut_previewing: bool,
        world_cut_angle: f32,
        paused: bool,
        aim_assist: bool,
        colliders: &[Collider],
        marked_enemy_ids: &[u64],
        cfg: &SlashConfig,
    ) -> CombatStepOutput {
        let mut out = CombatStepOutput::default();

        self.dimensional_cooldown = (self.dimensional_cooldown - dt).max(0.0);
        self.hit_stop_timer = (self.hit_stop_timer - dt).max(0.0);
        self.dimensional_rift_intensity = (self.dimensional_rift_intensity - dt * 2.2).max(0.0);
        self.slash_energy = (self.slash_energy + 14.0 * dt).min(self.max_slash_energy);
        self.world_cut_preview_active = world_cut_previewing && !paused;
        if world_cut_previewing {
            self.world_cut_angle = world_cut_angle;
            self.rift_plane_angle = world_cut_angle;
        }
        self.charge_ratio = if charging && !paused {
            (charge_secs / 0.35).clamp(0.0, 1.0)
        } else {
            0.0
        };

        for trail in &mut self.active_trails {
            trail.remaining_secs -= dt;
        }
        self.active_trails.retain(|t| t.remaining_secs > 0.0);

        for spark in &mut self.active_sparks {
            spark.position += spark.velocity * dt;
            spark.velocity.y -= 14.0 * dt;
            spark.remaining_secs -= dt;
        }
        self.active_sparks.retain(|s| s.remaining_secs > 0.0);

        if paused {
            return out;
        }

        self.step_dimensional_rifts(dt, colliders, marked_enemy_ids, cfg, &mut out);

        if self.state == AttackState::Idle && charging && self.charge_ratio >= 1.0 {
            self.state = AttackState::Charged;
        } else if self.state == AttackState::Charged && !charging {
            self.state = AttackState::Idle;
        }

        if matches!(
            self.state,
            AttackState::Windup | AttackState::Active | AttackState::Recovery
        ) {
            self.state_timer -= dt;
        }

        match self.state {
            AttackState::Windup => {
                if self.state_timer <= 0.0 {
                    let kind = self.current_attack.unwrap_or(AttackKind::LightA);
                    self.state = AttackState::Active;
                    let active_dur = match kind {
                        AttackKind::LightA | AttackKind::LightB | AttackKind::LightC => 0.13,
                        AttackKind::Heavy => 0.16,
                        AttackKind::Dimensional => 0.15,
                    };
                    self.state_timer = active_dur;
                    self.state_duration = active_dur;
                    out.swing_started = Some(kind);

                    if kind == AttackKind::Dimensional {
                        let fwd = look_dir.normalize_or_zero();
                        let right = fwd.cross(Vec3::Y).normalize_or_zero();
                        self.dimensional_rift_intensity = 1.0;
                        let mut plane = Self::build_cut_plane(
                            eye, look_dir, self.world_cut_angle,
                            cfg.dimensional_range, cfg.dimensional_width,
                        );
                        // The old plane passed through camera height; short walls
                        // and roofs often lay entirely on one side, so nothing cut.
                        // Aim at sliceable geometry and cut through its volume.
                        if let Some(hit) = raycast_world(colliders, eye, fwd,
                            cfg.dimensional_range, CollisionLayer::SLICEABLE_WORLD) {
                            if let Some(target) = colliders.iter().find(|c| c.id == hit.collider_id) {
                                plane.origin = target.bounds.center();
                            }
                        }
                        out.committed_world_cut = Some(plane);
                        self.active_rifts.push(DimensionalRiftWave {
                            origin: eye + fwd * 0.8,
                            position: eye + fwd * 0.8,
                            direction: fwd,
                            right,
                            traveled: 0.0,
                            max_range: cfg.dimensional_range,
                            width: cfg.dimensional_width,
                            hit_ids: Vec::new(),
                        });
                        out.dimensional_launched = true;
                    } // Only the explicit Dimensional/World-Cut control may cut architecture.
                }
            }
            AttackState::Active => {
                let kind = self.current_attack.unwrap_or(AttackKind::LightA);
                let (base, tip) = self.current_blade_segment(eye, look_dir);

                self.active_trails.push(SlashTrailSegment {
                    base,
                    tip,
                    remaining_secs: 0.20,
                    max_secs: 0.20,
                    is_dimensional: kind == AttackKind::Dimensional,
                });

                if kind != AttackKind::Dimensional {
                    self.perform_blade_sweep(
                        kind,
                        eye,
                        look_dir,
                        base,
                        tip,
                        aim_assist,
                        colliders,
                        marked_enemy_ids,
                        cfg,
                        &mut out,
                    );
                }
                self.prev_blade_tip = Some(tip);

                if self.state_timer <= 0.0 {
                    self.state = AttackState::Recovery;
                    let rec = match kind {
                        AttackKind::LightA | AttackKind::LightB | AttackKind::LightC => 0.14,
                        AttackKind::Heavy => 0.24,
                        AttackKind::Dimensional => 0.22,
                    };
                    self.state_timer = rec;
                    self.state_duration = rec;
                    if matches!(kind, AttackKind::LightA | AttackKind::LightB | AttackKind::LightC)
                    {
                        self.combo_step = self.combo_step.wrapping_add(1);
                    }
                }
            }
            AttackState::Recovery => {
                if self.state_timer <= 0.0 {
                    if let Some(next) = self.buffered_attack.take() {
                        self.start_windup(next, cfg);
                    } else {
                        self.state = AttackState::Idle;
                        self.current_attack = None;
                        self.combo_step = 0;
                    }
                }
            }
            _ => {}
        }

        out
    }

    #[allow(clippy::too_many_arguments)]
    fn perform_blade_sweep(
        &mut self,
        kind: AttackKind,
        eye: Vec3,
        look_dir: Vec3,
        base: Vec3,
        tip: Vec3,
        aim_assist: bool,
        colliders: &[Collider],
        marked_enemy_ids: &[u64],
        cfg: &SlashConfig,
        out: &mut CombatStepOutput,
    ) {
        let fwd = look_dir.normalize_or_zero();
        let blade_vec = tip - base;
        let blade_len = blade_vec.length().max(0.1);
        if let Some(wall_hit) = raycast_world(
            colliders,
            base,
            blade_vec / blade_len,
            blade_len.min(cfg.melee_reach),
            CollisionLayer::STRUCTURAL,
        ) {
            out.wall_clang_point = Some(wall_hit.point);
            // Combat sword contacts produce sparks only. Never modify building meshes.
            self.spawn_sparks(wall_hit.point, wall_hit.normal, 4, false);
        }

        let base_damage = match kind {
            AttackKind::Heavy => cfg.heavy_damage,
            _ => cfg.light_damage,
        };
        let knockback_mag = if kind == AttackKind::Heavy { 9.5 } else { 5.0 };

        let mut samples = [Vec3::ZERO; 6];
        for i in 0..5 {
            let t = i as f32 / 4.0;
            samples[i] = base.lerp(tip, t);
        }
        samples[5] = self.prev_blade_tip.map_or(tip, |p| p.lerp(tip, 0.5));

        for col in colliders {
            if !col.active || !col.layers.intersects(cfg.allowed_target_layers) {
                continue;
            }
            if self.hit_ids_this_swing.contains(&col.id) {
                continue;
            }

            let center = col.bounds.center();
            let to_target = center - eye;
            let dist = to_target.length();
            if dist > cfg.melee_reach + 1.2 {
                continue;
            }

            let mut touched = samples.iter().any(|&pt| {
                let closest = col.bounds.closest_point(pt);
                closest.distance(pt) <= cfg.blade_sweep_radius
            });

            if !touched && aim_assist && dist <= cfg.melee_reach {
                let dir = to_target.normalize_or_zero();
                if dir.dot(fwd) >= cfg.aim_assist_cone_cos {
                    touched = true;
                }
            }

            if !touched {
                continue;
            }

            let dir_to_target = to_target.normalize_or_zero();
            if let Some(wall) = raycast_world(
                colliders,
                eye,
                dir_to_target,
                (dist - 0.25).max(0.05),
                CollisionLayer::STRUCTURAL,
            ) {
                if wall.distance < dist - 0.15 {
                    continue;
                }
            }

            self.hit_ids_this_swing.push(col.id);
            let is_marked = marked_enemy_ids.contains(&col.id);
            let final_damage = if is_marked {
                base_damage * cfg.paint_mark_multiplier
            } else {
                base_damage
            };

            self.hit_stop_timer = if kind == AttackKind::Heavy { 0.065 } else { 0.045 };
            let hit_point = col.bounds.closest_point(tip);
            self.spawn_sparks(hit_point, -fwd, 8, true);

            out.hits.push(SlashHitResult {
                collider_id: col.id,
                kind,
                damage: final_damage,
                hit_point,
                knockback: fwd * knockback_mag + Vec3::Y * 2.5,
                triggers_split_vfx: kind == AttackKind::Heavy
                    || col.layers.contains(CollisionLayer::SLASHABLE_PROP),
                is_prop: col.layers.contains(CollisionLayer::SLASHABLE_PROP),
            });
        }
    }

    fn step_dimensional_rifts(
        &mut self,
        dt: f32,
        colliders: &[Collider],
        marked_enemy_ids: &[u64],
        cfg: &SlashConfig,
        out: &mut CombatStepOutput,
    ) {
        let step_dist = cfg.dimensional_speed * dt;
        let mut new_sparks = Vec::new();

        for wave in &mut self.active_rifts {
            let prev_pos = wave.position;
            let next_pos = prev_pos + wave.direction * step_dist;
            wave.position = next_pos;
            wave.traveled += step_dist;

            let half = Vec3::new(
                wave.width * 0.5 + step_dist * 0.5,
                1.4,
                wave.width * 0.5 + step_dist * 0.5,
            );
            let swept_box = Aabb::from_center_half_extents(prev_pos.lerp(next_pos, 0.5), half);

            for col in colliders {
                if !col.active || !col.layers.intersects(CollisionLayer::SLASHABLE_ENEMY) {
                    continue;
                }
                if wave.hit_ids.contains(&col.id) {
                    continue;
                }
                if swept_box.intersects_aabb(&col.bounds) {
                    wave.hit_ids.push(col.id);
                    let is_marked = marked_enemy_ids.contains(&col.id);
                    let dmg = if is_marked {
                        cfg.dimensional_damage * cfg.paint_mark_multiplier
                    } else {
                        cfg.dimensional_damage
                    };
                    let hit_pt = col.bounds.center();
                    new_sparks.push((hit_pt, -wave.direction));
                    out.hits.push(SlashHitResult {
                        collider_id: col.id,
                        kind: AttackKind::Dimensional,
                        damage: dmg,
                        hit_point: hit_pt,
                        knockback: wave.direction * 14.0 + Vec3::Y * 3.5,
                        triggers_split_vfx: true,
                        is_prop: false,
                    });
                }
            }
        }

        for (pt, n) in new_sparks {
            self.spawn_sparks(pt, n, 10, true);
        }
        self.active_rifts.retain(|w| w.traveled < w.max_range);
    }

    fn spawn_sparks(&mut self, origin: Vec3, normal: Vec3, count: usize, orange: bool) {
        for i in 0..count {
            let angle = (i as f32 / count.max(1) as f32) * std::f32::consts::TAU;
            let spread = Vec3::new(
                angle.cos() * 3.2,
                2.0 + (i % 3) as f32 * 1.1,
                angle.sin() * 3.2,
            );
            self.active_sparks.push(ImpactSpark {
                position: origin,
                velocity: normal * 3.0 + spread,
                remaining_secs: 0.28,
                is_orange_accent: orange || i % 2 == 0,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_core::FIXED_DT;

    #[test]
    fn enemy_sword_buttons_never_emit_world_cut_planes() {
        let cfg = SlashConfig::default();
        let collider = Collider {
            id: 501, surface_id: 501, cell_id: (0, 0),
            bounds: Aabb::new(Vec3::new(-1.0, 11.0, -3.0),
                Vec3::new(1.0, 15.0, -1.0)),
            layers: CollisionLayer::STRUCTURAL.union(CollisionLayer::SLICEABLE_WORLD)
                .union(CollisionLayer::SLASHABLE_PROP),
            active: true,
        };
        for kind in [AttackKind::LightA, AttackKind::Heavy] {
            let mut combat = CombatState::default();
            assert!(combat.request_attack(kind, 0.9, false, &cfg));
            for _ in 0..75 {
                let out = combat.step(FIXED_DT, Vec3::new(0.0, 13.0, 1.0),
                    Vec3::NEG_Z, false, 0.0, false, 0.9, false, true,
                    &[collider.clone()], &[], &cfg);
                assert!(out.committed_world_cut.is_none(),
                    "Normal combat button must never cut the environment");
            }
        }
    }

    #[test]
    fn dimensional_attack_emits_real_world_cut_plane() {
        let cfg = SlashConfig::default();
        let mut combat = CombatState::default();
        assert!(combat.request_attack(AttackKind::Dimensional, 0.25, false, &cfg));

        let mut got_plane = None;
        for _ in 0..20 {
            let res = combat.step(
                FIXED_DT,
                Vec3::new(0.0, 13.0, 2.0),
                Vec3::NEG_Z,
                false,
                0.0,
                false,
                0.25,
                false,
                true,
                &[],
                &[],
                &cfg,
            );
            if res.committed_world_cut.is_some() {
                got_plane = res.committed_world_cut;
            }
        }
        assert!(got_plane.is_some(), "World-Cut attack must commit a real 3D CutPlane");
    }
}
