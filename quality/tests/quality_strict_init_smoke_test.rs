mod quality_check_smoke_common;

use dx_nested as nested;
use dx_test_scratch::scratch;
use dx_testing::{read, resolve_runfiles, rules_dx_source, write, Run};
use quality_check_smoke_common::{library, MODULE};

const ASSERTING: &str = "def check(value: int) -> int:\n    assert value > 0\n    return value\n";

fn dx(root: &nested::Case, args: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let home_dir = scratch("strict-init-home");
    let home = home_dir.path().to_string_lossy().into_owned();
    root.run(
        &resolve_runfiles(&rel),
        args,
        &[("HOME", home.as_str()), ("PATH", "/usr/bin:/bin")],
    )
}

fn lint_findings(root: &nested::Case) -> Run {
    let workspace = root.to_string_lossy().into_owned();
    dx(
        root,
        &[
            "lint",
            "--output=json",
            "--workspace",
            workspace.as_str(),
            "//app/...",
        ],
    )
}

#[test]
fn strict_init_enables_rules_the_root_file_can_disable() {
    let rules = rules_dx_source();
    let root = nested::case("strict-init");
    write(
        &root.join(".bazelversion"),
        &read(&rules.join(".bazelversion")),
    );
    write(
        &root.join("MODULE.bazel"),
        &MODULE.replace("RULES_DX", &rules.display().to_string()),
    );
    write(&root.join(".bazelrc"), "common --enable_bzlmod\n");
    write(
        &root.join("app/BUILD.bazel"),
        &format!(
            "load(\"@rules_dx//python/rules:defs.bzl\", \"python_library\")\n\n{}",
            library("app")
        ),
    );
    write(&root.join("app/app.py"), ASSERTING);

    let init = dx(&root, &["init", "--apply", "--profile=strict"]);
    assert_eq!(init.status.code(), Some(0), "{}", init.combined());
    assert!(root.join(".dx/config/BUILD.bazel").is_file());

    let strict = lint_findings(&root);
    assert_eq!(strict.status.code(), Some(1), "{}", strict.combined());
    assert!(strict.stdout.contains("S101"), "{}", strict.stdout);

    let ruff = read(&root.join("ruff.toml"));
    write(
        &root.join("ruff.toml"),
        &format!("{ruff}\n[lint]\nignore = [\"S101\"]\n"),
    );
    let relaxed = lint_findings(&root);
    assert_eq!(relaxed.status.code(), Some(0), "{}", relaxed.combined());
    assert!(!relaxed.stdout.contains("S101"), "{}", relaxed.stdout);
}
