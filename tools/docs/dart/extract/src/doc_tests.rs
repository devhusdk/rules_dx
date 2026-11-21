use super::*;

fn pairs(rows: &[(&str, &str)]) -> Vec<(String, String)> {
    rows.iter()
        .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
        .collect()
}

#[test]
fn sources_stage_under_the_longest_package_root() {
    let packages = pairs(&[("outer", "pkgs"), ("inner", "pkgs/inner")]);
    let srcs = pairs(&[
        ("pkgs/inner/lib/b.dart", "pkgs/inner/lib/b.dart"),
        ("pkgs/lib/a.dart", "pkgs/lib/a.dart"),
    ]);
    let plan = plan(&packages, &srcs, "outer", "//x:docs").unwrap();
    assert_eq!(
        plan.links,
        pairs(&[
            ("inner/lib/b.dart", "pkgs/inner/lib/b.dart"),
            ("outer/lib/a.dart", "pkgs/lib/a.dart"),
        ])
    );
}

#[test]
fn files_outside_lib_are_skipped_below_a_package_root() {
    let packages = pairs(&[("p", "p")]);
    let srcs = pairs(&[
        ("p/test/t.dart", "p/test/t.dart"),
        ("p/lib/a.dart", "p/lib/a.dart"),
    ]);
    let plan = plan(&packages, &srcs, "p", "//p:docs").unwrap();
    assert_eq!(plan.links, pairs(&[("p/lib/a.dart", "p/lib/a.dart")]));
}

#[test]
fn an_empty_root_stages_every_file() {
    let packages = pairs(&[("p", "")]);
    let srcs = pairs(&[("bin/x.dart", "bin/x.dart")]);
    let plan = plan(&packages, &srcs, "p", "//:docs").unwrap();
    assert_eq!(plan.links, pairs(&[("p/bin/x.dart", "bin/x.dart")]));
}

#[test]
fn an_external_source_matches_on_its_repo_relative_path() {
    let packages = pairs(&[("dep", "dep")]);
    let srcs = pairs(&[("external/r/dep/lib/d.dart", "../r/dep/lib/d.dart")]);
    let plan = plan(&packages, &srcs, "dep", "//:docs").unwrap();
    assert_eq!(
        plan.links,
        pairs(&[("dep/lib/d.dart", "external/r/dep/lib/d.dart")])
    );
}

#[test]
fn sources_are_staged_in_path_order() {
    let packages = pairs(&[("p", "")]);
    let srcs = pairs(&[("z.dart", "z.dart"), ("a.dart", "a.dart")]);
    let plan = plan(&packages, &srcs, "p", "//:docs").unwrap();
    assert_eq!(
        plan.links,
        pairs(&[("p/a.dart", "a.dart"), ("p/z.dart", "z.dart")])
    );
}

#[test]
fn two_sources_staging_as_one_path_fail() {
    let packages = pairs(&[("p", "a"), ("q", "b")]);
    let srcs = pairs(&[
        ("bazel-out/bin/a/lib/x.dart", "a/lib/x.dart"),
        ("a/lib/x.dart", "a/lib/x.dart"),
    ]);
    assert_eq!(
        plan(&packages, &srcs, "p", "//a:docs").unwrap_err(),
        "dart_doc_shard //a:docs: a/lib/x.dart and bazel-out/bin/a/lib/x.dart both stage as p/lib/x.dart"
    );
}

#[test]
fn a_later_root_replaces_the_first_and_keeps_its_position() {
    let packages = pairs(&[("own", "old"), ("dep", "d"), ("own", "new")]);
    let srcs = pairs(&[
        ("old/lib/o.dart", "old/lib/o.dart"),
        ("new/lib/n.dart", "new/lib/n.dart"),
    ]);
    let plan = plan(&packages, &srcs, "own", "//:docs").unwrap();
    assert_eq!(plan.links, pairs(&[("own/lib/n.dart", "new/lib/n.dart")]));
}

#[test]
fn pubspecs_and_package_config_name_every_package() {
    let packages = pairs(&[("b", "b"), ("a", "a")]);
    let plan = plan(&packages, &[], "b", "//:docs").unwrap();
    assert_eq!(
        plan.files,
        pairs(&[
            ("a/pubspec.yaml", "name: a\nenvironment:\n  sdk: ^3.0.0\n"),
            ("b/pubspec.yaml", "name: b\nenvironment:\n  sdk: ^3.0.0\n"),
            (
                "b/.dart_tool/package_config.json",
                "{\"configVersion\":2,\"packages\":[{\"name\":\"a\",\"rootUri\":\"../../a/\",\"packageUri\":\"lib/\",\"languageVersion\":\"3.0\"},{\"name\":\"b\",\"rootUri\":\"../\",\"packageUri\":\"lib/\",\"languageVersion\":\"3.0\"}]}\n",
            ),
        ])
    );
}

#[test]
fn parse_reads_every_flag() {
    let args: Vec<String> = [
        "--dart",
        "d",
        "--packages",
        "p",
        "--srcs",
        "s",
        "--own",
        "o",
        "--label",
        "l",
        "--scratch",
        "st",
        "--html",
        "h",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    let options = parse(&args).unwrap();
    assert_eq!(options.own, "o");
    assert_eq!(options.scratch, "st");
    assert!(parse(&args[2..]).is_err());
}

#[test]
fn empty_dir_keeps_the_directory_and_drops_its_entries() {
    let dir = std::env::temp_dir().join("dart_doc_empty_dir");
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(dir.join("stage/pkg/lib")).unwrap();
    fs::write(dir.join("stage/pkg/lib/a.dart"), "").unwrap();
    fs::create_dir_all(dir.join("pub")).unwrap();
    fs::write(dir.join(".dartServer"), "").unwrap();
    empty_dir(&dir).unwrap();
    assert!(dir.is_dir());
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
    fs::remove_dir(&dir).unwrap();
}
