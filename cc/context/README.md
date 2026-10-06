# C/C++ compilation context

`//cc/context:compile_db` emits a clang-tidy consumption bundle for one Bazel target.

```sh
bazel aquery "mnemonic(CppCompile, deps(//pkg:target))" --output=jsonproto > actions.json
bazel run //cc/context:compile_db -- \
  --aquery actions.json \
  --execroot "$(bazel info execution_root)" \
  --label //pkg:target \
  --require-source pkg/target.c \
  --config pkg/.clang-tidy \
  --output out
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

## Fail-closed behaviour

The command exits non-zero and writes no bundle when the record has no compile action for
`--label`, a `--require-source` has no compile command, one source has two conflicting
compile commands, `--execroot` is relative, or `--config` is missing, empty or not named
`.clang-tidy`.