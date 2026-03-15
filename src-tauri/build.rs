fn main() {
    // Point linker to pre-built sherpa-onnx shared libraries
    let lib_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("sherpa-onnx-lib");
    println!("cargo:rustc-link-search=native={}", lib_dir.display());

    tauri_build::build()
}
