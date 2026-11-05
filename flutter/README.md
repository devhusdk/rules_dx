# Flutter

Pinned versions:

- rules_flutter 0.0.8
- Flutter 3.47.8

The fixture in `flutter/tests/runtime` builds a Flutter web app and runs one widget test. It needs no host Flutter, Dart, or Android SDK. It runs on Linux only.

The app in `flutter/tests/fixtures/flutter-app` has a generated tips file, localized strings, a font, a web bundle with a browser test, and builds that fail on purpose. Its consumer qualification is `//tools/testing/hermetic:flutter`. Only Linux hosts are advertised for now.

Consumers load the rules from `@rules_dx//flutter/rules:defs.bzl`. rules_dx depends on rules_flutter and registers its toolchains.

Start a project:

```sh
dx init demo-app
```

Then add `rules_flutter` with a `flutter.pub` hub to `MODULE.bazel`, add a `pubspec.lock`, and declare `flutter_library` and `flutter_test` targets.

`flutter_application(target = "web")` builds the web bundle.

```sh
tools/dx test //flutter/tests/runtime/...
```

Regenerate the lock file:

```sh
bazel run @rules_flutter//flutter:pub -- get --directory flutter/tests/runtime
```
