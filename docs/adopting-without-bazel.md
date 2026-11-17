# Adopting rules_dx Without Bazel Experience

This page helps you run a Cargo, Python, or JavaScript repo with `dx` when the
repo has no Bazel files yet.

`dx` runs Bazel for you. You keep your manifests and lock files. You run `dx`
in place of `cargo`, `pytest`, `ruff`, `ty`, `tsc` or `biome`.

## Steps

1. Install `dx` and Bazelisk as in the [tutorial](tutorial.md#install).

2. From the repo root, preview what `dx init` would write, then write it:

   ```sh
   dx init
   dx init --apply
   ```

3. Fill the editor files:

   ```sh
   dx sync
   ```

4. Build and test:

   ```sh
   dx build
   dx test
   ```

5. Commit the files listed in [What to commit](#what-to-commit).

## What `dx init --apply` writes

- `MODULE.bazel`: the file that makes the directory a Bazel module, the unit
  Bazel builds. It gets one stanza per lock file, which tells Bazel to fetch
  the packages that lock file pins.
- `.bazelversion`: the Bazel version Bazelisk runs.
- `.bazelrc`: Bazel's settings file.
- `tools/dx`: the launcher that runs the pinned `dx`.

It ends with `dx generate --apply`, which writes the `BUILD.bazel` files from
your manifests. A `BUILD.bazel` file tells Bazel what to build in its
directory. `dx update` reads the lock file stanzas, so your manifest and lock
file stay the source of truth.

## Example

A repo with `Cargo.toml`, `pyproject.toml` and `package.json` at the root:

```sh
dx init --apply
dx check --apply
dx test
dx test //pytools/...
```

`dx check --apply` formats and writes the lint fixes. `dx test` runs every
test. `//pytools/...` is a scope: it limits the run to the `pytools`
directory and everything under it.

[examples/adopt-polyglot](https://github.com/devhusdk/rules_dx/blob/main/examples/adopt-polyglot/README.md)
is a mixed tree inside the rules_dx repository. It is built from the rules_dx
root, so it shows the layout and not a standalone consumer.

## Commands

| You ran | Run |
| --- | --- |
| `cargo build`, `npm run build` | `dx build` |
| `cargo test`, `pytest`, `npm test` | `dx test` |
| `cargo clippy`, `ruff check`, `ty`, `tsc`, `biome check` | `dx check` |
| `ruff format`, `cargo fmt`, `biome format` | `dx check --apply` |
| `cargo update`, `uv lock --upgrade`, `npm update` | `dx update --apply` |

`dx update` without `--apply` only checks. Python type checking is `ty`. The
JavaScript and TypeScript lint default is `biome`.

## Editors

Run `dx sync`. It fills `.dx/local/ide/`, which the Python and TypeScript
settings in `.vscode/settings.json` point at. rust-analyzer uses the rules_rust
discover and flycheck launchers that `dx sync` copies to
`.dx/local/ide/rust_analyzer/`. Other editors are on the
[Editors](editors.md) page.

## What to commit

- `tools/dx*`
- `.dx/pins`
- `MODULE.bazel` and `MODULE.bazel.lock`
- `.bazelrc`
- `AGENTS.md`

Do not commit the paths `dx init` adds to `.gitignore`. The full file list is
in [`dx init`](cli/commands/init.md).
