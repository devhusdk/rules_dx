use super::*;

const FILES: [(&str, &str); 10] = [
    (
        "crate/Cargo.toml",
        "# top\n[package]\nname = \"a\"\nversion   =   \"1.2.3\" # keep\n\n[dependencies]\nversion = \"9\"\n\n[workspace.package]\nversion='1.2.3'\n",
    ),
    (
        "web/package.json",
        "{\n  \"name\": \"w\",\n  \"scripts\": {\"version\": \"x\"},\n\t\"version\" :  \"2.0.0\" ,\n  \"dependencies\": {}\n}\n",
    ),
    (
        "py/pyproject.toml",
        "[build-system]\nrequires = []\n\n[project]\nname = \"p\"\nversion = '0.4.0'\n\n[tool.poetry]\nversion = \"0.4.0\"\n",
    ),
    (
        "dart/pubspec.yaml",
        "name: d\n# version: 0.0.0\nversion:   3.1.0+4   # build\ndependencies:\n  version: 1\n",
    ),
    (
        "jvm/build.gradle.kts",
        "plugins {}\n    version = \"9.9.9\"\ngroup = \"g\"\nversion = \"1.0.0-SNAPSHOT\"\n",
    ),
    (
        "dotnet/App.csproj",
        "<Project>\n  <PropertyGroup>\n    <TargetFramework>net8.0</TargetFramework>\n    <Version>4.5.6</Version>\n  </PropertyGroup>\n</Project>\n",
    ),
    (
        "elixir/mix.exs",
        "defmodule M do\n  def project do\n    [app: :m, otp_version: \"1\", version: \"0.1.0\", deps: []]\n  end\nend\n",
    ),
    (
        "MODULE.bazel",
        "# header\nmodule(\n    name = \"m\",  # n\n    version   = \"5.0.0\",\n)\nbazel_dep(name = \"x\", version = \"1.0\")\n",
    ),
    (
        "gem/g.gemspec",
        "Gem::Specification.new do |s|\n  s.name = 'g'\n  s.version    = '6.0.1'\n  s.required_ruby_version = '>= 3'\nend\n",
    ),
    (
        "chart/Chart.yaml",
        "apiVersion: v2\nappVersion: \"1.0\"\nversion: \"7.0.0\"\n",
    ),
];

fn fixture() -> dx_test_scratch::TempDir {
    let dir = dx_test_scratch::scratch("own_version");
    for (path, text) in FILES {
        dx_test_scratch::write(dir.path(), path, text);
    }
    for skipped in [
        "node_modules/x/package.json",
        "target/Cargo.toml",
        "bazel-out/Cargo.toml",
        "third_party/t/Cargo.toml",
    ] {
        dx_test_scratch::write(
            dir.path(),
            skipped,
            "{\"version\": \"1.0.0\"}\n[package]\nversion = \"1.0.0\"\n",
        );
    }
    dx_test_scratch::write(
        dir.path(),
        "virtual/Cargo.toml",
        "[workspace]\nmembers = []\n",
    );
    dir
}

#[test]
fn find_lists_every_manifest_with_a_version_and_skips_vendored_trees() {
    let dir = fixture();
    let found = find(dir.path()).expect("find");
    let mut got: Vec<(String, String)> = found
        .iter()
        .map(|m| {
            let rel = m.path.strip_prefix(dir.path()).expect("inside");
            (rel.to_string_lossy().replace('\\', "/"), m.version.clone())
        })
        .collect();
    got.sort();
    let want = [
        ("MODULE.bazel", "5.0.0"),
        ("chart/Chart.yaml", "7.0.0"),
        ("crate/Cargo.toml", "1.2.3"),
        ("dart/pubspec.yaml", "3.1.0+4"),
        ("dotnet/App.csproj", "4.5.6"),
        ("elixir/mix.exs", "0.1.0"),
        ("gem/g.gemspec", "6.0.1"),
        ("jvm/build.gradle.kts", "1.0.0-SNAPSHOT"),
        ("py/pyproject.toml", "0.4.0"),
        ("web/package.json", "2.0.0"),
    ];
    let want: Vec<(String, String)> = want
        .iter()
        .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
        .collect();
    assert_eq!(got, want);
}

#[test]
fn set_changes_only_the_version_text() {
    let dir = fixture();
    for manifest in find(dir.path()).expect("find") {
        set(&manifest, "8.8.8").expect("set");
        let rel = manifest.path.strip_prefix(dir.path()).expect("inside");
        let rel = rel.to_string_lossy().replace('\\', "/");
        let before = FILES.iter().find(|(p, _)| *p == rel).expect("fixture").1;
        let after = fs::read_to_string(&manifest.path).expect("read");
        let old: Vec<_> = before.split_inclusive('\n').collect();
        let new: Vec<_> = after.split_inclusive('\n').collect();
        assert_eq!(old.len(), new.len(), "{rel}");
        let changed: Vec<_> = old.iter().zip(&new).filter(|(a, b)| a != b).collect();
        assert!(!changed.is_empty(), "{rel}");
        for (a, b) in changed {
            assert_eq!(a.replace(&manifest.version, "8.8.8"), **b, "{rel}");
        }
        let again = Kind::of(
            manifest
                .path
                .file_name()
                .expect("name")
                .to_str()
                .expect("utf8"),
        )
        .expect("kind")
        .spans(&after);
        assert_eq!(&after[again[0].clone()], "8.8.8", "{rel}");
    }
}

#[test]
fn cargo_bumps_package_and_workspace_package() {
    let out = Kind::Cargo.rewrite(FILES[0].1, "2.0.0").expect("version");
    assert_eq!(out.matches("2.0.0").count(), 2);
    assert!(out.contains("[dependencies]\nversion = \"9\""));
}

#[test]
fn manifest_without_a_version_is_not_rewritten() {
    assert_eq!(Kind::Cargo.rewrite("[workspace]\n", "1.0.0"), None);
    assert_eq!(Kind::PackageJson.rewrite("{\"version\": 1}", "1.0.0"), None);
    assert_eq!(
        Kind::ModuleBazel.rewrite("module(name = \"m\")\n", "1"),
        None
    );
}
