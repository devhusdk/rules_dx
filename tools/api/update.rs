//! `bazel run //tools/api:update` rewrites the API snapshot under `api/` and the page generated from it.

mod page;
mod snapshot;

use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::var("BUILD_WORKSPACE_DIRECTORY")
        .map_err(|_| "run with `bazel run //tools/api:update`")?;
    let root = PathBuf::from(root);
    let dir = root.join("api");
    let builds = |paths: Vec<String>| -> Result<Vec<(String, String)>, String> {
        paths
            .into_iter()
            .map(|path| {
                let file = root.join(&path);
                let text = std::fs::read_to_string(&file)
                    .map_err(|error| format!("{}: {error}", file.display()))?;
                Ok((snapshot::package_of(&path)?, text))
            })
            .collect()
    };
    let builds = snapshot::Builds {
        langs: builds(snapshot::lang_build_paths()?)?,
        commands: builds(snapshot::command_build_paths()?)?,
        packages: builds(snapshot::package_build_paths()?)?,
    };
    let runfiles = runfiles::Runfiles::create()?;
    let mut starlark = Vec::new();
    for key in std::env::var("DX_API_STARLARK")?.split_whitespace() {
        let file =
            runfiles::rlocation!(runfiles, key).ok_or(format!("{key} is not in the runfiles"))?;
        starlark
            .push(std::fs::read(&file).map_err(|error| format!("{}: {error}", file.display()))?);
    }
    let mut forwards = Vec::new();
    for key in std::env::var("DX_API_FORWARDS")?.split_whitespace() {
        let file =
            runfiles::rlocation!(runfiles, key).ok_or(format!("{key} is not in the runfiles"))?;
        forwards.push(
            std::fs::read_to_string(&file)
                .map_err(|error| format!("{}: {error}", file.display()))?,
        );
    }
    let files = snapshot::render(&builds, &starlark, &forwards)?;
    let snapshot_file = |name: &str| files.get(name).ok_or(format!("the snapshot has no {name}"));
    let surface = page::render(
        snapshot_file("cli.json")?,
        snapshot_file("starlark.json")?,
        snapshot_file("labels.txt")?,
        snapshot_file("schemas.txt")?,
    )?;
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
    let path = root.join(page::PATH);
    std::fs::write(&path, surface).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(())
}
