# dx_deployment

`dx deploy <label>` builds the target and runs its dry run: it stages,
validates and prints what would be published, and contacts no registry.
`dx deploy --apply <label>` publishes. Each launcher still needs its own
`<X>_APPROVED=1` and credentials, which dx never sets: a human sets them in
the shell, CI sets them in `reusable-release.yml`. In an agent session
`--apply` is refused unless `CI=true`. A plain `bazel run` of a launcher
publishes only with `<X>_LIVE=1`.

`size_budget` is optional, such as `25MB` (decimal `B`, `KB`, `MB` or `GB`).
Building the deployment fails when its `artifacts` together are larger, and
prints both sizes in bytes. Empty means no budget.

# maven_deploy

Publishes one jar and its pom to a Maven repository.

```starlark
load("@rules_dx//deploy/rules:maven.bzl", "maven_deploy")

maven_deploy(
    name = "maven_demo",
    artifact = "maven_demo",
    group = "com.example",
    jar = ":maven_demo_lib",
    pom = "maven_demo-0.0.0.pom",
    version = "0.0.0",
)
```

- `jar`, `pom`: required. The files to upload.
- `group`, `artifact`: required. The Maven coordinates.
- `sources`, `javadoc`: optional single `.jar` targets staged as
  `<artifact>-<version>-sources.jar` and `-javadoc.jar`, each with md5, sha1,
  sha256, sha512 and (when signing) `.asc`. Central requires both, so a live
  upload refuses before any network call when either is missing. Local staging
  works without them and prints `sources: yes|no` and `javadoc: yes|no`. A jar
  that holds only a README is accepted by Central as a placeholder.
- `version`: defaults to `0.0.0`.
- `repository_url`: defaults to the Central Portal Publisher API below. No
  other host is accepted.
- `profile`: `debug`, `dev` or `release`. Defaults to `release`.

Default run stages a local file repository under `<artifact>-repo/`. No
credentials needed. It writes, next to the jar and the pom:

- `.md5`, `.sha1`, `.sha256` and `.sha512` files with bare lowercase hex.
- `.asc` detached signatures when `MAVEN_GPG_KEY` is set.
- `maven-metadata.xml` one level up.

Output ends with `signed: yes` or `signed: no`.

A default run checks the pom against `group`, `artifact` and `version` and
the Central fields, and stages the repository. It does not contact Central.
Rerunning a published version: the Portal rejects the second upload in its
validation, after the deployment id is printed. Nothing is checked locally.

Signing env:

- `MAVEN_GPG_KEY`: an ASCII-armored OpenPGP secret key. Ed25519 is
  recommended. A signing subkey is used when the key has one.
- `MAVEN_GPG_PASSPHRASE`: unlocks `MAVEN_GPG_KEY`.

The pom must match `group`, `artifact` and `version` (`groupId` may come
from `parent`). `packaging` must be absent or `jar`. It must also have
`name`, `description`, `url`, `licenses/license/name`,
`developers/developer`, `scm/url`, `scm/connection` and
`scm/developerConnection`. The error names every missing field.

Live upload goes to the Central Portal Publisher API:
`https://central.sonatype.com/api/v1/publisher/`

Setup:

- A Central Portal account with a verified namespace.
- A Portal user token for `MAVEN_USERNAME` and `MAVEN_PASSWORD`.

Run:

```
MAVEN_PUBLISH_APPROVED=1 \
MAVEN_USERNAME=... MAVEN_PASSWORD=... \
MAVEN_GPG_KEY=... MAVEN_GPG_PASSPHRASE=... \
dx deploy --apply //pkg:maven_demo
```

Live upload builds the same signed file repository as a default run, zips
it without `maven-metadata.xml`, and uploads it with
`publishingType=USER_MANAGED`. Output names the deployment id. The
deployment stays a draft until you validate and publish it in the Central
Portal. The Portal rejects unsigned bundles.

Refusals, all exit 1:

- Version `0.0.0` in a live run.
- A `-SNAPSHOT` version in a live run.
- Missing `MAVEN_USERNAME` or `MAVEN_PASSWORD`.
- Missing `MAVEN_PUBLISH_APPROVED=1`.
- No `MAVEN_GPG_KEY` without `MAVEN_ALLOW_UNSIGNED=1`.
- `MAVEN_GPG_PASSPHRASE` set without `MAVEN_GPG_KEY`.
- A `MAVEN_GPG_KEY` that is not an armored secret key, or a
  `MAVEN_GPG_PASSPHRASE` that does not unlock it.
- Any other `repository_url` than the Publisher API.
- Credentials the Portal rejects (HTTP 401 or 403).
- A pom that is missing Central fields or does not match the coordinates.

# npm_deploy

Packs files into an npm tarball and a local feed.

```starlark
load("@rules_dx//deploy/rules:npm.bzl", "npm_deploy")

npm_deploy(
    name = "npm_demo",
    srcs = ["npm_demo/package.json"],
    package = "npm-demo",
)
```

- `package`: required. The npm package name, `name` or `@scope/name`.
- `srcs`: files to pack, including `package.json`. Defaults to `[]`.
- `tag`: the dist-tag. Defaults to `latest`.
- `registry`: defaults to `https://registry.npmjs.org`.
- `access`: `public` or `restricted`. Defaults to `public`. `restricted`
  needs a scoped package.
- `provenance`: `required` or `none`. Defaults to `required` for `public`
  and `none` for `restricted`. `required` needs `access = "public"` and
  `registry.npmjs.org`.
- `profile`: `debug`, `dev` or `release`. Defaults to `release`.

Default run stages the tarball and feed locally. No credentials needed. It
checks that the files include `package.json`. It does not run `npm` or contact the
registry.

Rerunning a published version: `npm publish` fails and the run exits 1. Nothing
is checked before the call.

Setup:

- `npm` on `PATH`.
- An `NPM_TOKEN` from the registry.
- `provenance = "required"` needs a CI OIDC identity
  (`ACTIONS_ID_TOKEN_REQUEST_URL` or `SIGSTORE_ID_TOKEN`).

Run:

```
NPM_PUBLISH_APPROVED=1 NPM_TOKEN=... \
dx deploy --apply //pkg:npm_demo
```

`NPM_PUBLISH_DRY_RUN` is refused.

# cloudflare_pages_deploy

Publishes a static bundle directory to an existing Cloudflare Pages project
with a pinned Wrangler that Bazel builds.

```starlark
load("@rules_dx//deploy/rules:cloudflare.bzl", "cloudflare_pages_deploy")

cloudflare_pages_deploy(
    name = "site_deploy",
    account_id = "0123456789abcdef0123456789abcdef",
    project = "my-site",
    branch = "feature/x",
    production_branch = "main",
    site = ":bundle",
)
```

- `site`: one directory, such as a `javascript_web_bundle`. It needs an
  `index.html` at its root.
- `wrangler`: optional. Defaults to the pinned Wrangler in rules_dx, with its
  Node runtime. Set it to your own executable target to override. Ambient
  `npx` is never used.
- `branch`: required. A branch equal to `production_branch` deploys to
  production. Any other branch is a preview. Set `production_branch` to the
  production branch configured on the Pages project.
- The project must already exist. Project creation, DNS, Workers and Pages
  Functions are not handled. `_worker.js`, `functions/` and `_routes.json`
  in the site fail the check.

Check without uploading:

```
dx deploy //pkg:site_deploy
```

This builds the target and validates the site files, the account, the project
and the branch. Nothing is uploaded and no token is needed. It prints:

```
cloudflare_pages_deploy: checked my-site (branch feature/x, preview, profile: release); nothing uploaded
  account: 0123456789abcdef0123456789abcdef
  project: my-site
  branch: feature/x
  intent: preview
  files: 12 (48213 bytes)
  publish: CLOUDFLARE_PAGES_APPROVED=1 with CLOUDFLARE_API_TOKEN, then dx deploy --apply
```

Publish:

```
CLOUDFLARE_PAGES_APPROVED=1 \
CLOUDFLARE_API_TOKEN=... \
dx deploy --apply //pkg:site_deploy
```

`dx deploy --apply` without `CLOUDFLARE_PAGES_APPROVED=1` is refused.
The token needs the Cloudflare Pages edit permission. It is read only on a
live run. It is never written into the build. Wrangler runs without prompts,
telemetry or project discovery.

Output on success:

```
cloudflare_pages_deploy: deployed my-site (branch feature/x, preview, profile: release)
  account: 0123456789abcdef0123456789abcdef
  project: my-site
  branch: feature/x
  intent: preview
  files: 12 (48213 bytes)
  deployment: <deployment id>
  url: https://<hash>.my-site.pages.dev
```

Exit codes: `0` published or checked, `1` any failure or refusal. A
Wrangler failure exits with Wrangler's own code and prints its redacted
output. Arguments and `--dry-run` are refused.
If `CLOUDFLARE_ACCOUNT_ID` is set, it must equal `account_id`.

# archive_deploy

Packages one executable as a tarball plus a sha256 file. No credentials and
no live mode.

```starlark
load("@rules_dx//deploy/rules:archive.bzl", "archive_deploy")

archive_deploy(
    name = "tasks_deploy",
    app = ":tasks",
)
```

- `app`: required. One executable target. It is the only tarball member.
- `profile`: `debug`, `dev` or `release`. Defaults to `release`.
- `size_budget`: optional, such as `25MB`. A decimal `B`, `KB`, `MB` or `GB`
  amount. The run fails before copying when the tarball is larger, and
  prints both sizes in bytes. Empty means no budget.

For an app built with GCC on Linux, `@rules_dx//cc/toolchains:gcc_linux_glibc`
is a reviewed `os_supplied` runtime that records the pinned glibc floor.

Outputs: `<name>_archive` builds `<name>.tar.gz` and `<name>_checksum`
builds `<name>.tar.gz.sha256`.

```
dx deploy --apply //pkg:tasks_deploy
dx deploy --apply //pkg:tasks_deploy -- /tmp/out
```

The run checks the tarball against its checksum and copies both into the
workspace root, or into the one directory argument. More than one argument
is refused. Exit codes: `0` released, `1` any failure or refusal.

Example: [shared-rust](../../examples/shared-rust/README.md#deploy).

# consumer_archive

Packages one app with its runtime files, resources, a `NOTICE` and an
`IDENTITY.txt` as a tar.gz or zip plus a sha256 file. It builds files only and has no deploy run.

```starlark
load("@rules_dx//deploy/rules:archive.bzl", "consumer_archive")

consumer_archive(
    name = "ledger_release",
    app = ":ledger",
    arch = "x86_64",
    min_runtime = "glibc-2.34",
    notice = "NOTICE",
    os = "linux",
    resources = [":runtime"],
)
```

- `app`: required. One executable target, stored at the archive root.
- `arch`: required. `x86_64` or `aarch64`.
- `os`: required. `linux`, `macos` or `windows`.
- `min_runtime`: required. A non-empty requirement such as `glibc-2.34`.
- `linkage`: `static` or `dynamic`. Defaults to `dynamic`.
- `runtime`: files the app needs at run time. Each lands at the archive root
  under its basename, mode 0644. Native libraries (`.dll`, `.dylib`, `.so` or
  `.so.<n>`) must come through a `native_runtime` target; a plain file with
  such a name fails the archive. Defaults to `[]`.
- `format`: `tar.gz` or `zip`. Defaults to `tar.gz`.
- `resources`: targets whose files land under `resources/`. Each file must
  sit under a directory named after its target. Defaults to `[]`.
- `notice`: required. Exactly one file, stored as `NOTICE`.
- `tags`: Bazel tags for every generated target.
- `target_compatible_with`: platform constraints for every generated target,
  so an archive for another OS or CPU is skipped instead of built.

`IDENTITY.txt` lists `name`, `arch`, `os`, `linkage` and `min_runtime`,
then one sorted `runtime: <name> <version> <redistribution>` line per
`native_runtime` target in `runtime`.
A `runtime` entry may be a `native_runtime` target, which records where a
library comes from and whether it may ship:

```starlark
load("@rules_dx//deploy/rules:native_runtime.bzl", "native_runtime")

native_runtime(
    name = "vcruntime",
    srcs = ["vcruntime140.dll"],
    notice = "vcruntime_NOTICE.txt",
    redistribution = "app_local",
    source = "msvc",
    version = "14.3",
)
```

- `redistribution`: required. `app_local` ships `srcs` at the archive root
  and `notice` as `notices/<name>`. `os_supplied` ships nothing and needs
  empty `srcs`. `build_only` fails the archive, because compiler and SDK
  inputs cannot ship.
- `version` and `source`: required and non-empty.
- `notice`: required for `app_local`. The `notice` output group carries it,
  so a `filegroup(output_group = "notice")` can feed a wheel's
  `extra_distinfo_files`.
- `provides`: only for `os_supplied`. Library names the OS supplies for this
  runtime, such as `libGL.so.1`. A trailing `*` matches any name with that
  prefix.

`consumer_archive` checks the import closure of the app and of each shipped
runtime file as a validation action (`archiver --imports`). Every imported
library (ELF `DT_NEEDED`, Mach-O load command, PE import) must be a shipped
member's basename, a system import for that format (the `SYSTEM_IMPORTS` list
in `deploy/rules/src/imports.rs`, also used by release cells), or listed in an
`os_supplied` runtime's `provides`. Otherwise the build fails with
`<member>: not shipped and not a system import: [...]`. Files that are not
binary images are skipped. Set `check_imports = False` to opt out.

`//cc/toolchains:gcc_linux_libstdcxx` is an `app_local` runtime with the
pinned GCC's `libstdc++.so.6` and `libgcc_s.so.1` and the GCC Runtime Library
Exception notice. Build the app with `extra_features = ["dynamic_libstdcxx"]`
on `cc_toolchain_binary`, which adds `-Wl,-rpath,$ORIGIN`, and list it in
`runtime` next to `gcc_linux_glibc`.

Outputs: `<name>_archive` builds `<name>.<format>` and `<name>_checksum`
builds `<name>.<format>.sha256`. The app is stored with mode 0755 and every
other member with mode 0644, in both formats.

Examples: [Qt packaging](../../qt/README.md#packaging) and
[qt-rust](../../qt/tests/fixtures/qt-rust/app/BUILD.bazel).

# crates_deploy

Stages crate sources as a local vendor tree and publishes them to crates.io.

```starlark
load("@rules_dx//deploy/rules:crates.bzl", "crates_deploy")

crates_deploy(
    name = "crates_demo",
    crate = ":crates_demo_crate",
    version = "0.0.0",
)
```

- `name`: the crate name.
- `crate`: required. The crate files, including `Cargo.toml`.
- `version`: defaults to `0.0.0`.
- `profile`: `debug`, `dev` or `release`. Defaults to `release`.

Default run stages `<name>-vendor/vendor/<name>/` and
`<name>-vendor/registry/<name>/<version>/`. No credentials needed.

Publish with `cargo` on `PATH`:

```
CRATES_PUBLISH_APPROVED=1 CARGO_REGISTRY_TOKEN=... \
dx deploy --apply //pkg:crates_demo
```

A live run refuses version `0.0.0` and `CRATES_ALLOW_DIRTY=1`.

A default run checks the staged `Cargo.toml` and runs `cargo package --list`
when `cargo` is on `PATH`. Without `cargo` it prints a skip line. It never
uploads. Rerunning a published version: `cargo publish` fails with the
registry's message and the run exits 1.

# hex_deploy

Stages an Elixir or Erlang package and publishes it to Hex.

```starlark
load("@rules_dx//deploy/rules:hex.bzl", "hex_deploy")

hex_deploy(
    name = "hex_demo",
    package = ":hex_demo_package",
    tool = "mix",
    version = "1.0.0",
)
```

- `name`: the Hex package name.
- `package`: required. The package files, including `mix.exs` (`rebar.config` for `rebar3`).
- `tool`: `mix` or `rebar3`. Defaults to `mix`.
- `version`: defaults to `0.0.0`. It must equal the `version` in `mix.exs`, or the `vsn` in the `.app.src` for `rebar3`.
- `profile`: `debug`, `dev` or `release`. Defaults to `release`.

Default run stages `<name>-hex/package/`, checks the version, and runs
`mix hex.build` (`rebar3 hex build`) when the tool is on `PATH`. Without it the
run prints a skip line. `mix` also writes `<name>-hex/<name>-<version>.tar`.
No credentials needed, nothing is uploaded.

Publish with `mix` or `rebar3` on `PATH`:

```
HEX_PUBLISH_APPROVED=1 HEX_API_KEY=... \
dx deploy --apply //pkg:hex_demo
```

A live run refuses version `0.0.0`. It runs `mix hex.publish --yes`
(`rebar3 hex publish --yes`) in a staged copy with an empty `HOME` and only
`HEX_API_KEY` as the credential. `ERL_LIBS`, `ERL_ROOTDIR`, `HEX_HOME` and
`MIX_HOME` pass through, so point them at the pinned OTP.

# rubygems_deploy

Builds a gem from its sources and pushes it to rubygems.org.

```starlark
load("@rules_dx//deploy/rules:rubygems.bzl", "rubygems_deploy")

rubygems_deploy(
    name = "rubygems_demo",
    gem = ":rubygems_demo_gem",
    version = "1.0.0",
)
```

- `name`: the gem name. The gemspec must be `<name>.gemspec`.
- `gem`: required. The gemspec and the files it packs, such as a
  `ruby_library`.
- `version`: defaults to `0.0.0`. Must equal `spec.version` in the gemspec.
- `ruby`: optional pinned `ruby` executable, such as `@ruby//:ruby_file`. The
  `gem` next to it is used instead of the one on `PATH`.
- `ruby_files`: the files that ruby needs, such as `@ruby//:files`.
- `profile`: `debug`, `dev` or `release`. Defaults to `release`.

Default run needs `gem` on `PATH` unless `ruby` is set. It stages the sources, runs
`gem build`, checks that the gemspec name and version match the target and
that the gem packs exactly the sources (the gemspec itself excluded), then
writes `<name>-<version>.gem` to the output directory. It never pushes. No
credentials needed.

Push with `gem` on `PATH` or from `ruby`:

```
RUBYGEMS_PUBLISH_APPROVED=1 RUBYGEMS_API_TOKEN=... \
dx deploy --apply //pkg:rubygems_demo
```

`RUBYGEMS_API_TOKEN` is sent to `gem push` as `GEM_HOST_API_KEY`. A live run
refuses version `0.0.0`. Rerunning a pushed version: rubygems.org refuses
it, the run prints the last `gem` output lines and exits 1.

# github_deploy

Stages pinned files for a draft GitHub Release and creates the draft.

```starlark
load("@rules_dx//deploy/rules:github.bzl", "github_deploy")

github_deploy(
    name = "github_demo",
    artifacts = [
        ":release_demo_archive",
        ":release_demo_checksum",
    ],
)
```

- `artifacts`: files to attach. An empty list creates a draft with release notes only. Asset names must be unique.
- `tag`: defaults to `v0.0.0-dryrun`.
- `profile`: `debug`, `dev` or `release`. Defaults to `release`.
- `tags`: Bazel tags for the generated targets. Defaults to `[]`.
- `api_compat`: `off` (default), `warn` or `fail`. With `go_modules` (workspace-relative module directories; empty means the `go.mod` at the root), the dry run
  checks each module with `apidiff` against the highest earlier `v*` tag and, on `fail`, refuses breaking changes
  unless the bump is major. It needs `go` on `PATH`; a missing `go`, `apidiff` or earlier tag skips the check.

Default run stages `<name>-release/` with the assets (none for a notes-only draft) and a `would-run.txt`
holding their sha256 and the `gh release create` command. Nothing is
published.

The run also prints each asset's size in the latest published release, read with `gh release view`,
so a budget can be set from it. Without `gh` or network, it prints why that release is unavailable and still succeeds.

Create the draft with `gh` on `PATH` and logged in:

```
GH_RELEASE_APPROVED=1 dx deploy --apply //pkg:github_demo
```

This runs `gh release create <tag> <assets> --draft --verify-tag`, so the
tag must already exist on GitHub. A live run refuses `v0.0.0-dryrun`.

# homebrew_tap_deploy

Renders a Homebrew formula from a `github_deploy` release and pushes it to a tap repository.

```starlark
load("@rules_dx//deploy/rules:homebrew.bzl", "homebrew_tap_deploy")

homebrew_tap_deploy(
    name = "tool_tap",
    release = ":github_demo",
    repo = "org/tool",
    tap = "org/homebrew-tap",
    formula = "tool",
)
```

- `release`: the `github_deploy` target. Its tag gives the version; its `.tar.gz`, `.tgz` and `.zip` assets become the formula urls.
- `repo`: GitHub `owner/repository` that hosts the release.
- `tap`: GitHub `owner/repository` of the tap.
- `formula`: formula name. Defaults to `name`.
- `binary`: file at the top of each archive, installed into `bin`. Defaults to `formula`.
- `desc`, `homepage`: formula fields. Default to the formula name and `https://github.com/<repo>`.

An asset name must say its platform: `darwin`, `macos` or `apple`, or `linux`; and `arm64` or `aarch64`, or `x86_64`, `amd64` or `x64`. Other assets are ignored. The sha256 comes from the asset file and must match a sibling `<asset>.sha256` when the release has one.

Default run writes `<name>-tap/Formula/<formula>.rb` and `would-run.txt` into the output tree and runs `ruby -c` on the formula when `ruby` is on `PATH`. Nothing is pushed.

Push the formula with `gh` on `PATH`:

```
HOMEBREW_TAP_PUBLISH_APPROVED=1 HOMEBREW_TAP_API_TOKEN=... \
dx deploy --apply //pkg:tool_tap
```

This commits `Formula/<formula>.rb` to the tap's default branch through the contents API. The token needs write access to the tap. A live run refuses placeholder tags such as `v0.0.0-dryrun`.

# linux_package_deploy

Builds `.deb`, `.rpm` and `.apk` packages from one binary with a pinned
`nfpm` and attaches them to a draft GitHub Release through `github_deploy`.

```starlark
load("@rules_dx//deploy/rules:linux_package.bzl", "linux_package_deploy")

linux_package_deploy(
    name = "demo",
    binary = ":demo_bin",
    extra_files = {":demo_conf": "/etc/demo/demo.conf"},
    formats = ["deb", "rpm"],
    maintainer = "Demo <demo@example.com>",
    version = "1.2.3",
)
```

- `binary`: executable installed at `install_path`, default `/usr/bin/<name>`.
- `formats`: any of `deb`, `rpm`, `apk`. Defaults to `["deb", "rpm"]`.
- `version`, `arch` (Go names such as `amd64`, `arm64`), `maintainer`, `description`: package metadata.
- `extra_files`: more files, mapped to absolute install paths, installed as `0644`.
- `tag`, `profile`, `tags`: as for `github_deploy`.

`bazel build //pkg:demo_deb` writes `demo_<version>_<arch>.deb`; each format
has its own `<name>_<format>` target. `dx deploy //pkg:demo` stages the
packages like `github_deploy`, and the live run (`GH_RELEASE_APPROVED=1 dx deploy
--apply`) attaches them to the draft release. `nfpm` runs on
Linux x86_64, Linux arm64 and macOS arm64 hosts.

# pub_deploy

Stages a Dart or Flutter package and publishes it to pub.dev.

```starlark
load("@rules_dx//deploy/rules:pub.bzl", "pub_deploy")

pub_deploy(
    name = "pub_demo",
    package = ":pub_demo_package",
    version = "0.0.0",
)
```

- `name`: the package name, lowercase letters, digits and `_`.
- `package`: required. The package files, including `pubspec.yaml` and
  `CHANGELOG.md`.
- `version`: defaults to `0.0.0`. Must equal the `pubspec.yaml` version and
  have a heading in `CHANGELOG.md`.
- `hosted_url`: defaults to `https://pub.dev`.
- `profile`: `debug`, `dev` or `release`. Defaults to `release`.

Default run checks `pubspec.yaml` and `CHANGELOG.md`, runs
`dart pub publish --dry-run` on a copy, and stages `<name>-package/`. A package
with a top-level `flutter:` section uses `flutter` instead of `dart`. Without
the tool on `PATH` the run prints a skip line. No credentials needed.

Publish with `dart` (or `flutter`) on `PATH`:

```
PUB_PUBLISH_APPROVED=1 PUB_TOKEN=... \
dx deploy --apply //pkg:pub_demo
```

A live run refuses version `0.0.0`, repeats the dry run, then runs
`pub publish --force`. The token reaches `dart pub token add --env-var` and
lands only in a temporary `PUB_CACHE` that is removed afterwards.

# nuget_deploy

Stages one nupkg as a local folder feed and pushes it to a NuGet source.

```starlark
load("@rules_dx//deploy/rules:nuget.bzl", "nuget_deploy")

nuget_deploy(
    name = "nuget_demo",
    nupkg = ":nuget_demo_nupkg",
)
```

- `nupkg`: required. One `.nupkg` file.
- `package_id`: optional. Must match the nuspec id.
- `version`: optional. Must match the nuspec version.
- `source`: defaults to `https://api.nuget.org/v3/index.json`.
- `profile`: `debug`, `dev` or `release`. Defaults to `release`.

Default run stages `<name>-feed/` with the nupkg. No credentials needed.

Push with `dotnet` on `PATH`:

```
NUGET_PUBLISH_APPROVED=1 NUGET_API_KEY=... \
dx deploy --apply //pkg:nuget_demo
```

This runs `dotnet nuget push --skip-duplicate`. The key goes to `dotnet`
through the `NUGET_API_KEY` env var, never the command line, so NuGet 7.6+
(.NET SDK 10.0.300+) is required; older versions are refused. A live run refuses nuspec
version `0.0.0`.

A default run checks the nuspec id and version against `package_id` and
`version`. It does not run `dotnet`. Rerunning a published version: the push is
skipped, the run prints `nuget_deploy: skipped <id> <version>: already on
<source>` and exits 0.

# hackage_deploy

Stages one cabal sdist locally and uploads it to Hackage.

```starlark
load("@rules_dx//deploy/rules:hackage.bzl", "hackage_deploy")

hackage_deploy(
    name = "hackage-demo",
    sdist = ":hackage_demo_sdist",
)
```

- `sdist`: required. One `<package>-<version>.tar.gz` made by `cabal sdist`.
- `package`: optional. Must match the `name` in the `.cabal` file.
- `version`: optional. Must match the `version` in the `.cabal` file.
- `profile`: `debug`, `dev` or `release`. Defaults to `release`.

Every run checks that the sdist holds `<package>-<version>/<package>.cabal`
and that the file declares the name and version in the sdist file name. A
default run stages `<name>-sdist/` with the sdist. No credentials needed.

Upload with `cabal` on `PATH`:

```
HACKAGE_PUBLISH_APPROVED=1 HACKAGE_API_TOKEN=... \
dx deploy --apply //pkg:hackage-demo
```

This runs `cabal upload --publish --token=...`, so the token is visible in the
`cabal` command line while it runs. A live run refuses version `0.0.0`.

# luarocks_deploy

Stages one rockspec locally and uploads it to LuaRocks.

```starlark
load("@rules_dx//deploy/rules:luarocks.bzl", "luarocks_deploy")

luarocks_deploy(
    name = "luarocks_demo",
    rockspec = ":luarocks_demo_rockspec",
)
```

- `rockspec`: required. One `<package>-<version>.rockspec`.
- `package`: optional. Must match the rockspec `package`.
- `version`: optional. Must match the rockspec `version`, with or without the
  `-<revision>` suffix.
- `profile`: `debug`, `dev` or `release`. Defaults to `release`.

Every run checks that the rockspec file name is `<package>-<version>.rockspec`
for the package and version it declares. A default run stages `<name>-rock/`
with the rockspec; it does not run `luarocks`. No credentials needed.

Upload with `luarocks` on `PATH`:

```
LUAROCKS_PUBLISH_APPROVED=1 LUAROCKS_API_KEY=... \
dx deploy --apply //pkg:luarocks_demo
```

This runs `luarocks upload <rockspec> --api-key=...`, which packs the source
rock and uploads it, so the key is visible in the `luarocks` command line while
it runs. A live run refuses upstream version `0.0.0`.

# oci_image_from_binary

Builds an OCI image from one binary, with no Dockerfile.

```starlark
load("@rules_dx//deploy/rules:image.bzl", "oci_image_from_binary")
load("@rules_dx//deploy/rules:oci.bzl", "oci_deploy")

oci_image_from_binary(
    name = "image",
    binary = ":hello",
)

oci_deploy(
    name = "publish",
    image_tar = ":image",
    platform = "linux/amd64",
)
```

- `binary`: required. A `rust_binary`, `go_binary`, `cc_binary` or zig binary.
  The binary and its runfiles go in one layer under `/app`, and the
  entrypoint is `/app/<binary name>`.
- `base`: `static` (default) or `cc`, the pinned distroless images from the
  `containers` family.
- `main`: the entry script of a `python` or `node` base. It defaults to the
  binary's package and name with `.py` or `.js`, so `//app:server` runs
  `app/server.py`.
- `main_class`: the class of a `java` base. It defaults to the `main_class` of
  the `java_binary`, written to an argument file next to the jars.
- `runtime`: for the `python`, `java`, `node` and `dotnet` bases. Replaces the
  base interpreter with a layer from the managed toolchain, installed under
  `/opt/<base>`, on the distroless `cc` image. The dotnet runtime runs with
  invariant globalization.
- `platforms`: defaults to `["linux/amd64"]`. One platform gives an
  `oci_image` and an `<name>_load` target that `dx run` loads into a local
  runtime. Several platforms give an `oci_image_index` that builds the binary
  once per platform; `oci_deploy` takes one `platform` of it, and
  `<name>_load` loads the image of the first platform.

The `oci_deploy` dry run prints the image size in bytes, and the `size_budget`
when one is set.

# oci_deploy

Stages one image as a local OCI layout and pushes it to a registry.

```starlark
load("@rules_dx//deploy/rules:oci.bzl", "oci_deploy")

oci_deploy(
    name = "oci_demo",
    image_tar = ":oci_demo_tar",
    platform = "linux/amd64",
)
```

- `image_tar`: required. A `docker save` or OCI layout archive, `.tar` or
  `.tar.gz`, or an OCI layout directory such as a rules_oci `oci_image`,
  holding one image.
- `platform`: required. `linux/amd64` or `linux/arm64`.
- `tag`: defaults to `latest`. At most 128 chars, not starting with `.` or `-`.
- `registry`: defaults to `ghcr.io`.
- `repository`: lowercase OCI path segments. Defaults to `name`, so a
  mixed-case target needs `repository` set.
- `profile`: `debug`, `dev` or `release`. Defaults to `release`.
- `size_budget`: optional, such as `500MB`. A decimal `B`, `KB`, `MB` or `GB`
  amount. The run fails before staging or pushing when the image (the
  archive file, or the layout directory tree) is larger, and prints both
  sizes in bytes. Empty means no budget.

Default run stages `<repository>-oci-layout/` with an `index.json`. No
credentials needed.

Push:

```
OCI_PUBLISH_APPROVED=1 \
OCI_REGISTRY_USER=... OCI_REGISTRY_TOKEN=... \
dx deploy --apply //pkg:oci_demo
```

A live run stages the same layout into the output dir and uploads it over
HTTPS with the OCI Distribution API. No docker needed. It prints
`oci_deploy: pushed <registry>/<repository>:<tag>@sha256:...`, the digest of
the staged manifest. Layers go up as staged, so uncompressed layers stay
uncompressed.

- Auth: `OCI_REGISTRY_USER` and `OCI_REGISTRY_TOKEN`, sent as a Basic login or
  exchanged for a bearer token, as the registry asks.
- `registry` may carry a path prefix, such as `ghcr.io/example`.
- `docker.io` pushes to `registry-1.docker.io`; a single-segment repository
  gets `library/`.
- Plain-HTTP registries are not supported.

A live run refuses tags `0.0.0` and `0.0.0-dryrun`.

# octopus_deploy

Stages one package for Octopus Deploy and pushes it to an Octopus server.

```starlark
load("@rules_dx//deploy/rules:octopus.bzl", "octopus_deploy")

octopus_deploy(
    name = "octopus_demo",
    channel = "Default",
    deploy_to = ["Production"],
    package = ":release_demo_archive",
    project = "octopus_demo",
    version = "0.0.0",
)
```

- `package`: required. One `.tar.gz`, such as an `archive_deploy`
  `<name>_archive`.
- `project`: required. The Octopus project.
- `channel`: defaults to `Default`.
- `version`: defaults to `0.0.0`.
- `deploy_to`: environments for the release command. Defaults to `[]`.
- `space`: defaults to empty, the default space. `OCTOPUS_SPACE` overrides it.
- `octopus_url`: an https URL. Defaults to `https://octopus.example.invalid`.
  `OCTOPUS_URL` overrides it.
- `profile`: `debug`, `dev` or `release`. Defaults to `release`.

Default run stages `<name>-drop/` with the package and a `would-run.txt`
holding the `octo push` and `octo create-release` commands. No credentials
needed.

Push with `octo` on `PATH`:

```
OCTOPUS_PUBLISH_APPROVED=1 OCTOPUS_API_KEY=... \
OCTOPUS_URL=https://octopus.example.com \
dx deploy --apply //pkg:octopus_demo
```

This runs `octo push --package`, then `octo create-release` with a
`--deployTo` for each `deploy_to`. Deployments are queued, not awaited.

A default run checks the package and the spec and runs no `octo` command.
Rerunning a published version: `octo push` fails because the package exists,
the run exits 1 and the hint names `FailIfExists`. That includes a rerun after
a failed `create-release`. The key goes to `octo` as `OCTOPUS_CLI_API_KEY` in its
env, never the command line. A live run refuses version `0.0.0` and the default
`octopus_url`.

# promotion_deploy

Records the promotion of one pinned artifact from one environment to another.

```starlark
load("@rules_dx//deploy/rules:promotion.bzl", "promotion_deploy")

promotion_deploy(
    name = "promotion_demo",
    artifact = ":release_demo_archive",
    from_environment = "staging",
    to_environment = "production",
    version = "0.0.0",
)
```

- `artifact`: required. One `.tar.gz`, `.tar`, `.tgz`, `.whl`, `.jar`,
  `.nupkg` or `.zip` file.
- `from_environment`: defaults to `staging`.
- `to_environment`: defaults to `production`. Must differ from
  `from_environment`.
- `version`: defaults to `0.0.0`.
- `profile`: `debug`, `dev` or `release`. Defaults to `release`.

Default run stages `<name>-promotion/` with the artifact, a
`promotion.json` and a `would-run.txt`. No credentials needed. Health and
registry gates do not run.

`PROMOTION_SECRET_REFS` is a comma list of `ENV_NAME`, `file:<path>` or
`cmd:<tool>` refs. Only the names are recorded.

Promote:

```
PROMOTION_APPROVED=1 dx deploy --apply //pkg:promotion_demo
```

A live run refuses version `0.0.0` and checks the approval, the registry env,
that every secret ref exists and the health command. It records the health
result (`passed` or `not-required`) in `promotion.json`. It then exits `1`,
because publishing is not implemented yet. Nothing is pushed.

- `PROMOTION_REGISTRY_REQUIRED=1` needs `PROMOTION_REGISTRY_USER` and
  `PROMOTION_REGISTRY_TOKEN`.
- `PROMOTION_REQUIRE_HEALTH=1` needs `PROMOTION_HEALTH_CMD`. The promotion is
  refused unless it exits `0`.

Roll back:

```
PROMOTION_ROLLBACK=1 PROMOTION_ROLLBACK_TO=<version> \
dx deploy --apply //pkg:promotion_demo
```

This writes `rollback.txt` in the promotion directory.
`PROMOTION_ROLLBACK_SHA` adds the expected sha256.

# pypi_deploy

Stages one wheel and an optional sdist as a local PEP 503 index. A live run
uploads them to PyPI with `twine`.

```starlark
load("@rules_dx//deploy/rules:pypi.bzl", "pypi_deploy")

pypi_deploy(
    name = "pypi_demo",
    sdist = ":pypi_demo_sdist",
    wheel = ":pypi_demo_wheel",
)
```

Attributes:

- `name`: the distribution name. Letters, digits, `.`, `_` and `-` only. It
  must start and end with a letter or digit. It names the wheelhouse
  directory.
- `wheel`: required. One `.whl` file named
  `name-version-python-abi-platform.whl`. An optional build tag adds a sixth
  field.
- `sdist`: optional. One `.tar.gz` file named `name-version.tar.gz`. Its
  project and version must match the wheel's.
- `repository_url`: must start with `https://`. Defaults to
  `https://upload.pypi.org/legacy/`.
- `profile`: `debug`, `dev` or `release`. Defaults to `release`.

Project names are PEP 503 normalized: lowercased, with each run of `.`, `_`
or `-` replaced by one `-`.

A default run writes into the output directory (see Common behavior):

```
<name>-wheelhouse/
  <wheel file>
  <sdist file>          only when sdist is set
  simple-index/
    <project>/
      index.html
```

`index.html` has one link per file, sorted by file name. Each link points to
`../../<file>#sha256=<digest>`. The run prints:

```
pypi_deploy: staged <name> (profile: <profile>)
  wheelhouse: <path>/<name>-wheelhouse
  index: <path>/<name>-wheelhouse/simple-index/<project>/index.html
```

A dry run needs no credentials. `dx deploy --apply` publishes and needs:

- `PYPI_PUBLISH_APPROVED=1`
- `PYPI_API_TOKEN`: sent to `twine` as the password, with username `__token__`.
  It never goes on the command line.

`twine` must be on `PATH`. The run calls `twine upload --non-interactive
--repository-url <repository_url>` with the wheel and sdist files. It does
not write `<name>-wheelhouse/` and does not check file names. On success it
prints:

```
pypi_deploy: uploaded <wheel file> to <repository_url>
```

A default run checks the wheel and sdist file names and that their project and
version match. It does not run `twine`. A live run first runs `twine check
--strict`. Rerunning a published version: PyPI refuses the file, the run
prints `pypi_deploy: <file> version <version> is already on <url>; upload a new
version` and exits 1.

`twine` runs with a cleared environment. Only `REQUESTS_CA_BUNDLE` and the
keep-list in Common behavior pass through.

```
PYPI_PUBLISH_APPROVED=1 PYPI_API_TOKEN=... \
dx deploy --apply //pkg:pypi_demo
```

Exit codes:

- `0`: the wheelhouse is staged, or the upload succeeds.
- `1`: invalid file names or mismatched sdist, a refused live run, or a
  failed `twine` upload.

# helm_chart_deploy

Packages a `helm_chart` with the pinned `helm` and pushes it to an OCI registry.

```starlark
load("@rules_dx//deploy/rules:helm.bzl", "helm_chart_deploy")

helm_chart_deploy(
    name = "helm_demo",
    chart = ":chart",
    registry = "ghcr.io/acme/charts",
    version = "1.2.3",
)
```

- `chart`: a `helm_chart` target. Its files are copied to a staging directory.
- `version`: must equal `version` in `Chart.yaml`, or the run fails.
- `registry`: host plus optional path. `repository` adds a path below it.
- `helm`: defaults to the pinned helm from `//config:tool_helm`.

A dry run runs `helm lint`, then `helm package`, and prints the `.tgz` path.
A live run (`dx deploy --apply`) uses the OCI credentials:
`OCI_PUBLISH_APPROVED=1`, `OCI_REGISTRY_USER` and `OCI_REGISTRY_TOKEN`. It logs
in with `helm registry login` into a throwaway registry config, runs
`helm push <chart>.tgz oci://<registry>/<repository>`, and removes the config.
Placeholder versions (`0.0.0`) are refused. `helm` runs with a cleared
environment.

Exit codes:

- `0`: the chart is packaged, or pushed.
- `1`: a version mismatch, a lint or package failure, a refused live run, or a
  failed push.

# dx_split_debug_info

Splits the debug info out of a Linux `cc_binary`.

```
load("@rules_dx//deploy/rules:debug_info.bzl", "dx_split_debug_info")

dx_split_debug_info(
    name = "app_split",
    binary = ":app",
)
```

Build it to get the stripped binary. The debug file and the manifest are in the
`symbols` output group.

```
bazel build --output_groups=symbols //pkg:app_split
```

This writes `app_split.debug` and `app_split.symbols.json`. The manifest has
the build-id, arch, sha256 of the stripped binary, debug file name and
`llvm-objcopy` version. The target is Linux only.

Compile with `-g` and do not strip at link time. Look up an address with
`llvm-symbolizer`:

```
llvm-symbolizer --obj=app_split.debug 0x401136
```

The build fails if the stripped binary and the debug file differ in build-id
or arch.

# dx_dsym

Pairs the dSYM of a macOS `cc_binary` with that binary.

```
load("@rules_dx//deploy/rules:debug_info.bzl", "dx_dsym")

dx_dsym(
    name = "app_dsym",
    binary = ":app",
)
```

The `binary` is built again with `apple_generate_dsym` set, through a
transition, so the link writes the dSYM bundle. Build the `symbols` output
group to get the bundle and the manifest:

```
bazel build --output_groups=symbols //pkg:app_dsym
```

This writes `app_dsym.dSYM` and `app_dsym.dsym.json`. The bundle keeps its
DWARF file under `Contents/Resources/DWARF/`. The manifest has the Mach-O UUID,
arch, sha256 of the binary, DWARF path and `dsymutil` version. The build fails
if the bundle has no DWARF file, or if its UUID or arch differ from the binary.
The target is macOS only.

Look up an address in the bundle with `llvm-symbolizer`:

```
llvm-symbolizer --obj=app_dsym.dSYM/Contents/Resources/DWARF/app 0x100003f40
```

# dx_pdb

Pairs the PDB of a Windows `cc_binary` with that binary.

```
load("@rules_dx//deploy/rules:debug_info.bzl", "dx_pdb")

dx_pdb(
    name = "app_pdb",
    binary = ":app",
)
```

The `binary` is built again with the `generate_pdb_file` feature added,
through a transition, so the link writes the PDB. Build the `symbols` output
group to get the PDB and the manifest:

```
bazel build --output_groups=symbols //pkg:app_pdb
```

This writes the PDB the link wrote, such as `app.pdb`, and `app_pdb.pdb.json`. The manifest has the
CodeView GUID and age, arch, sha256 of the binary, PDB name, CodeView path and
linker version. The build fails if the PDB is not a PDB, or if its GUID, age or
machine differ from the binary. The target is Windows only.

Look up an address (a relative virtual address) with `llvm-symbolizer`:

```
llvm-symbolizer --obj=app.exe --pdb=app.pdb --relative-address 0x1000
```

# Registries released by tag

These registries have no deploy rule. The release is a git tag, a pull
request, or an upload, so the last step is manual:

- Go: push the tag `vX.Y.Z`. The module proxy fetches it from the repository.
  For a GitHub release, set `tag` on `github_deploy` to that tag.
- Julia: push the tag, then comment `@JuliaRegistrator register` on the commit.
- Swift: push a semver tag such as `1.2.0`. SwiftPM reads versions from tags.
- PHP: run `composer validate`, then push a tag. Packagist reads tags from the
  repository.
- opam: open a pull request to the opam repository that adds
  `packages/<name>/<name>.<version>/opam`.
- CPAN: upload the tarball to PAUSE.

# Common behavior

Except for `cloudflare_pages_deploy`, a default run writes into the
workspace root, or into the directory given as the first argument. A live
run without its credentials or its `*_APPROVED=1` variable is refused. Exit
codes: `0` staged or published, `1` any failure or refusal.

Every deploy target, `npm_deploy` included, rejects arguments beyond that output
directory. Deploy targets run through `bazel run` or `dx deploy`, which set
`DX_DEPLOY_CONFIG`. Publisher tools (`twine`, `dotnet`, `cargo`, `gh`, `octo`,
`npm`) run with a cleared environment plus a per-tool keep-list. Secrets reach
them only through that environment, stdin or a temp file, never
argv. On Unix the temp file is mode 0600. On Windows it relies on the user
temp directory's access list. The exceptions are the promotion `PROMOTION_HEALTH_CMD` and `wrangler`,
which has its own narrower list. The `npm`, `twine`, `dotnet` and `octo`
output has the token replaced with `[redacted]` and appears when the tool exits.
