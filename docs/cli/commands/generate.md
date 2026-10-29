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
missing or invalid generation result (`invalid_result`), a generation error
reported by a language extension, or a patch that fails to render
(`diff_failed`). Bazel failures keep Bazel's code.

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
nonzero exit that does not come with a Gazelle diff. A generation error from a
language extension emits one `generation_failed` error per entry instead, with
`message`, `phase` `execute`, and `path` when the failing package is known. It
is not followed by a `bazel_failed` event, and the results are incomplete.
Text output is unchanged: Gazelle's stderr lists every error. `command_finished` ends the stream with
`results_complete`, `exit_code`, and `changes` with `create` and `modify`
counts, plus `applied` and `refused` with `--apply`. An incomplete `--apply`
run still emits its `change` events and `changes` with `applied` and `refused`.
An incomplete `--check` run, or a Bazel failure that leaves no result, emits no
`change` events and no `changes`.

## Codegen pairs

The codegen plan admits the generator/language pairs `("protobuf", "rust")`,
`("flatbuffers", "cpp")`, `("flatbuffers", "rust")`, `("flatbuffers", "java")`, `("capnp", "cpp")` and `("openapi", "typescript")`.

The openapi/typescript pair comes from `openapi_typescript_client` in
`@rules_dx//api_spec/rules:codegen.bzl`, which turns one OpenAPI root of an
`api_spec_library` into a single self-contained `NAME.ts` with the pinned
`@redocly/cli` `generate-client`. Every generator flag is fixed in the rule
(`--output-mode=single --runtime=inline --import-ext=js --generator=typescript
--args-style=grouped --error-mode=throw --date-type=string`), the config is dx's
own, and the action runs without network. `root` is required when the spec has
more than one root. `openapi_codegen_shard` binds one plan entry with an
`EXEC_PATH` suffix to the generated file.

A dx preflight runs before Redocly and fails the build with
`path:line:col rule message` when the root is not an OpenAPI 3.0, 3.1 or 3.2
document (AsyncAPI and Swagger roots are refused), when a `$ref` names a remote
document (`dx/api-spec-remote-ref`), or when a `$ref` names a file the spec does
not declare (`dx/api-spec-unresolved-ref`). Only OpenAPI is supported; AsyncAPI
generation is a separate pair. Verify generated sources with
`dx codegen --check`, which fails with "no current selection" until
`dx codegen --apply` has run and with "stale selection" after the plan changes.

The flatbuffers/cpp pair comes from `flatbuffers_cc_library` in
`@rules_dx//flatbuffers/rules:codegen.bzl`, which runs the pinned `flatc`
(`--cpp --keep-prefix`) once per `flatbuffers_library` schema over its declared
include closure, and from `flatbuffers_codegen_shard`, which binds one plan entry
with an `EXEC_PATH` suffix to each generated header. The headers are compiled
against the C++ runtime of the same pinned archive, so the compiler and the
runtime cannot drift. `gen_object_api = True` adds the object API; each variant writes under its own `_fbs` or `_fbs_obj` directory, so both can share one library. The shard
lists the headers of the libraries in the `deps` of the `flatbuffers_cc_library`
only; a library reached through those deps needs its own target and shard.

The flatbuffers/rust pair comes from `flatbuffers_rust_library` in the same file.
It makes one crate per root schema: `flatc --rust --gen-all` runs on the `root`
(optional when the library owns one schema, otherwise required and one of its
srcs) and writes one `<root>_generated.rs` that inlines the include closure,
because flatc's Rust output uses `crate::` and `super::` paths that only compile
inside one crate. The crate links the Rust runtime built from the same pinned
archive. Bind the shard to the `NAME_generated` target, not to `NAME`; it binds
only the flatc file, never the `lib.rs` dx writes. Two crates whose roots share a
schema each get their own copy of its types, and those copies are not
interchangeable.

The flatbuffers/java pair comes from `flatbuffers_java_library`, which runs
`flatc --java` once per `flatbuffers_library` (one compile per library, shared by
every library that depends on it), packs the output into
`_fbs_java/<lib>.srcjar` and compiles it with `java_common.compile`. The Java
runtime is built from the same pinned archive and not taken from Maven, because
the latest `flatbuffers-java` on Maven (25.2.10) is older than the generated
code's version check. Bind `flatbuffers_codegen_shard` with `language = "java"`
to the `flatbuffers_java_library`; each entry's `EXEC_PATH` is the srcjar of one
direct dep. For breaking-change checks see `flatbuffers_conform_test` in
[quality](quality.md#breaking-change-check).

The capnp/cpp pair comes from `capnp_cc_library` in
`@rules_dx//capnproto/rules:codegen.bzl`, which runs the pinned `capnp compile`
with `capnpc-c++` once per `capnp_library` and compiles the output once per
library, so two C++ targets that share a schema library never link duplicate
symbols. The declared import closure (the library's schemas, its deps and the
bundled `/capnp/*.capnp`) is staged as symlinks under a private root and compiled
with `--no-standard-import`, so an import outside `deps` fails with or without a
sandbox. Relative imports resolve inside that root, but the generated headers
include them relatively, so a relative import must stay in the importing
schema's own directory; use an absolute `/pkg/file.capnp` import across packages.
`capnp_codegen_shard` binds one plan entry with an `EXEC_PATH` suffix to each
generated `.capnp.h` and `.capnp.c++` file; the shard lists the files of the
libraries in the `deps` of the `capnp_cc_library` only. The pin is built for
Linux x86_64 only, as for `dx quality`; Rust and managed-language generation are
not part of this pair yet.

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

Generation never writes `app_id`, permissions, `api_level`, `abis`, signing or
app topology. The packaging rules validate ABIs.

For a `rust_android_apk` with a custom `manifest` file in the same package,
`dx generate` checks the manifest against the rule's literal attributes. It
exits 1 and names the rule when the manifest is malformed, when its `package`
differs from `custom_package` (or `app_id` when `custom_package` is unset), or
when its `minSdkVersion` differs from a non-zero `api_level`. Generated
manifests, labels from other packages and `select` values are skipped.

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

## Directives

`# gazelle:dx_ignore_import <language> <import>` skips one import in this
directory and below. `dx generate` fails when no literal reference matches it,
when the language is unknown, or when the value is malformed. A scoped run
checks only directives in packages inside the scope.

Accepted languages: `astro`, `cc`, `csharp`, `elixir`, `erlang`, `fsharp`, `go`, `haskell`, `java`,
`javascript`, `julia`, `kotlin`, `lua`, `mdx`, `perl`, `php`, `powershell`, `python`, `ruby`, `rust`,
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

`# gazelle:dx_external <ecosystem> <path|off> [<repo>]` names the manifest or
lockfile that third-party imports resolve through, for this directory and
below. The path is relative to the repository root. Ecosystems: `go` (a
`go.mod`, no repo) and `maven` (a rules_jvm_external `maven_install.json`,
repo `maven` by default). Repeat the line to feed one ecosystem from several
files. The lines in one `BUILD` file replace what the directory inherited, and
`off` drops it. Without a directive, a `go.mod` or `maven_install.json` in a
directory applies to that directory and below. `dx generate` fails on a
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
- `python`: an import whose top-level module matches the normalized name of a
  package in `uv.lock` resolves to `@pypi//<name>`. Name a lockfile with
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
