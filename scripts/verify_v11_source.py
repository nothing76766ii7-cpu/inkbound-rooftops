#!/usr/bin/env python3
"""Fast offline V11 source/asset consistency checks (NOT a Rust compiler)."""
from pathlib import Path
import struct

root = Path(__file__).resolve().parents[1]
markers = {
    'crates/world_streaming/src/lib.rs': ['fn build_level(', 'fn build_generated_level(', 'pub level_exit: Vec3', 'level_{level_number}_checkpoint_{i}'],
    'crates/app_android/src/lib.rs': ['fn restart_checkpoint(', 'fn restart_level(', 'fn advance_level(', 'fn start_new_run(', 'checkpoint_cut_count'],
    'crates/app_android/src/brush_hook.rs': ['outward_speed * 0.96', 'player.position = proposed'],
    'crates/gameplay_combat/src/lib.rs': ['target.bounds.center()', 'CollisionLayer::SLICEABLE_WORLD'],
    'crates/gameplay_player/src/lib.rs': ['jump_buffer_time: 0.19', 'coyote_time: 0.17'],
    'crates/platform_input/src/lib.rs': ['TouchButton::Slash => self.pending_light_slash = true'],
    'crates/ui/src/lib.rs': ['GENERATE NEW RUN', 'RESTART CHECKPOINT', 'RESTART LEVEL', 'LEVEL {} COMPLETE'],
    'crates/persistence/src/lib.rs': ['CURRENT_SCHEMA_VERSION: u32 = 3', 'pub level_number: u32'],
}
for filename, needles in markers.items():
    text = (root / filename).read_text(encoding='utf8')
    for needle in needles:
        assert needle in text, f'Missing {needle!r} in {filename}'
for file in ['assets/models/inkblade.obj', 'assets/models/terracotta_roof.obj', 'assets/models/sandstone_tower.obj',
             'assets/materials/sandstone.png', 'assets/materials/terracotta.png', 'assets/audio/hook_attach.wav',
             'assets/audio/slash_impact.wav', 'assets/audio/jump.wav']:
    assert (root / file).is_file(), f'Asset missing: {file}'
print('V11 source markers and important assets: OK')
print('NOTE: This check does not parse or compile Rust; run cargo check/test on your machine.')
