# `dx bench`

Runs the tests tagged `dx-bench` and prints the numbers they report.

```sh
dx bench //...
dx bench //app:parse_bench --output=json
dx bench --compare main --max-regression=20
```

```text
dx bench [--compare <path|ref>] [--max-regression <pct>] [--here] [scope ...] [-- bazel-options ...]
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

Every `<lang>_bench` macro (`go_bench`, `rust_bench`, `java_bench`, `kotlin_bench`, `python_bench`, `csharp_bench`, `fsharp_bench`, `javascript_bench`, `typescript_bench`, `cc_bench`, `zig_bench`) declares `<name>` as the manual `dx-bench` test dx runs. Test attributes such as `size` (default `small`), `timeout`, `env`, `args`, `data`, `flaky` and `shard_count` go to `<name>`, the other attributes go to the private `<name>_bin`, and `tags` are merged with `dx-bench` and `manual`.

Go: `go_bench(name, srcs, embed = ...)` from `//go/rules:defs.bzl` runs the `Benchmark*` functions with `-test.bench=. -test.benchmem` and turns each `ns/op`, `B/op` and `allocs/op` column into one result named `<Benchmark> <unit>`. The Go gazelle extension adds `<name>_bench` next to the `go_test` when a test file declares a `Benchmark*` function.

Each test writes `$TEST_UNDECLARED_OUTPUTS_DIR/dx-bench.json`:

```json
{"schema":"dx-bench/v1","results":[{"name":"parse","unit":"ms","mean":1.2,"stddev":0.1,"iterations":50}]}
```

Text output prints one table per target with name, mean, stddev and unit. `--output=json` prints the merged document, one entry per target. Both modes write it to `.dx/local/bench/last.json`.

`--compare <path|ref>` reads a `dx-bench/v1` document, either one test's `dx-bench.json` or a merged `last.json`. A value that is not a file is a git ref: `dx bench` downloads the `dx-bench-<platform>` artifact of the latest run on that ref with `gh run download` and reads the json file in it. The table gains `baseline` and `delta %` columns, and `--output=json` adds `baseline` and `delta_pct` to each result the baseline covers.

`--max-regression <pct>` needs `--compare` and exits 1 when a result is slower than its baseline by more than `<pct>` percent and by more than two standard deviations (the larger of the two stddevs). Each regression is printed to stderr as `bench_regression`.

A target that passes but wrote no `dx-bench.json`, or wrote an invalid one, fails the run. With no `dx-bench` test in scope, `dx bench` exits 1 with `no_tests`.

Scopes: same as `dx test`. With no scope, it runs `//...`.

Rust: `rust_bench` from `//rust/rules:defs.bzl` runs a criterion or `#[bench]` target with `--output-format bencher` and writes `dx-bench.json` in ns/iter. Gazelle turns `[[bench]]` entries with `harness = false` into `rust_bench` targets.

JavaScript and TypeScript: `javascript_bench(name, srcs, node_modules)` from `//javascript/rules:defs.bzl` and `typescript_bench` from `//typescript/rules:defs.bzl` run a [tinybench](https://github.com/tinylibs/tinybench) `Bench` that the first source (or `main`) exports as its default export. Each task becomes one row, in milliseconds. List `tinybench` in `package.json` so `node_modules` links it. `typescript_bench` also takes the `typescript_test` attributes such as `deps` and `tsconfig`.
