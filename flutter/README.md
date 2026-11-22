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

## Dependencies

### Setup

rules_dx depends on rules_flutter and registers its toolchains, so you need no
Flutter, Dart or Android SDK on your machine. Start a project:

```sh
dx init --apply demo-app
```

Your `MODULE.bazel` needs only `bazel_dep(name = "rules_dx")`. You add no
`rules_flutter` `bazel_dep`, no `flutter.toolchain` tag and no engine
`use_repo`. [`tests/consumer`](https://github.com/devhusdk/rules_dx/blob/main/flutter/tests/consumer) is a minimal project.

When your code imports pub packages, add a `pubspec.lock` and declare a hub
with the `pub` extension. A hub is a Bazel repository that holds one target
per locked package:

```python
pub = use_extension("@rules_dx//flutter:extensions.bzl", "pub")
pub.from_lock(
    name = "my_flutter_deps",
    lock = "//app:pubspec.lock",
)
use_repo(pub, "my_flutter_deps")
```

To write or refresh `pubspec.lock` for a package directory, run `pub get`
from the pinned Flutter SDK:

```sh
bazel run @rules_flutter//flutter/pub -- get --directory <dir>
```

## API docs

`dx docs` runs the pinned `dart doc` on every `flutter_library` with a `lib/`
source. To declare a docs target yourself, use `dart_doc_shard` from
`@rules_dx//dart:defs.bzl`. `dart doc` is not available on
`windows_arm64`, where rules_flutter ships no SDK.

## Packaging

### Apps

- `flutter_web_app` builds the web bundle.
- `flutter_android_app` builds an Android app with `android_binary`, so no
  Gradle runs. Set `android_abi` to `arm64`, the default, or `x64`. It needs
  the managed `androidsdk` override from rules_dx.
- `flutter_ios_app` and every target it creates build only on a macOS host.
  Other hosts skip them as incompatible.

### Profile builds

To build a `flutter_application` in profile mode, write a transition that
sets `@rules_dx//flutter:profile` to `True` and `//command_line_option:compilation_mode`
to `opt`, and apply it to the app's label attribute.
