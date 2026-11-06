# `dx explain`

Lists the actions that missed the cache by comparing two Bazel execution logs. It never runs Bazel and changes no files.

```sh
dx build --explain //app:lib
dx explain
dx explain --old=/path/to/other/worktree/.dx/local/explain/3.binpb.zst
dx explain --output=json
```

```text
dx explain [--old=<log>] [--new=<log>]
```

Save a log with `--explain` on `dx build`, `dx test`, `dx coverage`, or `dx check`. Each run writes `.dx/local/explain/<n>.binpb.zst` and keeps the two newest.

| Flag | What it does |
| --- | --- |
| `--old=<log>` | The older log. Default: the second newest saved log. |
| `--new=<log>` | The newer log. Default: the newest saved log. |
| `-o, --output text\|json` | Choose the output format. |
| `--workspace`, `-q`, `-v`, `--color`, `--build-slots`, `--bazel-startup-option` | Global flags. |

With one flag, the other log is the newest saved log that is not the same file. Relative paths resolve against the workspace root.

Scopes: none. Targets and `-- bazel-options` are usage errors.

## Output

Text prints one line per action, `<mnemonic> <target> <primary output>`, then one indented line per change, then `<n> actions ran`. Each action shows at most 20 changes, then `... <n> more changes`.

| Change | Meaning |
| --- | --- |
| `input added\|removed\|modified <path>` | An input changed. |
| `env added\|removed\|modified <name>` | An environment variable changed. Values are never shown. |
| `arg added\|removed <value>`, `arg order_changed` | The arguments changed. |
| `platform added\|removed\|modified <name>` | An execution platform property changed. |
| `not_in_old_log` | The older log has no action with this output, or this target and mnemonic. |
| `not_cacheable` | The action is marked no-cache. |

JSON emits `command_started`, one `cache_miss` event per action with `mnemonic`, `target`, `primary_output`, and `changes`, then `command_finished`.

With `--explain`, a Bazel option after `--` that sets another execution log is refused before Bazel runs.

Reports: none.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Success, even when actions missed the cache. |
| `1` | `explain_log_missing`: fewer than two logs exist or a named log is missing. `explain_log_unreadable`: a log is not a valid execution log. |
| `2` | Usage error. |
