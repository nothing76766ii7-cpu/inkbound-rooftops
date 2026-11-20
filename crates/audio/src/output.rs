//! Native Android AAudio output with original embedded 48 kHz WAV effects.
//! Android minSdk 29 supports AAudio. No Java audio activity or third-party audio crate required.
//! Desktop builds keep the clips and game event routing but do not initialize AAudio.
use crate::SfxEvent;

const SAMPLE_RATE: usize = 48_000;
const MAX_VOICES: usize = 18;

const BANK: &[&[u8]] = &[
    include_bytes!("../../../assets/audio/footstep_roof.wav"),
    include_bytes!("../../../assets/audio/jump.wav"),
    include_bytes!("../../../assets/audio/wall_jump.wav"),
    include_bytes!("../../../assets/audio/landing_hard.wav"),
    include_bytes!("../../../assets/audio/dash.wav"),
    include_bytes!("../../../assets/audio/sword_light.wav"),
    include_bytes!("../../../assets/audio/sword_heavy.wav"),
    include_bytes!("../../../assets/audio/dimensional_rift.wav"),
    include_bytes!("../../../assets/audio/slash_impact.wav"),
    include_bytes!("../../../assets/audio/paint_stroke.wav"),
    include_bytes!("../../../assets/audio/paint_platform.wav"),
    include_bytes!("../../../assets/audio/glyph_activated.wav"),
    include_bytes!("../../../assets/audio/ui_tick.wav"),
    include_bytes!("../../../assets/audio/hook_fire.wav"),
    include_bytes!("../../../assets/audio/hook_attach.wav"),
    include_bytes!("../../../assets/audio/hook_release.wav"),
    include_bytes!("../../../assets/audio/hook_miss.wav"),
    include_bytes!("../../../assets/audio/hook_reel.wav"),
    include_bytes!("../../../assets/audio/stone_crack.wav"),
    include_bytes!("../../../assets/audio/debris_fall.wav"),
];

fn sound_index(event: SfxEvent) -> usize {
    match event {
        SfxEvent::FootstepRoof => 0, SfxEvent::Jump => 1,
        SfxEvent::WallJump => 2, SfxEvent::LandingHard => 3,
        SfxEvent::Dash => 4, SfxEvent::SwordSwingLight => 5,
        SfxEvent::SwordSwingHeavy => 6, SfxEvent::DimensionalSlashRift => 7,
        SfxEvent::SlashImpact => 8, SfxEvent::PaintStroke => 9,
        SfxEvent::PaintPlatformSpawn => 10, SfxEvent::GlyphActivated => 11,
        SfxEvent::UiTick => 12, SfxEvent::HookFire => 13,
        SfxEvent::HookAttach => 14, SfxEvent::HookRelease => 15,
        SfxEvent::HookMiss => 16, SfxEvent::HookReel => 17,
        SfxEvent::StoneCrack => 18, SfxEvent::DebrisFall => 19,
    }
}

/// Decode a PCM16 mono WAV without external dependencies. Original bank is always 48 kHz.
fn decode_wav(data: &[u8]) -> Result<Vec<i16>, &'static str> {
    if data.len() < 44 || &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return Err("Invalid WAV header");
    }
    let mut cursor = 12usize;
    let mut fmt_ok = false;
    while cursor + 8 <= data.len() {
        let size = u32::from_le_bytes(data[cursor+4..cursor+8].try_into().unwrap()) as usize;
        let start = cursor + 8;
        let end = start.checked_add(size).ok_or("WAV chunk overflow")?;
        if end > data.len() { return Err("Truncated WAV chunk"); }
        if &data[cursor..cursor+4] == b"fmt " && size >= 16 {
            let bits = u16::from_le_bytes(data[start+14..start+16].try_into().unwrap());
            let fmt = u16::from_le_bytes(data[start..start+2].try_into().unwrap());
            let channels = u16::from_le_bytes(data[start+2..start+4].try_into().unwrap());
            let rate = u32::from_le_bytes(data[start+4..start+8].try_into().unwrap());
            fmt_ok = fmt == 1 && bits == 16 && channels == 1 && rate == SAMPLE_RATE as u32;
        }
        if &data[cursor..cursor+4] == b"data" {
            if !fmt_ok || size % 2 != 0 { return Err("Unexpected WAV format"); }
            return Ok(data[start..end].chunks_exact(2)
                .map(|c| i16::from_le_bytes([c[0], c[1]])).collect());
        }
        cursor = end + (size & 1);
    }
    Err("WAV has no data chunk")
}

#[derive(Clone)]
struct Voice { clip: usize, position: usize, gain: f32 }

struct Mixer {
    samples: Vec<Vec<i16>>,
    voices: Vec<Voice>,
    master_gain: f32,
}

impl Mixer {
    fn new() -> Self {
        Self { samples: BANK.iter().map(|b| decode_wav(b).expect("Embedded SFX WAV must be valid")).collect(),
            voices: Vec::with_capacity(MAX_VOICES), master_gain: 0.8 }
    }
    fn enqueue(&mut self, events: &[SfxEvent], gain: f32) {
        self.master_gain = gain.clamp(0.0, 1.0);
        for event in events {
            // Repeated footsteps are the quietest voices, avoiding audio overload.
            let local = match event { SfxEvent::FootstepRoof => 0.34,
                SfxEvent::HookReel => 0.48, SfxEvent::UiTick => 0.33,
                SfxEvent::DimensionalSlashRift => 0.95,
                SfxEvent::DebrisFall => 0.85, _ => 0.72 };
            if self.voices.len() >= MAX_VOICES { self.voices.remove(0); }
            self.voices.push(Voice { clip: sound_index(*event), position: 0, gain: local });
        }
    }
    fn mix_stereo(&mut self, out: &mut [i16]) {
        let samples = &self.samples;
        let voices = &mut self.voices;
        for frame in out.chunks_exact_mut(2) {
            let mut value = 0.0;
            voices.retain_mut(|v| {
                let sample = samples[v.clip].get(v.position);
                if let Some(s) = sample {
                    value += *s as f32 / 32768.0 * v.gain;
                    v.position += 1;
                    true
                } else { false }
            });
            let sample = (value * self.master_gain).tanh();
            let pcm = (sample * 32767.0) as i16;
            frame[0] = pcm;
            frame[1] = pcm;
        }
    }
}

#[cfg(target_os = "android")]
mod android {
    use super::*;
    use std::{ffi::c_void, ptr, sync::Mutex};
    #[repr(C)] struct AAudioStreamBuilder { _private: [u8; 0] }
    #[repr(C)] struct AAudioStream { _private: [u8; 0] }
    #[link(name = "aaudio")]
    extern "C" {
        fn AAudio_createStreamBuilder(builder: *mut *mut AAudioStreamBuilder) -> i32;
        fn AAudioStreamBuilder_setDirection(builder: *mut AAudioStreamBuilder, direction: i32);
        fn AAudioStreamBuilder_setSampleRate(builder: *mut AAudioStreamBuilder, sample_rate: i32);
        fn AAudioStreamBuilder_setChannelCount(builder: *mut AAudioStreamBuilder, channels: i32);
        fn AAudioStreamBuilder_setFormat(builder: *mut AAudioStreamBuilder, format: i32);
        fn AAudioStreamBuilder_setPerformanceMode(builder: *mut AAudioStreamBuilder, mode: i32);
        fn AAudioStreamBuilder_setDataCallback(builder: *mut AAudioStreamBuilder,
            callback: Option<unsafe extern "C" fn(*mut AAudioStream, *mut c_void, *mut c_void, i32) -> i32>,
            user_data: *mut c_void);
        fn AAudioStreamBuilder_openStream(builder: *mut AAudioStreamBuilder, stream: *mut *mut AAudioStream) -> i32;
        fn AAudioStreamBuilder_delete(builder: *mut AAudioStreamBuilder) -> i32;
        fn AAudioStream_requestStart(stream: *mut AAudioStream) -> i32;
        fn AAudioStream_requestStop(stream: *mut AAudioStream) -> i32;
        fn AAudioStream_close(stream: *mut AAudioStream) -> i32;
    }
    struct Bridge { mixer: Mutex<Mixer> }
    unsafe extern "C" fn callback(_: *mut AAudioStream, user: *mut c_void,
        data: *mut c_void, frames: i32) -> i32 {
        if user.is_null() || data.is_null() || frames <= 0 { return 0; }
        let state = &*(user as *const Bridge);
        let output = std::slice::from_raw_parts_mut(data as *mut i16, frames as usize * 2);
        output.fill(0);
        if let Ok(mut mixer) = state.mixer.try_lock() { mixer.mix_stereo(output); }
        0 // AAUDIO_CALLBACK_RESULT_CONTINUE
    }
    pub struct AudioOutput { stream: *mut AAudioStream, bridge: Box<Bridge> }
    impl AudioOutput {
        pub fn new() -> Self {
            let mut bridge = Box::new(Bridge { mixer: Mutex::new(Mixer::new()) });
            let mut stream = ptr::null_mut();
            unsafe {
                let mut builder = ptr::null_mut();
                if AAudio_createStreamBuilder(&mut builder) == 0 && !builder.is_null() {
                    AAudioStreamBuilder_setDirection(builder, 0); // output
                    AAudioStreamBuilder_setSampleRate(builder, SAMPLE_RATE as i32);
                    AAudioStreamBuilder_setChannelCount(builder, 2);
                    AAudioStreamBuilder_setFormat(builder, 1); // PCM_I16
                    AAudioStreamBuilder_setPerformanceMode(builder, 12); // LOW_LATENCY
                    AAudioStreamBuilder_setDataCallback(builder, Some(callback),
                        (&mut *bridge) as *mut Bridge as *mut c_void);
                    if AAudioStreamBuilder_openStream(builder, &mut stream) != 0 { stream = ptr::null_mut(); }
                    AAudioStreamBuilder_delete(builder);
                    if !stream.is_null() && AAudioStream_requestStart(stream) != 0 {
                        AAudioStream_close(stream);
                        stream = ptr::null_mut();
                    }
                }
            }
            Self { stream, bridge }
        }
        pub fn enqueue(&mut self, events: &[SfxEvent], gain: f32) {
            if !self.stream.is_null() {
                if let Ok(mut mixer) = self.bridge.mixer.lock() { mixer.enqueue(events, gain); }
            }
        }
    }
    impl Drop for AudioOutput {
        fn drop(&mut self) {
            if !self.stream.is_null() {
                unsafe { AAudioStream_requestStop(self.stream); AAudioStream_close(self.stream); }
            }
        }
    }
}
#[cfg(target_os = "android")]
pub use android::AudioOutput;

#[cfg(not(target_os = "android"))]
pub struct AudioOutput { mixer: Mixer }
#[cfg(not(target_os = "android"))]
impl AudioOutput {
    pub fn new() -> Self { Self { mixer: Mixer::new() } }
    pub fn enqueue(&mut self, events: &[SfxEvent], gain: f32) {
        // The desktop development harness can render audio to WAV for QA. Playback is Android AAudio.
        self.mixer.enqueue(events, gain);
    }
    pub fn render_to_pcm(&mut self, stereo_pcm: &mut [i16]) { self.mixer.mix_stereo(stereo_pcm); }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wav_bank_is_valid_mono_pcm_48khz() {
        assert_eq!(BANK.len(), 20);
        for wav in BANK { assert!(decode_wav(wav).unwrap().len() > 3000); }
    }
    #[test]
    fn mixer_produces_nonzero_audio_after_slash_event() {
        let mut mixer = Mixer::new();
        mixer.enqueue(&[SfxEvent::SwordSwingHeavy], 1.0);
        let mut output = vec![0i16; 16_000];
        mixer.mix_stereo(&mut output);
        assert!(output.iter().any(|s| *s != 0));
    }
}
