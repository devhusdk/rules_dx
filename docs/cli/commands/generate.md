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
`bazel_failed` error. So does a Gazelle failure with `--check`, including a
nonzero exit that does not come with a Gazelle diff. `command_finished` ends the stream with
`results_complete`, `exit_code`, and `changes` with `create` and `modify`
counts, plus `applied` and `refused` with `--apply`. An incomplete `--apply`
run still emits its `change` events and `changes` with `applied` and `refused`.
An incomplete `--check` run, or a Bazel failure that leaves no result, emits no
`change` events and no `changes`.

## Android inputs

The `cc` language reads `<android/*.h>` includes and native entry points in a
generated `cc_library`:

- An `<android/*.h>` include outside any `__ANDROID__` conditional sets
  `target_compatible_with = ["@platforms//os:android"]` and plain `linkopts`.
  `<android/log.h>` adds `-llog`. Native activity, looper, input, asset,
  configuration, sensor and window headers add `-landroid`.
- When every such include sits under an `__ANDROID__` conditional, `linkopts`
  becomes a `select` on `@platforms//os:android` and the target stays
  host-compatible.
- A definition of `ANativeActivity_onCreate`, `JNI_OnLoad` or a `Java_*`
  function sets `alwayslink = True`. `<jni.h>` alone sets nothing else.

Gazelle sets these attributes only when the rule doesn't have them. Set one
yourself to keep your value. Gazelle never removes them when the include goes
away, so delete a stale value by hand. After an upgrade, `--check` can report
an existing NDK or JNI `cc_library` as stale.

Generation never writes `app_id`, permissions, `api_level`, `abis`, signing or
app topology. The packaging rules validate ABIs.

For a `rust_android_apk` with a custom `manifest` file in the same package,
`dx generate` checks the manifest against the rule's literal attributes. It
exits 1 and names the rule when the manifest is malformed, when its `package`
differs from `custom_package` (or `app_id` when `custom_package` is unset), or
when its `minSdkVersion` differs from a non-zero `api_level`. Generated
manifests, labels from other packages and `select` values are skipped.

## Directives

`# gazelle:dx_ignore_import <language> <import>` skips one import in this
directory and below. `dx generate` fails when no literal reference matches it,
when the language is unknown, or when the value is malformed. A scoped run
checks only directives in packages inside the scope.

Accepted languages: `astro`, `cc`, `csharp`, `elixir`, `fsharp`, `gleam`, `go`,
`java`, `javascript`, `kotlin`, `mdx`, `powershell`, `python`, `ruby`, `rust`,
`scala`, `svelte`, `typescript`, `vue`, `zig`.

`# gazelle:dx_cuda_archs <spec> <spec>...` sets the `archs` of a generated
`cuda_library`. It applies to this directory and below, and a deeper directive
replaces it. Separate specs with spaces, because a spec can contain commas, as
in `compute_89:compute_89,sm_89`. The `cc` language generates a `<dir>_cuda`
target for `.cu` and `.cuh` files that no hand-written CUDA rule lists, and it
fails when the directive is missing. Gazelle never touches hand-written
`cuda_library`, `cuda_binary`, `cuda_test` or `cuda_gpu_test` targets. It sets
`archs` only when it creates the target, so a hand-edited value survives later
runs. Sources named `*_test.cu` and sources that define `main` are not
generated, and `main` fails the run.

`# gazelle:lang` must list `dx_dispatch` (for example
`# gazelle:lang go,dx_dispatch`), or `dx generate` fails.

`# gazelle:resolve <language> <import> <label>` maps one import to an exact
label.

```starlark
# gazelle:dx_ignore_import python requests
# gazelle:resolve python yaml @pypi//pyyaml
```

For `java`, `kotlin` and `scala`, a `resolve` mapping names the fully qualified
import or the import's own package, and a `dx_ignore_import` names the fully
qualified import. A wildcard import such as `kotlinx.coroutines.*` resolves
through its package. A bare class name such as `ImmutableList` no longer
matches, so a stale bare-name entry fails the run.

```starlark
# gazelle:resolve kotlin kotlinx.coroutines @maven//:org_jetbrains_kotlinx_kotlinx_coroutines_core
# gazelle:resolve kotlin com.google.common.collect.ImmutableList @maven//:com_google_guava_guava
```

For `javascript` and `typescript`, a bare import such as `@/lib/util` also
resolves through `compilerOptions.paths` and `baseUrl` in the nearest
`tsconfig.json`, else `jsconfig.json`, in the package or its parents.
`extends` (a string or an array) and `${configDir}` work as in TypeScript.
The order is: a `resolve` mapping for the bare import, then each alias
target (the exact `paths` key, else the wildcard with the longest prefix,
then `baseUrl`) as a mapping, a library, or `<target>/index`, then a
library for the bare import. Alias targets outside the workspace are
skipped. A malformed config fails the run. Once an alias resolves, a
`dx_ignore_import` for it is stale and fails the run.

For `csharp`, a `using` resolves by the full namespace that a local
`csharp_library` declares, or by that namespace plus a source's file name.
A `using static` or alias directive may name a type, so when its target
matches no library it also tries the parent namespace. A `resolve` mapping
or a `dx_ignore_import` names the full namespace, such as `Newtonsoft.Json`.
A bare last segment such as `Json` no longer matches, so a stale bare-name
entry fails the run. A namespace that more than one local library declares
fails as ambiguous.
