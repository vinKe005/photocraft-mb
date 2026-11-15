# PhotoCraft Mobile (Android fork)

This fork adds an experimental native Android host at `apps/photocraft-android`.
It uses the existing Rust editor and engine through eframe/winit NativeActivity;
there is no WebView, JavaScript, or duplicate image-processing implementation.
Device acceptance testing is required before treating it as a stable mobile release.

## Build a test APK

Requirements:

- Rust 1.95+, with `rustup target add aarch64-linux-android`.
- Android SDK platform 35 and build-tools 35.0.0.
- Android NDK r27c (27.2.12479018), or a compatible newer NDK.
- Java 17+ and `cargo install cargo-apk --locked --version 0.10.0`.

Set `ANDROID_HOME` to the SDK directory and `ANDROID_NDK_HOME` to the NDK directory:

```sh
packaging/android/build.sh
```

The debug-signed APK is written under `target/debug/apk/`. Debug signing material
stays under the ignored `target/android-user/` directory unless `ANDROID_USER_HOME`
is explicitly set. Do not use a debug key for a production release.

The initial ABI is **arm64-v8a**, with **Android 10 / API 29** or later. The
application ID is `io.github.vinke005.photocraft`, separate from the upstream desktop
identifier. The native library is linked for 16 KiB page compatibility.

## Mobile interaction

- The Android host uses scrollable top actions and a bottom tool strip, with
  48-point action targets. More tools are available through the Tools menu.
  The NativeActivity build does not yet support AccessKit/TalkBack; existing
  desktop and web accessibility features stay enabled.
- Layers open in a bottom panel capped at 45% of the available height. Opening a
  phone panel does not change the saved desktop dock preferences.
- Open shows a native Rust photo list over the images Android currently permits
  the app to read. The app requests media access through Android's permission UI;
  Android 14+ supports access to selected photos. Refresh access from the list.
- The list shows up to 256 recent local MediaStore images by name. It is not the
  Android system photo picker and does not browse arbitrary document providers.
- Open With / Share content-URI intents can supply the photo at application start.
  Only startup intents are read; while the app is running, open another photo
  through its in-app photo list.
- Export creates a new file in Android's Downloads collection using MediaStore.
  It does not overwrite the original provider resource. A failed stream write
  removes its pending export; incomplete exports remain unpublished.
- Preferences and incremental recovery use app-private storage. Recovery follows
  the shared editor's configured autosave interval; Android process termination
  and background lifecycle recovery still need device testing.
- NativeActivity's content rectangle supplies system-bar and keyboard insets.

Photo reads are limited to 128 MiB of encoded data, with interrupted-read retry,
provider errors distinguished from EOF, and bounded allocation. Existing decoder
and document memory checks still apply after import.

## Safety and dependencies

The fork owner authorized only the exported `android_main` symbol attribute as an
unsafe-code exception. The Android package uses `unsafe_code = "deny"`, allowing
that attribute on that function only. The workspace and editor/engine keep their
existing prohibition. No unsafe blocks are introduced.

Android services use `robius-android-env` 0.2.0 (MIT) to obtain safe JNI access to
`android-activity`'s initialized context, and `jni` 0.21.1 (MIT OR Apache-2.0).
There is no additional app-authored FFI exception.

## Validation

Host tests for content-stream bounds, interrupted reads and permission failures:

```sh
cargo test -p photocraft-android --lib
```

Shared UI tests, architecture and platform gates:

```sh
cargo test -p photocraft-ui-egui --test android_layout
cargo clippy -p photocraft-android --target aarch64-linux-android --all-targets -- -D warnings
cargo xtask layers
cargo xtask panel-commands
cargo xtask wasm
```

The Android clippy invocation also needs the NDK compiler environment (the APK
packager supplies it during builds). An offscreen preview can exercise the phone
chrome without an Android device:

```sh
cargo run -p photocraft-ui-egui --example snapshot -- \
  --mobile --size 412x915 --out mobile.png --open photo.png
```

Before a stable release, install on physical arm64 devices and verify gallery
permissions (including denial and selected-photo grants), open/edit/undo/export,
rotation, keyboard insets, pinch/pan without accidental painting, GPU restoration,
backgrounding and recovery after process termination. The existing desktop dialogs
and tool-options controls also need further touch-specific refinement.
