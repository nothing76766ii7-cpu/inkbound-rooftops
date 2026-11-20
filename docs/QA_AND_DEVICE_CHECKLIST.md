# QA Plan, Device Compatibility Checklist & Known Limitations

## 1. Automated Test Suite
Run locally or in GitHub Actions CI:
```bash
cargo test --workspace --exclude app_android --exclude render_wgpu
```
Covers:
- `engine_core`: Collision layer isolation, ray-AABB hits, capsule depenetration, fixed-timestep catch-up clamping.
- `platform_input`: Multitouch pointer ownership (left joystick vs right look), long-press dimensional slash charge & cancel-by-drag-off, left-handed mirroring.
- `gameplay_player`: Coyote time, jump buffering, wall-run stamina & wall-jump impulse, slide boost clamp, mantle assist, fall rescue to checkpoint.
- `gameplay_combat`: Light/heavy/charged dimensional slash, energy/cooldown enforcement, strict structural building immunity vs. enemy/prop slicing, paint-mark damage amplification.
- `gameplay_paint`: Pigment consumption/refill, per-cell stroke cap & LRU eviction, temporary platform anchor validation & expiration, 3-glyph ordered puzzle completion.
- `world_streaming`: Cell activation/deactivation around player, LOD transitions, seam-safe collision queries.
- `ai`: Ink Wraith state transitions (Patrol -> Chase -> Telegraph -> Attack -> Stagger -> Defeated -> Reset) and paint-marked vulnerability.
- `persistence`: Schema versioning, atomic temp+rename save writes, v0 -> v1 migration, corrupted JSON recovery.

## 2. Manual Android Hardware Matrix (To Execute On Device)
| Tier | Example Candidate | Min Spec | Target FPS | Status |
|---|---|---|---|---|
| Low | Mid-2020 Snapdragon 665 / Mali-G52 | Android 10, 4 GB RAM, Vulkan 1.1 | 30 FPS (Low preset) | Pending hardware test |
| Mid | Snapdragon 7 Gen 1 / Dimensity 7050 | Android 12, 6 GB RAM, Vulkan 1.1 | 30–60 FPS (Balanced) | Pending hardware test |
| High | Snapdragon 8 Gen 2+ | Android 13+, 8+ GB RAM, Vulkan 1.3 | 60 FPS (High preset) | Pending hardware test |

### Required On-Device Scenarios
- [ ] Cold launch to playable rooftop district in < 60 seconds.
- [ ] Simultaneous left-thumb joystick + right-thumb look + jump/slash multitouch without pointer drops.
- [ ] Display cutout / punch-hole safe area verification in both landscape orientations.
- [ ] App backgrounding/resuming while painting or mid-jump (autosave triggered, Vulkan surface recreated cleanly).
- [ ] Phone call / audio focus interruption recovery.
- [ ] Dimensional slash aimed directly at structural buildings (buildings remain collision-stable; Ink Wraiths slice cleanly).
- [ ] Temporary paint platform expiration while standing on it (clean fall + checkpoint rescue, no crash or soft-lock).
- [ ] 20-minute sustained playtest logging frame times (`adb shell dumpsys gfxinfo com.inkbound.rooftops.debug`), PSS memory (`adb shell dumpsys meminfo`), and thermal throttling.

## 3. Honest Known Limitations (v0.1.0 Vertical Slice)
1. **Uncompiled in offline authoring sandbox**: `rustc`, `cargo-ndk`, and Android NDK were not present in the authoring environment; run GitHub Actions CI or `./scripts/build_android_apk.sh` for the first compilation pass.
2. **Procedural geometry**: All vertical-slice rooftops, towers, sword, and Ink Wraith meshes are generated procedurally in Rust (100% original, MIT-licensed) rather than external glTF art assets.
3. **Audio hardware output**: Audio buses, dimensional-slash ducking, and SFX event queues are implemented in pure Rust; connecting the mixer buffer to Android's native `AAudio`/`Oboe` stream is gated behind an opt-in platform step to avoid NDK C++ STL packaging issues on first smoke test.
4. **Bitmap UI text**: The 2D overlay renderer renders crisp geometric bars, icons, buttons, and 5x7 pixel-vector HUD digits (`100/100`, `0/3`) with zero external font licensing dependencies; full TTF/SDF localized text rendering is planned for Phase 4 polish.
