use std::env;
use std::path::PathBuf;

fn main() {
    // Tell cargo to invalidate the built crate whenever the wrapper changes
    println!("cargo:rerun-if-changed=wrapper.h");
    
    // Get the Python library name
    let python_lib = match env::var("PYTHON_LIB") {
        Ok(lib) => lib,
        Err(_) => {
            let output = std::process::Command::new("python3")
                .args(["-c", "import sysconfig; print(sysconfig.get_config_var('LDLIBRARY'))"])
                .output()
                .expect("Failed to execute python3");
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
    };
    
    // If library name is found, explicitly link to it
    if !python_lib.is_empty() {
        let lib_name = python_lib.strip_prefix("lib").unwrap_or(&python_lib);
        let lib_name = lib_name.strip_suffix(".so").unwrap_or(lib_name);
        let lib_name = lib_name.strip_suffix(".dylib").unwrap_or(lib_name);
        println!("cargo:rustc-link-lib={}", lib_name);
    }
    
    // Try to add additional possible Python library paths
    if let Ok(python_path) = std::process::Command::new("python3")
        .args(["-c", "import sysconfig; print(sysconfig.get_config_var('LIBDIR'))"])
        .output()
    {
        let python_lib_dir = String::from_utf8_lossy(&python_path.stdout).trim().to_string();
        if !python_lib_dir.is_empty() {
            println!("cargo:rustc-link-search={}", python_lib_dir);
        }
    }
    
    // Add framework path for macOS
    if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-search=framework=/System/Library/Frameworks");
        // Explicitly link to Python library
        println!("cargo:rustc-link-lib=framework=Python");
    }
    
    // Add debugging output
    println!("cargo:warning=Python lib: {}", python_lib);
} 