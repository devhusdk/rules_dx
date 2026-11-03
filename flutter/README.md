# Flutter

Pinned versions:

- rules_flutter 0.0.8
- Flutter 3.47.8

The probe in `flutter/tests/probe` builds a Flutter web app and runs one widget test. It needs no host Flutter, Dart, or Android SDK. It runs on Linux only.

```sh
tools/dx test //flutter/tests/probe/...
```

Regenerate the probe lock file:

```sh
bazel run @rules_flutter//flutter:pub -- get --directory flutter/tests/probe
```
