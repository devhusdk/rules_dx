# Dart

Build, run and test Dart with the Dart SDK that
[rules_dart](https://github.com/aran/rules_dart) 0.8.1 pins to 3.13.7. Nothing
is installed on the host and nothing is fetched until a target needs it.
rules_dart registers the toolchains, so there is no toolchain setup beyond the
version pin in `MODULE.bazel`.

## Rules

Load the rules from `@rules_dx//dart/rules:defs.bzl`. They wrap rules_dart and
take the same attributes.

```python
load("@rules_dx//dart/rules:defs.bzl", "dart_binary", "dart_library", "dart_test")

dart_library(
    name = "greet",
    srcs = ["lib/greet.dart"],
    package_name = "greet",
)

dart_binary(
    name = "greet_bin",
    main = "bin/greet.dart",
    deps = [":greet"],
)

dart_test(
    name = "greet_test",
    deps = [
        ":greet",
        "@my_pub_deps//:test",
    ],
)
```

| Rule | What it makes |
| --- | --- |
| `dart_library` | A package from `.dart` files, importable as `package:<name>` |
| `dart_binary` | A compiled executable that runs with `bazel run` |
| `dart_test` | Runs one test file with the package:test runner |
| `dart_codegen` | Runs a generator script over one `.dart` file and makes the output files |

`dart_binary(target = "js")` or `"wasm"` compiles a web app to `.js`, or to `.wasm` plus its `.mjs` loader and `.support.js`.

`dart_binary` and `dart_test` default `main` to `<name>.dart`, then `bin/<name>.dart` or `test/<name>.dart`.

- The generated upstream target is `manual`, so `//...` builds only the
  public wrapper.
- An import of a package that the target does not list in `deps` fails the
  build, even when a dependency re-exports it.
- `dart_test` needs `package:test` as a direct dependency.
- Read data files through `@rules_dart//dart/runfiles`.
- `DartInfo`, `DartAnalyzableInfo` and `DartCompileInfo` pass through
  unchanged.
- `dart_codegen` is the upstream rule, unchanged. Its outputs can be `srcs` of a
  `dart_library`.

## Gazelle

`dx generate` writes the Dart rules for each pub package. It makes one Bazel
package per folder that holds a `pubspec.yaml`:

- one `dart_library` named after the pubspec `name`, over every `.dart` file
  under `lib/`
- one `dart_binary` per `bin/*.dart`, named after the file stem (`_bin` is
  added when that clashes with the library); files under `bin/src/` go into
  each binary's `srcs`
- one `dart_test` per `test/**/*_test.dart`; the other `.dart` files under
  `test/` go into each test's `srcs`

Point Gazelle at the lock so pub imports resolve to hub targets:

```python
# gazelle:dx_external pub third_party/dart/pubspec.lock my_pub_deps
```

A `pubspec.lock` next to the pubspec, or in a parent folder, is found without
the directive and uses the hub `pub_deps`. An import resolves to a library in
the repository first, so workspace members and `path:` packages need no hub
entry.

Generation fails, and never guesses a label, when:

- a `package:` import is not declared in `dependencies` (tests may also use
  `dev_dependencies`)
- a declared package has no direct entry in the lock
- a `part` file is neither on disk nor an output of a `genrule` in the same
  BUILD file; write the generator rule by hand
- a BUILD file sits below `lib/`, `bin/` or `test/`
- a directive cannot be parsed

Loose `.dart` files outside a pubspec folder get no rule. `data`, `defines`,
`tags` and `aspect_hints` are never touched. `analysis_options.yaml` is not
turned into a rule: bind `dart_config` by hand through `aspect_hints`.

## Examples

- `dart/tests/fixtures/adopt-dart` adopts a pub workspace with `dx generate`.
- `dart/tests/fixtures/dart-app` is a multi-package app with a generated source file, runfiles data and failure targets.

## Start a project

```sh
dx init demo-app
```

Then add `rules_dart` and a `pub` hub to `MODULE.bazel`, add a `pubspec.lock`, and declare `dart_library`, `dart_binary` and `dart_test` targets.

## Pub dependencies

Declare a hub with the `pub` extension from `@rules_dx//dart:extensions.bzl`. The lock pins each package
to a sha256. Make the lock with `dart pub get` on a developer machine, never
inside an action.

```python
bazel_dep(name = "rules_dart", version = "0.8.1")

pub = use_extension("@rules_dx//dart:extensions.bzl", "pub")
pub.from_lock(
    name = "my_pub_deps",
    lock = "//third_party/dart:pubspec.lock",
    on_version_conflict = "upgrade",
)
use_repo(pub, "my_pub_deps")
```

rules_dart shares one pub hub across modules and does not support two versions
of a package, so the hub name must differ from `ext_pub_deps`.

## Quality

Dart sources form the `dart` class, so `dx check` sees them. Format, lint and
typecheck are on by default. `dart format` is the
real formatter: it runs the `dart` binary of the resolved SDK (nothing is
fetched until a Dart target is analysed), formats each file at the language
version of its package, and `dx format` fixes what it reports. Formatter options
come from a `dart_config` (an `analysis_options.yaml`) bound with
`aspect_hints`, from `@rules_dx//quality:native_config.bzl`.

`dx lint` and `dx typecheck` run one `dart analyze --format=json` per capability over the
target's sources, with its transitive Dart sources and external packages staged for resolution.
The two share the run's output and split it by diagnostic type: typecheck keeps
`COMPILE_TIME_ERROR` and `SYNTACTIC_ERROR`; lint keeps `LINT`, `HINT`, `STATIC_WARNING` and
`TODO`. Any other type fails closed. The staged `analysis_options.yaml` is analyzed too, so an
unusable `include:` or an unknown lint is an error, not a silent pass. Without a `dart_config`
only the analyzer's default warnings are reported and no lint rules are on. An
`include: package:...` needs the package in the config's `deps`:

```starlark
load("@rules_dx//quality:native_config.bzl", "dart_config")

dart_config(
    name = "analysis_options",
    src = "analysis_options.yaml",
    deps = ["@my_pub_deps//:lints"],
)
```

Bind it with `aspect_hints = [":analysis_options"]`. Format ignores `deps`.
`dx lint --fix` does not rewrite Dart sources.

## Platforms

Supported on linux x86_64 and arm64, macOS arm64 and Windows x86_64. Windows arm64 is unsupported: rules_dart ships no SDK for it, so its targets are incompatible there.

## Cross-compiling

```
bazel build --platforms=//:linux_arm64 //app:cli
```

`exe` and `aot-snapshot` cross-compile. `dart_test` runs on the host.
