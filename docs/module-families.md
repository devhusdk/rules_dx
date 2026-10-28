# Module families

This page is the plan for splitting rules_dx into a core module and per-family modules (issue #3748). Nothing here changes behavior yet. It records the target layout so each later step can ship alone.

## Layout

- Core stays at the repository root.
- Each family becomes a subdirectory module at `families/<family>/MODULE.bazel` with its own Bazel Central Registry entry.
- All modules release from one tag at one version. `modules/` already holds version tables, so families do not use that name.
- Quality aspect labels do not move. `@rules_dx//quality:real_aspects.bzl%real_*_aspect` stays the public surface. That file becomes an assembly file that loads each installed family's `FAMILY` struct.

## Families

Each family owns the directories, `bazel_dep`s, quality tools, Gazelle languages and `dx new` languages listed here.

| Family | Directories | Owned `bazel_dep`s | `dx new` languages |
| --- | --- | --- | --- |
| python | `python/` | rules_python, aspect_rules_py | python |
| rust | `rust/` | rules_rust, rules_rust_prost, rules_rust_wasm_bindgen | rust |
| jvm | `java/`, `kotlin/`, `scala/` | rules_java, rules_kotlin, rules_scala, rules_jvm_external | java, kotlin, scala |
| dotnet | `csharp/`, `fsharp/` | rules_dotnet | csharp, fsharp |
| js | `javascript/`, `typescript/`, `astro/`, `svelte/`, `vue/`, `mdx/` | aspect_rules_js, aspect_rules_ts, aspect_rules_jest, rules_nodejs | javascript, typescript, astro, svelte, vue, mdx |
| cc | `cc/`, `qt/`, `cuda/` | rules_cc, llvm, rules_cuda | c, cpp, cuda |
| go | `go/` | rules_go, gazelle | go |
| ruby | `ruby/` | rules_ruby | ruby |
| powershell | `powershell/` | rules_powershell | powershell |
| beam | `elixir/`, `erlang/` | none yet | elixir, erlang |
| zig | `zig/` | rules_zig | zig |
| mobile | `android/`, `apple/` | rules_android, rules_apple, apple_support | none |
| haskell, ocaml, lua, nix, pkl, terraform, cue, jsonnet, html_template | one directory each | lua (for lua) | haskell, ocaml |

## What core keeps

- The CLI, `dx` rules, the quality pipeline, the probe, `libs/`, `config/` and `tools/`.
- Shell, YAML, Dockerfile, Markdown, TOML, Vale and keep-sorted quality tools.
- `rules_proto` and `protobuf`, because `generation/` needs them.
- Python-built tools such as `yamllint` and `djlint` (from `//quality/tools/python`). A follow-up decides whether they move to the python family or ship prebuilt.

## Order of work

1. Split `quality/real_aspects.bzl` into `quality/real_core.bzl` and `quality/families/<family>.bzl`, with identical Bazel actions.
2. Build Gazelle languages from the family list.
3. Add `dx new` and `dx init` family manifests.
4. Add a label migration table and a docs test for the public label surface.
5. Cut the first real module (android or apple), with a `dx_quality` extension.
6. Move the remaining families, one per pull request.
7. Release all modules together, then add a `dx migrate` manifest and the major release.

## Constraints for every step

- Public shard names and the probe output format (`{"shards": [...]}`) do not change.
- A family builds its own `attr.label` and toolchain type objects in its own file, so labels resolve against the family repository.
- Hooks hold no mutable captured state.
- Every aspect is bound to a top-level global, or Bazel reports it as not exported.
