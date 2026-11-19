# D

This directory helps you build, run and test D code with rules_dx.

Load its rules in a `BUILD.bazel` file, the file that declares what Bazel
builds, from `@rules_dx//d:defs.bzl`.
`extensions.bzl` holds the module extension: the part you call from
`MODULE.bazel` to set up toolchains and dependencies.
The tests live in `tests/`. The [D page](../docs/languages/d.md) shows
setup, every rule and the tools dx runs.

Run the D tests from a rules_dx checkout:

```sh
tools/dx test //d/...
```

## Dependencies

### DUB packages

List your dependencies in `dub.json`. Then:

1. Run `dx update --apply`. It resolves exact versions from the
   `code.dlang.org` API, downloads each archive and writes `dub.lock.json`
   and the state file `dub.selections.json`. Packages that did not change
   are kept without a download.
2. Run `dx generate --apply`. With `dub.lock.json` beside the root
   `dub.json`, it adds the `dub.lock` stanza for the `@dub` hub to
   `MODULE.bazel`. A hub is one Bazel repository that holds a target for
   every locked package.
3. Put the package in the `deps` of a `d_library`, `d_binary` or `d_test`,
   as `@dub//<name>`, for example `@dub//silly`.

How each package builds:

- Its own `dependencies` become its deps.
- `sourcePaths`, `sourceFiles`, `importPaths` and `stringImportPaths` in its
  `dub.json` set the sources and imports.
- Without `sourcePaths`, the `source` or `src` directory is used.
- Without `stringImportPaths`, the `views` directory is used if it exists.

Only registry versions work. A `path` or `repository` dependency fails, and
so does a package that has a `dub.sdl` instead of a `dub.json`.

## Generate

`dx generate` writes one `d_library` per directory of D modules.
`gazelle/d/testdata/modules/README.md` shows how modules map to targets.

Next to a `dub.json`:

- Generation reads the import roots: `importPaths`, else `sourcePaths`,
  else `source` or `src`. It fails when a package's modules need a
  different root.
- With `"targetType": "executable"`, the package of the root that defines
  `main` becomes a `d_binary`.
- An import resolves to `@dub//<package>` when its first segment names a
  dependency in `dub.json` (with `-` read as `_`) and `dub.selections.json`
  pins it.

Two directives change resolution. Put them in a BUILD file:

- `# gazelle:dx_external dub dub.json <repo>` names the hub something other
  than `dub`, for example `dub_packages`.
- `# gazelle:resolve d <module> <label>` overrides one import.

## Quality

`dx format` has no D formatter.

## Coverage

`bazel coverage` and `dx coverage` build D libraries and tests with `-cov`.
Each test runs under a small launcher, and the `.lst` files that ldc writes
become LCOV.

- Only sources of targets that match `--instrumentation_filter` are covered.
- The test's own sources count only with `--instrument_test_targets`.
- Tag a test `no-coverage` to skip it.

## API docs

`dx docs` documents every `d_library` in scope with the pinned LDC. Nothing
needs to be installed. `ldc2 -D -X` reads the ddoc comments, and
`d_doc_shard` from `@rules_dx//d:defs.bzl` turns them into the
API reference.

## Platforms

The [support matrix](../docs/support-matrix.md) lists the platforms. On
Windows arm64, the x64 LDC runs under emulation.
