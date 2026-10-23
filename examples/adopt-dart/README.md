# Adopt Dart example

Pub-layout Dart tree with a `greet` package that uses `collection` and a
stdlib-only `solo` package.

```sh
bazel run @rules_dx//:dx -- init //examples/adopt-dart/...
bazel run @rules_dx//:dx -- generate //examples/adopt-dart/...
bazel build //examples/adopt-dart/...
bazel test //examples/adopt-dart/...
```

`dx generate` writes a `dart_library` for each `lib/` and a `dart_test` for
each `test/` file. It writes `dart_binary` targets for `bin/` only in a package
at the workspace root. The root `# gazelle:dart_pub_deps_repo dx_dart_deps`
directive maps pub imports to the pinned `@dx_dart_deps` hub.
