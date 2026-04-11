#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ANDROID_PROJECT_DIR="$ROOT_DIR/android-tv"
JNI_LIBS_DIR="$ANDROID_PROJECT_DIR/app/src/main/jniLibs"
ANDROID_SDK_DEFAULT="$HOME/Library/Android/sdk"
ABIS="${SOL_ANDROID_ABIS:-arm64-v8a,armeabi-v7a}"

cd "$ROOT_DIR"

if ! command -v cargo-ndk >/dev/null 2>&1; then
  echo "cargo-ndk is required. Install with: cargo install cargo-ndk"
  exit 1
fi

export ANDROID_SDK_ROOT="${ANDROID_SDK_ROOT:-${ANDROID_HOME:-$ANDROID_SDK_DEFAULT}}"

if [[ -z "${ANDROID_NDK_HOME:-}" ]]; then
  if [[ -d "$ANDROID_SDK_ROOT/ndk" ]]; then
    latest_ndk="$(ls -1 "$ANDROID_SDK_ROOT/ndk" 2>/dev/null | sort -V | tail -n 1)"
    if [[ -n "$latest_ndk" && -d "$ANDROID_SDK_ROOT/ndk/$latest_ndk" ]]; then
      export ANDROID_NDK_HOME="$ANDROID_SDK_ROOT/ndk/$latest_ndk"
    fi
  elif [[ -d "$ANDROID_SDK_ROOT/ndk-bundle" ]]; then
    export ANDROID_NDK_HOME="$ANDROID_SDK_ROOT/ndk-bundle"
  fi
fi

if [[ -z "${ANDROID_NDK_HOME:-}" || ! -d "$ANDROID_NDK_HOME" ]]; then
  cat <<MSG
Could not find Android NDK.
Install it in Android Studio:
  Settings > Languages & Frameworks > Android SDK > SDK Tools > NDK (Side by side)
Then re-run this script.
Expected default path:
  $ANDROID_SDK_ROOT/ndk/<version>
MSG
  exit 1
fi

IFS=',' read -r -a ABI_LIST <<< "$ABIS"

TARGET_ARGS=()
for abi in "${ABI_LIST[@]}"; do
  TARGET_ARGS+=("-t" "$abi")
done

cargo ndk "${TARGET_ARGS[@]}" -o "$JNI_LIBS_DIR" build --release --lib

echo "Native libraries copied to:"
for abi in "${ABI_LIST[@]}"; do
  echo "  $JNI_LIBS_DIR/$abi"
  ls -la "$JNI_LIBS_DIR/$abi" || true
done
