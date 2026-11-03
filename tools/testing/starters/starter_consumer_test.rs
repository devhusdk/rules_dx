//! Starts a project with `dx init`, adds one hello-world fixture, and builds and tests it as a
//! consumer of this checkout.

use std::path::Path;

use dx_nested as nested;

/// What one fixture checks beyond `bazel test //...`.
struct Starter {
    fixture: &'static str,
    build_only: bool,
    tests: &'static [&'static str],
    run: Option<(&'static str, &'static str)>,
    analyze: &'static [&'static str],
    sdk: Option<Sdk>,
    renamed_rules_dx: bool,
    compiler: Option<(&'static str, &'static str)>,
    host_compiler: Option<&'static str>,
    probes: Option<Probes>,
    format_check: Option<(&'static str, &'static [&'static str])>,
    broken: Option<(&'static str, &'static str)>,
}

/// Targets that build and test only when every variable in `env` is set.
struct Sdk {
    env: &'static [&'static str],
    flags: &'static [&'static str],
    build: &'static [&'static str],
    test: &'static [&'static str],
}

/// A `probe` package: its BUILD file and the sources whose build must pass or fail.
struct Probes {
    build: &'static str,
    files: &'static [(&'static str, &'static str, bool)],
}

const PLAIN: Starter = Starter {
    fixture: "",
    build_only: false,
    tests: &[],
    run: None,
    analyze: &[],
    sdk: None,
    renamed_rules_dx: false,
    compiler: None,
    host_compiler: None,
    probes: None,
    format_check: None,
    broken: None,
};

const C_PROBES: Probes = Probes {
    build: "load(\"@rules_dx//cc/rules:defs.bzl\", \"cc_library\")\n\ncc_library(\n    name = \"c_only\",\n    srcs = [\"c_only.c\"],\n)\n\ncc_library(\n    name = \"cxx_in_c\",\n    srcs = [\"cxx_in_c.c\"],\n)\n",
    files: &[
        (
            "c_only.c",
            "#include <stdlib.h>\n\nint class_slot(void) {\n  int class = 1;\n  int *slot = malloc(sizeof *slot);\n  if (slot == NULL) {\n    return 0;\n  }\n  *slot = class;\n  class = *slot;\n  free(slot);\n  return class;\n}\n",
            true,
        ),
        (
            "cxx_in_c.c",
            "#include <string>\n\nint length(void) { return 0; }\n",
            false,
        ),
    ],
};

const CPP_PROBES: Probes = Probes {
    build: "load(\"@rules_dx//cc/rules:defs.bzl\", \"cc_library\")\n\ncc_library(\n    name = \"cxx_only\",\n    srcs = [\"cxx_only.cc\"],\n)\n\ncc_library(\n    name = \"c_in_cxx\",\n    srcs = [\"c_in_cxx.cc\"],\n)\n",
    files: &[
        (
            "cxx_only.cc",
            "#include <string>\n\nstd::string greet(const std::string& name) { return \"hi \" + name; }\n",
            true,
        ),
        (
            "c_in_cxx.cc",
            "int class_slot() {\n  int class = 1;\n  return class;\n}\n",
            false,
        ),
    ],
};

const STARTERS: &[Starter] = &[
    Starter {
        fixture: "mdx",
        build_only: true,
        ..PLAIN
    },
    Starter {
        fixture: "c",
        tests: &["//:demo-app_test"],
        run: Some((":demo-app_bin", "hello world")),
        compiler: Some(("c", "clang")),
        probes: Some(C_PROBES),
        ..PLAIN
    },
    Starter {
        fixture: "cpp",
        tests: &["//:demo-app_test"],
        run: Some((":demo-app_bin", "hello world")),
        compiler: Some(("cc", "clang++")),
        probes: Some(CPP_PROBES),
        ..PLAIN
    },
    Starter {
        fixture: "cuda",
        tests: &["//:demo-app_test"],
        host_compiler: Some("cuda_host_llvm"),
        ..PLAIN
    },
    Starter {
        fixture: "elixir",
        tests: &["//:demo-app_test"],
        run: Some((":demo-app_bin", "hello world")),
        ..PLAIN
    },
    Starter {
        fixture: "erlang",
        tests: &["//:demo-app_test"],
        run: Some((":demo-app_bin", "hello world")),
        ..PLAIN
    },
    Starter {
        fixture: "haskell",
        tests: &["//:demo-app_test"],
        run: Some((":demo-app_bin", "hello")),
        broken: Some(("src/Hello.hs", "module Hello where\nthis is not haskell\n")),
        ..PLAIN
    },
    Starter {
        fixture: "julia",
        tests: &["//:DemoApp_test"],
        run: Some((":DemoApp_bin", "hello world")),
        format_check: Some((
            "@rules_dx//julia/tools:julia_format",
            &[
                "Project.toml",
                "src/DemoApp.jl",
                "main.jl",
                "test/runtests.jl",
            ],
        )),
        ..PLAIN
    },
    Starter {
        fixture: "rust-android",
        tests: &[
            "//:demo-app_test",
            "//:demo-app_fmt_test",
            "//:demo-app_clippy_test",
        ],
        analyze: &[":demo-app_start", ":demo-app_log"],
        sdk: Some(Sdk {
            env: &["ANDROID_HOME", "ANDROID_SDK_ROOT", "ANDROID_NDK_REVISION"],
            flags: &["--config=android"],
            build: &[":demo-app_apk_file", ":demo-app_install"],
            test: &[":demo-app_check_test"],
        }),
        renamed_rules_dx: true,
        ..PLAIN
    },
];

fn bazel_ok(root: &Path, args: &[&str]) -> String {
    let mut argv = vec![args[0], "--color=no", "--curses=no"];
    argv.extend_from_slice(&args[1..]);
    let run = nested::bazel_in(root, &argv);
    assert_eq!(
        run.status.code(),
        Some(0),
        "bazel {args:?}\n{}\n{}",
        run.stdout,
        run.stderr
    );
    run.stdout
}

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir {parent:?}: {error}"));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

fn sdk_present(sdk: &Sdk) -> bool {
    sdk.env
        .iter()
        .all(|var| std::env::var(var).is_ok_and(|value| !value.is_empty()))
}

fn assert_tests_and_runs(root: &Path, starter: &Starter) {
    let run = nested::bazel_in(root, &["test", "--color=no", "--curses=no", "//..."]);
    let text = format!("{}\n{}", run.stdout, run.stderr);
    assert_eq!(run.status.code(), Some(0), "{text}");
    assert!(
        text.lines()
            .any(|line| line.starts_with("//") && line.contains("PASSED")),
        "no test target passed:\n{text}"
    );
    for target in starter.tests {
        assert!(
            text.lines()
                .any(|line| line.starts_with(target) && line.contains("PASSED")),
            "{target} did not pass:\n{text}"
        );
    }

    if let Some((target, expected)) = starter.run {
        let out = bazel_ok(root, &["run", target]);
        assert!(out.contains(expected), "{target} printed {out:?}");
    }

    if !starter.analyze.is_empty() {
        let mut argv = vec!["build", "--nobuild"];
        argv.extend_from_slice(starter.analyze);
        bazel_ok(root, &argv);
    }

    if let Some(sdk) = &starter.sdk {
        if sdk_present(sdk) {
            let mut argv = vec!["build"];
            argv.extend_from_slice(sdk.flags);
            argv.extend_from_slice(sdk.build);
            bazel_ok(root, &argv);
            let mut argv = vec!["test"];
            argv.extend_from_slice(sdk.flags);
            argv.extend_from_slice(sdk.test);
            bazel_ok(root, &argv);
        }
    }
}

fn rename_rules_dx(root: &Path) {
    let module = read(&root.join("MODULE.bazel"));
    let dep = "bazel_dep(name = \"rules_dx\", version = ";
    let at = module.find(dep).expect("module depends on rules_dx");
    let end = at + module[at..].find(")\n").expect("bazel_dep closes");
    let renamed = format!("{}, repo_name = \"dxr\"{}", &module[..end], &module[end..])
        .replace("@rules_dx//", "@dxr//");
    write(&root.join("MODULE.bazel"), &renamed);
    let build = read(&root.join("BUILD.bazel"));
    assert!(build.contains("@rules_dx//"), "{build}");
    write(
        &root.join("BUILD.bazel"),
        &build.replace("@rules_dx//", "@dxr//"),
    );
}

fn assert_llvm_compiles(root: &Path, ext: &str, compiler: &str) {
    let out = bazel_ok(
        root,
        &[
            "aquery",
            "--output=text",
            "mnemonic(\"CppCompile\", deps(//:demo-app))",
        ],
    );
    let head = format!("action 'Compiling hello.{ext}'");
    let action = out
        .split("\n\n")
        .find(|action| action.contains(&head))
        .unwrap_or_else(|| panic!("no compile of hello.{ext}:\n{out}"));
    assert!(
        action.lines().any(|line| {
            let line = line.replace('\\', "/");
            let line = line.trim_end().trim_end_matches('\\').trim_end();
            let line = line.strip_suffix(".exe").unwrap_or(line);
            line.contains("external/llvm+") && line.ends_with(&format!("/bin/{compiler}"))
        }),
        "{action}"
    );
    if ext == "c" {
        assert!(!action.contains("-std=c++"), "{action}");
    }
}

fn assert_host_compiler(root: &Path, repo: &str) {
    let out = bazel_ok(
        root,
        &[
            "aquery",
            "--output=text",
            "mnemonic(\"CudaCompile\", //:demo-app)",
        ],
    );
    let ccbin = out
        .lines()
        .find(|line| line.contains("-ccbin"))
        .unwrap_or_else(|| panic!("no -ccbin in aquery\n{out}"));
    let after = ccbin.split("-ccbin").nth(1).unwrap_or_default();
    assert!(
        after.contains(repo),
        "-ccbin must come from {repo}: {ccbin}"
    );
}

fn assert_probes(root: &Path, probes: &Probes) {
    let dir = root.join("probe");
    write(&dir.join("BUILD.bazel"), probes.build);
    for (file, body, _) in probes.files {
        write(&dir.join(file), body);
    }
    for (file, _, builds) in probes.files {
        let stem = file.rsplit_once('.').map_or(*file, |(stem, _)| stem);
        let target = format!("//probe:{stem}");
        let run = nested::bazel_in(root, &["build", "--color=no", "--curses=no", &target]);
        let text = format!("{}{}", run.stdout, run.stderr);
        if *builds {
            assert_eq!(run.status.code(), Some(0), "{target}\n{text}");
        } else {
            assert_ne!(run.status.code(), Some(0), "{target} must fail\n{text}");
            let path = format!("probe/{file}:");
            assert!(
                run.stderr
                    .lines()
                    .any(|line| line.contains(&path) && line.contains("error:")),
                "{target} names {path}\n{text}"
            );
        }
    }
}

#[test]
fn starter_builds_and_tests_as_a_consumer() {
    let fixture = std::env::var("DX_FIXTURE").unwrap_or_else(|_| panic!("DX_FIXTURE must be set"));
    let starter = STARTERS
        .iter()
        .find(|starter| starter.fixture == fixture)
        .unwrap_or(&PLAIN);
    let case = nested::case(&format!("starter_{fixture}"));
    let root = nested::scaffold_consumer(case.root(), &fixture, "demo-app");
    if starter.build_only {
        bazel_ok(&root, &["build", "//..."]);
        return;
    }

    assert_tests_and_runs(&root, starter);

    if let Some((ext, compiler)) = starter.compiler {
        assert_llvm_compiles(&root, ext, compiler);
    }

    if let Some(repo) = starter.host_compiler {
        assert_host_compiler(&root, repo);
    }

    if let Some(probes) = &starter.probes {
        assert_probes(&root, probes);
    }

    if let Some((tool, files)) = starter.format_check {
        let paths: Vec<String> = files
            .iter()
            .map(|file| root.join(file).display().to_string())
            .collect();
        let mut argv = vec!["run", tool, "--", "--check"];
        argv.extend(paths.iter().map(String::as_str));
        bazel_ok(&root, &argv);
    }

    if let Some((file, body)) = starter.broken {
        std::fs::write(root.join(file), body)
            .unwrap_or_else(|error| panic!("write {file}: {error}"));
        let run = nested::bazel_in(&root, &["build", "--color=no", "--curses=no", "//..."]);
        assert_ne!(
            run.status.code(),
            Some(0),
            "broken {file} must fail the build\n{}\n{}",
            run.stdout,
            run.stderr
        );
    }

    if starter.renamed_rules_dx {
        rename_rules_dx(&root);
        assert_tests_and_runs(&root, starter);
    }
}
