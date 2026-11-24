# Repository Instructions

This repository uses Bazel.

- Treat warnings as errors.
- Docs rules also live in `docs/AGENTS.md`. Read it before you edit docs.
- Repo layout rules live in `docs/design/layout.md`. New language code goes in `<lang>/`. Never add a family module. Build-rule changes for `php`, `lua` and `qt` go in the `rules_php`, `rules_lua` and `rules_qt` rule-set modules.
- Public API only in `<lang>/defs.bzl` and `<lang>/extensions.bzl`. Implementation goes in `<lang>/private/`. Tests go in `<lang>/tests/`. No label points into `private/`.
- Language facts live in `<lang>/lang.json` and `<lang>/pins.json`. Code reads them. Never repeat their values.
- `core/` never loads a language `.bzl` file. One exception: `core/quality/tools.bzl` loads the `_tools.bzl` files.
- Root holds only language directories and the shared directories in `docs/design/layout.md`.
- Docs are for people who use or release rules_dx: command pages, guides, tutorials, the generated `docs/support-matrix.md`, READMEs, `examples/`, and runbooks like `RELEASING.md`. Never write design rationale, history, alternatives, or implementation notes. One exception: the "What rules_dx replaces" section in `README.md` may state what you get and what it costs. Keep it to those two lists.
- User docs stay complete: every command page documents its flags, scopes, output, reports, exit codes, and examples. Match `dx <command> --help`. Keep them user-facing; still no rationale, history, or issue numbers.
- No comments in any language, Rust `///`/`//!` and Go doc comments included. Use names and tests instead. Assigned strings like `_HUB_BUILD = """..."""` are code, not comments.
- Kept: tool directives, shebangs, license headers, lines tools parse (`# Why:`, `# Upstream:`, `# dx:`), `LCOV_EXCL_*`, `# floor:`, `// SAFETY:`, one Starlark docstring line per item, one clap `--help` line per flag.
- `//tools/ci:comment_budget_test` caps comments per file. Never raise a budget; `tools/dx run //tools/ci:comment_budget_update` lowers it.
- Nothing is released, so write no compatibility code: no deprecated attr, in-module `alias`, re-export `.bzl` file, version conditional, or name that says so. `//tools/ci:zero_compat_test` checks it.
- Rules that choose what to build for take `platforms`, a string list; `target_compatible_with` only marks compatibility.
- Write plain and human, like you are showing a colleague. Short sentences, everyday words, commands first. In docs, one line on what a thing does and when to use it is welcome. Leave out design backstory, except the "What rules_dx replaces" section in `README.md`.
- Don't wait on upstream. Patch it here, or fetch the fixed rule set in rules_dx's own module extension, so consumers get the fix now. Then fix it upstream too: open an upstream issue first, then a small PR that follows that repo's contributing guide and style and includes a regression test. Link both from the rules_dx issue. Delete our patch once a release carries the fix.
- No `actions: write`? Comment `/evidence platforms=["macos_arm64"] task=test scope=//pkg/...` on the issue or PR. It dispatches `evidence.yml` and replies with the run URL. Wait with `gh pr checks <n> --watch`.
- Need a nested lock re-record? Comment `/lock-refresh nested=true` on the PR. It dispatches `lock-refresh.yml` on the PR branch, which commits the refreshed locks and nested cache sets there, and replies with the run URL.
- Changed `MODULE.bazel` or a `bazel_dep`? Comment `/lock-refresh` on the PR. A Linux `bazel mod deps` refreshes only the linux_amd64 go_sdk and python entries.
- Fresh container or clone: run `tools/dx --version` first. It fetches Bazelisk when `bazel` is missing and builds dx. See "Work in a cloud container" in CONTRIBUTING.md.
- A PR that touches `.github/review-paths.txt` paths gets `needs-review` and a second-session `Review:` comment before merge. See CONTRIBUTING.md.

<!-- dx:begin -->
- Start: `tools/dx worktree add <branch>`. Finish: `tools/dx worktree release`. Mutating git, bazel and dx commands in the primary checkout are denied while you hold a slot. Never build, edit or commit in the main checkout.
- Fix first: `tools/dx check --apply --affected`.
- Then verify: `tools/dx test --affected`.
- Before a PR: `tools/dx check --affected`. It is the gate.
- One file: `tools/dx test <file>`.
- One test: `tools/dx test <label> -- --test_filter=<name>`.
- Read the `--output=json` result.
- Lint config: edit the tool's own root file; for tools without extends, run `tools/dx config override <tool>`; the tier is `tier` in dx.toml; never edit `.dx/` (generated, drift-checked).
<!-- dx:end -->
