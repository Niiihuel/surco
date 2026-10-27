#!/usr/bin/env bash
# Builds the Rust library for each ABI in SURCO_ANDROID_ABIS (default arm64-v8a) and puts it,
# with the NDK's libc++, into the Gradle app. ABIs not listed are removed, so none goes stale.
#
# Usage: ANDROID_NDK_HOME=/path/to/ndk [SURCO_ANDROID_ABIS="arm64-v8a x86_64"] scripts/build-android.sh [release|debug]
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

abis="${SURCO_ANDROID_ABIS:-arm64-v8a}"
for abi in $abis; do
    case "$abi" in
        arm64-v8a|x86_64) ;;
        *) echo "Unknown ABI '$abi': use arm64-v8a or x86_64." >&2; exit 1 ;;
    esac
done

# limn is a path dependency, so the repository must sit next to ../lienzo.
cd "$root"
jni_root="$root/android/app/src/main/jniLibs"
for dir in "$jni_root"/*/; do
    [ -d "$dir" ] || continue
    case " $abis " in
        *" $(basename "$dir") "*) ;;
        *) rm -rf "$dir" ;;
    esac
done

target_dir="${CARGO_TARGET_DIR:-$root/target}"
for abi in $abis; do
    case "$abi" in
        arm64-v8a) triple=aarch64-linux-android ;;
        x86_64) triple=x86_64-linux-android ;;
    esac
    cargo ndk -t "$abi" --platform 26 rustc -p surco --lib "${profile_flag[@]}" --crate-type cdylib

    jni="$jni_root/$abi"
    mkdir -p "$jni"
    cp "$target_dir/$triple/$profile/libsurco_app.so" "$jni/"
    # The C++ dependencies may need the shared runtime, which Android does not ship.
    cp "$ndk/toolchains/llvm/prebuilt/linux-x86_64/sysroot/usr/lib/$triple/libc++_shared.so" "$jni/"

    echo "Copied $profile libsurco_app.so and libc++_shared.so for $abi into $jni"
done
