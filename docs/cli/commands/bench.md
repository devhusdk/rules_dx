# `dx bench`

Use `dx bench` to run your benchmarks, see their numbers, and catch a change that makes code slower.

```sh
dx bench //...
dx bench //app:parse_bench --output=json
dx bench --compare main --max-regression=20
```

`dx bench` runs the tests tagged `dx-bench` and prints the numbers each one reports. A *test* here is a Bazel test target. A *tag* is a word in the target's `tags` list.

## Usage

```text
dx bench [--compare <path|ref>] [--max-regression <pct>] [--here] [scope ...] [-- bazel-options ...]
```

## Flags

| Flag | What it does |
| --- | --- |
| `--compare <path\|ref>` | Compare with a `dx-bench/v1` file, or with the `dx-bench` artifact of the latest CI run on a git ref. |
| `--max-regression <pct>` | With `--compare`: exit `1` when a result is slower than its baseline by more than this percent. |
| `--here` | Use the current directory tree: `//path/...`, or `//...` at the workspace root. |
| `-o, --output text\|json` | Choose the output format. |
| `--workspace <dir>` | Use this workspace directory. |
| `-q, --quiet` | Hide summaries. |
| `-v, --verbose` | Show more logs. |
| `--color auto\|always\|never` | Set color output. |
| `--build-slots <auto\|n\|off>` | Set build slots: `auto`, a count, or `off`. |
| `--build-slots-wait <duration>` | Give up waiting for a build slot after this long. `0` fails at once. |
| `--bazel-startup-option <token>` | Pass one Bazel startup option. Repeatable. |

## Scopes

A scope is a label such as `//app:parse_bench`, a pattern such as `//pkg/...`, or a workspace file or directory. `//...` means every target in the workspace. With no scope, `dx bench` uses `//...`. Only tests tagged `dx-bench` inside the scope run.

`--here` cannot be combined with a scope. Scopes work as in `dx test`. Read [Scope defaults](scope-defaults.md) for details.

## How it runs

`dx bench` runs each `dx-bench` test once, alone, and without cached results. It passes Bazel `--runs_per_test=1 --nocache_test_results --local_test_jobs=1 --test_output=errors --test_env=DX_BENCH=1`.

Bazel options after `--` are passed on. Options that change how often or how many tests run (`--runs_per_test`, `--cache_test_results`, `--local_test_jobs`), and the protected flags of `dx test`, exit `2`.

`dx test //...` also runs `dx-bench` tests, as plain tests.

## Write a benchmark

Tag any test `dx-bench`:

```starlark
sh_test(
    name = "parse_bench",
    srcs = ["parse_bench.sh"],
    tags = ["dx-bench"],
)
```

Each test writes `$TEST_UNDECLARED_OUTPUTS_DIR/dx-bench.json`:

```json
{"schema":"dx-bench/v1","results":[{"name":"parse","unit":"ms","mean":1.2,"stddev":0.1,"iterations":50}]}
```

A test that passes but writes no `dx-bench.json`, or an invalid one, fails the run.

### `<lang>_bench` macros

Each `<lang>_bench` macro declares `<name>` as the `dx-bench` test and adds `dx-bench` to `tags`.

- `cc_bench`, `d_bench`, `go_bench`, `haskell_bench`, `ocaml_bench`, `rust_bench`, `swift_bench` and `zig_bench` declare a private `<name>_bin` plus a `<name>` runner.
- `java_bench`, `kotlin_bench` and `scala_bench` declare a JMH library `<name>_bin` plus a `<name>` runner.
- `csharp_bench`, `fsharp_bench`, `dart_bench`, `elixir_bench`, `erlang_bench`, `javascript_bench`, `lua_bench`, `perl_bench`, `php_bench`, `python_bench`, `ruby_bench` and `typescript_bench` declare only the `<name>` test, which takes every attribute. They set no `size` default, so Bazel's default applies.

For macros with a `<name>_bin`, test attributes such as `size` (default `small`), `timeout`, `env`, `args`, `data`, `flaky` and `shard_count` go to `<name>`. All other attributes go to `<name>_bin`.

### Per language

- **Go:** `go_bench(name, srcs, embed = ...)` from `//go:defs.bzl` runs the `Benchmark*` functions with `-test.bench=. -test.benchmem`. Each `ns/op`, `B/op` and `allocs/op` column becomes one result named `<Benchmark> <unit>`. The Go gazelle extension adds `<name>_bench` next to the `go_test` when a test file declares a `Benchmark*` function.
- **Rust:** `rust_bench` from `//rust:defs.bzl` runs a criterion or `#[bench]` target with `--output-format bencher` and reports ns/iter. Gazelle turns `[[bench]]` entries with `harness = false` into `rust_bench` targets.
- **D:** `d_bench(name, srcs)` from `//d:defs.bzl` is a `d_binary` that writes `dx-bench.json` itself, usually from `std.datetime.stopwatch.benchmark`.
- **JavaScript and TypeScript:** `javascript_bench(name, srcs, node_modules)` from `//javascript:defs.bzl` and `typescript_bench` from `//typescript:defs.bzl` run a [tinybench](https://github.com/tinylibs/tinybench) `Bench` that the first source (or `main`) exports as its default export. Each task becomes one row, in milliseconds. List `tinybench` in `package.json` so `node_modules` links it. `typescript_bench` also takes the `typescript_test` attributes such as `deps` and `tsconfig`.
- **Perl:** `perl_bench` from `//perl:defs.bzl` is a `perl_test` whose script calls `DxBench::run_benchmarks([name => sub { ... }, ...])`. It times each sub with the core `Benchmark` module and reports seconds per call.
- **Ruby:** `ruby_bench` from `//ruby:defs.bzl` takes `main`, a script (`<name>.rb`, else the single source) that calls `DxBench.run { |x| x.report("name") { ... } }` with the benchmark-ips API. rules_dx ships the pinned benchmark-ips gem, so you need no Gemfile entry. Results are in seconds per call.
- **Dart:** `dart_bench` from `//dart:defs.bzl` takes `main`, a program that calls `runBenchmarks([...])` from `package:dx_bench` with benchmark_harness `BenchmarkBase` objects. The pinned benchmark_harness comes from the `@dx_dart_bench_deps` hub of rules_dx. Results are in seconds per run.
- **PHP:** `php_bench` from `//php:defs.bzl` takes `srcs`: phpbench files with one class per file, named like the file, with `#[Revs]` and `#[Iterations]` attributes. rules_dx ships the pinned phpbench phar and runs it on the pinned PHP. Results are in seconds per revolution.

## Compare with a baseline

```sh
dx bench --compare .dx/local/bench/last.json
dx bench --compare main --max-regression=20
```

`--compare <path|ref>` reads a `dx-bench/v1` file: one test's `dx-bench.json` or a merged `last.json`. A relative path starts at the workspace root.

A value that is not a file is a git ref. `dx bench` uses `gh` to find the latest workflow run on that branch and download its `dx-bench-<platform>` artifact. It reads `last.json` from the artifact, else the first JSON file in it. You need the `gh` CLI, logged in. When `gh` fails, dx exits `3` with `bench_compare`.

With a baseline, the text table gains `baseline` and `delta %` columns. JSON adds `baseline` and `delta_pct` to each result the baseline covers.

`--max-regression <pct>` needs `--compare`. Without it, dx exits `1` with `bench_compare`. A result is a regression when it is slower than its baseline by more than `<pct>` percent and by more than two standard deviations (the larger stddev of the two). Each regression prints a `bench_regression` line on stderr, and the run exits `1`.

## Output

- Text prints one table per target with `name`, `mean`, `stddev` and `unit`.
- `--output=json` prints the merged document, one entry per target.
- Both modes write the merged document to `.dx/local/bench/last.json`.
- A missing or invalid `dx-bench.json` prints a line on stderr.
- `dx bench` writes no reports.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Success. |
| `1` | Operational failure, `no_tests` (no `dx-bench` test in scope), `bench_regression`, a baseline dx cannot use (`bench_compare`), or a missing or invalid `dx-bench.json`. |
| `2` | Usage, scope, or owner error. |
| `3` | A benchmark test failed (Bazel's code), or `gh` could not list or download the baseline (`bench_compare`). |
| Bazel's code | Bazel failed. dx keeps Bazel's exit code. |
