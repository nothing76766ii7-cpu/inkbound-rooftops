//! Sketchbook UI design system, screen state machine, circular icon-based Android HUD,
//! and illustrated menus (PRD v2.0 section 4.5).

pub mod icons;

use engine_core::{GameSettings, Palette};
use gameplay_paint::PaintMode;
use icons::{
    push_text,
    push_button_icon, push_circle, push_heart_icon, push_hud_number_pair, push_line, push_rect,
    push_ring, push_tri, UiVert,
};
use platform_input::{InputRouter, JoystickVisual, TouchButton};
use serde::{Deserialize, Serialize};

/// Complete screen state machine covering PRD sections 3.12 & 4.5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScreenState {
    Splash,
    MainMenu,
    SaveSelect,
    InGameHud,
    Pause,
    Settings(SettingsTab),
    Upgrades,
    WorldMap,
    MissionSummary { victory: bool },
    Tutorial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SettingsTab {
    Graphics,
    Audio,
    Controls,
    Gameplay,
    Accessibility,
    Language,
}

/// Interactive menu actions returned when tapping menu buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    ResumeOrPlay,
    RestartCheckpoint,
    RestartLevel,
    NewRun,
    NextLevel,
    ToggleReducedMotion,
    ToggleSound,
    MainMenu,
    OpenTutorial,
    BackToPause,
    OpenSettings,
    NextTutorial,
    PreviousTutorial,
}

/// Snapshot of gameplay telemetry rendered onto the sketchbook HUD.
#[derive(Debug, Clone)]
pub struct HudSnapshot {
    pub health: f32,
    pub max_health: f32,
    pub stamina: f32,
    pub max_stamina: f32,
    pub pigment: f32,
    pub max_pigment: f32,
    pub slash_charge_ratio: f32,
    pub dimensional_cooldown_ratio: f32,
    pub objective_done: u32,
    pub objective_total: u32,
    pub paint_mode_active: bool,
    pub active_paint_mode: PaintMode,
    pub joystick: JoystickVisual,
    pub pressed_buttons: Vec<TouchButton>,
    pub hook_active: bool,
    pub hook_target_valid: bool,
    pub hook_cooldown_ratio: f32,
    pub tutorial_page: usize,
    pub level_number: u32,
    pub checkpoint_flash: f32,
    pub level_complete: bool,
}

impl Default for HudSnapshot {
    fn default() -> Self {
        Self {
            health: 100.0,
            max_health: 100.0,
            stamina: 100.0,
            max_stamina: 100.0,
            pigment: 100.0,
            max_pigment: 100.0,
            slash_charge_ratio: 0.0,
            dimensional_cooldown_ratio: 0.0,
            objective_done: 0,
            objective_total: 4,
            paint_mode_active: false,
            active_paint_mode: PaintMode::Brush,
            joystick: JoystickVisual {
                center: glam::Vec2::new(0.14, 0.76),
                thumb: glam::Vec2::new(0.14, 0.76),
                radius: 0.115,
                active: false,
            },
            pressed_buttons: Vec::new(),
            hook_active: false,
            hook_target_valid: false,
            hook_cooldown_ratio: 0.0,
            tutorial_page: 0,
            level_number: 1,
            checkpoint_flash: 0.0,
            level_complete: false,
        }
    }
}

/// Hit-tests normalized tap coordinates against active Pause / Main Menu buttons.
pub fn hit_test_menu(screen: ScreenState, x: f32, y: f32) -> Option<MenuAction> {
    if screen == ScreenState::Tutorial {
        if (0.81..=0.94).contains(&y) {
            if x < 0.36 { return Some(MenuAction::PreviousTutorial); }
            if x > 0.64 { return Some(MenuAction::NextTutorial); }
            return Some(MenuAction::BackToPause);
        }
        return None;
    }
    if matches!(screen, ScreenState::Settings(_)) {
        if (0.72..=0.90).contains(&y) { return Some(MenuAction::BackToPause); }
        if (0.39..=0.56).contains(&y) { return Some(MenuAction::ToggleReducedMotion); }
        if (0.57..=0.72).contains(&y) { return Some(MenuAction::ToggleSound); }
        return None;
    }
    if !(0.33..=0.67).contains(&x) { return None; }
    if let ScreenState::MissionSummary { .. } = screen {
        if (0.43..=0.54).contains(&y) { return Some(MenuAction::NextLevel); }
        if (0.56..=0.67).contains(&y) { return Some(MenuAction::RestartLevel); }
        if (0.69..=0.80).contains(&y) { return Some(MenuAction::MainMenu); }
        return None;
    }
    if !matches!(screen, ScreenState::MainMenu | ScreenState::Pause | ScreenState::Splash) { return None; }
    let index = (0..6).find(|i| {
        let top = 0.326 + *i as f32 * 0.095;
        (top..=top + 0.085).contains(&y)
    })?;
    if screen == ScreenState::Pause {
        Some(match index {
            0 => MenuAction::ResumeOrPlay,
            1 => MenuAction::RestartCheckpoint,
            2 => MenuAction::RestartLevel,
            3 => MenuAction::OpenTutorial,
            4 => MenuAction::OpenSettings,
            _ => MenuAction::MainMenu,
        })
    } else {
        Some(match index {
            0 => MenuAction::ResumeOrPlay,
            1 => MenuAction::NewRun,
            2 => MenuAction::RestartLevel,
            3 => MenuAction::OpenTutorial,
            4 => MenuAction::OpenSettings,
            _ => MenuAction::RestartCheckpoint,
        })
    }
}

/// Generates all crisp 2D vector triangles (circular buttons, pictograms, joystick,
/// numeric counters, and illustrated skyline menus) for `render_wgpu`.
pub fn build_ui_vertices(
    screen: ScreenState,
    hud: &HudSnapshot,
    settings: &GameSettings,
    aspect: f32,
) -> Vec<UiVert> {
    let mut verts = Vec::with_capacity(2048);
    let alpha = settings.hud_opacity.clamp(0.40, 1.0);

    match screen {
        ScreenState::InGameHud => {
            // 1. Upper-left Health bar with heart icon & numeric counter (100/100)
            let hp_ratio = (hud.health / hud.max_health.max(1.0)).clamp(0.0, 1.0);
            push_heart_icon(&mut verts, 0.036, 0.052, 0.022, aspect, Palette::HEALTH_RED);
            push_rect(&mut verts, 0.056, 0.038, 0.19, 0.028, Palette::INK);
            push_rect(&mut verts, 0.059, 0.042, 0.184, 0.020, Palette::PAPER);
            push_rect(&mut verts, 0.059, 0.042, 0.184 * hp_ratio, 0.020, Palette::HEALTH_RED);
            push_hud_number_pair(
                &mut verts,
                0.255,
                0.041,
                0.020,
                aspect,
                hud.health.round() as u32,
                hud.max_health.round() as u32,
                Palette::INK,
            );

            // Stamina (warm orange) & Paint Pigment (teal) bars beneath health
            let stam_ratio = (hud.stamina / hud.max_stamina.max(1.0)).clamp(0.0, 1.0);
            let pig_ratio = (hud.pigment / hud.max_pigment.max(1.0)).clamp(0.0, 1.0);
            push_rect(&mut verts, 0.056, 0.074, 0.15, 0.012, Palette::INK);
            push_rect(&mut verts, 0.058, 0.076, 0.146 * stam_ratio, 0.008, Palette::ROOF_ORANGE);

            push_rect(&mut verts, 0.056, 0.092, 0.15, 0.012, Palette::INK);
            push_rect(&mut verts, 0.058, 0.094, 0.146 * pig_ratio, 0.008, Palette::PAINT_TEAL);

            // Progress state uses real game level + checkpoint changes.
            push_rect(&mut verts, 0.355, 0.033, 0.079, 0.039, [0.12, 0.17, 0.22, 0.92]);
            push_text(&mut verts, &format!("LEVEL {}", hud.level_number),
                0.362, 0.049, 0.015, aspect, Palette::PAPER);
            if hud.checkpoint_flash > 0.01 {
                push_rect(&mut verts, 0.37, 0.105, 0.26, 0.054,
                    [0.09, 0.28, 0.26, (hud.checkpoint_flash / 1.0).clamp(0.25, 0.98)]);
                push_text(&mut verts, "CHECKPOINT SAVED", 0.39, 0.126, 0.018, aspect, Palette::PAPER);
            }
            // 2. Top-center objective counter capsule with actual numeric text (never blank!)
            push_rect(&mut verts, 0.435, 0.032, 0.130, 0.044, Palette::INK);
            push_rect(&mut verts, 0.438, 0.036, 0.124, 0.036, [Palette::PAPER[0], Palette::PAPER[1], Palette::PAPER[2], 0.92]);
            let obj_ratio = if hud.objective_total == 0 {
                0.0
            } else {
                (hud.objective_done as f32 / hud.objective_total as f32).clamp(0.0, 1.0)
            };
            push_rect(&mut verts, 0.438, 0.066, 0.124 * obj_ratio, 0.006, Palette::ROOF_ORANGE);
            push_hud_number_pair(
                &mut verts,
                0.478,
                0.042,
                0.020,
                aspect,
                hud.objective_done,
                hud.objective_total,
                Palette::INK,
            );

            // 3. Center crisp ink crosshair reticle + heavy slash charge arc
            let ret_col = if hud.paint_mode_active {
                Palette::PAINT_TEAL
            } else if hud.slash_charge_ratio > 0.15 {
                Palette::ROOF_ORANGE
            } else {
                Palette::INK
            };
            push_line(&mut verts, [0.494, 0.50], [0.506, 0.50], 0.0035, aspect, ret_col);
            push_line(&mut verts, [0.50, 0.490], [0.50, 0.510], 0.0035, aspect, ret_col);
            if hud.slash_charge_ratio > 0.05 {
                let bar_w = 0.09 * hud.slash_charge_ratio;
                push_rect(&mut verts, 0.5 - bar_w * 0.5, 0.535, bar_w, 0.010, Palette::ROOF_ORANGE);
            }

            // Brush-hook status indicator: orange reticle ring while tethered.
            if hud.hook_target_valid && !hud.hook_active {
                push_ring(&mut verts, 0.5, 0.5, 0.027, 0.023, aspect, 32, Palette::PAINT_TEAL);
            }
            if hud.hook_active {
                push_ring(&mut verts, 0.5, 0.5, 0.022, 0.018, aspect, 32, Palette::ROOF_ORANGE);
            }

            // 4. Lower-left circular virtual joystick (outer ink ring + translucent base + draggable thumb)
            let joy = hud.joystick;
            let joy_bg = [Palette::PAPER[0], Palette::PAPER[1], Palette::PAPER[2], alpha * 0.32];
            let joy_ink = [Palette::INK[0], Palette::INK[1], Palette::INK[2], alpha * 0.80];
            push_circle(&mut verts, joy.center.x, joy.center.y, joy.radius, aspect, 28, joy_bg);
            push_ring(&mut verts, joy.center.x, joy.center.y, joy.radius, joy.radius - 0.006, aspect, 28, joy_ink);
            let thumb_col = if joy.active {
                [Palette::ROOF_ORANGE[0], Palette::ROOF_ORANGE[1], Palette::ROOF_ORANGE[2], alpha * 0.90]
            } else {
                [Palette::PAPER[0], Palette::PAPER[1], Palette::PAPER[2], alpha * 0.80]
            };
            push_circle(&mut verts, joy.thumb.x, joy.thumb.y, joy.radius * 0.36, aspect, 20, thumb_col);
            push_ring(&mut verts, joy.thumb.x, joy.thumb.y, joy.radius * 0.36, joy.radius * 0.31, aspect, 20, joy_ink);

            // 5. Alpha-masked circular parchment action buttons with vector pictograms (PRD 4.5)
            for reg in InputRouter::button_regions(settings) {
                let pressed = hud.pressed_buttons.contains(&reg.button);
                let r = if pressed { reg.radius * 0.91 } else { reg.radius };
                let fill = match reg.button {
                    TouchButton::Slash => [
                        Palette::PAPER[0],
                        Palette::PAPER[1],
                        Palette::PAPER[2],
                        alpha * 0.88,
                    ],
                    TouchButton::DimensionalSlash => {
                        if hud.dimensional_cooldown_ratio > 0.01 {
                            [Palette::MUTED_GRAY[0], Palette::MUTED_GRAY[1], Palette::MUTED_GRAY[2], alpha * 0.55]
                        } else {
                            [Palette::PAPER[0], Palette::PAPER[1], Palette::PAPER[2], alpha * 0.88]
                        }
                    }
                    TouchButton::Paint if hud.paint_mode_active => [
                        Palette::PAINT_TEAL[0],
                        Palette::PAINT_TEAL[1],
                        Palette::PAINT_TEAL[2],
                        alpha * 0.45,
                    ],
                    _ => [
                        Palette::PAPER[0],
                        Palette::PAPER[1],
                        Palette::PAPER[2],
                        alpha * 0.78,
                    ],
                };
                let ring_col = if reg.button == TouchButton::Slash || pressed {
                    Palette::ROOF_ORANGE
                } else {
                    [Palette::INK[0], Palette::INK[1], Palette::INK[2], alpha]
                };

                push_circle(&mut verts, reg.center.x, reg.center.y, r, aspect, 24, fill);
                push_ring(&mut verts, reg.center.x, reg.center.y, r, r - 0.006, aspect, 24, ring_col);
                push_button_icon(
                    &mut verts,
                    reg.button,
                    reg.center.x,
                    reg.center.y,
                    r * 0.85,
                    aspect,
                    Palette::INK,
                );
                if reg.button == TouchButton::Interact {
                    push_text(&mut verts, "HOOK", reg.center.x - 0.017, reg.center.y + 0.055, 0.011, aspect, Palette::INK);
                }
            }
        }
        ScreenState::MainMenu | ScreenState::Pause | ScreenState::Splash => {
            // Translucent overcast paper scrim
            push_rect(&mut verts, 0.0, 0.0, 1.0, 1.0, [0.055, 0.065, 0.085, 0.86]);

            // Illustrated white-and-orange sketchbook skyline silhouette along bottom edge (PRD 4.5)
            let towers = [(0.05, 0.72, 0.10), (0.14, 0.64, 0.09), (0.76, 0.62, 0.11), (0.86, 0.70, 0.10)];
            for (tx, ty, tw) in towers {
                push_rect(&mut verts, tx, ty, tw, 1.0 - ty, Palette::STONE);
                push_tri(&mut verts, [tx - 0.01, ty], [tx + tw + 0.01, ty], [tx + tw * 0.5, ty - 0.09], Palette::ROOF_ORANGE);
            }

            // Opaque dark illustrated menu: never leave a bright white modal over the world.
            push_rect(&mut verts, 0.28, 0.13, 0.44, 0.79, [0.08, 0.105, 0.15, 0.97]);
            push_rect(&mut verts, 0.284, 0.134, 0.432, 0.006, Palette::ROOF_ORANGE);
            push_text(&mut verts, "INKBOUND ROOFTOPS", 0.365, 0.23, 0.025, aspect, Palette::PAPER);
            push_text(&mut verts, "PAINT  /  PARKOUR  /  SLICE", 0.374, 0.30, 0.014, aspect, Palette::ROOF_ORANGE);
            let labels = if screen == ScreenState::Pause {
                ["RESUME", "RESTART CHECKPOINT", "RESTART LEVEL", "HOW TO PLAY", "SETTINGS", "MAIN MENU"]
            } else {
                ["CONTINUE", "GENERATE NEW RUN", "RESTART LEVEL", "HOW TO PLAY", "SETTINGS", "RESTART CHECKPOINT"]
            };
            for (i, label) in labels.iter().enumerate() {
                let y = 0.326 + i as f32 * 0.095;
                push_rect(&mut verts, 0.34, y, 0.32, 0.085,
                    if i == 0 { Palette::ROOF_ORANGE } else { [0.20, 0.25, 0.32, 1.0] });
                push_text(&mut verts, label, 0.365, y + 0.034, 0.015, aspect, Palette::PAPER);
            }
        }
        ScreenState::MissionSummary { victory: true } => {
            push_rect(&mut verts, 0.0, 0.0, 1.0, 1.0, [0.045, 0.065, 0.095, 0.96]);
            push_rect(&mut verts, 0.28, 0.15, 0.44, 0.69, [0.08, 0.105, 0.15, 0.97]);
            push_text(&mut verts, &format!("LEVEL {} COMPLETE", hud.level_number),
                0.345, 0.27, 0.024, aspect, Palette::ROOF_ORANGE);
            push_text(&mut verts, "ROOFTOP ROUTE CLEARED", 0.37, 0.34, 0.017, aspect, Palette::PAPER);
            for (i, label) in [if hud.level_number >= 5 { "GENERATE NEW RUN" } else { "NEXT LEVEL" },
                "RESTART LEVEL", "MAIN MENU"].iter().enumerate() {
                let y = 0.43 + i as f32 * 0.13;
                push_rect(&mut verts, 0.34, y, 0.32, 0.105,
                    if i == 0 { Palette::ROOF_ORANGE } else { [0.20, 0.25, 0.32, 1.0] });
                push_text(&mut verts, label, 0.38, y + 0.040, 0.016, aspect, Palette::PAPER);
            }
        }
        ScreenState::Settings(_) => {
            push_rect(&mut verts, 0.0, 0.0, 1.0, 1.0, [0.045, 0.065, 0.095, 0.96]);
            push_text(&mut verts, "SETTINGS", 0.41, 0.19, 0.034, aspect, Palette::ROOF_ORANGE);
            push_text(&mut verts, "CAMERA MOTION", 0.37, 0.34, 0.020, aspect, Palette::PAPER);
            push_rect(&mut verts, 0.34, 0.42, 0.32, 0.12, [0.20, 0.25, 0.32, 1.0]);
            push_text(&mut verts, if settings.reduced_motion { "REDUCED MOTION: ON" } else { "REDUCED MOTION: OFF" }, 0.365, 0.466, 0.018, aspect, Palette::PAPER);
            push_text(&mut verts, "SHAKE / SWAY DISABLED WHEN ON", 0.34, 0.56, 0.012, aspect, Palette::PAPER);
            push_rect(&mut verts, 0.34, 0.60, 0.32, 0.10, [0.20, 0.25, 0.32, 1.0]);
            push_text(&mut verts, if settings.sfx_volume < 0.01 { "SOUND EFFECTS: OFF" } else { "SOUND EFFECTS: ON" },
                0.365, 0.645, 0.018, aspect, Palette::PAPER);
            push_rect(&mut verts, 0.34, 0.77, 0.32, 0.10, Palette::ROOF_ORANGE);
            push_text(&mut verts, "BACK", 0.46, 0.809, 0.020, aspect, Palette::INK);
        }
        ScreenState::Tutorial => {
            push_rect(&mut verts, 0.0, 0.0, 1.0, 1.0, [0.045, 0.065, 0.095, 0.97]);
            let pages: [(&str, [&str; 4]); 5] = [
                ("01 / MOVE AND PARKOUR", ["LEFT STICK: MOVE", "RIGHT SWIPE: LOOK", "JUMP: LEAP / WALL JUMP", "DASH AND SLIDE: KEEP MOMENTUM"]),
                ("02 / BRUSH WEB", ["AIM AT WALLS OR HIGH ROOFS", "TEAL RETICLE: VALID ANCHOR", "MOVE: STEER  SPRINT: REEL IN", "JUMP: LAUNCH  HOOK: DETACH"]),
                ("03 / SWORD AND WORLD CUT", ["SWORD: QUICK SLASH", "HEAVY: STRONG IMPACT", "RIFT: CHARGE THEN RELEASE", "CUTS CAN SPLIT BUILDING MESHES"]),
                ("04 / PAINT AND SURVIVE", ["BRUSH: PAINT TARGET SURFACES", "SWITCH MODE: CHANGE PAINT TYPE", "HEALTH / STAMINA / INK: TOP LEFT", "PAUSE: OPEN MENU ANY TIME"]),
                ("05 / LEVELS AND SAVES", ["TEAL PADS SAVE CHECKPOINTS", "REACH FINAL TEAL BEACON TO EXIT", "RESTART CHECKPOINT: RESTORE SAVED WORLD", "RESTART LEVEL: START WITH INTACT ROOFS"]),
            ];
            let (title, lines) = pages[hud.tutorial_page.min(pages.len() - 1)];
            push_text(&mut verts, title, 0.29, 0.16, 0.025, aspect, Palette::ROOF_ORANGE);
            for (i, line) in lines.iter().enumerate() {
                push_rect(&mut verts, 0.20, 0.27 + i as f32 * 0.115, 0.60, 0.085, [0.17, 0.22, 0.29, 0.95]);
                push_text(&mut verts, line, 0.245, 0.298 + i as f32 * 0.115, 0.018, aspect, Palette::PAPER);
            }
            for (x, w, label) in [(0.08, 0.24, "PREVIOUS"), (0.38, 0.24, "CLOSE"), (0.68, 0.24, "NEXT")] {
                push_rect(&mut verts, x, 0.82, w, 0.10, Palette::ROOF_ORANGE);
                push_text(&mut verts, label, x + 0.045, 0.86, 0.017, aspect, Palette::INK);
            }
        }
        _ => {
            push_rect(&mut verts, 0.16, 0.12, 0.68, 0.76, Palette::INK);
            push_rect(&mut verts, 0.166, 0.128, 0.668, 0.744, Palette::PAPER);
        }
    }

    verts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hud_generates_circular_buttons_icons_and_numeric_counters() {
        let settings = GameSettings::default();
        let hud = HudSnapshot::default();
        let verts = build_ui_vertices(ScreenState::InGameHud, &hud, &settings, 16.0 / 9.0);
        assert!(verts.len() > 600, "Circular buttons and vector icons should emit rich triangle mesh");
        assert_eq!(verts.len() % 3, 0, "UI mesh must be valid triangle list");
    }

    #[test]
    fn menu_hit_test_maps_primary_and_secondary_buttons() {
        assert_eq!(hit_test_menu(ScreenState::Pause, 0.50, 0.37), Some(MenuAction::ResumeOrPlay));
        assert_eq!(hit_test_menu(ScreenState::Pause, 0.50, 0.48), Some(MenuAction::RestartCheckpoint));
        assert_eq!(hit_test_menu(ScreenState::Pause, 0.50, 0.58), Some(MenuAction::RestartLevel));
        assert_eq!(hit_test_menu(ScreenState::Pause, 0.50, 0.68), Some(MenuAction::OpenTutorial));
        assert_eq!(hit_test_menu(ScreenState::MainMenu, 0.50, 0.48), Some(MenuAction::NewRun));
        assert_eq!(hit_test_menu(ScreenState::MissionSummary { victory: true }, 0.50, 0.48), Some(MenuAction::NextLevel));
        assert_eq!(hit_test_menu(ScreenState::InGameHud, 0.50, 0.45), None);
        assert_eq!(hit_test_menu(ScreenState::Settings(SettingsTab::Audio), 0.50, 0.64), Some(MenuAction::ToggleSound));
    }
}
