# Gleam app example

A Gleam application split into three packages. Each package builds for Erlang
on the pinned OTP runtime and for JavaScript on the managed Node.js runtime,
with nothing installed on the host.

| Package | Role |
| --- | --- |
| `catalog` | Item parsing, a unit table generated from `units.txt`, and JSON through `gleam_json` |
| `stock` | Typed stock ledger with a per-item limit, sources in `lib/` and the JavaScript package `stock_ledger` |
| `cli` | `stockctl` command line on Erlang and `stockctl_js` on Node.js |

```sh
bazel build //examples/gleam-app/...
bazel test //examples/gleam-app/...
bazel run //examples/gleam-app/cli:stockctl -- "add 2 dozen pears" "count pears"
bazel run //examples/gleam-app/cli:stockctl_js -- "add 2 dozen pears" "count pears"
```

Both runs print `24`, `24` and the snapshot `[{"name":"pears","units":24}]`.
A command that fails prints `error: ...` and exits `1`.

Every package has a gleeunit test for each target. `cli:launchers_test` runs
both launchers and checks their output and exit codes.

`gleam_json` is a pinned `gleam_hex_package` declared in `MODULE.bazel`.
