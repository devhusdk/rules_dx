# Shared Rust task board

One Rust task board runs as a native CLI, a browser app, a WASI module,
an Android NativeActivity, a macOS CLI, an iOS simulator app, and an
iOS XCFramework. Every output loads the same board and asset files,
applies the same commands, and prints the same replies.

```sh
bazel build //examples/shared-rust/...
bazel test //examples/shared-rust/...
```

## Layout

| Crate or target | Role |
| --- | --- |
| `tasks_model` | Task and board parsing, title and asset checks. |
| `tasks_core` | Board commands, summaries, and asset digests through the pinned `adler2` 2.0.1 crate. |
| `tasks` | Native and WASI CLI: `tasks [--asset=PATH]... BOARD [COMMAND]...`. |
| `shared_tasks` | Browser `rust_binary(wasm = "browser")` with the `wasm-bindgen` interface. |
| `tasks_ffi`, `tasks_interface` | C interface (`ffi/tasks.h`) over the same core. |
| `tasks_activity`, `tasks_apk` | Android NativeActivity and APK. |
| `assets/board.txt`, `assets/notes.txt` | Resources every output loads. |

Commands are `add TITLE`, `done ID`, `reopen ID`, `attach ID ASSET`, and
`remove ID`. Replies are `ok:<event>` or `err:<kind>`. The CLI exits 0 on
success, 1 when a command is rejected, and 2 for a usage error, an
unreadable or invalid board, or a missing asset.

## Run

```sh
bazel run //examples/shared-rust:tasks -- \
  --asset=$PWD/examples/shared-rust/assets/notes.txt \
  $PWD/examples/shared-rust/assets/board.txt "add Ship it" "done 2"
bazel run //examples/shared-rust:serve
bazel test //examples/shared-rust:wasi_test
```

For Android, install the NDK and set `ANDROID_HOME`, then use
`//examples/shared-rust:install`, `:start`, and `:log` against an emulator.
The emulator run (`evidence.yml` with `android=true`, run 42102949707) used
Ubuntu 26.04.1, emulator 37.2.13, platform-tools 37.0.1 and the Android 14
(API 34) Google APIs x86_64 system image, and checked the full lifecycle log.
For Apple, `//apple/tests/fixtures/shared_core:core_build_test` builds
the same core into a macOS CLI and an XCFramework.

## Deploy

```sh
dx deploy //examples/shared-rust:tasks_deploy
dx deploy --apply //examples/shared-rust:tasks_deploy
```

`tasks_deploy` is an `archive_deploy` of `tasks`. `--apply` checks the
tarball against its checksum and copies it into the workspace root.

Qualification checks, the independent consumer, and the APK inspections are
in [rust/tests/fixtures/shared-rust-checks](../../rust/tests/fixtures/shared-rust-checks/README.md).
