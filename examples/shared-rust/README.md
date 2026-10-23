# Shared Rust task board

One Rust task board runs as a native CLI, a browser app, a WASI module,
an Android NativeActivity, an iOS simulator app, and an iOS XCFramework. Every output loads the same board and
asset files, applies the same commands, and prints the same replies.

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

## iOS

```sh
bazel test //apple/tests:macos_shared_rust_consumer_test
```

Run it on a macOS arm64 host with Xcode 16 and an iOS simulator
runtime. The test writes an independent workspace that depends on
`@rules_dx//examples/shared-rust:tasks_core`. That workspace owns a Rust
adapter with its own feature (`pocket_board`), `rustc` flag
(`--cfg=consumer_apple`), and Rust 1.98.0 toolchain with the iOS
triples, plus a `release` config with `-c opt`. It builds through the
public `apple_ios_app`, `apple_ios_app_test`, `apple_ios_framework`, and
simulator launchers.

The simulator app test:

- builds `pocket_app` with `--config=release` and copies the `.ipa` out
  of `bazel-bin`;
- checks the bundle identifier, version, minimum OS, `board.txt` and
  `notes.txt`, and the arm64 simulator binary with every adapter symbol;
- runs XCTest cases that apply commands, reject a bad board and asset,
  and match the native CLI's replies and `adler32:4fce0732` digest;
- installs the relocated bundle on a new simulator, launches it, and
  reads `profile=release` and the replies from its saved state;
- expects a device app without `provisioning_profile` and an
  `export_ipa` app to fail before any bundle exists.

The framework test builds `pocket_framework` with `device/arm64` and
`simulator/arm64` slices in release mode. It checks each slice's
platform, minimum OS, install name, symbols, header, and resources.
A second workspace without `rules_rust` imports the shipped
XCFramework, runs XCTest cases against it, and launches the app on a
simulator.

Each test keeps `pocket-evidence.txt` or `framework-evidence.txt` in its
outputs: macOS version and build, host arch, Xcode version and build,
SDK versions, `rustc --version`, simulator runtime and UDID, each slice's
platform and SDK, the launched state, and the pending device and signing
cells.

## Release

```sh
bazel test //examples/shared-rust:release_test
```

`tasks_release`, `tasks_wasi_release`, and `app_release` build with
`-c opt`. `release_test` relocates the native binary and WASI module and
expects identical output plus `build=release`. It also runs the WASI
module with no preopen and expects `err:unreadable: /board/board.txt`
with exit code 2.

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

## Offline and errors

```sh
bazel fetch //examples/shared-rust/... @shared_consumer//...
bazel test //examples/shared-rust/... @shared_consumer//... \
  --repository_disable_download
```

Tests tagged `block-network` run without network. With an empty
repository cache, `--repository_disable_download` fails with
`download is disabled`. `missing_interface_test` expects a web app
without its bindgen interface to fail with
`requires bindgen to provide RustWasmBindgenInfo`.
`missing_asset_test` and `bad_asset_path_test` cover asset errors.

## Pending

- Custom toolchain override in `@shared_consumer`: it overrides
  features, `rustc` flags, and the WASI runtime wrapper only.
- Automated offline check: the fetch and `--repository_disable_download`
  commands above are not run by any test.
- Emulator run: needs KVM. Without it the emulator boot loops.
- Physical arm64 device run.
- Multi-ABI install on a device.
- iOS device run: needs a physical device. The `device/arm64` slice is
  built and inspected only.
- iOS device signing and IPA export: needs a consumer provisioning
  profile and signing identity.

Debugger integration and game-engine portability are not supported.
