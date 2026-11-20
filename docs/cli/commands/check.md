# `dx check`

Use `dx check` to find and fix generated-file drift, formatting, lint and type errors in one run before you push.

```sh
dx check --affected
dx check --apply --affected
dx check --apply //pkg/...
dx check --diff //pkg/...
dx check --output json --report sarif=out.sarif //pkg/...
```

`dx check` runs `dx generate`, `dx format`, `dx lint` and `dx typecheck` in that order. By default it only reports. `--apply` writes the fixes, then checks again without writing.

## Usage

```text
dx check [--here] [--affected [--since <ref>|--base <ref>]] [--no-retry] [--apply] [--diff] [--unsafe] [--include-edits] [--max-findings <n>] [--fail-on info|warning|error] [--report sarif=<dest>] [scope...] [-- bazel-options...]
```

Args after `--` go to Bazel.

## Flags

| Flag | What it does |
| --- | --- |
| `--apply` | Write fixes, then verify with a read-only check. |
| `--diff` | Print the edits as a patch. |
| `--unsafe` | With `--apply` or `--diff`, also include fixes the tools mark unsafe. Without either, exits `2`. The read-only check after `--apply` runs without it. |
| `--here` | Use the current directory tree: `//path/...`, or `//...` at the workspace root. |
| `--affected` | Use only the targets your git changes reach, then run their tests as a last phase. |
| `--since <ref>` | With `--affected`: compare against this git ref instead of the merge-base with the default branch. Without `--affected`, exits `2`. |
| `--base <ref>` | With `--affected`: take the merge-base with this branch instead of the default branch. Not with `--since`. |
| `--fail-on info\|warning\|error` | Fail on findings of this severity or worse. Applies to `format`, `lint` and `typecheck`. |
| `--include-edits` | Add full edits to JSON change records. Needs `--output json`. |
| `--max-findings <n>` | Print at most `n` findings per tool in JSON output. Default `50`. `0` means no limit. Needs `--output json`. |
| `--no-retry` | Do not run Bazel again without the remote cache after a remote cache error. |
| `--report sarif=<dest>` | Merge the `lint` and `typecheck` findings into one SARIF file. Repeatable. `-` (stdout) is rejected. |
| `-o, --output text\|json` | Choose the output format. |
| `--workspace <dir>` | Use this workspace directory. |
| `-q, --quiet` | Hide summaries. |
| `-v, --verbose` | Show more logs. |
| `--color auto\|always\|never` | Set color output. |
| `--build-slots <auto\|n\|off>` | Set build slots: `auto`, a count, or `off`. |
| `--build-slots-wait <duration>` | Give up waiting for a build slot after this long. `0` fails at once. |
| `--bazel-startup-option <token>` | Pass one Bazel startup option. Repeatable. |

## Scopes

A scope picks what to check. Pass a label such as `//pkg:target` (one Bazel target), a pattern such as `//pkg/...` (every target in `pkg` and below), or a workspace file or directory. With no scope, dx checks `//...`, every target in the workspace.

- `--here` uses the current directory tree.
- `--affected` uses the targets changed since the merge-base with the default branch (or with `--base <ref>`), or since `--since <ref>`, plus the targets that depend on them.
- `--here` does not combine with scopes or `--affected`.
- `--affected` with scopes keeps only the affected targets inside them.
- With nothing affected, dx prints `no affected targets`, runs no phase, and exits `0`. With `--diff`, that line goes to stderr and stdout stays empty.
- The generate phase writes only the `BUILD` files of packages in scope. A file or `//pkg:target` is its own package. `//pkg/...` or a directory is the tree under it. `--affected` is the directories of the changed files.
- With `--affected`, the other phases write only the changed files and the `BUILD` file of each one's package. A fix to any other file is refused as `outside_scope`.

Read [Scope defaults](scope-defaults.md) for details.

## Phases

- Every phase runs, even after an earlier phase reports findings. The run still exits nonzero.
- `generate` compares each lock that a rules_dx lock step writes in a package the scope reaches with its manifest, offline: `<ecosystem>.lock.json`, `pubspec.lock` and a dx-written `maven_install.json`. A `maven_install.json` that rules_jvm_external pins is skipped. A `pubspec.lock` or `maven_install.json` without the dx marker is stale until one `dx generate --apply`. A stale lock prints `dx: lock of <set> is stale; run dx generate --apply` and fails the run.
- Without `--apply`, `format`, `lint` and `typecheck` share one Bazel build. Each phase still reports only its own findings and exit code.
- The run stops when the scope is bad, `generate` fails, Bazel cannot run, or you cancel.
- With `--apply`, the run also stops when a phase fails to compile. The remaining phases are skipped.
- With `--apply`, dx runs `generate` again after `typecheck` when a phase wrote a file.
- `--affected` adds a test phase. `--apply` skips it.
- A failed run ends with a `dx: phases:` line on stderr.

### Commit messages

Set this in `dx.toml` to lint commit subjects:

```toml
[check]
commits = "conventional"
```

Then `dx check --affected` checks every commit subject since the merge-base. Each subject that is not a conventional commit is a `warning` with its sha, and the run exits `1`. Merge commits are skipped. The check runs even when nothing is affected. It does not run under `--apply`. Without the key, it is off.

### Build output check

Before any phase, `dx check` fails when your change adds build output or a large file:

- a new file under `target/`, `node_modules/`, `__pycache__/`, `.gradle/`, or a top-level `bazel-*` directory;
- a new file under a `build/` directory that has no `BUILD`, `BUILD.bazel`, `pom.xml`, `CMakeLists.txt` or `build.gradle` in it;
- a changed file over 1 MiB. Lock files such as `MODULE.bazel.lock` are exempt.

To keep such a path, list it in `dx.toml`. An entry ending in `/` keeps everything under that directory:

```toml
[large_files]
allow = ["assets/video.mp4", "testdata/big/"]
```

Outside a Git checkout, this check is skipped.

## Output

Text prints one line per finding, such as `src/lib.rs:12:5: warning: indexing may panic [clippy/clippy::indexing_slicing]`: the path, line, column, severity, message, and `[tool/rule]`. Coding agents get JSON by default; pass `--output text` to see these lines.

Text also prints `N file(s) need changes. Apply them with: dx check --apply ...` when a phase finds changes. It ends with `dx: <total> actions, <executed> executed, <cached> cached` on stderr.

JSON has one `command_started` and one `command_finished` event. Each phase sits between `phase_started` and `phase_finished`. Its events carry `phase` and a `correlation` such as `check/format`. The verify phases after `--apply` use `verify/<phase>`. JSON streams events while the command runs.

## Reports

`--report sarif=<dest>` writes one SARIF file with the `lint` and `typecheck` findings. Results carry `partialFingerprints`, `ruleIndex` and `fixes`. A phase that ran but left no readable SARIF is a `collection_failed` error and marks the report incomplete.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Success. |
| `1` | A phase failed or found changes, a capture was lost, verification failed, a commit subject failed, or a report could not be written. |
| `2` | Usage or scope error. |
| Bazel's code | The phase that stopped the run keeps Bazel's code. |

The test phase keeps Bazel's code:

- `3` with complete results means tests failed.
- `3` or `1` with incomplete results means the phase is incomplete.
- `2` and `4` mean the scope selected no runnable tests.
