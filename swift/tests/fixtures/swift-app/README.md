# Swift app

A small ledger command line for Linux and macOS. It shows a Swift binary that uses
a third-party package (swift-argument-parser) that Swift Package Manager fetches from `Package.resolved`, a C library imported
as a Swift module, and the Swift test frameworks.

- `ledger/cdigest` is a C library that Swift imports as `CDigest`.
- `ledger/core` is the `LedgerCore` Swift library.
- `ledger/cli` is the command line, built with `-DLEDGER_STRICT`.

## Commands

```sh
tools/dx build //swift/tests/fixtures/swift-app/ledger/cli:ledger
tools/dx run //swift/tests/fixtures/swift-app/ledger/cli:ledger -- tea=250 milk=120
tools/dx test //swift/tests/fixtures/swift-app/...
```

The run prints:

```text
mode: strict
total: 370
digest: 4277201274
```

Bad input exits with code 64 and the usage text.

`ledger/cli:relocate_test` reads the ELF binary with `llvm-readelf`, so it runs on Linux only.
