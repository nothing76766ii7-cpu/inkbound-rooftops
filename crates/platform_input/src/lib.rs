//! Multitouch, gyro, gamepad, and desktop keyboard/mouse input router.
//!
//! Enforces strict pointer ownership so the left virtual joystick never steals
//! right-half camera look drags, and supports long-press dimensional slash charging
//! with drag-off cancellation (PRD section 3.11).

use engine_core::GameSettings;
use glam::Vec2;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Logical touch button identifiers in the right-hand (or left-handed mirrored) action cluster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TouchButton {
    Jump,
    Slash,
    Paint,
    Dash,
    Slide,
    Interact,
    Pause,
}

/// Normalized screen-space button region (`[0.0, 1.0]` in X and Y).
#[derive(Debug, Clone, Copy)]
pub struct ButtonRegion {
    pub button: TouchButton,
    pub center: Vec2,
    pub radius: f32,
}

impl ButtonRegion {
    pub fn contains(&self, norm_pos: Vec2, aspect: f32) -> bool {
        let dx = (norm_pos.x - self.center.x) * aspect;
        let dy = norm_pos.y - self.center.y;
        (dx * dx + dy * dy).sqrt() <= self.radius
    }
}

/// Per-frame gameplay command snapshot consumed by player, combat, and paint systems.
#[derive(Debug, Clone, Default)]
pub struct InputFrame {
    /// Analog movement vector in local XZ (`x`: right, `y`: forward), magnitude `<= 1.0`.
    pub move_vec: Vec2,
    /// Camera look delta in radians (`x`: yaw, `y`: pitch).
    pub look_delta: Vec2,
    pub sprint: bool,
    pub jump_pressed: bool,
    pub jump_held: bool,
    pub dash_pressed: bool,
    pub slide_held: bool,
    pub interact_pressed: bool,
    pub pause_pressed: bool,
    /// Quick tap or swipe slash trigger.
    pub light_slash_pressed: bool,
    /// Directional swipe heavy slash trigger.
    pub heavy_slash_pressed: bool,
    /// True while holding Slash button to charge Dimensional Slash.
    pub slash_charging: bool,
    /// Charge duration in seconds for the active hold.
    pub slash_charge_secs: f32,
    /// Released after crossing charge threshold (fires Dimensional Slash).
    pub dimensional_slash_released: bool,
    /// Toggles Paint mode on tap.
    pub paint_toggle_pressed: bool,
    /// Opens Paint radial palette while holding Paint button.
    pub paint_radial_held: bool,
    /// Active brush stroke screen-space position when drawing in Paint mode.
    pub paint_stroke_pos: Option<Vec2>,
}

#[derive(Debug, Clone, Copy)]
enum PointerRole {
    Joystick { origin: Vec2, current: Vec2 },
    Look { last_pos: Vec2 },
    Button {
        button: TouchButton,
        press_time: f32,
        start_pos: Vec2,
        cancelled: bool,
    },
}

/// Stateful multitouch + desktop input router.
#[derive(Debug, Clone)]
pub struct InputRouter {
    pointers: HashMap<u64, PointerRole>,
    pub elapsed_secs: f32,
    pub joystick_deadzone: f32,
    pub joystick_max_radius: f32,
    pub charge_threshold_secs: f32,
    pub paint_mode_active: bool,
    /// Desktop / gamepad fallback state.
    pub key_move: Vec2,
    pub key_sprint: bool,
    pub key_slide: bool,
    pub pending_look_delta: Vec2,
    pending_jump: bool,
    pub jump_held: bool,
    pending_dash: bool,
    pending_light_slash: bool,
    pending_heavy_slash: bool,
    pending_dim_slash: bool,
    pending_paint_toggle: bool,
    pending_interact: bool,
    pending_pause: bool,
}

impl Default for InputRouter {
    fn default() -> Self {
        Self {
            pointers: HashMap::new(),
            elapsed_secs: 0.0,
            joystick_deadzone: 0.015,
            joystick_max_radius: 0.12,
            charge_threshold_secs: 0.45,
            paint_mode_active: false,
            key_move: Vec2::ZERO,
            key_sprint: false,
            key_slide: false,
            pending_look_delta: Vec2::ZERO,
            pending_jump: false,
            jump_held: false,
            pending_dash: false,
            pending_light_slash: false,
            pending_heavy_slash: false,
            pending_dim_slash: false,
            pending_paint_toggle: false,
            pending_interact: false,
            pending_pause: false,
        }
    }
}

impl InputRouter {
    /// Returns normalized touch button regions respecting `left_handed` and `hud_scale`.
    pub fn button_regions(settings: &GameSettings) -> Vec<ButtonRegion> {
        let scale = settings.hud_scale.clamp(0.75, 1.4);
        let mirror = |x: f32| if settings.left_handed { 1.0 - x } else { x };
        vec![
            ButtonRegion {
                button: TouchButton::Jump,
                center: Vec2::new(mirror(0.90), 0.78),
                radius: 0.075 * scale,
            },
            ButtonRegion {
                button: TouchButton::Slash,
                center: Vec2::new(mirror(0.79), 0.72),
                radius: 0.082 * scale,
            },
            ButtonRegion {
                button: TouchButton::Paint,
                center: Vec2::new(mirror(0.88), 0.56),
                radius: 0.068 * scale,
            },
            ButtonRegion {
                button: TouchButton::Dash,
                center: Vec2::new(mirror(0.74), 0.87),
                radius: 0.062 * scale,
            },
            ButtonRegion {
                button: TouchButton::Slide,
                center: Vec2::new(mirror(0.64), 0.87),
                radius: 0.058 * scale,
            },
            ButtonRegion {
                button: TouchButton::Interact,
                center: Vec2::new(mirror(0.76), 0.54),
                radius: 0.055 * scale,
            },
            ButtonRegion {
                button: TouchButton::Pause,
                center: Vec2::new(0.95, 0.08),
                radius: 0.055 * scale,
            },
        ]
    }

    /// Handles a touch-down event at normalized screen coordinates (`[0, 1] x [0, 1]`).
    pub fn on_touch_down(
        &mut self,
        pointer_id: u64,
        norm_pos: Vec2,
        aspect: f32,
        settings: &GameSettings,
    ) {
        let regions = Self::button_regions(settings);
        for reg in &regions {
            if reg.contains(norm_pos, aspect) {
                match reg.button {
                    TouchButton::Jump => {
                        self.pending_jump = true;
                        self.jump_held = true;
                    }
                    TouchButton::Dash => self.pending_dash = true,
                    TouchButton::Interact => self.pending_interact = true,
                    TouchButton::Pause => self.pending_pause = true,
                    _ => {}
                }
                self.pointers.insert(
                    pointer_id,
                    PointerRole::Button {
                        button: reg.button,
                        press_time: self.elapsed_secs,
                        start_pos: norm_pos,
                        cancelled: false,
                    },
                );
                return;
            }
        }

        let in_move_half = if settings.left_handed {
            norm_pos.x > 0.5
        } else {
            norm_pos.x < 0.5
        };

        let joystick_already_owned = self
            .pointers
            .values()
            .any(|r| matches!(r, PointerRole::Joystick { .. }));

        if in_move_half && norm_pos.y > 0.35 && !joystick_already_owned {
            self.pointers.insert(
                pointer_id,
                PointerRole::Joystick {
                    origin: norm_pos,
                    current: norm_pos,
                },
            );
        } else {
            self.pointers
                .insert(pointer_id, PointerRole::Look { last_pos: norm_pos });
        }
    }

    /// Handles touch-move events without crossing pointer ownership boundaries.
    pub fn on_touch_move(
        &mut self,
        pointer_id: u64,
        norm_pos: Vec2,
        aspect: f32,
        settings: &GameSettings,
    ) {
        let regions = Self::button_regions(settings);
        if let Some(role) = self.pointers.get_mut(&pointer_id) {
            match role {
                PointerRole::Joystick { current, .. } => {
                    *current = norm_pos;
                }
                PointerRole::Look { last_pos } => {
                    let delta = norm_pos - *last_pos;
                    *last_pos = norm_pos;
                    let invert = if settings.invert_y { 1.0 } else { -1.0 };
                    self.pending_look_delta.x -= delta.x * 3.2 * settings.sensitivity_x;
                    self.pending_look_delta.y += delta.y * 2.4 * settings.sensitivity_y * invert;
                }
                PointerRole::Button {
                    button, cancelled, ..
                } => {
                    // Sliding finger off Slash cancels Dimensional Slash charge (PRD 3.11).
                    if *button == TouchButton::Slash {
                        if let Some(reg) = regions.iter().find(|r| r.button == TouchButton::Slash) {
                            if !reg.contains(norm_pos, aspect) {
                                *cancelled = true;
                            }
                        }
                    }
                }
            }
        }
    }

    /// Handles touch-up or touch-cancel events.
    pub fn on_touch_up(&mut self, pointer_id: u64, norm_pos: Vec2) {
        if let Some(role) = self.pointers.remove(&pointer_id) {
            if let PointerRole::Button {
                button,
                press_time,
                start_pos,
                cancelled,
            } = role
            {
                let held = (self.elapsed_secs - press_time).max(0.0);
                match button {
                    TouchButton::Jump => self.jump_held = false,
                    TouchButton::Slash => {
                        if !cancelled {
                            let swipe_dist = (norm_pos - start_pos).length();
                            if held >= self.charge_threshold_secs {
                                self.pending_dim_slash = true;
                            } else if swipe_dist > 0.06 {
                                self.pending_heavy_slash = true;
                            } else {
                                self.pending_light_slash = true;
                            }
                        }
                    }
                    TouchButton::Paint => {
                        if held < 0.35 {
                            self.paint_mode_active = !self.paint_mode_active;
                            self.pending_paint_toggle = true;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// Clears all active touch pointers (required on Android focus loss / pause).
    pub fn clear_pointers(&mut self) {
        self.pointers.clear();
        self.jump_held = false;
    }

    /// Desktop/gamepad trigger helpers.
    pub fn trigger_jump(&mut self, held: bool) {
        if held && !self.jump_held {
            self.pending_jump = true;
        }
        self.jump_held = held;
    }

    pub fn trigger_light_slash(&mut self) {
        self.pending_light_slash = true;
    }

    pub fn trigger_dimensional_slash(&mut self) {
        self.pending_dim_slash = true;
    }

    pub fn trigger_dash(&mut self) {
        self.pending_dash = true;
    }

    pub fn trigger_paint_toggle(&mut self) {
        self.paint_mode_active = !self.paint_mode_active;
        self.pending_paint_toggle = true;
    }

    pub fn trigger_pause(&mut self) {
        self.pending_pause = true;
    }

    /// Produces the current `InputFrame` and consumes one-shot triggers.
    pub fn sample(&mut self, dt: f32) -> InputFrame {
        self.elapsed_secs += dt.max(0.0);

        let mut move_vec = self.key_move;
        let mut sprint = self.key_sprint;
        let mut slide_held = self.key_slide;
        let mut slash_charging = false;
        let mut slash_charge_secs = 0.0;
        let mut paint_radial_held = false;
        let mut paint_stroke_pos = None;

        for role in self.pointers.values() {
            match role {
                PointerRole::Joystick { origin, current } => {
                    let raw = *current - *origin;
                    // Screen Y increases downward; forward is negative screen Y.
                    let mapped = Vec2::new(raw.x, -raw.y);
                    let len = mapped.length();
                    if len > self.joystick_deadzone {
                        let mag = ((len - self.joystick_deadzone)
                            / (self.joystick_max_radius - self.joystick_deadzone))
                            .clamp(0.0, 1.0);
                        move_vec = (mapped / len) * mag;
                        if mag > 0.85 {
                            sprint = true;
                        }
                    }
                }
                PointerRole::Look { last_pos } => {
                    if self.paint_mode_active {
                        paint_stroke_pos = Some(*last_pos);
                    }
                }
                PointerRole::Button {
                    button,
                    press_time,
                    cancelled,
                    ..
                } => {
                    let held = (self.elapsed_secs - *press_time).max(0.0);
                    match button {
                        TouchButton::Slide => slide_held = true,
                        TouchButton::Slash if !cancelled => {
                            if held > 0.12 {
                                slash_charging = true;
                                slash_charge_secs = held;
                            }
                        }
                        TouchButton::Paint => {
                            if held >= 0.35 {
                                paint_radial_held = true;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        if move_vec.length_squared() > 1.0 {
            move_vec = move_vec.normalize();
        }

        let frame = InputFrame {
            move_vec,
            look_delta: self.pending_look_delta,
            sprint,
            jump_pressed: self.pending_jump,
            jump_held: self.jump_held,
            dash_pressed: self.pending_dash,
            slide_held,
            interact_pressed: self.pending_interact,
            pause_pressed: self.pending_pause,
            light_slash_pressed: self.pending_light_slash,
            heavy_slash_pressed: self.pending_heavy_slash,
            slash_charging,
            slash_charge_secs,
            dimensional_slash_released: self.pending_dim_slash,
            paint_toggle_pressed: self.pending_paint_toggle,
            paint_radial_held,
            paint_stroke_pos,
        };

        self.pending_look_delta = Vec2::ZERO;
        self.pending_jump = false;
        self.pending_dash = false;
        self.pending_light_slash = false;
        self.pending_heavy_slash = false;
        self.pending_dim_slash = false;
        self.pending_paint_toggle = false;
        self.pending_interact = false;
        self.pending_pause = false;

        frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multitouch_joystick_and_look_do_not_interfere() {
        let settings = GameSettings::default();
        let mut router = InputRouter::default();
        let aspect = 16.0 / 9.0;

        // Left thumb touches joystick area; right thumb drags look area.
        router.on_touch_down(1, Vec2::new(0.18, 0.75), aspect, &settings);
        router.on_touch_down(2, Vec2::new(0.60, 0.40), aspect, &settings);

        router.on_touch_move(1, Vec2::new(0.18, 0.63), aspect, &settings);
        router.on_touch_move(2, Vec2::new(0.68, 0.40), aspect, &settings);

        let frame = router.sample(0.016);
        assert!(frame.move_vec.y > 0.85, "Expected forward movement");
        assert!(frame.sprint, "High joystick deflection should auto-sprint");
        assert!(frame.look_delta.x.abs() > 0.1, "Expected camera yaw delta");
    }

    #[test]
    fn dimensional_slash_charge_and_drag_off_cancel() {
        let settings = GameSettings::default();
        let mut router = InputRouter::default();
        let aspect = 16.0 / 9.0;

        // Hold Slash button past charge threshold -> fires dimensional slash.
        let slash_center = Vec2::new(0.79, 0.72);
        router.on_touch_down(10, slash_center, aspect, &settings);
        let mid = router.sample(0.50);
        assert!(mid.slash_charging);
        router.on_touch_up(10, slash_center);
        let released = router.sample(0.016);
        assert!(released.dimensional_slash_released);

        // Hold Slash then drag off button -> cancels cleanly.
        router.on_touch_down(11, slash_center, aspect, &settings);
        router.sample(0.50);
        router.on_touch_move(11, Vec2::new(0.20, 0.20), aspect, &settings);
        router.on_touch_up(11, Vec2::new(0.20, 0.20));
        let cancelled = router.sample(0.016);
        assert!(!cancelled.dimensional_slash_released);
        assert!(!cancelled.light_slash_pressed);
    }
}
