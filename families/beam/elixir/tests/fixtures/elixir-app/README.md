# Elixir app example

An Elixir application split into three packages. It builds and tests on the pinned Elixir and Erlang/OTP runtimes with nothing installed on the host.

| Package | Role |
| --- | --- |
| `catalog` | `catalog` library: item parsing, a unit table generated from `units.txt`, and JSON through `jason` |
| `stock` | `stock` application: a supervised `GenServer` ledger with a per-item limit from `app_env` |
| `cli` | `stockctl` escript that runs commands against the ledger |

```sh
bazel build //elixir/tests/fixtures/elixir-app/...
bazel test //elixir/tests/fixtures/elixir-app/...
bazel run //elixir/tests/fixtures/elixir-app/cli:stockctl -- "add 2 dozen pears" "count pears"
```

The run prints `24`, `24` and the snapshot `[{"name":"pears","units":24}]`. A command that fails prints `error: ...` and exits `1`.

`catalog:catalog_test` covers parsing, the generated table and the JSON output. `stock:ledger_test` covers the limit from `app_env`, the application graph, and supervisor restarts after a kill and after a crash. `cli:stockctl_test` runs the commands in process and through the `stockctl` launcher.

`jason` comes from `mix.lock` through the `elixir.mix` tag in `MODULE.bazel`; the hub repo `@elixir_app_deps` aliases it.
