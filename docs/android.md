# Android

The Android app is the Rust app in a GameActivity. Kotlin owns the activity, the manifest,
the permissions and the media service; Rust owns the interface and the player. The shell is in
`android/`, and the Rust entry point is `android_main` in `crates/app/src/lib.rs`.

## The bridge

`SurcoBridge` (`android/app/src/main/java/io/github/niiihuel/surco/SurcoBridge.kt`) is the only
JNI door between the two sides. Its package is fixed, because Rust finds the class by name.

- Rust calls Kotlin with `android::call(kind, json)`, which runs `SurcoBridge.call(kind, json)`.
  Kotlin answers with a JSON string, or null.
- Kotlin calls Rust with `SurcoBridge.nativeEvent(kind, json)`, which Rust receives in
  `Java_io_github_niiihuel_surco_SurcoBridge_nativeEvent`.

`MainActivity` sends two kinds of event:

- `window.insets`: `{"top","bottom","left","right","ime"}` in dp. The first four are the system
  bars and display cutout; `ime` is the keyboard's height. Rust lays out around them, so the
  activity does not resize for the keyboard (`adjustNothing`).
- `window.back`: the system back gesture or button. Rust decides what back does, and asks the
  shell to leave when it has nowhere to go.

The activity is not recreated on configuration changes, because the Rust loop lives in it. The
manifest lists those changes under `android:configChanges`.

## Building

You need:

- JDK 17, with `JAVA_HOME` set or `java` on the path.
- The Android SDK, with `ANDROID_HOME` set, for compileSdk 35.
- Gradle 8.11.1. There is no wrapper in the repository; `gradle wrapper --gradle-version 8.11.1`
  in `android/` makes one if you want it.
- An Android NDK (r27 or later), in `ANDROID_NDK_HOME`.
- `cargo install cargo-ndk --locked`, and `rustup target add aarch64-linux-android`.
- limn checked out next to this repository as `../lienzo`.

Then, from the repository root:

```sh
ANDROID_NDK_HOME=/path/to/ndk scripts/build-android.sh        # or ... debug
gradle -p android assembleDebug
```

The script builds `libsurco_app.so` for arm64-v8a and copies it with the NDK's `libc++_shared.so`
into `android/app/src/main/jniLibs/`, which Git ignores. The APK is in
`android/app/build/outputs/apk/debug/`. The script assumes a Linux host for the NDK's
prebuilt toolchain; on macOS, change `linux-x86_64` in it to `darwin-x86_64`.

To check the Rust alone, without the SDK or Gradle, the NDK is enough:

```sh
cargo ndk -t arm64-v8a --platform 26 clippy -p surco --lib
```

On NixOS the NDK from Google's site does not run as it is; nixpkgs builds one patched for it:

```sh
nix-build -E 'with import <nixpkgs> { config = { android_sdk.accept_license = true; allowUnfree = true; }; }; androidenv.androidPkgs.ndk-bundle' -o ~/Android/ndk
export ANDROID_NDK_HOME=$(readlink -f ~/Android/ndk/libexec/android-sdk/ndk-bundle)
```

CI runs the same two commands on `ubuntu-24.04` (`.github/workflows/android.yml`) and uploads the
APK as `android-debug-apk`.

## Forks

The application id defaults to `io.github.niiihuel.surco`. Override it without editing the
build:

```sh
gradle -p android assembleDebug -Psurco.applicationId=com.example.surco
```

The Kotlin package and the Gradle namespace stay `io.github.niiihuel.surco`, so the Rust side
still finds `SurcoBridge`.

## Versions

`versionName` is the workspace version in `Cargo.toml`, and `versionCode` is
`major * 10000 + minor * 100 + patch`. The `rustls-platform-verifier` Android component must match
the crate version in `Cargo.lock`, so Gradle reads that version from the lock file.
