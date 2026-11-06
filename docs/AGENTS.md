# Documentation Instructions

- Docs are for people who use or release rules_dx: command pages, guides, tutorials, the generated `docs/support-matrix.md`, READMEs, `examples/`, and runbooks like `RELEASING.md`.
- One page per command or task. Commands first. Short sentences.
- User docs stay complete: every command page documents its flags, scopes, output, reports, exit codes, and examples. Match `dx <command> --help`.
- Delete rationale, history, alternatives, status notes, qualification notes, and issue numbers. If a user does not type it or decide it, delete it. The exception is the "What rules_dx replaces" section in `README.md`.
- Never add `See:`/`Contract:` chains or links to deleted pages. Fix old links by deleting the sentence, not by repointing it.
- `docs/design/` is the exception: it holds design records that fix decisions for maintainers. Write rules, not history, one section per decision, and keep each page under 1300 words. These pages stay out of the user book.
