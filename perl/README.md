# Perl

Build, run and test Perl with the relocatable Perl 5.40 that
[rules_perl](https://github.com/bazel-contrib/rules_perl) pins. Nothing is
installed on the host and nothing is fetched until a target needs it.
rules_perl registers the toolchains, so there is no toolchain setup in
`MODULE.bazel`.

## Start

Scaffold a working project with `dx new --apply perl demo-app`.

## Rules

Load the rules from `@rules_dx//perl/rules:defs.bzl`. They wrap rules_perl
and take the same attributes, `select()` values included.

```python
load("@rules_dx//perl/rules:defs.bzl", "perl_binary", "perl_library", "perl_test")

perl_library(
    name = "ledger",
    srcs = ["lib/Inventory/Ledger.pm"],
)

perl_test(
    name = "ledger_test",
    srcs = ["t/ledger.t"],
    deps = [":ledger"],
)

perl_binary(
    name = "stockctl",
    srcs = ["stockctl.pl"],
    deps = [":ledger"],
)
```

| Rule | What it makes |
| --- | --- |
| `perl_library` | Modules from `.pm` files, with `lib` and the package on `@INC` |
| `perl_test` | Runs one `.t` script; a non-zero exit fails the test |
| `perl_binary` | Runs one `.pl` script with `bazel run` |

- Library `tags` reach the library; the generated upstream target is
  `manual`, so `//...` builds only the library.
- A module that a target does not list in `deps` is not on `@INC`, so
  `use` fails with `Can't locate`.
- Read data files through `@rules_perl//perl/runfiles` and its `Runfiles`
  module.
- `PerlInfo` passes through unchanged. Load it from
  `@rules_dx//perl/rules:defs.bzl`.
- There is no build-time compile check. A syntax error fails the first test
  or run that loads the module.
- `perl_xs` is not wrapped, and the `cpan` extension does not build XS
  distributions.

## CPAN dependencies

Declare a hub with the rules_perl `cpan` extension. Its lock pins each
distribution to a URL and a sha256.

```python
bazel_dep(name = "rules_perl", version = "1.5.0")

cpan = use_extension("@rules_perl//perl/cpan:extensions.bzl", "cpan")
cpan.install(
    name = "cpan",
    lock = "//third_party/perl:cpanfile.snapshot.lock.json",
)
use_repo(cpan, "cpan")
```

Depend on `@cpan//:Text-CSV` for one distribution. To make or refresh the
lock, add a `cpan_compiler` target from
`@rules_perl//perl/cpan:cpan_compiler.bzl` next to the `cpanfile` and run it.
A wrong digest fails the fetch.

## Generating BUILD files

`dx generate` writes `perl_library` and `perl_test` rules. Each package takes
its own `.pm` files plus every `.pm` under `lib/` that has no `BUILD` file of
its own, and gets one `perl_test` per `t/*.t`. A module is named by its path
under `lib/` (`lib/Inventory/Item.pm` is `Inventory::Item`), never by its
`package` statement. This matches the default `includes` of rules_perl.

- Modules that ship with Perl need no dep.
- A CPAN module needs one mapping, for example
  `# gazelle:resolve perl Text::CSV @cpan//:Text-CSV`.
- `perl_binary` targets and `.pl` scripts stay hand-written.
- Loads inside `eval` are optional and are skipped. Loads in POD, heredocs,
  strings and after `__END__` are ignored.
- Helper libraries under `t/lib` are not generated.

## Quality

Perl sources form the `perl` class, so `dx check` sees them. The Perl
wrappers carry perltidy 20260826, which `dx check` and `dx fix` run as the
formatter with its default style, LF line endings and no profile, so
`.perltidyrc` and `$PERLTIDY` are never read. No Perl linter runs yet. Until one
does, the Perl rules add the `no-lint` and `no-typecheck` tags to every target.
Set `--@rules_dx//config:tool_perltidy` to use another perltidy.

## Platforms

| Cell | Status |
| --- | --- |
| `linux_x86_64` | Qualified |
| `linux_arm64` | Not qualified |
| `macos_x86_64` | Not qualified |
| `macos_arm64` | Not qualified |
| `windows_x86_64` | Not qualified |
| `windows_arm64` | Unsupported |

rules_perl has no toolchain for other platforms. The wrappers mark their
targets incompatible there, so a build fails instead of running the host
Perl. Unix launchers start through a bash script.
