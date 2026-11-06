use std::path::PathBuf;

fn main() {
    let root = std::env::var("BUILD_WORKSPACE_DIRECTORY")
        .expect("run with `bazel run //cli/dx_config:update_schema`");
    let path = PathBuf::from(root).join("cli/dx_config/schema/dx.schema.json");
    std::fs::write(path, dx_config::schema::dx_schema()).expect("write schema");
}
