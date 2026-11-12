# Perl

Build, run and test Perl with the relocatable Perl 5.44 that
[rules_perl](https://github.com/bazel-contrib/rules_perl) pins. Nothing is
installed on the host and nothing is fetched until a target needs it.
rules_perl registers the toolchains, so there is no toolchain setup in
`MODULE.bazel`.

## Dependencies

### Packages

Declare a hub with the `cpan` extension. Its lock, `cpan.lock.json`, pins
each distribution of `cpanfile.snapshot` to a URL and a sha256.

```python
cpan = use_extension("@rules_dx//perl:extensions.bzl", "cpan")
cpan.lock(
    name = "cpan",
    lock = "//third_party/perl:cpan.lock.json",
)
use_repo(cpan, "cpan")
```

Depend on `@cpan//Text-CSV` for one distribution, and on
`@cpan//Perl-Tidy:bin_perltidy` for a script under its `bin/` or `script/`.
`bazel run @rules_dx//perl/private/tools/lock -- --dir <set directory>`
writes the lock from `cpanfile` and `cpanfile.snapshot`. A wrong digest
fails the fetch.

## Generate

`dx generate` writes `perl_library` and `perl_test` rules. Each package takes
its own `.pm` files plus every `.pm` under `lib/` that has no `BUILD` file of
its own, and gets one `perl_test` per `t/*.t`. A module is named by its path
under `lib/` (`lib/Inventory/Item.pm` is `Inventory::Item`), never by its
`package` statement. This matches the default `includes` of rules_perl.

- Modules that ship with Perl need no dep.
- A CPAN module resolves to the distribution whose `provides` in `cpan.lock.json` lists it.
- `perl_binary` targets and `.pl` scripts stay hand-written.
- Loads inside `eval` are optional and are skipped. Loads in POD, heredocs,
  strings and after `__END__` are ignored.
- Helper libraries under `t/lib` are not generated.

## Quality

Perl sources form the `perl` class, so `dx check` sees them. The Perl
wrappers carry perltidy 20260826, which `dx check` runs as the
formatter with its default style, LF line endings and no profile, so
`.perltidyrc` and `$PERLTIDY` are never read. They also carry Perl::Critic
1.156, which runs as the linter at severity 5 only with no profile, so
`.perlcriticrc` and `$PERLCRITIC` are never read. Perl::Critic needs two XS
modules the cpan hub cannot build, so `perl/private/tools/perlcritic.pl` swaps in
pure-Perl stand-ins (Clone::PP and Params::Util's PP mode). Until typecheck
exists, the Perl rules add the `no-typecheck` tag to every target. Set
`--@rules_dx//config:tool_perltidy` or `tool_perlcritic` to use another tool.

## Coverage

`dx coverage` runs `perl_test` through the `DxLcov` collector in
`@rules_dx//perl/private/coverage`. It loads in every Perl the test starts, counts the
statements that run, and writes LCOV for the instrumented sources to
`$COVERAGE_OUTPUT_FILE`. Devel::Cover needs XS, which the `cpan` extension
cannot build.

## Platforms

See the [support matrix](../docs/support-matrix.md).

`windows_arm64` is unsupported.

rules_perl has no toolchain for other platforms. The wrappers mark their
targets incompatible there, so a build fails instead of running the host
Perl. Unix launchers start through a bash script.
