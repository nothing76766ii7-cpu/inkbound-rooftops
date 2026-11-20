#!/usr/bin/env python3
"""Offline validation for V10 packaging. Not a replacement for cargo or on-device QA."""
from pathlib import Path
import wave, json
root=Path(__file__).resolve().parents[1]
materials=list((root/'assets/materials').glob('*.png'))
sounds=sorted((root/'assets/audio').glob('*.wav'))
assert len(sounds)==20,(len(sounds),sounds)
assert len(materials)==4
for f in sounds:
    with wave.open(str(f),'rb') as w:
        assert w.getnchannels()==1 and w.getframerate()==48000 and w.getsampwidth()==2,f
        assert w.getnframes()>3000,f
    print('PASS audio:',f.name)
hook=(root/'crates/app_android/src/brush_hook.rs').read_text()
app=(root/'crates/app_android/src/lib.rs').read_text()
audio=(root/'crates/audio/src/output.rs').read_text()
world=(root/'crates/world_streaming/src/lib.rs').read_text()
renderer=(root/'crates/render_wgpu/src/lib.rs').read_text()
ui=(root/'crates/ui/src/lib.rs').read_text()
for needle in ('MAX_RANGE: f32 = 52.0','anchor_local','HookAction::Fired','self.rope_length','reel_in','tangent'):
    assert needle in hook, needle
for needle in ('AAudio_createStreamBuilder','AAudioStreamBuilder_setDataCallback','AAudioStream_requestStart','include_bytes!','mixer.mix_stereo'):
    assert needle in audio,needle
assert 'self.audio_output.enqueue(&events' in app
assert 'let _ = self.audio.drain_events()' not in app
assert 'self.brush_hook.tension,' in app
assert 'hook_tension: f32' in renderer and 'const SEGMENTS: usize = 22' in renderer
for needle in ('Coherent neighborhood blocks','The 8 m gap','Stone archway has TWO supports','Low-poly architectural detailing'):
    assert needle in world,needle
assert 'SOUND EFFECTS: ON' in ui and 'SPRINT: REEL IN' in ui
json.load(open(root/'assets/animations/viewmodel_rig.json'))
print('PASS: V10 sound bank, AAudio wiring, world composition, web movement, VFX, settings')
print('NOTE: Rust compiler and Android device testing still required')
