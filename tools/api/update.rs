//! `bazel run //tools/api:update` rewrites the API snapshot under `api/`.

mod snapshot;

use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::var("BUILD_WORKSPACE_DIRECTORY")
        .map_err(|_| "run with `bazel run //tools/api:update`")?;
    let dir = PathBuf::from(root).join("api");
    let files = snapshot::render()?;
    let schemas = dir.join("schemas.txt");
    // A missing file is the first run; any other read error is real.
    let checked_in = match std::fs::read_to_string(&schemas) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(format!("{}: {error}", schemas.display()).into()),
    };
    let problems = snapshot::unbumped_schemas(&checked_in, &snapshot::schemas_txt());
    if !problems.is_empty() {
        return Err(problems.join("\n").into());
    }
    for (name, text) in files {
        let path = dir.join(name);
        std::fs::write(&path, text).map_err(|error| format!("{}: {error}", path.display()))?;
    }
    Ok(())
}
