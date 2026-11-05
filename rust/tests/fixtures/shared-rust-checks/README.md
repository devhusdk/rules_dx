# Shared Rust checks

Tests and evidence for `examples/shared-rust` that are not part of the
example itself.

- `inspect_apk_test` and `inspect_consumer_apk_test` check the package, SDK
  floor, label, launch activity, byte-identical assets, exported symbols per
  ABI, no extra ABIs, and the signature.
- `testdata/` holds logcat captures for `check_lifecycle.sh`. The `android`
  workflow runs it on the emulator log of `:install` and `:start`.
- `repos.bzl` defines `@shared_consumer`, a separate workspace that owns its
  HTML, board, notes, resources, features, `rustc` flags, and WASI runtime
  wrapper while reusing `tasks_core`, `shared_tasks`, and `tasks_activity`.
- `consumer_browser_test` and `consumer_native_test` run that consumer.

```sh
bazel test @shared_consumer//... \
  //rust/tests/fixtures/shared-rust-checks:consumer_browser_test \
  //rust/tests/fixtures/shared-rust-checks:consumer_native_test
```

`//tools/testing/hermetic:rust_wasi` builds the core from a fresh module
whose `rust.toolchain` adds `wasm32-wasip1` and the flag
`--cfg=consumer_toolchain`.

The `Pending` items: a recorded emulator run in CI, multi-ABI install, and
iOS signing and export, which need a consumer provisioning profile.
