# Repository Instructions

This repository uses Bazel.

- Treat warnings as errors.
- Docs rules also live in `docs/AGENTS.md`. Read it before you edit docs.
- Docs are for people who use or release rules_dx: command pages, guides, tutorials, the generated `docs/support-matrix.md`, READMEs, `examples/`, and runbooks like `RELEASING.md`. Never write design rationale, history, alternatives, or implementation notes. One exception: the "What rules_dx replaces" section in `README.md` may state what you get and what it costs. Keep it to those two lists.
- User docs stay complete: every command page documents its flags, scopes, output, reports, exit codes, and examples. Match `dx <command> --help`. Keep them user-facing; still no rationale, history, or issue numbers.
- Comments: delete `#` inline comments by default. Keep Starlark docstrings: one short plain line per module, function, provider, rule, and attr. No links, no issue numbers, no `See:`/`Contract:`/`policy:`, no ADR, no backtick chains, no restating the code. When in doubt, delete inline comments and shorten docstrings. Assigned strings like `_HUB_BUILD = \"\"\"...\"\"\"` are code, not comments. Keep. One line like `# Windows needs the runfiles tree.` is the most ever allowed for `#` comments, and often nothing is better.
- Rules that choose what to build for take `platforms`, a string list; `target_compatible_with` only marks compatibility.
- Three exceptions only: `--help` text stays one short line per flag, `LCOV_EXCL_*` markers keep their required short `reason:` plus `issue:`, and `# floor: <reason>` stays above each `bazel_dep`. Tools parse it.
- Write plain and human. Short sentences. Commands first. No filler, no essays, no AI tone. If a paragraph explains why something exists, delete it, except the "What rules_dx replaces" section in `README.md`.
- Don't wait on upstream. Patch it here, or fetch the fixed rule set in rules_dx's own module extension, so consumers get the fix now. Then fix it upstream too: open an upstream issue first, then a small PR that follows that repo's contributing guide and style and includes a regression test. Link both from the rules_dx issue. Delete our patch once a release carries the fix.
- No `actions: write`? Comment `/evidence platforms=["macos_arm64"] task=test scope=//pkg/...` on the issue or PR. It dispatches `evidence.yml` and replies with the run URL.
- A PR that touches `.github/review-paths.txt` paths gets `needs-review` and a second-session `Review:` comment before merge. See CONTRIBUTING.md.

<!-- dx:begin -->
- Start: `tools/dx worktree add <branch>`. Finish: `tools/dx worktree release`.
- Fix first: `tools/dx check --apply --affected`.
- Then verify: `tools/dx test --affected`.
- Before a PR: `tools/dx check --affected`. It is the gate.
- One file: `tools/dx test <file>`.
- One test: `tools/dx test <label> -- --test_filter=<name>`.
- Read the `--output=json` result.
<!-- dx:end -->
