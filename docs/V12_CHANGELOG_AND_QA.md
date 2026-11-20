# V12 Change Log and Build QA

## Code changes

- `crates/world_streaming/src/campaign.rs` — six places, 24 level names, seasons, weather, 18 story panels, daytime lighting and fog definition.
- `crates/world_streaming/src/lib.rs` — longer procedurally generated rooftop courses, scaled challenge, web spans, checkpoint safeguards, route waypoint metadata, chapter-specific scene structures.
- `crates/gameplay_combat/src/lib.rs` — only `AttackKind::Dimensional` commits world-cut planes. Light, Heavy, blade props and wall clangs never modify environment meshes.
- `crates/app_android/src/lib.rs` — level cap raised to 24, chapter storyboard transitions, day/night simulation and save state, region/season/clock HUD, route completion display.
- `crates/ui/src/lib.rs` — actual illustrated story screen with working Next/Skip buttons, campaign labels, improved tutorial language and chapter-dependent illustrations.
- `crates/render_wgpu/src/lib.rs` and `shaders/world.wgsl` — sun direction changes with clock; seasonal material tint, sky and fog palette; lightweight weather particles.
- `crates/persistence/src/lib.rs` — save schema 4, backward-compatible default for day phase, keeping prior saved cuts and progress.
- `assets/storyboard/chapter_storyboard.png` — optional 18-panel art reference; the interactive story screen is rendered procedurally and does not depend on this PNG.

## Build on development machine

```bash
python3 scripts/verify_v12_source.py
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
./scripts/build_android_apk.sh debug
```

## Device QA (manual)

1. **New game**: Chapter I / Lost Ink three-page story appears. Tap NEXT twice then PLAY; verify game starts at level 1. SKIP must also work.
2. **Combat isolation**: Aim a close-range normal or heavy sword attack at a sliceable roof. It may spark but roof must remain whole. Press RIFT and verify roof slices and moves.
3. **Hard parkour**: Complete levels 2, 10, 18 and 24. Later levels should have longer and narrower jumps; web-anchored sections require a grapple or paint foothold.
4. **Checkpoint consistency**: Reach a teal pad, cut a nonessential wall with RIFT, die, restart checkpoint: pad should be intact and respawn should be safe. Restart Level should remove all cuts.
5. **Story progression**: Complete level 4 and tap NEXT LEVEL: chapter II story appears before level 5. Repeat at 8→9, 12→13, 16→17, 20→21.
6. **Weather**: Level 13 should show blue winter mood and snow; level 17 should show rain; level 10 should have smoky atmosphere.
7. **Day/night**: Stay in one level; lighting changes progressively with time, without resetting camera/player; pause freezes clock; load save should preserve clock.
8. **Input**: Confirm sword, rift, hook, jump, camera swipe and pause all work on multi-touch device.
9. **Performance**: Profile GPU load and battery on lower-end devices, especially long regions and cut-heavy scenes. If needed, reduce particle counts and lod0 radius.

## Verification status

`verify_v12_source.py` performs offline source/asset checks. It does **not** parse WGSL, compile Rust or prove Android build/device gameplay. These checks must run on a machine with a Rust toolchain and Android SDK/NDK. Any failures should be resolved before deployment.
