#!/usr/bin/env bash
set -euo pipefail

# Build Inkbound Rooftops arm64-v8a Android APK
# Prerequisites:
#   - Rust 1.83.0 with target aarch64-linux-android (rustup target add aarch64-linux-android)
#   - cargo-ndk (cargo install cargo-ndk)
#   - Android SDK + NDK r27+ (ANDROID_NDK_HOME set)
#   - JDK 17 + Gradle 8.10+ (or ./gradlew)

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD_TYPE="${1:-debug}" # debug or release

echo "==> [1/3] Running Rust workspace unit tests..."
cargo test --manifest-path "${ROOT_DIR}/Cargo.toml" --workspace --exclude app_android --exclude render_wgpu

echo "==> [2/3] Compiling Rust native library for arm64-v8a..."
mkdir -p "${ROOT_DIR}/android/app/src/main/jniLibs"
if [[ "${BUILD_TYPE}" == "release" ]]; then
  cargo ndk -t arm64-v8a -p 29 -o "${ROOT_DIR}/android/app/src/main/jniLibs" \
    build --manifest-path "${ROOT_DIR}/Cargo.toml" -p app_android --release
else
  cargo ndk -t arm64-v8a -p 29 -o "${ROOT_DIR}/android/app/src/main/jniLibs" \
    build --manifest-path "${ROOT_DIR}/Cargo.toml" -p app_android
fi

echo "==> [3/3] Packaging Android APK via Gradle..."
cd "${ROOT_DIR}/android"
GRADLE_CMD="gradle"
if [[ -x "./gradlew" ]]; then
  GRADLE_CMD="./gradlew"
fi

if [[ "${BUILD_TYPE}" == "release" ]]; then
  ${GRADLE_CMD} assembleRelease
  echo "APK generated at: android/app/build/outputs/apk/release/"
else
  ${GRADLE_CMD} assembleDebug
  echo "APK generated at: android/app/build/outputs/apk/debug/app-debug.apk"
fi
