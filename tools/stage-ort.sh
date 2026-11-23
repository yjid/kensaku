#!/usr/bin/env bash
# Stage the ONNX Runtime 1.31.0 library for a local OCR build.
#
# Release builds take it from the v1.0.18 bundles; local builds normally have
# no runtime at all, so Doctor warns and PP-OCR/Indra cannot load. Run
# `make ort` once after cloning, then start the app with ORT_DYLIB_PATH set.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
base="https://github.com/yjid/kensaku/releases/download/v1.0.18"
tmp="$(mktemp -d)"
cleanup() { rm -rf "$tmp"; }
trap cleanup EXIT

case "$(uname -s)" in
  Linux)
    deb="$tmp/kensaku_1.0.18_amd64.deb"
    echo "Downloading the v1.0.18 Linux bundle..."
    curl -fsSL --retry 3 -o "$deb" "$base/kensaku_1.0.18_amd64.deb"
    echo "2ea9437340abde54e690208b3152f91be0f98a3fc9561352a61c6e42ed03b09b  $deb" | sha256sum -c -
    dpkg-deb -x "$deb" "$tmp/deb"
    cp "$tmp/deb/usr/lib/kensaku/libonnxruntime.so" "$root/src-tauri/libonnxruntime.so"
    echo "Staged src-tauri/libonnxruntime.so"
    echo "export ORT_DYLIB_PATH=\"$root/src-tauri/libonnxruntime.so\""
    ;;
  Darwin)
    dmg="$tmp/kensaku_1.0.18_universal.dmg"
    echo "Downloading the v1.0.18 macOS bundle..."
    curl -fsSL --retry 3 -o "$dmg" "$base/kensaku_1.0.18_universal.dmg"
    echo "1985891ec5884ea2e3448e506bd6f240b083c67c7a957f26cba6eb8f60f72c28  $dmg" | shasum -a 256 -c -
    mount="$tmp/mnt"
    mkdir -p "$mount"
    hdiutil attach -nobrowse -readonly -mountpoint "$mount" "$dmg" >/dev/null
    trap 'hdiutil detach "$mount" >/dev/null 2>&1 || true; cleanup' EXIT
    cp "$mount/kensaku.app/Contents/Resources/libonnxruntime.dylib" "$root/src-tauri/libonnxruntime.dylib"
    echo "Staged src-tauri/libonnxruntime.dylib"
    echo "export ORT_DYLIB_PATH=\"$root/src-tauri/libonnxruntime.dylib\""
    ;;
  *)
    echo "Unsupported OS. On Windows run: powershell -File tools/stage-ort.ps1" >&2
    exit 1
    ;;
esac

echo "Set ORT_DYLIB_PATH before launching the app, e.g.:"
echo "  ORT_DYLIB_PATH=... cargo run --manifest-path src-tauri/Cargo.toml"
