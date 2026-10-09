# Shared Rust core application (#1488)

One portable Rust core (`shared_core`) drives four products without
redefined SDK or toolchain setup: a native binary, a served browser
application, an independent consumer browser and native pair, and the
mobile library builds below.

## Layout

- `src/lib.rs`: the shared core. Pure portable Rust plus one
  `wasm-bindgen` export surface (`greet`, stateful `Counter`). The
  `set_from_str` error case stays outside the `wasm-bindgen` impl
  block because its error type cannot cross the wasm boundary.
- `src/main.rs`: the native binary entry plus the `wasm-bindgen`
  browser test module.
- `index.html`, `assets/greeting.txt`: the staged browser product.
- `repos.bzl`: the independent consumer (`shared_app_consumer`)
  browser and native products over the same core outputs.
- `src/shared_app_test.rs`: staged layout and relocation assertions.
- `src/shared_consumer_test.rs`: the consumer server product check.
- `src/shared_browser_test.rs`: the real Firefox run.

## Run

```sh
bazel test //rust/tests/fixtures/shared_app/...
bazel test //rust/tests/fixtures/shared_app:shared_wasm_test \
    --test_env=GECKODRIVER_REMOTE=http://127.0.0.1:<port>
```

Start one `geckodriver --port <port>` first; the manual wasm test
needs it the same way `wasm_hello_browser_test` does.

## Qualified combinations

| Product | Evidence | Result |
| --- | --- | --- |
| Native binary and core tests (Linux x86_64) | `shared_native_output_test`, `shared_core_test` | pass |
| Browser app layout and serve (Linux x86_64) | `shared_app_test`, real Firefox `shared_browser_test` | pass |
| Independent consumer native and browser (Linux x86_64) | `consumer_native_output_test`, `shared_consumer_test` | pass |
| Shared core as Android device static library | per-tuple build with the NDK toolchains | pending host SDK, same command as `rust_apk` |
| Shared core as Android emulator static library | per-tuple build with the NDK toolchains | pending host SDK, same command as `rust_apk` |
| APK packaging, install, launch | `rust_android_apk` over this core | pending, owned by #1484 follow-ups |
| iOS library, app, signing, device | Apple toolchain selection | pending, owned by #1485 and #1486 |

## Negatives

- `shared_bindgen_shape_test`: a raw wasm file is not a bindgen
  output and cannot stage an application.
- `shared_missing_wasm_test`: a bindgen result without wasm output
  cannot stage an application.
- `counter_rejects_non_numeric_input`: bad core input returns an
  error and keeps the old value.
- The browser run asserts a missing asset stays a visible 404.
