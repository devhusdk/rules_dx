#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub const PRESET_BAZEL_VERSION: &str = "9.2.0";

pub const PRESET_DX_VERSION: &str = "0.0.0";

/// Where rules_dx keeps its own copy of the preset layers.
pub const REPO_DIR: &str = "tools/bazelrc";

/// Where `dx init` writes the preset layers in a consumer workspace.
pub const CONSUMER_DIR: &str = ".dx/bazelrc";

/// The first line of every generated layer; a file that starts with it belongs to dx.
pub const HEADER_MARKER: &str = "# Vendored Bazel execution preset -- GENERATED";

/// One generated line: the flag and the comment rendered directly above it.
type Flag = (&'static str, &'static str);

const UPSTREAM_FLAGS: [Flag; 3] = [
    (
        "common --enable_bzlmod",
        "Resolve external dependencies through MODULE.bazel.",
    ),
    (
        "build --verbose_failures",
        "Print the full command line of a failed action.",
    ),
    (
        "test --test_output=errors",
        "Show the log of a failing test in the console.",
    ),
];

const COVERAGE_FLAGS: [Flag; 8] = [
    (
        "common --enable_platform_specific_config",
        "Apply the linux, macos and windows config lines on matching hosts.",
    ),
    (
        "coverage --test_env=GENERATE_LLVM_LCOV=1",
        "Make LLVM coverage write lcov data.",
    ),
    (
        "coverage --combined_report=lcov",
        "Merge the coverage of every test into one lcov report.",
    ),
    (
        "coverage --test_tag_filters=-no-coverage",
        "Skip tests tagged no-coverage during coverage runs.",
    ),
    (
        "coverage --enable_runfiles",
        "Coverage collectors read their inputs from a runfiles tree.",
    ),
    (
        "coverage:linux --test_env=COVERAGE_GCOV_PATH=/usr/bin/gcov",
        "Use the system gcov for gcc coverage on Linux.",
    ),
    (
        "coverage:macos --test_env=COVERAGE_GCOV_PATH=/usr/bin/gcov",
        "Use the system gcov for gcc coverage on macOS.",
    ),
    (
        "coverage --instrumentation_filter=^//",
        "Instrument only targets in the main repository.",
    ),
];

const BUILD_PROFILES: [Flag; 7] = [
    (
        "build --host_compilation_mode=fastbuild",
        "Build host tools without optimization so builds finish sooner.",
    ),
    (
        "build:dx_debug --compilation_mode=dbg",
        "The dx_debug config builds with debug symbols.",
    ),
    (
        "build:dx_dev --compilation_mode=fastbuild",
        "The dx_dev config builds unoptimized for quick edit and test loops.",
    ),
    (
        "build:dx_release --compilation_mode=opt",
        "The dx_release config builds optimized target code.",
    ),
    (
        "build:dx_release --host_compilation_mode=opt",
        "The dx_release config also optimizes host tools.",
    ),
    (
        "build:dx_dev_remote --compilation_mode=fastbuild",
        "The dx_dev_remote config builds unoptimized on remote executors.",
    ),
    (
        "build:dx_toolchain --compilation_mode=fastbuild",
        "The dx_toolchain config builds toolchains unoptimized.",
    ),
];

const RUNFILES_FLAGS: [Flag; 2] = [
    (
        "common --nolegacy_external_runfiles",
        "Keep external repositories out of the main runfiles directory.",
    ),
    (
        "build:windows --nobuild_runfile_links",
        "On Windows, build runfiles trees only for actions that run.",
    ),
];

const WINDOWS_FLAGS: [Flag; 1] = [(
    "build:windows --enable_runfiles",
    "On Windows, give tests a runfiles directory instead of only a manifest.",
)];

const RESOURCE_FLAGS: [Flag; 12] = [
    (
        "startup --host_jvm_args=-XX:MaxHeapFreeRatio=30",
        "Let the Bazel server return unused heap to the system sooner.",
    ),
    (
        "startup --host_jvm_args=-XX:MinHeapFreeRatio=10",
        "Keep little spare heap after a garbage collection.",
    ),
    (
        "startup --host_jvm_args=-XX:MaxRAMPercentage=25",
        "Cap the Bazel server heap at a quarter of host memory.",
    ),
    (
        "common --local_resources=memory=HOST_RAM*.5",
        "Schedule local actions within half of host memory.",
    ),
    (
        "common --local_resources=cpu=HOST_CPUS*.75",
        "Leave a quarter of the CPUs for the rest of the machine.",
    ),
    (
        "startup --max_idle_secs=900",
        "Stop a Bazel server after 15 idle minutes.",
    ),
    (
        "startup --shutdown_on_low_sys_mem",
        "Stop an idle Bazel server when the host runs low on memory.",
    ),
    (
        "common --experimental_disk_cache_gc_max_age=14d",
        "Drop disk cache entries unused for 14 days.",
    ),
    (
        "common --experimental_disk_cache_gc_max_size=20G",
        "Keep the disk cache under 20 GB.",
    ),
    (
        "common --repo_contents_cache_gc_max_age=60d",
        "Drop repository contents cache entries unused for 60 days.",
    ),
    (
        "build --experimental_shrink_worker_pool",
        "Shrink persistent worker pools when memory runs low.",
    ),
    (
        "build --experimental_total_worker_memory_limit_mb=4096",
        "Cap the memory of all persistent workers together at 4 GB.",
    ),
];

/// One generated preset file. A workspace imports every layer it wants from its root `.bazelrc`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Layer {
    /// Flags that change what a build or test produces.
    Correctness,
    /// Flags that trade build speed against output, plus the dx configs.
    Performance,
    /// Flags that bound memory, CPU and cache use on the host.
    Resources,
}

impl Layer {
    /// Every layer, in the order a root `.bazelrc` imports them.
    pub const ALL: [Layer; 3] = [Layer::Correctness, Layer::Performance, Layer::Resources];

    /// The layer name, as used in its file name.
    pub fn name(self) -> &'static str {
        match self {
            Layer::Correctness => "correctness",
            Layer::Performance => "performance",
            Layer::Resources => "resources",
        }
    }

    /// The layer's file name, for example `correctness.bazelrc`.
    pub fn file_name(self) -> &'static str {
        match self {
            Layer::Correctness => "correctness.bazelrc",
            Layer::Performance => "performance.bazelrc",
            Layer::Resources => "resources.bazelrc",
        }
    }

    fn summary(self) -> &'static str {
        match self {
            Layer::Correctness => "flags that change what a build or test produces.",
            Layer::Performance => "build speed and the dx compilation configs.",
            Layer::Resources => "memory, CPU and cache limits for the host.",
        }
    }

    fn groups(self) -> &'static [&'static [Flag]] {
        match self {
            Layer::Correctness => &[
                &UPSTREAM_FLAGS,
                &COVERAGE_FLAGS,
                &RUNFILES_FLAGS,
                &WINDOWS_FLAGS,
            ],
            Layer::Performance => &[&BUILD_PROFILES],
            Layer::Resources => &[&RESOURCE_FLAGS],
        }
    }

    fn flags(self) -> impl Iterator<Item = &'static Flag> {
        self.groups().iter().flat_map(|group| group.iter())
    }
}

/// Renders one layer file. The bytes are the same in rules_dx and in consumer workspaces.
pub fn render_layer(layer: Layer) -> String {
    let mut out = format!(
        "{HEADER_MARKER}, do not edit.\n\
         # Regenerate: `tools/dx init --apply`, or `bazel run //tools/bazelrc:preset_update` in rules_dx.\n\
         # Layer {}: {}\n",
        layer.name(),
        layer.summary()
    );
    for (flag, comment) in layer.flags() {
        out.push('\n');
        out.push_str("# ");
        out.push_str(comment);
        out.push('\n');
        out.push_str(flag);
        out.push('\n');
    }
    out
}

/// Renders every layer in import order, for collision checks over the whole preset.
pub fn render_all() -> String {
    Layer::ALL
        .iter()
        .map(|layer| render_layer(*layer))
        .collect()
}

/// The root `.bazelrc` line that imports `layer` from `dir`.
pub fn import_line(dir: &str, layer: Layer) -> String {
    format!("import %workspace%/{dir}/{}", layer.file_name())
}

/// How a root `.bazelrc` refers to one layer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportState {
    /// An `import` or `try-import` line names the layer.
    Active,
    /// Only a commented-out import names the layer, so the workspace opted out of it.
    CommentedOut,
    /// No line names the layer.
    Absent,
}

fn unquote(word: &str) -> &str {
    let bytes = word.as_bytes();
    if bytes.len() >= 2
        && ((bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\''))
    {
        &word[1..word.len() - 1]
    } else {
        word
    }
}

fn joined_lines(text: &str) -> String {
    text.strip_prefix('\u{feff}')
        .unwrap_or(text)
        .replace("\\\r\n", "")
        .replace("\\\n", "")
}

/// The workspace-relative file one rc line imports through `%workspace%`, if it is such an import.
pub fn workspace_import(line: &str) -> Option<String> {
    let rest = line
        .strip_prefix("import")
        .or_else(|| line.strip_prefix("try-import"))?;
    if !rest.starts_with([' ', '\t']) {
        return None;
    }
    let target = unquote(rest.trim());
    target
        .strip_prefix("%workspace%/")
        .map(|rel| rel.trim_start_matches('/').to_owned())
}

/// Lists the workspace-relative files a root `.bazelrc` imports through `%workspace%`.
pub fn bazelrc_imports(text: &str) -> Vec<String> {
    joined_lines(text)
        .lines()
        .filter_map(|raw| workspace_import(raw.trim()))
        .collect()
}

/// Reports, for every layer, whether `text` imports it from `dir`, comments the import out, or omits it.
pub fn layer_import_state(text: &str, dir: &str) -> [(Layer, ImportState); 3] {
    let joined = joined_lines(text);
    let mut active = BTreeSet::new();
    let mut commented = BTreeSet::new();
    for raw in joined.lines() {
        let line = raw.trim();
        let (body, set) = match line.strip_prefix('#') {
            Some(body) => (body.trim(), &mut commented),
            None => (line, &mut active),
        };
        if let Some(target) = workspace_import(body) {
            set.insert(target);
        }
    }
    Layer::ALL.map(|layer| {
        let rel = format!("{dir}/{}", layer.file_name());
        let state = if active.contains(&rel) {
            ImportState::Active
        } else if commented.contains(&rel) {
            ImportState::CommentedOut
        } else {
            ImportState::Absent
        };
        (layer, state)
    })
}

pub fn rendered_flag_lines(rendered: &str) -> BTreeSet<String> {
    rendered
        .lines()
        .filter_map(|line| {
            if line.is_empty() || line.starts_with('#') {
                None
            } else {
                Some(line.to_owned())
            }
        })
        .collect()
}

/// Lists root .bazelrc lines that exactly repeat a generated preset line.
pub fn owned_collisions_in_content(root_content: &str, rendered: &str) -> Vec<String> {
    let owned = rendered_flag_lines(rendered);
    let joined = root_content.replace("\\\r\n", "").replace("\\\n", "");
    let mut collisions = Vec::new();
    for raw in joined.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with("import ") || line.starts_with("try-import ") {
            continue;
        }
        if owned.contains(line) {
            collisions.push(line.to_owned());
        }
    }
    collisions.sort();
    collisions.dedup();
    collisions
}

/// The path of every layer file under `dir` in `workspace`, in import order.
pub fn preset_paths(workspace: &Path, dir: &str) -> Vec<(Layer, PathBuf)> {
    Layer::ALL
        .iter()
        .map(|layer| (*layer, workspace.join(dir).join(layer.file_name())))
        .collect()
}

pub fn source_dir(workspace: &Path) -> PathBuf {
    workspace.join(REPO_DIR)
}

pub fn bin_usage(err: &mut dyn std::io::Write) -> i32 {
    let _ = writeln!(err, "usage: preset.update [--verify-only]");
    1
}

pub fn bin_cannot(err: &mut dyn std::io::Write, error: impl std::fmt::Display) -> i32 {
    let _ = writeln!(err, "{error}");
    1
}

pub fn resolve_workspace() -> std::io::Result<PathBuf> {
    if let Ok(workspace) = std::env::var("BUILD_WORKSPACE_DIRECTORY") {
        if !workspace.is_empty() {
            return Ok(PathBuf::from(workspace));
        }
    }
    std::env::current_dir()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PresetError {
    OwnedCollision { lines: String },
    Stale { detail: String },
    Unwritable { path: String, detail: String },
}

impl std::fmt::Display for PresetError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OwnedCollision { lines } => write!(
                formatter,
                "preset.update: root .bazelrc duplicates preset lines; reconcile (remove owned duplicates, keep project overrides explicit): {lines}"
            ),
            Self::Stale { detail } => write!(formatter, "{detail}"),
            Self::Unwritable { path, detail } => {
                write!(formatter, "preset.update: cannot write {path}: {detail}")
            }
        }
    }
}

impl std::error::Error for PresetError {}

pub fn unified_diff(checked_in: &str, regenerated: &str) -> String {
    let old: Vec<&str> = checked_in.lines().collect();
    let new: Vec<&str> = regenerated.lines().collect();
    let mut out = vec!["--- checked-in".to_owned(), "+++ regenerated".to_owned()];
    let max = old.len().max(new.len());
    for i in 0..max {
        let old_line = old.get(i).copied().unwrap_or("");
        let new_line = new.get(i).copied().unwrap_or("");
        if old_line != new_line {
            if i < old.len() {
                out.push(format!("-{old_line}"));
            }
            if i < new.len() {
                out.push(format!("+{new_line}"));
            }
        }
    }
    out.join("\n")
}

fn read_collisions(root_path: &Path, rendered: &str) -> std::io::Result<Vec<String>> {
    match std::fs::read_to_string(root_path) {
        Ok(content) => Ok(owned_collisions_in_content(&content, rendered)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}

fn collision_error(collisions: &[String]) -> PresetError {
    PresetError::OwnedCollision {
        lines: collisions.join(", "),
    }
}

fn require_source_and_no_collisions(workspace: &Path) -> Result<(), PresetError> {
    let source = source_dir(workspace);
    if !source.is_dir() {
        return Err(PresetError::Unwritable {
            path: source.display().to_string(),
            detail: "source directory not found".to_owned(),
        });
    }
    let collisions =
        read_collisions(&workspace.join(".bazelrc"), &render_all()).map_err(|error| {
            PresetError::Unwritable {
                path: ".bazelrc".to_owned(),
                detail: error.to_string(),
            }
        })?;
    if collisions.is_empty() {
        Ok(())
    } else {
        Err(collision_error(&collisions))
    }
}

pub fn check_preset(workspace: &Path) -> Result<(), PresetError> {
    require_source_and_no_collisions(workspace)?;
    let mut stale = Vec::new();
    for (layer, path) in preset_paths(workspace, REPO_DIR) {
        let rendered = render_layer(layer);
        let checked_in = std::fs::read_to_string(&path).unwrap_or_default();
        if checked_in != rendered {
            let diff = unified_diff(&checked_in, &rendered);
            stale.push(format!(
                "preset.update: {REPO_DIR}/{} is stale; run the regen command\n{diff}",
                layer.file_name()
            ));
        }
    }
    if stale.is_empty() {
        Ok(())
    } else {
        Err(PresetError::Stale {
            detail: stale.join("\n"),
        })
    }
}

pub fn update_preset(workspace: &Path) -> Result<(), PresetError> {
    require_source_and_no_collisions(workspace)?;
    for (layer, path) in preset_paths(workspace, REPO_DIR) {
        let rendered = render_layer(layer);
        if std::fs::read_to_string(&path).ok().as_deref() == Some(rendered.as_str()) {
            continue;
        }
        std::fs::write(&path, rendered.as_bytes()).map_err(|error| PresetError::Unwritable {
            path: format!("{REPO_DIR}/{}", layer.file_name()),
            detail: error.to_string(),
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flag_sequence(rendered: &str) -> Vec<&str> {
        rendered
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect()
    }

    #[test]
    fn pins_track_canonical_versions() {
        assert_eq!(PRESET_BAZEL_VERSION, "9.2.0");
        assert_eq!(PRESET_DX_VERSION, "0.0.0");
    }

    #[test]
    fn layer_flags_keep_the_retired_python_bytes() {
        assert_eq!(
            flag_sequence(&render_layer(Layer::Correctness)),
            [
                "common --enable_bzlmod",
                "build --verbose_failures",
                "test --test_output=errors",
                "common --enable_platform_specific_config",
                "coverage --test_env=GENERATE_LLVM_LCOV=1",
                "coverage --combined_report=lcov",
                "coverage --test_tag_filters=-no-coverage",
                "coverage --enable_runfiles",
                "coverage:linux --test_env=COVERAGE_GCOV_PATH=/usr/bin/gcov",
                "coverage:macos --test_env=COVERAGE_GCOV_PATH=/usr/bin/gcov",
                "coverage --instrumentation_filter=^//",
                "common --nolegacy_external_runfiles",
                "build:windows --nobuild_runfile_links",
                "build:windows --enable_runfiles",
            ]
        );
        assert_eq!(
            flag_sequence(&render_layer(Layer::Performance)),
            [
                "build --host_compilation_mode=fastbuild",
                "build:dx_debug --compilation_mode=dbg",
                "build:dx_dev --compilation_mode=fastbuild",
                "build:dx_release --compilation_mode=opt",
                "build:dx_release --host_compilation_mode=opt",
                "build:dx_dev_remote --compilation_mode=fastbuild",
                "build:dx_toolchain --compilation_mode=fastbuild",
            ]
        );
        assert_eq!(
            flag_sequence(&render_layer(Layer::Resources)),
            [
                "startup --host_jvm_args=-XX:MaxHeapFreeRatio=30",
                "startup --host_jvm_args=-XX:MinHeapFreeRatio=10",
                "startup --host_jvm_args=-XX:MaxRAMPercentage=25",
                "common --local_resources=memory=HOST_RAM*.5",
                "common --local_resources=cpu=HOST_CPUS*.75",
                "startup --max_idle_secs=900",
                "startup --shutdown_on_low_sys_mem",
                "common --experimental_disk_cache_gc_max_age=14d",
                "common --experimental_disk_cache_gc_max_size=20G",
                "common --repo_contents_cache_gc_max_age=60d",
                "build --experimental_shrink_worker_pool",
                "build --experimental_total_worker_memory_limit_mb=4096",
            ]
        );
    }

    #[test]
    fn every_layer_starts_with_the_shared_header() {
        for layer in Layer::ALL {
            let rendered = render_layer(layer);
            let mut lines = rendered.lines();
            assert_eq!(
                lines.next(),
                Some("# Vendored Bazel execution preset -- GENERATED, do not edit.")
            );
            assert_eq!(
                lines.next(),
                Some("# Regenerate: `tools/dx init --apply`, or `bazel run //tools/bazelrc:preset_update` in rules_dx.")
            );
            assert_eq!(
                lines.next(),
                Some(format!("# Layer {}: {}", layer.name(), layer.summary()).as_str())
            );
            assert!(rendered.starts_with(HEADER_MARKER));
            assert!(rendered.ends_with('\n'));
            assert!(!rendered.ends_with("\n\n"));
            assert!(!rendered.contains('\r'));
            assert_eq!(layer.file_name(), format!("{}.bazelrc", layer.name()));
        }
    }

    #[test]
    fn every_flag_line_has_a_comment_directly_above() {
        for layer in Layer::ALL {
            let rendered = render_layer(layer);
            let lines: Vec<&str> = rendered.lines().collect();
            for (index, line) in lines.iter().enumerate() {
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let above = index
                    .checked_sub(1)
                    .and_then(|i| lines.get(i))
                    .copied()
                    .unwrap_or_default();
                assert!(above.starts_with("# "), "{layer:?}: {line} has no comment");
                let below = lines.get(index + 1).copied().unwrap_or_default();
                assert!(
                    !below.starts_with('#'),
                    "{layer:?}: {line} comment is below"
                );
            }
            for (_, comment) in layer.flags() {
                assert!(!comment.contains('\n'), "{comment}");
                assert!(!comment.contains(':'), "{comment}");
            }
        }
    }

    #[test]
    fn layers_partition_all_flags_in_original_order() {
        let mut seen = BTreeSet::new();
        let mut total = 0;
        for layer in Layer::ALL {
            for (flag, _) in layer.flags() {
                assert!(seen.insert(*flag), "{flag} is in two layers");
                total += 1;
            }
        }
        assert_eq!(total, 33);
        assert_eq!(rendered_flag_lines(&render_all()).len(), 33);
        let options = |layer: Layer| -> BTreeSet<String> {
            layer
                .flags()
                .map(|(flag, _)| {
                    let option = flag.split_once(' ').map_or(*flag, |(_, rest)| rest);
                    option.split('=').next().unwrap_or(option).to_owned()
                })
                .collect()
        };
        for (left, right) in [
            (Layer::Correctness, Layer::Performance),
            (Layer::Correctness, Layer::Resources),
            (Layer::Performance, Layer::Resources),
        ] {
            assert!(
                options(left).is_disjoint(&options(right)),
                "{left:?} and {right:?} set the same option"
            );
        }
        assert_eq!(
            render_all(),
            Layer::ALL.map(render_layer).concat(),
            "render_all keeps import order"
        );
    }

    #[test]
    fn imports_resolve_workspace_targets() {
        let text = "\u{feff}import %workspace%/a.bazelrc\r\n\
                    try-import \"%workspace%/b.bazelrc\"\n\
                    import \\\n  %workspace%/c.bazelrc\n\
                    # import %workspace%/d.bazelrc\n\
                    import /abs/e.bazelrc\n\
                    imports %workspace%/f.bazelrc\n\
                    build --x\n";
        assert_eq!(
            bazelrc_imports(text),
            vec!["a.bazelrc", "b.bazelrc", "c.bazelrc"]
        );
    }

    #[test]
    fn import_state_reads_active_commented_and_absent() {
        let lines: Vec<String> = Layer::ALL
            .iter()
            .map(|layer| import_line(CONSUMER_DIR, *layer))
            .collect();
        assert_eq!(
            lines[0],
            "import %workspace%/.dx/bazelrc/correctness.bazelrc"
        );
        let text = format!(
            "{}\r\n#  {}\nbuild --x\n",
            lines[0]
                .replace("import ", "try-import '")
                .replace(".bazelrc", ".bazelrc'"),
            lines[1]
        );
        assert_eq!(
            layer_import_state(&text, CONSUMER_DIR),
            [
                (Layer::Correctness, ImportState::Active),
                (Layer::Performance, ImportState::CommentedOut),
                (Layer::Resources, ImportState::Absent),
            ]
        );
        let continued = "import \\\n%workspace%/.dx/bazelrc/resources.bazelrc\n";
        assert_eq!(
            layer_import_state(continued, CONSUMER_DIR)[2],
            (Layer::Resources, ImportState::Active)
        );
        assert!(layer_import_state(&lines.join("\n"), REPO_DIR)
            .iter()
            .all(|(_, state)| *state == ImportState::Absent));
        let both = format!("# {}\n{}\n", lines[2], lines[2]);
        assert_eq!(
            layer_import_state(&both, CONSUMER_DIR)[2].1,
            ImportState::Active,
            "an active line wins over a commented-out one"
        );
    }

    #[test]
    fn flag_lines_skip_comments_and_blanks() {
        let flags = rendered_flag_lines(&render_all());
        assert!(flags.contains("common --enable_bzlmod"));
        assert!(flags.contains("build:dx_dev --compilation_mode=fastbuild"));
        assert!(flags.contains("build --host_compilation_mode=fastbuild"));
        assert!(flags.contains("common --nolegacy_external_runfiles"));
        assert_eq!(flags.len(), 33);
        assert!(!flags.iter().any(|line| line.starts_with('#')));
        assert!(!flags.iter().any(String::is_empty));
    }

    #[test]
    fn collisions_ignore_imports_comments_and_blanks() {
        let rendered = render_all();
        let root = "# comment\n\nimport %workspace%/tools/bazelrc/correctness.bazelrc\ntry-import %workspace%/user.bazelrc\nbuild --verbose_failures\n";
        let collisions = owned_collisions_in_content(root, &rendered);
        assert_eq!(collisions, vec!["build --verbose_failures".to_owned()]);
        let clean = "import %workspace%/tools/bazelrc/correctness.bazelrc\nbuild --@rules_rust//rust/settings:extra_rustc_flags=--deny=warnings\n";
        assert!(owned_collisions_in_content(clean, &rendered).is_empty());
        let blank = "# only\n\n";
        assert!(owned_collisions_in_content(blank, &rendered).is_empty());
    }

    #[test]
    fn collisions_are_sorted_and_deduped() {
        let rendered = render_all();
        let root =
            "test --test_output=errors\nbuild --verbose_failures\ntest --test_output=errors\n";
        let collisions = owned_collisions_in_content(root, &rendered);
        assert_eq!(
            collisions,
            vec![
                "build --verbose_failures".to_owned(),
                "test --test_output=errors".to_owned()
            ]
        );
    }

    #[test]
    fn collisions_match_whole_generated_lines_only() {
        let rendered = render_all();
        for root in [
            "build --verbose_failures # why\n",
            "build  --verbose_failures\n",
            "build --verbose_failures --keep_going\n",
            "build --verbose_failures=\"x\"\n",
            "# build --verbose_failures \\\nbuild --verbose_failures\n",
            "build --foo \\\nbuild --verbose_failures\n",
            "build --verbose_failures\\",
            "\\",
        ] {
            assert!(
                owned_collisions_in_content(root, &rendered).is_empty(),
                "{root:?}"
            );
        }
        for root in [
            "build --verbose_failures\r\n",
            "  build --verbose_failures  \n",
            "\tbuild --verbose_failures",
            "build \\\n--verbose_failures\n",
            "build \\\r\n--verbose_failures\r\n",
        ] {
            assert_eq!(
                owned_collisions_in_content(root, &rendered),
                vec!["build --verbose_failures".to_owned()],
                "{root:?}"
            );
        }
    }

    #[test]
    fn paths_join_workspace() {
        let workspace = Path::new("/tmp/ws");
        assert_eq!(
            preset_paths(workspace, REPO_DIR),
            vec![
                (
                    Layer::Correctness,
                    PathBuf::from("/tmp/ws/tools/bazelrc/correctness.bazelrc")
                ),
                (
                    Layer::Performance,
                    PathBuf::from("/tmp/ws/tools/bazelrc/performance.bazelrc")
                ),
                (
                    Layer::Resources,
                    PathBuf::from("/tmp/ws/tools/bazelrc/resources.bazelrc")
                ),
            ]
        );
        assert_eq!(
            preset_paths(workspace, CONSUMER_DIR)[2].1,
            Path::new("/tmp/ws/.dx/bazelrc/resources.bazelrc")
        );
        assert_eq!(source_dir(workspace), Path::new("/tmp/ws/tools/bazelrc"));
    }

    #[test]
    fn diff_marks_changed_lines() {
        let diff = unified_diff("a\nb\n", "a\nc\n");
        assert!(diff.contains("--- checked-in"));
        assert!(diff.contains("+++ regenerated"));
        assert!(diff.contains("-b"));
        assert!(diff.contains("+c"));
        let clean = unified_diff("same\n", "same\n");
        assert!(clean.contains("--- checked-in"));
        assert!(!clean.contains("-same"));
    }

    #[test]
    fn error_display_matches_python_messages() {
        let collision = PresetError::OwnedCollision {
            lines: "build --verbose_failures".to_owned(),
        };
        assert!(collision.to_string().contains("duplicates preset lines"));
        let stale = PresetError::Stale {
            detail: "preset.update: correctness.bazelrc is stale; run the regen command".to_owned(),
        };
        assert!(stale.to_string().contains("is stale"));
        let unwritable = PresetError::Unwritable {
            path: "tools/bazelrc/correctness.bazelrc".to_owned(),
            detail: "denied".to_owned(),
        };
        assert!(unwritable.to_string().contains("cannot write"));
    }

    fn repo_bazelrc() -> String {
        let mut text: String = Layer::ALL
            .iter()
            .map(|layer| format!("{}\n", import_line(REPO_DIR, *layer)))
            .collect();
        text.push_str("try-import %workspace%/user.bazelrc\n");
        text
    }

    #[test]
    fn check_and_update_round_trip_in_scratch() {
        let scratch = tempfile::TempDir::new().expect("scratch");
        let root = scratch.path();
        std::fs::create_dir_all(source_dir(root)).expect("source dir");
        std::fs::write(root.join(".bazelrc"), repo_bazelrc()).expect("bazelrc");
        assert!(matches!(check_preset(root), Err(PresetError::Stale { .. })));
        update_preset(root).expect("update creates every layer");
        assert_eq!(check_preset(root), Ok(()));
        for (layer, path) in preset_paths(root, REPO_DIR) {
            assert_eq!(
                std::fs::read_to_string(&path).expect("read"),
                render_layer(layer)
            );
        }
        let resources = &preset_paths(root, REPO_DIR)[2].1;
        std::fs::write(resources, "# dirty\n").expect("dirty");
        match check_preset(root) {
            Err(PresetError::Stale { detail }) => {
                assert!(detail.contains("tools/bazelrc/resources.bazelrc is stale"));
                assert!(!detail.contains("correctness.bazelrc is stale"));
                assert!(detail.contains("checked-in"));
            }
            other => panic!("want Stale, got {other:?}"),
        }
        update_preset(root).expect("update fixes dirty");
        assert_eq!(check_preset(root), Ok(()));
    }

    #[test]
    fn missing_source_dir_fails_closed() {
        let scratch = tempfile::TempDir::new().expect("scratch");
        let root = scratch.path();
        assert!(matches!(
            check_preset(root),
            Err(PresetError::Unwritable { .. })
        ));
        assert!(matches!(
            update_preset(root),
            Err(PresetError::Unwritable { .. })
        ));
    }

    #[test]
    fn duplicates_fail_closed_on_both_paths() {
        let scratch = tempfile::TempDir::new().expect("scratch");
        let root = scratch.path();
        std::fs::create_dir_all(source_dir(root)).expect("source dir");
        std::fs::write(root.join(".bazelrc"), repo_bazelrc()).expect("clean");
        update_preset(root).expect("update creates every layer");
        std::fs::write(
            root.join(".bazelrc"),
            format!("{}startup --max_idle_secs=900\n", repo_bazelrc()),
        )
        .expect("collision");
        assert!(matches!(
            check_preset(root),
            Err(PresetError::OwnedCollision { .. })
        ));
        assert!(matches!(
            update_preset(root),
            Err(PresetError::OwnedCollision { .. })
        ));
    }

    #[test]
    fn missing_root_bazelrc_means_no_collisions() {
        let scratch = tempfile::TempDir::new().expect("scratch");
        let root = scratch.path();
        std::fs::create_dir_all(source_dir(root)).expect("source dir");
        update_preset(root).expect("update without root");
        assert_eq!(check_preset(root), Ok(()));
    }
}
