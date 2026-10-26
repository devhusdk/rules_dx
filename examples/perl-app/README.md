# Perl app example

A Perl stock CLI split into three packages. Every target runs on the
relocatable Perl 5.40 that rules_perl pins, with nothing installed on the host.

| Package | Role |
| --- | --- |
| `inventory` | `Inventory::Item` and `Inventory::Ledger`, a stock ledger with a per-item limit |
| `report` | Unit table from `data/units.csv` read through runfiles, and CSV output through `Text::CSV` |
| `cli` | `stockctl` command line and a launcher test |

```sh
bazel build //examples/perl-app/...
bazel test //examples/perl-app/...
bazel run //examples/perl-app/cli:stockctl -- "add 2 dozen pears" "count pears"
bazel test //examples/perl-app/... --//examples/perl-app/cli:limit=144
```

The run prints `24`, `24` and the report `name,units` / `pears,24`. A command
that fails prints `error: ...` and exits `1`. The `limit` flag selects the
per-item limit through `select()` on `env`, and the launcher test follows it.

`Text::CSV` 2.06 comes from CPAN through the rules_perl `cpan` extension. Its
lock is `third_party/cpanfile.snapshot.lock.json`, and every distribution
carries a sha256. `Text::CSV` runs on its pure-Perl `Text::CSV_PP` backend. To
change the pin, edit `third_party/cpanfile` and run
`bazel run //examples/perl-app/third_party:compiler`.

| Platform | Status |
| --- | --- |
| linux_x86_64 | Qualified |
| linux_arm64, macos_arm64, windows_x86_64 | Not qualified |
| windows_arm64 | Unsupported |
