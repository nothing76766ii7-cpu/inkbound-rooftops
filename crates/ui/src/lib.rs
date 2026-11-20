//! Sketchbook UI design system, screen state machine, HUD layout, and 2D quad
//! generator (PRD section 3.12).

use engine_core::{GameSettings, Palette};
use gameplay_paint::PaintMode;
use platform_input::{InputRouter, TouchButton};
use serde::{Deserialize, Serialize};

/// Complete screen state machine covering PRD section 3.12 (A–K).
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

/// Snapshot of gameplay telemetry rendered onto the reference-aligned minimal HUD.
#[derive(Debug, Clone)]
pub struct HudSnapshot {
    pub health: f32,
    pub max_health: f32,
    pub stamina: f32,
    pub max_stamina: f32,
    pub pigment: f32,
    pub max_pigment: f32,
    pub slash_charge_ratio: f32,
    pub objective_done: u32,
    pub objective_total: u32,
    pub paint_mode_active: bool,
    pub active_paint_mode: PaintMode,
    pub tutorial_hint: Option<String>,
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
            objective_done: 0,
            objective_total: 3,
            paint_mode_active: false,
            active_paint_mode: PaintMode::Brush,
            tutorial_hint: Some("Drag left to move, drag right to look, hold Slash to cut".to_string()),
        }
    }
}

/// Normalized `[0, 1]` screen-space rectangle consumed by `render_wgpu`'s 2D HUD pass.
#[derive(Debug, Clone, Copy)]
pub struct UiQuad {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub color: [f32; 4],
}

/// Generates all 2D sketchbook HUD/menu quads for the current frame.
pub fn build_ui_quads(
    screen: ScreenState,
    hud: &HudSnapshot,
    settings: &GameSettings,
) -> Vec<UiQuad> {
    let mut quads = Vec::with_capacity(48);
    let alpha = settings.hud_opacity.clamp(0.35, 1.0);

    match screen {
        ScreenState::InGameHud => {
            // 1. Top-left thin red health bar (100/100 reference style) with charcoal ink border
            let hp_ratio = (hud.health / hud.max_health.max(1.0)).clamp(0.0, 1.0);
            quads.push(UiQuad {
                x: 0.03,
                y: 0.035,
                w: 0.22,
                h: 0.026,
                color: Palette::INK,
            });
            quads.push(UiQuad {
                x: 0.033,
                y: 0.039,
                w: 0.214,
                h: 0.018,
                color: Palette::PAPER,
            });
            quads.push(UiQuad {
                x: 0.033,
                y: 0.039,
                w: 0.214 * hp_ratio,
                h: 0.018,
                color: Palette::HEALTH_RED,
            });

            // Stamina (charcoal) & Pigment (orange/teal) thin sub-bars directly beneath health
            let stam_ratio = (hud.stamina / hud.max_stamina.max(1.0)).clamp(0.0, 1.0);
            let pig_ratio = (hud.pigment / hud.max_pigment.max(1.0)).clamp(0.0, 1.0);
            quads.push(UiQuad {
                x: 0.03,
                y: 0.068,
                w: 0.16 * stam_ratio,
                h: 0.008,
                color: Palette::INK,
            });
            quads.push(UiQuad {
                x: 0.03,
                y: 0.082,
                w: 0.16 * pig_ratio,
                h: 0.008,
                color: if hud.paint_mode_active {
                    Palette::PAINT_TEAL
                } else {
                    Palette::ROOF_ORANGE
                },
            });

            // 2. Top-center objective counter badge (0/3 reference-aligned sparse counter)
            quads.push(UiQuad {
                x: 0.45,
                y: 0.032,
                w: 0.10,
                h: 0.034,
                color: Palette::INK,
            });
            quads.push(UiQuad {
                x: 0.453,
                y: 0.036,
                w: 0.094,
                h: 0.026,
                color: Palette::PAPER,
            });
            let obj_ratio = if hud.objective_total == 0 {
                0.0
            } else {
                (hud.objective_done as f32 / hud.objective_total as f32).clamp(0.0, 1.0)
            };
            quads.push(UiQuad {
                x: 0.453,
                y: 0.056,
                w: 0.094 * obj_ratio,
                h: 0.006,
                color: Palette::ROOF_ORANGE,
            });

            // 3. Center reticle (colorblind-safe shape cue: wider orange bar while charging slash)
            let reticle_color = if hud.paint_mode_active {
                Palette::PAINT_TEAL
            } else if hud.slash_charge_ratio > 0.1 {
                Palette::ROOF_ORANGE
            } else {
                Palette::INK
            };
            quads.push(UiQuad {
                x: 0.496,
                y: 0.494,
                w: 0.008,
                h: 0.012,
                color: reticle_color,
            });
            if hud.slash_charge_ratio > 0.05 {
                let cw = 0.10 * hud.slash_charge_ratio;
                quads.push(UiQuad {
                    x: 0.5 - cw * 0.5,
                    y: 0.53,
                    w: cw,
                    h: 0.009,
                    color: Palette::ROOF_ORANGE,
                });
            }

            // 4. Touch buttons (safe-area friendly, respects left_handed & hud_scale)
            for reg in InputRouter::button_regions(settings) {
                let size = reg.radius * 1.1;
                let fill = match reg.button {
                    TouchButton::Slash => [
                        Palette::ROOF_ORANGE[0],
                        Palette::ROOF_ORANGE[1],
                        Palette::ROOF_ORANGE[2],
                        alpha * 0.85,
                    ],
                    TouchButton::Paint => [
                        Palette::PAINT_TEAL[0],
                        Palette::PAINT_TEAL[1],
                        Palette::PAINT_TEAL[2],
                        alpha * 0.85,
                    ],
                    _ => [
                        Palette::PAPER[0],
                        Palette::PAPER[1],
                        Palette::PAPER[2],
                        alpha * 0.72,
                    ],
                };
                // Outer ink stroke
                quads.push(UiQuad {
                    x: reg.center.x - size * 0.55,
                    y: reg.center.y - size * 0.55,
                    w: size * 1.1,
                    h: size * 1.1,
                    color: [Palette::INK[0], Palette::INK[1], Palette::INK[2], alpha],
                });
                // Inner fill
                quads.push(UiQuad {
                    x: reg.center.x - size * 0.50,
                    y: reg.center.y - size * 0.50,
                    w: size,
                    h: size,
                    color: fill,
                });
            }
        }
        ScreenState::MainMenu | ScreenState::Pause | ScreenState::Splash => {
            // Sketchbook paper backdrop card with ink frame and orange accent stripe
            quads.push(UiQuad {
                x: 0.25,
                y: 0.18,
                w: 0.50,
                h: 0.64,
                color: Palette::INK,
            });
            quads.push(UiQuad {
                x: 0.256,
                y: 0.188,
                w: 0.488,
                h: 0.624,
                color: Palette::PAPER,
            });
            quads.push(UiQuad {
                x: 0.256,
                y: 0.188,
                w: 0.488,
                h: 0.035,
                color: Palette::ROOF_ORANGE,
            });
            // Primary action button ("Play / Resume")
            quads.push(UiQuad {
                x: 0.35,
                y: 0.42,
                w: 0.30,
                h: 0.10,
                color: Palette::INK,
            });
            quads.push(UiQuad {
                x: 0.354,
                y: 0.426,
                w: 0.292,
                h: 0.088,
                color: Palette::ROOF_ORANGE,
            });
        }
        _ => {
            // Generic modal panel for Settings / Upgrades / WorldMap / SaveSelect
            quads.push(UiQuad {
                x: 0.15,
                y: 0.12,
                w: 0.70,
                h: 0.76,
                color: Palette::INK,
            });
            quads.push(UiQuad {
                x: 0.156,
                y: 0.128,
                w: 0.688,
                h: 0.744,
                color: Palette::PAPER,
            });
        }
    }

    quads
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hud_generates_health_objective_and_touch_quads() {
        let settings = GameSettings::default();
        let hud = HudSnapshot::default();
        let quads = build_ui_quads(ScreenState::InGameHud, &hud, &settings);
        assert!(quads.len() >= 18);
    }
}
