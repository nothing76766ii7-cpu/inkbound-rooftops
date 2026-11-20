//! Multitouch, gyro, gamepad, and desktop keyboard/mouse input router (PRD v3.0).
//!
//! - Left 35%: floating virtual joystick with deadzone & adjustable radius
//! - Right 65%: camera drag look
//! - Dedicated Slash button (tap = Light A/B/C combo, hold 0.35s = Heavy)
//! - Dedicated World-Cut / Dimensional Slash button (tap = quick diagonal world cut,
//!   press-and-hold + drag = live aiming-plane angle preview, release = commit real 3D cut!)

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
    pub move_vec: Vec2,
    pub look_delta: Vec2,
    pub sprint: bool,
    pub jump_pressed: bool,
    pub jump_held: bool,
    pub dash_pressed: bool,
    pub slide_held: bool,
    pub interact_pressed: bool,
    pub pause_pressed: bool,
    pub light_slash_pressed: bool,
    pub heavy_slash_pressed: bool,
    pub slash_charging: bool,
    pub slash_charge_secs: f32,
    /// True while holding World-Cut button to preview the 3D cut plane angle (PRD v3.0).
    pub world_cut_previewing: bool,
    /// Chosen cut-plane roll angle in radians (0 = horizontal cut, PI/2 = vertical cut).
    pub world_cut_angle: f32,
    /// Released World-Cut button (commits real 3D mesh slice along `world_cut_angle`).
    pub dimensional_slash_released: bool,
    pub paint_toggle_pressed: bool,
    pub paint_radial_held: bool,
    pub paint_stroke_pos: Option<Vec2>,
    pub pressed_buttons: Vec<TouchButton>,
}

#[derive(Debug, Clone, Copy)]
enum PointerRole {
    Joystick { origin: Vec2, current: Vec2 },
    Look { last_pos: Vec2 },
    Button {
        button: TouchButton,
        press_time: f32,
        start_pos: Vec2,
        current_pos: Vec2,
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
    pub heavy_hold_secs: f32,
    pub paint_mode_active: bool,
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
    pending_cut_angle: f32,
    pending_paint_toggle: bool,
    pending_interact: bool,
    pending_pause: bool,
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
            pending_cut_angle: 0.24, // Slight dramatic diagonal cut by default
            pending_paint_toggle: false,
            pending_interact: false,
            pending_pause: false,
        }
    }
}

impl InputRouter {
    pub fn button_regions(settings: &GameSettings) -> Vec<ButtonRegion> {
        let scale = settings.hud_scale.clamp(0.75, 1.5);
        let mirror = |x: f32| if settings.left_handed { 1.0 - x } else { x };
        vec![
            ButtonRegion {
                button: TouchButton::Slash,
                center: Vec2::new(mirror(0.84), 0.74),
                radius: 0.090 * scale,
            },
            ButtonRegion {
                button: TouchButton::DimensionalSlash,
                center: Vec2::new(mirror(0.73), 0.65),
                radius: 0.072 * scale,
            },
            ButtonRegion {
                button: TouchButton::Jump,
                center: Vec2::new(mirror(0.92), 0.56),
                radius: 0.072 * scale,
            },
            ButtonRegion {
                button: TouchButton::Paint,
                center: Vec2::new(mirror(0.81), 0.50),
                radius: 0.062 * scale,
            },
            ButtonRegion {
                button: TouchButton::Dash,
                center: Vec2::new(mirror(0.72), 0.84),
                radius: 0.062 * scale,
            },
            ButtonRegion {
                button: TouchButton::Slide,
                center: Vec2::new(mirror(0.62), 0.86),
                radius: 0.056 * scale,
            },
            ButtonRegion {
                button: TouchButton::Pause,
                center: Vec2::new(0.945, 0.09),
                radius: 0.052 * scale,
            },
        ]
    }

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
                        current_pos: norm_pos,
                        cancelled: false,
                    },
                );
                return;
            }
        }

        // Left 35% (or right 35% in left-handed mode) owns the floating joystick (PRD v3.0)
        let in_move_zone = if settings.left_handed {
            norm_pos.x > 0.65
        } else {
            norm_pos.x < 0.35
        };

        let joystick_already_owned = self
            .pointers
            .values()
            .any(|r| matches!(r, PointerRole::Joystick { .. }));

        if in_move_zone && norm_pos.y > 0.25 && !joystick_already_owned {
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
                    button,
                    current_pos,
                    cancelled,
                    ..
                } => {
                    *current_pos = norm_pos;
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

    pub fn on_touch_up(&mut self, pointer_id: u64, norm_pos: Vec2) {
        if let Some(role) = self.pointers.remove(&pointer_id) {
            if let PointerRole::Button {
                button,
                press_time,
                start_pos,
                cancelled,
                ..
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
                    TouchButton::DimensionalSlash => {
                        // Swipe vector chooses cut-plane angle; quick tap uses clean diagonal cut
                        let drag = norm_pos - start_pos;
                        if drag.length() > 0.03 {
                            self.pending_cut_angle = (-drag.y).atan2(drag.x);
                        } else {
                            self.pending_cut_angle = 0.24;
                        }
                        self.pending_dim_slash = true;
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

    pub fn clear_pointers(&mut self) {
        self.pointers.clear();
        self.jump_held = false;
        self.key_move = Vec2::ZERO;
        self.key_sprint = false;
        self.key_slide = false;
    }

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
        self.pending_cut_angle = 0.24;
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

    pub fn sample(&mut self, dt: f32) -> InputFrame {
        self.elapsed_secs += dt.max(0.0);

        let mut move_vec = self.key_move;
        let mut sprint = self.key_sprint;
        let mut slide_held = self.key_slide;
        let mut slash_charging = false;
        let mut slash_charge_secs = 0.0;
        let mut world_cut_previewing = false;
        let mut world_cut_angle = self.pending_cut_angle;
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
                    start_pos,
                    current_pos,
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
                        TouchButton::DimensionalSlash => {
                            world_cut_previewing = true;
                            let drag = *current_pos - *start_pos;
                            if drag.length() > 0.03 {
                                world_cut_angle = (-drag.y).atan2(drag.x);
                            } else {
                                world_cut_angle = 0.24;
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
            world_cut_previewing,
            world_cut_angle,
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
    fn world_cut_hold_previews_angle_and_release_commits() {
        let settings = GameSettings::default();
        let mut router = InputRouter::default();
        let aspect = 16.0 / 9.0;

        let cut_btn = Vec2::new(0.73, 0.65);
        router.on_touch_down(1, cut_btn, aspect, &settings);
        router.on_touch_move(1, cut_btn + Vec2::new(0.08, -0.04), aspect, &settings);
        let preview = router.sample(0.10);
        assert!(preview.world_cut_previewing);

        router.on_touch_up(1, cut_btn + Vec2::new(0.08, -0.04));
        let commit = router.sample(0.016);
        assert!(commit.dimensional_slash_released);
    }
}
