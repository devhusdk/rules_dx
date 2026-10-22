fn main() {
    let target = std::env::var("TARGET").expect("cargo sets TARGET for build scripts");
    let family = if cfg!(target_family = "wasm") {
        "wasm"
    } else {
        "native"
    };
    println!("cargo:rustc-env=DX_WASI_BUILD_TARGET={target}");
    println!("cargo:rustc-env=DX_WASI_BUILD_SCRIPT_FAMILY={family}");
}
