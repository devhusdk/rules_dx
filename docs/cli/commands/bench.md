# `dx bench`

Runs the tests tagged `dx-bench` and prints the numbers they report.

```sh
dx bench //...
dx bench //app:parse_bench --output=json
```

```text
dx bench [--here] [scope ...] [-- bazel-options ...]
```

Tag a test `dx-bench`. Add `manual` so `dx test //...` skips it:

```starlark
sh_test(
    name = "parse_bench",
    srcs = ["parse_bench.sh"],
    tags = ["dx-bench", "manual"],
)
```

`dx bench` selects the `dx-bench` tests inside the scope with `attr(tags, ...)` over `tests(<scope>)`, then runs them with `--runs_per_test=1 --nocache_test_results --local_test_jobs=1 --test_output=errors --test_env=DX_BENCH=1`. Bazel flags after `--` are forwarded, except the ones dx owns.

Each test writes `$TEST_UNDECLARED_OUTPUTS_DIR/dx-bench.json`:

```json
{"schema":"dx-bench/v1","results":[{"name":"parse","unit":"ms","mean":1.2,"stddev":0.1,"iterations":50}]}
```

Text output prints one table per target with name, mean, stddev and unit. `--output=json` prints the merged document, one entry per target. Both modes write it to `.dx/bench/last.json`.

A target that passes but wrote no `dx-bench.json`, or wrote an invalid one, fails the run. With no `dx-bench` test in scope, `dx bench` exits 1 with `no_tests`.

Scopes: same as `dx test`. With no scope, it runs `//...`.
