fn main() {
    if std::env::var("CARGO_FEATURE_EMBED").is_ok() {
        let nsis_dir = std::path::PathBuf::from("../src-tauri/target/release/bundle/nsis");
        let installer = std::fs::read_dir(&nsis_dir)
            .expect("NSIS bundle directory not found")
            .filter_map(|e| e.ok())
            .find(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                name.starts_with("Chirp_") && name.ends_with("_x64-setup.exe")
            })
            .expect("No Chirp NSIS installer found in bundle dir");

        let out_dir = std::env::var("OUT_DIR").unwrap();
        let dest = std::path::PathBuf::from(&out_dir).join("installer.exe");
        std::fs::copy(installer.path(), &dest).expect("Failed to copy installer");
    }
}
