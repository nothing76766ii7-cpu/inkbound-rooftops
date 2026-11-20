//! V4 brush-hook traversal. A real physics-driven pull, not a cosmetic paint stroke.
use engine_core::{raycast_world, Collider, CollisionLayer};
use gameplay_player::PlayerState;
use glam::Vec3;

#[derive(Debug, Clone, Default)]
pub struct BrushHook {
    pub anchor: Option<Vec3>,
    pub remaining: f32,
    pub cooldown: f32,
}

impl BrushHook {
    pub fn can_target(&self, eye: Vec3, forward: Vec3, colliders: &[Collider]) -> bool {
        let mask = CollisionLayer::PAINTABLE.union(CollisionLayer::STRUCTURAL);
        raycast_world(colliders, eye, forward, 38.0, mask)
            .is_some_and(|hit| hit.normal.y.abs() <= 0.92)
    }

    /// Toggle a hook on the nearest paintable structural surface in the view ray.
    pub fn activate(&mut self, eye: Vec3, forward: Vec3, colliders: &[Collider], pigment: &mut f32) -> bool {
        if self.anchor.is_some() {
            self.anchor = None;
            return true;
        }
        if self.cooldown > 0.0 || *pigment < 18.0 { return false; }
        let mask = CollisionLayer::PAINTABLE.union(CollisionLayer::STRUCTURAL);
        if let Some(hit) = raycast_world(colliders, eye, forward, 38.0, mask) {
            if hit.normal.y.abs() > 0.92 { return false; } // wall only
            self.anchor = Some(hit.point + hit.normal * 0.12);
            self.remaining = 3.8;
            *pigment -= 18.0;
            return true;
        }
        false
    }

    /// Apply spring pull before player collision integration. Keeps sideways velocity for swinging.
    pub fn tick(&mut self, player: &mut PlayerState, dt: f32) {
        self.cooldown = (self.cooldown - dt).max(0.0);
        if let Some(anchor) = self.anchor {
            self.remaining -= dt;
            let delta = anchor - player.position;
            let distance = delta.length();
            if self.remaining <= 0.0 || distance < 1.65 || distance > 44.0 {
                self.anchor = None;
                self.cooldown = 0.35;
                return;
            }
            let dir = delta / distance;
            let radial_speed = player.velocity.dot(dir);
            let acceleration = ((distance - 2.0) * 15.0 - radial_speed * 4.0).clamp(-20.0, 62.0);
            player.velocity += dir * acceleration * dt;
            player.velocity = player.velocity.clamp_length_max(25.0);
            player.grounded = false;
            player.camera_kick = player.camera_kick.max(0.08);
        }
    }
}
