use std::{env, fs, path::Path};

fn main() {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR is set for build scripts");
    fs::write(
        Path::new(&out_dir).join("generated.rs"),
        "/// Version written by the build script.\npub const GENERATED_VERSION: &str = \"1.0.0\";\n",
    )
    .expect("write generated.rs");
}
