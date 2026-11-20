//! Audio routing, dynamic dimensional-slash ducking, and haptic cue queue
//! (PRD section 3.13).

mod output;
pub use output::AudioOutput;

use engine_core::GameSettings;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SfxEvent {
    FootstepRoof,
    Jump,
    WallJump,
    LandingHard,
    Dash,
    SwordSwingLight,
    SwordSwingHeavy,
    DimensionalSlashRift,
    SlashImpact,
    PaintStroke,
    PaintPlatformSpawn,
    GlyphActivated,
    UiTick,
    HookFire,
    HookAttach,
    HookRelease,
    HookMiss,
    HookReel,
    StoneCrack,
    DebrisFall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HapticCue {
    LightTap,
    MediumImpact,
    HeavyRiftPulse,
}

/// Pure-Rust audio mixer state with dynamic ducking on dimensional slash.
#[derive(Debug, Clone)]
pub struct AudioEngine {
    pub music_duck_factor: f32,
    pub pending_sfx: Vec<SfxEvent>,
    pub pending_haptics: Vec<HapticCue>,
}

impl Default for AudioEngine {
    fn default() -> Self {
        Self {
            music_duck_factor: 1.0,
            pending_sfx: Vec::new(),
            pending_haptics: Vec::new(),
        }
    }
}

impl AudioEngine {
    pub fn play_sfx(&mut self, event: SfxEvent, settings: &GameSettings) {
        self.pending_sfx.push(event);
        if event == SfxEvent::DimensionalSlashRift {
            // Duck ambient/music bed sharply during dimensional slash transient.
            self.music_duck_factor = 0.25;
            if settings.haptics_enabled {
                self.pending_haptics.push(HapticCue::HeavyRiftPulse);
            }
        } else if matches!(event, SfxEvent::SlashImpact | SfxEvent::LandingHard)
            && settings.haptics_enabled
        {
            self.pending_haptics.push(HapticCue::MediumImpact);
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.music_duck_factor = (self.music_duck_factor + dt * 1.2).min(1.0);
    }

    pub fn effective_music_gain(&self, settings: &GameSettings) -> f32 {
        (settings.master_volume * settings.music_volume * self.music_duck_factor).clamp(0.0, 1.0)
    }

    pub fn effective_sfx_gain(&self, settings: &GameSettings) -> f32 {
        (settings.master_volume * settings.sfx_volume).clamp(0.0, 1.0)
    }

    pub fn drain_events(&mut self) -> (Vec<SfxEvent>, Vec<HapticCue>) {
        (
            std::mem::take(&mut self.pending_sfx),
            std::mem::take(&mut self.pending_haptics),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimensional_slash_ducks_music_and_recovers() {
        let settings = GameSettings::default();
        let mut audio = AudioEngine::default();
        let base = audio.effective_music_gain(&settings);

        audio.play_sfx(SfxEvent::DimensionalSlashRift, &settings);
        assert!(audio.effective_music_gain(&settings) < base * 0.5);

        audio.tick(1.0);
        assert!((audio.effective_music_gain(&settings) - base).abs() < 1e-4);
    }
}
