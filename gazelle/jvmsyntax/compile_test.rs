use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

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
        if std::fs::metadata(&source).expect("fixture metadata").is_dir() {
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

fn quoted(line: &str) -> impl Iterator<Item = String> + '_ {
    line.split('"').skip(1).step_by(2).map(str::to_owned)
}

fn deps(root: &Path, package: &str) -> Vec<String> {
    let build = std::fs::read_to_string(root.join(package).join("BUILD.bazel"))
        .expect("read generated BUILD.bazel");
    let mut out = Vec::new();
    let mut inside = false;
    for line in build.lines().map(str::trim) {
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

fn external() -> BTreeMap<String, Vec<PathBuf>> {
    var("DX_DEPS")
        .split_whitespace()
        .map(|pair| {
            let (label, jars) = pair.split_once('=').expect("DX_DEPS pairs are label=jar,jar");
            let jars = jars.split(',').map(dx_testing::resolve_runfiles).collect();
            (label.to_owned(), jars)
        })
        .collect()
}

struct Fixture {
    root: PathBuf,
    external: BTreeMap<String, Vec<PathBuf>>,
}

impl Fixture {
    fn generate(name: &str) -> Self {
        let root = scratch(name);
        let fixture = file("DX_FIXTURE");
        copy_tree(fixture.parent().expect("fixture directory"), &root);
        let repo_root = format!("-repo_root={}", root.display());
        let dir = root.to_string_lossy().into_owned();
        let run = dx_testing::run(&file("DX_GAZELLE"), &[repo_root.as_str(), dir.as_str()], &[])
            .expect("gazelle runs");
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
        let suffix: String = skip
            .unwrap_or("all")
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        let classes = self.root.join(format!("classes-{package}-{suffix}"));
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
}

#[test]
fn generated_deps_compile() {
    let fixture = Fixture::generate("complete");
    let deps = deps(&fixture.root, "app");
    assert!(deps.iter().any(|dep| dep.starts_with("//")), "{deps:?}");
    assert!(deps.iter().any(|dep| dep.starts_with('@')), "{deps:?}");
    let (run, _) = fixture.compile("app", None);
    assert!(run.status.success(), "{}", run.combined());
}

#[test]
fn each_missing_dep_fails_to_compile() {
    let fixture = Fixture::generate("missing");
    let deps = deps(&fixture.root, "app");
    assert!(!deps.is_empty(), "app has generated deps");
    for dep in &deps {
        let (run, _) = fixture.compile("app", Some(dep));
        assert!(!run.status.success(), "app compiled without {dep}");
        assert!(
            run.combined().to_lowercase().contains("error"),
            "{dep}: {}",
            run.combined()
        );
    }
}
