# PHP

Build, run and test PHP 8.5 with a pinned static PHP CLI build from
[static-php-cli](https://github.com/crazywhalecc/static-php-cli), the pinned
[Composer](https://getcomposer.org) 2.10 for autoloading and the pinned
[PHPUnit](https://phpunit.de) 13 for tests. Nothing is installed on the host,
and nothing is fetched until a target needs it. rules_dx registers the
toolchains, so there is no toolchain setup in `MODULE.bazel`.

## Start

Run `dx init --apply demo-app` and add sources. The language is wired when
its sources appear.

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
PSR-4 layout is not required.

## Composer packages

Packages come from a `composer.lock` that Composer wrote. Bazel fetches every
package through its repository cache; Composer never runs at fetch time and
no resolver runs.

```starlark
composer = use_extension("@rules_dx//php/composer:extensions.bzl", "composer")
composer.lock(
    name = "vendor",
    digests = "//:composer.digests.json",
    lock = "//:composer.lock",
)
use_repo(composer, "vendor")
```

Set `dev = True` on the tag when you use the extension with `dev_dependency = True`.

Each package is the label `@vendor//<vendor>/<name>`, for example
`@vendor//psr/http-factory`; put it in the `deps` of a `php_library`,
`php_binary` or `php_test`. Its `require`, `replace` and `provide` entries
become deps, so a requirement that matches no package in the lock fails the
build. `@vendor//:all_packages` and `@vendor//:dev_packages` group the
`packages` and `packages-dev` entries. A target stages its packages under
`vendor/<name>/` and writes their lock entries into
`vendor/composer/installed.json`; the same `composer dump-autoload` then builds
the PSR-4, PSR-0, classmap and `files` autoload. A class whose package is not in
`deps` still fails with `Class "..." not found`. Package files are not linted.

`composer.digests.json` pins the archives, because GitHub dists have an empty
`shasum` in the lock:

```json
{
  "version": 1,
  "packages": {
    "psr/log": {
      "reference": "<dist reference of composer.lock>",
      "sha256": "<archive sha256>",
      "urls": ["https://mirror.example/psr-log.zip"]
    }
  }
}
```

`urls` is optional; those URLs are tried before the lock mirrors and
`dist.url`. GitHub keeps the files of a commit stable but not the zip bytes,
so mirror archives you depend on. After `composer update` a digest whose
`reference` no longer matches the lock fails with "re-pin". A missing or stale
digest downloads the archive without a checksum, then fails and prints the
entry to commit; review it in the diff.

Refused: `composer-plugin` packages, packages without a zip or tar dist
(source-only), and `ext-*` requirements the pinned PHP lacks. Scripts never
run. Do not depend on `phpunit/phpunit` from the lock; `php_test` uses the
pinned PHPUnit phar.

## Generating BUILD files

`dx generate` writes `php_library` and `php_test` rules. Each package takes
its own `.php` files plus every `.php` under `src/` that has no `BUILD` file of
its own, and gets one `php_test` per `*Test.php` under `tests/` or `test/`
(`GreeterTest.php` becomes `greeter_test`). One library holds a whole tree, so
namespaces that refer to each other do not form dependency cycles.

A class is found by what a file declares, not by its path, to match the
classmap autoload: the parser indexes every class, interface, trait and enum
declared at the top level of a file or namespace body, plus namespaced
functions and constants. References come from `use` clauses and from the
class positions of the code (`extends`, `implements`, `new`, `X::`,
`instanceof`, `catch`, type declarations, attributes and `::class`). They
resolve by PHP's lexical rules: the current namespace and the `use` aliases.

- Classes, functions and constants that ship with PHP need no dep.
  `PHPUnit\` names need none in a `php_test`.
- A Composer package needs one mapping, for example
  `# gazelle:resolve php Monolog\Logger @vendor//monolog/monolog`.
- Unqualified function and constant names are skipped, because PHP falls back
  to the global one. `require` and `include` are not read.
- `php_binary` targets and `bin/` scripts stay hand-written. A hand-set
  `autoload_files` stays out of `srcs` and its declarations are indexed.

## Quality

PHP sources form the `php` class, so `dx check` sees them. No formatter runs
yet, and there is no coverage: the pinned build has neither Xdebug nor PCOV.
Until the formatter lands, the PHP rules add the `no-format` tag to every target.

PHP_CodeSniffer 4.0.4 lints `php` targets against PSR12 or a bound ruleset.
Select `phpcs` in a `php` family and pass the tool flag:

```sh
dx lint -- --@rules_dx//config:tool_phpcs=@rules_dx//php/tools:phpcs
```

PHPStan 2.3.3 type checks `php` targets. Select `phpstan` in a `php` family
and pass the tool flag:

```sh
dx typecheck -- --@rules_dx//config:tool_phpstan=@rules_dx//php/tools:phpstan
```

Dependencies are scanned, not run. See `docs/cli/commands/quality.md`.

## Platforms

| Cell | Status |
| --- | --- |
| `linux_x86_64` | Pinned; qualified by CI |
| `linux_arm64` | Pinned; qualified by CI |
| `macos_arm64` | Pinned; qualified by CI |
| `windows_x86_64` | Gap (#4552); needs a Windows launcher |
| `windows_arm64` | Unsupported; no Windows arm64 PHP 8.5 build exists |

Three cells have a pinned PHP. The rules mark their targets
incompatible elsewhere, so a build fails instead of running the host PHP.
Launchers start through a bash script.

## License

The PHP binary is under the PHP License 3.01. It links extension libraries
that keep their own licenses, such as libxml2, OpenSSL, curl, SQLite and
zlib; static-php-cli lists them in the `license/` folder next to each build.
Composer and PHPUnit are under the MIT and BSD-3-Clause licenses.
