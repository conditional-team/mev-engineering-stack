//! Build script for linking C hot path library
//! Compiles C sources with maximum SIMD optimizations

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let fast_dir = PathBuf::from(&manifest_dir).parent().unwrap().join("fast");
    
    println!("cargo:rerun-if-changed=../fast/src/");
    println!("cargo:rerun-if-changed=../fast/include/");
    
    // Check if we're on Windows or Unix
    let is_windows = cfg!(target_os = "windows");
    
    if is_windows {
        // Windows: Use cc crate for compilation
        compile_with_cc(&fast_dir);
    } else {
        // Unix: Use Makefile
        compile_with_make(&fast_dir);
    }
    
    // Link the library
    println!("cargo:rustc-link-search=native={}/lib", fast_dir.display());
    println!("cargo:rustc-link-lib=static=mev_fast");
    
    // Link pthread on Unix
    if !is_windows {
        println!("cargo:rustc-link-lib=pthread");
    }
}

fn compile_with_make(fast_dir: &PathBuf) {
    let status = Command::new("make")
        .current_dir(fast_dir)
        .arg("clean")
        .status()
        .expect("Failed to run make clean");
    
    if !status.success() {
        eprintln!("Warning: make clean failed");
    }
    
    let status = Command::new("make")
        .current_dir(fast_dir)
        .status()
        .expect("Failed to run make");
    
    if !status.success() {
        panic!("Failed to build C library");
    }
}

fn compile_with_cc(fast_dir: &PathBuf) {
    let src_dir = fast_dir.join("src");
    let include_dir = fast_dir.join("include");
    let lib_dir = fast_dir.join("lib");
    
    // Create lib directory
    std::fs::create_dir_all(&lib_dir).ok();
    
    // Collect all C source files
    let sources: Vec<PathBuf> = std::fs::read_dir(&src_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|e| e == "c").unwrap_or(false))
        .collect();
    
    // Build with cc crate
    let mut build = cc::Build::new();
    
    build
        .opt_level(3)
        .include(&include_dir)
        .warnings(true)
        .extra_warnings(true);
    
    // Add SIMD flags for MSVC or GCC/Clang
    if cfg!(target_env = "msvc") {
        build.flag("/arch:AVX2");
        build.flag("/O2");
        build.flag("/GL");  // Link-time optimization
    } else {
        build.flag("-mavx2");
        build.flag("-msse4.2");
        build.flag("-mfma");
        build.flag("-mbmi2");
        build.flag("-ffast-math");
        build.flag("-funroll-loops");
        build.flag("-flto");
        build.flag("-march=native");
    }
    
    // Add all sources
    for source in sources {
        println!("cargo:warning=Compiling: {:?}", source);
        build.file(&source);
    }
    
    // Compile
    build.compile("mev_fast");
}
