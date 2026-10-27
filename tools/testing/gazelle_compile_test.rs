//! Compiles each Gazelle fixture package with only its generated deps.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const RESOLVE: &str = "# gazelle:resolve ";

fn var(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} is set"))
}

fn file(name: &str) -> PathBuf {
    dx_testing::resolve_runfiles(&var(name))
}

fn files(name: &str) -> Vec<PathBuf> {
    var(name)
        .split_whitespace()
        .map(dx_testing::resolve_runfiles)
        .collect()
}

fn fixture_dir() -> PathBuf {
    file("DX_FIXTURE")
        .parent()
        .expect("fixture directory")
        .to_path_buf()
}

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(var("TEST_TMPDIR")).join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("clear scratch directory");
    }
    std::fs::create_dir_all(&dir).expect("create scratch directory");
    dir
}

fn copy_tree(from: &Path, to: &Path) {
    for entry in std::fs::read_dir(from).expect("read fixture directory") {
        let entry = entry.expect("fixture entry");
        let source = entry.path();
        let name = entry.file_name();
        if std::fs::metadata(&source)
            .expect("fixture metadata")
            .is_dir()
        {
            let target = to.join(&name);
            std::fs::create_dir_all(&target).expect("create fixture directory");
            copy_tree(&source, &target);
        } else if name != "BUILD.out" {
            let target = if name == "BUILD.in" {
                to.join("BUILD.bazel")
            } else {
                to.join(&name)
            };
            let bytes = std::fs::read(&source).expect("read fixture file");
            std::fs::write(&target, bytes).expect("write fixture file");
        }
    }
}

fn packages(dir: &Path, rel: &str, out: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).expect("read fixture directory") {
        let entry = entry.expect("fixture entry");
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            let child = if rel.is_empty() {
                name
            } else {
                format!("{rel}/{name}")
            };
            packages(&path, &child, out);
        } else if name == "BUILD.out" {
            out.push(rel.to_owned());
        }
    }
    out.sort();
}

fn fixture_packages() -> Vec<String> {
    let mut out = Vec::new();
    packages(&fixture_dir(), "", &mut out);
    assert!(!out.is_empty(), "the fixture has packages with a BUILD.out");
    out
}

fn quoted(line: &str) -> impl Iterator<Item = String> + '_ {
    line.split('"').skip(1).step_by(2).map(str::to_owned)
}

fn build_file(root: &Path, package: &str) -> String {
    std::fs::read_to_string(root.join(package).join("BUILD.bazel"))
        .expect("read generated BUILD.bazel")
}

fn deps(root: &Path, package: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in build_file(root, package).lines().map(str::trim) {
        if let Some(rest) = line.strip_prefix("deps = [") {
            out.extend(quoted(rest));
            inside = !rest.trim_end_matches(',').ends_with(']');
        } else if inside {
            if line.starts_with(']') {
                inside = false;
            } else {
                out.extend(quoted(line));
            }
        }
    }
    out
}

fn resolves(root: &Path, package: &str) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for line in build_file(root, package).lines() {
        let Some(rest) = line.trim().strip_prefix(RESOLVE) else {
            continue;
        };
        let fields: Vec<&str> = rest.split_whitespace().collect();
        if let [_, import, label] = fields.as_slice() {
            out.entry((*label).to_owned())
                .or_default()
                .push((*import).to_owned());
        }
    }
    out
}

fn external() -> BTreeMap<String, Vec<PathBuf>> {
    var("DX_DEPS")
        .split_whitespace()
        .map(|pair| {
            let (label, jars) = pair
                .split_once('=')
                .expect("DX_DEPS pairs are label=jar,jar");
            let jars = jars.split(',').map(dx_testing::resolve_runfiles).collect();
            (label.to_owned(), jars)
        })
        .collect()
}

fn gazelle(root: &Path) -> dx_testing::Run {
    let repo_root = format!("-repo_root={}", root.display());
    let dir = root.to_string_lossy().into_owned();
    dx_testing::run(
        &file("DX_GAZELLE"),
        &[repo_root.as_str(), dir.as_str()],
        &[],
    )
    .expect("gazelle runs")
}

fn last_segment(name: &str) -> &str {
    name.rsplit(['.', '/', ':']).next().unwrap_or(name)
}

struct Fixture {
    root: PathBuf,
    external: BTreeMap<String, Vec<PathBuf>>,
}

impl Fixture {
    fn generate(name: &str) -> Self {
        let root = scratch(name);
        copy_tree(&fixture_dir(), &root);
        let run = gazelle(&root);
        assert!(run.status.success(), "{}", run.combined());
        Self {
            root,
            external: external(),
        }
    }

    fn sources(&self, package: &str) -> Vec<String> {
        let ext = var("DX_EXTENSION");
        let mut out: Vec<String> = std::fs::read_dir(self.root.join(package))
            .expect("read generated package")
            .map(|entry| entry.expect("package entry").path())
            .filter(|path| path.extension().is_some_and(|found| found == ext.as_str()))
            .map(|path| path.to_string_lossy().into_owned())
            .collect();
        out.sort();
        out
    }

    fn classpath(&self, package: &str, skip: Option<&str>) -> Vec<PathBuf> {
        let mut entries = files("DX_CLASSPATH");
        for dep in deps(&self.root, package) {
            if skip == Some(dep.as_str()) {
                continue;
            }
            if let Some(local) = dep.strip_prefix("//") {
                let local = local.split(':').next().expect("local package");
                let (run, classes) = self.compile(local, None);
                assert!(run.status.success(), "{dep}: {}", run.combined());
                entries.push(classes);
            } else {
                let jars = self
                    .external
                    .get(&dep)
                    .unwrap_or_else(|| panic!("{dep} has a jar in DX_DEPS"));
                entries.extend(jars.iter().cloned());
            }
        }
        entries
    }

    fn compile(&self, package: &str, skip: Option<&str>) -> (dx_testing::Run, PathBuf) {
        let tag: String = format!("{package}-{}", skip.unwrap_or("all"))
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        let classes = self.root.join("bazel-classes").join(tag);
        std::fs::create_dir_all(&classes).expect("create classes directory");
        let classpath = std::env::join_paths(self.classpath(package, skip))
            .expect("classpath entries join")
            .to_string_lossy()
            .into_owned();
        let out = classes.to_string_lossy().into_owned();
        let mut args: Vec<&str> = match var("DX_LANGUAGE").as_str() {
            "java" => vec!["-Xlint:all", "-Werror", "-proc:none"],
            "kotlin" => vec!["-Werror", "-no-stdlib", "-no-reflect"],
            "scala" => vec!["-usejavacp", "-Werror"],
            other => panic!("unknown DX_LANGUAGE {other}"),
        };
        args.extend(["-d", out.as_str()]);
        if !classpath.is_empty() {
            args.extend(["-classpath", classpath.as_str()]);
        }
        let sources = self.sources(package);
        assert!(!sources.is_empty(), "{package} has sources");
        args.extend(sources.iter().map(String::as_str));
        let run = dx_testing::run(&file("DX_COMPILER"), &args, &[]).expect("compiler runs");
        (run, classes)
    }

    fn imports(&self, package: &str) -> usize {
        self.sources(package)
            .iter()
            .map(|source| std::fs::read_to_string(source).expect("read source"))
            .map(|text| {
                text.lines()
                    .filter(|line| line.trim_start().starts_with("import "))
                    .count()
            })
            .sum()
    }
}

#[test]
fn generated_build_files_match_goldens() {
    let fixture = Fixture::generate("goldens");
    for package in fixture_packages() {
        let want = std::fs::read_to_string(fixture_dir().join(&package).join("BUILD.out"))
            .expect("read BUILD.out");
        let got = build_file(&fixture.root, &package);
        assert_eq!(got.trim(), want.trim(), "{package}/BUILD.bazel");
    }
}

#[test]
fn generated_deps_compile() {
    let fixture = Fixture::generate("complete");
    let all: Vec<String> = fixture_packages()
        .iter()
        .flat_map(|package| deps(&fixture.root, package))
        .collect();
    assert!(all.iter().any(|dep| dep.starts_with("//")), "{all:?}");
    assert!(all.iter().any(|dep| dep.starts_with('@')), "{all:?}");
    for package in fixture_packages() {
        let (run, _) = fixture.compile(&package, None);
        assert!(run.status.success(), "{package}: {}", run.combined());
    }
}

#[test]
fn each_missing_dep_fails_to_compile_and_names_the_symbol() {
    let fixture = Fixture::generate("missing");
    let mut checked = 0;
    for package in fixture_packages() {
        let mapped = resolves(&fixture.root, &package);
        for dep in deps(&fixture.root, &package) {
            let (run, _) = fixture.compile(&package, Some(&dep));
            let output = run.combined();
            assert!(!run.status.success(), "{package} compiled without {dep}");
            assert!(
                output.contains("error"),
                "{package} without {dep}: {output}"
            );
            let symbols: Vec<&str> = match mapped.get(&dep) {
                Some(imports) => imports.iter().map(|i| last_segment(i)).collect(),
                None => vec![last_segment(&dep)],
            };
            assert!(
                symbols.iter().any(|symbol| output.contains(symbol)),
                "{package} without {dep} does not name any of {symbols:?}: {output}"
            );
            checked += 1;
        }
    }
    assert!(checked > 0, "the fixture has generated deps");
}

#[test]
fn platform_only_package_has_no_deps_and_compiles_on_the_platform_classpath() {
    let fixture = Fixture::generate("platform");
    let platform: Vec<String> = fixture_packages()
        .into_iter()
        .filter(|package| deps(&fixture.root, package).is_empty() && fixture.imports(package) > 0)
        .collect();
    assert!(
        !platform.is_empty(),
        "the fixture has a package with platform imports and no deps"
    );
    for package in platform {
        let (run, _) = fixture.compile(&package, None);
        assert!(run.status.success(), "{package}: {}", run.combined());
    }
}

#[test]
fn each_removed_mapping_fails_as_unresolved() {
    let hint = var("DX_UNRESOLVED_HINT");
    let mut checked = 0;
    for package in fixture_packages() {
        let input = fixture_dir().join(&package).join("BUILD.in");
        let Ok(text) = std::fs::read_to_string(&input) else {
            continue;
        };
        let lines: Vec<&str> = text.lines().collect();
        for (index, line) in lines.iter().enumerate() {
            let Some(rest) = line.trim().strip_prefix(RESOLVE) else {
                continue;
            };
            let import = rest
                .split_whitespace()
                .nth(1)
                .expect("resolve names an import");
            let root = scratch(&format!("unresolved-{checked}"));
            copy_tree(&fixture_dir(), &root);
            let kept: Vec<&str> = lines
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != index)
                .map(|(_, line)| *line)
                .collect();
            std::fs::write(root.join(&package).join("BUILD.bazel"), kept.join("\n"))
                .expect("write BUILD.bazel");
            let run = gazelle(&root);
            let output = run.combined();
            assert!(!run.status.success(), "gazelle passed without {line}");
            let named = format!("unresolved import \"{import}");
            assert!(output.contains(&named), "without {line}: {output}");
            assert!(output.contains(&hint), "without {line}: {output}");
            checked += 1;
        }
    }
    assert!(checked > 0, "the fixture has # gazelle:resolve mappings");
}
