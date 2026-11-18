# Android development (experimental)

Kensaku does not have a supported Android app yet. This guide covers the
development environment and the planned first milestone: launching a minimal
Tauri shell on an Android device. It does not mean desktop search or indexing
already works on Android.

## What is needed

- A Mac, Linux, or Windows development computer with Rust stable and Node.js
  20.19+ (or Bun 1.1+).
- [Android Studio](https://developer.android.com/studio), including its bundled
  Java runtime.
- In Android Studio's SDK Manager, install an Android SDK Platform, SDK Platform
  Tools, SDK Build-Tools, Command-line Tools, and an NDK (Side by side).
- An Android phone with USB debugging enabled, or an Android emulator.

Tauri's complete prerequisite list is in its
[Android setup guide](https://v2.tauri.app/start/prerequisites/#android).

## Configure the toolchain

On macOS, set the paths for the current terminal session (adjust the Android
Studio path if it is installed elsewhere):

```sh
export JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home"
export ANDROID_HOME="$HOME/Library/Android/sdk"
export NDK_HOME="$ANDROID_HOME/ndk/<installed-version>"
```

On Linux, use the Android Studio `jbr` directory for `JAVA_HOME` and your SDK
installation directory for `ANDROID_HOME`. On Windows, set the equivalent
environment variables in the user environment settings. `NDK_HOME` must point
to one installed NDK version directory.

Install the Rust Android targets once:

```sh
rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
```

Check that the tools are visible before building:

```sh
java -version
adb version
rustup target list --installed
```

## Initialize and run the shell

From the repository root:

```sh
npm install
npm run tauri -- android init
```

Connect an Android phone with USB debugging enabled, or start an emulator from
Android Studio's Device Manager. Confirm that `adb devices` lists the target,
then start the app:

```sh
adb devices
npm run tauri -- android dev
```

For a local APK build:

```sh
npm run tauri -- android build --apk --target aarch64
```

Tauri creates the Android Studio project under `src-tauri/gen/android/`; the
repository ignores this generated directory. See Tauri's
[mobile development guide](https://v2.tauri.app/develop/#developing-your-mobile-application)
for device selection and Android WebView inspection.

## Scope and known gaps

The first milestone is only a shell that launches on a real Android device.
Do not treat it as a release or as evidence that search works. The desktop app
currently assumes launcher/tray behavior, a global shortcut, and unrestricted
folder walking. A usable Android search app still needs Storage Access Framework
folder grants, Android lifecycle-aware indexing, mobile search UI, and Android
builds of its native extraction/runtime dependencies. Those pieces need separate
design and device testing before the app can be described as supported.

The maintainer's guidance is in [issue #131](https://github.com/yjid/kensaku/issues/131).
