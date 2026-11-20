# V9 — Actual Source & Asset Integration

This is an **editable Rust Android game project**, not just a prompt or mockup. It is based on the user-provided V8 source.

## New systems implemented

### 1. Imported skeletal animation and weapon mesh
- `assets/animations/viewmodel_rig.json`: eight-joint hierarchical skeletal rig and **11 authored animation clips**, including idle, run, wall-run, slide, fall, hook, light attacks A/B/C, heavy and dimensional slash.
- `crates/render_wgpu/src/skinned.rs`: imports the JSON once at startup; samples two animation frames, interpolates rotations via quaternion SLERP, updates parent-child bone transforms and performs weighted two-joint CPU vertex skinning.
- `assets/models/inkblade.obj` + `.glb`: original 220-triangle sword model, actually imported by the skinner and attached to the right wrist. The OBJ has color-coded steel, bevels, bronze guard, pommel and grip wraps; the GLB is an editable alternative asset.
- Rendering now calls `skinned::draw_viewmodel` instead of the old procedural combat-stance box rig.
- These are authored game-ready **stylized** assets, not motion-captured human performances or AAA-character meshes.

### 2. Textures actually sampled on GPU
- `render_wgpu` decodes the existing four artist PNGs during initialization via `image` PNG support and writes an sRGB **1024x1024 atlas** to a `wgpu` texture.
- The world vertex format has a fourth attribute, material index. Building triangles select plaster, sandstone, terracotta or charcoal steel.
- `shaders/world.wgsl` samples that atlas with world-projected UVs, meaning rotated and cut pieces retain material detail.
- Approximate atlas cost: 4 MiB of GPU memory; Android support must be verified on target hardware.

### 3. Real directional shadow render pass
- `render_wgpu/src/shaders/shadow.wgsl` and `SketchRenderer::shadow_pipeline`: **1024x1024 Depth32Float** camera-following directional shadow-map pass with depth bias.
- World shader uses a **3x3 percentage-closer filtered** shadow lookup. Receivers include walls, roofs, dynamically cut halves and props. First-person weapon is intentionally excluded as a shadow caster.
- This is real-time dynamic lighting/shadows, **not** ray tracing, SSAO, or global illumination.

### 4. Authored cuttable world geometry and structural collapse
- `assets/models/terracotta_roof.obj` (120 triangles) and `sandstone_tower.obj` (168 triangles) are watertight original authored meshes. Matching editable `.glb` files are supplied.
- `engine_core::mesh_slicer::SliceableMesh::from_obj_scaled` imports triangulated OBJ into the actual slicing mesh. Roofs use the imported mesh, and the demo tower base uses the imported tower asset.
- Inverse rotation plus pivot correction allows repeated slices of moving, tumbling pieces without jumps.
- `WorldStreamer::reconcile_structural_support` propagates support through connected vertical modules. Removing load-bearing pieces detaches unsupported roofs, chimneys and upper walls. AABB rotated bounds, collisions against stationary roofs, restitution, friction, gravity, spin and landing dust are updated in physics ticks.
- This is a **support-graph/rigid-fragment approximation**, not a full finite-element structural solver or continuous-collision rigid-body engine.

### 5. Heavier sword cuts and camera response
- Close-range light attacks now commit an architectural cut when the blade ray hits a sliceable wall. Heavy and dimensional cuts continue to work at longer range.
- World cuts apply hit-stop proportional to the number of severed modules. Camera recoil combines multiple non-periodic shake bands and positional kick; reduced-motion settings remain honored.
- Brush hook, tether visuals, redesigned menu, tutorial and touch controls from V8 are preserved.

## Files modified

- `crates/render_wgpu/Cargo.toml`
- `crates/render_wgpu/src/lib.rs`
- `crates/render_wgpu/src/skinned.rs` (new)
- `crates/render_wgpu/src/shaders/world.wgsl`
- `crates/render_wgpu/src/shaders/shadow.wgsl` (new)
- `crates/engine_core/src/mesh_slicer.rs`
- `crates/world_streaming/src/lib.rs`
- `crates/gameplay_combat/src/lib.rs`
- `crates/app_android/src/lib.rs`
- `assets/animations/viewmodel_rig.json` (new)
- `assets/models/{inkblade,terracotta_roof,sandstone_tower}.{obj,glb}` (new)
- `scripts/verify_v9_assets.py` (new)

## Build and verification

```bash
cd inkbound-rooftops
python3 scripts/verify_v9_assets.py
cargo fmt --all --check
cargo check --workspace
cargo test --workspace
./scripts/build_android_apk.sh debug
```

Use the Gradle / Android NDK prerequisites from `README.md`. The renderer uses an additional `image` crate (`0.24.9` with PNG feature). If WGSL validation, device capabilities, or the target GPU require changes, resolve and rerun the build steps above.

## Required device QA

1. Launch main menu. Choose tutorial, navigate all pages, resume and verify touch control labels.
2. Aim at a vertical wall; press **HOOK**. Verify orange tether, anchor, spring pull, and re-tap detach.
3. Move within melee range of the demo freestanding wall and use **light sword slash**. Verify two closed cut halves and debris (not just a trail).
4. Use **HEAVY** / **WORLD CUT** through the tower and upper roof. Verify support loss, rigid-piece fall and collision-generated dust.
5. Rotate around the falling section and slice a piece again: no teleporting or spurious fixed-orientation cut planes.
6. Look at plaster, stone and terracotta surfaces: textures should be visible with real directional shadows and fog depth.
7. Observe run, wall run, slide, hook, light combo, heavy and world-cut weapon animation clips.
8. Verify reduced-motion setting suppresses camera shake without disabling sword combat.
9. Profile an actual Android device for FPS, fragment counts, heat and GPU memory. Shader shadow taps may need a lower quality preset on weak devices.

## Validation status

Passed here: JSON animation-schema and chronology validation; OBJ and GLB asset presence/format checks; shader material-binding source consistency; PNG resolution checks; standalone source-asset checker.

**Not performed here**: Rust compilation, `wgpu` shader validation by GPU, Android packaging, or real device tests—there is no Rust/NDK toolchain or device available in this execution environment. This project is an implemented source upgrade, **not** a verified finished APK. Do not consider a feature device-working until the tests above pass on your phone.
