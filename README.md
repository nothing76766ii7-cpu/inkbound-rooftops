# Inkbound Rooftops — V9 Integrated Visual & Destruction Upgrade

**New in V9:** Imported OBJ/GLB source meshes (cuttable roofs, a tower, and a sword), 8-bone first-person skeletal animation with 11 keyframed clips, PNG material atlas sampled by WGSL, dynamic directional shadows, support-aware collapse, and heavier world-cut shake. See `docs/V9_REAL_SOURCE_UPGRADE.md` for implementation details and testing requirements.


An original, native-Rust 3D first-person open-world parkour action game for **Android (`arm64-v8a`)**, featuring a hand-drawn white-and-orange sketchbook fantasy city, fluid rooftop traversal, sword & dimensional slash combat, and paint-based traversal/puzzles.

---

## Architecture Overview

- **Authoritative Language**: Rust (`edition = "2021"`, pinned `1.83.0` via `rust-toolchain.toml`)
- **Renderer**: `wgpu 22.1` (Vulkan primary on Android 10+) with 1024px directional shadow-map pass, PCF shadow receiver shader, decoded sRGB atlas for four original material PNGs, dynamic sketch outlines, fog, imported OBJ sword skinned to authored JSON skeletal clips, and dimensional slash effects.
- **Window / Lifecycle**: `winit 0.30` + `android-activity 0.6` (`native-activity` backend) wrapped in a minimal Android Kotlin `MainActivity` for immersive fullscreen, landscape lock, and display cutout handling.
- **Physics & Simulation**: Deterministic 60 Hz fixed-timestep simulation (`engine_core`) with swept-capsule collision; most `SLICEABLE_WORLD` buildings, roofs and props can be severed. Cut pieces fall via support-graph and approximate rigid-body simulation; essential spawn platforms are protected.
- **Persistence**: Versioned JSON save slots (`persistence`) with atomic temp-file-and-rename writes and corruption recovery.

### Workspace Layout

```text
inkbound-rooftops/
├── Cargo.toml                     # Workspace manifest (pinned versions)
├── rust-toolchain.toml            # Pinned Rust 1.83.0 + aarch64-linux-android
├── .github/workflows/             # GitHub Actions CI (tests + arm64-v8a APK build)
├── scripts/                       # Local one-command APK build scripts (sh / ps1)
├── docs/                          # Architecture, UI/UX Spec, QA & Device Checklist
├── android/                       # Gradle Android project (minSdk 29, arm64-v8a)
└── crates/
    ├── engine_core/               # Fixed timestep, swept-capsule physics, layers, settings
    ├── platform_input/            # Multitouch joystick, drag-look, long-press, gyro, gamepad
    ├── gameplay_player/           # Parkour controller (sprint, coyote jump, wall-run, slide, dash, mantle)
    ├── gameplay_combat/           # Sword light/heavy/charged dimensional slash + target filtering
    ├── gameplay_paint/            # Brush, Platform, Mark, Reveal, Erase + pigment & cell budgets
    ├── world_streaming/           # Cell streaming + Vertical Slice Rooftop District
    ├── ai/                        # Ink Wraith state machine + paint vulnerability
    ├── ui/                        # Sketchbook HUD, menus, touch layout, localization
    ├── audio/                     # Audio buses, dimensional-slash ducking, haptics queue
    ├── persistence/               # Versioned atomic saves + migration
    ├── render_wgpu/               # wgpu 22.1 renderer + WGSL shaders
    ├── tools_asset_pipeline/      # Procedural mesh & palette validator CLI
    └── app_android/               # Native Android cdylib (`android_main`) + desktop runner
```

---

## Quick Start: Push to Git & Compile APK

### Option A: Automated Cloud Build via GitHub Actions (Recommended)

1. Unzip this repository and push it to a GitHub repository:
   ```bash
   cd inkbound-rooftops
   git init -b main
   git add .
   git commit -m "Initial commit: Inkbound Rooftops Rust Android vertical slice"
   git remote add origin https://github.com/<YOUR_USERNAME>/inkbound-rooftops.git
   git push -u origin main
   ```
2. Open the **Actions** tab on GitHub. The `Build Android APK (arm64-v8a)` workflow automatically:
   - Runs `cargo test` across all core crates
   - Installs Android NDK r27 & `cargo-ndk`
   - Compiles `libinkbound_rooftops.so` for `aarch64-linux-android`
   - Packages `app-debug.apk` with Gradle and uploads it as a downloadable workflow artifact.

### Option B: Local Build (Linux / macOS / Windows)

#### Prerequisites
- **Rust**: Install via [rustup.rs](https://rustup.rs/). `rust-toolchain.toml` selects the stable channel and `aarch64-linux-android`.
- **cargo-ndk**: `cargo install cargo-ndk --locked`
- **Android SDK + NDK r27+**: Install via Android Studio SDK Manager (`platforms;android-35`, `build-tools;35.0.0`, `ndk;27.1.12297006`), and set `ANDROID_HOME` and `ANDROID_NDK_HOME`.
- **JDK 17** and **Gradle 8.10+** (or run `gradle wrapper` once in `android/` to generate `gradlew`).

#### Build Commands
```bash
# Linux / macOS
./scripts/build_android_apk.sh debug

# Windows PowerShell
.\scripts\build_android_apk.ps1 -BuildType debug
```

Install on a connected `arm64-v8a` Android 10+ phone:
```bash
adb install -r android/app/build/outputs/apk/debug/app-debug.apk
```

#### Desktop Dev Testing (Keyboard + Mouse)
```bash
cargo run -p app_android
```

---

## Platform Glue Exceptions (Per PRD)

Rust is the authoritative language for all gameplay, physics, AI, UI logic, audio routing, persistence, and rendering. Non-Rust platform glue is strictly limited to:
- `android/app/src/main/AndroidManifest.xml` (Android package metadata, Vulkan requirement, landscape orientation)
- `android/app/build.gradle.kts`, `android/build.gradle.kts`, `android/settings.gradle.kts` (Gradle APK packaging)
- `android/app/src/main/java/com/inkbound/rooftops/MainActivity.kt` (Thin `NativeActivity` subclass for immersive fullscreen and display cutout handling)

---

## Honest Build & Verification Status

This workspace was authored in an offline environment without `rustc`, `cargo`, Android NDK, or physical Android hardware attached:
- **Not yet executed in this environment**: `cargo test`, `cargo-ndk`, Gradle APK assembly, and real-device frame-time/memory profiling.
- **Included for immediate verification**: `scripts/verify_v9_assets.py`, Rust unit tests across the workspace, `docs/V9_REAL_SOURCE_UPGRADE.md` and device QA checklist. The archive does not include a ready-made GitHub Actions workflow.
- **Audio backend note**: The `audio` crate implements pure-Rust bus mixing, dynamic dimensional-slash ducking, and procedural SFX event synthesis without requiring C++ NDK STL (`libc++_shared.so`) out of the box; hardware audio output hookup via `oboe` is documented as an opt-in platform step once the base APK smoke test passes on device.


## V10 layout, web and sound update

See [`docs/V10_LAYOUT_WEB_AUDIO_IMPLEMENTATION.md`](docs/V10_LAYOUT_WEB_AUDIO_IMPLEMENTATION.md) for code changes,
Android AAudio routing, hook controls, new sound assets and device test steps.
In-game: **HOOK** to attach/release, steer with joystick, press joystick far to reel,
**JUMP** to release with momentum. Use `G`/`Shift`/`Space` on desktop.
Run `python3 scripts/verify_v10.py` to check the 20 original WAV assets and code connections.

## V11 campaign and checkpoints

V11 adds an authored Level 1 and seeded Levels 2–5 with navigable rooftop chains, hookable towers, checkpoints, and an exit beacon. The game saves your level number, world seed, active checkpoint and cut history. Select **RESTART CHECKPOINT** to restore the saved point and its world condition, **RESTART LEVEL** to begin the current level with intact geometry, or **GENERATE NEW RUN** to start a fresh campaign seed.

The mobile **JUMP** region is enlarged with buffered/coyote-timed jumping, and **SLASH** starts on touch-down. Heavy/dimensional attacks now center their cut plane on aimed sliceable structures. The web-string constraint is applied after player movement to make swinging less elastic.

See `docs/V11_LEVELS_CHECKPOINTS_CUTTING_GRAPPLE.md` for controls and verification instructions. This is Rust *source* for you to build; no APK is included.

## V12: The Broken Sky campaign

The source now defines 24 levels in six regions, four seasonal climates, a ten-minute day/night cycle with region-specific fog/weather, illustrated three-page story interludes, longer challenging routes, and a strict separation between enemy sword attacks and the dedicated world-cut control. See `docs/V12_CAMPAIGN_AND_STORYBOARD.md` and `docs/V12_CHANGELOG_AND_QA.md`.
