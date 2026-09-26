# Repository Instructions

This repository uses Bazel.

- Treat warnings as errors.
- Only user docs. Allowed: `docs/cli/commands/`, `docs/github-ci.md`, short READMEs, `examples/`. Delete anything else. Never write design rationale, history, alternatives, or implementation notes.
- User docs stay complete: every command page documents its flags, scopes, output, reports, exit codes, and examples. Match `dx <command> --help`. Keep them user-facing; still no rationale, history, or issue numbers.
- Comments: delete `#` inline comments by default. Keep Starlark docstrings: one short plain line per module, function, provider, rule, and attr. No links, no issue numbers, no `See:`/`Contract:`/`policy:`, no ADR, no backtick chains, no restating the code. When in doubt, delete inline comments and shorten docstrings. Assigned strings like `_HUB_BUILD = \"\"\"...\"\"\"` are code, not comments. Keep. One line like `# Windows needs the runfiles tree.` is the most ever allowed for `#` comments, and often nothing is better.
- Two exceptions only: `--help` text stays one short line per flag, and `LCOV_EXCL_*` markers keep their required short `reason:` plus `issue:`.
- Write plain and human. Short sentences. Commands first. No filler, no essays, no AI tone. If a paragraph explains why something exists, delete it.
