# `dx docs`

Builds the documentation sites, doc shards and API references in scope.

```sh
tools/dx docs
tools/dx docs //lib:site
tools/dx docs --here
tools/dx docs --serve --port 8080
tools/dx docs --serve --host 0.0.0.0 --open //tools/docs/site:user_site
tools/dx docs --output=json
tools/dx docs -- --config=ci
```

```text
dx docs [--serve [--port <n>] [--host <addr>] [--open]] [--here] [scope...] [-- bazel-options...]
```

| Flag | What it does |
| --- | --- |
| `--serve` | Preview the built site locally. The scope must build one site tree with an `index.html`. With no site in scope, it previews the API index. |
| `--port <n>` | Preview port, `1`-`65535`. Default `8000`. Needs `--serve`. |
| `--host <addr>` | Preview bind address. Default `127.0.0.1`. Needs `--serve`. |
| `--open` | Open the preview URL in a browser. Needs `--serve`. |
| `--here` | Build the current directory tree. Cannot be combined with explicit scopes. |
| `-o`, `--output text\|json` | Output format. |
| `--workspace <dir>` | Run in another workspace. |
| `-q`, `--quiet` | Hide summaries. |
| `-v`, `--verbose` | Show more logs. |
| `--color auto\|always\|never` | Color output. |
| `--build-slots auto\|<n>\|off` | Build slots for this run. |
| `--build-slots-wait <duration>` | Give up waiting for a build slot after this long. `0` fails at once. |
| `--bazel-startup-option <token>` | Bazel startup option. Repeatable. |

Args after `--` go to Bazel unchanged.

Scopes: Bazel labels or patterns (`//...`, `//pkg:target`), or
workspace-relative files or dirs. No scope means `//...`. A scope with no docs
targets builds nothing and exits `0`. When the scope has no docs site but has
Rust libraries, `dx docs` renders their API into `.dx/local/docs/index` and prints
the path of its `index.html`.
Rustdoc lints from a crate's `lint_config` apply to its docs.

Output: text prints `Running docs build for <scope> (mdBook render)`. With
`--serve` it then prints `Serving docs at <url> (<dir>)`, and
`Opening <url>` with `--open`. JSON streams `command_started`, `operation`
events and `command_finished`. A failure adds one `error`. `--diff` and
`--report` are usage errors.

| Exit code | Meaning |
| --- | --- |
| `0` | Success. |
| `1` | Launch, signal or preview failure, such as no single site tree, a missing `index.html` or a port in use. |
| `2` | Usage or scope error, such as `--here` with an explicit scope. |
| Bazel's code | Bazel failed. `36` means a disk has under 1 GiB free. |
| `128`+N | Signal N stopped the preview, such as `130` for Ctrl-C. |

## Doc shard rules

Rust libraries go straight into `docs_site` from
`@rules_dx//tools/docs/site:site.bzl`. The other languages have a rule that
documents one library and produces a shard; list the shard in `libraries` too.
Then build the site:

```starlark
load("@rules_dx//tools/docs/site:site.bzl", "docs_site")

docs_site(
    name = "site",
    book_toml = "book.toml",
    prose = ["guide.md"],
    libraries = [":mylib"],
    source_url = "https://github.com/me/mylib/blob/main/",
)
```

```sh
tools/dx docs //lib:site
```

| Language | Load | Rule | Attributes |
| --- | --- | --- | --- |
| Rust | `@rules_dx//tools/docs/site:site.bzl` | `docs_site` | `libraries` |
| TypeScript | `@rules_dx//tools/docs/typescript:defs.bzl` | `typescript_doc_shard` | `srcs`, `entry_points`, `package`, `tsconfig`, `options` |
| Vue | `@rules_dx//tools/docs/components:defs.bzl` | `vue_doc_shard` | `srcs`, `entry_points`, `node_modules`, `package`, `tsconfig` |
| Svelte | `@rules_dx//tools/docs/components:defs.bzl` | `svelte_doc_shard` | `srcs`, `entry_points`, `node_modules`, `package`, `tsconfig` |
| Astro | `@rules_dx//tools/docs/components:defs.bzl` | `astro_doc_shard` | `srcs`, `entry_points`, `node_modules`, `package`, `tsconfig` |
| MDX | `@rules_dx//tools/docs/components:defs.bzl` | `mdx_doc_shard` | `srcs`, `entry_points`, `package`, `tsconfig` |
| Go | `@rules_dx//tools/docs/go:defs.bzl` | `go_doc_shard` | `library`, `package`, `goos`, `goarch`, `build_tags` |
| Ruby | `@rules_dx//tools/docs/ruby:defs.bzl` | `ruby_doc_shard` | `lib`, `package`, `deps` |
| Julia | `@rules_dx//tools/docs/julia:defs.bzl` | `julia_doc_shard` | `lib`, `module`, `package` |
| Python | `@rules_dx//tools/docs/python:defs.bzl` | `python_doc_shard` | `library`, `package`, `docstyle` (`google`, `numpy` or `sphinx`) |
| Java | `@rules_dx//tools/docs/jvm:defs.bzl` | `java_doc_shard` | `lib`, `package`, `deps` |
| Kotlin | `@rules_dx//tools/docs/jvm:defs.bzl` | `kotlin_doc_shard` | `lib`, `package`, `deps` |
| Scala | `@rules_dx//tools/docs/jvm:defs.bzl` | `scala_doc_shard` | `lib`, `package`, `deps` |
| PowerShell | `@rules_dx//tools/docs/powershell:defs.bzl` | `powershell_doc_shard` | `srcs`, `manifest`, `package`, `config` |
| Dart | `@rules_dx//tools/docs/dart:defs.bzl` | `dart_doc_shard` | `lib`, `package` |
| C# | `@rules_dx//tools/docs/dotnet:defs.bzl` | `csharp_doc_shard` | `library`, `package` |
| F# | `@rules_dx//tools/docs/dotnet:defs.bzl` | `fsharp_doc_shard` | `library`, `package` |

Rust, Go, Python, Java, Kotlin, Scala, Dart, C# and F# libraries in scope that have
no shard get one automatically. A C# library needs `nullable = "disable"` and
an F# library needs `generate_documentation_file = True`; others are skipped.

A source error fails the build and writes no pages. Bazel's exit code is kept.
