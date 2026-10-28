# `dx docs`

```text
dx docs [--check] [--serve [--port <n>] [--host <addr>] [--open]] [--here] [scope...] [-- bazel-options...]
```

Builds the documentation site with Bazel and the pinned upstream mdBook:
every `dx` command page, the GitHub CI guide, the examples, and an API reference
for every public Rust crate.
With no scope it builds the repository site (`//tools/docs/site:user_site`,
`//tools/docs/site:user_site_check` in `--check`). Args after `--` go to Bazel unchanged.
The build also passes
`--@rules_dx//config:clippy_output_diagnostics=true`
and `--@rules_dx//config:rustc_output_diagnostics=true`. Another value for one of them,
or for the matching `@rules_rust//rust/settings` flag, fails before Bazel starts with exit `2`.
dx spells them with the name your MODULE.bazel gives rules_dx, for example
`--@my_dx//config:clippy_output_diagnostics=true` when `repo_name = "my_dx"`.

Scopes: Bazel labels or patterns (`//...`, `//pkg:target`), or
workspace-relative files or dirs resolved through Bazel query. `--here` builds
the current directory tree (`//path/...`, or `//...` at the root) and cannot be
combined with explicit scopes (exit `2`).

- `--check`: validate the book without rendering it. Builds the generated
  summary and landing page only.
- `--serve`: build, then preview the built site tree locally with
  `python3 -m http.server`. Needs `python3` on `PATH` and exactly one built
  site tree with an `index.html` entry page.
- `--port <n>`: preview port, `1`-`65535`, default `8000`. `0` is a usage error.
- `--host <addr>`: preview bind address, non-empty, default `127.0.0.1`.
- `--open`: open the preview URL in a browser through `python3`. If that
  fails, `dx` prints a warning and the preview keeps running.

`--port`, `--host`, and `--open` need `--serve`.

Output: `--output text|json`. `--output diff` is a usage error (exit `2`).
Text prints `Running docs build for <scope> (mdBook render)`, or
`Running docs check for <scope> (no render)` with `--check`. With `--serve`
it then prints `Serving docs at <url> (<dir>)`, and `Opening <url>` with
`--open`. JSON starts with `command_started` (mode `check` or `default`),
then `operation` events (`summary`, plus `render` without `--check`). On
failure one `error` follows: `launch_failed` when Bazel cannot be launched,
`bazel_failed`, `disk_full`, `bazel_signalled`, or `serve_failed`. The stream
ends with `command_finished`. A usage or option error fails before the command
starts: one `error` with `invalid_arguments`, then `command_finished`, and no
`command_started`. A scope that does not resolve fails the same way with
`invalid_scope` (missing path, external or relative label), `no_owner`,
`query_failed` (the Bazel query exited nonzero), or `query_launch_failed` (the
query could not start). Under `--serve`, `command_finished` comes when the
preview server exits.

Reports: none. `--report` is a usage error.

Exit codes: `0` success, `2` usage errors, including `--here` with an
explicit scope, `--output diff`, and `--report`, and scope errors
(`invalid_scope`, `no_owner`, `query_failed`). `1` a launch, signal, or
preview failure, including `query_launch_failed` (the scope query cannot be
launched), `launch_failed` (Bazel cannot be launched),
`serve_failed` (no single site tree, a missing `index.html`, or a preview that
fails to launch) and a signalled preview. Bazel failures keep Bazel's code.
`disk_full` keeps exit `36`: Bazel exited 36 and a disk it writes to has under
1 GiB free. dx then suggests `dx worktree prune --apply`. Under `--serve` the
preview server's code is the exit code.

```sh
bazel run @rules_dx//:dx -- docs --check
bazel run @rules_dx//:dx -- docs
bazel run @rules_dx//:dx -- docs --serve --port 8080
bazel run @rules_dx//:dx -- docs --here
bazel run @rules_dx//:dx -- docs --serve --host 0.0.0.0 --port 8080 --open //tools/docs/site:user_site
bazel run @rules_dx//:dx -- docs --check --output=json
bazel run @rules_dx//:dx -- docs --check -- --config=ci
```

The build never changes sources. Serve runs a local preview only. The site
renders Markdown to HTML with syntax-highlighted code, a sidebar, a search
index, and one route per page.

API pages show `Undocumented.` when the source has no doc comment.

## Rust API reference

`rust_doc_shard` from `@rules_dx//tools/docs/rust:defs.bzl` documents one Rust
library. It builds the crate's rustdoc JSON with the pinned nightly rustdoc, not
your compiler, and produces a docs IR shard with language `rust`. Only public
items get pages. Symbol ids have the form `rust:<crate>:<path>`, where `<path>`
joins the module path with `.`, for example `rust:mylib:shapes.Circle`. Pass the
shard to `docs_site`.

Rust keeps types, values and macros in separate namespaces, so two public items
can share a name. The item that comes first by namespace (types, then values,
then macros, and for members fields and variants, then associated types, then
methods and constants) and then by source position keeps the plain id. Each
other item gets `@<tag>`, for example `rust:mylib:foo@fn` or
`rust:mylib:Point.x@method`, and `@<tag>-2`, `@<tag>-3` when several share a
tag. A glob re-export of a module outside the crate becomes one symbol with the
id `rust:mylib:<module>@glob`. Glob names follow Rust's shadowing rules: an
explicit name wins, and a name given by two globs is dropped. Ids of crates
without collisions never change. Source links use `source_url` plus the file's path in its
repository.

```starlark
load("@rules_dx//tools/docs/rust:defs.bzl", "rust_doc_shard")
load("@rules_dx//tools/docs/site:site.bzl", "docs_site")

rust_doc_shard(
    name = "api_docs",
    crate = ":mylib",
)

docs_site(
    name = "site",
    book_toml = "book.toml",
    prose = ["guide.md"],
    shards = [":api_docs"],
    source_url = "https://github.com/me/mylib/blob/main/",
)
```

```sh
bazel run @rules_dx//:dx -- docs //lib:site
```

A rustdoc or extraction error fails the build, and `dx docs` keeps Bazel's exit code.

## TypeScript API reference

`typescript_doc_shard` from `@rules_dx//tools/docs/typescript:defs.bzl` runs the pinned
TypeDoc over one TypeScript project and produces a docs IR shard with language
`typescript`. Pass the shard to `docs_site` beside any Rust shards, and the shared
pipeline renders the pages. Symbol ids have the form `typescript:<package>:<name>`.

```starlark
load("@rules_dx//tools/docs/typescript:defs.bzl", "typescript_doc_shard")

typescript_doc_shard(
    name = "api_docs",
    srcs = glob(["src/*.ts"]),
    entry_points = ["src/index.ts"],
    package = "mylib",
    tsconfig = "tsconfig.json",
)
```

Build the shard's site with the scope of the target:

```sh
bazel run @rules_dx//:dx -- docs //lib:api_docs
```

Exit codes: `0` success, `2` usage or scope errors, `1` a launch, signal or preview
failure. Bazel failures keep Bazel's code, so a TypeDoc error in the shard fails the build.

The rule runs TypeDoc through an isolated launcher. It never loads plugins, and it
ignores `typedoc.json`, `package.json` and `typedocOptions` in `tsconfig.json`,
including inherited ones. Only the compiler options of the given `tsconfig` are read.
Put extra TypeDoc flags in `options`. The rule owns `plugin`, `options`, `tsconfig`,
`entryPoints`, `json`, `out` and `basePath`, and fails at load time if `options` sets
one of them. TypeDoc still reports an error if `typedocOptions` sets `options` or
`tsconfig`.

## Vue component API reference

`vue_doc_shard` from `@rules_dx//tools/docs/components:defs.bzl` reads the props,
events, slots and exposed members of Vue components with the pinned
`vue-component-meta` and produces a docs IR shard with language `vue`. Pass the
shard to `docs_site` beside any other shards.

```starlark
load("@rules_dx//tools/docs/components:defs.bzl", "vue_doc_shard")

vue_doc_shard(
    name = "ui_docs",
    srcs = glob(["src/*.vue"]),
    entry_points = ["src/Button.vue"],
    node_modules = "//:node_modules",
    package = "ui",
    tsconfig = "tsconfig.json",
)
```

Supported shapes are `.vue` single file components with `<script setup lang="ts">` or
`defineComponent`. Each entry point is one component, taken from its default export.
Symbol ids are `vue:<package>:<Component>`, and the members are
`<Component>.prop_<name>`, `.event_<name>`, `.slot_<name>` and `.exposed_<name>`.
Component names are file names without the extension and must be unique in a shard.
Events and exposed members carry no source link when `vue-component-meta` reports no
declaration for them.

`node_modules` is the consumer's npm link target. Only its `vue` package is read, for
the type declarations. Pass further type packages the components import, for example
`deps = ["//:node_modules/csstype"]`; without them the build fails with TS2307. The
`tsconfig` may sit at any path in the package and may extend files listed in `srcs`. The metadata tools are pinned by dx, so their versions and the
TypeScript version of the extractor are part of the shard's configuration identity.

The rule runs through an isolated launcher. It rejects `vueCompilerOptions.plugins` in
the tsconfig and in every file it extends, refuses to load modules outside its own
`node_modules`, and never runs Vite or other consumer JavaScript. A type error in an
entry point, a missing tsconfig, an entry without a default export and a malformed
result all fail the build with a nonzero exit and never write a stub shard.

Svelte, Astro and MDX components are not supported yet.

## Go API reference

`go_doc_shard` from `@rules_dx//tools/docs/go:defs.bzl` reads one Go library's
exported API with `go/doc` from the pinned Go SDK and produces a docs IR shard with
language `go`. It never runs `go`, the library's code or host tools. Pass the shard
to `docs_site` beside other shards.

```starlark
load("@rules_dx//tools/docs/go:defs.bzl", "go_doc_shard")

go_doc_shard(
    name = "api_docs",
    library = ":money",
)
```

Attributes:

- `library`: the documented `go_library`.
- `package`: the package segment of the symbol ids. Defaults to the library's `importpath`.
- `goos`, `goarch`: the platform the build constraints are evaluated for. Defaults to `linux` and `amd64`, on every host.
- `build_tags`: extra Go build tags.

Symbol ids have the form `go:<importpath>:<Name>`, and `go:<importpath>:<Type>.<Member>`
for methods, struct fields and interface methods. The package itself is
`go:<importpath>:<package name>`. Functions, methods, struct and other defined types,
interfaces, aliases, struct fields, interface methods, constants and variables are
documented. Unexported names and `_test.go` files are left out.

cgo is off. Files left out by build constraints or by `import "C"` are listed in the
package's `go.excluded` extension, and the `go.config` extension records the
configuration on every symbol. Use one configuration per import path in a site: two
shards with the same ids fail the site build.

Build the shard's site with the scope of the target:

```sh
bazel run @rules_dx//:dx -- docs //lib:api_docs
```

Exit codes: `0` success, `2` usage or scope errors, `1` a launch, signal or preview
failure. Bazel failures keep Bazel's code. A parse error, mixed package names,
`package main`, no files left after build constraints, or no exported API fails the
build, and no pages are written.

## Ruby API reference

`ruby_doc_shard` from `@rules_dx//tools/docs/ruby:defs.bzl` reads one Ruby library's
public API with YARD 0.9.45, run by the registered `rules_ruby` toolchain, and produces
a docs IR shard with language `ruby`. YARD parses the sources without running them, and
the sources are never changed. Pass the shard to `docs_site` beside other shards.

```starlark
load("@rules_dx//tools/docs/ruby:defs.bzl", "ruby_doc_shard")

ruby_doc_shard(
    name = "api_docs",
    lib = ":geometry",
)
```

Attributes:

- `lib`: the documented `ruby_library`. Its own `srcs` are documented; files from its `deps` are only read for context.
- `package`: the package segment of the symbol ids. Defaults to the name of `lib`.
- `deps`: `ruby_doc_shard` targets of libraries whose modules this one includes, extends or subclasses. Their symbols are linked, not documented again.

Symbol ids have the form `ruby:<package>:<Namespace>.<Name>`, for example
`ruby:geometry:Geometry.Circle.area`. Singleton methods add a `self` segment, as in
`ruby:geometry:Geometry.Circle.self.unit`. Characters other than letters, digits and
`_` are written as `~` and two hex digits, so `empty?` becomes `empty~3F`. Pages are
titled with the Ruby name: `Geometry::Circle#area`, `Geometry::Circle.unit`.

Modules, classes, methods, constants and attributes are documented, with the types
and text of their `@param` and `@return` tags. Private and protected
methods, and objects tagged `@private` or `@api private`, are left out. A method with
no doc text, an untyped `@param` or no `@return` type is listed in its
`ruby.incomplete` extension; the `ruby.config` extension records the Ruby and YARD
versions.

YARD plugins, `.yardopts` and `~/.yard` are never loaded. Any YARD warning, a syntax
error, a public API that is empty, or two objects with the same id fails the build,
and no pages are written.

Build the shard's site with the scope of the target:

```sh
bazel run @rules_dx//:dx -- docs //lib:api_docs
```

Exit codes: `0` success, `2` usage or scope errors, `1` a launch, signal or preview
failure. Bazel failures keep Bazel's code.

## Python API reference

`python_doc_shard` documents one top-level Python package of a `python_library`
without importing it. griffe reads the sources statically.

```starlark
load("@dx//tools/docs/python:defs.bzl", "python_doc_shard")

python_doc_shard(
    name = "mylib_docs",
    library = ":mylib",
    package = "mylib",   # defaults to the library name
    docstyle = "google", # or "numpy", "sphinx"
)
```

Pass the shard to `docs_site` like any other language, or let `python_docs`
declare both. It takes the `python_doc_shard` attributes plus `book_toml`,
`prose`, `prose_route` and `source_url`, creates `<name>_shard` and a
`docs_site` named `<name>`:

```starlark
python_docs(
    name = "mylib_site",
    library = ":mylib",
    book_toml = "book.toml",
    prose = ["guide.md"],
)
```

Run `bazel run @rules_dx//:dx -- docs //pkg:mylib_site` to build and preview it.
`docstyle` selects the docstring convention griffe parses. A broken package
fails the build, so `dx docs` exits with Bazel's code and writes no pages.

Symbol ids and shard output are the same as with `python_doc_shard` alone. Symbol ids have the form
`python:<package>:<module path>.<name>`.

Supported shapes are flat and `src` layouts (through `imports`), regular
packages, generated modules and a package split across libraries. All files of
`<package>` are staged into one import root, so generated and source halves of
one package load together. Namespace packages, compiled modules and `.pth`
files are not supported.

The public API is `__all__` when a module defines it, otherwise every name
without a leading underscore. A package with no source, a syntax error or a
missing package fails the build and produces no shard and no page.

## Java API reference

`java_doc_shard` from `@rules_dx//tools/docs/jvm:defs.bzl` reads one Java library's
public API with the JDK's Javadoc tool and produces a docs IR shard with language
`java`. The sources are parsed, never run or changed. Pass the shard to `docs_site`
beside other shards.

```starlark
load("@rules_dx//tools/docs/jvm:defs.bzl", "java_doc_shard")

java_doc_shard(
    name = "api_docs",
    lib = ":shapes",
)
```

Attributes:

- `lib`: the documented `java_library`. Its own `.java` `srcs` are documented; its `deps` are only on the classpath.
- `package`: the package segment of the symbol ids. Defaults to the name of `lib`.
- `deps`: `java_doc_shard` targets of libraries whose types this one uses. Their symbols are linked, not documented again.

Symbol ids have the form `java:<package>:<qualified name>`, for example
`java:shapes:dx.fixture.shapes.Circle`. Overloaded methods add the erased parameter
types, as in `java:shapes:dx.fixture.shapes.Circle.scale~28double~2Cdouble~29`.
Characters other than letters, digits and `_` are written as `~` and two hex digits.

Public and package-visible-by-`public` types, methods, fields, constructors and
enum constants are documented, with the text of `@param`, `@return`, `@throws`,
`@deprecated`, `@see` and `@since`, and `{@link}` references. Protected and private
members are left out. Only `.java` sources are read; source jars and annotation
processors are not supported. The Javadoc tool needs a full JDK as the tool runtime.

A type error in a signature, a missing source or an empty public API fails the
build, and no pages are written.

Build the shard's site with the scope of the target:

```sh
bazel run @rules_dx//:dx -- docs //lib:api_docs
```

Exit codes: `0` success, `2` usage or scope errors, `1` a launch, signal or preview
failure. Bazel failures keep Bazel's code.

## Kotlin API reference

`kotlin_doc_shard` from `@rules_dx//tools/docs/jvm:defs.bzl` reads one Kotlin library's
public API with Dokka and produces a docs IR shard with language `kotlin`. The sources
are analysed, never run or changed. Pass the shard to `docs_site` beside other shards.

```starlark
load("@rules_dx//tools/docs/jvm:defs.bzl", "kotlin_doc_shard")

kotlin_doc_shard(
    name = "api_docs",
    lib = ":shapes",
)
```

Attributes:

- `lib`: the documented `kotlin_library`. Its own `.kt` `srcs` are documented; its `deps` are only on the classpath.
- `package`: the package segment of the symbol ids. Defaults to the name of `lib`.
- `deps`: `kotlin_doc_shard` targets of libraries whose types this one uses. Their symbols are linked, not documented again.

Symbol ids have the form `kotlin:<package>:<qualified name>`, for example
`kotlin:shapes:dx.fixture.shapes.Circle`. Overloaded functions add the erased simple
parameter types, with the receiver first for an extension function, as in
`kotlin:shapes:dx.fixture.shapes.grow~28Circle~2CDouble~29`. An extension property
adds its receiver type after `@`, as in `kotlin:shapes:dx.fixture.shapes.diameter~40Circle`.
Characters other than letters, digits and `_` are written as `~` and two hex digits.

Public classes, interfaces, objects, companions, enum entries, functions, properties,
constructors and type aliases are documented, with the text of KDoc tags and `[links]`.
`internal`, `protected` and `private` declarations are left out. Only `.kt` sources
are read; Kotlin libraries with `.java` sources, source jars and compiler plugins are
not supported.

An unresolved type in a public signature, a missing source or an empty public API fails
the build, and no pages are written.

## PowerShell API reference

`powershell_doc_shard` from `@rules_dx//tools/docs/powershell:defs.bzl` reads one
PowerShell script module and produces a docs IR shard with language `powershell`.
The parser inside the pinned `pwsh` runtime reads the sources. The rule never
imports, dot-sources or runs them, so code at the top of a `.psm1` never runs.
Pass the shard to `docs_site` beside other shards, and the shared pipeline renders
the pages. Symbol ids have the form `powershell:<package>:<Name>`, with the
function name spelled as in its definition.

```starlark
load("@rules_dx//tools/docs/powershell:defs.bzl", "powershell_doc_shard")

powershell_doc_shard(
    name = "api_docs",
    srcs = glob(["Ledger/**"]),
)
```

Attributes:

- `srcs`: the module's `.psm1`, every `.ps1` file it dot-sources, and its `.psd1`.
  `pwsh_library` targets work too. Required.
- `manifest`: the module manifest. Defaults to the one `.psd1` file in `srcs`.
- `package`: the package segment of the ids. Defaults to the manifest file name
  without `.psd1`. Required when there is no manifest.
- `config`: extra text added to the `powershell.config` value of every symbol.

Build the shard's site with the scope of the target:

```sh
bazel run @rules_dx//:dx -- docs //lib:api_docs
```

Supported shapes:

- Script modules with or without a `.psd1` manifest.
- Functions in dot-sourced `.ps1` files, when those files are in `srcs`.
- `FunctionsToExport` and `AliasesToExport` lists, with `*` and `?` wildcards,
  matched without regard to case.
- `Export-ModuleMember -Function` with literal names.
- `function script:Name` and `function local:Name`, documented as `Name`.
  Functions defined as `global:` or `private:` are never exported.
- `[CmdletBinding()]`, parameter sets, `[Parameter()]`, `[Alias()]` and `[OutputType()]`.
- Comment-based help: `.SYNOPSIS`, `.DESCRIPTION`, `.PARAMETER`, `.EXAMPLE` and
  `.OUTPUTS`, before, inside or at the end of the function.

Each function page shows one syntax line per parameter set, the synopsis and
description, and a link to the definition. A function without help, or with
parameters that have no `.PARAMETER` text, gets a `powershell.incomplete` value
that names what is missing.

Not supported yet: binary modules, classes and enums, aliases from `Set-Alias` or
`New-Alias`, external MAML help and `about_` topics.

The build fails, and writes no shard and no page, when:

- a source has a syntax error. The message has the form
  `docs_powershell: <file>:<line>: <message>`.
- `Export-ModuleMember -Function` takes a variable or an expression.
- an export list names a function that no source defines.
- the manifest's `RootModule` is a `.dll`.
- the module exports no functions.
- two exported functions differ only by case.
- a manifest key the rule reads is not a constant value.

Exit codes are the same as for TypeScript: Bazel failures keep Bazel's code.

## C# API reference

`csharp_doc_shard` from `@rules_dx//tools/docs/dotnet:defs.bzl` reads one C# library's
public API with docfx's Roslyn reader and produces a docs IR shard with language
`csharp`. The sources are parsed, never run or changed. Pass the shard to `docs_site`
beside other shards.

```starlark
load("@rules_dx//csharp/rules:defs.bzl", "csharp_library")
load("@rules_dx//tools/docs/dotnet:defs.bzl", "csharp_doc_shard")

csharp_library(
    name = "ledger",
    srcs = ["Account.cs"],
)

csharp_doc_shard(
    name = "ledger_docs",
    library = ":ledger",
    package = "Ledger",
)
```

Attributes:

- `library`: the documented `csharp_library` from `@rules_dx//csharp/rules:defs.bzl`. Its own `.cs` `srcs` are documented; its `deps` are only references.
- `package`: the package segment of the symbol ids. Defaults to the assembly name of `library`.

Symbol ids have the form `csharp:<package>:<Namespace.Type.Member>`, for example
`csharp:Ledger:Ledger.Account.Deposit~~1a2b3c4d`. Overloads add `~~` and eight hex
digits taken from the documentation comment id. Characters other than letters, digits
and `_` are written as `~` and two hex digits, so `Larger<T>` becomes `Larger~3CT~3E`.

Public and protected namespaces, types, methods, constructors, properties, indexers,
events, fields, constants, enum members and delegates are documented, with the
`summary`, `remarks`, `param`, `returns`, `example` and `inheritdoc` text and the
`cref` links. Internal and private members are left out.

The compile settings come from `library`: `defines` select the members behind `#if`,
`allow_unsafe_blocks` is passed on, and `deps` become reference assemblies. Nullable
annotations are read only from `#nullable enable` in the sources; a library that sets
`nullable = "enable"` fails the build. One target framework is read: the library's
first.

The build fails, and writes no shard and no page, when:

- a source has a compile error or docfx reports a warning.
- the library has no public API.
- the member list docfx read differs from the compiled reference assembly.
- two documented symbols differ only by case.

The F# group, other target frameworks and Visual Basic are not supported yet.

Exit codes are the same as for Java: Bazel failures keep Bazel's code.
