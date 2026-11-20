# V10 — Coherent District, Ink Web and Real Android Sound

This is an EDITABLE RUST SOURCE UPGRADE built on V9, not a mockup or prompt.

## 1. Streets, rooftops and geometry

- Replaced the irregular wall-in-front-of-camera start with an open sightline and a left-side cutting demo.
- Placed 10 main street houses on aligned lots with consistent gaps and contrasting roof heights, plus four setback skyline houses. Each new house is a cuttable masonry volume with textured roof, windows, doors, corner pilasters and cornice.
- Rebuilt the former floating wall-running billboard as a grounded wall with roof trim, and supported the stone overhead beam with two columns.
- Added two actual short roof-height kiosks in the long courtyard gap to turn an awkward jump into a linked parkour route.
- Replaced the first glaring orange/white roof surface with slate to improve local contrast.
- Existing authored rooftops, tower mesh, shadows, mesh slicing, combat and animations are retained.

## 2. Web/brush-hook controls

- Crosshair cone-assisted candidate search (center-first, small offsets); range 2.5–52 m.
- Can hit vertical architecture and roofs significantly above the player, but not empty sky or flat ground beneath the player.
- Tap HOOK to attach (G on desktop); tap again to release.
- Steering: move the left joystick (WASD) while airborne. The tangential impulse preserves pendulum momentum.
- Reel: push the left joystick to its outer 80% (sprint in gameplay) or hold Shift on desktop.
- Jump during grapple to release into a boosted parkour leap. Anchors follow moving structural collider centers; destroyed/disappearing targets detach.
- The new VFX uses three animated braided ink-web strands, gravity sag proportional to slack and a pulsing wall-contact splat.
- Ink cost 12 (was 18); 9-second max attachment, short retarget cooldown. Orange web is visibly different from blue-green valid-target reticle.

## 3. New actual sound output

- Previously, SfxEvents were queued and then discarded in `step_simulation()`; no audio output was wired.
- `audio::AudioOutput` now drains those queued game events once per redraw and mixes them into a stream using Android AAudio via Rust FFI (`libaaudio.so`). Android minSdk 29, mono sample bank at 48 kHz, stereo PCM16 output, soft saturation and 18 overlapping voices.
- 20 original PCM WAV samples are included in `assets/audio`. They are embedded in the native binary with `include_bytes!`, so there is no asset path lookup required on Android.
- New contexts: hook fire/attach/reel/release/miss, cutting stone, and debris landing; existing footsteps, jump, walljump, dash, sword hits, dimensional slash, paint, UI now audible.
- Sound-effect on/off has been added to Settings. It follows master_volume × sfx_volume.
- Desktop builds keep a PCM mixer API (`AudioOutput::render_to_pcm`) for QA, but this upgrade does not add a desktop speaker backend; runtime playback uses Android AAudio.
- These synthesized sounds are original/game-stylized, not recorded Foley or an orchestral soundtrack.

## 4. What to compile

```
cd inkbound-rooftops
python3 scripts/verify_v9_assets.py
python3 scripts/verify_v10.py
cargo fmt --all --check
cargo check --workspace
cargo test --workspace
./scripts/build_android_apk.sh debug
```

Only the Python asset/source wiring checks could be run in the authoring environment: no Cargo/Rust compiler, Android NDK, emulator or test device was available. Device gameplay, Rust compilation, GPU validation, streaming latency, and AAudio output still require your build and QA. Do not interpret packaging checks as a successful Android build.

## 5. Phone QA

1. Launch app, Settings -> SOUND EFFECTS: ON. Confirm UI tick is audible.
2. On the roof, move to the edge and listen for footsteps, jump, landing.
3. Aim at a wall or elevated rooftop; reticle changes blue-green. Tap HOOK. Listen for launch/contact and watch the orange animated web.
4. Move left/right while swinging; hold joystick fully forward to reel, then jump for release. Tap HOOK again for manual detachment. Missing targets play a quieter misfire cue.
5. Slice demo wall, building or tower. Hear sword swoosh, sharp crack, then collision rubble rumble as pieces land.
6. Pause and toggle SFX off; confirm silence; toggle on and confirm sound returns.
7. Visit the new gap kiosk route and wall-run facade; verify collision and no visible floating columns.
8. Run a 10-minute device performance test for AAudio underruns, heat, 60 fps target and GPU-memory usage; adjust sound polyphony or world LOD if needed.

## Changed/added code

- `crates/app_android/src/brush_hook.rs` — improved gameplay physics and targeting
- `crates/app_android/src/lib.rs` — grapple controls, events, native audio dispatch
- `crates/audio/src/lib.rs` and `src/output.rs` — SFX routing and 48 kHz AAudio PCM mixer
- `crates/world_streaming/src/lib.rs` — street grid, structural placements, facade details, landing event
- `crates/render_wgpu/src/lib.rs` — braided web VFX geometry
- `crates/ui/src/lib.rs` — hook tutorial and sound switch
- `assets/audio/*.wav` — original sound clips
- `scripts/verify_v10.py` — package validation
- `scripts/generate_original_sfx.py` — optional sound-bank regeneration
