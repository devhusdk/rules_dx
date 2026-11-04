use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=cc_build.rs");
    println!("cargo:rerun-if-changed=native_cc.c");
    println!("cargo:rerun-if-changed=native_cc.cc");
    println!("cargo:rerun-if-changed=native_cc.h");

    let mut c = cc::Build::new();
    c.file("native_cc.c");
    c.compile("native_cc_c");

    let mut cxx = cc::Build::new();
    cxx.cpp(true).cpp_link_stdlib(None).file("native_cc.cc");
    cxx.compile("native_cc_cxx");

    let compiler = cc::Build::new().get_compiler();
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set"));
    fs::write(
        out_dir.join("cc_probe.rs"),
        format!(
            "pub const CC_PATH: &str = {:?};\n",
            compiler.path().display().to_string()
        ),
    )
    .expect("probe is staged");
}
