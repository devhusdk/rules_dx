# Parallel worktree load test

Manual. Never runs in CI.

```
bazel run //tools/testing/load:parallel_worktrees -- --out run1.json
bazel run //tools/testing/load:parallel_worktrees -- --out run2.json
bazel run //tools/testing/load:parallel_worktrees -- --compare run1.json run2.json
```

Flags:

- `--worktrees N`: worktrees to build at once. Default 4.
- `--target LABEL`: what each worktree tests. Default `//...`.
- `--out FILE`: JSON result. Default `parallel_worktrees.json`.
- `--compare A B`: print the percent difference per metric. Exit 1 if any is over 15%.

It runs `bazel test` in all worktrees at once, cold, then warm. It prints a table and writes the JSON:
memory pressure peaks (`some` and `full`, avg10), Bazel server count, peak server RSS (total and largest),
JVM heap (peak, or committed when Bazel reports none), wall time, output base size, and OOM kills.

Worktrees go under `.claude/worktrees/parallel-load-*`, built from `HEAD`. They, their Bazel servers
and their output bases are removed when the run ends, including on failure. Linux only.
