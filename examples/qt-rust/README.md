# Qt Rust example

A headless Qt 6 ledger whose model is Rust. QML calls the Rust type,
reads its properties and handles its signal. It builds against the pinned
`@dx_qt` SDK on Linux x86_64 and Linux arm64 and packages into a relocatable archive.

| Package | Role |
| --- | --- |
| `ledger` | `ledger` crate: positive amounts, a running total and an Adler-32 checksum from the pinned `adler2` crate |
| `model` | `Ledger.Model`: `LedgerModel`, a CXX-Qt QObject with read-only `count`, `total` and `checksum` properties, an `add` invokable and a `rejected` signal |
| `ui` | `Ledger.Ui`: `Main.qml` calls `add` and records each `rejected` signal |
| `app` | `ledger` binary, `ledger_test` QtTest suite, `runtime` bundle and `ledger_release` archive |

```sh
bazel build //examples/qt-rust/...
bazel test //examples/qt-rust/...
bazel run //examples/qt-rust/app:ledger -- 5 -2 7
```

`ledger` takes one or more integer amounts. It prints `added <n>` or
`rejected <n>` per amount, then a summary such as
`2 entries, total 12, adler32 004c000d`.

Exit codes: `0` every amount added, `1` the QML module failed to load, `2`
usage error, `3` at least one amount was rejected.

## Package

```sh
bazel build //examples/qt-rust/app:ledger_release_archive
mkdir /tmp/ledger && tar -xzf bazel-bin/examples/qt-rust/app/ledger_release.tar.gz -C /tmp/ledger
env -i /tmp/ledger/ledger 5 7
```

The archive holds `ledger`, `NOTICE`, `IDENTITY.txt` and `resources/` with
the Qt libraries, the `QML` and `QtQml` import modules and the license texts.
The Rust code is linked into `ledger`. Only glibc 2.34 or newer comes from
the host.
