# V4 source patch — implementation status

## Implemented in Rust source
- `crates/app_android/src/brush_hook.rs`: wall-targeted brush hook using world raycast, pigment cost, spring-damped pull, sideways momentum retention, timeout, detachment, cooldown, and speed cap.
- `crates/app_android/src/lib.rs`: hook integrated into fixed simulation before player movement; separate from paint stroke activation.
- `crates/platform_input/src/lib.rs`: desktop G key action through `trigger_interact`. Mobile Interact button already existed and is reused for hook.
- Imported 12 SVG icon designs and concept artwork into `assets/`; **these are reference assets, not yet wired into the wgpu renderer**.
- Included full V4 PRD in `docs/`.

## Not implemented / not verified
- Building destruction, existing world slicing and animations are not changed by this patch.
- The hook's tether is not rendered; the spring behavior is implemented but collision and swing feel need testing on device.
- V4 menu redesign, tutorial overlays, lighting/shadows, audio impact design, new animation rigs and visual polish are not implemented.
- No Rust compiler or Android toolchain was available in this environment; no `cargo check`, automated tests, or APK build were run. This ZIP is source, not a working APK.

## Controls
- Mobile: Interact button = attach/detach brush hook when aiming at a vertical wall within 27m; requires 18 pigment. Paint button continues to control paint mode.
- Desktop: G = brush hook, F = dimensional slash, R = heavy slash, E = paint mode, Q = dash, Space = jump, Shift = sprint, Ctrl = slide, WASD = move, mouse = look, Esc = pause.
- Hook expires after 2.4 seconds, or when reaching the anchor; repeat input detaches.

## Suggested next implementation passes
1. Compile and test hook on device; resolve input region and camera collision issues.
2. Add visible tether and reticle plus contextual hook tutorial in the wgpu UI renderer.
3. Implement material/lighting changes and replace procedural menu visuals with polished labeled icons.
4. Improve sword animation curves and impact response; profile world-cut geometry and fragment budgets on Android.
