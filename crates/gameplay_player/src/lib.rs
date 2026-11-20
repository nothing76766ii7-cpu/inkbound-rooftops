//! First-person parkour movement controller & locomotion state graph (PRD v2.0 section 4.4).
//!
//! Provides an explicit `LocomotionState` machine, fixed-timestep movement, coyote time,
//! jump buffering, wall-run/wall-jump, slide, dash, mantle, landing compression dip,
//! dynamic speed FOV/distortion factor, and automatic fall rescue.

use engine_core::{raycast_world, Capsule, Collider, CollisionLayer};
use glam::{Mat4, Vec3};
use platform_input::InputFrame;
use serde::{Deserialize, Serialize};

/// Explicit locomotion state graph (PRD 4.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LocomotionState {
    #[default]
    Grounded,
    Ascending,
    Falling,
    Sliding,
    WallRunning,
    Mantling,
    Disabled,
}

/// Unlocked progression abilities (PRD section 3.10).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerAbilities {
    pub double_jump_unlocked: bool,
    pub dash_cooldown_scale: f32,
    pub wall_run_stamina_scale: f32,
}

impl Default for PlayerAbilities {
    fn default() -> Self {
        Self {
            double_jump_unlocked: false,
            dash_cooldown_scale: 1.0,
            wall_run_stamina_scale: 1.0,
        }
    }
}

/// Persistent + runtime player state (PRD section 3.15 & 4.4).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerState {
    pub position: Vec3,
    pub prev_position: Vec3,
    pub velocity: Vec3,
    /// Radians around +Y (0 faces -Z).
    pub yaw: f32,
    /// Radians up/down clamped to `[-1.45, 1.45]`.
    pub pitch: f32,
    pub health: f32,
    pub max_health: f32,
    pub stamina: f32,
    pub max_stamina: f32,
    pub pigment: f32,
    pub max_pigment: f32,
    pub checkpoint_position: Vec3,
    pub checkpoint_id: String,
    pub abilities: PlayerAbilities,
    pub locomotion: LocomotionState,
    pub grounded: bool,
    pub sliding: bool,
    pub wall_running: bool,
    pub wall_normal: Vec3,
    pub wall_run_timer: f32,
    pub coyote_timer: f32,
    pub jump_buffer_timer: f32,
    pub double_jump_available: bool,
    pub dash_cooldown: f32,
    pub dash_active_timer: f32,
    pub mantle_timer: f32,
    pub mantle_target: Vec3,
    pub bob_phase: f32,
    pub paint_speed_buff_timer: f32,
    /// Smooth landing compression offset `[0.0, 0.25]` meters.
    pub landing_dip: f32,
    /// Normalized speed intensity `[0.0, 1.0]` driving subtle FOV shift and edge speed distortion.
    pub speed_factor: f32,
    /// Camera recoil/impulse from slashes & landings.
    pub camera_kick: f32,
}

impl Default for PlayerState {
    fn default() -> Self {
        let spawn = Vec3::new(0.0, 11.4, 3.0);
        Self {
            position: spawn,
            prev_position: spawn,
            velocity: Vec3::ZERO,
            yaw: 0.0,
            pitch: -0.05,
            health: 100.0,
            max_health: 100.0,
            stamina: 100.0,
            max_stamina: 100.0,
            pigment: 100.0,
            max_pigment: 100.0,
            checkpoint_position: spawn,
            checkpoint_id: "cp_tutorial_start".to_string(),
            abilities: PlayerAbilities::default(),
            locomotion: LocomotionState::Grounded,
            grounded: false,
            sliding: false,
            wall_running: false,
            wall_normal: Vec3::ZERO,
            wall_run_timer: 0.0,
            coyote_timer: 0.0,
            jump_buffer_timer: 0.0,
            double_jump_available: false,
            dash_cooldown: 0.0,
            dash_active_timer: 0.0,
            mantle_timer: 0.0,
            mantle_target: Vec3::ZERO,
            bob_phase: 0.0,
            paint_speed_buff_timer: 0.0,
            landing_dip: 0.0,
            speed_factor: 0.0,
            camera_kick: 0.0,
        }
    }
}

/// Movement tuning parameters (PRD 4.4).
#[derive(Debug, Clone)]
pub struct MovementConfig {
    pub capsule_radius: f32,
    pub stand_height: f32,
    pub slide_height: f32,
    pub walk_speed: f32,
    pub sprint_speed: f32,
    pub slide_max_speed: f32,
    pub dash_speed: f32,
    pub dash_duration: f32,
    pub dash_base_cooldown: f32,
    pub ground_accel: f32,
    pub air_accel: f32,
    pub gravity: f32,
    pub wall_run_gravity: f32,
    pub jump_velocity: f32,
    pub coyote_time: f32,
    pub jump_buffer_time: f32,
    pub max_wall_run_secs: f32,
    pub fall_rescue_y: f32,
}

impl Default for MovementConfig {
    fn default() -> Self {
        Self {
            capsule_radius: 0.38,
            stand_height: 1.78,
            slide_height: 1.05,
            walk_speed: 6.4,
            sprint_speed: 10.2,
            slide_max_speed: 13.8,
            dash_speed: 18.5,
            dash_duration: 0.16,
            dash_base_cooldown: 1.25,
            ground_accel: 56.0,
            air_accel: 18.0,
            gravity: 24.0,
            wall_run_gravity: 4.2,
            jump_velocity: 9.4,
            coyote_time: 0.12,
            jump_buffer_time: 0.13,
            max_wall_run_secs: 1.8,
            fall_rescue_y: -12.0,
        }
    }
}

/// Events emitted during a simulation step.
#[derive(Debug, Clone, Default)]
pub struct PlayerStepEvents {
    pub jumped: bool,
    pub wall_jumped: bool,
    pub landed_impact: f32,
    pub dashed: bool,
    pub mantled: bool,
    pub rescued_to_checkpoint: bool,
    pub footstep_triggered: bool,
}

impl PlayerState {
    pub fn forward_xz(&self) -> Vec3 {
        Vec3::new(-self.yaw.sin(), 0.0, -self.yaw.cos()).normalize_or_zero()
    }

    pub fn right_xz(&self) -> Vec3 {
        Vec3::new(self.yaw.cos(), 0.0, -self.yaw.sin()).normalize_or_zero()
    }

    pub fn look_direction(&self) -> Vec3 {
        let cp = self.pitch.cos();
        Vec3::new(-self.yaw.sin() * cp, self.pitch.sin(), -self.yaw.cos() * cp).normalize_or_zero()
    }

    pub fn eye_height(&self) -> f32 {
        if self.sliding {
            0.88
        } else {
            1.58
        }
    }

    pub fn dynamic_fov_degrees(&self, base_fov: f32, reduced_motion: bool) -> f32 {
        if reduced_motion {
            base_fov
        } else {
            base_fov + self.speed_factor * 9.5 + self.camera_kick * 3.0
        }
    }

    pub fn eye_position(&self, alpha: f32, head_bob_strength: f32, reduced_motion: bool) -> Vec3 {
        let interp = self.prev_position.lerp(self.position, alpha.clamp(0.0, 1.0));
        let bob = if reduced_motion || !self.grounded || self.sliding {
            0.0
        } else {
            self.bob_phase.sin() * 0.045 * head_bob_strength
        };
        let dip = if reduced_motion { 0.0 } else { self.landing_dip };
        interp + Vec3::new(0.0, self.eye_height() + bob - dip, 0.0)
    }

    pub fn view_matrix(&self, alpha: f32, head_bob_strength: f32, reduced_motion: bool) -> Mat4 {
        let eye = self.eye_position(alpha, head_bob_strength, reduced_motion);
        let dir = self.look_direction();
        let roll = if reduced_motion {
            0.0
        } else if self.wall_running {
            self.right_xz().dot(self.wall_normal) * 0.13
        } else if self.sliding {
            -0.06
        } else {
            0.0
        };
        let up = if roll.abs() > 1e-4 {
            let right = dir.cross(Vec3::Y).normalize_or_zero();
            (Vec3::Y * roll.cos() + right * roll.sin()).normalize_or_zero()
        } else {
            Vec3::Y
        };
        Mat4::look_to_rh(eye, dir, up)
    }

    pub fn capsule(&self, cfg: &MovementConfig) -> Capsule {
        let h = if self.sliding {
            cfg.slide_height
        } else {
            cfg.stand_height
        };
        Capsule::new(self.position, cfg.capsule_radius, h)
    }

    pub fn apply_damage(&mut self, amount: f32) {
        self.health = (self.health - amount.max(0.0)).max(0.0);
        self.camera_kick = (self.camera_kick + 0.35).min(1.0);
        if self.health <= 0.0 {
            self.respawn_at_checkpoint();
        }
    }

    pub fn respawn_at_checkpoint(&mut self) {
        self.position = self.checkpoint_position;
        self.prev_position = self.checkpoint_position;
        self.velocity = Vec3::ZERO;
        self.health = self.max_health;
        self.stamina = self.max_stamina;
        self.wall_running = false;
        self.sliding = false;
        self.mantle_timer = 0.0;
        self.landing_dip = 0.0;
        self.locomotion = LocomotionState::Grounded;
    }

    /// Advances the player controller by one fixed timestep (`dt = 1/60`).
    pub fn step(
        &mut self,
        input: &InputFrame,
        colliders: &[Collider],
        cfg: &MovementConfig,
        dt: f32,
    ) -> PlayerStepEvents {
        let mut events = PlayerStepEvents::default();
        self.prev_position = self.position;

        if self.locomotion == LocomotionState::Disabled {
            return events;
        }

        // 1. Camera orientation & decay
        self.yaw += input.look_delta.x;
        self.pitch = (self.pitch + input.look_delta.y).clamp(-1.45, 1.45);
        self.landing_dip = (self.landing_dip - dt * 1.1).max(0.0);
        self.camera_kick = (self.camera_kick - dt * 3.5).max(0.0);

        // 2. Cooldowns & timers
        self.dash_cooldown = (self.dash_cooldown - dt).max(0.0);
        self.dash_active_timer = (self.dash_active_timer - dt).max(0.0);
        self.paint_speed_buff_timer = (self.paint_speed_buff_timer - dt).max(0.0);
        if input.jump_pressed {
            self.jump_buffer_timer = cfg.jump_buffer_time;
        } else {
            self.jump_buffer_timer = (self.jump_buffer_timer - dt).max(0.0);
        }

        // 3. Active mantle easing
        if self.mantle_timer > 0.0 {
            self.locomotion = LocomotionState::Mantling;
            self.mantle_timer = (self.mantle_timer - dt).max(0.0);
            let t = 1.0 - (self.mantle_timer / 0.22).clamp(0.0, 1.0);
            self.position = self.position.lerp(self.mantle_target, t.min(1.0));
            self.velocity = Vec3::ZERO;
            self.grounded = self.mantle_timer <= 0.0;
            if self.grounded {
                self.locomotion = LocomotionState::Grounded;
            }
            return events;
        }

        // 4. Desired horizontal velocity
        let fwd = self.forward_xz();
        let right = self.right_xz();
        let wish_dir = (fwd * input.move_vec.y + right * input.move_vec.x).normalize_or_zero();
        let buff_mult = if self.paint_speed_buff_timer > 0.0 {
            1.22
        } else {
            1.0
        };
        let target_speed = if input.sprint {
            cfg.sprint_speed * buff_mult
        } else {
            cfg.walk_speed * buff_mult
        };

        let horiz_speed = Vec3::new(self.velocity.x, 0.0, self.velocity.z).length();
        self.sliding = self.grounded && input.slide_held && horiz_speed > 4.2;
        if self.sliding {
            let dir = Vec3::new(self.velocity.x, 0.0, self.velocity.z).normalize_or_zero();
            let boosted = (horiz_speed + 2.6 * dt).min(cfg.slide_max_speed);
            self.velocity.x = dir.x * boosted * (1.0 - 0.60 * dt);
            self.velocity.z = dir.z * boosted * (1.0 - 0.60 * dt);
        } else if self.dash_active_timer <= 0.0 {
            let accel = if self.grounded {
                cfg.ground_accel
            } else {
                cfg.air_accel
            };
            let desired_xz = wish_dir * target_speed;
            let cur_xz = Vec3::new(self.velocity.x, 0.0, self.velocity.z);
            let diff = desired_xz - cur_xz;
            let max_delta = accel * dt;
            let delta = if diff.length() > max_delta {
                diff.normalize() * max_delta
            } else {
                diff
            };
            self.velocity.x += delta.x;
            self.velocity.z += delta.z;
        }

        // 5. Dash trigger
        if input.dash_pressed && self.dash_cooldown <= 0.0 && self.stamina >= 18.0 {
            let dash_dir = if wish_dir.length_squared() > 0.1 {
                wish_dir
            } else {
                fwd
            };
            self.velocity.x = dash_dir.x * cfg.dash_speed;
            self.velocity.z = dash_dir.z * cfg.dash_speed;
            self.velocity.y = self.velocity.y.max(0.6);
            self.dash_active_timer = cfg.dash_duration;
            self.dash_cooldown = cfg.dash_base_cooldown * self.abilities.dash_cooldown_scale;
            self.stamina = (self.stamina - 18.0).max(0.0);
            self.camera_kick = (self.camera_kick + 0.45).min(1.0);
            events.dashed = true;
        }

        // 6. Wall-run detection
        self.wall_running = false;
        if !self.grounded && self.velocity.y > -6.0 && horiz_speed > 4.8 && self.stamina > 8.0 {
            let chest = self.position + Vec3::new(0.0, 1.1, 0.0);
            for side_dir in [right, -right] {
                if let Some(hit) = raycast_world(
                    colliders,
                    chest,
                    side_dir,
                    cfg.capsule_radius + 0.48,
                    CollisionLayer::WALL_RUNNABLE,
                ) {
                    if hit.normal.y.abs() < 0.25 && self.wall_run_timer < cfg.max_wall_run_secs {
                        self.wall_running = true;
                        self.wall_normal = hit.normal;
                        self.wall_run_timer += dt;
                        self.stamina =
                            (self.stamina - 16.0 * dt / self.abilities.wall_run_stamina_scale)
                                .max(0.0);
                        self.velocity.y = self.velocity.y.max(-1.0);
                        break;
                    }
                }
            }
        }
        if self.grounded {
            self.wall_run_timer = 0.0;
            self.coyote_timer = cfg.coyote_time;
            self.double_jump_available = self.abilities.double_jump_unlocked;
            self.stamina = (self.stamina + 28.0 * dt).min(self.max_stamina);
        } else {
            self.coyote_timer = (self.coyote_timer - dt).max(0.0);
        }

        // 7. Jump / Wall-jump / Double-jump
        if self.jump_buffer_timer > 0.0 {
            if self.wall_running {
                let impulse = self.wall_normal * 8.4 + fwd * 4.8 + Vec3::Y * cfg.jump_velocity;
                self.velocity = impulse;
                self.wall_running = false;
                self.jump_buffer_timer = 0.0;
                events.wall_jumped = true;
            } else if self.coyote_timer > 0.0 {
                self.velocity.y = cfg.jump_velocity;
                self.grounded = false;
                self.coyote_timer = 0.0;
                self.jump_buffer_timer = 0.0;
                events.jumped = true;
            } else if self.double_jump_available {
                self.velocity.y = cfg.jump_velocity * 0.92;
                self.double_jump_available = false;
                self.jump_buffer_timer = 0.0;
                events.jumped = true;
            }
        }

        if !input.jump_held && self.velocity.y > 4.0 && !self.wall_running {
            self.velocity.y *= 0.85;
        }

        // 8. Gravity & integration
        let grav = if self.wall_running {
            cfg.wall_run_gravity
        } else {
            cfg.gravity
        };
        self.velocity.y -= grav * dt;

        let was_grounded = self.grounded;
        let pre_impact_vy = self.velocity.y;
        self.position += self.velocity * dt;

        // 9. Swept/iterative capsule-vs-solid collision resolution
        self.grounded = false;
        for _ in 0..3 {
            let cap = self.capsule(cfg);
            let mut resolved_any = false;
            for col in colliders {
                if !col.active || !col.layers.is_solid() {
                    continue;
                }
                if let Some((push, normal)) = cap.resolve_aabb(&col.bounds) {
                    self.position += push;
                    let vn = self.velocity.dot(normal);
                    if vn < 0.0 {
                        self.velocity -= normal * vn;
                    }
                    if normal.y > 0.65 {
                        self.grounded = true;
                    }
                    resolved_any = true;
                }
            }
            if !resolved_any {
                break;
            }
        }

        if !was_grounded && self.grounded && pre_impact_vy < -4.5 {
            let impact = (-pre_impact_vy / 18.0).clamp(0.0, 1.0);
            events.landed_impact = impact;
            self.landing_dip = (impact * 0.22).min(0.22);
        }

        // 10. Context vault/mantle assist on CLIMBABLE ledges
        if !self.grounded && input.move_vec.y > 0.35 && self.velocity.y > -4.5 {
            let chest = self.position + Vec3::new(0.0, 1.15, 0.0);
            let head = self.position + Vec3::new(0.0, 1.85, 0.0);
            if let Some(wall_hit) =
                raycast_world(colliders, chest, fwd, 0.90, CollisionLayer::CLIMBABLE)
            {
                let head_blocked =
                    raycast_world(colliders, head, fwd, 0.95, CollisionLayer::STRUCTURAL).is_some();
                if !head_blocked {
                    let probe_top = wall_hit.point + fwd * 0.45 + Vec3::Y * 1.15;
                    if let Some(top_hit) = raycast_world(
                        colliders,
                        probe_top,
                        Vec3::NEG_Y,
                        1.45,
                        CollisionLayer::STRUCTURAL.union(CollisionLayer::PAINT_PLATFORM),
                    ) {
                        self.mantle_target = top_hit.point + Vec3::Y * 0.05;
                        self.mantle_timer = 0.22;
                        self.velocity = Vec3::ZERO;
                        self.locomotion = LocomotionState::Mantling;
                        events.mantled = true;
                    }
                }
            }
        }

        // 11. Update locomotion state graph & smooth speed factor (drives FOV + edge distortion)
        self.locomotion = if self.mantle_timer > 0.0 {
            LocomotionState::Mantling
        } else if self.wall_running {
            LocomotionState::WallRunning
        } else if self.sliding {
            LocomotionState::Sliding
        } else if self.grounded {
            LocomotionState::Grounded
        } else if self.velocity.y > 0.2 {
            LocomotionState::Ascending
        } else {
            LocomotionState::Falling
        };

        let cur_horiz = Vec3::new(self.velocity.x, 0.0, self.velocity.z).length();
        let target_speed_factor = ((cur_horiz - cfg.walk_speed) / (cfg.dash_speed - cfg.walk_speed))
            .clamp(0.0, 1.0);
        self.speed_factor += (target_speed_factor - self.speed_factor) * (dt * 8.0).min(1.0);

        // 12. Footstep & head-bob phase
        if self.grounded && !self.sliding && cur_horiz > 1.5 {
            let prev_sin = self.bob_phase.sin();
            self.bob_phase += cur_horiz * dt * 1.35;
            if prev_sin >= 0.0 && self.bob_phase.sin() < 0.0 {
                events.footstep_triggered = true;
            }
        }

        // 13. Fall rescue to checkpoint
        if self.position.y < cfg.fall_rescue_y {
            self.respawn_at_checkpoint();
            events.rescued_to_checkpoint = true;
        }

        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_core::{Aabb, FIXED_DT};
    use glam::Vec2;

    #[test]
    fn locomotion_state_transitions_cleanly_on_jump_and_land() {
        let cfg = MovementConfig::default();
        let floor = Collider {
            id: 1,
            surface_id: 1,
            cell_id: (0, 0),
            bounds: Aabb::new(Vec3::new(-20.0, 9.0, -20.0), Vec3::new(20.0, 10.0, 20.0)),
            layers: CollisionLayer::STRUCTURAL,
            active: true,
        };
        let mut player = PlayerState {
            position: Vec3::new(0.0, 10.02, 0.0),
            grounded: true,
            coyote_timer: cfg.coyote_time,
            ..Default::default()
        };

        let mut jump = InputFrame::default();
        jump.jump_pressed = true;
        jump.jump_held = true;
        player.step(&jump, &[floor.clone()], &cfg, FIXED_DT);
        assert_eq!(player.locomotion, LocomotionState::Ascending);

        // Let player fall back to floor
        for _ in 0..90 {
            player.step(&InputFrame::default(), &[floor.clone()], &cfg, FIXED_DT);
        }
        assert_eq!(player.locomotion, LocomotionState::Grounded);
    }

    #[test]
    fn wall_run_and_wall_jump_impulse_away_from_wall() {
        let cfg = MovementConfig::default();
        let wall = Collider {
            id: 2,
            surface_id: 2,
            cell_id: (0, 0),
            bounds: Aabb::new(Vec3::new(0.7, 8.0, -15.0), Vec3::new(1.5, 16.0, 15.0)),
            layers: CollisionLayer::STRUCTURAL.union(CollisionLayer::WALL_RUNNABLE),
            active: true,
        };
        let mut player = PlayerState {
            position: Vec3::new(0.0, 11.0, 0.0),
            velocity: Vec3::new(0.0, 0.5, -8.5),
            grounded: false,
            ..Default::default()
        };

        let mut input = InputFrame::default();
        input.move_vec = Vec2::new(0.0, 1.0);
        input.sprint = true;
        player.step(&input, &[wall.clone()], &cfg, FIXED_DT);
        assert_eq!(player.locomotion, LocomotionState::WallRunning);

        input.jump_pressed = true;
        input.jump_held = true;
        let ev = player.step(&input, &[wall], &cfg, FIXED_DT);
        assert!(ev.wall_jumped);
        assert!(player.velocity.x < -5.0);
    }
}
