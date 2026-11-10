# C/C++ compilation context

`cc_context` (`src/lib.rs`) turns a `bazel aquery --output=jsonproto` record into a
clang-tidy consumption bundle for one Bazel target: `compile_commands.json` and the bound
`.clang-tidy`, to be run with `clang-tidy -p <bundle>`. The crate has no binary; the
`//cc/private/context:identity` and `//cc/private/context:identity_exec` targets provide the compiler identity.

## Fail-closed behaviour

The exporter returns an error and writes no bundle when the record has no compile action for
the label, a required source has no compile command, one source has two conflicting
compile commands, the execroot is relative, the `.clang-tidy` policy is missing, empty or not named
`.clang-tidy`, no identity is given, an action's configuration has no identity or an unknown driver,
or an MSVC action has neither `INCLUDE` in its env nor `/imsvc`, `-imsvc`, `/winsysroot` or `-winsysroot` in argv, or no identity target.

Every recorded argument reaches `compile_commands.json` unchanged, including the
compiler, its sysroot and include flags and the macros. MSVC entries also get, right
after argv0, the context cl.exe read from the action env: one `/imsvc<dir>` per
`INCLUDE` then `EXTERNAL_INCLUDE` directory, `/X` so clang-cl does not look for a Visual
Studio, and `--target=<identity target>` unless the arguments carry a target. When the env has no `INCLUDE` but argv already has `/imsvc`, `-imsvc`, `/winsysroot` or `-winsysroot` (clang-cl from the llvm module), nothing is added and the arguments are exported unchanged. `@response` files expand with
the quoting of the selected driver. A flag the
clang driver does not know is a clang-tidy error, never a run without flags.

## Qualification

`bazel run //cc/private/context:qualify` runs the exporter on real configured actions and
checks every exported command against Bazel's own argv, with `@param` files taken from
aquery `paramFiles` rather than this crate's tokenizer. Run it from a host that has the
compiler:

```sh
bazel run //cc/private/context:qualify -- --target //cc/tests/fixtures/compilation_db:probe_plain \
  --compiler llvm --output /tmp/qualify.json
bazel run //cc/private/context:qualify -- --target //cc/tests/fixtures/compilation_db:probe_plain \
  --compiler msvc --output qualify-msvc.json   # Windows host
bazel run //cc/private/context:qualify -- --target //cc/tests/fixtures/compilation_db:probe_plain \
  --output qualify-custom-gcc.json -- \
  --extra_toolchains=//cc/private/toolchains/external:custom_gcc_x86_64   # linux_x86_64 host
```

Extra Bazel flags (for a custom toolchain) go after `--`. The evidence JSON records the
git SHA, Bazel version, compiler banner, every command run and every qualified entry.

The qualifier builds and its unit tests pass on `linux_x86_64` and `windows_x86_64` CI
(evidence run 41989006621). On `windows_x86_64` CI (evidence run 42124336539, SHA 43e0f98127ee), the qualifier
reported `1 compile command(s) qualified` for `probe_plain` with `--compiler llvm` and
with `--compiler msvc`.

A consumer-registered custom toolchain is supported on `linux_x86_64`. With only
`--extra_toolchains=//cc/private/toolchains/external:custom_gcc_x86_64` and no `--compiler`
(SHA 94f1917d6a3b, Bazel 9.3.1, `x86_64-linux-gcc (GCC) 16.2.0`), the qualifier reported
`1 compile command(s) qualified` for `probe_plain` with driver `Gnu` and argv0 the
custom archive's `gcc`. A command whose identity has an unknown driver, or none, fails
with `no compiler identity with a known driver` instead of passing.

Dispatch `evidence.yml` with `qualify: true` on `windows_x86_64` or `linux_x86_64` to
rerun; the artifact holds `qualify-llvm.json`, `qualify-msvc.json` and
`msvc-vswhere.json` on Windows, and `qualify-custom-gcc.json` on Linux.

## Fixture records

`//cc/tests/fixtures/compilation_db` holds one aquery record per fixture target, taken
with the consumer's own clang as the C compiler:

```sh
CC="$(bazel info output_base)/external/llvm++llvm_toolchain_minimal+llvm-toolchain-minimal-linux-amd64/bin/clang"
bazel aquery "mnemonic(CppCompile, //cc/tests/fixtures/compilation_db:probe_legacy_upstream)" \
  --output=jsonproto --repo_env=CC="$CC" > probe_legacy.aquery.json
```

The recorded compiler is the configured compiler path, absolute because Bazel writes it
that way.
