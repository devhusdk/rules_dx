# PHP

This directory helps you build, run and test PHP code with rules_dx.

Load its rules in a `BUILD.bazel` file, the file that declares what Bazel
builds, from `@rules_dx//php:defs.bzl`.
`extensions.bzl` holds the module extension: the part you call from
`MODULE.bazel` to set up toolchains and dependencies.
The tests live in `tests/`. The [PHP page](../docs/languages/php.md) shows
setup, every rule and the tools dx runs.

Run the PHP tests from a rules_dx checkout:

```sh
tools/dx test //php/...
```

## Dependencies

### Autoloading

Each `php_binary` and `php_test` runs one build action on the pinned PHP:

1. It copies the target's sources and the sources of all its `deps` into
   `<name>.php/`.
2. It runs `composer dump-autoload --classmap-authoritative` offline, with
   no network, no git and an empty home directory.
3. The binary loads the result with `auto_prepend_file`. The test loads it
   with PHPUnit's `--bootstrap`.

The class map lists only classes from the target and its `deps`. A class
from a library that is not in `deps` fails with `Class "..." not found`. You
do not need a PSR-4 layout.

### Runfiles

Runfiles are the files Bazel places beside a program when it runs, such as
its `data`. Put `@rules_dx//php:runfiles` in `deps` and look a file up by its
runfiles path:

```php
use Bazel\Runfiles\Runfiles;

$path = Runfiles::create()->rlocation('_main/app/data.txt');
```

- `rlocation` works with a runfiles directory and with a manifest only
  (`RUNFILES_MANIFEST_ONLY=1`).
- It returns `null` when the manifest does not list the path.
- An apparent repository name, like `my_dep/file.txt`, maps through the repo
  mapping.
- `envVars()` returns the environment a child Bazel program needs to find
  the same runfiles.

### Composer packages

List your requirements in `composer.json`. Then:

1. Run `dx update --apply`. It resolves the requirements from the Packagist
   API and writes `composer.lock.json` beside `composer.json`. It also
   writes the `composer.lock` that Composer reads.
2. Run `dx generate --apply`. With `composer.lock.json` beside the root
   `composer.json`, it adds the `composer.lock` stanza for the `@composer`
   hub to `MODULE.bazel`. A hub is one Bazel repository that holds a target
   for every locked package.
3. Put a package in the `deps` of a `php_library`, `php_binary` or
   `php_test` as `@composer//<vendor>/<name>`, for example
   `@composer//psr/http-factory`.

Bazel fetches every package through its repository cache. Composer never
runs at fetch time, and package scripts never run.

How packages load:

- A package's `require`, `replace` and `provide` entries become its deps. A
  requirement that matches no package in the lock fails the build.
- A target stages its packages under `vendor/<name>/` and writes their lock
  entries into `vendor/composer/installed.json`. The same
  `composer dump-autoload` then builds the PSR-4, PSR-0, classmap and
  `files` autoload.
- A class whose package is not in `deps` fails with `Class "..." not found`.
- Package files are not linted.

How the lock picks versions:

- It keeps a package's locked version while its requirements allow it.
  Otherwise it picks the newest stable release they allow.
- `dx update <set>:<package>` resolves that one package again.
- It records each package's archive sha256 and PSR-4 namespaces, and reuses
  unchanged packages without downloading them.

The lock refuses:

- branch constraints and a `minimum-stability` other than `stable`;
- `composer-plugin` packages;
- packages without a zip dist (source-only);
- `ext-*` requirements the pinned PHP lacks.

Do not depend on `phpunit/phpunit` from the lock. `php_test` uses the pinned
PHPUnit phar.

## Generate

`dx generate` writes `php_library` and `php_test` rules.

- Each package takes its own `.php` files plus every `.php` under `src/`
  that has no `BUILD` file of its own. One library holds a whole tree, so
  namespaces that refer to each other do not form dependency cycles.
- Each `*Test.php` under `tests/` or `test/` gets one `php_test`:
  `GreeterTest.php` becomes `greeter_test`.

A class is found by what a file declares, not by its path, to match the
classmap autoload. The parser indexes every class, interface, trait and enum
declared at the top level of a file or namespace body, plus namespaced
functions and constants. References come from `use` clauses and from class
positions in the code: `extends`, `implements`, `new`, `X::`, `instanceof`,
`catch`, type declarations, attributes and `::class`. They resolve by PHP's
own rules: the current namespace and the `use` aliases.

- Classes, functions and constants that ship with PHP need no dep.
  `PHPUnit\` names need none in a `php_test`.
- A Composer package resolves through the lock. You need no directive.
- Unqualified function and constant names are skipped, because PHP falls
  back to the global one. `require` and `include` are not read.
- `php_binary` targets and `bin/` scripts stay hand-written. A hand-set
  `autoload_files` stays out of `srcs`, and its declarations are indexed.

## Quality

PHP sources form the `php` class, so `dx check` sees them. Select the tools
in a `php` family in `dx.toml`:

```toml
[families.php]
format = ["prettier"]
lint = ["phpcs"]
typecheck = ["phpstan"]
```

| Tool | What it does | Override flag |
| --- | --- | --- |
| `prettier` | formats with `@prettier/plugin-php` | |
| `phpcs` | PHP_CodeSniffer 4.0.4 lints against PSR12 or a ruleset you bind | `--@rules_dx//config:tool_phpcs` |
| `phpstan` | PHPStan 2.3.5 type checks; dependencies are scanned, not run | `--@rules_dx//config:tool_phpstan` |

Pass another binary's label to the override flag, for example:

```sh
dx lint -- --@rules_dx//config:tool_phpcs=//tools:phpcs
```

The [quality commands page](../docs/cli/commands/quality.md) covers the
flags and family setup.

## Coverage

On Linux, macOS and Windows, `dx coverage` runs `php_test` with PCOV and
writes LCOV. On Windows, coverage runs on the official windows.php.net NTS
build with the prebuilt `php_pcov.dll`. Every other Windows action uses the
static `spc-max` build.

## API docs

`php_doc_shard` from `@rules_dx//core:defs.bzl` turns the public
API of a `php_library` into a DocIr shard. The pinned phpDocumentor 3.10.0
phar reads the library's own `srcs` on the pinned PHP.

- Public classes, interfaces, traits, enums, functions, methods, properties
  and constants are documented.
- `@internal` and `@ignore` elements are left out.
- Platforms with no pinned PHP are skipped.

## Platforms

The [support matrix](../docs/support-matrix.md) lists the platforms. There
is no Windows arm64 build of PHP 8.5.

Elsewhere the rules mark their targets incompatible, so a build fails
instead of running the host PHP. Launchers start through a bash script. On
Windows, a small native launcher starts PHP from a descriptor file beside
the `.exe`.

### Refresh the PHP pins

1. Download each archive from `dl.static-php.dev`.
2. Check its SHA-256 against the upstream `.sha256` sidecar.
3. Run `gh release upload mirror-php-<version> <archive>`.
4. Update `php/pins.json`: the mirror URL first, the upstream URL second.

## License

The PHP binary is under the PHP License 3.01. It links extension libraries
that keep their own licenses, such as libxml2, OpenSSL, curl, SQLite and
zlib. static-php-cli lists them in the `license/` folder next to each build.
Composer and PHPUnit are under the MIT and BSD-3-Clause licenses.
