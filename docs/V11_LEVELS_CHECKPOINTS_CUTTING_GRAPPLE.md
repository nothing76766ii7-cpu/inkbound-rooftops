# Inkbound Rooftops V11 — gameplay source changes

This archive contains **edited Rust source code**, not just a game design document. It builds on the supplied V10 project and preserves V10 audio, assets, terrain materials, models, UI icons and Android build scripts.

## Levels and procedural worlds

- **Level 1 — Inkwell Quarter:** authored tutorial/cutting district, with a new alley checkpoint and visible exit shrine in the plaza.
- **Level 2 — Saffron Market:** seeded 8-roof traversal course.
- **Level 3 — High Chimneys:** seeded 10-roof course.
- **Level 4 — Copper Canyon:** seeded 12-roof course.
- **Level 5 — Spirefall District:** seeded 14-roof course.
- Reach the glowing teal beacon on the final roof to open the level-completed menu, then tap **NEXT LEVEL**. At level 5, the option becomes **GENERATE NEW RUN**.
- Procedural layouts have bounded rooftop jump gaps, varying elevation, climbable side facades, hookable towers and supporting architectural scenery. The same saved world seed creates the same course.

## Save, checkpoints, restart and new run

- Teal pads mark checkpoints. Walk onto one to activate it and autosave. The HUD briefly displays **CHECKPOINT SAVED**.
- **RESTART CHECKPOINT:** builds a fresh copy of the level, restores only the cuts that had occurred when the checkpoint activated, resets enemies/combat/paint/web states and respawns on that pad.
- **RESTART LEVEL:** rebuilds an intact copy of the current level and starts on the original starting roof, discarding current level cut history.
- **GENERATE NEW RUN:** resets to level 1 with a newly generated world seed for procedural levels 2–5.
- Automatic rescue after falling and death use checkpoint restart too, not just a player teleport.
- Saves now include `level_number`, `world_seed` and `checkpoint_cut_count`. Existing V10 save files migrate with defaults to level 1.

## More consistent slash behavior

- The heavy and dimensional slash planes now follow the **actual center of the first aimed sliceable object**, rather than slicing through empty air at the camera’s elevation. Close-range light slashes also center their plane on the struck module.
- Mobile **SLASH** fires on touch-down rather than after lifting the finger. Holding the button triggers a heavy follow-up on release.
- The existing V10 watertight mesh cutting, cap surfaces, falling fragments, sound and screen shake remain enabled.

## Web/string and jumping

- Web constraint is solved after movement each frame so movement acceleration no longer cancels the rope pull. Tension cancels most outward radial velocity, adds a bounded stretch correction, and preserves sideways swing velocity.
- Web rendering vibrates less when taut. Aim at walls/roof edges and tap **HOOK**. Steer with the stick, sprint to reel, jump to launch, tap hook again to detach.
- Larger Android JUMP target, shorter joystick deadzone, ~190ms input jump buffer, ~170ms coyote time, stronger launch, smoothly varying jump height.

## Main menu actions

`CONTINUE`, `GENERATE NEW RUN`, `RESTART LEVEL`, `HOW TO PLAY`, `SETTINGS`, `RESTART CHECKPOINT`.

Pause menu: `RESUME`, `RESTART CHECKPOINT`, `RESTART LEVEL`, `HOW TO PLAY`, `SETTINGS`, `MAIN MENU`.

The fifth tutorial page explains levels, the exit beacon, checkpoints and restart choices.

## Build on your own machine

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
./scripts/build_android_apk.sh debug
```

**Verification limitation:** The file ZIP and source-level checks have been performed in the packaging environment, but `cargo`/`rustc` and Android SDK/NDK are not installed here. Compilation and Android device gameplay must be checked on your machine. Do not treat the source patch as a pre-built APK.

### Device tests to run

1. On a clean save, jump twice across the starting rooftops. Try short and held jump taps with two fingers on the screen.
2. Slice a wall with light slash, then heavy slash; aim the dimensional slash at a roof and check that the mesh actually splits.
3. Fire HOOK at a high tower and swing; verify the web stays attached without rubber-band stretching. Release and re-attach.
4. Reach Level 1 checkpoint on the alley roof, cut a building, then restart at checkpoint; verify that only older cuts remain.
5. Restart the level and verify buildings, enemies and starting position are restored.
6. Reach the Level 1 teal exit beacon and play Levels 2–5; compare two generated runs for variation.
7. Close/reopen the app and verify level, seed and checkpoint are restored correctly.
