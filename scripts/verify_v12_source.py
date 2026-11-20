#!/usr/bin/env python3
"""Offline structural V12 source and asset validation (does not compile Rust/WGSL)."""
from pathlib import Path
import struct

root = Path(__file__).resolve().parents[1]
required = {
    'crates/world_streaming/src/campaign.rs': [
        'pub const CAMPAIGN_LEVELS: u32 = 24', 'pub fn level_theme(',
        'pub fn story_beat(', 'pub fn atmosphere(', 'Weather::Snow', 'Weather::Rain',
        '"RESTORE THE SKY"',
    ],
    'crates/world_streaming/src/lib.rs': [
        'Self::build_generated_level(level_number.min(CAMPAIGN_LEVELS), seed)',
        'let count = (14 + theme.chapter * 3 + theme.act * 2).min(36)',
        'pub route_roofs: Vec<Vec3>', 'let protected_cp =',
        'let body_layers = if protected_cp { floor } else { sliceable }',
        'chapter == 3', 'long_campaign_routes_are_seeded_and_get_more_demanding',
    ],
    'crates/gameplay_combat/src/lib.rs': [
        'if kind == AttackKind::Dimensional {', 'out.committed_world_cut = Some(plane)',
        'fn enemy_sword_buttons_never_emit_world_cut_planes',
        'fn dimensional_attack_emits_real_world_cut_plane',
    ],
    'crates/app_android/src/lib.rs': [
        's.level_number.clamp(1, CAMPAIGN_LEVELS)',
        'self.day_phase = (self.day_phase + FIXED_DT / 600.0).fract()',
        'MenuAction::NextStoryPanel', 'ScreenState::StoryBoard',
        '&atmosphere(self.level_number, self.day_phase)', 'route_progress,',
    ],
    'crates/ui/src/lib.rs': [
        'StoryBoard,', 'MenuAction::SkipStory', 'MenuAction::NextStoryPanel',
        'ROOF {} / {}', 'LEVEL {}/24', 'RIFT BUTTON: CUT BUILDINGS',
    ],
    'crates/persistence/src/lib.rs': [
        'CURRENT_SCHEMA_VERSION: u32 = 4', 'pub day_phase: f32',
        'obj.entry("day_phase")',
    ],
    'crates/render_wgpu/src/lib.rs': [
        'env_sky:', 'push_weather_particles(', 'atmosphere.sun_direction',
        'r: atmosphere.sky[0] as f64',
    ],
    'crates/render_wgpu/src/shaders/world.wgsl': [
        'env_sky: vec4<f32>', 'env_fog: vec4<f32>', 'camera.env_tint.rgb',
        'camera.env_sun.xyz',
    ],
}
for filename, needles in required.items():
    text = (root / filename).read_text(encoding='utf-8')
    for needle in needles:
        assert needle in text, f'Missing marker {needle!r}: {filename}'

# Strict exclusivity of world-cut event assignments in combat crate.
combat = (root / 'crates/gameplay_combat/src/lib.rs').read_text()
assert combat.count('out.committed_world_cut = Some(') == 1, (
    'World cuts must only originate from the dedicated Dimensional ability.')

# Shader uniforms must agree with the Rust C-layout structure.
render = (root / 'crates/render_wgpu/src/lib.rs').read_text()
shader = (root / 'crates/render_wgpu/src/shaders/world.wgsl').read_text()
for field in ('view_proj', 'camera_pos', 'fx_params', 'env_sky', 'env_fog', 'env_sun', 'env_tint'):
    assert f'{field}:' in render and f'{field}:' in shader, field

# Image signatures and WAV RIFF headers (no need for image/audio decoders).
assets = [
    'assets/storyboard/chapter_storyboard.png',
    'assets/materials/sandstone.png',
    'assets/materials/plaster.png',
    'assets/materials/terracotta.png',
    'assets/models/inkblade.glb',
    'assets/models/terracotta_roof.glb',
    'assets/audio/hook_attach.wav',
    'assets/audio/slash_impact.wav',
    'assets/audio/footstep_roof.wav',
]
for filename in assets:
    path = root / filename
    assert path.is_file() and path.stat().st_size > 36, f'Missing / empty {filename}'
    with path.open('rb') as f:
        sig = f.read(12)
    if filename.endswith('.png'): assert sig[:8] == b'\x89PNG\r\n\x1a\n', filename
    if filename.endswith('.wav'): assert sig[:4] == b'RIFF' and sig[8:12] == b'WAVE', filename
    if filename.endswith('.glb'): assert sig[:4] == b'glTF', filename
print('PASS: V12 source markers, world-cut exclusivity, shader uniform names, assets and headers')
print('NOTE: Run cargo fmt/check/test and Android build on a machine with Rust/SDK/NDK installed.')
