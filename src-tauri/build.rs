fn main() {
    // Point the linker at the pre-built sherpa-onnx libraries
    // (src-tauri/sherpa-onnx-lib/windows, or the flat directory as a fallback).
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib_dir = manifest_dir.join("sherpa-onnx-lib").join("windows");
    let lib_dir = if lib_dir.exists() {
        lib_dir
    } else {
        manifest_dir.join("sherpa-onnx-lib")
    };
    println!("cargo:rustc-link-search=native={}", lib_dir.display());

    // Select the verified CPU/CUDA runtime before the first native call.
    println!("cargo:rustc-link-lib=delayimp");
    println!("cargo:rustc-link-arg=/DELAYLOAD:sherpa-onnx-c-api.dll");
    tauri_build::build()
}
