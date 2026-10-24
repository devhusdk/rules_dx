# Parallel worktree load test

Manual. Never runs in CI.

```sh
bazel run //tools/testing/load:parallel_worktrees -- --out run1.json
bazel run //tools/testing/load:parallel_worktrees -- --out run2.json
bazel run //tools/testing/load:parallel_worktrees -- --compare run1.json run2.json
```

Flags:

- `--worktrees N`: worktrees to build at once. Default 4.
- `--target LABEL`: what each worktree tests. Default `//...`.
- `--out FILE`: JSON result. Default `parallel_worktrees.json`.
- `--isolated-repo-cache`: give each worktree its own empty repository cache. Cold runs then fetch every external repository again.
- `--compare A B`: print the percent difference per metric. Exit 1 if any is over 15%. Exit 2 if the runs used different `--worktrees` or `--target`, or have no metrics. Relative paths are relative to where you run it.

It runs `bazel test` in all worktrees at once, cold, then warm. Remote cache, disk cache and user.bazelrc are not used. Worktrees share your repository cache unless you pass `--isolated-repo-cache`. It prints a table and writes the JSON:
memory pressure peaks (`some` and `full`, avg10), Bazel server count, peak server RSS (total and largest),
JVM peak post-GC heap (from the build event log, or `bazel info used-heap-size-after-gc` when the log has none), wall time, output base size, and OOM kills.

Worktrees go under `.claude/worktrees/parallel-load-*`, built from `HEAD`. They, their Bazel servers
and their output bases are removed when the run ends, including on failure or Ctrl-C. Leftovers from dead runs are removed on the next run. Linux only.
