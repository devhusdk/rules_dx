# PHP

Build, run and test PHP 8.5 with a pinned static PHP CLI build from
[static-php-cli](https://github.com/crazywhalecc/static-php-cli), the pinned
[Composer](https://getcomposer.org) 2.10 for autoloading and the pinned
[PHPUnit](https://phpunit.de) 13 for tests. Nothing is installed on the host,
and nothing is fetched until a target needs it. rules_dx registers the
toolchains, so there is no toolchain setup in `MODULE.bazel`.

## Rules

Load the rules from `@rules_dx//php/rules:defs.bzl`.

```python
load("@rules_dx//php/rules:defs.bzl", "php_binary", "php_library", "php_test")

php_library(
    name = "ledger",
    srcs = ["src/Inventory/Ledger.php"],
)

php_test(
    name = "ledger_test",
    srcs = ["tests/LedgerTest.php"],
    deps = [":ledger"],
)

php_binary(
    name = "stockctl",
    ini = {"memory_limit": "256M"},
    main = "bin/stockctl.php",
    deps = [":ledger"],
)
```

| Rule | What it makes |
| --- | --- |
| `php_library` | Sources whose classes other targets load; every file is checked with `php -l` at build time |
| `php_binary` | Runs `main` with `bazel run` |
| `php_test` | Runs PHPUnit test files; a failure or an empty suite fails the test |

- A syntax error fails the build, not the first run.
- PHP runs with `-n`, so no `php.ini` from the host is read. Set ini values
  with `ini`; each entry becomes one `-d key=value` flag.
- `env` sets environment variables for `bazel run` and `bazel test`. Values
  expand `$(rlocationpath ...)` of `data`.
- The launcher exports `RUNFILES_DIR`. Read a data file as
  `getenv('RUNFILES_DIR') . '/' . $path`, with `$path` from
  `$(rlocationpath ...)`.
- `php_test` runs PHPUnit with `--no-configuration`. Pass a `phpunit.xml` as
  `config` to use one.
- `autoload_files` on `php_library` lists files that are loaded on every run,
  like Composer's `files` autoload. Use it for function files.
- `PhpInfo` is exported from `@rules_dx//php/rules:defs.bzl`.

## Autoloading

Each `php_binary` and `php_test` runs one build action on the pinned PHP. The
action copies the target's sources and the sources of all its `deps` into
`<name>.php/`, then runs `composer dump-autoload --classmap-authoritative`
offline. Composer gets no network, no git and an empty home directory. The
binary loads the result with `auto_prepend_file`; the test loads it with
PHPUnit's `--bootstrap`.

The class map lists only classes from the target and its `deps`. A class
from a library that is not in `deps` fails with `Class "..." not found`.
PSR-4 layout is not required, and Composer packages from Packagist are not
supported yet.

## Quality

PHP sources form the `php` class, so `dx check` sees them. No formatter,
linter or type checker runs yet, and there is no coverage: the pinned build
has neither Xdebug nor PCOV. Until they land, the PHP rules add the
`no-format`, `no-lint` and `no-typecheck` tags to every target.

## Platforms

| Cell | Status |
| --- | --- |
| `linux_x86_64` | Qualified once its native CI run is green |
| `linux_arm64` | Not qualified |
| `macos_arm64` | Not qualified |
| `windows_x86_64` | Not qualified; needs a Windows launcher |
| `windows_arm64` | Unsupported; static-php-cli has no build |

Only `linux_x86_64` has a pinned PHP. The rules mark their targets
incompatible elsewhere, so a build fails instead of running the host PHP.
Launchers start through a bash script.

## License

The PHP binary is under the PHP License 3.01. It links extension libraries
that keep their own licenses, such as libxml2, OpenSSL, curl, SQLite and
zlib; static-php-cli lists them in the `license/` folder next to each build.
Composer and PHPUnit are under the MIT and BSD-3-Clause licenses.
