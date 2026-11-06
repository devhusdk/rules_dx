# Adopting rules_dx Without Bazel Experience

Use this when your repo has Cargo, Python, or JavaScript projects and no
Bazel files. `dx` runs Bazel for you. You keep your manifests and lock files
and use `dx` where you used `cargo`, `pytest`, `ruff`, `ty`, `tsc` or `biome`.

## Set up

Install `dx` and Bazelisk as in the [tutorial](tutorial.md). Then, from the
repo root:

```sh
dx init
dx init --apply
dx generate --apply
```

`dx init` shows what it would write. `--apply` writes it. `dx generate` writes
the `BUILD.bazel` files from your manifests.

Declare the lock files in `MODULE.bazel` yourself. Add a `crate.from_cargo`
stanza for `Cargo.lock`, a `uv.project` stanza for `uv.lock` and an
`npm_translate_lock` stanza for your JavaScript lock file. `dx update` reads
them, so your manifest and lock file stay the source of truth.

## Commands

| You ran | Run |
| --- | --- |
| `cargo build`, `npm run build` | `dx build` |
| `cargo test`, `pytest`, `npm test` | `dx test` |
| `cargo clippy`, `ruff check`, `ty`, `tsc`, `biome check` | `dx check` |
| `ruff format`, `cargo fmt`, `biome format` | `dx check --apply` |
| `cargo update`, `uv lock --upgrade`, `npm update` | `dx update` |

Python type checking is `ty`. The JavaScript and TypeScript lint default is
`biome`. Add a scope such as `dx test //pytools/...` to run part of the repo.

## Editors

Run `dx env --apply`. It fills `.dx/local/ide/`, which the Python and TypeScript
settings point at. rust-analyzer uses the rules_rust discover targets that
`dx init` configures. See [Editors](editors.md).

## What to commit

- `tools/dx*`
- `.dx/pins`
- `MODULE.bazel` and `MODULE.bazel.lock`
- `.bazelrc`
- `AGENTS.md`

Ignore the paths `dx init` adds to `.gitignore`. See [Init](cli/commands/init.md).

## Example

[examples/adopt-polyglot](https://github.com/devhusdk/rules_dx/blob/main/examples/adopt-polyglot/README.md) is a mixed
tree inside the rules_dx repository. It is built from the rules_dx root, so it
shows the layout and not a standalone consumer.
