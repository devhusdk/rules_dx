# Shared Rust task board

One Rust task board runs as a native CLI, a browser app, a WASI module,
an Android NativeActivity, a macOS CLI, an iOS simulator app, and an
iOS XCFramework. Every output loads the same board and asset files,
applies the same commands, and prints the same replies.

```sh
bazel build //examples/shared-rust/... @shared_consumer//...
bazel test //examples/shared-rust/... @shared_consumer//...
```

## Layout

| Crate or target | Role |
| --- | --- |
| `tasks_model` | Task and board parsing, title and asset checks. |
| `tasks_core` | Board commands, summaries, and asset digests through the pinned `adler2` 2.0.1 crate. |
| `tasks` | Native and WASI CLI: `tasks [--asset=PATH]... BOARD [COMMAND]...`. |
| `shared_tasks` | `wasm-bindgen` interface (`WebBoard`, `asset_report`, `core_version`, `build_profile`). |
| `tasks_ffi`, `tasks_interface` | C interface (`ffi/tasks.h`) over the same core. |
| `tasks_activity`, `tasks_apk` | Android NativeActivity and APK. |
| `assets/board.txt`, `assets/notes.txt` | Resources every output loads. |

Commands are `add TITLE`, `done ID`, `reopen ID`, `attach ID ASSET`, and
`remove ID`. Replies are `ok:<event>` or `err:<kind>`.

The CLI exits 0 on success, 1 when a command is rejected, and 2 for a
usage error, an unreadable or invalid board, or a missing asset.

## Native

```sh
bazel run //examples/shared-rust:tasks -- \
  --asset=$PWD/examples/shared-rust/assets/notes.txt \
  $PWD/examples/shared-rust/assets/board.txt "add Ship it" "done 2"
bazel test //examples/shared-rust:native_test //examples/shared-rust:interface_test
```

`native_test` copies the binary and assets to a scratch directory and
checks output, exit codes, and error text. `interface_test` drives the
C interface.

## Browser

```sh
bazel run //examples/shared-rust:serve
bazel test //examples/shared-rust:asset_test //examples/shared-rust:browser_test
```

`browser_test` serves the relocated release app in Firefox 157.0. It
checks the build profile is `release`, the rendered tasks, summary,
asset digest, and load error, then
applies commands through the input and button.

## WASI

```sh
bazel test //examples/shared-rust:wasi_test \
  //examples/shared-rust:wasi_error_test \
  //examples/shared-rust:wasi_capability_test
```

`tasks_wasi` is `tasks` built for `wasm32-wasip1` and run under
Wasmtime 49.0.2. `wasi_error_test` expects exit 1 for a rejected
command. `wasi_capability_test` expects exit 2 and
`err:unreadable: /board/board.txt` when the board is not preopened.

## Android

```sh
export ANDROID_HOME=$HOME/Android/Sdk ANDROID_NDK_REVISION=27.2.12479018
NDK=--extra_toolchains=@android_ndk_toolchain//:ndk_cc_toolchain_device,@android_ndk_toolchain//:ndk_cc_toolchain_emulator
bazel test $NDK --test_env=ANDROID_HOME \
  //examples/shared-rust:inspect_apk_test \
  //examples/shared-rust:inspect_consumer_apk_test
bazel run $NDK //examples/shared-rust:install
bazel run $NDK //examples/shared-rust:start
bazel run $NDK //examples/shared-rust:log
```

The inspect tests check the package, SDK floor, label, launch activity,
byte-identical assets, exported symbols per ABI, no extra ABIs, and the
signature. The activity logs under the `DxSharedTasks` tag:
`lifecycle=create` through `lifecycle=destroy`, `board=loaded tasks=4`
or `board=invalid`, the summary or `summary none`, the asset report, and `input=touch|key <reply>` after a
tap or key press.

Tools: NDK 27.2.12479018, build-tools 34.0.0, platform-tools 37.0.0,
emulator 37.2.12.0, system image `android-34;default;x86_64`.

## Apple

```sh
bazel test //apple/tests:macos_shared_rust_consumer_test
```

Runs on an arm64 Mac with Xcode 16. A separate consumer workspace
reuses `tasks_core` with its own features and `rustc` flags, and
builds:

- A native macOS arm64 release CLI. The test relocates it, checks the
  replies, exit codes 0, 1, and 2, and the `notes.txt` digest. Evidence:
  `native-evidence.txt`.
- An iOS simulator release app. The test relocates the bundle, runs
  XCTest, installs it, and checks the state it writes on launch.
  Evidence: `pocket-evidence.txt`.
- An iOS XCFramework with device and simulator slices. An independent
  host app embeds it, runs XCTest, and launches in the simulator.
  Evidence: `framework-evidence.txt`.

Evidence files are written to the test's undeclared outputs.

## Release

```sh
bazel test //examples/shared-rust:release_test
```

`tasks_release`, `tasks_wasi_release`, and `app_release` build with
`-c opt`. `release_test` relocates the native binary and WASI module and
expects identical output plus `build=release`. It also runs the WASI
module with no preopen and expects `err:unreadable: /board/board.txt`
with exit code 2.

## Deploy

```sh
dx deploy //examples/shared-rust:tasks_deploy
dx deploy --apply //examples/shared-rust:tasks_deploy
bazel test //examples/shared-rust:tasks_deploy_verify
```

`tasks_deploy` is an
`archive_deploy` of `tasks`.
The first command builds it and publishes nothing. `--apply` checks the
tarball against its checksum and copies `tasks_deploy.tar.gz` and
`tasks_deploy.tar.gz.sha256` into the workspace root, or into the directory
given after `--`:

```text
archive_deploy: released tasks (profile: release)
  tarball:  /path/to/rules_dx/tasks_deploy.tar.gz
  checksum: /path/to/rules_dx/tasks_deploy.tar.gz.sha256
```

`tasks_deploy_verify` checks that the tarball holds `tasks` and matches the
checksum.

## Independent consumer

`@shared_consumer` is a separate workspace that owns its HTML, board,
notes, Android resources, permissions, features, `rustc` flags, and
WASI runtime wrapper. It reuses `tasks_core`, `shared_tasks`,
`tasks_activity`, and the public toolchains with no SDK or linker setup
of its own.

```sh
bazel test @shared_consumer//... \
  //examples/shared-rust:consumer_browser_test \
  //examples/shared-rust:consumer_native_test
```

`//tools/testing/hermetic:rust_wasi` builds the core from a fresh
module that names rules_dx `@my_dx`. That module's own `rust.toolchain`
adds the `wasm32-wasip1` target and the `rustc` flag
`--cfg=consumer_toolchain`. `consumer_report_test` only compiles with
that flag.

## Offline and errors

```sh
bazel fetch //examples/shared-rust/... @shared_consumer//...
bazel test //examples/shared-rust/... @shared_consumer//... \
  --repository_disable_download
```

Tests tagged `block-network` run without network.
`//tools/testing/hermetic:rust_wasi` runs this check: it fetches with
network, then tests in a fresh output root with the network denied and
`--repository_disable_download`. With an empty repository cache and no
network, the build fails with `Unknown host`. A binary that leaves out
`tasks_core` fails with ``error[E0432]: unresolved import `shared_tasks` ``.
`missing_interface_test` expects a web app
without its bindgen interface to fail with
`requires bindgen to provide RustWasmBindgenInfo`.
`missing_asset_test` and `bad_asset_path_test` cover asset errors.

## Pending

- Emulator run: needs KVM. Without it the emulator boot loops.
- Physical arm64 device run.
- Multi-ABI install on a device.
- iOS device run: needs a physical device.
- iOS device signing and IPA export: needs a consumer provisioning
  profile and signing identity.

Debugger integration and game-engine portability are not supported.
