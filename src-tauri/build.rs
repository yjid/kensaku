fn main() {
    // The manifest is embedded below instead of by `tauri-build`, which only
    // links its resource into binaries; the test binaries need it too.
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest()),
    )
    .expect("failed to run tauri-build");

    embed_app_manifest();

    // Copy onnxruntime library from src-tauri to target directories
    // so tests and debug builds can find and load it without manual setup.
    let manifest_dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default());
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default());

    #[cfg(windows)]
    let dll_name = "onnxruntime.dll";
    #[cfg(target_os = "linux")]
    let dll_name = "libonnxruntime.so";
    #[cfg(target_os = "macos")]
    let dll_name = "libonnxruntime.dylib";

    let src_dll = manifest_dir.join(dll_name);
    if src_dll.is_file()
        && let Some(target_dir) = out_dir.ancestors().nth(3)
    {
        let _ = std::fs::copy(&src_dll, target_dir.join(dll_name));
        let _ = std::fs::copy(&src_dll, target_dir.join("deps").join(dll_name));
    }
}

/// Embeds the application manifest into every artifact of the crate.
///
/// `tauri-plugin-dialog` enables `rfd/common-controls-v6`, so the test binaries
/// import `TaskDialogIndirect`. That export only exists in Common Controls v6,
/// which only loads when the executable's manifest asks for it; without the
/// manifest Windows resolves `System32\comctl32.dll` (v5) and the test binary
/// dies with `STATUS_ENTRYPOINT_NOT_FOUND` before `main`. `tauri-build` links
/// its resource into binaries only, so the manifest is added here for every
/// target instead, binaries included.
fn embed_app_manifest() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    println!("cargo:rerun-if-changed=app.manifest.rc");
    println!("cargo:rerun-if-changed=windows-app-manifest.xml");
    embed_resource::compile_for_everything("app.manifest.rc", embed_resource::NONE)
        .manifest_required()
        .expect("embed the application manifest");
}
