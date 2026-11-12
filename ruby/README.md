# Ruby

Build, run and test Ruby with a pinned Ruby 3.4.12 and rules_ruby 0.29.0.
Nothing is installed on the host and nothing is fetched until a target needs
it. rules_dx registers the toolchain, so there is no setup in `MODULE.bazel`.

## Dependencies

### Gems

Declare your own bundle in `MODULE.bazel`:

```python
ruby = use_extension("@rules_dx//ruby:extensions.bzl", "ruby")
ruby.bundle_fetch(
    name = "gems",
    gem_checksums = {...},
    gemfile = "//:Gemfile",
    gemfile_lock = "//:Gemfile.lock",
)
use_repo(ruby, "gems")
```

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
files and `require` lines. The command reference covers `dx generate`. The `ruby/tests/fixtures/adopt-ruby` tree shows a full run.

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
SimpleCov. rules_dx brings pinned SimpleCov 0.22.0 and simplecov-lcov 0.9.0
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
