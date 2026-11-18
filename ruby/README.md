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

`bazel run @rules_dx//ruby/private/tools/lock -- --dir <absolute set directory>` reads `Gemfile`,
resolves exact versions from the rubygems.org API, downloads each gem and writes `ruby.lock.json`
and the state file `Gemfile.lock`. Unchanged gems are kept without a download. `dx update` on a
ruby set runs the resolver with `--check`, `dx update --apply` runs it, and `<set>:<gem>` adds
`--upgrade <gem>`. `dx update <set>:<gem> <version> --apply` edits the gem's `Gemfile` line
and runs the resolver with `--upgrade <gem>`.

With `ruby.lock.json` beside the root `Gemfile`, `dx generate --apply` adds the `ruby.lock`
stanza for the `@gems` hub to `MODULE.bazel`:

```python
ruby = use_extension("@rules_dx//ruby:extensions.bzl", "ruby")
ruby.lock(
    name = "gems",
    lock = "//:ruby.lock.json",
)
use_repo(ruby, "gems")
```

The hub downloads each gem by the sha256 in `ruby.lock.json` and installs the bundle as one
tree with the pinned Bundler. Each gem is also the label `@gems//<name>`.

Depend on the bundle as `@gems`. To run RSpec, set `main` to its bin and pass
the spec path in `args`:

```python
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
files and `require` lines. The command reference covers `dx generate`. The `ruby/tests/fixtures/adopt_ruby` tree shows a full run.

## Quality

### Formatting and linting

`rubocop` formats and lints. It runs from the gems of a bundle you
declare, and you point `rubocop` at it in `.bazelrc`. See
the Quality page of the command reference for the bundle, the flags and the
family setup.

```sh
bazel run @rules_dx//:dx -- format //app/...
bazel run @rules_dx//:dx -- lint //app/...
```

### Type checking with Sorbet

`dx typecheck` runs the pinned Sorbet 0.6.13650 (the `sorbet-static` gem, no Ruby
needed) over `ruby_library`, `ruby_binary` and `ruby_test` sources. Select
`sorbet` in a `ruby` family. `--@rules_dx//config:tool_sorbet` overrides the pinned binary.

- Sorbet follows the `# typed:` sigil of each file. Without a sigil a file is
  `typed: false`, which still reports syntax errors and unresolved constants.
- `sorbet/config` of your source tree is never read (`--no-config`), and no cache
  directory is used.
- The sources and `.rbi` files of `deps` are passed along, so their constants
  resolve; findings in them are never reported.
- Sorbet does not read gems. A constant from a gem needs an RBI (for example from
  tapioca) in a dep, or a `# typed: ignore` on the file.
- Sorbet has no Windows build, so the check is not available on Windows.

## Coverage

Ruby tests write LCOV under `bazel coverage` and `dx coverage` through
SimpleCov. rules_dx brings pinned SimpleCov 1.3.2 and simplecov-lcov 0.9.0
and puts them on the load path only under coverage, so your `Gemfile` needs
neither and `bazel test` never fetches them. A bundle that has its own
`simplecov` wins: its SimpleCov then runs with the pinned simplecov-lcov.
Records name each file as `SF:./<path>`.

Tag a test `no-coverage` to skip it under coverage.

## Platforms

| Cell | Format and lint |
| --- | --- |
| `linux_x86_64` | Supported |
| Other cells | Not supported |
