# Dart

Build, run and test Dart with the Dart SDK that
[rules_dart](https://github.com/aran/rules_dart) 0.8.0 pins to 3.13.5. Nothing
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
    main = "test/greet_test.dart",
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

- The generated upstream target is `manual`, so `//...` builds only the
  public wrapper.
- An import of a package that the target does not list in `deps` fails the
  build, even when a dependency re-exports it.
- `dart_test` needs `package:test` as a direct dependency.
- Read data files through `@rules_dart//dart/runfiles`.
- `DartInfo`, `DartAnalyzableInfo` and `DartCompileInfo` pass through
  unchanged.
- `dart_codegen` and the web binaries are not wrapped yet.

## Pub dependencies

Declare a hub with the rules_dart `pub` extension. The lock pins each package
to a sha256. Make the lock with `dart pub get` on a developer machine, never
inside an action.

```python
bazel_dep(name = "rules_dart", version = "0.8.0")

pub = use_extension("@rules_dart//dart/pub:extensions.bzl", "pub")
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

Dart sources form the `dart` class, so `dx check` sees them. No Dart formatter
or analyzer runs yet. Until one does, the Dart rules add the `no-format`,
`no-lint` and `no-typecheck` tags to every target.

## Platforms

| Cell | Status |
| --- | --- |
| `linux_x86_64` | Qualified |
| `linux_arm64` | Not qualified |
| `macos_arm64` | Not qualified |
| `windows_x86_64` | Not qualified |
| `windows_arm64` | Unsupported |

rules_dart 0.8.0 ships no Windows arm64 SDK, because its version table has no
`windows-arm64` entry. The wrappers mark their targets incompatible there, so a
build fails instead of resolving a missing toolchain.

## Cross-compiling

A Dart SDK that cross-compiles is refused at analysis time. `dart compile
--target-os` downloads the target snapshotter while the action runs, which
breaks hermetic and offline builds. Build on a host that matches the target.
