# `dx mutate`

Runs the mutation targets in scope for a time budget and lists the mutants your tests did not catch.

```sh
dx mutate //pkg:lib
dx mutate --time 5m //pkg/...
dx mutate --time 90s --fail-on=survived:20
```

```text
dx mutate [--here] [--time <duration>] [--fail-on=survived:<pct>] [scope ...]
```

A mutation target is a rule tagged `dx-mutate` with a runner named `<name>_run` next to it. Targets without the tag are skipped. Targets run one after another.

| Flag | What it does |
| --- | --- |
| `--time <duration>` | Time budget per target: `90`, `90s`, `5m` or `1h`. Default: `60s`. |
| `--fail-on=survived:<pct>` | Exit 1 when more than this percent of a target's mutants survive. |
| `--here` | Run on the current directory tree. |
| `--workspace`, `-q`, `-v`, `--color`, `--build-slots`, `--bazel-startup-option` | Global flags. |

Scopes: labels, patterns, files, or dirs. With no scope, dx mutate runs `//...`.

## Runner contract

For each target dx runs:

```sh
bazel run //pkg:lib_run -- --time <seconds> --out .dx/mutate/pkg/lib/
```

The runner resolves `--out` against `BUILD_WORKSPACE_DIRECTORY`, stops within the time budget, and writes `mutants.json` there:

```json
{
  "killed": 6,
  "survived": 3,
  "timed_out": 1,
  "survivors": [{"file": "src/lib.rs", "line": 12, "mutation": "replace + with -"}]
}
```

| Runner exit | Meaning |
| --- | --- |
| `0` | `mutants.json` was written. |
| Other | Tool error. |

## Output

Each target prints one count line and one line per surviving mutant on stdout:

```text
//pkg:lib: killed 6, survived 3, timed out 1 (30.0% survived)
  survived src/lib.rs:12 replace + with -
```

`/.dx/mutate/` is in the `.gitignore` that `dx init` writes.

Reports: none.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | No limit was crossed. Also when no target in scope carries `dx-mutate`. |
| `1` | A runner failed, or a target went over `--fail-on=survived`. |
| `2` | Usage error. |

## Rust

`rust_mutate(name = "mutate")` from `@rules_dx//rust/rules:mutate.bzl` declares the `dx-mutate` target and its runner. The runner runs `cargo-mutants` (from `PATH`, or `CARGO_MUTANTS`) in the package directory, which needs a `Cargo.toml`, and converts its `caught.txt`, `missed.txt` and `timeout.txt` into `mutants.json`.

## Other languages

Each wrapper declares the target the same way and the runner runs the tool in the package directory:

| Macro | Load from | Tool | Override |
| --- | --- | --- | --- |
| `python_mutate` | `@rules_dx//python/rules:mutate.bzl` | `mutmut` | `MUTMUT` |
| `go_mutate` | `@rules_dx//go/rules:mutate.bzl` | `gremlins unleash` | `GREMLINS` |
| `ts_mutate` | `@rules_dx//typescript/rules:mutate.bzl` | `stryker` with the json reporter, needs `node` | `STRYKER` |
| `java_mutate` | `@rules_dx//java/rules:mutate.bzl` | `pitest-maven` | `PITEST` |
| `kotlin_mutate` | `@rules_dx//kotlin/rules:mutate.bzl` | `pitest-maven` | `PITEST` |

The override variable replaces the whole tool command. mutmut does not report lines, so its survivors show line 0. Survivors that stryker and pitest report as uncovered count as survived.
