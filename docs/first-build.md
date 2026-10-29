# First build times

How long the first `dx test` takes in a fresh `dx init` project with the
hello-world fixture copied in, with every cache empty. This page is
generated from [first-build.json](first-build.json) by
`tools/ci/first_build.py render`. Do not edit it by hand.

| Language | Init | Test | Total | Download MB | Date | dx version |
| --- | --- | --- | --- | --- | --- | --- |
| `rust` | pending | pending | pending | pending | pending | pending |
| `python` | pending | pending | pending | pending | pending | pending |
| `javascript` | skipped | skipped | skipped | skipped | skipped | skipped |
| `typescript` | skipped | skipped | skipped | skipped | skipped | skipped |
| `go` | pending | pending | pending | pending | pending | pending |
| `java` | skipped | skipped | skipped | skipped | skipped | skipped |
| `kotlin` | skipped | skipped | skipped | skipped | skipped | skipped |
| `scala` | skipped | skipped | skipped | skipped | skipped | skipped |
| `csharp` | skipped | skipped | skipped | skipped | skipped | skipped |
| `fsharp` | skipped | skipped | skipped | skipped | skipped | skipped |
| `c` | pending | pending | pending | pending | pending | pending |
| `cpp` | pending | pending | pending | pending | pending | pending |
| `rust-web` | skipped | skipped | skipped | skipped | skipped | skipped |
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
| `d` | pending | pending | pending | pending | pending | pending |
| `fortran` | pending | pending | pending | pending | pending | pending |
| `perl` | pending | pending | pending | pending | pending | pending |
| `php` | pending | pending | pending | pending | pending | pending |

Skipped:

- `javascript`: no hello-world fixture yet (#4701)
- `typescript`: no hello-world fixture yet (#4701)
- `java`: no hello-world fixture yet (#4701)
- `kotlin`: no hello-world fixture yet (#4701)
- `scala`: no hello-world fixture yet (#4701)
- `csharp`: no hello-world fixture yet (#4701)
- `fsharp`: no hello-world fixture yet (#4701)
- `rust-web`: no hello-world fixture yet (#4701)

## Method

Times come from a hosted GitHub Linux x86_64 runner and vary on other
hardware and networks. Each language runs `dx init --apply demo-app`, copies in the hello-world fixture and runs `dx test //...` 3 times.
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
