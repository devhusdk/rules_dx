# `dx check`

```sh
dx check --affected
dx check --apply //pkg/...
dx check --output json --report sarif=out.sarif //pkg/...
```

## `dx check`

```text
dx check [--here] [--affected] [--no-retry] [--apply] [--diff] [--unsafe] [--include-edits] [--max-findings n] [--fail-on info|warning|error] [--report sarif=<dest>] [scope...] [-- bazel-options...]
```

Runs `generate`, `format`, `lint`, and `typecheck` in order. It only reports. `--apply` writes validated fixes, then re-checks read-only.

| Flag | What it does |
| --- | --- |
| `--apply` | Write fixes, then verify with a read-only check. |
| `--no-retry` | Do not run Bazel again without the remote cache after a remote cache error. |
| `--here` | Use the current directory tree. |
| `--affected` | Use the targets your git changes reach, then run the affected tests as a last phase. |
| `--diff` | Print the edits as a patch. |
| `--unsafe` | With `--apply` or `--diff`, also include the fixes tools mark unsafe. Without either, exit `2`. The read-only re-check after `--apply` runs without it. |
| `--include-edits` | Add full edits to JSON change records. Needs `--output json`. |
| `--max-findings <n>` | Print at most `n` findings per tool in JSON output (default `50`, `0` for all). Needs `--output json`. |
| `--fail-on <level>` | Fail on `info`, `warning`, or `error`. Applies to `format`, `lint`, and `typecheck`. |
| `--report sarif=<dest>` | Merge the `lint` and `typecheck` findings into one SARIF document. `-` is rejected. |
| `-o, --output text\|json` | Choose the output format. |
| `--workspace`, `-q`, `-v`, `--color`, `--build-slots`, `--build-slots-wait`, `--bazel-startup-option` | Global flags. |

Scopes: no scope means `//...`. `--here` and `--affected` do not combine with scopes. See [Scope defaults](scope-defaults.md). With nothing affected, dx prints `no affected targets`, runs no phase, and exits `0`. With `--diff`, the line goes to stderr and stdout stays empty.

### Phases

- Every phase runs even after an earlier phase reports findings. The run still exits nonzero.
- The run stops when the scope is bad, `generate` fails, Bazel cannot run, or you cancel.
- `check --apply` also stops when a phase fails to compile. Remaining phases are skipped.
- `--apply` reruns `generate` after `typecheck` when a phase wrote a file.
- `--affected` skips the test phase under `--apply`.
- `--affected` also lints the commit subjects since the merge base when `dx.toml` sets `[check] commits = "conventional"`. Each subject that is not a conventional commit is a `warning` with its sha, and the run exits `1`. Merge commits are skipped. Left out, the key is off. It runs when nothing is affected too. Not run under `--apply`.
- A failed run ends with a `dx: phases:` line on stderr.

### Build output check

Before any phase, `dx check` fails when your change adds build output. That is a new file under `target/`, `bazel-*`, `node_modules/`, `__pycache__/`, `.gradle/`, or a `build/` directory, or a file over 1 MiB. List paths or `dir/` prefixes to keep in `[large_files] allow` in `dx.toml`; lock files such as `MODULE.bazel.lock` are always skipped. The check is skipped outside a Git checkout.

### Output

Text prints `N file(s) need changes. Apply them with: dx check --apply ...` when a phase finds changes, and one `dx: <total> actions, <executed> executed, <cached> cached` line on stderr.

JSON has one `command_started` and one `command_finished`. Each phase sits between `phase_started` and `phase_finished`, and its events carry `phase` and a `correlation` such as `check/format`. `--apply` verification phases use `verify/<phase>`.

SARIF results carry `partialFingerprints`, `ruleIndex`, and `fixes`. A phase that ran but left no readable SARIF is a `collection_failed` error and marks the report incomplete.

### Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Success. |
| `1` | A phase failed or found changes, a capture was lost, verification failed, or a report could not be written. |
| `2` | Usage or scope error. |
| Bazel's code | The phase that stopped the run keeps its code. |

The test phase keeps Bazel's code. `3` with complete results means tests failed. `3` or `1` with incomplete results means the phase is incomplete. `2` and `4` mean the scope selected no runnable tests.
