#!/usr/bin/env bash
# Builds the Rust library for arm64 and puts it, with the NDK's libc++, into the Gradle app.
#
# Usage: ANDROID_NDK_HOME=/path/to/ndk scripts/build-android.sh [release|debug]
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
ndk="${ANDROID_NDK_HOME:-${ANDROID_NDK_LATEST_HOME:-}}"
if [ -z "$ndk" ] || [ ! -d "$ndk" ]; then
    echo "Set ANDROID_NDK_HOME to an Android NDK directory." >&2
    exit 1
fi
export ANDROID_NDK_HOME="$ndk"

case "${1:-release}" in
    release) profile_flag=(--release); profile=release ;;
    debug) profile_flag=(); profile=debug ;;
    *) echo "Unknown profile '$1': use release or debug." >&2; exit 1 ;;
esac

# limn is a path dependency, so the repository must sit next to ../lienzo.
cd "$root"
cargo ndk -t arm64-v8a --platform 26 rustc -p surco --lib "${profile_flag[@]}" --crate-type cdylib

target_dir="${CARGO_TARGET_DIR:-$root/target}"
jni="$root/android/app/src/main/jniLibs/arm64-v8a"
mkdir -p "$jni"
cp "$target_dir/aarch64-linux-android/$profile/libsurco_app.so" "$jni/"
# The C++ dependencies may need the shared runtime, which Android does not ship.
cp "$ndk/toolchains/llvm/prebuilt/linux-x86_64/sysroot/usr/lib/aarch64-linux-android/libc++_shared.so" "$jni/"

echo "Copied $profile libsurco_app.so and libc++_shared.so into $jni"
