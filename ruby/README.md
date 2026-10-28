# Ruby

Build, run and test Ruby with a pinned Ruby 3.4.12 and rules_ruby 0.29.0.
Nothing is installed on the host and nothing is fetched until a target needs
it. rules_dx registers the toolchain, so there is no setup in `MODULE.bazel`.

## Start

```sh
bazel run @rules_dx//:dx -- new --apply ruby demo
cd demo
bazel build //...
bazel run :demo_bin
bazel test //...
```

## Rules

Load the rules from `@rules_dx//ruby/rules:defs.bzl`. They wrap
[rules_ruby](https://github.com/bazel-contrib/rules_ruby) and take the same
attributes.

```python
load("@rules_dx//ruby/rules:defs.bzl", "ruby_binary", "ruby_library", "ruby_test")

ruby_library(
    name = "hello_lib",
    srcs = ["hello.rb"],
)

ruby_binary(
    name = "hello",
    srcs = ["main.rb"],
    main = "main.rb",
    deps = [":hello_lib"],
)

ruby_test(
    name = "hello_test",
    srcs = ["hello_test.rb"],
    deps = [":hello_lib"],
)
```

| Rule | What it wraps |
| --- | --- |
| `ruby_library` | `rb_library` |
| `ruby_binary` | `rb_binary`. `srcs` is optional |
| `ruby_test` | `rb_test`. Set `main` to a runner such as an RSpec bin |

## Gems

Declare your own bundle with rules_ruby in `MODULE.bazel`:

```python
ruby = use_extension("@rules_ruby//ruby:extensions.bzl", "ruby")
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

## Formatting and linting

`standardrb` formats and `rubocop` lints. Both run from the gems of a bundle
you declare, and you point them at it in `.bazelrc`. See
the Quality page of the command reference for the bundle, the flags and the
family setup.

```sh
bazel run @rules_dx//:dx -- format //app/...
bazel run @rules_dx//:dx -- lint //app/...
```

## Generating BUILD files

`dx generate` writes `ruby_library` and `ruby_test` targets from your `.rb`
files and `require` lines. The command reference covers `dx generate`. The `examples/adopt-ruby` tree shows a full run.

## Platforms

| Cell | Format and lint |
| --- | --- |
| `linux_x86_64` | Supported |
| Other cells | Not supported |

## Coverage

Ruby tests write LCOV under `bazel coverage` and `dx coverage` through
SimpleCov. rules_dx brings pinned SimpleCov 0.22.0 and simplecov-lcov 0.9.0
and puts them on the load path only under coverage, so your `Gemfile` needs
neither and `bazel test` never fetches them. A bundle that has its own
`simplecov` wins: its SimpleCov then runs with the pinned simplecov-lcov.
Records name each file as `SF:./<path>`.

A test without a bundle and Windows hosts need the rules_ruby overrides dx
ships (`patches/rules_ruby/coverage.patch`); run `dx init --apply` to include
them. Without them, Windows still exits 1 under coverage. Tag a test
`no-coverage` to skip it under coverage.
