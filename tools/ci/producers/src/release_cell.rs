//! Builds the dx release archive and the pure-Go dx gazelle twice and qualifies them as one release cell.

use std::path::{Path, PathBuf};
use std::process::Command;

const BAZEL: &str = "bazel";
const CONFIG: &str = "--config=dx_build_release";
const PURE: &str = "--@rules_go//go/config:pure";
const RELEASE: &str = "//deploy/private/release";
const GAZELLE: &str = "@dx_gazelle//:gazelle";

struct Env {
    cell: String,
    workflow_ref: String,
    workflow_sha: String,
    run_id: String,
    job: String,
    sha: String,
    temp: PathBuf,
    workspace: PathBuf,
}

fn var(name: &str) -> Result<String, String> {
    std::env::var(name).map_err(|_| format!("release-cell: set {name}"))
}

impl Env {
    fn read() -> Result<Self, String> {
        let env = Self {
            cell: var("CELL")?,
            workflow_ref: var("WORKFLOW_REF")?,
            workflow_sha: var("WORKFLOW_SHA")?,
            run_id: var("RUN_ID")?,
            job: var("JOB")?,
            sha: var("SHA")?,
            temp: PathBuf::from(var("RUNNER_TEMP")?),
            workspace: PathBuf::from(var("GITHUB_WORKSPACE")?),
        };
        if env.workflow_ref.is_empty() || env.workflow_sha.is_empty() {
            return Err("job.workflow_ref or job.workflow_sha is empty".to_string());
        }
        Ok(env)
    }
}

/// A bazel server, the default one or one with its own output base.
struct Bazel {
    base: Option<PathBuf>,
}

impl Bazel {
    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(BAZEL);
        if let Some(base) = &self.base {
            command.arg(format!("--output_base={}", base.display()));
        }
        command.args(args);
        command
    }

    fn run(&self, args: &[&str]) -> Result<(), String> {
        let status = self
            .command(args)
            .status()
            .map_err(|error| format!("spawn bazel: {error}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("bazel {} failed: {status}", args.join(" ")))
        }
    }

    fn output(&self, args: &[&str]) -> Result<String, String> {
        let out = self
            .command(args)
            .output()
            .map_err(|error| format!("spawn bazel: {error}"))?;
        if !out.status.success() {
            return Err(format!(
                "bazel {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    fn build(&self, flags: &[&str], targets: &[&str]) -> Result<(), String> {
        let mut args = vec!["build", CONFIG];
        args.extend_from_slice(flags);
        args.extend_from_slice(targets);
        self.run(&args)
    }

    fn execution_root(&self) -> Result<PathBuf, String> {
        self.output(&["info", CONFIG, "execution_root"])
            .map(PathBuf::from)
    }

    /// The one file `target` builds, under `root`.
    fn file(&self, root: &Path, flags: &[&str], target: &str) -> Result<PathBuf, String> {
        let mut args = vec!["cquery", CONFIG];
        args.extend_from_slice(flags);
        args.extend_from_slice(&["--output=files", target]);
        Ok(root.join(self.output(&args)?))
    }
}

fn copy(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::copy(from, to)
        .map(|_| ())
        .map_err(|error| format!("copy {} to {}: {error}", from.display(), to.display()))
}

fn checksum_line(file: &Path, name: &str) -> Result<String, String> {
    Ok(format!("{}  {name}\n", dx_testing::sha256_file(file)?))
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    std::fs::write(path, text).map_err(|error| format!("write {}: {error}", path.display()))
}

fn name_of(path: &Path) -> Result<String, String> {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| format!("{} has no file name", path.display()))
}

/// The symbols archive name of a cell, empty when the cell ships none.
fn symbols_name(cell: &str) -> Option<String> {
    cell.starts_with("dx-windows-")
        .then(|| format!("{cell}-pdb.zip"))
}

/// The gazelle asset name of a cell.
fn gazelle_name(cell: &str) -> String {
    format!("dx-gazelle-{}", cell.strip_prefix("dx-").unwrap_or(cell))
}

fn same_bytes(left: &Path, right: &Path) -> Result<bool, String> {
    let read = |path: &Path| {
        std::fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))
    };
    Ok(read(left)? == read(right)?)
}

fn build_cell(env: &Env, out: &Path) -> Result<(), String> {
    let main = Bazel { base: None };
    let symbols = symbols_name(&env.cell);
    main.build(
        &[],
        &[
            "//cli/cli:dx",
            &format!("{RELEASE}:release_cell"),
            &format!("{RELEASE}:release_matrix"),
            &format!("{RELEASE}:dx_host_cell"),
            &format!("{RELEASE}:dx_host_cell_sbom"),
        ],
    )?;
    let pdb = format!("{RELEASE}:dx_host_cell_pdb");
    let pdb_sbom = format!("{RELEASE}:dx_host_cell_pdb_sbom");
    if symbols.is_some() {
        main.build(&[], &[&pdb, &pdb_sbom])?;
    }
    let gazelle_flags = [PURE];
    main.build(&gazelle_flags, &[GAZELLE])?;
    let root = main.execution_root()?;
    let gazelle_asset = gazelle_name(&env.cell);
    copy(
        &main.file(&root, &gazelle_flags, GAZELLE)?,
        &out.join(&gazelle_asset),
    )?;
    let dx = main.file(&root, &[], "//cli/cli:dx")?;
    let tool = main.file(&root, &[], &format!("{RELEASE}:release_cell"))?;
    let matrix = main.file(&root, &[], &format!("{RELEASE}:release_matrix"))?;
    let built = main.file(&root, &[], &format!("{RELEASE}:dx_host_cell"))?;
    let version = run_output(Command::new(&tool).args(["version", "--exe"]).arg(&dx))?;
    let archive_name = name_of(&built)?;
    let archive = out.join(&archive_name);
    copy(&built, &archive)?;
    copy(
        &main.file(&root, &[], &format!("{RELEASE}:dx_host_cell_sbom"))?,
        &out.join(format!("{archive_name}.spdx.json")),
    )?;
    write(
        &out.join(format!("{archive_name}.sha256")),
        &checksum_line(&archive, &archive_name)?,
    )?;
    if let Some(symbols) = &symbols {
        let target = out.join(symbols);
        copy(&main.file(&root, &[], &pdb)?, &target)?;
        copy(
            &main.file(&root, &[], &pdb_sbom)?,
            &out.join(format!("{symbols}.spdx.json")),
        )?;
        write(
            &out.join(format!("{symbols}.sha256")),
            &checksum_line(&target, symbols)?,
        )?;
    }

    // The determinism input is a second, independent build: its own output base, no caches.
    let rebuild_base = env.temp.join("rebuild-base");
    let _ = std::fs::remove_dir_all(&rebuild_base);
    let rebuild = Bazel {
        base: Some(rebuild_base),
    };
    let no_cache = ["--disk_cache=", "--remote_cache="];
    let mut rebuilt_targets = vec![format!("{RELEASE}:dx_host_cell")];
    if symbols.is_some() {
        rebuilt_targets.push(pdb.clone());
    }
    let rebuilt_targets: Vec<&str> = rebuilt_targets.iter().map(String::as_str).collect();
    rebuild.build(&no_cache, &rebuilt_targets)?;
    let rebuild_root = rebuild.execution_root()?;
    let rebuilt_archive = env.temp.join(format!("rebuilt-{archive_name}"));
    copy(
        &rebuild.file(&rebuild_root, &[], &format!("{RELEASE}:dx_host_cell"))?,
        &rebuilt_archive,
    )?;
    let mut qualify_symbols = Vec::new();
    if let Some(symbols) = &symbols {
        let rebuilt_symbols = env.temp.join(format!("rebuilt-{symbols}"));
        copy(&rebuild.file(&rebuild_root, &[], &pdb)?, &rebuilt_symbols)?;
        qualify_symbols = vec![
            "--symbols".to_string(),
            out.join(symbols).display().to_string(),
            "--symbols-rebuild".to_string(),
            rebuilt_symbols.display().to_string(),
        ];
    }
    let mut gazelle_build = vec![PURE];
    gazelle_build.extend_from_slice(&no_cache);
    rebuild.build(&gazelle_build, &[GAZELLE])?;
    let rebuilt_gazelle = rebuild.file(&rebuild_root, &gazelle_flags, GAZELLE)?;
    if !same_bytes(&out.join(&gazelle_asset), &rebuilt_gazelle)? {
        return Err(format!("{gazelle_asset} differs between two builds"));
    }
    rebuild.run(&["shutdown"])?;

    let mut qualify = Command::new(&tool);
    qualify
        .arg("qualify")
        .arg("--matrix")
        .arg(&matrix)
        .args(["--cell", &env.cell])
        .arg("--archive")
        .arg(&archive)
        .arg("--rebuild")
        .arg(&rebuilt_archive)
        .arg("--work")
        .arg(env.temp.join("qualify-work"))
        .args(["--sha", &env.sha])
        .args(["--version", &version])
        .args(["--workflow-ref", &env.workflow_ref])
        .args(["--workflow-sha", &env.workflow_sha])
        .args(["--run-id", &env.run_id])
        .args(["--job", &env.job])
        .arg("--consumer")
        .arg(env.workspace.join("deploy/private/release/consumer"))
        .arg("--checkout")
        .arg(&env.workspace)
        .arg("--out")
        .arg(out.join(format!("{}.json", env.cell)))
        .args(&qualify_symbols);
    let status = qualify
        .status()
        .map_err(|error| format!("spawn {}: {error}", tool.display()))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("release_cell qualify failed: {status}"))
    }
}

fn run_output(command: &mut Command) -> Result<String, String> {
    let out = command
        .output()
        .map_err(|error| format!("spawn release_cell: {error}"))?;
    if !out.status.success() {
        return Err(format!(
            "release_cell failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Builds and qualifies the release cell named by the environment into the directory `OUT`.
pub fn cell(args: &[String]) -> Result<(), String> {
    let [out] = args else {
        return Err("usage: producers_bin --release-cell OUT".to_string());
    };
    let env = Env::read()?;
    let out = PathBuf::from(out);
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).map_err(|error| format!("create {}: {error}", out.display()))?;
    build_cell(&env, &out)
}

#[cfg(test)]
mod tests {
    use super::{gazelle_name, symbols_name};

    #[test]
    fn only_windows_cells_ship_symbols() {
        assert_eq!(
            symbols_name("dx-windows-x86_64").as_deref(),
            Some("dx-windows-x86_64-pdb.zip")
        );
        assert_eq!(symbols_name("dx-linux-x86_64"), None);
    }

    #[test]
    fn gazelle_assets_drop_the_dx_prefix() {
        assert_eq!(gazelle_name("dx-linux-x86_64"), "dx-gazelle-linux-x86_64");
    }
}
