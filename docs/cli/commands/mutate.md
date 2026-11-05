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
