//!
//! My build.rs
use std::path::PathBuf;
use std::{env, fs};

///
/// My main function
///
pub fn main() {
    // Put the memory definitions somewhere the linker can find it
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    println!("cargo:rustc-link-search={}", out_dir.display());

    #[allow(unused_results, reason = "I like it")]
    let _ = fs::copy("device.x", out_dir.join("device.x")).unwrap_or_else(|err| {
        panic!(
            "Failed to copy 'device.x' to '{}': {}",
            out_dir.display(),
            err
        );
    });
    println!("cargo:rerun-if-changed=device.x");
}
