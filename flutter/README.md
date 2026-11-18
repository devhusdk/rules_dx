# Flutter

This directory helps you build, run and test Flutter code with rules_dx.

Load its rules in a `BUILD.bazel` file, the file that declares what Bazel
builds, from `@rules_dx//flutter:defs.bzl`.
`extensions.bzl` holds the module extension: the part you call from
`MODULE.bazel` to set up toolchains and dependencies.
The tests live in `tests/`. The [Flutter page](../docs/languages/flutter.md) shows
setup, every rule and the tools dx runs.

Run the Flutter tests from a rules_dx checkout:

```sh
tools/dx test //flutter/...
```

## Apps

- `flutter_web_app` builds the web bundle.
- `flutter_android_app` wraps the upstream rule and uses `android_binary`, so
  no Gradle runs. Set `android_abi` to `arm64` (default) or `x64`. It needs
  the managed `androidsdk` override from rules_dx.
- `flutter_ios_app` and every target it generates build only on a macOS host.
  On other hosts Bazel skips them as incompatible.

## Setup

rules_dx depends on rules_flutter and registers its toolchains. You need no
host Flutter, Dart or Android SDK.

Start a project:

```sh
dx init --apply demo-app
```

Then add `rules_flutter` with a `flutter.pub` hub to `MODULE.bazel`, add a
`pubspec.lock`, and declare `flutter_library` and `flutter_test` targets.

Regenerate `pubspec.lock` for a package directory:

```sh
bazel run @rules_flutter//flutter:pub -- get --directory <dir>
```

## API docs

`dart_doc_shard` in `@rules_dx//tools/docs/dart:defs.bzl` runs the pinned `dart doc` over a `flutter_library`. `dx docs` runs it on every Flutter library with a `lib/` source. `dart doc` is not available on `windows_arm64`, where rules_flutter ships no SDK.
