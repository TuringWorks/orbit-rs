fn main() {
    println!("cargo:rerun-if-env-changed=CUDA_PATH");
    println!("cargo:rerun-if-env-changed=CUDA_ROOT");
    println!("cargo:rerun-if-env-changed=CUDA_TOOLKIT_ROOT_DIR");

    // Declare custom configuration flags
    println!("cargo::rustc-check-cfg=cfg(has_cuda)");
    println!("cargo::rustc-check-cfg=cfg(has_metal)");

    // Check for CUDA
    if has_cuda() {
        println!("cargo:rustc-cfg=has_cuda");
    }

    // Check for Metal (macOS)
    if has_metal() {
        println!("cargo:rustc-cfg=has_metal");
    }
}

fn has_cuda() -> bool {
    // Check environment variables first
    if std::env::var("CUDA_PATH").is_ok()
        || std::env::var("CUDA_ROOT").is_ok()
        || std::env::var("CUDA_TOOLKIT_ROOT_DIR").is_ok()
    {
        return true;
    }

    // Check for nvcc in PATH
    if let Ok(path) = std::env::var("PATH") {
        for p in path.split(if cfg!(windows) { ';' } else { ':' }) {
            let p = std::path::Path::new(p);
            if p.join("nvcc").exists() || p.join("nvcc.exe").exists() {
                return true;
            }
        }
    }

    // Check common locations
    if cfg!(target_os = "linux")
        && std::path::Path::new("/usr/local/cuda/bin/nvcc").exists() {
            return true;
        }

    false
}

fn has_metal() -> bool {
    cfg!(target_os = "macos")
}
