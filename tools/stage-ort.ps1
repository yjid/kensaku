# Stage the ONNX Runtime 1.31.0 library for a local OCR build.
#
# Release builds take it from the v1.0.18 bundles; local builds normally have
# no runtime at all, so Doctor warns and PP-OCR/Indra cannot load. Run
# `make ort` (or this script) once after cloning, then set ORT_DYLIB_PATH.
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$base = 'https://github.com/yjid/kensaku/releases/download/v1.0.18'
$arch = if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') { 'arm64' } else { 'x64' }
$asset = "kensaku-portable-1.0.18-$arch.zip"
$sha256 = if ($arch -eq 'arm64') {
  '1360a8b84f8d83e79d2be92374e59a0e0b300ae8a693c83aa9b7184066fde331'
} else {
  'b645c688056e5a8ac154d4d66e38bb4222811535699317a4798a3ab195f5496d'
}

$tmp = Join-Path $env:TEMP "kensaku-ort-$PID"
$zip = Join-Path $tmp $asset
$out = Join-Path $tmp 'unpacked'
New-Item -ItemType Directory -Force -Path $tmp | Out-Null
try {
  Write-Host "Downloading the v1.0.18 $arch bundle..."
  Invoke-WebRequest -Uri "$base/$asset" -OutFile $zip -UseBasicParsing
  $actual = (Get-FileHash -Algorithm SHA256 $zip).Hash.ToLower()
  if ($actual -ne $sha256) { throw "checksum mismatch: expected $sha256, got $actual" }
  Expand-Archive -Path $zip -DestinationPath $out -Force
  $dest = Join-Path $root 'src-tauri\onnxruntime.dll'
  Copy-Item (Join-Path $out 'onnxruntime.dll') $dest
  Write-Host "Staged $dest"
  Write-Host "Set this before launching the app:"
  Write-Host "  `$env:ORT_DYLIB_PATH = `"$dest`""
} finally {
  Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}
