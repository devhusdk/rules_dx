# D

Build, run and test D with the pinned `rules_d` 0.12.1 and LDC 1.43.0.
Nothing needs to be installed. rules_dx registers the toolchain, so there is
no setup in `MODULE.bazel`. Linking uses the hermetic LLVM C++ toolchain.

## Dependencies

### DUB packages

`dx update` runs `bazel run @rules_dx//d/private/tools/lock -- --dir <set directory>`. It reads `dub.json`,
resolves exact versions from the `code.dlang.org` API, downloads each archive and writes
`dub.lock.json` and the state file `dub.selections.json`. Unchanged packages are kept without a download.

With `dub.lock.json` beside the root `dub.json`, `dx generate --apply` adds the `dub.lock` stanza for the `@dub` hub to `MODULE.bazel`.

Each package is the label `@dub//<name>`, for example
`@dub//silly`; put it in the `deps` of a `d_library`, `d_binary` or
`d_test`. The package's own `dependencies` become deps. `sourcePaths`, `sourceFiles`,
`importPaths` and `stringImportPaths` of its `dub.json` set the sources and imports; without
`sourcePaths` the `source` or `src` directory is used, and without `stringImportPaths` the
`views` directory is used if it exists.

Only registry versions are supported: a `path` or `repository` dependency fails, and so does a
package that has a `dub.sdl` instead of a `dub.json`.

## Generate

`dx generate` writes one `d_library` per directory of D modules. See
`gazelle/d/testdata/modules/README.md` for the module rules.

Next to a `dub.json`, generation reads the import roots (`importPaths`, else `sourcePaths`, else
`source` or `src`) and fails when a package's modules need a different root. With
`"targetType": "executable"`, the package of the root that defines `main` becomes a `d_binary`.

An import resolves to `@dub//<package>` when its first segment is the name of a dependency in
`dub.json` (with `-` read as `_`) and `dub.selections.json` pins it. Set
`# gazelle:dx_external dub dub.json <repo>` in a BUILD file to name the hub repo other than `dub`, for example
`dub_packages`. `# gazelle:resolve d <module> <label>` overrides one import.

## Quality

`dx format` has no D formatter.

## Coverage

`bazel coverage` and `dx coverage` build D libraries and tests with `-cov`, run each test under a small launcher, and convert the `.lst` files ldc writes into LCOV. Coverage covers the sources of targets that match `--instrumentation_filter`. The test's own sources count only with `--instrument_test_targets`. Tag a test `no-coverage` to skip it.

## API docs

`dx docs` documents every `d_library` in scope with the pinned LDC: `ldc2 -D -X`
reads the ddoc comments, and `d_doc_shard` (`@rules_dx//tools/docs/d:defs.bzl`)
turns them into the API reference. Nothing needs to be installed.

## Platforms

See the [support matrix](https://github.com/devhusdk/rules_dx/blob/main/docs/support-matrix.md).

On Windows arm64 the x64 LDC runs under emulation.
