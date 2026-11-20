//! Multitouch, gyro, gamepad, and desktop keyboard/mouse input router (PRD v2.0).
//!
//! Enforces strict pointer ownership so the left virtual joystick never steals
//! right-half camera look drags, maps dedicated Slash (tap = light, hold 0.35s = heavy)
//! and DimensionalSlash buttons, and exposes live joystick coordinates for HUD rendering.

use engine_core::GameSettings;
use glam::Vec2;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Logical touch button identifiers in the action cluster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TouchButton {
    Jump,
    Slash,
    DimensionalSlash,
    Paint,
    Dash,
    Slide,
    Interact,
    Pause,
}

/// Normalized screen-space circular button region (`[0.0, 1.0]` in X and Y).
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

/// Live virtual joystick visual state for HUD rendering.
#[derive(Debug, Clone, Copy)]
pub struct JoystickVisual {
    pub center: Vec2,
    pub thumb: Vec2,
    pub radius: f32,
    pub active: bool,
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
    /// Quick tap sword attack.
    pub light_slash_pressed: bool,
    /// Hold >= 0.35s and release sword attack.
    pub heavy_slash_pressed: bool,
    /// True while holding Slash or DimensionalSlash to show charge feedback.
    pub slash_charging: bool,
    /// Charge duration in seconds for the active hold.
    pub slash_charge_secs: f32,
    /// Dedicated Dimensional Slash button trigger.
    pub dimensional_slash_released: bool,
    /// Toggles Paint mode on tap.
    pub paint_toggle_pressed: bool,
    /// Opens Paint radial palette while holding Paint button.
    pub paint_radial_held: bool,
    /// Active brush stroke screen-space position when drawing in Paint mode.
    pub paint_stroke_pos: Option<Vec2>,
    /// Currently pressed touch buttons (for visual pressed-scale feedback).
    pub pressed_buttons: Vec<TouchButton>,
}

#[derive(Debug, Clone, Copy)]
enum PointerRole {
    Joystick { origin: Vec2, current: Vec2 },
    Look { last_pos: Vec2 },
    Button {
        button: TouchButton,
        press_time: f32,
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
    /// Heavy slash hold threshold (PRD 4.3: 0.35s).
    pub heavy_hold_secs: f32,
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
    pub last_menu_tap: Option<Vec2>,
}

impl Default for InputRouter {
    fn default() -> Self {
        Self {
            pointers: HashMap::new(),
            elapsed_secs: 0.0,
            joystick_deadzone: 0.014,
            joystick_max_radius: 0.115,
            heavy_hold_secs: 0.35,
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
            last_menu_tap: None,
        }
    }
}

impl InputRouter {
    /// Returns normalized circular touch button regions respecting safe areas, `left_handed`, and `hud_scale`.
    pub fn button_regions(settings: &GameSettings) -> Vec<ButtonRegion> {
        let scale = settings.hud_scale.clamp(0.75, 1.5);
        let mirror = |x: f32| if settings.left_handed { 1.0 - x } else { x };
        vec![
            // Primary sword slash button (largest thumb target)
            ButtonRegion {
                button: TouchButton::Slash,
                center: Vec2::new(mirror(0.84), 0.74),
                radius: 0.090 * scale,
            },
            // Dedicated Dimensional Slash crescent button
            ButtonRegion {
                button: TouchButton::DimensionalSlash,
                center: Vec2::new(mirror(0.73), 0.65),
                radius: 0.068 * scale,
            },
            // Jump button
            ButtonRegion {
                button: TouchButton::Jump,
                center: Vec2::new(mirror(0.92), 0.56),
                radius: 0.072 * scale,
            },
            // Paint brush toggle button
            ButtonRegion {
                button: TouchButton::Paint,
                center: Vec2::new(mirror(0.81), 0.50),
                radius: 0.062 * scale,
            },
            // Dash double-chevron button
            ButtonRegion {
                button: TouchButton::Dash,
                center: Vec2::new(mirror(0.72), 0.84),
                radius: 0.062 * scale,
            },
            // Slide/crouch button
            ButtonRegion {
                button: TouchButton::Slide,
                center: Vec2::new(mirror(0.62), 0.86),
                radius: 0.056 * scale,
            },
            // Pause icon button (upper-right safe inset)
            ButtonRegion {
                button: TouchButton::Pause,
                center: Vec2::new(0.945, 0.09),
                radius: 0.052 * scale,
            },
        ]
    }

    /// Returns the live virtual joystick visual coordinates for HUD rendering.
    pub fn joystick_visual(&self, settings: &GameSettings) -> JoystickVisual {
        let default_center = if settings.left_handed {
            Vec2::new(0.86, 0.76)
        } else {
            Vec2::new(0.14, 0.76)
        };
        for role in self.pointers.values() {
            if let PointerRole::Joystick { origin, current } = role {
                let delta = *current - *origin;
                let clamped = if delta.length() > self.joystick_max_radius {
                    *origin + delta.normalize() * self.joystick_max_radius
                } else {
                    *current
                };
                return JoystickVisual {
                    center: *origin,
                    thumb: clamped,
                    radius: self.joystick_max_radius,
                    active: true,
                };
            }
        }
        JoystickVisual {
            center: default_center,
            thumb: default_center,
            radius: self.joystick_max_radius,
            active: false,
        }
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
                    TouchButton::DimensionalSlash => self.pending_dim_slash = true,
                    _ => {}
                }
                self.pointers.insert(
                    pointer_id,
                    PointerRole::Button {
                        button: reg.button,
                        press_time: self.elapsed_secs,
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

        if in_move_half && norm_pos.y > 0.30 && !joystick_already_owned {
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
                    self.pending_look_delta.x -= delta.x * 3.4 * settings.sensitivity_x;
                    self.pending_look_delta.y += delta.y * 2.5 * settings.sensitivity_y * invert;
                }
                PointerRole::Button {
                    button, cancelled, ..
                } => {
                    // Sliding finger far off Slash cancels a held charge cleanly.
                    if *button == TouchButton::Slash {
                        if let Some(reg) = regions.iter().find(|r| r.button == TouchButton::Slash) {
                            let dx = (norm_pos.x - reg.center.x) * aspect;
                            let dy = norm_pos.y - reg.center.y;
                            if (dx * dx + dy * dy).sqrt() > reg.radius * 1.45 {
                                *cancelled = true;
                            }
                        }
                    }
                }
            }
        }
    }

    /// Handles touch-up or touch-cancel events.
    pub fn on_touch_up(&mut self, pointer_id: u64, _norm_pos: Vec2) {
        if let Some(role) = self.pointers.remove(&pointer_id) {
            if let PointerRole::Button {
                button,
                press_time,
                cancelled,
            } = role
            {
                let held = (self.elapsed_secs - press_time).max(0.0);
                match button {
                    TouchButton::Jump => self.jump_held = false,
                    TouchButton::Slash => {
                        if !cancelled {
                            if held >= self.heavy_hold_secs {
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

    /// Clears all active touch pointers (mandatory on Pause / Android focus loss — PRD 4.5).
    pub fn clear_pointers(&mut self) {
        self.pointers.clear();
        self.jump_held = false;
        self.key_move = Vec2::ZERO;
        self.key_sprint = false;
        self.key_slide = false;
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

    pub fn trigger_heavy_slash(&mut self) {
        self.pending_heavy_slash = true;
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
        let mut pressed_buttons = Vec::new();

        for role in self.pointers.values() {
            match role {
                PointerRole::Joystick { origin, current } => {
                    let raw = *current - *origin;
                    let mapped = Vec2::new(raw.x, -raw.y);
                    let len = mapped.length();
                    if len > self.joystick_deadzone {
                        let mag = ((len - self.joystick_deadzone)
                            / (self.joystick_max_radius - self.joystick_deadzone))
                            .clamp(0.0, 1.0);
                        move_vec = (mapped / len) * mag;
                        if mag > 0.80 {
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
                } => {
                    pressed_buttons.push(*button);
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
            pressed_buttons,
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
    fn multitouch_joystick_look_and_slash_work_simultaneously() {
        let settings = GameSettings::default();
        let mut router = InputRouter::default();
        let aspect = 16.0 / 9.0;

        // Left thumb joystick + right thumb look + simultaneous slash tap
        router.on_touch_down(1, Vec2::new(0.16, 0.75), aspect, &settings);
        router.on_touch_down(2, Vec2::new(0.58, 0.38), aspect, &settings);
        let slash_btn = Vec2::new(0.84, 0.74);
        router.on_touch_down(3, slash_btn, aspect, &settings);

        router.on_touch_move(1, Vec2::new(0.16, 0.62), aspect, &settings);
        router.on_touch_move(2, Vec2::new(0.65, 0.38), aspect, &settings);
        router.on_touch_up(3, slash_btn);

        let frame = router.sample(0.05);
        assert!(frame.move_vec.y > 0.80);
        assert!(frame.look_delta.x.abs() > 0.1);
        assert!(frame.light_slash_pressed);
    }

    #[test]
    fn heavy_hold_and_dedicated_dimensional_slash_trigger() {
        let settings = GameSettings::default();
        let mut router = InputRouter::default();
        let aspect = 16.0 / 9.0;

        // Hold Slash 0.40s -> Heavy Slash
        let slash_btn = Vec2::new(0.84, 0.74);
        router.on_touch_down(10, slash_btn, aspect, &settings);
        router.sample(0.40);
        router.on_touch_up(10, slash_btn);
        let f1 = router.sample(0.016);
        assert!(f1.heavy_slash_pressed);

        // Tap dedicated Dimensional Slash button
        let dim_btn = Vec2::new(0.73, 0.65);
        router.on_touch_down(11, dim_btn, aspect, &settings);
        let f2 = router.sample(0.016);
        assert!(f2.dimensional_slash_released);
    }
}
