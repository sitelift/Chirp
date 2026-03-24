fn main() {
    // Load .env file so secrets are available via env!() / option_env!() at compile time.
    // The .env file sits in the repo root (one level up from src-tauri/).
    let env_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.env");
    if env_path.exists() {
        for line in std::fs::read_to_string(&env_path).unwrap_or_default().lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();
                if !value.is_empty() {
                    println!("cargo:rustc-env={key}={value}");
                }
            }
        }
        println!("cargo:rerun-if-changed={}", env_path.display());
    }

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

    // Embed rpath so the binary can find dylibs inside the .app bundle at runtime
    if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-arg=-Wl,-rpath,@executable_path/../Resources/sherpa-onnx-lib/macos");
    }

    tauri_build::build()
}
