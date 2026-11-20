# V8 — Implemented source changes and remaining work

This archive is a **source-code upgrade**, not a verified Android release.

## Source changes actually made

- `crates/render_wgpu/src/lib.rs`: weapon model now has additional blade-edge geometry, bronze pommel, guard endcaps, grip wraps, and layered material details. All additions follow the existing procedural animated weapon basis, including its light/heavy/dimensional attack poses.
- `crates/render_wgpu/src/lib.rs`: brush-hook tether now includes a bright inner strand and brighter attachment core.
- `crates/render_wgpu/src/shaders/world.wgsl`: additional sky fill, warm bounced light and fine procedural surface grain, integrated into the existing fog and toon lighting.
- `assets/materials/`: four original 512px tileable-ish reference textures. These are **not loaded by the renderer**; the current renderer uses vertex colors and procedural WGSL shading. The textures are provided for a future material/UV pipeline.

## Existing systems retained from V7

- CPU world-plane mesh splitting, caps, rubble fragments and falling-piece simulation.
- Procedural animated two-handed first-person weapon poses.
- Touch UI, tutorial and hook activation paths.
- Camera kick and reduced-motion settings.

## Not completed / not claimed

- Imported, production-grade skinned 3D characters and animation clips (requires glTF import, skinning and animation runtime).
- Physically based shadows, SSAO and advanced global illumination (requires render passes and GPU profiling).
- Fully physically simulated building collapse, continuous collision and structural stability.
- Verified Android build, device runtime, or exact visual match to the supplied video.

## Build verification

No Rust toolchain is available in the execution environment used for this package. Run `cargo check --workspace`, `cargo test --workspace`, and your Android build script before installing on device. The project is not represented as compiled or tested.
