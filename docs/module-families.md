# Module families

This page is the plan for splitting rules_dx into a core module and per-family modules (issue #3748). Nothing here changes behavior yet. It records the target layout so each later step can ship alone.

## Layout

- Core stays at the repository root.
- Each family becomes a subdirectory module at `families/<family>/MODULE.bazel` with its own Bazel Central Registry entry.
- All modules release from one tag at one version. `modules/` already holds version tables, so families do not use that name.
- Quality aspect labels do not move. `@rules_dx//quality:real_aspects.bzl%real_*_aspect` stays the public surface. That file becomes an assembly file that loads each installed family's `FAMILY` struct.

## Families

Each family owns the directories, `bazel_dep`s, quality tools, and Gazelle languages listed here.

| Family | Directories | Owned `bazel_dep`s |
| --- | --- | --- |
| python | `python/` | rules_python, aspect_rules_py |
| rust | `rust/` | rules_rust, rules_rust_prost, rules_rust_wasm_bindgen |
| jvm | `java/`, `kotlin/`, `scala/` | rules_java, rules_kotlin, rules_scala, rules_jvm_external |
| dotnet | `csharp/`, `fsharp/` | rules_dotnet |
| js | `javascript/`, `typescript/`, `astro/`, `svelte/`, `vue/`, `mdx/` | aspect_rules_js, aspect_rules_ts, aspect_rules_jest, rules_nodejs |
| cc | `cc/`, `qt/`, `cuda/` | rules_cc, llvm, rules_cuda |
| go | `go/` | rules_go, gazelle |
| ruby | `ruby/` | rules_ruby |
| powershell | `powershell/` | rules_powershell |
| beam | `elixir/`, `erlang/` | none yet |
| zig | `zig/` | rules_zig |
| mobile | `android/`, `apple/` | rules_android, rules_apple, apple_support |
| haskell, ocaml, lua, nix, pkl, terraform, cue, jsonnet, html_template | one directory each | lua (for lua) |

## What core keeps

- The CLI, `dx` rules, the quality pipeline, the probe, `libs/`, `config/` and `tools/`.
- Shell, YAML, Dockerfile, Markdown, TOML, Vale and keep-sorted quality tools.
- `rules_proto` and `protobuf`, because `generation/` needs them.
- Python-built tools such as `yamllint` and `djlint` (from `//quality/tools/python`). A follow-up decides whether they move to the python family or ship prebuilt.

## Order of work

1. Split `quality/real_aspects.bzl` into `quality/real_core.bzl` and `quality/families/<family>.bzl`, with identical Bazel actions.
2. Build Gazelle languages from the family list.
3. Add `dx init` family manifests.
4. Add a label migration table and a docs test for the public label surface.
5. Cut the first real module (android or apple), with a `dx_quality` extension.
6. Move the remaining families, one per pull request.
7. Release all modules together, then add a `dx migrate` manifest and the major release.

## Constraints for every step

- Public shard names and the probe output format (`{"shards": [...]}`) do not change.
- A family builds its own `attr.label` and toolchain type objects in its own file, so labels resolve against the family repository.
- Hooks hold no mutable captured state.
- Every aspect is bound to a top-level global, or Bazel reports it as not exported.
