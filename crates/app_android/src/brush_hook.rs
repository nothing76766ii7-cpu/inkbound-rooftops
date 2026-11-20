//! Ink-web grapple: assisted acquisition, a taut swing rope, steering and reeling.
//! The anchor is a real world hit, never a fabricated point in empty space.
use engine_core::{raycast_world, Collider, CollisionLayer, RayHit};
use gameplay_player::PlayerState;
use glam::{Vec2, Vec3};

const MAX_RANGE: f32 = 52.0;
const MIN_RANGE: f32 = 2.5;
const PIGMENT_COST: f32 = 12.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookAction { Fired, Detached, Missed }

#[derive(Debug, Clone)]
pub struct BrushHook {
    pub anchor: Option<Vec3>,
    pub anchor_id: Option<u64>,
    pub anchor_local: Vec3,
    pub remaining: f32,
    pub cooldown: f32,
    pub rope_length: f32,
    pub tension: f32,
    pub reel_velocity: f32,
}

impl Default for BrushHook {
    fn default() -> Self {
        Self { anchor: None, anchor_id: None, anchor_local: Vec3::ZERO,
            remaining: 0.0, cooldown: 0.0,
            rope_length: 0.0, tension: 0.0, reel_velocity: 0.0 }
    }
}

impl BrushHook {
    /// Cone-assisted aim: try the crosshair first, then small offsets up/left/right.
    /// Floors at player level are rejected, but elevated roof edges are valid.
    fn find_target(eye: Vec3, forward: Vec3, colliders: &[Collider]) -> Option<RayHit> {
        let mask = CollisionLayer::STRUCTURAL.union(CollisionLayer::PAINTABLE);
        let fwd = forward.normalize_or_zero();
        if fwd.length_squared() < 0.5 { return None; }
        let right = fwd.cross(Vec3::Y).normalize_or_zero();
        let up = right.cross(fwd).normalize_or_zero();
        let dirs = [fwd, (fwd + up * 0.085).normalize_or_zero(),
            (fwd + right * 0.075).normalize_or_zero(),
            (fwd - right * 0.075).normalize_or_zero(),
            (fwd + up * 0.16).normalize_or_zero()];
        let mut best: Option<(f32, RayHit)> = None;
        for (idx, dir) in dirs.iter().enumerate() {
            if let Some(hit) = raycast_world(colliders, eye, *dir, MAX_RANGE, mask) {
                if hit.distance < MIN_RANGE { continue; }
                let wall = hit.normal.y.abs() < 0.70;
                let elevated_roof = hit.normal.y > 0.7 && hit.point.y > eye.y + 1.6;
                if !wall && !elevated_roof { continue; }
                // Prefer the true center hit, but accept near-crosshair architecture.
                let score = idx as f32 * 4.0 + hit.distance * 0.025;
                if best.as_ref().map_or(true, |(s, _)| score < *s) {
                    best = Some((score, hit));
                }
            }
        }
        best.map(|(_, hit)| hit)
    }

    pub fn can_target(&self, eye: Vec3, forward: Vec3, colliders: &[Collider]) -> bool {
        self.anchor.is_some() || (self.cooldown <= 0.0 && Self::find_target(eye, forward, colliders).is_some())
    }

    /// Tap to attach; tap again to launch away with carried swing momentum.
    pub fn activate(&mut self, eye: Vec3, forward: Vec3, colliders: &[Collider],
        player: &mut PlayerState) -> HookAction {
        if self.anchor.is_some() {
            self.detach();
            return HookAction::Detached;
        }
        if self.cooldown > 0.0 || player.pigment < PIGMENT_COST { return HookAction::Missed; }
        if let Some(hit) = Self::find_target(eye, forward, colliders) {
            let anchor = hit.point + hit.normal * 0.16;
            let distance = (anchor - eye).length();
            self.anchor = Some(anchor);
            self.anchor_id = Some(hit.collider_id);
            self.anchor_local = colliders.iter().find(|c| c.id == hit.collider_id)
                .map(|c| anchor - c.bounds.center()).unwrap_or(Vec3::ZERO);
            self.remaining = 9.0;
            self.rope_length = (distance * 0.89).clamp(3.0, MAX_RANGE);
            self.tension = 0.0;
            self.reel_velocity = 0.0;
            player.pigment -= PIGMENT_COST;
            let dir = (anchor - eye).normalize_or_zero();
            // The initial impulse creates an immediate and readable hook launch.
            player.velocity += (dir + Vec3::Y * 0.24).normalize_or_zero() * 7.5;
            player.grounded = false;
            player.camera_kick = player.camera_kick.max(0.19);
            HookAction::Fired
        } else { HookAction::Missed }
    }

    pub fn detach(&mut self) {
        self.anchor = None;
        self.anchor_id = None;
        self.anchor_local = Vec3::ZERO;
        self.cooldown = 0.20;
        self.tension = 0.0;
    }

    /// Apply before player integration. Radial impulses keep the cable taut while
    /// tangential motion is preserved for real pendulum swings. Sprint reels in.
    /// Joystick steers the swing. Jump detaches with a boosted launch.
    pub fn tick(&mut self, player: &mut PlayerState, move_vec: Vec2,
        reel_in: bool, jump: bool, colliders: &[Collider], dt: f32) -> bool {
        self.cooldown = (self.cooldown - dt).max(0.0);
        let previous_anchor = match self.anchor { Some(a) => a, None => return false };
        self.remaining -= dt;
        let target = self.anchor_id.and_then(|id|
            colliders.iter().find(|c| c.id == id && c.active));
        let anchor = match target {
            Some(col) => col.bounds.center() + self.anchor_local,
            None => { self.detach(); return true; },
        };
        if anchor.distance(previous_anchor) > 5.0 { self.detach(); return true; }
        self.anchor = Some(anchor);
        let from_eye = player.position + Vec3::Y * 1.35;
        let delta = anchor - from_eye;
        let distance = delta.length();
        if jump {
            let tangent = player.velocity - delta.normalize_or_zero() * player.velocity.dot(delta.normalize_or_zero());
            player.velocity = tangent + player.forward_xz() * 2.4 + Vec3::Y * 5.0;
            self.detach();
            return true;
        }
        if self.remaining <= 0.0 || distance > MAX_RANGE + 8.0 {
            self.detach();
            return true;
        }
        let dir = delta / distance.max(0.001);
        if reel_in {
            self.reel_velocity = (self.reel_velocity + 32.0 * dt).min(12.0);
            self.rope_length = (self.rope_length - self.reel_velocity * dt).max(2.8);
        } else {
            self.reel_velocity = (self.reel_velocity - 24.0 * dt).max(0.0);
        }
        let stretch = (distance - self.rope_length).max(0.0);
        let outward_speed = (-player.velocity.dot(dir)).max(0.0);
        self.tension = (stretch / 1.1).clamp(0.0, 1.0);
        if stretch > 0.0 {
            // Project away outward radial velocity: the string is inextensible.
            // This removes the elastic rubber-band effect, preserves tangential
            // swing, and prevents the camera from outrunning its drawn rope.
            if outward_speed > 0.0 {
                player.velocity += dir * outward_speed * 0.96;
            }
            let correction = (stretch - 0.10).max(0.0).min(1.2);
            if correction > 0.0 {
                // Don't pull the player capsule into a solid wall.
                let proposed = player.position + dir * correction;
                let inside_wall = colliders.iter().any(|col| {
                    if !col.active || !col.layers.is_solid() { return false; }
                    let p = proposed + Vec3::Y * 0.9;
                    p.x > col.bounds.min.x + 0.25 && p.x < col.bounds.max.x - 0.25
                        && p.y > col.bounds.min.y && p.y < col.bounds.max.y
                        && p.z > col.bounds.min.z + 0.25 && p.z < col.bounds.max.z - 0.25
                });
                if !inside_wall { player.position = proposed; }
            }
            // Gentle pull while taut; excessive spring force caused jitter.
            player.velocity += dir * (stretch * 7.5).min(25.0) * dt;
        }
        // Wind-up in direction of movement, projected perpendicular to rope.
        let steer = player.forward_xz() * move_vec.y + player.right_xz() * move_vec.x;
        let tangent = steer - dir * steer.dot(dir);
        player.velocity += tangent.normalize_or_zero() * move_vec.length().min(1.0) * 12.0 * dt;
        // Maintain some momentum and lift while reeling so grappling can reach rooftops.
        if reel_in && distance > 4.0 { player.velocity.y += 6.0 * dt; }
        player.velocity = player.velocity.clamp_length_max(31.0);
        player.grounded = false;
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_core::Aabb;
    #[test]
    fn taut_web_prevents_runaway_rope_stretch() {
        let col = Collider { id: 19, surface_id: 19, cell_id: (0, 0), active: true,
            layers: CollisionLayer::STRUCTURAL,
            bounds: Aabb::new(Vec3::new(-2.0, 12.0, -13.0), Vec3::new(2.0, 17.0, -11.0)) };
        let mut player = PlayerState::default();
        player.position = Vec3::new(0.0, 11.4, 0.0);
        player.velocity = Vec3::new(0.0, 0.0, 20.0);
        let mut hook = BrushHook::default();
        hook.anchor = Some(Vec3::new(0.0, 15.0, -11.0));
        hook.anchor_id = Some(19);
        hook.anchor_local = hook.anchor.unwrap() - col.bounds.center();
        hook.rope_length = 10.0;
        hook.remaining = 9.0;
        hook.tick(&mut player, Vec2::ZERO, false, false, &[col], 1.0 / 60.0);
        assert!(player.velocity.z < 20.0);
        assert!(player.position.is_finite());
    }

    #[test]
    fn can_grapple_elevated_roof_and_detach() {
        let col = Collider { id: 12, surface_id: 12, cell_id: (0,0), active: true,
            layers: CollisionLayer::STRUCTURAL, bounds: Aabb::new(Vec3::new(-3., 8., -12.), Vec3::new(3., 10., -6.)) };
        let eye = Vec3::new(0., 5., 0.);
        let dir = Vec3::new(0., 5., -8.).normalize();
        let mut hook = BrushHook::default();
        let mut player = PlayerState::default();
        player.position = eye - Vec3::Y * 1.35;
        player.pigment = 100.0;
        assert!(hook.can_target(eye, dir, &[col.clone()]));
        assert_eq!(hook.activate(eye, dir, &[col.clone()], &mut player), HookAction::Fired);
        assert!(hook.rope_length > 2.8);
        assert_eq!(hook.activate(eye, dir, &[col], &mut player), HookAction::Detached);
    }
}
