param(
    [ValidateSet("debug", "release")]
    [string]$BuildType = "debug"
)

$ErrorActionPreference = "Stop"
$RootDir = Split-Path -Parent $PSScriptRoot

Write-Host "==> [1/3] Running Rust workspace unit tests..."
cargo test --manifest-path "$RootDir\Cargo.toml" --workspace --exclude app_android --exclude render_wgpu

Write-Host "==> [2/3] Compiling Rust native library for arm64-v8a..."
New-Item -ItemType Directory -Force -Path "$RootDir\android\app\src\main\jniLibs" | Out-Null

if ($BuildType -eq "release") {
    cargo ndk -t arm64-v8a -p 29 -o "$RootDir\android\app\src\main\jniLibs" build --manifest-path "$RootDir\Cargo.toml" -p app_android --release
} else {
    cargo ndk -t arm64-v8a -p 29 -o "$RootDir\android\app\src\main\jniLibs" build --manifest-path "$RootDir\Cargo.toml" -p app_android
}

Write-Host "==> [3/3] Packaging Android APK via Gradle..."
Push-Location "$RootDir\android"
try {
    $GradleCmd = if (Test-Path ".\gradlew.bat") { ".\gradlew.bat" } else { "gradle" }
    if ($BuildType -eq "release") {
        & $GradleCmd assembleRelease
        Write-Host "APK generated at: android\app\build\outputs\apk\release\"
    } else {
        & $GradleCmd assembleDebug
        Write-Host "APK generated at: android\app\build\outputs\apk\debug\app-debug.apk"
    }
} finally {
    Pop-Location
}
