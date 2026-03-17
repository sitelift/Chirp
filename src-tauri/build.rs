fn main() {
    // Point linker to pre-built sherpa-onnx shared libraries
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));

    let lib_dir = if cfg!(target_os = "windows") {
        manifest_dir.join("sherpa-onnx-lib").join("windows")
    } else if cfg!(target_os = "macos") {
        manifest_dir.join("sherpa-onnx-lib").join("macos")
    } else {
        manifest_dir.join("sherpa-onnx-lib").join("linux")
    };

    // Fall back to flat directory if platform subdirectory doesn't exist
    let lib_dir = if lib_dir.exists() {
        lib_dir
    } else {
        manifest_dir.join("sherpa-onnx-lib")
    };

    println!("cargo:rustc-link-search=native={}", lib_dir.display());

    tauri_build::build()
}
