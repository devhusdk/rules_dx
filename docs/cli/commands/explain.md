# `dx explain`

```text
dx explain [--old=<log>] [--new=<log>]
```

Explains why actions missed the cache. It compares two Bazel execution logs
and lists each action in the newer log that ran without a cache hit. It never
runs Bazel and changes no files.

Save a log with `dx build --explain` or `dx test --explain`. Each run writes
`.dx/explain/<n>.binpb.zst` and keeps the two newest logs.

Flags:

- `--old=<log>`: the older log. Default: the second newest saved log.
- `--new=<log>`: the newer log. Default: the newest saved log.

With only one flag, the other log is the newest saved log that is not the
same file. Relative paths resolve against the workspace root, so a log from
another worktree works with an absolute path.

Scopes: none. Targets and `-- bazel-options` are usage errors.

Output: `--output text|json`. Text prints one line per action,
`<mnemonic> <target> <primary output>`, then one indented line per change,
then `<n> actions ran`. A change line is `<kind> <change> <subject>`:

- `input added|removed|modified <path>`
- `env added|removed|modified <name>`. Values are never shown.
- `arg added|removed <value>`, or `arg order_changed`.
- `platform added|removed|modified <name>`
- `not_in_old_log`: no action in the older log has the same primary output
  or the same target and mnemonic.
- `not_cacheable`: the action is marked no-cache.

Each action shows at most 20 changes, then `... <n> more changes`.

`json` emits `command_started`, one `cache_miss` event per action, and
`command_finished`. A `cache_miss` event has `mnemonic`, `target`,
`primary_output`, and `changes`. Each change has `kind`, plus `change` and
`path`, `name`, or `value` for the kinds above. `changes_omitted` appears
when changes were cut.

Actions the local action cache skips are not in a log. When such an action
runs later, it shows `not_in_old_log`.

With `--explain`, a Bazel option after `--` that sets another execution log
is refused before Bazel runs. Bazel rejects the run when `.bazelrc` sets
`--execution_log_binary_file` or `--execution_log_json_file`.

Reports: none.

Exit codes: `0` success, even when actions missed the cache. `2` usage
errors. `1` `explain_log_missing` when fewer than two logs exist or a named
log does not exist, and `explain_log_unreadable` when a log is not a valid
compact execution log.

```sh
bazel run @rules_dx//:dx -- build --explain //app:lib
bazel run @rules_dx//:dx -- build --explain //app:lib
bazel run @rules_dx//:dx -- explain
bazel run @rules_dx//:dx -- explain --old=/path/to/other/worktree/.dx/explain/3.binpb.zst
bazel run @rules_dx//:dx -- explain --output=json
```
