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
- `--force`: run even when available memory is below the minimum.
- `--compare A B`: print the percent difference per metric. Exit 1 if any is over 15%. Exit 2 if the runs used different `--worktrees` or `--target`, or have no metrics. Relative paths are relative to where you run it.

It runs `dx test` in all worktrees at once, cold, then warm. Each run takes a build slot, so only as many build as there are slots; the rest wait with their Bazel servers idle. Run it with `bazel run`, not `dx run`, so the load test itself holds no slot. Remote cache, disk cache and your user.bazelrc are not used. Worktrees share your repository cache unless you pass `--isolated-repo-cache`. It prints a table and writes the JSON:
memory pressure peaks (`some` and `full`, avg10), Bazel server count, peak server RSS (total and largest),
JVM heap after the last GC of each phase (`jvm_heap_after_gc_bytes`, from `bazel info used-heap-size-after-gc`), wall time, output base size, OOM kills, and how many worktrees waited for a slot (`slot_waits`, not compared).

Each worktree is a `dx worktree` slot on a new branch `parallel-load-<pid>-<n>`, built from `HEAD`.
Its Bazel server uses a fresh output base under `<cache>/dx/parallel-load/`, so the cold phase stays cold.
When the run ends, including on failure, the driver stops its servers, removes their output bases,
releases the slots and deletes the branches. Leftovers from dead runs are removed on the next run. Linux only.

Memory: it refuses to start when `MemAvailable` is below 3 GiB per worktree, and says how much it needs.
`--force` skips the check.

Signals: SIGINT and SIGTERM clean up as above and exit 130. The driver gets SIGTERM when its parent exits.
Its dx and Bazel clients get SIGINT when the driver dies, and its servers exit after 120 seconds idle.

Exit codes: 0 success, 1 metrics over tolerance in `--compare`, 2 usage errors, the memory refusal and
failed runs, 130 interrupted.
