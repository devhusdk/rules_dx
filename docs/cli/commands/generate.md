# `dx generate`

Updates `BUILD` declarations from source with Gazelle.

```sh
dx generate
dx generate --apply //pkg/...
dx generate --here
dx generate --apply --affected
dx generate --apply svc/new.py
dx generate --output json //pkg/...
dx generate --apply -- --jobs=4
```

```text
dx generate [--here] [--affected] [--apply] [--diff] [scope...] [-- bazel-options...]
```

| Flag | What it does |
| --- | --- |
| `--apply` | Write the updated BUILD files. Without it, stale files fail the run. |
| `--here` | Use the current directory tree. |
| `--affected` | Use the directories of files changed since the default branch merge-base. |
| `--diff` | Print one unified patch per stale file, or per file `--apply` wrote or refused. |
| `-o, --output text\|json` | Choose the output format. |
| `--workspace`, `-q`, `-v`, `--color`, `--build-slots`, `--build-slots-wait`, `--bazel-startup-option` | Global flags. |

`--affected` does not combine with scopes or `--here`. `--fail-on` and `--report` are usage errors.

Scopes: no scope means the whole repo. Pass labels, patterns, files, or directories of the main repository. A file or directory maps to its own directory without asking Bazel, so a new file or a directory with no `BUILD` file works. Bazel options go after `--`.

## Output

Text starts with `Running generate for <scope>`. `--quiet` hides it. It then prints `Created <path>` or `Modified <path>` per file, sorted by path. Without `--apply` these are the stale files. Files not written go to stderr as `Not applied: <path> (<reason>)`.

Each ignored import prints `Ignored import '<import>' in <path> [<language>]: ...`.

JSON emits `command_started` with `mode` `check`, or `default` with `--apply`. It then emits one `change` per file with `path`, `kind` (`create` or `modify`), `edit_count`, and `effect` (`proposed`, `applied`, or `refused`). A refused change has a `reason`. A Bazel failure emits `bazel_failed`. A language extension failure emits one `generation_failed` error per entry with `language`. An unresolved or ambiguous import also carries `kind` (`unresolved_import` or `ambiguous_import`), `import`, and `fix` with the BUILD `file` and the `directive` to add; `message` keeps the same text. `command_finished` has `results_complete`, `exit_code`, and `changes` counts.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Success. |
| `1` | Stale files, `MODULE.bazel` or a lock file that needs wiring (check), a wiring or lock step that failed (`--apply`), a file `--apply` did not write, incomplete results, `invalid_result`, a generation error, or `diff_failed`. |
| `2` | Usage or scope error. |
| Bazel's code | Bazel failed. |

## Add a dependency

Add the package with its own tool, then run `dx generate --apply`:

```sh
cargo add serde
uv add requests
pnpm add left-pad
dx generate --apply
```

When the root `Cargo.lock`, `uv.lock`, `pnpm-lock.yaml`, or `pubspec.lock` has its manifest beside it and `MODULE.bazel` has no stanza for it, `dx generate --apply` adds one: a `crates`, `pypi`, `npm`, or `pub_deps` hub plus its `bazel_dep`. A stanza you wrote is left alone. Without `--apply`, dx says what it would add and exits `1`. Each wiring step is also a `notice` in JSON, with code `wiring`, `wiring_pending`, or `wiring_failed`. List Cargo workspace members written as globs in `manifests` yourself.

When files of a language family are in the tree and its `rules_dx_<family>` module is published, `--apply` adds `bazel_dep(name = "rules_dx_<family>")`.

## The `.dx` tree

Every dx command except `init`, `status` and `upgrade` first brings `.dx` in step with `dx.toml`: `.dx/BUILD.bazel` with the workspace policy, the tool bases under `.dx/config/`, the `.dx/bazelrc/` layers, and the marked exports block in the root `BUILD.bazel` when root tool configs exist. Only the families whose files are in the tree are wired. dx removes files under `.dx` it no longer renders and keeps `.dx/local` and `.dx/pins`. A `.dx` file that a root tool config still names is kept too, with `dx: warning: <file> references <path>, which dx does not render; kept it`. A named path that does not exist prints `dx: warning: <file> references <path>, which does not exist`. When nothing it reads has changed, it skips the work. `dx generate` and `dx check` always look at the tree again, so a language added later is wired by the next `dx generate`. When the sync fails, the command prints `dx: warning: .dx not refreshed: <error>` and runs anyway.

## Directives

Set directives as `# gazelle:<name> <value>` comments in a `BUILD` file. They apply to the directory and below. A `dx_` directive that dx does not know fails generation.

| Directive | What it does |
| --- | --- |
| `dx_ignore_import <language> <import>` | Skip one import. Fails when nothing matches, the language is unknown, or the value is malformed. |
| `resolve <language> <import> <label>` | Map one import to an exact label. |
| `dx_external <ecosystem> <path\|off> [<repo>]` | Name the manifest or lockfile that third-party imports resolve through. |
| `dx_rust_corpus true\|false` | Turn the Rust corpus targets on or off. |
| `dx_rust_crates_hub <repo>` | Load `aliases` and `all_crate_deps` for Cargo packages from the crate_universe hub `@<repo>` instead of `@crates`. The package needs a `BUILD` file. |
| `dx_cuda_archs <spec> <spec>...` | Set `cuda_archs` of a generated `cuda_library`. Required for `.cu` and `.cuh` files. |
| `dx_fixture_testonly true\|false` | Mark generated libraries under a `tests/`, `fixtures/` or `testdata/` directory `testonly`. Off by default. Inherited by child packages. |
| `lang <languages>` | Must list `dx_dispatch`, for example `go,dx_dispatch`. |

When `.dx/BUILD.bazel` from `dx generate` already has a `ruff_config` for the root ruff file, gazelle writes no root `ruff_config` and no `aspect_hints` for it.

`dx_ignore_import` languages: `astro`, `cc`, `csharp`, `dart`, `elixir`, `erlang`, `fsharp`, `go`, `haskell`, `java`, `javascript`, `julia`, `kotlin`, `lua`, `mdx`, `ocaml`, `perl`, `php`, `powershell`, `python`, `ruby`, `rust`, `scala`, `svelte`, `swift`, `typescript`, `vue`, `zig`.

`dx_external` ecosystems: `go` (`go.mod`, no repo), `maven` (`maven_install.json`, repo `maven`), `python` (`uv.lock`, repo `pypi`), `pub` (`pubspec.lock`, repo `pub_deps`), `dub` (`dub.json` with `dub.selections.json`, repo `dub`), `npm` (root `package.json`). Repeat the line to use several files. `off` drops an inherited file. Without the directive, a `go.mod`, `maven_install.json`, `uv.lock`, `pubspec.lock`, or root `package.json` applies to its directory and below. When an import is unresolved but such a file in another directory provides it, the error names the `dx_external` line to add.

```starlark
# gazelle:dx_external maven third_party/jvm/maven_install.json maven
# gazelle:dx_ignore_import python requests
# gazelle:resolve python yaml @pypi//pyyaml
```

For `java`, `kotlin`, and `scala`, name the fully qualified import or its package. A bare class name does not match.

```starlark
# gazelle:resolve kotlin com.google.common.collect.ImmutableList @maven//:com_google_guava_guava
```

For `csharp`, name the full namespace, such as `Newtonsoft.Json`.

For `javascript` and `typescript`, a bare import also resolves through `compilerOptions.paths` and `baseUrl` in the nearest `tsconfig.json` or `jsconfig.json`.

## Third-party imports

An import resolves in this order:

1. An exact `resolve` mapping.
2. A library in the workspace.
3. A `dx_ignore_import` entry.
4. The `dx_external` sources for the language.
5. Otherwise the run fails with an unresolved import.

| Language | Resolves through |
| --- | --- |
| `go` | `go.mod` direct requires, as `@<repo>//<package>`. An `// indirect` module fails. |
| `java`, `kotlin`, `scala` | `maven_install.json`, as `@maven//:<group>_<artifact>`. A package in several artifacts needs a `resolve` mapping. |
| `rust` | `Cargo.toml`. |
| `python` | `uv.lock`, as `@pypi//<name>`. Lowercase the name and replace `-`, `_`, `.` with `_`. A different import name needs a `resolve` mapping. |
| `javascript`, `typescript` | Root `package.json`, as `//:node_modules/<package>`. |
| `julia` | `[deps]` of `Project.toml`. Anything else needs a `resolve` line. |
| `csharp`, `fsharp`, `lua`, `ruby`, `perl`, `php`, others | One `resolve` line per import. |

## Python hub

Export the lockfile, then parse it in `MODULE.bazel`:

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

## Generated targets by language

Gazelle sets these attributes only when the rule lacks them. Set one yourself to keep your value. Gazelle never removes them.

- Java and Kotlin: each `*Test.java` or `*Test.kt` gets its own test named in snake case, such as `greet_test`. A handwritten rule that lists a test source in `srcs` owns it. Mark a dependency no import gives with `# keep`.
- Java and Kotlin: an `android.*` or `dalvik.*` import selects `java_android_library` or `kotlin_android_library` from `@rules_dx//android:defs.bzl`. Gazelle also sets `custom_package` and `tags = ["manual"]`. It never derives resources, the `R` class, or manifests.
- C and C++: an `<android/*.h>` include sets `target_compatible_with = ["@platforms//os:android"]` and `linkopts` (`-llog`, `-landroid`). Under `__ANDROID__`, `linkopts` becomes a `select`. `ANativeActivity_onCreate`, `JNI_OnLoad`, or a `Java_*` function sets `alwayslink = True`.
- D: a directory with `.d` files gets a `d_library`. A module-scope `main` gets a `d_binary`. A `unittest` block gets a `d_test` named `<library>_test`.
- Dart: a `pubspec.yaml` gets a `dart_library`, a `dart_binary` per `bin/*.dart`, and a `dart_test` per `test/**/*_test.dart`. With a `flutter` dependency they become `flutter_library` and `flutter_test`. A `package:` import must name a `pubspec.yaml` dependency.
- Dockerfile, CMake, Bicep and AsciiDoc: files found by name get one ownership rule per directory so `dx lint` covers them. `Dockerfile`, `Containerfile` and variants give `dockerfile_library` `dockerfile`. `CMakeLists.txt` and `*.cmake` give `cmake_library` `cmake`. `*.bicep` and `*.bicepparam` give `bicep_library` `bicep`. `*.adoc` and `*.asciidoc` give `asciidoc_library` `asciidoc`. A handwritten rule of the same kind that lists a file owns it.
- Julia: a `Project.toml` with `src/<name>.jl` gets a `julia_library`, and `test/runtests.jl` adds a `julia_test`.
- JavaScript and TypeScript: `*.test.*`, `*.spec.*`, and `*_test.*` sources become `js_test` or `typescript_test`. `.d.ts` files are never tests.
- Scala: sources ending in `Test`, `Spec`, or `Suite` become `scala_test`.
- Python, JavaScript, and TypeScript: an entry file such as `main.py` or `main.js` gets a binary named `<target>_bin` that depends on `:<target>`. Generate edits or removes only binaries with that shape. A handwritten binary that already runs the entry file stops generate from adding `<target>_bin`.
- `rust_android_apk` with a custom `manifest`: dx checks the manifest `package` and `minSdkVersion` against the rule. A mismatch exits `1`.
