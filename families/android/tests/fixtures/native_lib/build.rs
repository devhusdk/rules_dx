use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set"));
    let host = env::var("HOST").expect("HOST is set");
    let target = env::var("TARGET").expect("TARGET is set");
    fs::write(
        out_dir.join("build_probe.rs"),
        format!(
            "pub const BUILD_SCRIPT_HOST: &str = {host:?};\n\
            pub const BUILD_SCRIPT_TARGET: &str = {target:?};\n"
        ),
    )
    .expect("probe is staged");
    println!("cargo:rustc-check-cfg=cfg(has_native_lib_stamp)");
    println!("cargo:rustc-cfg=has_native_lib_stamp");
}
