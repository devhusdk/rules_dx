# First build times

How long the first `dx test` takes in a fresh `dx new` project, with every
cache empty. This page is generated from [first-build.json](first-build.json)
by `tools/ci/first_build.py render`. Do not edit it by hand.

| Language | New | Test | Total | Download MB | Date | dx version |
| --- | --- | --- | --- | --- | --- | --- |
| `rust` | pending | pending | pending | pending | pending | pending |
| `python` | pending | pending | pending | pending | pending | pending |
| `javascript` | pending | pending | pending | pending | pending | pending |
| `typescript` | pending | pending | pending | pending | pending | pending |
| `go` | pending | pending | pending | pending | pending | pending |
| `java` | pending | pending | pending | pending | pending | pending |
| `kotlin` | pending | pending | pending | pending | pending | pending |
| `scala` | pending | pending | pending | pending | pending | pending |
| `csharp` | pending | pending | pending | pending | pending | pending |
| `fsharp` | pending | pending | pending | pending | pending | pending |
| `c` | pending | pending | pending | pending | pending | pending |
| `cpp` | pending | pending | pending | pending | pending | pending |
| `rust-web` | pending | pending | pending | pending | pending | pending |
| `rust-android` | pending | pending | pending | pending | pending | pending |
| `ruby` | pending | pending | pending | pending | pending | pending |
| `powershell` | pending | pending | pending | pending | pending | pending |
| `astro` | pending | pending | pending | pending | pending | pending |
| `svelte` | pending | pending | pending | pending | pending | pending |
| `vue` | pending | pending | pending | pending | pending | pending |
| `mdx` | pending | pending | pending | pending | pending | pending |
| `zig` | pending | pending | pending | pending | pending | pending |
| `elixir` | pending | pending | pending | pending | pending | pending |
| `erlang` | pending | pending | pending | pending | pending | pending |
| `ocaml` | pending | pending | pending | pending | pending | pending |
| `haskell` | pending | pending | pending | pending | pending | pending |
| `cuda` | pending | pending | pending | pending | pending | pending |
| `lua` | pending | pending | pending | pending | pending | pending |
| `julia` | pending | pending | pending | pending | pending | pending |

## Method

Times come from a hosted GitHub Linux x86_64 runner and vary on other
hardware and networks. Each language runs `dx new --apply <language> demo` and then `dx test //...` 3 times.
Every run gets its own empty home directory, so the output base, repository
cache, disk cache and dx cache all start empty. The table shows the median.

Bazel version: pending. Not counted: the Bazel binary download (Bazelisk cache), the build of dx itself, and the time to start the runner.

Download MB is the size of the downloaded files in Bazel's repository cache
(content_addressable) after the run. Unpacked repository trees are not
counted. Downloads without a hash bypass that cache, so it is a lower bound.

## Ceilings

Each language has a ceiling of 1.5 times its first median total, rounded up to
a whole minute. The weekly run fails when a median total goes over its ceiling.
A run never raises a ceiling. Raising one takes a pull request that edits
[first-build.json](first-build.json) and says why.

For slow Rust first builds see
[#2955](https://github.com/devhusdk/rules_dx/issues/2955).
