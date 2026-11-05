# C/C++ compilation context

`//cc/context:compile_db` emits a clang-tidy consumption bundle for one Bazel target.

```sh
bazel build //cc/context:identity //cc/context:identity_exec
bazel cquery --output=files //cc/context:identity //cc/context:identity_exec
bazel aquery "mnemonic(CppCompile, deps(//pkg:target))" --output=jsonproto > actions.json
bazel run //cc/context:compile_db -- \
  --aquery actions.json \
  --execroot "$(bazel info execution_root)" \
  --label //pkg:target \
  --require-source pkg/target.c \
  --config pkg/.clang-tidy \
  --output out \
  --identity <first file from cquery> \
  --identity <second file from cquery>
```

The output directory holds `compile_commands.json` and the bound `.clang-tidy`. Run
clang-tidy against it with `-p out`.

## Flags

- `--aquery <path|->`: the `bazel aquery --output=jsonproto` record to read, `-` for stdin.
- `--execroot <dir>`: the execroot the record's relative paths resolve against; must be absolute.
- `--label <label>`: the target whose compile actions must be present.
- `--require-source <path>`: a source that must have a compile command; repeatable.
- `--config <path>`: the checked-in `.clang-tidy` policy to bind; required.
- `--output <dir>`: directory to write the bundle into.
- `--identity <file>`: compiler identity from `//cc/context:identity` or `//cc/context:identity_exec`; required, repeatable, one per configuration.

## Fail-closed behaviour

The command exits non-zero and writes no bundle when the record has no compile action for
`--label`, a `--require-source` has no compile command, one source has two conflicting
compile commands, `--execroot` is relative, `--config` is missing, empty or not named
`.clang-tidy`, no `--identity` is given, an action's configuration has no identity or an unknown driver,
or an MSVC action has neither `INCLUDE` in its env nor `/imsvc`, `-imsvc`, `/winsysroot` or `-winsysroot` in argv, or no identity target.

Every recorded argument reaches `compile_commands.json` unchanged, including the
compiler, its sysroot and include flags and the macros. MSVC entries also get, right
after argv0, the context cl.exe read from the action env: one `/imsvc<dir>` per
`INCLUDE` then `EXTERNAL_INCLUDE` directory, `/X` so clang-cl does not look for a Visual
Studio, and `--target=<identity target>` unless the arguments carry a target. When the env has no `INCLUDE` but argv already has `/imsvc`, `-imsvc`, `/winsysroot` or `-winsysroot` (clang-cl from the llvm module), nothing is added and the arguments are exported unchanged. `@response` files expand with
the quoting of the selected driver. A flag the
clang driver does not know is a clang-tidy error, never a run without flags.

## Qualification

`bazel run //cc/context:qualify` runs the exporter on real configured actions and
checks every exported command against Bazel's own argv, with `@param` files taken from
aquery `paramFiles` rather than this crate's tokenizer. Run it from a host that has the
compiler:

```sh
bazel run //cc/context:qualify -- --target //cc/tests/fixtures/compilation_db:probe_plain \
  --compiler llvm --output /tmp/qualify.json
bazel run //cc/context:qualify -- --target //cc/tests/fixtures/compilation_db:probe_plain \
  --compiler msvc --output qualify-msvc.json   # Windows host
```

Extra Bazel flags (for a custom toolchain) go after `--`. The evidence JSON records the
git SHA, Bazel version, compiler banner, every command run and every qualified entry.

The qualifier builds and its unit tests pass on `linux_x86_64` and `windows_x86_64` CI
(evidence run 41989006621). On `windows_x86_64` CI (evidence run 42124336539, SHA 43e0f98127ee), the qualifier
reported `1 compile command(s) qualified` for `probe_plain` with `--compiler llvm` and
with `--compiler msvc`. A custom (non-LLVM, non-MSVC) toolchain has not been qualified
and is not claimed as supported. Dispatch `evidence.yml` with `qualify: true` on
`windows_x86_64` to rerun; the artifact holds `qualify-llvm.json`, `qualify-msvc.json`
and `msvc-vswhere.json`.

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
