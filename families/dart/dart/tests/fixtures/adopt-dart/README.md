# Adopt Dart example

Pub workspace with a `greet` library and a `tool` binary that uses `package:args`.

```sh
bazel run @rules_dx//:dx -- init //dart/tests/fixtures/adopt-dart/...
bazel run @rules_dx//:dx -- generate //dart/tests/fixtures/adopt-dart/...
bazel build //dart/tests/fixtures/adopt-dart/...
bazel test //dart/tests/fixtures/adopt-dart/...
bazel run //dart/tests/fixtures/adopt-dart/packages/tool -- --name dart
```

The `greet_test` test passes. The `# gazelle:dx_external` directive in `BUILD.bazel` points
Gazelle at the lock in `third_party/dart`. In your own repository, point it at your own
`pubspec.lock` and at the hub you declare with the `pub` extension from `@rules_dx//dart:pub.bzl`.
