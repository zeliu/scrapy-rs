use std::env;

fn main() {
    // Output debug information
    println!("cargo:warning=Building Scrapy-RS Python extension");
    
    // Set macOS deployment target (if building on macOS)
    if cfg!(target_os = "macos") {
        println!("cargo:rustc-env=MACOSX_DEPLOYMENT_TARGET=10.9");
    }
    
    // Output Python-related environment variables
    if let Ok(ldlibrary) = env::var("LDLIBRARY") {
        println!("cargo:warning=Python lib: {}", ldlibrary);
    }
    
    if let Ok(py_ver) = env::var("PY_VERSION") {
        println!("cargo:warning=Python version: {}", py_ver);
    }
    
    if let Ok(py_prefix) = env::var("PREFIX") {
        println!("cargo:warning=Python prefix: {}", py_prefix);
    }
} 