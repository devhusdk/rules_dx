use std::path::PathBuf;

use dx_process::WORKFLOW_STARTUP_OPTS;
use dx_testing::resolve_runfiles;

fn launcher_path(name: &str) -> PathBuf {
    let rel = std::env::var(name).unwrap_or_else(|_| panic!("{name} must be set"));
    resolve_runfiles(&rel)
}

#[test]
fn dev_launchers_build_dx_from_the_checkout() {
    let startup = WORKFLOW_STARTUP_OPTS.join(" ");
    let sh = std::fs::read_to_string(launcher_path("DX_LAUNCHER_SH")).expect("tools/dx");
    let cmd = std::fs::read_to_string(launcher_path("DX_LAUNCHER_CMD"))
        .expect("tools/dx.cmd")
        .replace("\r\n", "\n")
        .replace('\n', "\r\n");
    assert!(sh.starts_with("#!/bin/sh\nset -eu\n"));
    assert!(sh.ends_with("\nexec \"$bin\" \"$@\"\n"));
    assert!(sh.contains(&format!("\n  rc=\"{startup}\"\n")));
    assert!(sh.contains("\n  [ \"${CI:-}\" = true ] || [ -n \"${BAZEL_TEST:-}\" ] || rc=\n"));
    assert!(sh.contains(
        "(cd \"$root\" && bazel $rc build ${dc:+\"$dc\"} $gc --curses=no --show_progress_rate_limit=30 --progress_report_interval=30 --show_result=0"
    ));
    assert!(sh.contains("\n    dc=\"--disk_cache=$cache/dx/disk-cache\"\n"));
    assert!(sh.contains(&format!(
        "|| gc=\"--experimental_disk_cache_gc_max_size={} --experimental_disk_cache_gc_max_age={}\"\n",
        dx_process::DISK_CACHE_GC_MAX_SIZE,
        dx_process::DISK_CACHE_GC_MAX_AGE
    )));
    assert!(cmd.contains(&format!(
        "$o+=' --experimental_disk_cache_gc_max_size={} --experimental_disk_cache_gc_max_age={}'",
        dx_process::DISK_CACHE_GC_MAX_SIZE,
        dx_process::DISK_CACHE_GC_MAX_AGE
    )));
    let query = sh
        .lines()
        .find(|line| line.contains(" query "))
        .expect("sh query line");
    assert!(query.contains("(cd \"$root\" && bazel $rc query --noshow_progress"));
    assert!(!query.contains("dc"), "{query}");
    assert_eq!(query.matches("deps(//:dx)").count(), 2, "{query}");
    assert!(!query.contains("//cli/cli:dx"), "{query}");
    assert!(
        sh.contains("--ui_event_filters=-info //:dx >&2"),
        "build target matches query"
    );
    assert!(sh
        .lines()
        .filter(|line| line.contains(" build "))
        .all(|line| !line.contains("--noshow_progress")));
    assert_eq!(sh.matches("CARGO_BAZEL_REPIN=1 bazel $rc build").count(), 1);
    assert_eq!(
        sh.matches("CARGO_BAZEL_REPIN").count(),
        2,
        "set only in the retry"
    );
    assert_eq!(cmd.matches("set \"CARGO_BAZEL_REPIN=1\"").count(), 1);
    // cmd expands %errorlevel% in a block before the block runs.
    let mut depth = 0i32;
    let mut fail_reads = 0;
    for line in cmd.lines() {
        let line = line.trim();
        if line.starts_with(')') {
            depth -= 1;
        }
        if line.contains("DX_FAIL=%errorlevel%") {
            fail_reads += 1;
            assert_eq!(depth, 0, "DX_FAIL read inside a block: {line}");
        }
        if line.ends_with('(') {
            depth += 1;
        }
    }
    assert_eq!(depth, 0, "unbalanced blocks in tools/dx.cmd");
    assert_eq!(fail_reads, 2, "the build and the repin retry");
    let retry = cmd
        .find("set \"CARGO_BAZEL_REPIN=1\"\r\ncall bazel %DX_RC% build ")
        .expect("cmd repin build");
    assert!(cmd[retry..].contains("\r\nendlocal & set \"DX_FAIL=%errorlevel%\"\r\n:dxbuilt\r\n"));
    assert!(sh.contains("run tools/dx update --apply cargo to repin"));
    assert!(cmd.contains("update --apply cargo to repin"));
    assert!(sh.contains("$root/.dx/launcher"));
    assert!(sh.contains("\"$state/inputs\""));
    assert!(!sh.contains("cmd.exe"));
    let hook = sh
        .find("\nif [ \"${1:-}\" = worktree ] && [ \"${2:-}\" = hook ]; then\n  [ -x \"$state/dx$exe\" ] && exec \"$state/dx$exe\" \"$@\"\n  [ -x \"$bin\" ] && exec \"$bin\" \"$@\"\nfi\n")
        .expect("sh hook early exec");
    let handoff = sh
        .find("\n  exec \"$here/tools/dx\" \"$@\"\n")
        .expect("sh hand-off to the current checkout");
    assert!(hook < handoff);
    assert!(handoff < sh.find("\nif stale; then\n").expect("stale check"));
    let hook = cmd
        .find("\r\nif /i \"%~1\"==\"worktree\" if /i \"%~2\"==\"hook\" if exist \"%DX_STATE%\\dx.exe\" set \"BIN=%DX_STATE%\\dx.exe\"\r\nif /i \"%~1\"==\"worktree\" if /i \"%~2\"==\"hook\" if exist \"%BIN%\" goto run\r\n")
        .expect("cmd hook early exec");
    let handoff = cmd
        .find("\r\n\"%DX_HERE%\\tools\\dx.cmd\" %*\r\n:dxself\r\n")
        .expect("cmd hand-off to the current checkout");
    assert!(hook < handoff);
    assert!(
        handoff
            < cmd
                .find("call powershell -NoProfile -NonInteractive -Command \"$s=")
                .expect("stale check")
    );
    assert!(cmd.starts_with("@echo off\r\nsetlocal\r\n"));
    assert!(cmd.contains(&format!("\r\nset \"DX_RC={startup}\"\r\n")));
    assert!(
        cmd.contains("\r\nif not \"%CI%\"==\"true\" if \"%BAZEL_TEST%\"==\"\" set \"DX_RC=\"\r\n")
    );
    assert!(
        cmd.contains("\ncall bazel %DX_RC% build %DX_DC% --curses=no --show_progress_rate_limit=30 --progress_report_interval=30 --show_result=0")
    );
    assert!(cmd.contains("\r\nif not defined DX_RC call powershell -NoProfile"));
    let query = cmd
        .lines()
        .find(|line| line.starts_with("call bazel %DX_RC% query "))
        .expect("cmd query line");
    assert!(query.contains("call bazel %DX_RC% query --noshow_progress"));
    assert!(!query.contains("DX_DC"), "{query}");
    assert_eq!(query.matches("deps(//:dx)").count(), 2, "{query}");
    assert!(!query.contains("//cli/cli:dx"), "{query}");
    assert!(
        cmd.contains("--ui_event_filters=-info //:dx 1>&2"),
        "build target matches query"
    );
    assert!(cmd
        .lines()
        .filter(|line| line.contains(" build "))
        .all(|line| !line.contains("--noshow_progress")));
    assert!(cmd.contains("\\.dx\\launcher"));
    assert!(cmd.contains("\\bazel-bin\\cli\\cli\\dx.exe"));
    assert!(!cmd.contains("if exist \"%~dp0..\\bazel-bin"));
    assert!(sh.contains("exec \"$state/dx$exe\" \"$@\""));
    assert!(cmd.contains("set \"BIN=%DX_STATE%\\dx.exe\""));
    assert!(sh.contains(
        "\nelif [ ! -x \"$state/dx$exe\" ] && [ -x \"$bin\" ]; then\n  guard_temps\n  keep_copy\nfi\n"
    ));
    assert_eq!(sh.matches("\n  keep_copy\n").count(), 2);
    for sig in ["INT", "TERM", "HUP"] {
        assert!(sh.contains(&format!("trap 'on_signal {sig}' {sig}\n")));
    }
    assert!(
        sh.find("\n  guard_temps\n  : > \"$state/built.$$\"\n")
            .is_some(),
        "guard_temps must come before the first temp file"
    );
    assert_eq!(cmd.matches("set \"DX_SYNC=").count(), 1);
    assert_eq!(cmd.matches("%DX_SYNC%").count(), 2);
    let sync = cmd
        .lines()
        .find(|line| line.starts_with("set \"DX_SYNC="))
        .expect("cmd sync snippet");
    assert!(sync.contains(".old."), "{sync}");
    assert!(sync.contains("[IO.File]::Copy($env:BIN, $n)"), "{sync}");
    for bad in ['%', '!', '^', '&'] {
        assert!(!sync.contains(bad), "{sync}");
    }
    assert_eq!(sync.matches('"').count(), 2, "{sync}");
    assert!(!cmd.contains("copy /y \"%BIN%\""));
    assert!(!cmd
        .lines()
        .any(|line| line.starts_with("move /y") && line.contains("\\dx.exe\"")));
    assert_eq!(cmd.matches("\r\n:dxbuild\r\n").count(), 1);
    assert!(cmd.contains("\r\nif errorlevel 2 goto run\r\nif errorlevel 1 goto dxbuild\r\nset \"BIN=%DX_STATE%\\dx.exe\"\r\ngoto run\r\n:dxbuild\r\n"));
    assert!(cmd.contains("\r\nif defined DX_COPY set \"BIN=%DX_STATE%\\dx.exe\"\r\n:run\r\n"));
    assert_eq!(cmd.matches("\r\n:run\r\n").count(), 1);
    assert!(cmd.contains("\r\n:run\r\n\"%BIN%\" %*\r\nexit /b %errorlevel%\r\n"));
    assert!(!cmd.contains(":pinned"));
    assert!(!cmd.contains("goto bazel"));
    assert!(!sh.contains("fail "));
    assert!(!sh.contains("|| die \"bazel query"));
    let after_query = cmd
        .split("query --noshow_progress")
        .nth(1)
        .expect("query line");
    let before_run = after_query.split("\r\n:run\r\n").next().expect("run label");
    assert!(!before_run.contains("exit /b 1"), "{before_run}");
    for script in [&sh, &cmd] {
        assert!(script.contains("dx: building //:dx"));
        assert!(script.contains("dx: //:dx does not build; running the previous dx"));
        assert!(script.contains("bazel build //:dx failed"));
        assert!(script.contains(
            "dx: bazel query for the inputs of //:dx failed; running the fresh dx anyway"
        ));
        for gone in [
            "bazel run @rules_dx",
            "VERSION=",
            "releases/download",
            ".dx/version",
            ".dx\\version",
        ] {
            assert!(!script.contains(gone), "{gone}");
        }
    }
}

use dx_process::RcPolicy;

#[derive(Clone, Copy)]
enum Rc {
    System,
    Workspace,
    Home,
    Extra(&'static str),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Expect {
    Default,
    Xdg,
    Unset,
}

struct RcCase {
    name: &'static str,
    env: &'static [(&'static str, &'static str)],
    files: &'static [(Rc, &'static str)],
    expect: Expect,
    rc_gc: bool,
}

const fn case(
    name: &'static str,
    env: &'static [(&'static str, &'static str)],
    files: &'static [(Rc, &'static str)],
    expect: Expect,
) -> RcCase {
    RcCase {
        name,
        env,
        files,
        expect,
        rc_gc: false,
    }
}

const WS_USER: &str = "try-import %workspace%/user.bazelrc\n";

const RC_CASES: &[RcCase] = &[
    case("no rc files", &[], &[], Expect::Default),
    case("XDG_CACHE_HOME", &[("XDG_CACHE_HOME", "{xdg}")], &[], Expect::Xdg),
    case("empty XDG_CACHE_HOME", &[("XDG_CACHE_HOME", "")], &[], Expect::Default),
    case("home rc", &[], &[(Rc::Home, "build --disk_cache=/x\n")], Expect::Unset),
    case(
        "try-import",
        &[],
        &[
            (Rc::Workspace, WS_USER),
            (Rc::Extra("user.bazelrc"), "common --nodisk_cache\n"),
        ],
        Expect::Unset,
    ),
    case("comment", &[], &[(Rc::Home, "# build --disk_cache=/x\n")], Expect::Default),
    case("quoted", &[], &[(Rc::Home, "build \"--disk_cache=/x\"\n")], Expect::Unset),
    case(
        "single-quoted",
        &[],
        &[(Rc::Home, "build '--disk_cache=/x'\n")],
        Expect::Unset,
    ),
    case("split form", &[], &[(Rc::Home, "build --disk_cache /x\n")], Expect::Unset),
    case(
        "config line",
        &[],
        &[(Rc::Home, "build:cfg --disk_cache=/x\n")],
        Expect::Unset,
    ),
    RcCase {
        rc_gc: true,
        ..case(
            "rc gc bound",
            &[],
            &[(Rc::Home, "common --experimental_disk_cache_gc_max_size=40G\n")],
            Expect::Default,
        )
    },
    RcCase {
        rc_gc: true,
        ..case(
            "imported gc bound",
            &[],
            &[
                (Rc::Workspace, WS_USER),
                (
                    Rc::Extra("user.bazelrc"),
                    "build:x \"--experimental_disk_cache_gc_max_age=3d\"\n",
                ),
            ],
            Expect::Default,
        )
    },
    case(
        "longer gc flag",
        &[],
        &[(Rc::Home, "common --experimental_disk_cache_gc_max_size_x=1\n")],
        Expect::Default,
    ),
    case(
        "import",
        &[],
        &[
            (Rc::Workspace, "import %workspace%/user.bazelrc\n"),
            (Rc::Extra("user.bazelrc"), "build --disk_cache=/x\n"),
        ],
        Expect::Unset,
    ),
    case(
        "relative import",
        &[],
        &[
            (Rc::Workspace, "import user.bazelrc\n"),
            (Rc::Extra("user.bazelrc"), "build --disk_cache=/x\n"),
        ],
        Expect::Unset,
    ),
    case(
        "quoted import",
        &[],
        &[
            (Rc::Workspace, "try-import \"%workspace%/user.bazelrc\"\n"),
            (Rc::Extra("user.bazelrc"), "build --disk_cache=/x\n"),
        ],
        Expect::Unset,
    ),
    case(
        "missing try-import",
        &[],
        &[(Rc::Workspace, "try-import %workspace%/missing.bazelrc\n")],
        Expect::Default,
    ),
    case(
        "nested import",
        &[],
        &[
            (Rc::Workspace, "import %workspace%/a.bazelrc\n"),
            (Rc::Extra("a.bazelrc"), "import %workspace%/b.bazelrc\n"),
            (Rc::Extra("b.bazelrc"), "build --disk_cache=/x\n"),
        ],
        Expect::Unset,
    ),
    case(
        "import cycle",
        &[],
        &[
            (Rc::Workspace, "import %workspace%/a.bazelrc\n"),
            (Rc::Extra("a.bazelrc"), "import %workspace%/b.bazelrc\n"),
            (Rc::Extra("b.bazelrc"), "import %workspace%/a.bazelrc\n"),
        ],
        Expect::Default,
    ),
    case("CI", &[("CI", "true")], &[], Expect::Unset),
    case("BAZEL_TEST", &[("BAZEL_TEST", "1")], &[], Expect::Unset),
    RcCase {
        ..case(
            "ProgramData rc",
            &[],
            &[(Rc::System, "build --disk_cache=C:\\x\n")],
            Expect::Unset,
        )
    },
    RcCase {
        rc_gc: true,
        ..case(
            "ProgramData gc bound",
            &[],
            &[(Rc::System, "common --experimental_disk_cache_gc_max_age=3d\n")],
            Expect::Default,
        )
    },
];

fn rc_names() -> Vec<&'static str> {
    let mut names: Vec<&str> = RC_CASES
        .iter()
        .flat_map(|rc| rc.files.iter())
        .filter_map(|(slot, _)| match slot {
            Rc::Extra(name) => Some(*name),
            _ => None,
        })
        .collect();
    names.sort_unstable();
    names.dedup();
    names
}

fn policy_flags(policy: &RcPolicy) -> Vec<String> {
    policy
        .verb_options("build", None, &[])
        .into_iter()
        .filter(|flag| !flag.starts_with("--experimental_disk_cache_gc_idle_delay"))
        .collect()
}

#[cfg(unix)]
mod sh {
    use super::*;
    use dx_testing::mkscratch;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::process::Command;

    const PAYLOAD: &str = "#!/bin/sh\necho ran \"$@\"\n";

    fn spawn(cmd: &mut Command) -> std::process::Child {
        cmd.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let mut tries = 0;
        loop {
            match cmd.spawn() {
                Err(e) if e.kind() == std::io::ErrorKind::ExecutableFileBusy && tries < 50 => {
                    tries += 1;
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                other => return other.expect("launcher starts"),
            }
        }
    }

    struct SourceRepo {
        base: std::path::PathBuf,
    }

    fn checkout(root: &Path) {
        std::fs::create_dir_all(root.join("cli/cli")).expect("cli dir");
        std::fs::create_dir_all(root.join("tools/bazelrc")).expect("bazelrc dir");
        std::fs::write(
            root.join("MODULE.bazel"),
            "module(\n    name = \"rules_dx\",\n)\n",
        )
        .expect("module");
        std::fs::write(root.join("cli/cli/BUILD.bazel"), "").expect("build file");
        std::fs::write(root.join("BUILD.bazel"), "").expect("root build file");
        std::fs::write(root.join("tools/bazelrc/resources.bazelrc"), "").expect("preset");
        let launcher = root.join("tools/dx");
        std::fs::copy(launcher_path("DX_LAUNCHER_SH"), &launcher).expect("launcher");
        std::fs::set_permissions(&launcher, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }

    fn source_repo(name: &str) -> SourceRepo {
        let base = mkscratch(name).expect("scratch");
        let root = base.join("repo");
        checkout(&root);
        std::fs::create_dir_all(base.join("stub")).expect("stub dir");
        std::fs::create_dir_all(base.join("home")).expect("home");
        std::fs::write(base.join("payload"), PAYLOAD).expect("payload");
        let stub = base.join("stub/bazel");
        std::fs::write(
            &stub,
            "#!/bin/sh\n[ -z \"${STUB_ARGS:-}\" ] || { printf '%s\\t' \"$@\"; echo; } >> \"$STUB_ARGS\"\nfor a in \"$@\"; do\n  case \"$a\" in\n    build)\n      echo build >> \"$STUB_LOG\"\n      if [ -n \"${STUB_BUILD_HOLD:-}\" ]; then\n        : > \"$STUB_BUILD_HOLD.started\"\n        tries=0\n        while [ ! -e \"$STUB_BUILD_HOLD\" ]; do\n          tries=$((tries + 1))\n          if [ \"$tries\" -gt 600 ]; then echo 'stub bazel: hold timeout' >&2; exit 3; fi\n          sleep 0.05\n        done\n      fi\n      if [ -n \"${STUB_BARRIER:-}\" ]; then\n        : > \"$STUB_BARRIER/$$\"\n        tries=0\n        while [ \"$(ls \"$STUB_BARRIER\" | wc -l)\" -lt \"$STUB_BARRIER_N\" ]; do\n          tries=$((tries + 1))\n          if [ \"$tries\" -gt 600 ]; then echo 'stub bazel: barrier timeout' >&2; exit 3; fi\n          sleep 0.05\n        done\n      else\n        sleep 1\n      fi\n      mkdir -p bazel-bin/cli/cli\n      src=\"$STUB_PAYLOAD\"\n      [ -f payload ] && src=payload\n      cp \"$src\" \"bazel-bin/cli/cli/dx.$$\"\n      chmod +x \"bazel-bin/cli/cli/dx.$$\"\n      mv \"bazel-bin/cli/cli/dx.$$\" bazel-bin/cli/cli/dx\n      exit 0\n      ;;\n    query)\n      [ ! -f \"$STUB_QUERY_FAIL\" ] || { echo 'Server shut down explicitly' >&2; exit 37; }\n      case \"$*\" in *'deps(//:dx)'*) echo //:BUILD.bazel;; esac\n      echo //cli/cli:BUILD.bazel\n      sleep 1\n      echo @rules_rust//rust:defs.bzl\n      exit 0\n      ;;\n  esac\ndone\nexit 2\n",
        )
        .expect("bazel stub");
        std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        SourceRepo { base }
    }

    impl SourceRepo {
        fn root(&self) -> std::path::PathBuf {
            self.base.join("repo")
        }

        fn command(&self) -> Command {
            self.command_in(&self.root(), &["status"])
        }

        fn command_in(&self, cwd: &Path, args: &[&str]) -> Command {
            let base = &self.base;
            let mut cmd = Command::new(base.join("repo/tools/dx"));
            cmd.args(args)
                .current_dir(cwd)
                .env_clear()
                .env(
                    "PATH",
                    format!("{}:/usr/bin:/bin", base.join("stub").display()),
                )
                .env("HOME", base.join("home"))
                .env("STUB_LOG", base.join("calls"))
                .env("STUB_PAYLOAD", base.join("payload"))
                .env("STUB_QUERY_FAIL", base.join("query-fails"));
            cmd
        }

        fn spawn(&self) -> std::process::Child {
            spawn(&mut self.command())
        }

        fn run_ok(&self) {
            let out = self.spawn().wait_with_output().expect("launcher runs");
            assert_eq!(
                (
                    out.status.code(),
                    String::from_utf8_lossy(&out.stdout).into_owned()
                ),
                (Some(0), "ran status\n".to_owned()),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
        }

        fn run_in(&self, cwd: &Path, args: &[&str]) -> (Option<i32>, String, String) {
            let out = spawn(&mut self.command_in(cwd, args))
                .wait_with_output()
                .expect("launcher runs");
            (
                out.status.code(),
                String::from_utf8_lossy(&out.stdout).into_owned(),
                String::from_utf8_lossy(&out.stderr).into_owned(),
            )
        }

        fn builds(&self) -> usize {
            std::fs::read_to_string(self.base.join("calls"))
                .map_or(0, |text| text.lines().filter(|l| *l == "build").count())
        }

        fn inputs(&self) -> String {
            std::fs::read_to_string(self.root().join(".dx/launcher/inputs")).expect("inputs")
        }

        fn state_files(&self) -> Vec<String> {
            let mut names: Vec<String> = std::fs::read_dir(self.root().join(".dx/launcher"))
                .expect("state dir")
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();
            names
        }

        fn touch_build_file(&self) {
            let file = std::fs::File::options()
                .write(true)
                .open(self.root().join("cli/cli/BUILD.bazel"))
                .expect("open build file");
            file.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(3600))
                .expect("touch build file");
        }

        fn touch_root_build_file(&self) {
            let file = std::fs::File::options()
                .write(true)
                .open(self.root().join("BUILD.bazel"))
                .expect("open root build file");
            file.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(3600))
                .expect("touch root build file");
        }

        fn break_build(&self) {
            let stub = self.base.join("stub/bazel");
            std::fs::write(
                &stub,
                "#!/bin/sh\nfor a in \"$@\"; do\n  case \"$a\" in\n    build)\n      echo build >> \"$STUB_LOG\"\n      rm -f bazel-bin/cli/cli/dx\n      exit 1\n      ;;\n  esac\ndone\nexit 2\n",
            )
            .expect("failing bazel stub");
            std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        }

        /// Makes the first `times` runs of bazel `op` ("build" or "query") exit with `code`.
        fn crash(&self, op: &str, code: i32, times: usize) {
            let stub = self.base.join("stub/bazel");
            let real = self.base.join("stub/bazel.real");
            if !real.exists() {
                std::fs::rename(&stub, &real).expect("keep the real stub");
            }
            let left = self.base.join(format!("crash-{op}"));
            std::fs::write(&left, times.to_string()).expect("crash counter");
            let log = if op == "build" {
                "echo build >> \"$STUB_LOG\"\n      "
            } else {
                ""
            };
            std::fs::write(
                &stub,
                format!(
                    "#!/bin/sh\nfor a in \"$@\"; do\n  case \"$a\" in\n    {op})\n      n=$(cat '{left}')\n      if [ \"$n\" -gt 0 ]; then\n        echo $((n - 1)) > '{left}'\n        {log}exit {code}\n      fi\n      ;;\n  esac\ndone\nexec '{real}' \"$@\"\n",
                    left = left.display(),
                    real = real.display(),
                ),
            )
            .expect("crashing bazel stub");
            std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        }

        fn repin_gated(&self) {
            let stub = self.base.join("stub/bazel");
            std::fs::write(
                &stub,
                "#!/bin/sh\nfor a in \"$@\"; do\n  case \"$a\" in\n    build)\n      echo build >> \"$STUB_LOG\"\n      if [ \"${CARGO_BAZEL_REPIN-}\" != 1 ]; then rm -f bazel-bin/cli/cli/dx; exit 1; fi\n      echo repin >> \"$STUB_LOG\"\n      mkdir -p bazel-bin/cli/cli\n      cp \"$STUB_PAYLOAD\" bazel-bin/cli/cli/dx.$$\n      chmod +x bazel-bin/cli/cli/dx.$$\n      mv bazel-bin/cli/cli/dx.$$ bazel-bin/cli/cli/dx\n      exit 0\n      ;;\n    query)\n      echo //cli/cli:BUILD.bazel\n      exit 0\n      ;;\n  esac\ndone\nexit 2\n",
            )
            .expect("gated bazel stub");
            std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).expect("chmod");
            std::fs::write(
                self.base.join("payload"),
                "#!/bin/sh\necho ran \"$@\" ${CARGO_BAZEL_REPIN-unset}\n",
            )
            .expect("payload");
        }

        fn repins(&self) -> usize {
            std::fs::read_to_string(self.base.join("calls"))
                .map_or(0, |text| text.lines().filter(|l| *l == "repin").count())
        }

        fn run(&self) -> (Option<i32>, String, String) {
            let out = self.spawn().wait_with_output().expect("launcher runs");
            (
                out.status.code(),
                String::from_utf8_lossy(&out.stdout).into_owned(),
                String::from_utf8_lossy(&out.stderr).into_owned(),
            )
        }
    }

    #[test]
    fn source_launcher_runs_the_previous_dx_when_dx_does_not_build() {
        let repo = source_repo("dx-launcher-fallback-");
        repo.run_ok();
        assert!(repo.root().join(".dx/launcher/dx").exists());
        repo.break_build();
        repo.touch_build_file();
        let (code, out, err) = repo.run();
        assert_eq!((code, out.as_str()), (Some(0), "ran status\n"), "{err}");
        assert!(err.contains("dx: building //:dx\n"), "{err}");
        assert!(
            err.contains("dx: //:dx does not build; running the previous dx\n"),
            "{err}"
        );
        assert!(!err.contains("bazel run @rules_dx"), "{err}");
        assert!(!repo.root().join("bazel-bin/cli/cli/dx").exists());
        assert_eq!(repo.builds(), 2);
        assert_eq!(
            repo.state_files(),
            vec!["CACHEDIR.TAG", "built", "dx", "inputs"]
        );
        let (code, out, err) = repo.run();
        assert_eq!((code, out.as_str()), (Some(0), "ran status\n"), "{err}");
        assert_eq!(repo.builds(), 3, "a failed build is tried again");
    }

    #[test]
    fn source_launcher_rebuilds_after_the_root_build_file_changes() {
        let repo = source_repo("dx-launcher-root-build-");
        repo.run_ok();
        assert_eq!(repo.builds(), 1);
        assert!(
            repo.inputs().contains("//:BUILD.bazel"),
            "{}",
            repo.inputs()
        );
        repo.touch_root_build_file();
        repo.run_ok();
        assert_eq!(repo.builds(), 2);
    }

    const CRASH_NOTICE: &str = "dx: the Bazel server crashed (exit";

    #[test]
    fn source_launcher_builds_again_after_a_server_crash() {
        for code in [33, 37] {
            let repo = source_repo("dx-launcher-crash-once-");
            repo.crash("build", code, 1);
            let (exit, out, err) = repo.run();
            assert_eq!((exit, out.as_str()), (Some(0), "ran status\n"), "{err}");
            assert!(
                err.contains(&format!("{CRASH_NOTICE} {code}); trying //:dx again\n")),
                "{err}"
            );
            assert_eq!(repo.builds(), 2);
            assert_eq!(repo.state_files(), vec!["built", "dx", "inputs"]);
        }
    }

    #[test]
    fn source_launcher_retries_a_server_crash_once() {
        let repo = source_repo("dx-launcher-crash-twice-");
        repo.crash("build", 37, 10);
        let (exit, out, err) = repo.run();
        assert_eq!((exit, out.as_str()), (Some(1), ""), "{err}");
        assert!(err.contains("dx: bazel build //:dx failed\n"), "{err}");
        assert_eq!(repo.builds(), 2);

        let repo = source_repo("dx-launcher-crash-twice-copy-");
        repo.run_ok();
        repo.crash("build", 33, 10);
        repo.touch_build_file();
        let (exit, out, err) = repo.run();
        assert_eq!((exit, out.as_str()), (Some(0), "ran status\n"), "{err}");
        assert!(
            err.contains("dx: //:dx does not build; running the previous dx\n"),
            "{err}"
        );
        assert_eq!(repo.builds(), 3, "one build before, two after");
    }

    #[test]
    fn source_launcher_queries_again_after_a_server_crash() {
        let repo = source_repo("dx-launcher-query-crash-");
        repo.crash("query", 37, 1);
        let (exit, out, err) = repo.run();
        assert_eq!((exit, out.as_str()), (Some(0), "ran status\n"), "{err}");
        assert!(err.contains(CRASH_NOTICE), "{err}");
        assert!(!repo.inputs().contains("dx:"), "{}", repo.inputs());
        assert_eq!(repo.state_files(), vec!["built", "dx", "inputs"]);
    }

    #[test]
    fn source_launcher_tags_its_state_as_a_cache() {
        let repo = source_repo("dx-launcher-tag-");
        repo.run_ok();
        let tag =
            std::fs::read_to_string(repo.root().join(".dx/launcher/CACHEDIR.TAG")).expect("tag");
        assert!(
            tag.starts_with("Signature: 8a477f597d28d172789f06886806bc55\n"),
            "{tag}"
        );
    }

    #[test]
    fn the_tag_is_added_to_an_existing_untagged_copy() {
        let repo = source_repo("dx-launcher-late-tag-");
        repo.run_ok();
        let tag = repo.root().join(".dx/launcher/CACHEDIR.TAG");
        std::fs::remove_file(&tag).expect("remove the tag");
        repo.run_ok();
        assert_eq!(repo.builds(), 1, "tagging does not rebuild");
        assert!(tag.exists());
    }

    #[test]
    fn update_apply_cargo_repins_when_dx_does_not_build() {
        for args in [
            &["update", "--apply", "cargo"][..],
            &["update", "--apply"][..],
        ] {
            let repo = source_repo("dx-launcher-repin-");
            repo.repin_gated();
            let (code, out, err) = repo.run_in(&repo.root(), args);
            assert_eq!(code, Some(0), "{err}");
            assert!(out.ends_with(" unset\n"), "{out}");
            assert_eq!((repo.builds(), repo.repins()), (2, 1));
            assert_eq!(repo.state_files(), vec!["built", "dx", "inputs"]);
        }
    }

    #[test]
    fn checks_and_other_launches_never_repin() {
        for args in [
            &["update", "cargo"][..],
            &["update", "--check", "--apply", "cargo"][..],
            &["update", "--apply", "--frozen", "cargo"][..],
            &["update", "--apply", "--offline", "cargo"][..],
            &["update", "--apply", "npm"][..],
            &["status"][..],
        ] {
            let repo = source_repo("dx-launcher-norepin-");
            repo.repin_gated();
            let (code, _, err) = repo.run_in(&repo.root(), args);
            assert_eq!(code, Some(1), "{args:?} {err}");
            assert_eq!((repo.builds(), repo.repins()), (1, 0), "{args:?}");
            assert!(
                err.contains("run tools/dx update --apply cargo to repin"),
                "{err}"
            );
        }
    }

    #[test]
    fn a_healthy_build_does_not_repin() {
        let repo = source_repo("dx-launcher-healthy-");
        let (code, _, err) = repo.run_in(&repo.root(), &["update", "--apply", "cargo"]);
        assert_eq!(code, Some(0), "{err}");
        assert_eq!((repo.builds(), repo.repins()), (1, 0));
    }

    #[test]
    fn a_failed_repin_without_a_previous_dx_reports_the_failed_build() {
        let repo = source_repo("dx-launcher-repin-fails-");
        repo.break_build();
        let (code, _, err) = repo.run_in(&repo.root(), &["update", "--apply", "cargo"]);
        assert_eq!(code, Some(1), "{err}");
        assert!(err.contains("dx: bazel build //:dx failed"), "{err}");
        assert_eq!(repo.builds(), 2);
        assert!(!repo.root().join(".dx/launcher").exists() || repo.state_files().is_empty());
    }

    #[test]
    fn source_launcher_seeds_the_previous_dx_when_dx_is_up_to_date() {
        let repo = source_repo("dx-launcher-seed-");
        repo.run_ok();
        std::fs::remove_file(repo.root().join(".dx/launcher/dx")).expect("remove the copy");
        repo.run_ok();
        assert_eq!(repo.builds(), 1, "seeding the copy does not rebuild");
        assert_eq!(
            repo.state_files(),
            vec!["CACHEDIR.TAG", "built", "dx", "inputs"]
        );
        repo.break_build();
        repo.touch_build_file();
        let (code, out, err) = repo.run();
        assert_eq!((code, out.as_str()), (Some(0), "ran status\n"), "{err}");
        assert!(
            err.contains("dx: //:dx does not build; running the previous dx\n"),
            "{err}"
        );
        assert_eq!(repo.builds(), 2);
    }

    #[test]
    fn a_worktree_hook_run_does_not_seed_the_previous_dx() {
        let repo = source_repo("dx-launcher-hook-seed-");
        repo.run_ok();
        std::fs::remove_file(repo.root().join(".dx/launcher/dx")).expect("remove the copy");
        let (code, out, err) = repo.run_in(&repo.root(), &["worktree", "hook", "guard"]);
        assert_eq!(
            (code, out.as_str()),
            (Some(0), "ran worktree hook guard\n"),
            "{err}"
        );
        assert_eq!(repo.state_files(), vec!["CACHEDIR.TAG", "built", "inputs"]);
        assert_eq!(repo.builds(), 1);
    }

    #[test]
    fn source_launcher_without_a_previous_dx_reports_the_failed_build() {
        let repo = source_repo("dx-launcher-no-fallback-");
        repo.break_build();
        let (code, out, err) = repo.run();
        assert_eq!((code, out.as_str()), (Some(1), ""), "{err}");
        assert!(err.contains("dx: building //:dx\n"), "{err}");
        assert!(err.contains("dx: bazel build //:dx failed\n"), "{err}");
        assert!(!err.contains("bazel run @rules_dx"), "{err}");
        assert!(!err.contains("previous dx"), "{err}");
        assert!(!err.contains("crashed"), "{err}");
        assert_eq!(repo.builds(), 1, "a compile error is not tried again");
        assert_eq!(repo.state_files(), Vec::<String>::new());
    }

    #[test]
    fn the_source_launcher_uses_the_disk_cache_dx_picks() {
        let repo = source_repo("dx-launcher-disk-cache-");
        let root = repo.root();
        let home = repo.base.join("home");
        let args = repo.base.join("args");
        let xdg = repo.base.join("xdg cache");
        let host_rc = std::fs::read_to_string("/etc/bazel.bazelrc").unwrap_or_default();
        let host_sets = host_rc.contains("disk_cache");
        let in_home = home.join(".cache/dx/disk-cache");
        let in_xdg = xdg.join("dx/disk-cache");
        let xdg_value = xdg.to_string_lossy().into_owned();
        for rc in RC_CASES
            .iter()
            .filter(|rc| !rc.files.iter().any(|(slot, _)| matches!(slot, Rc::System))) {
            let name = rc.name;
            let expected = match rc.expect {
                Expect::Default => Some(in_home.as_path()),
                Expect::Xdg => Some(in_xdg.as_path()),
                Expect::Unset => None,
            };
            let vars: Vec<(String, String)> = rc
                .env
                .iter()
                .map(|(k, v)| ((*k).to_owned(), v.replace("{xdg}", &xdg_value)))
                .collect();
            let mut paths = vec![home.join(".bazelrc"), root.join(".bazelrc")];
            paths.extend(rc_names().iter().map(|extra| root.join(extra)));
            for path in &paths {
                let _ = std::fs::remove_file(path);
            }
            for (slot, text) in rc.files {
                let path = match slot {
                    Rc::Workspace => root.join(".bazelrc"),
                    Rc::Home => home.join(".bazelrc"),
                    Rc::Extra(extra) => root.join(extra),
                    Rc::System => unreachable!("{name}: /etc is not writable"),
                };
                std::fs::write(path, text).expect("rc file");
            }
            let _ = std::fs::remove_dir_all(root.join(".dx/launcher"));
            let _ = std::fs::remove_file(&args);
            let mut cmd = repo.command_in(&root, &["status"]);
            cmd.env("STUB_ARGS", &args);
            for (key, value) in &vars {
                cmd.env(key, value);
            }
            let out = spawn(&mut cmd).wait_with_output().expect("launcher runs");
            let stderr = String::from_utf8_lossy(&out.stderr);
            assert_eq!(out.status.code(), Some(0), "{name}: {stderr}");
            assert_eq!(stderr, "dx: building //:dx\n", "{name}");
            let mut lookup: std::collections::HashMap<String, String> = vars.into_iter().collect();
            lookup.insert("HOME".to_owned(), home.to_string_lossy().into_owned());
            let policy = RcPolicy::from_env(&|key| lookup.get(key).cloned(), &root, &|path| {
                std::fs::read_to_string(path).ok()
            });
            if expected.is_none() || !host_sets {
                assert_eq!(
                    policy.disk_cache,
                    expected.map(|path| path.to_string_lossy().into_owned()),
                    "{name}"
                );
            }
            let startup: Vec<String> = policy
                .startup_opts()
                .iter()
                .map(ToString::to_string)
                .collect();
            let mut want = startup.clone();
            want.push("build".to_owned());
            want.extend(policy_flags(&policy));
            want.extend(
                [
                    "--curses=no",
                    "--show_progress_rate_limit=30",
                    "--progress_report_interval=30",
                    "--show_result=0",
                    "--ui_event_filters=-info",
                    "//:dx",
                ]
                .map(str::to_owned),
            );
            let logged = std::fs::read_to_string(&args).expect("bazel argv");
            let calls: Vec<Vec<&str>> = logged
                .lines()
                .map(|line| {
                    line.strip_suffix('\t')
                        .unwrap_or(line)
                        .split('\t')
                        .collect()
                })
                .collect();
            assert_eq!(calls.len(), 2, "{name}: {logged}");
            assert_eq!(calls[0], want, "{name}");
            let bounded = calls[0]
                .iter()
                .any(|arg| arg.starts_with("--experimental_disk_cache_gc_max_size="));
            if !host_sets {
                assert_eq!(bounded, expected.is_some() && !rc.rc_gc, "{name}");
            }
            assert_eq!(calls[1][..startup.len()], startup[..], "{name}");
            assert_eq!(calls[1][startup.len()], "query", "{name}");
            assert!(
                !calls[1].iter().any(|arg| arg.contains("disk_cache")),
                "{name}: {logged}"
            );
        }
    }

    #[test]
    fn source_launcher_rebuilds_only_when_an_input_is_newer() {
        let repo = source_repo("dx-launcher-source-");
        repo.run_ok();
        assert_eq!(repo.builds(), 1, "a missing dx is built");
        assert_eq!(repo.inputs(), "//:BUILD.bazel\n//cli/cli:BUILD.bazel\n");
        repo.run_ok();
        assert_eq!(repo.builds(), 1, "an unchanged tree runs dx without Bazel");
        repo.touch_build_file();
        repo.run_ok();
        assert_eq!(repo.builds(), 2, "a newer input rebuilds dx");
    }

    #[test]
    fn a_launcher_run_from_another_checkout_runs_that_checkout_dx() {
        let repo = source_repo("dx-launcher-slot-");
        let slot = repo.base.join("slot");
        checkout(&slot);
        std::fs::write(slot.join("payload"), "#!/bin/sh\necho slot \"$@\"\n")
            .expect("slot payload");
        let (code, out, err) = repo.run_in(&slot.join("cli"), &["status"]);
        assert_eq!((code, out.as_str()), (Some(0), "slot status\n"), "{err}");
        assert!(slot.join(".dx/launcher/built").exists());
        assert!(!repo.root().join(".dx/launcher").exists());
        assert_eq!(repo.builds(), 1, "only the slot dx is built");
        repo.run_ok();
        assert_eq!(repo.builds(), 2, "the repo builds its own dx");
        let (code, out, err) = repo.run_in(&slot, &["worktree", "hook", "guard"]);
        assert_eq!(
            (code, out.as_str()),
            (Some(0), "ran worktree hook guard\n"),
            "{err}"
        );
        assert_eq!(repo.builds(), 2, "a hook never builds the slot dx");
        repo.run_ok();
        assert_eq!(repo.builds(), 2, "the repo runs its dx in place");
    }

    /// Builds `<base>/<name>/execroot/_main` the way Bazel lays it out: symlinks
    /// into `workspace`, a real `bazel-out/.../bin/cli`, `workspace/bazel-bin`
    /// pointing at that bin directory, and the `DO_NOT_BUILD_HERE` marker.
    /// Returns the execroot `_main` directory.
    fn execroot(
        base: &Path,
        name: &str,
        workspace: &Path,
        marker: Option<&str>,
    ) -> std::path::PathBuf {
        use std::os::unix::fs::symlink;
        let ob = base.join(name);
        let main = ob.join("execroot/_main");
        std::fs::create_dir_all(main.join("bazel-out/k8-fastbuild/bin/cli")).expect("bin");
        for entry in ["cli", "tools", "MODULE.bazel"] {
            symlink(workspace.join(entry), main.join(entry)).expect("link");
        }
        if let Some(text) = marker {
            std::fs::write(ob.join("execroot/DO_NOT_BUILD_HERE"), text).expect("marker");
        }
        let _ = std::fs::remove_file(workspace.join("bazel-bin"));
        symlink(
            main.join("bazel-out/k8-fastbuild/bin"),
            workspace.join("bazel-bin"),
        )
        .expect("bazel-bin");
        main
    }

    fn assert_nothing_built_in(main: &Path) {
        assert!(!main.join(".dx").exists(), "no dx state in the execroot");
        assert!(!main.join("bazel-bin").exists(), "no build in the execroot");
    }

    #[test]
    fn a_launcher_run_from_bazel_bin_runs_its_checkout_dx() {
        let repo = source_repo("dx-launcher-execroot-");
        let marker = repo.root().display().to_string();
        let main = execroot(&repo.base, "ob", &repo.root(), Some(&marker));
        let (code, out, err) = repo.run_in(&repo.root().join("bazel-bin/cli"), &["status"]);
        assert_eq!((code, out.as_str()), (Some(0), "ran status\n"), "{err}");
        assert_eq!(repo.builds(), 1);
        assert_nothing_built_in(&main);
    }

    #[test]
    fn a_launcher_run_from_another_checkout_bazel_bin_runs_that_checkout_dx() {
        let repo = source_repo("dx-launcher-execroot-slot-");
        let slot = repo.base.join("slot");
        checkout(&slot);
        std::fs::write(slot.join("payload"), "#!/bin/sh\necho slot \"$@\"\n").expect("payload");
        let marker = slot.display().to_string();
        let main = execroot(&repo.base, "ob", &slot, Some(&marker));
        let (code, out, err) = repo.run_in(&slot.join("bazel-bin/cli"), &["status"]);
        assert_eq!((code, out.as_str()), (Some(0), "slot status\n"), "{err}");
        assert!(slot.join(".dx/launcher/built").exists());
        assert!(!repo.root().join(".dx/launcher").exists());
        assert_eq!(repo.builds(), 1, "only the slot dx is built");
        assert_nothing_built_in(&main);
    }

    #[test]
    fn an_execroot_with_an_unusable_marker_is_never_entered() {
        for (i, marker) in [Some(""), Some("/nonexistent/checkout")]
            .into_iter()
            .enumerate()
        {
            let repo = source_repo("dx-launcher-execroot-bad-");
            let main = execroot(&repo.base, "ob", &repo.root(), marker);
            let (code, out, err) = repo.run_in(&repo.root().join("bazel-bin/cli"), &["status"]);
            assert_eq!(
                (code, out.as_str()),
                (Some(0), "ran status\n"),
                "{i}: {err}"
            );
            assert_nothing_built_in(&main);
        }
    }

    fn signalled_rebuild(name: &str, sig: &str, signo: i32, seeded: bool) {
        use std::os::unix::process::ExitStatusExt;
        let repo = source_repo(name);
        if seeded {
            repo.run_ok();
            repo.touch_build_file();
        }
        let hold = repo.base.join("hold");
        let child = spawn(repo.command().env("STUB_BUILD_HOLD", &hold));
        let started = repo.base.join("hold.started");
        for _ in 0..600 {
            if started.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(started.exists(), "stub build never started");
        let built = repo
            .root()
            .join(format!(".dx/launcher/built.{}", child.id()));
        assert!(built.exists(), "{}", built.display());
        let status = Command::new("kill")
            .args(["-s", sig, &child.id().to_string()])
            .status()
            .expect("kill runs");
        assert!(status.success());
        std::fs::write(&hold, "").expect("release the build");
        let out = child.wait_with_output().expect("launcher exits");
        let err = String::from_utf8_lossy(&out.stderr).into_owned();
        assert_eq!(out.status.signal(), Some(signo), "{err}");
        assert!(out.stdout.is_empty());
        assert!(!err.contains("running the previous dx"), "{err}");
        assert!(!err.contains("bazel build //:dx failed"), "{err}");
        let files = repo.state_files();
        assert!(
            files.iter().all(|f| !f
                .rsplit('.')
                .next()
                .is_some_and(|x| !x.is_empty() && x.bytes().all(|b| b.is_ascii_digit()))),
            "{files:?}"
        );
        if seeded {
            assert_eq!(files, vec!["CACHEDIR.TAG", "built", "dx", "inputs"]);
            let before = repo.builds();
            repo.run_ok();
            assert_eq!(repo.builds(), before + 1, "the input is still stale");
        }
    }

    #[test]
    fn a_symlinked_alias_marker_runs_the_checkout_once() {
        let repo = source_repo("dx-launcher-execroot-alias-");
        let alias = repo.base.join("alias");
        std::os::unix::fs::symlink(repo.root(), &alias).expect("alias");
        let marker = alias.display().to_string();
        let main = execroot(&repo.base, "ob", &repo.root(), Some(&marker));
        let (code, out, err) = repo.run_in(&repo.root().join("bazel-bin/cli"), &["status"]);
        assert_eq!((code, out.as_str()), (Some(0), "ran status\n"), "{err}");
        assert_eq!(repo.builds(), 1);
        assert_nothing_built_in(&main);
    }

    #[test]
    fn a_marker_naming_another_execroot_is_not_followed() {
        let repo = source_repo("dx-launcher-execroot-chain-");
        let other = execroot(&repo.base, "ob2", &repo.root(), Some("unused"));
        let marker = other.display().to_string();
        let main = execroot(&repo.base, "ob", &repo.root(), Some(&marker));
        let (code, out, err) =
            repo.run_in(&main.join("bazel-out/k8-fastbuild/bin/cli"), &["status"]);
        assert_eq!((code, out.as_str()), (Some(0), "ran status\n"), "{err}");
        assert_nothing_built_in(&main);
        assert_nothing_built_in(&other);
    }

    #[test]
    fn a_signalled_rebuild_leaves_no_temp_files() {
        for (sig, signo) in [("TERM", 15), ("INT", 2), ("HUP", 1)] {
            signalled_rebuild(&format!("dx-launcher-sig-{sig}-"), sig, signo, true);
            signalled_rebuild(&format!("dx-launcher-sig-fresh-{sig}-"), sig, signo, false);
        }
    }

    #[test]
    fn concurrent_stale_runs_all_succeed_and_keep_the_inputs() {
        let repo = source_repo("dx-launcher-concurrent-");
        let barrier = repo.base.join("barrier");
        std::fs::create_dir_all(&barrier).expect("barrier dir");
        let children: Vec<_> = (0..4)
            .map(|_| {
                spawn(
                    repo.command()
                        .env("STUB_BARRIER", &barrier)
                        .env("STUB_BARRIER_N", "4"),
                )
            })
            .collect();
        for child in children {
            let out = child.wait_with_output().expect("launcher runs");
            assert_eq!(
                (
                    out.status.code(),
                    String::from_utf8_lossy(&out.stdout).into_owned()
                ),
                (Some(0), "ran status\n".to_owned()),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        assert_eq!(repo.builds(), 4, "all four runs really built concurrently");
        assert_eq!(
            repo.state_files(),
            vec!["CACHEDIR.TAG", "built", "dx", "inputs"]
        );
        assert!(
            repo.inputs().contains("//cli/cli:BUILD.bazel"),
            "{}",
            repo.inputs()
        );
        let before = repo.builds();
        repo.touch_build_file();
        repo.run_ok();
        assert_eq!(repo.builds(), before + 1, "a newer input still rebuilds dx");
    }

    #[test]
    fn an_empty_input_list_rebuilds() {
        let repo = source_repo("dx-launcher-empty-inputs-");
        repo.run_ok();
        assert_eq!(repo.builds(), 1);
        std::fs::write(repo.root().join(".dx/launcher/inputs"), "").expect("truncate");
        repo.run_ok();
        assert_eq!(repo.builds(), 2, "an empty input list rebuilds dx");
        assert_eq!(repo.inputs(), "//:BUILD.bazel\n//cli/cli:BUILD.bazel\n");
    }
    #[test]
    fn source_launcher_runs_the_fresh_dx_when_the_inputs_query_fails() {
        let repo = source_repo("dx-launcher-query-fails-");
        let flag = repo.base.join("query-fails");
        std::fs::write(&flag, "").expect("flag");
        let (code, out, err) = repo.run();
        assert_eq!((code, out.as_str()), (Some(0), "ran status\n"), "{err}");
        assert!(
            err.contains(
                "dx: bazel query for the inputs of //:dx failed; running the fresh dx anyway\n"
            ),
            "{err}"
        );
        assert!(err.contains("Server shut down explicitly"), "{err}");
        assert_eq!(repo.builds(), 1);
        assert_eq!(repo.state_files(), vec!["dx"]);
        let (code, out, err) = repo.run();
        assert_eq!((code, out.as_str()), (Some(0), "ran status\n"), "{err}");
        assert_eq!(repo.builds(), 2, "still stale, so it rebuilds");
        assert_eq!(repo.state_files(), vec!["dx"]);
        std::fs::remove_file(&flag).expect("clear flag");
        repo.run_ok();
        assert_eq!(repo.builds(), 3);
        assert_eq!(repo.inputs(), "//:BUILD.bazel\n//cli/cli:BUILD.bazel\n");
        repo.run_ok();
        assert_eq!(repo.builds(), 3, "a recorded stamp skips Bazel");
        std::fs::write(&flag, "").expect("flag");
        repo.touch_build_file();
        repo.run_ok();
        assert_eq!(repo.builds(), 4);
        assert_eq!(repo.inputs(), "//:BUILD.bazel\n//cli/cli:BUILD.bazel\n");
        assert_eq!(repo.state_files(), vec!["built", "dx", "inputs"]);
        repo.run_ok();
        assert_eq!(repo.builds(), 5, "the stamp did not move, so it is stale");
    }
}

#[cfg(windows)]
mod cmd {
    use super::*;
    use dx_testing::mkscratch;
    use std::path::Path;
    use std::process::Command;

    struct SourceRepo {
        base: std::path::PathBuf,
    }

    fn checkout(root: &Path) {
        std::fs::create_dir_all(root.join("cli/cli")).expect("cli dir");
        std::fs::create_dir_all(root.join("tools/bazelrc")).expect("bazelrc dir");
        std::fs::write(
            root.join("MODULE.bazel"),
            "module(\r\n    name = \"rules_dx\",\r\n)\r\n",
        )
        .expect("module");
        std::fs::write(root.join("cli/cli/BUILD.bazel"), "").expect("build file");
        std::fs::write(root.join("tools/bazelrc/resources.bazelrc"), "").expect("preset");
        let text = std::fs::read_to_string(launcher_path("DX_LAUNCHER_CMD")).expect("launcher");
        std::fs::write(
            root.join("tools/dx.cmd"),
            text.replace("\r\n", "\n").replace('\n', "\r\n"),
        )
        .expect("launcher");
    }

    fn source_repo(name: &str) -> SourceRepo {
        let base = mkscratch(name).expect("scratch");
        checkout(&base.join("repo"));
        std::fs::create_dir_all(base.join("stub")).expect("stub dir");
        std::fs::write(
            base.join("stub/bazel.cmd"),
            "@echo off\r\nif defined STUB_ARGS echo %*>>\"%STUB_ARGS%\"\r\nset \"OP=%3\"\r\nif \"%1\"==\"build\" set \"OP=build\"\r\nif \"%1\"==\"query\" set \"OP=query\"\r\necho %OP%>>\"%STUB_LOG%\"\r\nset \"SRC=%SystemRoot%\\System32\\hostname.exe\"\r\nif defined STUB_EXE set \"SRC=%STUB_EXE%\"\r\nif \"%OP%\"==\"build\" (\r\n  if not exist bazel-bin\\cli\\cli mkdir bazel-bin\\cli\\cli\r\n  copy /y \"%SRC%\" bazel-bin\\cli\\cli\\dx.exe >nul\r\n)\r\nif \"%OP%\"==\"query\" if exist \"%STUB_QUERY_FAIL%\" exit /b 1\r\nif \"%OP%\"==\"query\" (\r\n  echo //cli/cli:BUILD.bazel\r\n  ping -n 2 127.0.0.1 >nul\r\n  echo @rules_rust//rust:defs.bzl\r\n)\r\nexit /b 0\r\n",
        )
        .expect("bazel stub");
        SourceRepo { base }
    }

    impl SourceRepo {
        fn root(&self) -> std::path::PathBuf {
            self.base.join("repo")
        }

        fn command(&self, args: &[&str], log: &Path) -> Command {
            self.command_in(&self.root(), args, log)
        }

        fn command_in(&self, cwd: &Path, args: &[&str], log: &Path) -> Command {
            let system = std::env::var("SystemRoot").expect("SystemRoot");
            let mut cmd = Command::new("cmd");
            cmd.arg("/c")
                .arg(self.root().join("tools/dx.cmd"))
                .args(args)
                .current_dir(cwd)
                .env(
                    "PATH",
                    format!(
                        "{};{system}\\System32;{system}\\System32\\WindowsPowerShell\\v1.0",
                        self.base.join("stub").display()
                    ),
                )
                .env("STUB_LOG", log)
                .env("STUB_QUERY_FAIL", self.base.join("query-fails"));
            cmd
        }

        fn run(&self, args: &[&str]) -> (Option<i32>, String) {
            self.run_in(&self.root(), args)
        }

        fn run_in(&self, cwd: &Path, args: &[&str]) -> (Option<i32>, String) {
            let out = self
                .command_in(cwd, args, &self.base.join("calls"))
                .output()
                .expect("launcher runs");
            (
                out.status.code(),
                format!(
                    "{}{}",
                    String::from_utf8_lossy(&out.stdout),
                    String::from_utf8_lossy(&out.stderr)
                ),
            )
        }

        fn builds(&self) -> usize {
            std::fs::read_to_string(self.base.join("calls")).map_or(0, |text| {
                text.lines().filter(|l| l.trim() == "build").count()
            })
        }

        fn state_files(&self) -> Vec<String> {
            let mut names: Vec<String> = std::fs::read_dir(self.root().join(".dx/launcher"))
                .expect("state dir")
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();
            names
        }

        fn inputs(&self) -> String {
            std::fs::read_to_string(self.root().join(".dx/launcher/inputs")).expect("inputs")
        }
    }

    #[test]
    fn cmd_launcher_rebuilds_only_when_an_input_is_newer() {
        let repo = source_repo("dx-launcher-cmd-source-");
        let root = repo.root();
        let run = |args: &[&str]| repo.run(args);
        let run_ok = |args: &[&str]| {
            let (code, out) = run(args);
            assert_eq!(code, Some(0), "{out}");
        };
        let builds = || repo.builds();
        run_ok(&[]);
        assert_eq!(builds(), 1, "a missing dx is built");
        assert!(root.join(".dx/launcher/inputs").exists());
        run_ok(&[]);
        assert_eq!(builds(), 1, "an unchanged tree runs dx without Bazel");
        let preset = std::fs::File::options()
            .write(true)
            .open(root.join("tools/bazelrc/resources.bazelrc"))
            .expect("open preset");
        preset
            .set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(3600))
            .expect("touch preset");
        drop(preset);
        run(&["worktree", "hook", "guard"]);
        assert_eq!(builds(), 1, "a hook runs the built dx without Bazel");
        run_ok(&[]);
        assert_eq!(builds(), 2, "a newer preset layer rebuilds dx");
    }

    #[test]
    fn cmd_a_launcher_run_from_bazel_bin_runs_its_checkout_dx() {
        let repo = source_repo("dx-launcher-cmd-execroot-");
        let slot = repo.base.join("slot");
        checkout(&slot);
        let ob = repo.base.join("ob");
        let main = ob.join("execroot/_main");
        let bin = main.join("bazel-out/x64_windows-fastbuild/bin/cli");
        std::fs::create_dir_all(&bin).expect("bin");
        std::fs::create_dir_all(main.join("tools/bazelrc")).expect("tools");
        std::fs::create_dir_all(main.join("cli/cli")).expect("cli");
        std::fs::copy(slot.join("MODULE.bazel"), main.join("MODULE.bazel")).expect("module");
        std::fs::copy(
            slot.join("cli/cli/BUILD.bazel"),
            main.join("cli/cli/BUILD.bazel"),
        )
        .expect("build");
        std::fs::copy(slot.join("tools/dx.cmd"), main.join("tools/dx.cmd")).expect("launcher");
        let marker = slot.display().to_string().replace('\\', "/");
        std::fs::write(ob.join("execroot/DO_NOT_BUILD_HERE"), marker).expect("marker");
        let (code, out) = repo.run_in(&bin, &[]);
        assert_eq!(code, Some(0), "{out}");
        assert!(slot.join(".dx/launcher/built").exists());
        assert!(!main.join(".dx").exists(), "no dx state in the execroot");
        assert_eq!(repo.builds(), 1, "only the slot dx is built");
    }

    #[test]
    fn cmd_a_launcher_run_from_another_checkout_runs_that_checkout_dx() {
        let repo = source_repo("dx-launcher-cmd-slot-");
        let slot = repo.base.join("slot");
        checkout(&slot);
        let (code, out) = repo.run_in(&slot.join("cli"), &[]);
        assert_eq!(code, Some(0), "{out}");
        assert!(slot.join(".dx/launcher/built").exists());
        assert!(slot.join("bazel-bin/cli/cli/dx.exe").exists());
        assert!(!repo.root().join(".dx/launcher").exists());
        assert_eq!(repo.builds(), 1, "only the slot dx is built");
        let (code, out) = repo.run(&[]);
        assert_eq!(code, Some(0), "{out}");
        assert!(repo.root().join(".dx/launcher/built").exists());
        assert_eq!(repo.builds(), 2, "the repo builds its own dx");
        std::fs::remove_file(slot.join("bazel-bin/cli/cli/dx.exe")).expect("drop slot dx");
        repo.run_in(&slot, &["worktree", "hook", "guard"]);
        assert_eq!(repo.builds(), 2, "a hook never builds the slot dx");
        assert!(!slot.join("bazel-bin/cli/cli/dx.exe").exists());
        let (code, out) = repo.run(&[]);
        assert_eq!(code, Some(0), "{out}");
        assert_eq!(repo.builds(), 2, "the repo runs its dx in place");
    }

    #[test]
    fn cmd_concurrent_stale_runs_all_succeed_and_keep_the_inputs() {
        let repo = source_repo("dx-launcher-cmd-concurrent-");
        let children: Vec<_> = (0..4)
            .map(|_| {
                repo.command(&[], Path::new("NUL"))
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .spawn()
                    .expect("launcher starts")
            })
            .collect();
        for child in children {
            let out = child.wait_with_output().expect("launcher runs");
            assert_eq!(
                out.status.code(),
                Some(0),
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
        }
        assert_eq!(
            repo.state_files(),
            vec!["CACHEDIR.TAG", "built", "dx.exe", "inputs"]
        );
        assert!(
            repo.inputs().contains("//cli/cli:BUILD.bazel"),
            "{}",
            repo.inputs()
        );
    }

    fn system_exe(name: &str) -> std::path::PathBuf {
        Path::new(&std::env::var("SystemRoot").expect("SystemRoot"))
            .join("System32")
            .join(name)
    }

    fn touch_preset(root: &Path) {
        let preset = std::fs::File::options()
            .write(true)
            .open(root.join("tools/bazelrc/resources.bazelrc"))
            .expect("open preset");
        preset
            .set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(3600))
            .expect("touch preset");
    }

    struct Running {
        child: std::process::Child,
        out: std::io::BufReader<std::process::ChildStdout>,
        stopped: bool,
    }

    impl Running {
        fn stop(&mut self) {
            if std::mem::replace(&mut self.stopped, true) {
                return;
            }
            let _ = Command::new(system_exe("taskkill.exe"))
                .args(["/T", "/F", "/PID", &self.child.id().to_string()])
                .output();
            let _ = self.child.kill();
            let _ = std::io::Read::read_to_end(&mut self.out, &mut Vec::new());
            let _ = self.child.wait();
        }
    }

    impl Drop for Running {
        fn drop(&mut self) {
            self.stop();
        }
    }

    #[test]
    fn cmd_a_running_dx_does_not_lock_its_rebuild() {
        let repo = source_repo("dx-launcher-cmd-running-");
        let root = repo.root();
        let calls = repo.base.join("calls");
        let out = repo
            .command(&["--exit-code=0"], &calls)
            .env(
                "STUB_EXE",
                dx_testing::process_probe()
                    .to_string_lossy()
                    .replace('/', "\\"),
            )
            .output()
            .expect("launcher runs");
        assert_eq!(out.status.code(), Some(0), "{out:?}");
        assert_eq!(repo.builds(), 1, "{out:?}");
        let mut child = repo
            .command(&["--print-exe", "--sleep-ms=30000"], &calls)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("launcher starts");
        let stdout = child.stdout.take().expect("stdout");
        let mut running = Running {
            child,
            out: std::io::BufReader::new(stdout),
            stopped: false,
        };
        let mut line = String::new();
        std::io::BufRead::read_line(&mut running.out, &mut line).expect("read the running dx");
        assert!(!line.is_empty(), "the launcher exited before dx ran");
        assert_eq!(
            std::fs::canonicalize(line.trim()).expect("the running dx exists"),
            std::fs::canonicalize(root.join(".dx/launcher/dx.exe")).expect("the launcher copy"),
            "the running dx is the launcher copy"
        );
        std::fs::remove_file(root.join("bazel-bin/cli/cli/dx.exe"))
            .expect("a running dx leaves bazel-bin dx.exe free");
        touch_preset(&root);
        let (code, out) = repo.run(&[]);
        assert_eq!(code, Some(0), "{out}");
        assert_eq!(repo.builds(), 2, "{out}");
        let name = Command::new(system_exe("hostname.exe"))
            .output()
            .expect("hostname runs");
        let name = String::from_utf8_lossy(&name.stdout).trim().to_owned();
        assert!(!name.is_empty());
        assert!(out.contains(&name), "{name}: {out}");
        running.stop();
        let (code, out) = repo.run(&[]);
        assert_eq!(code, Some(0), "{out}");
        assert_eq!(
            repo.state_files(),
            vec!["CACHEDIR.TAG", "built", "dx.exe", "inputs"]
        );
    }

    #[test]
    fn cmd_a_hook_runs_the_copy() {
        let repo = source_repo("dx-launcher-cmd-hook-copy-");
        let root = repo.root();
        let (code, out) = repo.run(&[]);
        assert_eq!(code, Some(0), "{out}");
        std::fs::remove_file(root.join("bazel-bin/cli/cli/dx.exe")).expect("drop dx");
        let args = ["worktree", "hook", "guard"];
        let direct = Command::new(system_exe("hostname.exe"))
            .args(args)
            .output()
            .expect("hostname runs");
        let (code, out) = repo.run(&args);
        assert_eq!(code, direct.status.code(), "{out}");
        assert!(!out.contains("dx: building"), "{out}");
        assert_eq!(repo.builds(), 1, "a hook runs the copy without Bazel");
        assert!(!root.join("bazel-bin/cli/cli/dx.exe").exists());
    }

    #[test]
    fn cmd_the_copy_follows_a_dx_rebuilt_outside_the_launcher() {
        let repo = source_repo("dx-launcher-cmd-follow-");
        let root = repo.root();
        let (code, out) = repo.run(&[]);
        assert_eq!(code, Some(0), "{out}");
        let bin = root.join("bazel-bin/cli/cli/dx.exe");
        let copy = root.join(".dx/launcher/dx.exe");
        std::fs::copy(system_exe("whoami.exe"), &bin).expect("rebuild dx");
        let len = |path: &Path| std::fs::metadata(path).expect("metadata").len();
        assert_ne!(len(&copy), len(&bin));
        let direct = Command::new(system_exe("whoami.exe"))
            .output()
            .expect("whoami runs");
        assert!(!direct.stdout.is_empty());
        let (code, out) = repo.run(&[]);
        assert_eq!(code, Some(0), "{out}");
        assert!(
            out.contains(String::from_utf8_lossy(&direct.stdout).trim()),
            "{out}"
        );
        assert_eq!(len(&copy), len(&bin));
        assert_eq!(
            repo.builds(),
            1,
            "a newer bazel-bin dx is copied without Bazel"
        );
        assert_eq!(
            repo.state_files(),
            vec!["CACHEDIR.TAG", "built", "dx.exe", "inputs"]
        );
    }

    #[test]
    fn cmd_launcher_uses_the_disk_cache_dx_picks() {
        let repo = source_repo("dx-launcher-cmd-disk-cache-");
        let root = repo.root();
        let profile = repo.base.join("home");
        std::fs::create_dir_all(&profile).expect("profile");
        let local = repo.base.join("local");
        let data = repo.base.join("programdata");
        std::fs::create_dir_all(&data).expect("programdata");
        let xdg = repo.base.join("xdg cache");
        let args = repo.base.join("args");
        let xdg_value = xdg.to_string_lossy().into_owned();
        let in_local = local.join("dx").join("disk-cache");
        let in_xdg = xdg.join("dx").join("disk-cache");
        for rc in RC_CASES {
            let name = rc.name;
            let mut paths = vec![
                profile.join(".bazelrc"),
                root.join(".bazelrc"),
                data.join("bazel.bazelrc"),
            ];
            paths.extend(rc_names().iter().map(|extra| root.join(extra)));
            for path in &paths {
                let _ = std::fs::remove_file(path);
            }
            for (slot, text) in rc.files {
                let path = match slot {
                    Rc::Workspace => root.join(".bazelrc"),
                    Rc::Home => profile.join(".bazelrc"),
                    Rc::Extra(extra) => root.join(extra),
                    Rc::System => data.join("bazel.bazelrc"),
                };
                std::fs::write(path, text.replace('\n', "\r\n")).expect("rc file");
            }
            let _ = std::fs::remove_dir_all(root.join(".dx/launcher"));
            let _ = std::fs::remove_file(&args);
            let mut vars: Vec<(String, String)> = vec![
                ("USERPROFILE".to_owned(), profile.to_string_lossy().into_owned()),
                ("LOCALAPPDATA".to_owned(), local.to_string_lossy().into_owned()),
                ("ProgramData".to_owned(), data.to_string_lossy().into_owned()),
            ];
            vars.extend(
                rc.env
                    .iter()
                    .map(|(k, v)| ((*k).to_owned(), v.replace("{xdg}", &xdg_value))),
            );
            let mut cmd = repo.command(&[], &repo.base.join("calls"));
            cmd.env_remove("CI")
                .env_remove("BAZEL_TEST")
                .env_remove("HOME")
                .env_remove("XDG_CACHE_HOME")
                .env("STUB_ARGS", &args);
            for (key, value) in &vars {
                cmd.env(key, value);
            }
            let out = cmd.output().expect("launcher runs");
            let text = format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            assert_eq!(out.status.code(), Some(0), "{name}: {text}");
            let lookup: std::collections::HashMap<String, String> = vars.into_iter().collect();
            let policy = RcPolicy::from_env(&|key| lookup.get(key).cloned(), &root, &|path| {
                std::fs::read_to_string(path).ok()
            });
            let expected = match rc.expect {
                Expect::Default => Some(&in_local),
                Expect::Xdg => Some(&in_xdg),
                Expect::Unset => None,
            };
            assert_eq!(
                policy.disk_cache,
                expected.map(|path| path.to_string_lossy().into_owned()),
                "{name}"
            );
            let flags: Vec<String> = policy_flags(&policy)
                .into_iter()
                .map(|flag| match flag.strip_prefix("--disk_cache=") {
                    Some(path) => format!("\"--disk_cache={}\"", path.replace('/', "\\")),
                    None => flag,
                })
                .collect();
            let logged = std::fs::read_to_string(&args).expect("bazel argv");
            let build = logged
                .lines()
                .find(|line| line.starts_with("build ") || line.contains(" build "))
                .expect("build argv");
            let query = logged
                .lines()
                .find(|line| line.starts_with("query ") || line.contains(" query "))
                .expect("query argv");
            let want = if flags.is_empty() {
                "build --curses=no ".to_owned()
            } else {
                format!("build {} --curses=no ", flags.join(" "))
            };
            assert!(build.contains(&want), "{name}: want {want}: {logged}");
            let bounded = build.contains("--experimental_disk_cache_gc_max_size=");
            assert_eq!(bounded, expected.is_some() && !rc.rc_gc, "{name}: {logged}");
            assert!(!query.contains("disk_cache"), "{name}: {logged}");
        }
    }

    #[test]
    fn cmd_launcher_runs_the_previous_dx_when_dx_does_not_build() {
        let repo = source_repo("dx-launcher-cmd-fallback-");
        let root = repo.root();
        let (code, out) = repo.run(&[]);
        assert_eq!(code, Some(0), "{out}");
        assert!(out.contains("dx: building //:dx"), "{out}");
        assert!(root.join(".dx/launcher/dx.exe").exists());
        std::fs::write(
            repo.base.join("stub/bazel.cmd"),
            "@echo off\r\nset \"OP=%3\"\r\nif \"%1\"==\"build\" set \"OP=build\"\r\necho %OP%>>\"%STUB_LOG%\"\r\nif \"%OP%\"==\"build\" (\r\n  del /q bazel-bin\\cli\\cli\\dx.exe\r\n  exit /b 1\r\n)\r\nexit /b 0\r\n",
        )
        .expect("failing bazel stub");
        let preset = std::fs::File::options()
            .write(true)
            .open(root.join("tools/bazelrc/resources.bazelrc"))
            .expect("open preset");
        preset
            .set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(3600))
            .expect("touch preset");
        drop(preset);
        let (code, out) = repo.run(&[]);
        assert_eq!(code, Some(0), "{out}");
        assert!(out.contains("dx: building //:dx"), "{out}");
        assert!(
            out.contains("dx: //:dx does not build; running the previous dx"),
            "{out}"
        );
        assert!(!out.contains("bazel run @rules_dx"), "{out}");
        assert!(!root.join("bazel-bin/cli/cli/dx.exe").exists());
        assert_eq!(repo.builds(), 2);
        assert_eq!(
            repo.state_files(),
            vec!["CACHEDIR.TAG", "built", "dx.exe", "inputs"]
        );
        let (code, out) = repo.run(&[]);
        assert_eq!(code, Some(0), "{out}");
        assert_eq!(repo.builds(), 3, "a failed build is tried again");
    }

    fn crash_stub(repo: &SourceRepo, code: i32, times: usize) {
        std::fs::write(repo.base.join("crash-left"), times.to_string()).expect("counter");
        std::fs::write(
            repo.base.join("stub/bazel.cmd"),
            format!(
                "@echo off\r\nset \"OP=%3\"\r\nif \"%1\"==\"build\" set \"OP=build\"\r\necho %OP%>>\"%STUB_LOG%\"\r\nif \"%OP%\"==\"build\" (\r\n  set /p LEFT=<\"{left}\"\r\n  setlocal enabledelayedexpansion\r\n  if !LEFT! gtr 0 (\r\n    set /a LEFT=!LEFT!-1\r\n    echo !LEFT!>\"{left}\"\r\n    exit /b {code}\r\n  )\r\n  endlocal\r\n  if not exist bazel-bin\\cli\\cli mkdir bazel-bin\\cli\\cli\r\n  copy /y \"%SystemRoot%\\System32\\hostname.exe\" bazel-bin\\cli\\cli\\dx.exe >nul\r\n)\r\nif \"%OP%\"==\"query\" (\r\n  echo //cli/cli:BUILD.bazel\r\n)\r\nexit /b 0\r\n",
                left = repo.base.join("crash-left").display(),
            ),
        )
        .expect("crashing bazel stub");
    }

    #[test]
    fn cmd_launcher_builds_again_after_a_server_crash() {
        let repo = source_repo("dx-launcher-cmd-crash-once-");
        crash_stub(&repo, 37, 1);
        let (code, out) = repo.run(&[]);
        assert_eq!(code, Some(0), "{out}");
        assert!(
            out.contains("dx: the Bazel server crashed (exit 37); trying //:dx again"),
            "{out}"
        );
        assert_eq!(repo.builds(), 2);
        assert_eq!(repo.state_files(), vec!["built", "dx.exe", "inputs"]);
    }

    #[test]
    fn cmd_launcher_retries_a_server_crash_once() {
        let repo = source_repo("dx-launcher-cmd-crash-twice-");
        crash_stub(&repo, 33, 10);
        let (code, out) = repo.run(&[]);
        assert_eq!(code, Some(1), "{out}");
        assert!(out.contains("dx: bazel build //:dx failed"), "{out}");
        assert_eq!(repo.builds(), 2);
    }

    #[test]
    fn cmd_launcher_tags_its_state_as_a_cache() {
        let repo = source_repo("dx-launcher-cmd-tag-");
        let root = repo.root();
        let (code, out) = repo.run(&[]);
        assert_eq!(code, Some(0), "{out}");
        let tag = root.join(".dx/launcher/CACHEDIR.TAG");
        let text = std::fs::read_to_string(&tag).expect("tag");
        assert!(
            text.starts_with("Signature: 8a477f597d28d172789f06886806bc55"),
            "{text}"
        );
        std::fs::remove_file(&tag).expect("remove the tag");
        let (code, out) = repo.run(&[]);
        assert_eq!(code, Some(0), "{out}");
        assert!(tag.exists(), "an existing untagged copy gets the tag");
    }

    #[test]
    fn cmd_launcher_seeds_the_previous_dx_when_dx_is_up_to_date() {
        let repo = source_repo("dx-launcher-cmd-seed-");
        let root = repo.root();
        let (code, out) = repo.run(&[]);
        assert_eq!(code, Some(0), "{out}");
        std::fs::remove_file(root.join(".dx/launcher/dx.exe")).expect("remove the copy");
        let (code, out) = repo.run(&[]);
        assert_eq!(code, Some(0), "{out}");
        assert_eq!(repo.builds(), 1, "seeding the copy does not rebuild");
        assert_eq!(
            repo.state_files(),
            vec!["CACHEDIR.TAG", "built", "dx.exe", "inputs"]
        );
        std::fs::write(
            repo.base.join("stub/bazel.cmd"),
            "@echo off\r\nset \"OP=%3\"\r\nif \"%1\"==\"build\" set \"OP=build\"\r\necho %OP%>>\"%STUB_LOG%\"\r\nif \"%OP%\"==\"build\" (\r\n  del /q bazel-bin\\cli\\cli\\dx.exe\r\n  exit /b 1\r\n)\r\nexit /b 0\r\n",
        )
        .expect("failing bazel stub");
        let preset = std::fs::File::options()
            .write(true)
            .open(root.join("tools/bazelrc/resources.bazelrc"))
            .expect("open preset");
        preset
            .set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(3600))
            .expect("touch preset");
        drop(preset);
        let (code, out) = repo.run(&[]);
        assert_eq!(code, Some(0), "{out}");
        assert!(
            out.contains("dx: //:dx does not build; running the previous dx"),
            "{out}"
        );
        assert_eq!(repo.builds(), 2);
    }

    #[test]
    fn cmd_an_empty_input_list_rebuilds() {
        let repo = source_repo("dx-launcher-cmd-empty-inputs-");
        let (code, out) = repo.run(&[]);
        assert_eq!(code, Some(0), "{out}");
        assert_eq!(repo.builds(), 1);
        std::fs::write(repo.root().join(".dx/launcher/inputs"), "").expect("truncate");
        let (code, out) = repo.run(&[]);
        assert_eq!(code, Some(0), "{out}");
        assert_eq!(repo.builds(), 2, "an empty input list rebuilds dx");
        assert!(
            repo.inputs().contains("//cli/cli:BUILD.bazel"),
            "{}",
            repo.inputs()
        );
    }

    #[test]
    fn cmd_launcher_runs_the_fresh_dx_when_the_inputs_query_fails() {
        let repo = source_repo("dx-launcher-cmd-query-fails-");
        let flag = repo.base.join("query-fails");
        std::fs::write(&flag, "").expect("flag");
        let (code, out) = repo.run(&[]);
        assert_eq!(code, Some(0), "{out}");
        assert!(
            out.contains(
                "dx: bazel query for the inputs of //:dx failed; running the fresh dx anyway"
            ),
            "{out}"
        );
        assert_eq!(repo.builds(), 1);
        assert_eq!(repo.state_files(), vec!["dx.exe"]);
        let (code, out) = repo.run(&[]);
        assert_eq!(code, Some(0), "{out}");
        assert_eq!(repo.builds(), 2, "still stale, so it rebuilds");
        assert_eq!(repo.state_files(), vec!["dx.exe"]);
        std::fs::remove_file(&flag).expect("clear flag");
        let (code, out) = repo.run(&[]);
        assert_eq!(code, Some(0), "{out}");
        assert_eq!(repo.builds(), 3);
        assert_eq!(
            repo.inputs().replace("\r\n", "\n"),
            "//cli/cli:BUILD.bazel\n"
        );
        let (code, out) = repo.run(&[]);
        assert_eq!(code, Some(0), "{out}");
        assert_eq!(repo.builds(), 3, "a recorded stamp skips Bazel");
    }
}
