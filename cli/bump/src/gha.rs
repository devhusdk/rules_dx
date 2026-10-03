pub const WORKFLOW_DIR: &str = ".github/workflows";

pub fn workflow_file(name: &str) -> bool {
    name.ends_with(".yml") || name.ends_with(".yaml")
}
