# `dx fuzz`

Runs the fuzz targets in scope for a time budget and keeps their crashes under `.dx/local/fuzz`.

```sh
dx fuzz //pkg:parser
dx fuzz --time 5m //pkg/...
dx fuzz --time 90s --jobs 4
```

```text
dx fuzz [--output json] [--here] [--time <duration>] [--jobs <n>] [scope ...]
```

A fuzz target is a rule tagged `dx-fuzz` with a runner named `<name>_run` next to it. Targets without the tag are skipped. Targets run one after another.

Go: `go_fuzz_test(name, fuzz = "FuzzParse", srcs = ..., ...)` from `//go/rules:defs.bzl` declares the test, tags it and adds the `<name>_run` runner.

Python: `python_fuzz_test(name, srcs, main, deps = ["@pypi//atheris"])` from `//python/rules:defs.bzl` declares a binary that calls `atheris.Setup` and `atheris.Fuzz`, tags it and adds the `<name>_run` runner. Linux x86_64 only: there is no atheris wheel for Windows.

Rust: `rust_fuzz_test(name, srcs, crate_root = ...)` from `//rust/rules:defs.bzl` builds a `libfuzzer-sys` binary with sancov and AddressSanitizer using `RUSTC_BOOTSTRAP=1` and the pinned LLVM's libFuzzer, tags it and adds the `<name>_run` runner. The source is `#![no_main]` and calls `fuzz_target!`. `libfuzzer-sys` comes from rules_dx's pinned crate hub by default; pass `libfuzzer_sys = "@<your_hub>//:libfuzzer-sys"` to use your own hub, for example when you use `arbitrary`. Tested on linux x86_64 only.

| Flag | What it does |
| --- | --- |
| `--time <duration>` | Time budget per target: `90`, `90s`, `5m` or `1h`. Default: `60s`. |
| `--jobs <n>` | Jobs each runner may use. Passed to the runner as `--jobs <n>`. |
| `--output json` | Write NDJSON events on stdout instead of text. |
| `--here` | Run on the current directory tree. |
| `--workspace`, `-q`, `-v`, `--color`, `--build-slots`, `--bazel-startup-option` | Global flags. |

Scopes: labels, patterns, files, or dirs. With no scope, dx fuzz runs `//...`.

## Runner contract

For each target dx runs:

```sh
bazel run //pkg:parser_run -- --time <seconds> --out .dx/local/fuzz/pkg/parser/
```

The runner resolves `--out` against `BUILD_WORKSPACE_DIRECTORY` and writes crash files named `crash-<hash>` there. It also accepts `--repro <file>` to replay one crash.

| Runner exit | Meaning |
| --- | --- |
| `0` | No crash. |
| `77` | A crash file was written. |
| Other | Tool error. |

## Output

A crash prints one line on stderr:

```text
dx: //pkg:parser crashed; reproduce with: dx run //pkg:parser_run -- --repro .dx/local/fuzz/pkg/parser/crash-<hash>
```

`/.dx/local/` is in the `.gitignore` that `dx init` writes.

With `--output json` dx writes `command_started`, one `notice` with code `fuzz_crash` per crash, and `command_finished`. The notice carries `target`, `runner`, `crash_path` and `repro`, the argv that replays the crash.

Reports: none.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | No target crashed. Also when no target in scope carries `dx-fuzz`. |
| `1` | A target crashed or its runner failed. |
| `2` | Usage error. |
