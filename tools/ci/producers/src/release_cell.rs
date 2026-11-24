use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use dx_process::capture::run_checked;

const BAZEL: &str = "bazel";
const CODESIGN: &str = "codesign";
const CONFIG: &str = "--config=dx_build_release";
const PURE: &str = "--@rules_go//go/config:pure";
const RELEASE: &str = "//deploy/private/release";
const GAZELLE: &str = "@dx_gazelle//:gazelle";
const LAUNCHER: &str = "//cli/launcher:exec_launcher";
const NATIVE_TOOLS: [Tool; 5] = [
    Tool {
        tool: "capnp",
        label: "@dx_capnproto//src/capnp:capnp_tool",
        licenses: &["@dx_capnproto//:LICENSE.txt"],
    },
    Tool {
        tool: "flatc",
        label: "@dx_flatbuffers//:flatc",
        licenses: &["@dx_flatbuffers//:LICENSE"],
    },
    Tool {
        tool: "ldoc-main",
        label: "//tools/docs/lua:ldoc_main",
        licenses: &[
            "//lua/private:lua_LICENSE.txt",
            "@dx_luafilesystem//:LICENSE",
        ],
    },
    Tool {
        tool: "runtime",
        label: "//cli/runtime:dx_runtime",
        licenses: &[],
    },
    Tool {
        tool: "thrift",
        label: "@dx_thrift//:thrift_compiler",
        licenses: &["@dx_thrift//:LICENSE", "@dx_thrift//:NOTICE"],
    },
];
const WINDOWS_TOOLS: [Tool; 1] = [Tool {
    tool: "msvc-link",
    label: "//rust/private/tools/msvc_paths:msvc_link",
    licenses: &[],
}];
const LICENSE_CELL: &str = "dx-linux-x86_64";
struct Tool {
    tool: &'static str,
    label: &'static str,
    licenses: &'static [&'static str],
}

const WINDOWS_ARM64_LIBRARIES: [Tool; 2] = [
    Tool {
        tool: "tree-sitter",
        label: "@jvm_tree_sitter//:tree-sitter.dll",
        licenses: &["@jvm_tree_sitter//:LICENSE"],
    },
    Tool {
        tool: "tree-sitter-pkl",
        label: "@jvm_tree_sitter_pkl//:tree-sitter-pkl.dll",
        licenses: &[
            "@jvm_tree_sitter_pkl//:LICENSE.txt",
            "@jvm_tree_sitter_pkl//:NOTICE.txt",
        ],
    },
];
const PCOV_LIBRARY: Tool = Tool {
    tool: "pcov",
    label: "@dx_php_pcov//:pcov.so",
    licenses: &["@dx_php_pcov//:pcov-1.0.12/LICENSE"],
};
const PCOV: [Tool; 1] = [PCOV_LIBRARY];
const LINUX_X86_64_LIBRARIES: [Tool; 2] = [
    PCOV_LIBRARY,
    Tool {
        tool: "julia-internal",
        label: "//julia/private/runtime:libjulia_internal",
        licenses: &[
            "@julia_src//:LICENSE.md",
            "@julia_src_libuv//:share/licenses/LibUV/LICENSE",
            "@julia_src_utf8proc//:LICENSE.md",
            "@julia_src_libllvm//:share/licenses/libLLVM/LICENSE.TXT",
        ],
    },
];
const QUERY_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const TOOL_TIMEOUT: Duration = Duration::from_secs(5 * 60);

struct Env {
    cell: String,
    workflow_ref: String,
    workflow_sha: String,
    run_id: String,
    job: String,
    sha: String,
    accept_licenses: String,
    temp: PathBuf,
    workspace: PathBuf,
}

fn var(name: &str) -> Result<String, String> {
    std::env::var(name).map_err(|error| format!("release-cell: set {name}: {error}"))
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
            accept_licenses: std::env::var("ACCEPT_LICENSES").unwrap_or_default(),
            temp: PathBuf::from(var("RUNNER_TEMP")?),
            workspace: PathBuf::from(var("GITHUB_WORKSPACE")?),
        };
        if env.workflow_ref.is_empty() || env.workflow_sha.is_empty() {
            return Err("job.workflow_ref or job.workflow_sha is empty".to_string());
        }
        Ok(env)
    }
}

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
        run_output(&mut self.command(args), QUERY_TIMEOUT)
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

    fn output_base(&self) -> Result<PathBuf, String> {
        self.output(&["info", CONFIG, "output_base"])
            .map(PathBuf::from)
    }

    fn workspace(&self) -> Result<PathBuf, String> {
        self.output(&["info", CONFIG, "workspace"])
            .map(PathBuf::from)
    }

    fn exec_path(&self, flags: &[&str], target: &str) -> Result<String, String> {
        let mut args = vec!["cquery", CONFIG];
        args.extend_from_slice(flags);
        args.extend_from_slice(&["--output=files", target]);
        self.output(&args)
    }

    fn file(&self, root: &Path, flags: &[&str], target: &str) -> Result<PathBuf, String> {
        Ok(root.join(self.exec_path(flags, target)?))
    }

    fn source(&self, output_base: &Path, workspace: &Path, label: &str) -> Result<PathBuf, String> {
        source_file(output_base, workspace, &self.exec_path(&[], label)?)
    }
}

fn source_file(output_base: &Path, workspace: &Path, exec_path: &str) -> Result<PathBuf, String> {
    if exec_path.starts_with("external/") {
        Ok(output_base.join(exec_path))
    } else if exec_path.starts_with("bazel-out/") || Path::new(exec_path).is_absolute() {
        Err(format!("{exec_path} is not a source file"))
    } else {
        Ok(workspace.join(exec_path))
    }
}

fn copy(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::copy(from, to)
        .map(|_| ())
        .map_err(|error| format!("copy {} to {}: {error}", from.display(), to.display()))
}

fn release_copy(cell: &str, from: &Path, to: &Path) -> Result<(), String> {
    copy(from, to)?;
    if !is_macos(cell) || codesign(&["--verify", "--strict"], to)? {
        return Ok(());
    }
    make_writable(to)?;
    if !codesign(&["--sign", "-"], to)? || !codesign(&["--verify", "--strict"], to)? {
        return Err(format!("ad-hoc signing {} failed", to.display()));
    }
    Ok(())
}

fn is_macos(cell: &str) -> bool {
    cell.starts_with("dx-macos-")
}

#[cfg(unix)]
fn make_writable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(path)
        .map_err(|error| format!("stat {}: {error}", path.display()))?
        .permissions()
        .mode();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode | 0o200))
        .map_err(|error| format!("chmod u+w {}: {error}", path.display()))
}

#[cfg(not(unix))]
fn make_writable(_path: &Path) -> Result<(), String> {
    Err("ad-hoc signing runs only on a macOS cell".to_owned())
}

fn codesign(args: &[&str], path: &Path) -> Result<bool, String> {
    Command::new(CODESIGN)
        .args(args)
        .arg(path)
        .status()
        .map(|status| status.success())
        .map_err(|error| format!("spawn {CODESIGN}: {error}"))
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

fn symbols_name(cell: &str) -> Option<String> {
    cell.starts_with("dx-windows-")
        .then(|| format!("{cell}-pdb.zip"))
}

fn tool_asset_name(tool: &str, cell: &str) -> String {
    format!("dx-{tool}-{}", cell.strip_prefix("dx-").unwrap_or(cell))
}

fn cell_tools(cell: &str) -> Vec<&'static Tool> {
    let windows: &'static [Tool] = if cell.starts_with("dx-windows-") {
        &WINDOWS_TOOLS
    } else {
        &[]
    };
    NATIVE_TOOLS.iter().chain(windows).collect()
}

fn libraries(cell: &str) -> &'static [Tool] {
    match cell {
        "dx-windows-arm64" => &WINDOWS_ARM64_LIBRARIES,
        "dx-linux-x86_64" => &LINUX_X86_64_LIBRARIES,
        "dx-linux-arm64" | "dx-macos-arm64" => &PCOV,
        _ => &[],
    }
}

fn library_licenses(cell: &str) -> &'static [Tool] {
    match cell {
        "dx-windows-arm64" => &WINDOWS_ARM64_LIBRARIES,
        LICENSE_CELL => &LINUX_X86_64_LIBRARIES,
        _ => &[],
    }
}

fn native_licenses(cell: &str) -> Vec<&'static Tool> {
    if cell == LICENSE_CELL {
        NATIVE_TOOLS
            .iter()
            .filter(|tool| !tool.licenses.is_empty())
            .collect()
    } else {
        Vec::new()
    }
}

fn library_asset_name(tool: &str, cell: &str) -> String {
    let ext = if cell.starts_with("dx-windows-") {
        "dll"
    } else {
        "so"
    };
    format!("{}.{ext}", tool_asset_name(tool, cell))
}

fn write_license(
    bazel: &Bazel,
    output_base: &Path,
    workspace: &Path,
    tool: &Tool,
    out: &Path,
) -> Result<(), String> {
    let mut text = String::new();
    for label in tool.licenses {
        let path = bazel.source(output_base, workspace, label)?;
        let part = std::fs::read_to_string(&path)
            .map_err(|error| format!("read {}: {error}", path.display()))?;
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&part);
        if !text.ends_with('\n') {
            text.push('\n');
        }
    }
    write(&out.join(format!("dx-{}-LICENSE", tool.tool)), &text)
}

fn same_bytes(left: &Path, right: &Path) -> Result<bool, String> {
    let read = |path: &Path| {
        std::fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))
    };
    Ok(read(left)? == read(right)?)
}

fn rebuild_cell(
    env: &Env,
    out: &Path,
    symbols: Option<&str>,
    pdb: &str,
    gazelle_asset: &str,
    launcher_asset: &str,
    archive_name: &str,
) -> Result<(PathBuf, Vec<String>), String> {
    let rebuild_base = env.temp.join("rebuild-base");
    std::fs::remove_dir_all(&rebuild_base).unwrap_or_default();
    let rebuild = Bazel {
        base: Some(rebuild_base),
    };
    let no_cache = ["--disk_cache=", "--remote_cache="];
    let mut rebuilt_targets = vec![format!("{RELEASE}:dx_host_cell"), LAUNCHER.to_owned()];
    rebuilt_targets.extend(
        cell_tools(&env.cell)
            .iter()
            .map(|tool| tool.label.to_owned()),
    );
    rebuilt_targets.extend(
        libraries(&env.cell)
            .iter()
            .map(|library| library.label.to_owned()),
    );
    if symbols.is_some() {
        rebuilt_targets.push(pdb.to_owned());
    }
    let rebuilt_targets: Vec<&str> = rebuilt_targets.iter().map(String::as_str).collect();
    rebuild.build(&no_cache, &rebuilt_targets)?;
    let rebuild_root = rebuild.execution_root()?;
    let rebuilt_archive = env.temp.join(format!("rebuilt-{archive_name}"));
    copy(
        &rebuild.file(&rebuild_root, &[], &format!("{RELEASE}:dx_host_cell"))?,
        &rebuilt_archive,
    )?;
    let rebuilt_launcher = env.temp.join(format!("rebuilt-{launcher_asset}"));
    release_copy(
        &env.cell,
        &rebuild.file(&rebuild_root, &[], LAUNCHER)?,
        &rebuilt_launcher,
    )?;
    if !same_bytes(&out.join(launcher_asset), &rebuilt_launcher)? {
        return Err(format!("{launcher_asset} differs between two builds"));
    }
    for tool in cell_tools(&env.cell) {
        let asset = tool_asset_name(tool.tool, &env.cell);
        let rebuilt = env.temp.join(format!("rebuilt-{asset}"));
        release_copy(
            &env.cell,
            &rebuild.file(&rebuild_root, &[], tool.label)?,
            &rebuilt,
        )?;
        if !same_bytes(&out.join(&asset), &rebuilt)? {
            return Err(format!("{asset} differs between two builds"));
        }
    }
    for library in libraries(&env.cell) {
        let asset = library_asset_name(library.tool, &env.cell);
        let rebuilt = env.temp.join(format!("rebuilt-{asset}"));
        copy(&rebuild.file(&rebuild_root, &[], library.label)?, &rebuilt)?;
        if !same_bytes(&out.join(&asset), &rebuilt)? {
            return Err(format!("{asset} differs between two builds"));
        }
    }
    let mut qualify_symbols = Vec::new();
    if let Some(symbols) = symbols {
        let rebuilt_symbols = env.temp.join(format!("rebuilt-{symbols}"));
        copy(&rebuild.file(&rebuild_root, &[], pdb)?, &rebuilt_symbols)?;
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
    let rebuilt_gazelle = env.temp.join(format!("rebuilt-{gazelle_asset}"));
    release_copy(
        &env.cell,
        &rebuild.file(&rebuild_root, &[PURE], GAZELLE)?,
        &rebuilt_gazelle,
    )?;
    if !same_bytes(&out.join(gazelle_asset), &rebuilt_gazelle)? {
        return Err(format!("{gazelle_asset} differs between two builds"));
    }
    rebuild.run(&["shutdown"])?;
    Ok((rebuilt_archive, qualify_symbols))
}

fn build_cell(env: &Env, out: &Path) -> Result<(), String> {
    let main = Bazel { base: None };
    let symbols = symbols_name(&env.cell);
    let release_targets = [
        format!("{RELEASE}:release_cell"),
        format!("{RELEASE}:release_matrix"),
        format!("{RELEASE}:dx_host_cell"),
        format!("{RELEASE}:dx_host_cell_sbom"),
    ];
    let mut targets = vec!["//cli/cli:dx", LAUNCHER];
    targets.extend(cell_tools(&env.cell).iter().map(|tool| tool.label));
    targets.extend(libraries(&env.cell).iter().map(|library| library.label));
    targets.extend(release_targets.iter().map(String::as_str));
    main.build(&[], &targets)?;
    let pdb = format!("{RELEASE}:dx_host_cell_pdb");
    let pdb_sbom = format!("{RELEASE}:dx_host_cell_pdb_sbom");
    if symbols.is_some() {
        main.build(&[], &[&pdb, &pdb_sbom])?;
    }
    let gazelle_flags = [PURE];
    main.build(&gazelle_flags, &[GAZELLE])?;
    let root = main.execution_root()?;
    let output_base = main.output_base()?;
    let workspace = main.workspace()?;
    let gazelle_asset = tool_asset_name("gazelle", &env.cell);
    release_copy(
        &env.cell,
        &main.file(&root, &gazelle_flags, GAZELLE)?,
        &out.join(&gazelle_asset),
    )?;
    let launcher_asset = tool_asset_name("launcher", &env.cell);
    release_copy(
        &env.cell,
        &main.file(&root, &[], LAUNCHER)?,
        &out.join(&launcher_asset),
    )?;
    for tool in cell_tools(&env.cell) {
        release_copy(
            &env.cell,
            &main.file(&root, &[], tool.label)?,
            &out.join(tool_asset_name(tool.tool, &env.cell)),
        )?;
    }
    for tool in native_licenses(&env.cell) {
        write_license(&main, &output_base, &workspace, tool, out)?;
    }
    for library in libraries(&env.cell) {
        copy(
            &main.file(&root, &[], library.label)?,
            &out.join(library_asset_name(library.tool, &env.cell)),
        )?;
    }
    for library in library_licenses(&env.cell) {
        write_license(&main, &output_base, &workspace, library, out)?;
    }
    let dx = main.file(&root, &[], "//cli/cli:dx")?;
    if is_macos(&env.cell) && !codesign(&["--verify", "--strict"], &dx)? {
        return Err(format!(
            "{} has no code signature; the macOS archive ships the linker-signed dx",
            dx.display()
        ));
    }
    let tool = main.file(&root, &[], &format!("{RELEASE}:release_cell"))?;
    let matrix = main.file(&root, &[], &format!("{RELEASE}:release_matrix"))?;
    let built = main.file(&root, &[], &format!("{RELEASE}:dx_host_cell"))?;
    let version = run_output(
        Command::new(&tool).args(["version", "--exe"]).arg(&dx),
        TOOL_TIMEOUT,
    )?;
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

    let (rebuilt_archive, qualify_symbols) = rebuild_cell(
        env,
        out,
        symbols.as_deref(),
        &pdb,
        &gazelle_asset,
        &launcher_asset,
        &archive_name,
    )?;

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
        .arg(&env.workspace);
    if !env.accept_licenses.is_empty() {
        qualify.args(["--accept-licenses", &env.accept_licenses]);
    }
    qualify
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

fn run_output(command: &mut Command, timeout: Duration) -> Result<String, String> {
    let out = run_checked(command, timeout).map_err(|error| error.to_string())?;
    Ok(out.stdout_text().trim().to_string())
}

pub fn cell(args: &[String]) -> Result<(), String> {
    let [out] = args else {
        return Err("usage: producers_bin --release-cell OUT".to_string());
    };
    let env = Env::read()?;
    let out = PathBuf::from(out);
    std::fs::remove_dir_all(&out).unwrap_or_default();
    std::fs::create_dir_all(&out).map_err(|error| format!("create {}: {error}", out.display()))?;
    build_cell(&env, &out)
}

#[cfg(test)]
mod tests {
    use super::{
        NATIVE_TOOLS, cell_tools, is_macos, libraries, library_asset_name, library_licenses,
        native_licenses, source_file, symbols_name, tool_asset_name,
    };
    use std::path::Path;

    #[test]
    fn source_files_resolve_under_the_output_base_or_the_workspace() {
        let base = Path::new("/base");
        let workspace = Path::new("/workspace");
        assert_eq!(
            source_file(base, workspace, "external/+ext+jvm_tree_sitter/LICENSE"),
            Ok(base.join("external/+ext+jvm_tree_sitter/LICENSE"))
        );
        assert_eq!(
            source_file(base, workspace, "lua/private/lua_LICENSE.txt"),
            Ok(workspace.join("lua/private/lua_LICENSE.txt"))
        );
        assert!(source_file(base, workspace, "bazel-out/x64_windows-opt/bin/a.dll").is_err());
    }

    fn names(tools: &[super::Tool]) -> Vec<&str> {
        tools.iter().map(|tool| tool.tool).collect()
    }

    #[test]
    fn windows_arm64_ships_tree_sitter_and_unix_cells_ship_pcov_and_julia() {
        assert_eq!(
            names(libraries("dx-windows-arm64")),
            ["tree-sitter", "tree-sitter-pkl"]
        );
        assert!(libraries("dx-windows-x86_64").is_empty());
        assert_eq!(
            names(libraries("dx-linux-x86_64")),
            ["pcov", "julia-internal"]
        );
        for cell in ["dx-linux-arm64", "dx-macos-arm64"] {
            assert_eq!(names(libraries(cell)), ["pcov"], "{cell}");
        }
    }

    #[test]
    fn each_library_licence_comes_from_one_cell() {
        assert_eq!(
            names(library_licenses("dx-windows-arm64")),
            ["tree-sitter", "tree-sitter-pkl"]
        );
        assert_eq!(
            names(library_licenses("dx-linux-x86_64")),
            ["pcov", "julia-internal"]
        );
        for cell in ["dx-linux-arm64", "dx-macos-arm64", "dx-windows-x86_64"] {
            assert!(library_licenses(cell).is_empty(), "{cell}");
        }
    }

    #[test]
    fn library_assets_end_in_dll_on_windows_and_so_elsewhere() {
        assert_eq!(
            library_asset_name("pcov", "dx-linux-x86_64"),
            "dx-pcov-linux-x86_64.so"
        );
        assert_eq!(
            library_asset_name("pcov", "dx-macos-arm64"),
            "dx-pcov-macos-arm64.so"
        );
        assert_eq!(
            library_asset_name("julia-internal", "dx-linux-x86_64"),
            "dx-julia-internal-linux-x86_64.so"
        );
        assert_eq!(
            library_asset_name("tree-sitter", "dx-windows-arm64"),
            "dx-tree-sitter-windows-arm64.dll"
        );
        assert_eq!(
            library_asset_name("tree-sitter-pkl", "dx-windows-arm64"),
            "dx-tree-sitter-pkl-windows-arm64.dll"
        );
    }

    #[test]
    fn only_macos_cells_sign() {
        assert!(is_macos("dx-macos-arm64"));
        assert!(!is_macos("dx-linux-arm64"));
        assert!(!is_macos("dx-windows-arm64"));
    }

    #[test]
    fn only_windows_cells_ship_symbols() {
        assert_eq!(
            symbols_name("dx-windows-x86_64").as_deref(),
            Some("dx-windows-x86_64-pdb.zip")
        );
        assert_eq!(symbols_name("dx-linux-x86_64"), None);
    }

    #[test]
    fn tool_assets_drop_the_dx_prefix() {
        assert_eq!(
            tool_asset_name("gazelle", "dx-linux-x86_64"),
            "dx-gazelle-linux-x86_64"
        );
        assert_eq!(
            tool_asset_name("launcher", "dx-windows-arm64"),
            "dx-launcher-windows-arm64"
        );
        assert_eq!(
            tool_asset_name("runtime", "dx-macos-arm64"),
            "dx-runtime-macos-arm64"
        );
        assert_eq!(
            tool_asset_name("ldoc-main", "dx-windows-arm64"),
            "dx-ldoc-main-windows-arm64"
        );
    }

    #[test]
    fn only_windows_cells_ship_msvc_link() {
        let tools = |cell| {
            cell_tools(cell)
                .iter()
                .map(|tool| tool.tool)
                .collect::<Vec<_>>()
        };
        for cell in ["dx-windows-arm64", "dx-windows-x86_64"] {
            assert_eq!(
                tools(cell),
                [
                    "capnp",
                    "flatc",
                    "ldoc-main",
                    "runtime",
                    "thrift",
                    "msvc-link"
                ],
                "{cell}"
            );
        }
        for cell in ["dx-linux-arm64", "dx-linux-x86_64", "dx-macos-arm64"] {
            assert_eq!(
                tools(cell),
                ["capnp", "flatc", "ldoc-main", "runtime", "thrift"],
                "{cell}"
            );
        }
    }

    #[test]
    fn native_licenses_ship_from_linux_x86_64_only() {
        let tools = |cell| {
            native_licenses(cell)
                .iter()
                .map(|tool| tool.tool)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            tools("dx-linux-x86_64"),
            ["capnp", "flatc", "ldoc-main", "thrift"]
        );
        for cell in [
            "dx-linux-arm64",
            "dx-macos-arm64",
            "dx-windows-arm64",
            "dx-windows-x86_64",
        ] {
            assert!(tools(cell).is_empty(), "{cell}");
        }
    }

    #[test]
    fn only_the_dx_runtime_ships_without_a_licence() {
        let unlicensed: Vec<_> = NATIVE_TOOLS
            .iter()
            .filter(|tool| tool.licenses.is_empty())
            .map(|tool| tool.tool)
            .collect();
        assert_eq!(unlicensed, ["runtime"]);
    }
}
