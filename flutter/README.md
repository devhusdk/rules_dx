# Flutter

Pinned versions:

- rules_flutter 0.0.8
- Flutter 3.47.8

The fixture in `flutter/tests/runtime` builds a Flutter web app and runs one widget test. It needs no host Flutter, Dart, or Android SDK. It runs on Linux only.

The app in `flutter/tests/fixtures/flutter-app` has a generated tips file, localized strings, a font, a web bundle with a browser test, and builds that fail on purpose. Its consumer qualification is `//tools/testing/hermetic:flutter`. Only Linux hosts are advertised for now.

Build the fixture as an Android APK on Linux:

```sh
bazel build //flutter/tests/fixtures/flutter-app/android:apk
```

`flutter_android_app` wraps the upstream rule and uses `android_binary`, so no Gradle runs. Set `android_abi` to `arm64` (default) or `x64`. Device and emulator runs are not covered yet. Consumers need `use_repo` of `flutter_android_engine_arm64` and `flutter_android_engine_x64` and the managed `androidsdk` override from rules_dx_android.

Consumers load the rules from `@rules_dx//flutter/rules:defs.bzl`. rules_dx depends on rules_flutter and registers its toolchains.

Start a project:

```sh
dx init demo-app
```

Then add `rules_flutter` with a `flutter.pub` hub to `MODULE.bazel`, add a `pubspec.lock`, and declare `flutter_library` and `flutter_test` targets.

`flutter_web_app` builds the web bundle.

```sh
tools/dx test //flutter/tests/runtime/...
```

Regenerate the lock file:

```sh
bazel run @rules_flutter//flutter:pub -- get --directory flutter/tests/runtime
```

## API docs

`dart_doc_shard` in `@rules_dx//tools/docs/dart:defs.bzl` runs the pinned `dart doc` over a `flutter_library`. `dx docs` runs it on every Flutter library with a `lib/` source. `dart doc` is not available on `windows_arm64`, where rules_flutter ships no SDK.
