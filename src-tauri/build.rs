fn main() {
    // Ensure LLVM is findable for bindgen (whisper-rs-sys)
    if std::env::var("LIBCLANG_PATH").is_err() {
        let llvm_path = "C:\\Program Files\\LLVM\\bin";
        if std::path::Path::new(llvm_path).join("libclang.dll").exists() {
            println!("cargo:rustc-env=LIBCLANG_PATH={llvm_path}");
            std::env::set_var("LIBCLANG_PATH", llvm_path);
        }
    }

    tauri_build::build()
}
