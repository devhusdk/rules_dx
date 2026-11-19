# Ruby

This directory helps you build, run and test Ruby code with rules_dx.

Load its rules in a `BUILD.bazel` file, the file that declares what Bazel
builds, from `@rules_dx//ruby:defs.bzl`.
`extensions.bzl` holds the module extension: the part you call from
`MODULE.bazel` to set up toolchains and dependencies.
The tests live in `tests/`. The [Ruby page](../docs/languages/ruby.md) shows
setup, every rule and the tools dx runs.

Run the Ruby tests from a rules_dx checkout:

```sh
tools/dx test //ruby/...
```

## Dependencies

### Gems

List your gems in `Gemfile`. Then:

1. Run `dx update --apply`. It resolves exact versions from the
   rubygems.org API, downloads each gem and writes `ruby.lock.json` and the
   state file `Gemfile.lock`. Gems that did not change are kept without a
   download.
2. Run `dx generate --apply`. With `ruby.lock.json` beside the root
   `Gemfile`, it adds the `ruby.lock` stanza for the `@gems` hub to
   `MODULE.bazel`. A hub is one Bazel repository that holds a target for
   every locked package.
3. Depend on the whole bundle as `@gems`, or on one gem as `@gems//<name>`.

The stanza looks like this:

```python
ruby = use_extension("@rules_dx//ruby:extensions.bzl", "ruby")
ruby.lock(
    name = "gems",
    lock = "//:ruby.lock.json",
)
use_repo(ruby, "gems")
```

The hub downloads each gem by the sha256 in `ruby.lock.json` and installs the
bundle as one tree with the pinned Bundler.

Update commands:

| Command | What it does |
| --- | --- |
| `dx update` | checks the lock |
| `dx update --apply` | resolves and writes the lock |
| `dx update <set>:<gem> --apply` | upgrades one gem |
| `dx update <set>:<gem> <version> --apply` | sets the gem's `Gemfile` line to that version, then upgrades it |

### RSpec

To run RSpec, set `main` to its bin and pass the spec path in `args`:

```python
load("@rules_dx//ruby:defs.bzl", "ruby_test")

ruby_test(
    name = "greeter_spec",
    srcs = ["greeter_spec.rb"],
    args = ["greeter_spec.rb"],
    main = "@gems//bin:rspec",
    deps = [
        ":spec_helper",
        "@gems",
    ],
)
```

## Generate

`dx generate` writes `ruby_library` and `ruby_test` targets from your `.rb`
files and `require` lines. The
[generate command page](../docs/cli/commands/generate.md) covers the flags.
The `ruby/tests/fixtures/adopt_ruby` tree shows a full run.

## Quality

### Formatting and linting

`rubocop` formats and lints:

```sh
bazel run @rules_dx//:dx -- format //app/...
bazel run @rules_dx//:dx -- lint //app/...
```

It runs from the gems of a bundle you declare, and you point `rubocop` at
that bundle in `.bazelrc`. The
[quality commands page](../docs/cli/commands/quality.md) covers the bundle,
the flags and the family setup.

### Type checking with Sorbet

`dx typecheck` runs the pinned Sorbet 0.6.13650 over `ruby_library`,
`ruby_binary` and `ruby_test` sources. It comes from the `sorbet-static` gem,
so it needs no Ruby. Select `sorbet` in a `ruby` family.

- Sorbet follows the `# typed:` sigil of each file. A file without a sigil
  is `typed: false`, which still reports syntax errors and unresolved
  constants.
- It never reads `sorbet/config` in your source tree (`--no-config`) and uses
  no cache directory.
- The sources and `.rbi` files of `deps` are passed along, so their
  constants resolve. Findings in them are never reported.
- Sorbet does not read gems. A constant from a gem needs an RBI in a dep,
  for example from tapioca, or a `# typed: ignore` on the file.
- Sorbet has no Windows build, so the check does not run on Windows.
- `--@rules_dx//config:tool_sorbet` takes the label of another Sorbet.

## Coverage

Ruby tests write LCOV under `bazel coverage` and `dx coverage` through
SimpleCov. rules_dx brings SimpleCov 1.3.2 and simplecov-lcov 0.9.0 and puts
them on the load path only under coverage. Your `Gemfile` needs neither, and
`bazel test` never fetches them.

- A bundle with its own `simplecov` wins. Its SimpleCov then runs with the
  pinned simplecov-lcov.
- Each record names a file as `SF:./<path>`.
- Tag a test `no-coverage` to skip it.

## Platforms

The [support matrix](../docs/support-matrix.md) lists the platforms. The
[quality commands page](../docs/cli/commands/quality.md) lists where each
quality tool runs.
