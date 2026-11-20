# V5 source modifications and limitations

## Implemented in this patch
- Visible 3D brush-hook tether, orange wall anchor marker, and active-hook crosshair ring. Tether is rendered using the existing translucent VFX pass.
- Lower-glare warm sandstone, darker masonry, blue-gray atmospheric fog, and increased distance separation. Colors remain stylized and are not a recreation of the reference game's proprietary assets.
- Existing V3 sword combat and destruction are retained, including cut mesh caps, hit stop, and camera kick. These systems were **not fully redesigned** in this pass.

## Not completed or validated
- Properly labeled in-game menus and interactive tutorial are **not implemented**; current menu still uses icon-only controls.
- Advanced weapon animation blending, new destruction physics, and a complete lighting/shadow system are **not implemented**.
- Android APK build, device testing, and gameplay validation have not been performed.
- The project may need integration fixes before compiling; no Rust toolchain is installed in this environment.

## Controls / tutorial reference
- Left joystick: walk / steer; drag on right side: look.
- Jump: jump or wall-jump near a wall; Sprint: run faster; Dash: burst movement; Slide: slide while moving.
- Slash: quick sword attack; Heavy slash: stronger swing; Dimensional slash: charge/release a world-cut that can cut supported buildings and objects.
- Paint: change paint mode / apply brush strokes according to current touch region.
- Hook (Interact): aim at a near-vertical structural wall within 27 m and tap to attach. Costs 18 pigment; tap again to release. Hook expires after 2.4 seconds.
- Pause: pause/resume game. Existing menu icons: triangle = resume, ring = restart checkpoint, dot = reduced motion.

## Required acceptance tests
1. Hook at a valid wall; confirm visible orange line from hand to anchor and ring at crosshair.
2. Turn camera during hook; tether remains anchored in world coordinates.
3. Slice a building; verify both halves, physics, cut surface, and collision after the cut.
4. Confirm scene is legible at midday and sunset on a real Android device.
5. Verify game menus and touch targets; currently incomplete.
