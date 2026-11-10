# Language skeleton design

This page fixes the shape of every language directory. A language directory is a root directory with a `lang.json`. Platform and packaging directories such as `android`, `apple`, `qt`, `cuda`, `appimage` and `msix` are language directories. `docs/design/layout.md` fixes the repository around them.

## Membership

- A language directory exports at least one rule or macro from `defs.bzl`, or one module extension from `extensions.bzl`.
- A directory that exports neither is not a root directory. Its quality code follows the quality rules in `docs/design/layout.md`. Its tests move to the language that owns the rules they test.
- A runtime shared by several languages is a language directory. Those languages load it from `//<runtime>/private`.

## Entries

A language directory holds these entries and no others.

| Entry | Required | Holds |
| --- | --- | --- |
| `BUILD.bazel` | yes | `toolchain_type`, public flags, public platforms |
| `defs.bzl` | yes | public rules, macros and providers |
| `extensions.bzl` | no | public module extensions and repository rules |
| `lang.json` | yes | manifest |
| `pins.json` | no | tool pins |
| `README.md` | no | guide |
| `private/` | yes | implementation |
| `tests/` | yes | tests and fixtures |

- A language has at most one pin file. The manifest `pins` field is `["pins.json"]` or empty.
- A repository `BUILD` template is `<lang>/private/<repo>.BUILD.bazel`.
- A public platform, constraint value or flag is a target in `<lang>/BUILD.bazel`. No public package sits below it.

## Private directories

These names are reserved inside `<lang>/private/`. Code with that role uses that name.

| Name | Holds |
| --- | --- |
| `toolchain/` | toolchain rules, repository rules and the hub |
| `quality/` | aspects, tool rows and tool packages |
| `coverage/` | coverage collectors and reports |
| `bench/` | the bench harness |
| `fuzz/` | the fuzz harness |
| `ide/` | editor integration |
| `tools/` | tool programs |
| `platforms/` | private platforms and constraint values |

- `private/` holds no directory named `toolchains`, `tests`, `testing`, `testdata` or `fixtures`. One exception: `private/tools/<tool>/testdata/`.
- `<lang>/private/defs.bzl` defines the public macros. Other files there hold what it loads.

## Public names

- `defs.bzl` holds a docstring, `load` statements and `name = _name` assignments. It defines nothing.
- `defs.bzl` exports rules, macros and providers. It exports no constant and no helper function.
- A rule or macro name starts with `<lang>_`.
- A rule that builds another language's sources is named `<lang>_<other>_<kind>`. `<other>` is a language directory name.
- The common kinds are `library`, `binary`, `test`, `bench` and `fuzz_test`. A rule with one of these roles uses that suffix.
- A provider name ends in `Info`. Lower-cased, it starts with the directory name without underscores.
- A module extension that creates the language toolchain is named `<lang>`. Any other extension name starts with `<lang>_` or is an ecosystem id from the manifest.
- The manifest `entry_points` field equals the set of rules and macros `defs.bzl` exports.
- The manifest `title` is the display name. It differs from `name`.

## Executable macros

- Every `<lang>_binary`, `<lang>_test`, `<lang>_bench` and `<lang>_fuzz_test` is a macro that calls `dx_wrap_binary` or `dx_wrap_test` from `//core:wrapper.bzl`.
- A language whose runner is not an upstream rule passes its own rule as `upstream_rule`.
- The signature rules in `docs/design/layout.md` apply.

## Platforms

- A public rule or macro that builds for a platform other than the command-line platform takes `platforms`.
- `platforms` is a string list of platform labels. Unset or empty means the command-line platform.
- No other parameter selects a target platform. `platform`, `target_platform`, `cells`, `os` and `arch` are not parameter names.
- The rule applies the transition in `core/platform.bzl`. No file under `<lang>/private/` names `//command_line_option:platforms`.
- `target_compatible_with` only marks compatibility. A macro derives it from the manifest `platforms` field through `core/modules/langs.bzl`.

## Toolchains

- A language that defines its own toolchain declares `//<lang>:toolchain_type`.
- Its toolchains come from the `<lang>` extension, built on `pins_hub` in `core/toolchain.bzl`.
- The hub repository is `dx_<lang>_toolchains`. `MODULE.bazel` registers it with one line: `register_toolchains("@dx_<lang>_toolchains//:all")`.
- A language that uses an upstream rule set's toolchain registers the upstream repository instead.
- `register_toolchains` never takes a `//` label and never sets `dev_dependency`.
- Where registration order matters, the hub names its targets so that name order is the order.
- Every `use_extension` on a rules_dx label names `//<dir>:extensions.bzl`.
- Every toolchain and SDK a public rule resolves is declared without `dev_dependency`. A consumer adds no `register_toolchains` line.
- An extension has defaults for every tag. A consumer tag only selects a version, adds components or accepts a license.
- An SDK under a license is declared for every consumer. Its repository fails with the acceptance line when it is fetched unaccepted.
- A `dev_dependency` proxy of a language extension calls only license tags and tags that declare test dependencies.

## Tests

- `<lang>/tests/BUILD.bazel` declares the language tests.
- Starlark suites are `<lang>/tests/<subject>_tests.bzl`. Rule and macro analysis tests are in `defs_tests.bzl`. Expected analysis failures are in `failure_tests.bzl`. Pin and hub checks are in `toolchain_tests.bzl`.
- A unit test of a private tool program sits in that program's package. No other test sits in `private/`.
- A suite loads private symbols through the package visibility of `//<lang>/private`. `defs.bzl` exports nothing for tests.
- A language with executable macros calls `dx_wrapper_contract_tests` from `//core:conformance.bzl` in `tests/BUILD.bazel`.
- Shared test macros come from `core/` and `tools/testing/`. A language defines no copy of them.

## Fixtures

- Fixtures live in `<lang>/tests/fixtures/<name>/`. `<name>` is lower-case with underscores.
- Every language has `tests/fixtures/hello/`. It holds the smallest target of each common kind the language exports.
- A fixture that is a workspace of its own lives in `tests/fixtures/<name>/repo/`.
- The path of every fixture directory appears in a `BUILD.bazel`, `.bzl` or `MODULE.bazel` file outside that fixture. A fixture that no file names is deleted.
- `examples/` holds only workspaces that span several languages. A one-language example is the `hello` fixture.

## README

- `docs/languages/<lang>.md` is generated. It holds `Install` and `Rules`.
- `<lang>/README.md` is the hand-written guide. A language with nothing to add has none.
- Its first line is `# <title>`.
- Its `##` headings come from this list, in this order: `Dependencies`, `Generate`, `Quality`, `Coverage`, `Bench and fuzz`, `API docs`, `Packaging`, `Toolchain`, `Platforms`, `License`.
- Any other topic is a `###` heading under one of them.
- It has no `Install`, `Start` or `Rules` section.
- The generator appends the README sections to the generated page.

## Structure lint

- `//tools/testing:language_skeleton_test` is a `rust_test`. It reads the snapshots that `docs/design/tests.md` names.
- It checks the Entries, Private directories, Public names, Toolchains and README rules, the test and fixture locations, and the two decidable Platforms rules: the type of each `platforms` attr and the option name under `private/`.
- `tools/testing/language_skeleton_allowlist.txt` holds one line per violation: `<rule> <path>`. Lines are sorted.
- The lint fails on a violation that has no line.
- The lint fails on a line that matches no violation.
- The allowlist only shrinks. No change adds a line.
- A new language directory passes with no line.

## New language

A new language is these steps and no others.

1. Add `<lang>/lang.json`.
2. Add `<lang>/BUILD.bazel`, `<lang>/defs.bzl`, `<lang>/private/defs.bzl` and `<lang>/tests/BUILD.bazel`.
3. Add `<lang>/tests/fixtures/hello/`.
4. Add `<lang>/pins.json` and `<lang>/extensions.bzl` when the language pins a toolchain.
5. Add the `use_extension` and `register_toolchains` lines to `MODULE.bazel`.
6. Run the generators.
