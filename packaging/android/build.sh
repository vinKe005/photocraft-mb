#!/usr/bin/env bash
# Build a debug-signed arm64 APK. SDK/NDK and Rust are installed separately.
set -euo pipefail
cd "$(dirname "$0")/../.."
: "${ANDROID_HOME:?Set ANDROID_HOME to an Android SDK with platform/build-tools 35}"
: "${ANDROID_NDK_HOME:?Set ANDROID_NDK_HOME to an Android NDK installation}"
command -v cargo >/dev/null
command -v cargo-apk >/dev/null
command -v java >/dev/null
# Avoid multi-GB debug symbols on phones and on build machines.
export ANDROID_USER_HOME="${ANDROID_USER_HOME:-$PWD/target/android-user}"
mkdir -p "$ANDROID_USER_HOME"
export CARGO_PROFILE_DEV_DEBUG=0
export CARGO_INCREMENTAL=0
cargo apk build -p photocraft-android --lib "$@"
