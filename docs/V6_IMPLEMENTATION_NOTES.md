# Inkbound V6 — implementation notes (source patch, not tested APK)

## What changed in executable Rust
- Added the previously missing **HOOK** touch button (`TouchButton::Interact`) to the right-hand control cluster. Previously the hook code existed but **no button region exposed it**, so Android users could not invoke it.
- Hook button is wired through the existing `InputRouter` event and `GameApp::step_simulation`, with anchor raycast and spring pull; increased wall raycast range to 38 m and active duration to 3.8 s.
- The existing orange hook tether renders while a valid anchor is active. Aim the reticle at a structural wall, tap HOOK; tap again to release. Hook costs 18 pigment, so ensure pigment is available.
- New readable vector-font labels in the pause/main menu; actual HOW TO PLAY screen with each control and touch-to-return behavior. The app now opens in the main menu instead of directly in gameplay.
- Added oscillating camera translation driven by existing camera_kick for visible impact shake (respects reduced_motion).
- Reduced the size and darkness of viewmodel forearm boxes. The large dark shape in footage appears to be the **procedurally generated first-person sword/arm mesh**, not a UI button; its appearance still needs an artist-made skinned rig for production quality.
- Darkened the menu overlay to reduce the washed-out white appearance.

## Assets
`assets/ui/icons/` includes 12 source SVGs for the UI art direction. The game currently draws its HUD icons as Rust vector geometry, not as texture-loaded SVGs; the assets are provided for future atlas import. `assets/reference/visual_direction.png` is concept art, **not an in-game environment texture**.

## Limitations / QA
No Rust compiler, Cargo or Android SDK is installed in this execution environment. Compilation, Android touch validation, and real-device GPU testing were **not performed**. Camera shake and UI need on-device tuning. Building destruction remains the existing mesh slicing implementation, not a new high-fidelity debris system. Art assets do not automatically replace procedural world meshes. Verify hook activation on actual walls, HUD placement on unusual aspect ratios, and menu legibility before release.

## Test plan
1. Launch: Main Menu shows readable PLAY / RESUME, RESTART CHECKPOINT, HOW TO PLAY.
2. Tap HOW TO PLAY, read every control, tap BACK TO GAME, then tap PLAY / RESUME.
3. Aim center reticle at a nearby vertical building wall with pigment >=18, press HOOK. Verify orange tether and pull. Press HOOK again to release.
4. Slash an actual sliceable building. Check fragments, impact shake, and no clipped viewmodel. Toggle reduced_motion to verify shake suppression.
5. Verify Android multitouch: left joystick + right look + hook and slash.
