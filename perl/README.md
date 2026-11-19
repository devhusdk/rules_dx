# Perl

This directory helps you build, run and test Perl code with rules_dx.

Load its rules in a `BUILD.bazel` file, the file that declares what Bazel
builds, from `@rules_dx//perl:defs.bzl`.
`extensions.bzl` holds the module extension: the part you call from
`MODULE.bazel` to set up toolchains and dependencies.
The tests live in `tests/`. The [Perl page](../docs/languages/perl.md) shows
setup, every rule and the tools dx runs.

Run the Perl tests from a rules_dx checkout:

```sh
tools/dx test //perl/...
```

## Dependencies

### CPAN packages

List your dependencies in `cpanfile` and pin them in `cpanfile.snapshot`.
Then:

1. Run `dx update --apply`. It writes `cpan.lock.json`, which pins each
   distribution of `cpanfile.snapshot` to a URL and a sha256.
2. Run `dx generate --apply`. With `cpan.lock.json` beside the root
   `cpanfile`, it adds the `cpan.lock` stanza for the `@cpan` hub to
   `MODULE.bazel`. A hub is one Bazel repository that holds a target for
   every locked package.
3. Depend on a distribution as `@cpan//Text-CSV`, or on a script under its
   `bin/` or `script/` as `@cpan//Perl-Tidy:bin_perltidy`.

A wrong digest fails the fetch.

## Generate

`dx generate` writes `perl_library` and `perl_test` rules.

- Each package takes its own `.pm` files plus every `.pm` under `lib/` that
  has no `BUILD` file of its own.
- Each `t/*.t` gets one `perl_test`.
- A module is named by its path under `lib/`, never by its `package`
  statement: `lib/Inventory/Item.pm` is `Inventory::Item`. This matches the
  default `includes` of rules_perl.
- Modules that ship with Perl need no dep.
- A CPAN module resolves to the distribution that lists it in
  `cpan.lock.json`.
- `perl_binary` targets and `.pl` scripts stay hand-written.
- Loads inside `eval` are optional and are skipped. Loads in POD, heredocs,
  strings and after `__END__` are ignored.
- Helper libraries under `t/lib` are not generated.

## Quality

Perl sources form the `perl` class, so `dx check` sees them.

| Role | Tool | Settings |
| --- | --- | --- |
| Formatter | perltidy 20260826 | default style, LF line endings |
| Linter | Perl::Critic 1.156 | severity 5 only |

Neither tool reads a profile: `.perltidyrc`, `$PERLTIDY`, `.perlcriticrc`
and `$PERLCRITIC` are ignored. To use another binary, pass its label to
`--@rules_dx//config:tool_perltidy` or `--@rules_dx//config:tool_perlcritic`.

Perl has no type checker in dx, so the Perl rules tag every target
`no-typecheck`.

## Coverage

`dx coverage` runs each `perl_test` with a statement collector that loads in
every Perl the test starts. It writes LCOV for the instrumented sources to
`$COVERAGE_OUTPUT_FILE`. You do not need Devel::Cover.

## Platforms

The [support matrix](../docs/support-matrix.md) lists the platforms.
`windows_arm64` is not supported.

rules_perl has no toolchain for other platforms. The rules mark their
targets incompatible there, so a build fails instead of running the host
Perl. Unix launchers start through a bash script.
