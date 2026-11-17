fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-check-cfg=cfg(has_build_stamp)");
    println!("cargo:rustc-cfg=has_build_stamp");
}
