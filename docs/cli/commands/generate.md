# `dx generate`

```sh
bazel run @rules_dx//:dx -- generate
bazel run @rules_dx//:dx -- generate --apply //...
bazel run @rules_dx//:dx -- generate --here
bazel run @rules_dx//:dx -- generate --output json //pkg/...
bazel run @rules_dx//:dx -- generate --apply //... -- --jobs=4
```

Updates `BUILD` declarations from source with Gazelle over the scope. To
materialize declared generated source, use [`dx env`](env.md). No scope means the whole
repo. Use `--here` for the current dir tree. Checks by default and reports
stale files without writing. Pass `--apply` to write them. Bazel options go
after `--`.

```text
dx generate [--here] [--apply] [scope...] [-- bazel-options...]
```

- `--apply`: write the updated BUILD files. Without it files stay untouched and stale files fail the run.
- `--fail-on`, `--report`, `--affected`: rejected as usage errors (exit `2`).
- `--output text|json`: result shape.
- `--diff`: print one unified patch per stale file, or per file `--apply` wrote
  or did not apply.

Exit codes: `0` success, `2` usage or scope errors, `1` stale files with
the default, a file `--apply` did not write, incomplete results, a
missing or invalid generation result (`invalid_result`), a generation error
reported by a language extension, or a patch that fails to render
(`diff_failed`). Bazel failures keep Bazel's code.

## Output

Text starts with `Running generate for <scope>`. `--quiet` hides it. Then it
prints `Created <path>` or `Modified <path>` for each written file, or each
stale file without `--apply`, sorted by path. Each ignored import prints
`Ignored import '<import>' in <path> [<language>]: Static import intentionally contributes no Bazel dependency`.
Files that were not written go to stderr as `Not applied: <path> (<reason>)`.
An incomplete `--apply` run still prints the files it wrote and the
`Not applied` lines. An incomplete check run, or a Bazel failure that
leaves no result, prints no file lines.

JSON starts with `command_started` with `mode` `check`, or `default` with
`--apply`. Then one `change` per file with `path`, `kind` (`create` or
`modify`), `edit_count`, and `effect` (`proposed`, `applied`, or `refused`).
A refused change has a `reason`: `write_mismatch`, `missing_file`, or
`unreadable_file`. Each ignored import emits a `notice` with `code`
`ignored_import`, `level` `warning`, `path`, `language`, and `import`. A Bazel
failure with `--apply`, or one that leaves results incomplete, emits a
`bazel_failed` error. So does a Gazelle failure without `--apply`, including a
nonzero exit that does not come with a Gazelle diff. A generation error from a
language extension emits one `generation_failed` error per entry instead, with
`message`, `phase` `execute`, and `path` when the failing package is known. It
is not followed by a `bazel_failed` event, and the results are incomplete.
Text output is unchanged: Gazelle's stderr lists every error. `command_finished` ends the stream with
`results_complete`, `exit_code`, and `changes` with `create` and `modify`
counts, plus `applied` and `refused` with `--apply`. An incomplete `--apply`
run still emits its `change` events and `changes` with `applied` and `refused`.
An incomplete check run, or a Bazel failure that leaves no result, emits no
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
away, so delete a stale value by hand. After an upgrade, `dx generate` can report
an existing NDK or JNI `cc_library` as stale.

The `java` and `kotlin` languages generate an Android library kind for
directories that import the Android SDK:

- An `android.*` or `dalvik.*` import selects `java_android_library` or
  `kotlin_android_library`, loaded from `@rules_dx//android/rules:defs.bzl`,
  instead of `java_library` or `kotlin_library`. `android.support.*`,
  `android.arch.*` and `androidx.*` are Maven artifacts and don't select it.
- In those directories `android.*`, `dalvik.*`, `org.json.*` and
  `org.xmlpull.v1.*` imports need no dependency. A `# gazelle:resolve`
  mapping still wins.
- Gazelle sets `custom_package` to the source package and
  `tags = ["manual"]` only when the rule doesn't have them. The sources must
  declare a package.
- A rule of the other kind with the same name fails generation instead of
  changing kind. Handwritten rules_android `android_library` rules are left
  alone.
- Resources, the `R` class and manifests are never derived.

Generation never writes `app_id`, permissions, `api_level`, `platforms`, signing or
app topology. The packaging rules validate ABIs.

For a `rust_android_apk` with a custom `manifest` file in the same package,
`dx generate` checks the manifest against the rule's literal attributes. It
exits 1 and names the rule when the manifest is malformed, when its `package`
differs from `custom_package` (or `app_id` when `custom_package` is unset), or
when its `minSdkVersion` differs from a non-zero `api_level`. Generated
manifests, labels from other packages and `select` values are skipped.

## Dart and Flutter

A directory with a `pubspec.yaml` gets a `dart_library` named after the
package. It holds every `lib/**/*.dart` file and sets `package_name`. Each
`bin/*.dart` gets a `dart_binary`, and each `test/**/*_test.dart` gets a
`dart_test`. Files in `bin/` subdirectories and `test/` files that don't end in
`_test.dart` are shared sources of every binary or test.

A test is named by its path under `test/`, with `.dart` removed and `/`
replaced by `_`. `test/ui/app_test.dart` becomes `ui_app_test`.

- A `package:` import must name a dependency in `pubspec.yaml`, or a
  `dev_dependencies` entry for a test. Other packages fail generation.
- `dart:` imports need no dependency. Relative imports must stay inside the
  package and must exist, or name an output of a `genrule` in the same BUILD
  file.
- A `BUILD` file below `lib/`, `bin/` or `test/` fails generation.

When `pubspec.yaml` depends on the `flutter` SDK, the library becomes a
`flutter_library` and each test becomes a `flutter_test`. Both load from
`@rules_dx//flutter/rules:defs.bzl`. `bin/` is not generated.

## Test targets

For Java and Kotlin, each `*Test.java` or `*Test.kt` source gets its own
`java_test` or `kotlin_test`. The target name is the file stem in snake case,
so `GreetTest.java` becomes `greet_test`. `test_class` is the class qualified
by its `package` line. When the directory
also has a library, the test depends on it. The test's own imports resolve
like library imports, except JUnit 4 (`org.junit`) in Java, which the test
runner already supplies. Kotlin tests resolve every import, JUnit included.

A handwritten test rule that already lists a test source in `srcs` owns it.
Generation then emits nothing for that source and keeps the rule. A `java_test`
whose `srcs` is a glob owns every test source in the directory; other test
kinds, such as an `sh_test`, own only the sources they list. Generation
replaces `deps` on a generated test, so mark any dependency that no import
gives, such as a runtime-only library, with `# keep`.

For Julia, a `Project.toml` with a `name` and a `src/<name>.jl` gets a
`julia_library` named `<name>` and, when `test/runtests.jl` exists, a
`julia_test` named `<name>_test`. Test dependencies come from
`test/Project.toml` `[deps]` when that file exists, otherwise from `[extras]`
filtered by `[targets].test`. `data` is never generated beyond `Project.toml`
on the library, so data you add survives.

JavaScript and TypeScript treat `*.test.*`, `*.spec.*` and `*_test.*` sources
as tests, matching the default patterns of Jest, Vitest and `node --test`. A
test source gets a `js_test` or `typescript_test` named after its stem without
the suffix, plus `_test`, so `a.spec.ts` becomes `a_test`. Imports still use the
full file name. Declaration files (`.d.ts`) are never tests. Scala sources
ending in `Test`, `Spec` or `Suite` get a `scala_test` with a snake-case name.

## Add a dependency

Add the package with its own tool, then run `dx generate --apply`:

```sh
cargo add serde
uv add requests
pnpm add left-pad
dx generate --apply
```

When the root `Cargo.lock`, `uv.lock` or `pnpm-lock.yaml` has its manifest next
to it and `MODULE.bazel` has no stanza for it, `dx init --apply` and
`dx generate --apply` add one: a `crates` hub, a `pypi` hub or an `npm` hub, plus
the `bazel_dep` it needs. A stanza you wrote is left alone. A stanza that names a
lock file that is gone is reported, not removed. Without `--apply`,
`dx generate` only says what it would add. Cargo workspace members written as
globs are reported; list them in `manifests` yourself. Lock files below the
root are not wired yet.

For each language family whose `rules_dx_<family>` module is published, the same
commands add `bazel_dep(name = "rules_dx_<family>")` when files of that family are
in the tree. A dep whose files are gone is reported, not removed.

## Directives

`# gazelle:dx_ignore_import <language> <import>` skips one import in this
directory and below. `dx generate` fails when no literal reference matches it,
when the language is unknown, or when the value is malformed. A scoped run
checks only directives in packages inside the scope.

Accepted languages: `astro`, `cc`, `csharp`, `dart`, `elixir`, `erlang`, `fsharp`, `go`,
`haskell`, `java`, `javascript`, `julia`, `kotlin`, `lua`, `mdx`, `ocaml`, `perl`, `php`,
`powershell`, `python`, `ruby`, `rust`, `scala`, `svelte`, `swift`, `typescript`, `vue`, `zig`.

`# gazelle:dx_native_tools <tool>...` picks the config tools that the Rust
language wires up for this directory and below. The tools are `buildifier`,
`taplo`, `vale` and `rustfmt`, and all four are on by default. Each selected
tool gets its config target when the directory holds its config file
(`.buildifier.json`, `taplo.toml`, `.vale.ini` or `rustfmt.toml`). An unknown
tool name or an empty value fails the run.

`# gazelle:dx_corpus true|false` turns the Rust corpus target on or off for this
directory and below. When it is `true`, generation emits `real_source_target`
corpus targets named `corpus`, `corpus_markdown`, `corpus_starlark`,
`corpus_toml` or `corpus_json`. Any other value fails the run.

`# gazelle:dx_cuda_platforms <spec> <spec>...` sets the `platforms` of a generated
`cuda_library`. It applies to this directory and below, and a deeper directive
replaces it. Separate specs with spaces, because a spec can contain commas, as
in `compute_89:compute_89,sm_89`. The `cc` language generates a `<dir>_cuda`
target for `.cu` and `.cuh` files that no hand-written CUDA rule lists, and it
fails when the directive is missing. Gazelle never touches hand-written
`cuda_library`, `cuda_binary`, or `cuda_test` targets. It sets
`platforms` only when it creates the target, so a hand-edited value survives later
runs. Sources named `*_test.cu` and sources that define `main` are not
generated, and `main` fails the run.

`# gazelle:lang` must list `dx_dispatch` (for example
`# gazelle:lang go,dx_dispatch`), or `dx generate` fails.

`# gazelle:dx_external <ecosystem> <path|off> [<repo>]` names the manifest or
lockfile that third-party imports resolve through, for this directory and
below. The path is relative to the repository root. Ecosystems: `go` (a
`go.mod`, no repo), `maven` (a rules_jvm_external `maven_install.json`,
repo `maven` by default), `python` (`uv.lock`, repo `pypi`), `pub`
(`pubspec.lock`, repo `pub_deps`) and `npm` (`package.json`, repo root only). Repeat the line to feed one ecosystem from several
files. The lines in one `BUILD` file replace what the directory inherited, and
`off` drops it. Without a directive, a `go.mod`, `maven_install.json`, `uv.lock`,
`pubspec.lock` or root `package.json` in a directory applies to that directory and below. `dx generate` fails on a
malformed line, an unknown ecosystem, a repo for `go`, or a file that is
missing or does not parse.

```starlark
# gazelle:dx_external maven third_party/jvm/maven_install.json maven
# gazelle:dx_external go off
```

`# gazelle:resolve <language> <import> <label>` maps one import to an exact
label.

```starlark
# gazelle:dx_ignore_import python requests
# gazelle:resolve python yaml @pypi//pyyaml
```

For `java`, `kotlin` and `scala`, a `resolve` mapping names the fully qualified
import or the import's own package, and a `dx_ignore_import` names the fully
qualified import. A wildcard import such as `kotlinx.coroutines.*` resolves
through its package. A bare class name such as `ImmutableList` does not
match, so a stale bare-name entry fails the run.

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
A bare last segment such as `Json` does not match, so a stale bare-name
entry fails the run. A namespace that more than one local library declares
fails as ambiguous.

## Third-party imports

An import resolves in this order:

1. An exact `# gazelle:resolve` mapping.
2. A library in the workspace.
3. A `# gazelle:dx_ignore_import` entry.
4. The `dx_external` sources for the language.
5. Otherwise the run fails with an unresolved import.

- `go`: an import under a module that `go.mod` requires directly resolves to
  `@<repo>//<package>`, with the repo named from the module path, as in
  `@com_github_google_go_cmp//cmp`. A module required only as `// indirect`
  fails with a hint to require it directly. A module replaced by a local path
  is skipped.
- `java`, `kotlin`, `scala`: an import resolves through the `packages` list in
  `maven_install.json` to `@<repo>//:<group>_<artifact>`, as in
  `@maven//:com_google_guava_guava`. The import itself is tried first, then its
  enclosing classes, then its own package. A package that more than one
  artifact ships fails as ambiguous; add a `resolve` mapping for it.
- `rust`: crates resolve through `Cargo.toml`.
- `python`: an import whose top-level module matches the name of a package in
  `uv.lock` resolves to `@pypi//<hub name>`. The hub name is the distribution
  name in lowercase, with each run of `-`, `_` or `.` replaced by `_`. So
  `typing-extensions` becomes `@pypi//typing_extensions` and `zope.interface`
  becomes `@pypi//zope_interface`. Name a lockfile with
  `# gazelle:dx_external python <path> [<repo>]`. A package whose import name
  differs from its distribution name needs a `resolve` mapping.
- `javascript`, `typescript`: an import of a package that the root
  `package.json` lists in any dependencies field resolves to
  `//:node_modules/<package>`. Only the root `package.json` is read.
- `julia`: the `[deps]` of `Project.toml` are the dependencies. A standard
  library, matched by name and UUID, needs nothing. A name with a standard
  library name but another UUID fails. A local `julia_library` with that name
  resolves to itself. Anything else needs one `resolve` line, as in
  `# gazelle:resolve julia Example @<hub>//:Example`.
- `csharp`, `fsharp`, `lua`, `ruby`, `perl`, `php` and the
  other languages need one `# gazelle:resolve` line per third-party import.

```starlark
# gazelle:resolve ruby rspec @bundle
# gazelle:resolve csharp Newtonsoft.Json @paket.main//newtonsoft.json
```

## Python hub

The `@pypi` hub is a `rules_python` pip hub. Export the lockfile to a
requirements file, then parse it in `MODULE.bazel`:

```sh
uv export --format requirements.txt --no-hashes --no-emit-project > requirements_lock.txt
```

```starlark
bazel_dep(name = "rules_python", version = "2.4.6")

python = use_extension("@rules_python//python/extensions:python.bzl", "python")
python.toolchain(python_version = "3.12")

pip = use_extension("@rules_python//python/extensions:pip.bzl", "pip")
pip.parse(
    hub_name = "pypi",
    python_version = "3.12",
    requirements_lock = "//:requirements_lock.txt",
)
use_repo(pip, "pypi")
```

`python_version` in `pip.parse` must match the toolchain version.
