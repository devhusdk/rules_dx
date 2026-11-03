# `dx env`

`dx env --apply` also refreshes the editor files in `.dx/ide/`, for example the TypeScript `lib` link and, for C and C++ trees, `cc/compile_commands.json` and the `cc/clangd` link. For Go trees it writes `go/gopackagesdriver`, a wrapper around the rules_go packages driver. Point `go.toolsEnvVars.GOPACKAGESDRIVER` at it. A failed refresh prints one `ide_refresh_failed` warning and never changes the exit code.

```sh
bazel run @rules_dx//:dx -- env
bazel run @rules_dx//:dx -- env --apply
```

```text
dx env [--apply] [--shell=direnv] [<label>] [-- bazel-options...]
```

`dx env` collects the managed tools for `.dx/bin`, the env values, and the
declared generated source in one Bazel build. It upgrades nothing. It is
repository-wide by default.

After you edit a `.capnp`, `.fbs` or OpenAPI spec that a capnproto, flatbuffers
or api_spec codegen rule reads, run `dx env --apply`. The selected outputs are
linked under `.dx/setups/current/generated`.

With one exact `//` or `@` label, `dx env` acts on that label plus every
codegen shard that depends on it. For example, `dx env //proto:api` also
builds each codegen shard that depends on `//proto:api`. dx finds the shards
in the workspace rule graph (`bazel query "kind('rule', //...)"`) and does
not load external repos.

A pattern, a relative label, a path, an empty value or a second label is
a usage error and exits `2`.

`--shell=direnv` prints the lines the scaffolded `.envrc` evaluates:
`PATH_add .dx/bin` and `watch_file .dx/bin`. It runs no Bazel, writes
nothing, and ignores `--apply`, the label and `--output`. If `.dx/bin` is
missing, it writes a `missing .dx/bin` message to stderr and exits `1`.
Any other value is a usage error.

It checks the intended selection by default and reports drift without
writing. Pass `--apply` to select it. A missing selection is drift, not
success. Checking never selects a new generation, populates managed
bins, or commits records.

`dx env` also passes
`--@rules_dx//config:clippy_output_diagnostics=true`
and `--@rules_dx//config:rustc_output_diagnostics=true`. Another value for one of them,
or for the matching `@rules_rust//rust/settings` flag, fails before Bazel starts with exit `2`.
These args fail the same way: `--aspects`, `--output_groups`,
`--remote_download_minimal`, `--remote_download_outputs=minimal`,
`--build_event_json_file`, any `--build_event_json_file_path_conversion` other
than `false`, `--test_arg`, and the startup options `--bazelrc`, `--home_rc`,
`--nohome_rc`, `--system_rc`, `--nosystem_rc`, `--output_base`,
`--output_user_root`, `--host_jvm_args` and `--server_jvm_out`.
Pass `--output_base` or `--output_user_root` with
[`--bazel-startup-option`](README.md#global-flags) instead.
It passes `--remote_download_outputs=toplevel` after your options, so a `--config`
cannot stop the plan outputs from downloading. Pass `--remote_download_outputs=all`
or `--remote_download_all` to keep downloading all outputs.

`dx env` keeps managed state under `.dx/`, which is gitignored.
`dx clean` reports unselected managed state. `dx clean --apply` prunes it.
It takes no `--fail-on` or `--report`.

Output: `--output text|json`.

Text output starts with `Running env for <roots>`. Then dx prints
one result line, where `<pair>` is `(environment <id>, generated <id>)`:

- Up to date: `dx env: already selected setup <setup> <pair>`.
- Drift with a selection:
  `dx env: stale selection: current setup <setup> want setup <setup> <pair>`.
- Drift with no selection:
  `dx env: no current selection: intended setup <setup> <pair>`.
- With `--apply`: `dx env: selected setup <setup> <pair>`, or
  `already selected setup` when nothing changed.

Drift adds `` run `dx env --apply [<label>]` to select it ``.
`--quiet` hides all of these lines, drift included. The exit status stays
the same.

JSON output writes these events in order:

- `command_started`, with mode `check` or `default` (`--apply`).
- `operation`, with phase `collect`. Its `scope` is the label and the
  codegen shards that depend on it. The whole repository has no `scope`.
- On drift, a `notice` with code `managed_drift`. Its message names the
  command that selects the setup, such as `dx env --apply`.
- With `--apply`, a `selection` with `setup`, `environment` and
  `generated`.
- When Bazel fails, an `error` with code `bazel_failed` and phase
  `collect`.
- Last, `command_finished`, with the build's Bazel `actions` counts.

Other failures write an `error` with its code, then `command_finished`.
Codes include `invalid_arguments`, `invalid_result`, `no_capability`,
`unreadable_bep` and `managed_commit_failed`.

Exit codes: `0` success. `2` usage or scope errors: a bad scope, a
conflicting diagnostics flag, a rejected arg after `--`
(`--aspects`, `--output_groups`, `--build_event_json_file`, `--test_arg` or a
startup option), or a failed label query. The JSON `error` keeps the query status. `1` drift, an
invalid collection result (`invalid_result`), no capability
(`no_capability`), a non-UTF-8 temporary event path (`unreadable_bep`),
a commit failure (`managed_commit_failed`), or a Bazel launch failure.
Bazel failures keep Bazel's code.

```sh
bazel run @rules_dx//:dx -- env //a:one
bazel run @rules_dx//:dx -- env --apply //proto:api
bazel run @rules_dx//:dx -- env --apply
bazel run @rules_dx//:dx -- env --output json
```

## Codegen pairs

The codegen plan admits the generator/language pairs `("protobuf", "rust")`,
`("flatbuffers", "cpp")`, `("flatbuffers", "rust")`, `("flatbuffers", "java")`, `("capnp", "cpp")`, `("capnp", "rust")`, `("capnp", "java")`, `("avro", "cpp")` and `("openapi", "typescript")`.

The protobuf/rust pair comes from `prost_codegen_shard` in
`@rules_dx//generation:codegen.bzl`. Point `proto_rs` at a `rust_prost_library`
and list one entry per generated file. Each entry's `EXEC_PATH` is the file
name prost writes, such as `result_proto.lib.rs`.

```starlark
load("@rules_dx//generation:codegen.bzl", "prost_codegen_shard")

prost_codegen_shard(
    name = "api_codegen",
    entries = ["gen/api.rs|gen|api|api_proto.lib.rs"],
    proto_rs = ":api_proto_rs",
)
```

The openapi/typescript pair comes from `openapi_typescript_client` in
`@rules_dx//api_spec/rules:codegen.bzl`, which turns one OpenAPI root of an
`api_spec_library` into a single self-contained `NAME.ts` with the pinned
`@redocly/cli` `generate-client`. Every generator flag is fixed in the rule
(`--output-mode=single --runtime=inline --import-ext=js --generator=typescript
--args-style=grouped --error-mode=throw --date-type=string`), the config is dx's
own, and the action runs without network. `root` is required when the spec has
more than one root. `openapi_codegen_shard` binds one plan entry with an
`EXEC_PATH` suffix to the generated file.

A dx preflight runs before Redocly and fails the build with
`path:line:col rule message` when the root is not an OpenAPI 3.0, 3.1 or 3.2
document (AsyncAPI and Swagger roots are refused), when a `$ref` names a remote
document (`dx/api-spec-remote-ref`), or when a `$ref` names a file the spec does
not declare (`dx/api-spec-unresolved-ref`). Only OpenAPI is supported; AsyncAPI
generation is a separate pair. Verify generated sources with
`dx env`, which fails with "no current selection" until
`dx env --apply` has run and with "stale selection" after the plan changes.

The flatbuffers/cpp pair comes from `flatbuffers_cc_library` in
`@rules_dx//flatbuffers/rules:codegen.bzl`, which runs the pinned `flatc`
(`--cpp --keep-prefix`) once per `flatbuffers_library` schema over its declared
include closure, and from `flatbuffers_codegen_shard`, which binds one plan entry
with an `EXEC_PATH` suffix to each generated header. The headers are compiled
against the C++ runtime of the same pinned archive, so the compiler and the
runtime cannot drift. `gen_object_api = True` adds the object API; each variant writes under its own `_fbs` or `_fbs_obj` directory, so both can share one library. The shard
lists the headers of the libraries in the `deps` of the `flatbuffers_cc_library`
only; a library reached through those deps needs its own target and shard.

The flatbuffers/rust pair comes from `flatbuffers_rust_library` in the same file.
It makes one crate per root schema: `flatc --rust --gen-all` runs on the `root`
(optional when the library owns one schema, otherwise required and one of its
srcs) and writes one `<root>_generated.rs` that inlines the include closure,
because flatc's Rust output uses `crate::` and `super::` paths that only compile
inside one crate. The crate links the Rust runtime built from the same pinned
archive. Bind the shard to the `NAME_generated` target, not to `NAME`; it binds
only the flatc file, never the `lib.rs` dx writes. Two crates whose roots share a
schema each get their own copy of its types, and those copies are not
interchangeable.

The flatbuffers/java pair comes from `flatbuffers_java_library`, which runs
`flatc --java` once per `flatbuffers_library` (one compile per library, shared by
every library that depends on it), packs the output into
`_fbs_java/<lib>.srcjar` and compiles it with `java_common.compile`. The Java
runtime is built from the same pinned archive and not taken from Maven, because
the latest `flatbuffers-java` on Maven (25.2.10) is older than the generated
code's version check. Bind `flatbuffers_codegen_shard` with `language = "java"`
to the `flatbuffers_java_library`; each entry's `EXEC_PATH` is the srcjar of one
direct dep. For breaking-change checks see `flatbuffers_conform_test` in
[quality](quality.md#breaking-change-check).

The capnp/cpp pair comes from `capnp_cc_library` in
`@rules_dx//capnproto/rules:codegen.bzl`, which runs the pinned `capnp compile`
with `capnpc-c++` once per `capnp_library` and compiles the output once per
library, so two C++ targets that share a schema library never link duplicate
symbols. The declared import closure (the library's schemas, its deps and the
bundled `/capnp/*.capnp`) is staged as symlinks under a private root and compiled
with `--no-standard-import`, so an import outside `deps` fails with or without a
sandbox. Relative imports resolve inside that root, but the generated headers
include them relatively, so a relative import must stay in the importing
schema's own directory; use an absolute `/pkg/file.capnp` import across packages.
`capnp_codegen_shard` binds one plan entry with an `EXEC_PATH` suffix to each
generated `.capnp.h` and `.capnp.c++` file; the shard lists the files of the
libraries in the `deps` of the `capnp_cc_library` only. The pin is built for
Linux x86_64 only, as for `dx quality`.

The capnp/rust pair comes from `capnp_rust_library(name, library, deps)` in the
same file. It generates one crate per `capnp_library`, with the pinned `capnp`
compiler and the `capnpc` code generator of the pinned `capnproto-rust` archive.
The crate's `deps` must mirror the library's direct deps: one `capnp_rust_library`
for each `capnp_library` in the library's `deps`, because Rust sees only direct
dependencies. A missing one fails at analysis. An import of a dependency's
schema becomes a `::dep_crate::x_capnp` path, so a type is never duplicated
across crates, and `/capnp/schema.capnp` maps to the `capnp` crate. An import
that is neither owned by the library nor provided by a dependency fails the
build, naming the import. Each schema becomes a top-level module `<stem>_capnp`,
so two schemas of one library with the same file stem fail at analysis. Annotation
schemas such as `/capnp/c++.capnp` generate no code. `crate_name` renames the
crate for dependents. Bind `capnp_codegen_shard` with `language = "rust"` to the
`NAME_generated` target; it lists the generated `*_capnp.rs` files, never the
`lib.rs` the driver writes.

The capnp/java pair comes from `capnp_java_library` in the same file, which runs
`capnp compile` with the `capnpc-java` plugin (capnproto-java, built from source)
once per `capnp_library`, packs the output into `_capnp_java/<lib>.srcjar` and
compiles it with `java_common.compile` against the Java runtime built from the
same archive. Every schema needs the `$Java.package` annotation from
`/capnp/java.capnp`, which is bundled with the other `/capnp/` schemas; without it
`capnpc-java` fails with `no Java package name found`. Bind `capnp_codegen_shard`
with `language = "java"` to the `capnp_java_library`; each entry's `EXEC_PATH` is
the srcjar of one direct dep. Both pairs are built for Linux x86_64 only.

The avro/cpp pair comes from `avro_cc_library(name, deps, namespace)` in
`@rules_dx//avro/rules:codegen.bzl`. It runs `avrogencpp` once per `.avsc` in the
`avro_library` closure and links the `avro-cpp` runtime. For an `.avdl`, it first
runs the pinned avro-tools `idl2schemata`. `namespace` is the C++ namespace of the
generated types. `avrogencpp` cannot resolve a reference to a type in another
`.avsc` file, so keep each type in one file or use an `.avdl`. Bind
`avro_codegen_shard` with `language = "cpp"` to the library; each entry's
`EXEC_PATH` is the header of one `.avsc`. Headers made from `.avdl` files are built
but not listed in the shard. Avro Rust codegen is not offered.

The avro/java pair comes from `avro_java_library(name, deps)` in
`@rules_dx//avro/rules:codegen.bzl`. It runs the pinned `avro-tools` on the
`.avsc` and `.avdl` schemas of the `avro_library` deps and their imports, packs
the output into `<name>.srcjar` and compiles it. Dependents get the Avro
runtime. Reading a generated class with `SpecificDatumReader` needs
`-Dorg.apache.avro.SERIALIZABLE_PACKAGES=<your.package>`. A schema reachable
from two `avro_java_library` targets is generated in both. Bind
`avro_codegen_shard` with `language = "java"` to the `avro_java_library`; its
one entry's `EXEC_PATH` is the srcjar.
