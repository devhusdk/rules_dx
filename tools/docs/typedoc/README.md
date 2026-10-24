# TypeScript API docs

`typedoc_site` builds a browsable API reference for one TypeScript project.

```starlark
load("@rules_dx//tools/docs/typedoc:defs.bzl", "typedoc_site")

typedoc_site(
    name = "api_docs",
    srcs = glob(["src/*.ts"]),
    entry_points = ["src/index.ts"],
    package = "ledger",
    source_url = "https://github.com/acme/ledger/blob/main/{file}#L{line}",
    tsconfig = "tsconfig.json",
    deps = ["//:node_modules/csstype"],
)
```

```sh
dx docs //lib:api_docs
```

The site lands in `bazel-bin/lib/api_docs`. It has one `api.html` page, a search
index, and a link from every symbol to its source line.

Attributes:

- `srcs`: the TypeScript sources.
- `tsconfig`: the project `tsconfig.json`.
- `entry_points`: the public entry files. Only what they export is documented.
- `package`: the package name used in symbol IDs.
- `deps`: npm packages the sources import, for example `//:node_modules/csstype`.
- `options`: extra TypeDoc flags, for example `--excludeNotDocumented`.
- `source_url`: link template with `{file}` and `{line}`.
- `title`: page title. Defaults to `<package> API reference`.

`typedoc_ir` produces only the `<name>.ir.pb` docs IR shard.

Private, protected and `@internal` members are left out. A missing entry
point, a type error, a missing dependency, or an unsupported TypeDoc type
fails the build and no site is written.

Supported shapes: ES modules with `export`, re-exports (`export { a as b }`),
classes, interfaces, enums, type aliases, functions with overloads, and
variables.
