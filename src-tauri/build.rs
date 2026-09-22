use std::path::PathBuf;

fn main() {
    tauri_build::build();

    // On Windows, ensure sherpa-onnx and onnxruntime DLLs are present in target/<profile>/deps/
    // so cargo test runs against the bundled ONNX runtime rather than any older C:\Windows\System32\onnxruntime.dll.
    if let Ok(out_dir) = std::env::var("OUT_DIR") {
        let out_path = PathBuf::from(out_dir);
        // out_path is target/<profile>/build/murmur-<hash>/out
        if let Some(target_profile_dir) = out_path.ancestors().nth(3) {
            let deps_dir = target_profile_dir.join("deps");
            let _ = std::fs::create_dir_all(&deps_dir);

            for dll in [
                "onnxruntime.dll",
                "onnxruntime_providers_shared.dll",
                "sherpa-onnx-c-api.dll",
                "sherpa-onnx-cxx-api.dll",
            ] {
                let src = target_profile_dir.join(dll);
                let dst = deps_dir.join(dll);
                if src.exists() {
                    let _ = std::fs::copy(&src, &dst);
                }
            }
        }
    }
}
