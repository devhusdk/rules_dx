# Erlang app example

An OTP application split into three packages. It builds and tests on the pinned Erlang/OTP runtime with nothing installed on the host.

| Package | Role |
| --- | --- |
| `proto` | `kv_proto` library: a leex lexer and a yecc parser for `put`, `get` and `del` commands |
| `store` | `kv_store` application: a supervised `gen_server` with a key limit, watchers and a JSON snapshot through `thoas` |
| `checksum` | `kv_checksum` application: an FNV-1a checksum implemented as a NIF built from C by the LLVM toolchain |
| `cli` | `kvctl` escript that runs commands against the store |

```sh
bazel build //examples/erlang-app/...
bazel test //examples/erlang-app/...
bazel run //examples/erlang-app/cli:kvctl -- "put apples 3" "get apples"
```

The run prints `ok`, `3` and the snapshot `{"apples":3}`. A command that does not parse prints `error: ...` and exits `1`.

`proto:eunit` runs the parser tests. `store:kv_store_SUITE` is a Common Test suite for the limit from `app_env`, watcher events, supervisor restarts and the snapshot. `cli:kvctl_SUITE` runs the commands in process and through the `kvctl` launcher.

`checksum:eunit` runs the NIF against known vectors; `bazel coverage //examples/erlang-app/checksum:eunit` reports both the Erlang and the C lines.

`thoas` is a pinned `http_archive` declared in `MODULE.bazel`.
