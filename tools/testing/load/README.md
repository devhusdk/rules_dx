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
- `--compare A B`: print the percent difference per metric. Exit 1 if any is over 15%. Exit 2 if the runs used different `--worktrees`, `--target`, rc files or disk cache, lack the `user_rc` or `shared_disk_cache` fields, or have no metrics. Relative paths are relative to where you run it.

Every worktree's Bazel server starts before the cold phase and stays up under the repository's own idle limit while that worktree waits for a slot.
It runs `dx test` in all worktrees at once, cold, then warm. Each run takes a build slot; worktrees beyond the slot count wait. Run it with `bazel run`. Worktrees share your repository cache unless you pass `--isolated-repo-cache`.
It prints the setup line `user rc: read|skipped, disk cache: <path>|off`, then a table, and writes the JSON.
The JSON header has `worktrees`, `target`, `user_rc` (rc files read), `shared_disk_cache` (a disk cache was on) and `disk_cache` (its path, empty when off).
Per phase it records:
memory pressure peaks (`some` and `full`, avg10), Bazel server count, peak server RSS (total and largest),
JVM heap after the last GC of each phase (`jvm_heap_after_gc_bytes`, from `bazel info used-heap-size-after-gc`; worktrees whose server already exited are skipped), wall time, output base size, OOM kills, and how many worktrees waited for a slot (`slot_waits`, not compared).

When it ends, including on failure, it removes its worktrees, branches and Bazel servers. Linux only.

Memory: it refuses to start when `MemAvailable` is below 3 GiB per worktree, and says how much it needs.
`--force` skips the check.

Signals: SIGINT or SIGTERM during a run stops it, cleans up and exits 130.

Exit codes: 0 success, 1 metrics over tolerance in `--compare`, 2 usage errors, the memory refusal and
failed runs, including a run where `bazel info` restarts a worktree's Bazel server or `dx test` restarts a server started before the cold phase, 130 interrupted.
