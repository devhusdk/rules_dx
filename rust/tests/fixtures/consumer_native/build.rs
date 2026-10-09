use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let src = PathBuf::from(env::var("DX_NATIVE_BINDINGS_SRC").expect("bindings path is set"));
    let bindings = fs::read_to_string(&src).expect("generated bindings are readable");
    for symbol in ["consumer_add", "consumer_offset"] {
        if !bindings.contains(symbol) {
            panic!("generated bindings are missing {symbol}");
        }
    }
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set"));
    fs::write(out_dir.join("bindings.rs"), bindings).expect("bindings are staged");
    let host = env::var("HOST").expect("HOST is set");
    let target = env::var("TARGET").expect("TARGET is set");
    fs::write(
        out_dir.join("build_probe.rs"),
        format!("pub const BUILD_SCRIPT_HOST: &str = {host:?};\n\
            pub const BUILD_SCRIPT_TARGET: &str = {target:?};\n"),
    )
    .expect("probe is staged");
    println!("cargo:rustc-check-cfg=cfg(has_consumer_native_stamp)");
    println!("cargo:rustc-cfg=has_consumer_native_stamp");
}
