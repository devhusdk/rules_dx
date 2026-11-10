fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-check-cfg=cfg(has_cc_optout_stamp)");
    println!("cargo:rustc-cfg=has_cc_optout_stamp");
}
