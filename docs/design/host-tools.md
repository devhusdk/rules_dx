# Host tools design

This page fixes how build actions, tests and launchers get a shell and POSIX tools. Conversions follow it.

## PATH

- An action or test never puts a host directory on `PATH`. `/usr/bin`, `/bin`, `/usr/local/bin` and the inherited `$PATH` are host directories.
- `PATH` holds only directories of declared inputs.
- An action that needs no program from `PATH` sets `PATH=/dx_no_path`.
- No action sets `use_default_shell_env = True`.

## Shell

- rules_dx pins no shell.
- rules_dx writes no shell scripts for actions, tests or launchers. This covers `run_shell`, `genrule` `cmd`, `sh_test`, `sh_binary` and files with a `#!` shell line.
- Logic that needs more than one command is a Rust binary in `cli/` or a Starlark rule with one `ctx.actions.run` per program.
- An upstream program may start `/bin/sh` itself, through a `#!/bin/sh` line or `system()`. That is the only host program an action may reach. Windows actions reach none.
- An upstream script that needs `bash` is replaced by a native program under the same file name, placed first on `PATH`. `//cli/ocaml_cc_driver` is the model: it replaces the `conda-ocaml-*` scripts.

## Coreutils

- The pinned coreutils is the uutils multicall binary. `core/coreutils/pins.json` holds one cell per host: Linux, macOS and Windows on x86_64 and arm64, each with a `sha256`.
- The toolchain type is `//core/coreutils:toolchain_type`. Toolchains resolve for the execution platform.
- `coreutils(ctx)` from `//core/coreutils:defs.bzl` returns `binary`, `files` and `path`. A rule adds `files` to its inputs and sets `PATH` to `path`.
- `path` is a directory of applets. `COREUTILS_APPLETS` in `core/coreutils/defs.bzl` is the only list of them. Add an applet there when an upstream script needs it.
- Coreutils serves upstream `/bin/sh` scripts only. rules_dx code never calls an applet to do work it can do in Rust or Starlark.
- `grep`, `sed`, `awk`, `tar`, `gzip`, `find` and `xargs` are not coreutils and are not pinned. Code that needs them is rewritten in Rust.

## Actions

- An action runs the real program, not a launcher. The rule names the program by its path in the tool tree and lists the tree as inputs.
- A rule exposes the program through a provider. `OtpToolInfo` in `erlang/private/providers.bzl` is the model: `program` is the exec path and `files` is the tree.
- Environment variables an upstream program needs are set in Starlark `env`, never by a wrapper script.
- A program that needs an absolute path it cannot compute gets it from a Rust driver, not from `$(cd ... && pwd)`.

## Launchers

- A launcher is what `bazel run`, `bazel test` and `dx` start. It is the native launcher from `//cli/launcher` plus a `.launch` descriptor, on every platform.
- No rule writes a `#!/usr/bin/env bash` launcher.
- A descriptor line that is missing is added to `cli/launcher/src/launch.rs` with a test. It is not worked around in shell.

## Tests

- A test that reads files is a `rust_test` that resolves them through runfiles, or a `starlark_test` with `file_checks`.
- A test that runs a tool is a `rust_test` that starts the tool by its runfiles path with an empty `PATH`.
- No `sh_test`. No `genrule` that calls `bash`, `grep`, `tar`, `dirname` or `env`.

## Archives

- Code writes and reads tar and zip archives with `cli/archive`: sorted entries, zero mtime, uid and gid 0, fixed modes.
- No code starts `tar`, `gzip`, `zip` or `unzip` from `PATH`.

## Repository rules

- A repository rule runs only programs it downloaded by `sha256` or programs of the Bazel distribution.
- `rctx.which` is used only to find a system SDK that cannot be redistributed, such as Xcode or MSVC.

## Checks

- Each converted rule has a test that fails when a host directory returns to its `PATH`.
- `//core/coreutils:applet_probe_test` runs one applet by name with only the applet directory on `PATH`.
- One repo test scans Starlark, shell and Rust sources for host `PATH` directories, `use_default_shell_env = True`, `sh_test` and shell `#!` lines. Its allowlist names the files not yet converted and only shrinks.

## Conversion order

1. Actions that set a host `PATH`.
2. Rule-written bash launchers.
3. `run_shell` actions and `genrule` commands.
4. `sh_test` targets and shell test helpers.
5. Rust code that starts host programs by name.
6. The repo scan test, added with the allowlist of what is left and emptied by the steps above.
