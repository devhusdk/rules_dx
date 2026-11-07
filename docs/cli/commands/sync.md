# `dx sync`

Selects the managed tools for `.dx/local/bin`, the env values, and the declared generated source in one Bazel build, and refreshes the editor files. It upgrades nothing. It always writes; run `dx status` to see drift first.

```sh
dx sync
dx sync //proto:api
dx sync --output json
```

```text
dx sync [<label>] [-- bazel-options...]
```

| Flag | What it does |
| --- | --- |
| `-o, --output text\|json` | Choose the output format. |
| `--workspace`, `-q`, `-v`, `--color`, `--build-slots`, `--bazel-startup-option` | Global flags. |

`--apply`, `--fail-on` and `--report` are usage errors.

Scopes: none selects the whole repository. One exact `//` or `@` label selects that label and every codegen shard that depends on it. A pattern, relative label, path, empty value, or second label is a usage error.

Run `dx sync` after you edit a `.capnp`, `.fbs`, or OpenAPI spec that a codegen rule reads. Selected outputs link under `.dx/local/setups/current/generated`. A second run with nothing changed reports the setup is already selected and writes nothing. Managed state lives under `.dx/local/`. After a new selection commits, `dx sync` prunes the setups it replaced, except generations a lease or a running process still holds through its working directory, executable, open files or mapped libraries. Processes dx may not read, such as another user's, are skipped. If one of your own processes cannot be read, nothing is pruned. Where it cannot observe processes at all, as on macOS and Windows, it keeps the current and the previous setup and prunes the rest. A prune failure prints one warning and does not change the exit code.

`dx sync` refreshes `.dx/local/ide/`: the TypeScript `lib` link, `cc/compile_commands.json` and the `cc/clangd` link for C and C++ trees, `go/gopackagesdriver` (`gopackagesdriver.cmd` on Windows) for Go trees, and for Rust trees `rust_analyzer/flycheck.exe`, `rust_analyzer/discover_bazel_rust_project.exe` and a `user_config.json` that makes flycheck run clippy. Set `go.toolsEnvVars.GOPACKAGESDRIVER` to the Go wrapper. A failed refresh prints one `ide_refresh_failed` warning and keeps the exit code.

## Bazel args

dx sets `--@rules_dx//config:clippy_output_diagnostics=true`, `--@rules_dx//config:rustc_output_diagnostics=true`, and `--remote_download_outputs=toplevel`. Pass `--remote_download_outputs=all` or `--remote_download_all` to download everything.

These args exit `2`: `--aspects`, `--output_groups`, `--remote_download_minimal`, `--remote_download_outputs=minimal`, `--build_event_json_file`, `--test_arg`, and the startup options `--bazelrc`, `--home_rc`, `--nohome_rc`, `--system_rc`, `--nosystem_rc`, `--output_base`, `--output_user_root`, `--host_jvm_args`, and `--server_jvm_out`. Pass `--output_base` and `--output_user_root` with `--bazel-startup-option`.

## Output

Text starts with `Running sync for <roots>`, then one result line:

- Up to date: `dx sync: already selected setup <setup> <pair>`.
- Changed: `dx sync: selected setup <setup> <pair>`.

`--quiet` hides all lines.

JSON emits `command_started`, an `operation` event, a `selection` event, and `command_finished` with `actions`. Failures emit an `error` with a code such as `bazel_failed`, `invalid_result`, `no_capability`, `unreadable_bep`, or `managed_commit_failed`.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Success. |
| `1` | `invalid_result`, `no_capability`, `unreadable_bep`, `managed_commit_failed`, or a Bazel launch failure. |
| `2` | Usage or scope error, a conflicting diagnostics flag, a rejected arg, or a failed label query. |
| Bazel's code | Bazel failed. |

## Codegen pairs

The plan admits these generator and language pairs: protobuf/rust, flatbuffers/cpp, flatbuffers/rust, flatbuffers/java, capnp/cpp, capnp/rust, capnp/java, thrift/cpp, thrift/rust, thrift/java, avro/cpp, openapi/typescript, openapi/go, openapi/python, and openapi/rust.

Each pair has a library rule and a shard rule. The shard rule binds a plan entry, matched by an `EXEC_PATH` suffix, to a generated file. Select generated sources with `dx sync`.

| Pair | Library rule | Shard rule |
| --- | --- | --- |
| protobuf/rust | `rust_prost_library` | `prost_codegen_shard` in `@rules_dx//generation:codegen.bzl` |
| openapi/typescript | `openapi_typescript_client` in `@rules_dx//api_spec/rules:codegen.bzl` | `openapi_codegen_shard` |
| openapi/go | `openapi_go_client` in `@rules_dx//api_spec/rules:codegen.bzl` | `openapi_codegen_shard` |
| openapi/python | `openapi_python_client` in `@rules_dx//api_spec/rules:codegen.bzl` | `openapi_codegen_shard` |
| openapi/rust | `openapi_rust_client` in `@rules_dx//api_spec/rules:codegen.bzl` | `openapi_codegen_shard` |
| flatbuffers | `flatbuffers_cc_library`, `flatbuffers_rust_library`, `flatbuffers_java_library` in `@rules_dx//flatbuffers/rules:codegen.bzl` | `flatbuffers_codegen_shard` |
| capnp | `capnp_cc_library`, `capnp_rust_library`, `capnp_java_library` in `@rules_dx//capnproto/rules:codegen.bzl` | `capnp_codegen_shard` |
| thrift | `thrift_cc_library`, `thrift_rust_library`, `thrift_java_library` in `@rules_dx//thrift/rules:codegen.bzl` | `thrift_codegen_shard` |
| avro | `avro_cc_library`, `avro_java_library` in `@rules_dx//avro/rules:codegen.bzl` | `avro_codegen_shard` |

```starlark
load("@rules_dx//generation:codegen.bzl", "prost_codegen_shard")

prost_codegen_shard(
    name = "api_codegen",
    entries = ["gen/api.rs|gen|api|api_proto.lib.rs"],
    proto_rs = ":api_proto_rs",
)
```

- gRPC for Rust needs no extra rule. `rust_prost_library` emits tonic stubs when the proto has a `service`, because the `rules_rust_prost` default toolchain sets `tonic_plugin` and `tonic_runtime`. Depend on `@rules_rust_prost//private/3rdparty/crates:tonic` and `:tokio` to serve or call it, as in `//rust/tests/tonic:greeter_test`. A custom `rust_prost_toolchain` must set `tonic_plugin`, `tonic_plugin_flag` and `tonic_runtime` together.
- Set `language = "rust"` or `language = "java"` on the shard of a Rust or Java library. Bind a Rust shard to the `NAME_generated` target.
- A Java shard entry is the srcjar of one direct dep.
- A shard lists the files of the libraries in the `deps` of its library rule only.
- `flatbuffers_cc_library` takes `gen_object_api = True` for the object API. `flatbuffers_rust_library` takes a `root` schema, required when the library owns more than one.
- `capnp_rust_library` needs one `capnp_rust_library` in `deps` for each `capnp_library` dep. Every schema for `capnp_java_library` needs the `$Java.package` annotation from `/capnp/java.capnp`. A relative capnp import must stay in the schema's directory. Use an absolute `/pkg/file.capnp` import across packages.
- `avro_cc_library` takes `namespace`. Keep each Avro type in one `.avsc` file or use an `.avdl`. An `.avdl` gives one `<stem>.hh`. Read Java classes with `SpecificDatumReader` using `-Dorg.apache.avro.SERIALIZABLE_PACKAGES=<your.package>`.
- Thrift C++ output is a directory and has no shard yet. Avro has no Rust codegen.
- OpenAPI roots must be OpenAPI 3.0, 3.1, or 3.2. A remote `$ref` or an undeclared file `$ref` fails the build. `root` is required when the spec has several roots.
- capnp and thrift codegen build for Linux x86_64 only.
