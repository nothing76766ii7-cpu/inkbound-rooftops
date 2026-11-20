# Inkbound Rooftops — Technical Architecture

## 1. Authoritative Rust Core & Platform Boundary
All simulation, physics, world streaming, combat, paint state, AI, UI layout/state, audio routing, rendering orchestration, and persistence live in the Rust workspace (`crates/`).

Non-Rust Android glue is strictly isolated to `android/`:
- `AndroidManifest.xml`: Vulkan 1.1 hardware requirement, landscape orientation, `libinkbound_rooftops.so` binding.
- `MainActivity.kt`: Thin `NativeActivity` subclass handling immersive sticky fullscreen, `LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES`, and keep-screen-on.
- `build.gradle.kts`: `arm64-v8a` ABI packaging (`minSdk = 29`, `targetSdk = 35`).

## 2. Deterministic Simulation Loop
- **Fixed Step**: 60 Hz (`FIXED_DT = 1.0 / 60.0` seconds) managed by `engine_core::FixedTimestepClock`.
- **Spiral-of-Death Guard**: Maximum 4 simulation substeps per frame; delta clamped to `0.1s` after background/resume events.
- **Render Interpolation**: Renderer receives interpolated camera transforms (`alpha = accumulator / FIXED_DT`) so 30 FPS and 60 FPS devices share identical parkour jump arcs and wall-run timing.

## 3. Collision Layers & Dimensional Slash World Safety
Static architecture is assigned `CollisionLayer::STRUCTURAL`. Dimensional slash queries explicitly filter against `CollisionLayer::SLASHABLE_ENEMY | CollisionLayer::SLASHABLE_PROP`.
- Buildings **never** lose structural colliders or develop holes in parkour geometry.
- Dramatic spatial separation is achieved via a screen-space/shader dimensional rift pulse (`render_wgpu/src/shaders/world.wgsl`) plus pre-authored split offset animations on sliced `Ink Wraith` enemies and tagged props.

## 4. Scalable World Streaming Architecture
- **Spatial Partition**: `world_streaming` divides the city into `48m x 48m` grid cells (`CellCoord(ix, iz)`).
- **Lifecycle**: Each `WorldCell` tracks `Unloaded -> Loading -> Active(LodLevel)` based on player cell Manhattan distance (`0..=1`: LOD0 full detail + collision proxies; `2`: LOD1 simplified silhouette; `>2`: unloaded).
- **Seam Safety**: Roof bridges spanning cell boundaries register shared collision proxies in both bordering cells so players never clip through streaming seams.
- **Vertical Slice District**: `District 01 — Inkwell Quarter` includes a tutorial alley, training courtyard, three linked rooftop parkour loops, one combat plaza, one 3-glyph paint puzzle, and the central Orange Spire landmark tower.

## 5. Rendering Pipeline (`render_wgpu`)
Forward mobile-safe pipeline targeting Vulkan on Android 10+:
1. **Opaque World Pass**: Flat/cel-shaded white/cream plaster walls (`#F5F4EF` / `#D8D8D5`), terracotta/orange roofs (`#F47B38`), stylized 2-band directional lighting, and linear atmospheric overcast fog (`#E8E9E7`).
2. **Ink Outline Pass**: Backface-culled inverted-hull charcoal outlines (`#25262B`) scaled by view distance and graphics preset (`Low` disables distant outlines).
3. **Paint Decals & Platforms Pass**: Batched surface decals and glowing translucent anchor platforms.
4. **Foreground Blade & Dimensional Rift Pass**: Depth-cleared first-person sword mesh with swing/charge animation and screen-aligned monochrome rift flash (honors `reduced_flashes`).
5. **2D Sketchbook HUD & Menu Pass**: Orthographic UI quad batch for health bar, objective counter, pigment/stamina bars, touch controls, and menu screens.
