//! Ink Wraith enemy AI archetype (PRD section 3.9).
//!
//! Full state machine: Idle, Patrol, Investigate, Chase, TelegraphAttack, Attack,
//! Stagger, Recover, Defeated, Reset. Supports paint-mark vulnerability and
//! dimensional-slash visual split animation.

use engine_core::{Aabb, Collider, CollisionLayer};
use glam::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WraithState {
    Idle,
    Patrol,
    Investigate,
    Chase,
    TelegraphAttack,
    Attack,
    Stagger,
    Recover,
    Defeated,
    Reset,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InkWraith {
    pub id: u64,
    pub cell_id: (i32, i32),
    pub position: Vec3,
    pub spawn_position: Vec3,
    pub patrol_a: Vec3,
    pub patrol_b: Vec3,
    pub patrol_to_b: bool,
    pub health: f32,
    pub max_health: f32,
    pub state: WraithState,
    pub state_timer: f32,
    pub paint_marked_timer: f32,
    /// Visual split separation `[0.0, 1.5]` meters when defeated by Dimensional Slash.
    pub split_offset: f32,
    pub attack_cooldown: f32,
}

impl InkWraith {
    pub fn new(id: u64, cell_id: (i32, i32), spawn: Vec3, patrol_a: Vec3, patrol_b: Vec3) -> Self {
        Self {
            id,
            cell_id,
            position: spawn,
            spawn_position: spawn,
            patrol_a,
            patrol_b,
            patrol_to_b: true,
            health: 120.0,
            max_health: 120.0,
            state: WraithState::Patrol,
            state_timer: 0.0,
            paint_marked_timer: 0.0,
            split_offset: 0.0,
            attack_cooldown: 0.0,
        }
    }

    pub fn is_marked(&self) -> bool {
        self.paint_marked_timer > 0.0
    }

    pub fn apply_paint_mark(&mut self, duration_secs: f32) {
        if self.state != WraithState::Defeated {
            self.paint_marked_timer = self.paint_marked_timer.max(duration_secs);
        }
    }

    pub fn take_damage(&mut self, damage: f32, knockback: Vec3, split_vfx: bool) {
        if self.state == WraithState::Defeated {
            return;
        }
        self.health = (self.health - damage.max(0.0)).max(0.0);
        self.position += Vec3::new(knockback.x, 0.0, knockback.z) * 0.08;
        if self.health <= 0.0 {
            self.state = WraithState::Defeated;
            self.state_timer = 0.0;
            if split_vfx {
                self.split_offset = 0.05;
            }
        } else {
            self.state = WraithState::Stagger;
            self.state_timer = 0.45;
        }
    }

    pub fn reset(&mut self) {
        self.position = self.spawn_position;
        self.health = self.max_health;
        self.state = WraithState::Patrol;
        self.state_timer = 0.0;
        self.paint_marked_timer = 0.0;
        self.split_offset = 0.0;
        self.attack_cooldown = 0.0;
    }

    pub fn collider(&self) -> Collider {
        Collider {
            id: self.id,
            surface_id: self.id as u32,
            cell_id: self.cell_id,
            bounds: Aabb::from_center_half_extents(
                self.position + Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(0.55, 1.0, 0.55),
            ),
            layers: CollisionLayer::SLASHABLE_ENEMY,
            active: self.state != WraithState::Defeated,
        }
    }

    /// Advances AI simulation; returns damage dealt to the player this tick (if any).
    pub fn tick(&mut self, player_pos: Vec3, dt: f32) -> f32 {
        self.paint_marked_timer = (self.paint_marked_timer - dt).max(0.0);
        self.attack_cooldown = (self.attack_cooldown - dt).max(0.0);

        let to_player = player_pos - self.position;
        let dist = to_player.length();

        // Variable rate / sleep culling far from player
        if dist > 65.0 && self.state != WraithState::Defeated {
            self.state = WraithState::Idle;
            return 0.0;
        }

        match self.state {
            WraithState::Idle => {
                if dist < 24.0 {
                    self.state = WraithState::Investigate;
                } else {
                    self.state = WraithState::Patrol;
                }
            }
            WraithState::Patrol => {
                if dist < 18.0 {
                    self.state = WraithState::Chase;
                    return 0.0;
                }
                let target = if self.patrol_to_b {
                    self.patrol_b
                } else {
                    self.patrol_a
                };
                let step = target - self.position;
                let flat = Vec3::new(step.x, 0.0, step.z);
                if flat.length() < 0.6 {
                    self.patrol_to_b = !self.patrol_to_b;
                } else {
                    self.position += flat.normalize() * 2.4 * dt;
                }
            }
            WraithState::Investigate => {
                if dist < 16.0 {
                    self.state = WraithState::Chase;
                } else if dist > 28.0 {
                    self.state = WraithState::Reset;
                }
            }
            WraithState::Chase => {
                if dist > 26.0 {
                    self.state = WraithState::Reset;
                    return 0.0;
                }
                if dist <= 2.4 && self.attack_cooldown <= 0.0 {
                    self.state = WraithState::TelegraphAttack;
                    self.state_timer = 0.55;
                    return 0.0;
                }
                let dir = Vec3::new(to_player.x, 0.0, to_player.z).normalize_or_zero();
                self.position += dir * 4.4 * dt;
            }
            WraithState::TelegraphAttack => {
                self.state_timer -= dt;
                if self.state_timer <= 0.0 {
                    self.state = WraithState::Attack;
                    self.state_timer = 0.15;
                }
            }
            WraithState::Attack => {
                self.state_timer -= dt;
                if self.state_timer <= 0.0 {
                    self.state = WraithState::Recover;
                    self.state_timer = 0.70;
                    self.attack_cooldown = 1.4;
                    if dist <= 3.0 {
                        return 14.0;
                    }
                }
            }
            WraithState::Stagger => {
                self.state_timer -= dt;
                if self.state_timer <= 0.0 {
                    self.state = WraithState::Chase;
                }
            }
            WraithState::Recover => {
                self.state_timer -= dt;
                if self.state_timer <= 0.0 {
                    self.state = WraithState::Chase;
                }
            }
            WraithState::Defeated => {
                if self.split_offset > 0.0 {
                    self.split_offset = (self.split_offset + dt * 1.4).min(1.2);
                }
            }
            WraithState::Reset => {
                let back = self.spawn_position - self.position;
                let flat = Vec3::new(back.x, 0.0, back.z);
                if flat.length() < 0.8 {
                    self.state = WraithState::Patrol;
                } else {
                    self.position += flat.normalize() * 4.0 * dt;
                }
            }
        }
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraith_transitions_chase_telegraph_attack_and_defeat() {
        let mut wraith = InkWraith::new(
            1,
            (0, -1),
            Vec3::new(0.0, 13.6, -60.0),
            Vec3::new(-4.0, 13.6, -60.0),
            Vec3::new(4.0, 13.6, -60.0),
        );
        // Player enters close range
        wraith.tick(Vec3::new(0.0, 13.6, -58.0), 0.016);
        assert_eq!(wraith.state, WraithState::Chase);

        wraith.tick(Vec3::new(0.0, 13.6, -58.5), 0.016);
        assert_eq!(wraith.state, WraithState::TelegraphAttack);

        // Mark and defeat with dimensional slash
        wraith.apply_paint_mark(6.0);
        assert!(wraith.is_marked());
        wraith.take_damage(150.0, Vec3::NEG_Z, true);
        assert_eq!(wraith.state, WraithState::Defeated);
        assert!(!wraith.collider().active);
    }
}
