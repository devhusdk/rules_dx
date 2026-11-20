# Dart

This directory helps you build, run and test Dart code with rules_dx.

Load its rules in a `BUILD.bazel` file, the file that declares what Bazel
builds, from `@rules_dx//dart:defs.bzl`.
`extensions.bzl` holds the module extension: the part you call from
`MODULE.bazel` to set up toolchains and dependencies.
The tests live in `tests/`. The [Dart page](../docs/languages/dart.md) shows
setup, every rule and the tools dx runs.

Run the Dart tests from a rules_dx checkout:

```sh
tools/dx test //dart/...
```

## Dependencies

### Pub dependencies

To use pub packages, list them in `dependencies` or `dev_dependencies` of
your root `pubspec.yaml` and run `dx generate --apply`. It writes the missing
`pubspec.lock` with `pub get` from the pinned Dart SDK. The lock pins each
package to a sha256. `dx update` moves the versions later. A pubspec that
lists no package gets no lock and no hub.

Then declare a hub with the `pub` extension. A hub is a Bazel repository that
holds one target per locked package:

```python
pub = use_extension("@rules_dx//dart:extensions.bzl", "pub")
pub.from_lock(
    name = "my_pub_deps",
    lock = "//third_party/dart:pubspec.lock",
)
use_repo(pub, "my_pub_deps")
```

Pick any hub name except `ext_pub_deps`. rules_dart shares one pub hub with
that name across modules and allows only one version of each package in it.

## Generate

### Gazelle

`dx generate` writes the Dart rules for each pub package. It makes one Bazel
package per folder that holds a `pubspec.yaml`:

- one `dart_library` named after the pubspec `name`, over every `.dart` file
  under `lib/`;
- one `dart_binary` per `bin/*.dart`, named after the file stem, with `_bin`
  added when that name clashes with the library. Files under `bin/src/` go
  into each binary's `srcs`;
- one `dart_test` per `test/**/*_test.dart`. The other `.dart` files under
  `test/` go into each test's `srcs`.

Point Gazelle at the lock so pub imports resolve to hub targets:

```python
# gazelle:dx_external pub third_party/dart/pubspec.lock my_pub_deps
```

Without the directive, Gazelle finds a `pubspec.lock` next to the pubspec or
in a parent folder and uses the hub `pub_deps`. An import resolves to a
library in your repository first, so workspace members and `path:` packages
need no hub entry.

Generation stops with an error, and never guesses a label, when:

- a `package:` import is not in `dependencies` (tests may also use
  `dev_dependencies`);
- a declared package has no direct entry in the lock;
- a `part` file is neither on disk nor an output of a `genrule` in the same
  BUILD file. Write that generator rule by hand;
- a BUILD file sits below `lib/`, `bin/` or `test/`;
- a directive cannot be parsed.

Loose `.dart` files outside a pubspec folder get no rule. Generation never
touches `data`, `defines`, `tags` or `aspect_hints`. It also does not turn
`analysis_options.yaml` into a rule: bind a `dart_config` by hand, as shown
below.

## Quality

Format, lint and typecheck are on by default for Dart:

```sh
dx format --apply
dx lint
dx typecheck
```

`dx format` runs `dart format` from the Dart SDK. It formats each file at the
language version of its package. Bazel fetches the SDK only when it analyzes a
Dart target.

`dx lint` and `dx typecheck` share one `dart analyze` run per target, over its
sources, its transitive Dart sources and its external packages. Typecheck
reports `COMPILE_TIME_ERROR` and `SYNTACTIC_ERROR` diagnostics. Lint reports
`LINT`, `HINT`, `STATIC_WARNING` and `TODO`. Any other type fails the run.
`dx lint --apply` does not rewrite Dart sources.

To configure the analyzer and formatter, wrap your `analysis_options.yaml` in
a `dart_config`. An `include: package:...` needs that package in `deps`:

```starlark
load("@rules_dx//dart:defs.bzl", "dart_config")

dart_config(
    name = "analysis_options",
    src = "analysis_options.yaml",
    deps = ["@my_pub_deps//:lints"],
)
```

Then set `aspect_hints = [":analysis_options"]` on your targets. Format
ignores `deps`. The analyzer checks `analysis_options.yaml` too, so an
include it cannot load or an unknown lint is an error. Without a
`dart_config`, you get only the analyzer's default warnings and no lint
rules.

## API docs

`dx docs` runs the pinned `dart doc` on every Dart library with a `lib/`
source. It works for `flutter_library` targets too. The docs hold each
symbol's first doc paragraph, not its signature or source location. To
declare a docs target yourself, use `dart_doc_shard` from
`@rules_dx//dart:defs.bzl`. `dart doc` is not available on
`windows_arm64`, where rules_dart ships no SDK.

## Toolchain

### Examples

- `dart/tests/fixtures/adopt_dart` adopts a pub workspace with `dx generate`.
- `dart/tests/fixtures/dart_app` is a multi-package app with a generated
  source file, runfiles data and targets that are meant to fail.

## Platforms

### Platforms

Dart builds on Linux x86_64 and arm64, macOS arm64 and Windows x86_64.
rules_dart ships no SDK for Windows arm64, so Dart targets are incompatible
there.

### Cross-compiling

Build for another platform with `--platforms`:

```sh
bazel build --platforms=//:linux_arm64 //app:cli
```

`exe` and `aot-snapshot` outputs cross-compile. `dart_test` runs on the host.
