# V7 source implementation / handoff

This is an edited Rust source tree, not an APK. No Android build or on-device tests were performed in this environment (Cargo toolchain unavailable).

## Changes actually made in code
- Replaced the bright menu panel with a dark, high-contrast illustrated menu and four labeled choices (play/resume, tutorial, settings, restart).
- Wired menu touch hit-testing to those four choices.
- Added an accessible settings screen with a functional reduced-motion toggle and back navigation.
- Added four paged tutorial screens covering movement, brush hook, sword/world cut, and painting. Touch previous/next/close buttons are handled by the app.
- Added live hook-target validity checking from the current camera ray to the structural/paintable collider set; a teal crosshair ring indicates a valid wall. Existing hook toggle, physics and visible orange tether remain.
- Increased and added roll to combat impact camera motion, with reduced-motion still respected.
- Preserved existing sword/world-cut, procedural geometry and rendering code.

## Known limitations / required manual QA
- There is no new character skeletal animation rig, authored 3D environment model set, or GPU lighting rewrite in this patch. Existing procedural rig/destruction still need work to match the reference video.
- The existing brush hook is only usable if the raycast hits a collider with a near-vertical surface within 38 world units and player has >=18 pigment; verify geometry collider flags and hook button on a physical device.
- Confirm touch coordinates, text readability, frame rate, shader compatibility, slicing collision behavior, and saved settings on Android.
- The SVG icons are design/source assets; the runtime HUD currently renders corresponding procedural vector icons rather than loading the SVG files directly.
- Cargo test/build is not verified. Run `cargo test --workspace` and the existing Android build instructions locally.
