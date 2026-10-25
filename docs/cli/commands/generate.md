# `dx generate`

```sh
bazel run @rules_dx//:dx -- generate
bazel run @rules_dx//:dx -- generate --apply //...
bazel run @rules_dx//:dx -- generate --here
bazel run @rules_dx//:dx -- generate --output json //pkg/...
bazel run @rules_dx//:dx -- generate --apply //... -- --jobs=4
```

Updates `BUILD` declarations from source with Gazelle over the scope. To
materialize declared generated source, use `dx codegen`. No scope means the whole
repo. Use `--here` for the current dir tree. Checks by default and reports
stale files without writing. Pass `--apply` to write them. Bazel options go
after `--`.

```text
dx generate [--here] [--check] [--apply] [scope...] [-- bazel-options...]
```

- `--check`: fail if files are stale instead of writing them. Same as the
  default.
- `--apply`: write the updated BUILD files. Without it files stay untouched.
- `--fail-on`, `--report`, `--affected`: rejected as usage errors (exit `2`).
- `--output text|diff|json`: result shape. `diff` prints one unified patch per
  stale file, or per file `--apply` wrote or did not apply.

Exit codes: `0` success, `2` usage or scope errors, `1` stale files with
`--check` or the default, a file `--apply` did not write, incomplete results, a
missing or invalid generation result (`invalid_result`), or a patch that fails
to render (`diff_failed`). Bazel failures keep Bazel's code.

## Output

Text starts with `Running generate for <scope>`. `--quiet` hides it. Then it
prints `Created <path>` or `Modified <path>` for each written file, or each
stale file with `--check`, sorted by path. Each ignored import prints
`Ignored import '<import>' in <path> [<language>]: Static import intentionally contributes no Bazel dependency`.
Files that were not written go to stderr as `Not applied: <path> (<reason>)`.
An incomplete `--apply` run still prints the files it wrote and the
`Not applied` lines. An incomplete `--check` run, or a Bazel failure that
leaves no result, prints no file lines.

JSON starts with `command_started` with `mode` `check`, or `default` with
`--apply`. Then one `change` per file with `path`, `kind` (`create` or
`modify`), `edit_count`, and `effect` (`proposed`, `applied`, or `refused`).
A refused change has a `reason`: `write_mismatch`, `missing_file`, or
`unreadable_file`. Each ignored import emits a `notice` with `code`
`ignored_import`, `level` `warning`, `path`, `language`, and `import`. A Bazel
failure with `--apply`, or one that leaves results incomplete, emits a
`bazel_failed` error. `command_finished` ends the stream with
`results_complete`, `exit_code`, and `changes` with `create` and `modify`
counts, plus `applied` and `refused` with `--apply`. An incomplete `--apply`
run still emits its `change` events and `changes` with `applied` and `refused`.
An incomplete `--check` run, or a Bazel failure that leaves no result, emits no
`change` events and no `changes`.

## Directives

`# gazelle:dx_ignore_import <language> <import>` skips one import in this
directory and below. `dx generate` fails when no literal reference matches it,
when the language is unknown, or when the value is malformed.

Accepted languages: `astro`, `cc`, `csharp`, `fsharp`, `gleam`, `go`, `java`,
`javascript`, `kotlin`, `mdx`, `python`, `ruby`, `rust`, `scala`, `svelte`,
`typescript`, `vue`, `zig`.

`# gazelle:resolve <language> <import> <label>` maps one import to an exact
label.

```starlark
# gazelle:dx_ignore_import python requests
# gazelle:resolve python yaml @pypi//pyyaml
```

For `java`, `kotlin`, and `scala`, imports keep their full names. A `resolve` or
`dx_ignore_import` value is a full name, or for `resolve` a package such as
`cats` or `scala.meta`. Simple class names do not match. Built in means the
JDK, the packages in `kotlin-stdlib` and `org.jetbrains.annotations`, and the
packages in `scala-library` and `scala-reflect`. Other namespaces need a
mapping, even under `kotlin.` or `scala.`: for example `kotlin.reflect.full`,
`kotlin.test`, `kotlin.metadata`, `scala.xml`, `scala.collection.compat`, and
`scala.meta`. A `resolve` mapping on a built-in name or package wins. An
unmapped import fails generation.

```starlark
# gazelle:resolve java com.google.common.collect.ImmutableList @maven//:com_google_guava_guava
# gazelle:resolve kotlin kotlinx.coroutines @maven//:org_jetbrains_kotlinx_kotlinx_coroutines_core_jvm
# gazelle:resolve scala scala.xml @maven//:org_scala_lang_modules_scala_xml_2_13
```
