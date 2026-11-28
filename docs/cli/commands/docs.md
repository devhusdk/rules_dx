# `dx docs`

Use `dx docs` to build your documentation site or the API reference of your libraries, and to preview it in a browser.

`dx docs` only builds. It never changes your source files.

## Examples

Build every docs site, doc shard and API target in the repository:

```sh
tools/dx docs
```

Build one site, or everything under the current directory:

```sh
tools/dx docs //lib:site
tools/dx docs --here
```

Build and preview a site in your browser:

```sh
tools/dx docs --serve --open
tools/dx docs --serve --port 8080
tools/dx docs --serve --host 0.0.0.0 --open //tools/docs/site:user_site
```

Stream JSON events, or pass options to Bazel after `--`:

```sh
tools/dx docs --output=json
tools/dx docs -- --config=ci
```

## Usage

```text
dx docs [--serve [--port <n>] [--host <addr>] [--open]] [--here] [scope ...] [-- bazel-options ...]
```

## Flags

| Flag | What it does |
| --- | --- |
| `--serve` | Preview the built site locally. The scope must build exactly one site with an `index.html`. With no site in scope, it previews the API index. |
| `--port <n>` | Preview port, `1`-`65535`. Default `8000`. Needs `--serve`. |
| `--host <addr>` | Preview bind address. Default `127.0.0.1`. Needs `--serve`. |
| `--open` | Open the preview in your browser. Needs `--serve`. |
| `--here` | Build the current directory tree. Cannot be combined with explicit scopes. |
| `-o, --output text\|json` | Choose the output format. Default `text`. |
| `--workspace <dir>` | Run in this workspace. |
| `-q, --quiet` | Hide summaries. |
| `-v, --verbose` | Show more logs. |
| `--color auto\|always\|never` | Set color output. |
| `--build-slots auto\|<n>\|off` | Set build slots. |
| `--build-slots-wait <duration>` | Give up waiting for a build slot after this long. `0` fails at once. |
| `--bazel-startup-option <token>` | Pass one Bazel startup option. Repeatable. |

Arguments after `--` go to Bazel unchanged.

## Scopes

A scope says what to build. It is one of:

- a Bazel label, such as `//lib:site`. A label names one target, the thing Bazel builds: the package directory `lib`, then the target name `site`.
- a pattern, such as `//lib/...` for every target under `lib/`.
- a workspace-relative file or directory.

With no scope, `dx docs` builds every docs target in the repository (`//...`). `--here` builds the current directory tree instead (`//<dir>/...`, or `//...` at the root).

A scope with no docs targets builds nothing and exits `0`.

When the scope has no docs site but has libraries with doc shards, `dx docs` renders their API into `.dx/local/docs/index` and prints `Docs index: <path>/index.html`.

Rustdoc lints from a crate's `lint_config` apply to its docs.

## Output

Text prints `Running docs build for <scope> (mdBook render)`, then Bazel's output. With `--serve` it then prints `Serving docs at <url> (<dir>)`, and `Opening <url>` with `--open`. The preview runs until you press Ctrl-C.

With `--output json`, dx streams `command_started`, two `operation` events and `command_finished`. A failure adds one `error`.

Reports: none.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Success. |
| `1` | Operational or preview failure, such as more than one site in scope, a missing `index.html`, or a port in use. |
| `2` | Usage or scope error, such as `--here` with an explicit scope, or `--port` without `--serve`. |
| Bazel's code | Bazel failed. `36` means a disk has under 1 GiB free. |
| `128`+N | Signal N stopped the preview, such as `130` for Ctrl-C. |

## Doc shard rules

A doc shard is the API reference of one library. A `BUILD` file is the file in each package directory that declares its targets. Declare your site there.

For Rust, list the libraries straight in `rust_docs_site` from `@rules_dx//rust:defs.bzl`:

```starlark
load("@rules_dx//rust:defs.bzl", "rust_docs_site")

rust_docs_site(
    name = "site",
    book_toml = "book.toml",
    prose = ["guide.md"],
    libraries = [":mylib"],
    source_url = "https://github.com/me/mylib/blob/main/",
)
```

For other languages, declare a shard with the rule from the table, list it in `libraries` of `docs_site` from `@rules_dx//core:defs.bzl`, then build the site:

```sh
tools/dx docs //lib:site
```

| Language | Load | Rule | Attributes |
| --- | --- | --- | --- |
| Rust | `@rules_dx//rust:defs.bzl` | `rust_docs_site` | `libraries` |
| TypeScript | `@rules_dx//typescript:defs.bzl` | `typescript_doc_shard` | `srcs`, `entry_points`, `package`, `tsconfig`, `options` |
| JavaScript | `@rules_dx//javascript:defs.bzl` | `javascript_doc_shard` | `srcs`, `entry_points`, `package`, `tsconfig` (sets `allowJs`), `options` |
| Vue | `@rules_dx//vue:defs.bzl` | `vue_doc_shard` | `srcs`, `entry_points`, `node_modules`, `package`, `tsconfig` |
| Svelte | `@rules_dx//svelte:defs.bzl` | `svelte_doc_shard` | `srcs`, `entry_points`, `node_modules`, `package`, `tsconfig` |
| Astro | `@rules_dx//astro:defs.bzl` | `astro_doc_shard` | `srcs`, `entry_points`, `node_modules`, `package`, `tsconfig` |
| MDX | `@rules_dx//mdx:defs.bzl` | `mdx_doc_shard` | `srcs`, `entry_points`, `package`, `tsconfig` |
| Go | `@rules_dx//go:defs.bzl` | `go_doc_shard` | `library`, `package`, `goos`, `goarch`, `build_tags` |
| Ruby | `@rules_dx//ruby:defs.bzl` | `ruby_doc_shard` | `lib`, `package`, `deps` |
| OCaml | `@rules_dx//ocaml:defs.bzl` | `ocaml_doc_shard` | `lib`, `package`, `config` |
| Perl | `@rules_dx//perl:defs.bzl` | `perl_doc_shard` | `lib`, `package` |
| Julia | `@rules_dx//julia:defs.bzl` | `julia_doc_shard` | `lib`, `module`, `package` |
| Fortran | `@rules_dx//fortran:defs.bzl` | `fortran_doc_shard` | `lib`, `package` |
| PHP | `@rules_dx//core:defs.bzl` | `php_doc_shard` | `lib`, `package` |
| Lua | `@rules_dx//core:defs.bzl` | `lua_doc_shard` | `lib`, `package` |
| Elixir | `@rules_dx//elixir:defs.bzl` | `elixir_doc_shard` | `app`, `package` |
| Erlang | `@rules_dx//erlang:defs.bzl` | `erlang_doc_shard` | `app`, `package` |
| Python | `@rules_dx//python:defs.bzl` | `python_doc_shard` | `library`, `package`, `docstyle` (`google`, `numpy` or `sphinx`) |
| C and C++ | `@rules_dx//cc:defs.bzl` | `cc_doc_shard` | `lib`, `package` |
| CUDA | `@rules_dx//cuda:defs.bzl` | `cuda_doc_shard` | `lib`, `package` |
| Java | `@rules_dx//java:defs.bzl` | `java_doc_shard` | `lib`, `package`, `deps` |
| Kotlin | `@rules_dx//kotlin:defs.bzl` | `kotlin_doc_shard` | `lib`, `package`, `deps` |
| Scala | `@rules_dx//scala:defs.bzl` | `scala_doc_shard` | `lib`, `package`, `deps` |
| D | `@rules_dx//d:defs.bzl` | `d_doc_shard` | `lib`, `package`, `config` |
| Haskell | `@rules_dx//haskell:defs.bzl` | `haskell_doc_shard` | `lib`, `package`, `config` |
| Swift | `@rules_dx//swift:defs.bzl` | `swift_doc_shard` | `lib`, `package`, `config` |
| Zig | `@rules_dx//zig:defs.bzl` | `zig_doc_shard` | `lib`, `package`, `config` |
| PowerShell | `@rules_dx//powershell:defs.bzl` | `powershell_doc_shard` | `srcs`, `manifest`, `package`, `config` |
| Dart | `@rules_dx//dart:defs.bzl` | `dart_doc_shard` | `lib`, `package` |
| C# | `@rules_dx//csharp:defs.bzl` | `csharp_doc_shard` | `library`, `package` |
| F# | `@rules_dx//fsharp:defs.bzl` | `fsharp_doc_shard` | `library`, `package` |
| Protobuf | `@rules_dx//api_spec:proto.bzl` | `api_spec_protobuf_doc_shard` | `proto`, `package` |
| Avro | `@rules_dx//avro:defs.bzl` | `avro_doc_shard` | `avro`, `package` |

### Shards you get without a rule

These libraries in scope get a shard automatically when they have none:

- Rust, Go, Python, C, C++, CUDA, Fortran, Java, Kotlin, Scala, D, Haskell, Swift, Zig, Dart, Perl and PHP libraries.
- C# libraries with `nullable = "disable"`, and F# libraries with `generate_documentation_file = True`. Others are skipped.
- A `javascript_library` of `.js`, `.mjs`, `.cjs` or `.jsx` files, when exactly one target in its direct `deps` gives a `tsconfig.json` with `allowJs` from the same package.
- A `vue_library`, `svelte_library`, `astro_library` or `mdx_library`, when exactly one target in its direct `deps` gives a `tsconfig.json` from the same package.
- An `elixir_app` or `erlang_app` that lists its sources in `srcs`.
- A `proto_library`.
- An `avro_library`.

### Language notes

- Elixir and Erlang: only `@doc` and `%% @doc` text is read. Elixir modules marked `@moduledoc false`, functions marked `@doc false`, and Erlang functions that are not exported are skipped. Windows execution platforms get no Elixir or Erlang API docs.
- C, C++ and CUDA: shards document the public `hdrs` of a `cc_library` or `cuda_library` with the pinned Doxygen. Linux arm64 and Windows arm64 hosts get no C, C++ or CUDA API docs.
- Protobuf: a message becomes a class, a field a field, an enum and its values an enum and its variants, a service an interface, and an rpc a method. The comment above an item, or after it on the same line, is its doc text. A `map<K, V>` field shows as a map. Streaming rpcs say `stream`.
- Avro: the `doc` attribute of the schemas is the doc text. A record becomes a class, a field a field, an enum an enum and its symbols variants, a fixed type a type alias, and a protocol message a method. Ids start with the schema namespace. `.avdl` files work too. Enum symbols carry no doc, so they are marked incomplete.

A source error fails the build and writes no pages. dx exits with Bazel's code.
