# Deploy rules

Use these rules to publish what you build to a package registry, a container
registry, a GitHub release or a local archive.

## Quick start

A deploy rule wraps a target you already build. A target is one thing Bazel
builds. A label names it: `//app:tasks` is the target `tasks` in the `app`
directory. Add the rule to `app/BUILD.bazel`, the file that declares `tasks`:

```starlark
load("@rules_dx//deploy:defs.bzl", "archive_deploy")

archive_deploy(
    name = "tasks_deploy",
    app = ":tasks",
)
```

Check what it would publish. Nothing leaves your machine:

```sh
dx deploy //app:tasks_deploy
```

Publish it:

```sh
dx deploy --apply //app:tasks_deploy
```

Registry rules also need credentials and an `<X>_APPROVED=1` variable in your
shell. Each rule's section names them. The [`dx deploy`](../docs/cli/commands/deploy.md)
page lists every flag.

## Rules

| Publishes to | Rule |
| --- | --- |
| Maven | [`maven_deploy`](#maven_deploy) |
| npm | [`npm_deploy`](#npm_deploy) |
| Cloudflare Pages | [`cloudflare_pages_deploy`](#cloudflare_pages_deploy) |
| Local archive | [`archive_deploy`](#archive_deploy), [`consumer_archive`](#consumer_archive) |
| crates.io | [`crates_deploy`](#crates_deploy) |
| Hex | [`hex_deploy`](#hex_deploy) |
| RubyGems | [`rubygems_deploy`](#rubygems_deploy) |
| GitHub releases | [`github_deploy`](#github_deploy) |
| Homebrew tap | [`homebrew_tap_deploy`](#homebrew_tap_deploy) |
| Linux packages | [`linux_package_deploy`](#linux_package_deploy) |
| pub.dev | [`pub_deploy`](#pub_deploy) |
| NuGet | [`nuget_deploy`](#nuget_deploy) |
| CPAN | [`cpan_deploy`](#cpan_deploy) |
| Conan | [`conan_deploy`](#conan_deploy) |
| Hackage | [`hackage_deploy`](#hackage_deploy) |
| LuaRocks | [`luarocks_deploy`](#luarocks_deploy) |
| Container registry | [`oci_image_from_binary`](#oci_image_from_binary), [`oci_deploy`](#oci_deploy) |
| Octopus Deploy | [`octopus_deploy`](#octopus_deploy) |
| PyPI | [`pypi_deploy`](#pypi_deploy) |
| Helm chart repository | [`helm_chart_deploy`](#helm_chart_deploy) |
| opam | [`opam_deploy`](#opam_deploy) |
| Packagist | [`packagist_deploy`](#packagist_deploy) |

Debug symbols: [`dx_split_debug_info`](#dx_split_debug_info),
[`dx_dsym`](#dx_dsym) and [`dx_pdb`](#dx_pdb).

## Dry run and publish

`dx deploy <label>` builds the target and runs a dry run. The dry run
stages, checks and prints what it would publish. It contacts no registry.

`dx deploy --apply <label>` publishes. Each rule still needs its own
`<X>_APPROVED=1` variable and credentials. dx never sets them: you set them in
your shell, and CI sets them in `reusable-release.yml`. In an agent session
`--apply` is refused unless `CI=true`. A plain `bazel run` of a deploy target
publishes only with `DX_DEPLOY_LIVE=1`.

## Attributes every rule takes

`size_budget` is optional, such as `25MB`. Use a decimal `B`, `KB`, `MB` or
`GB` amount. Every `*_deploy` macro takes it except `homebrew_tap_deploy` and
`linux_package_deploy`. The build fails when the published files together are
larger, and prints both sizes in bytes. Empty means no budget.

`sbom` is optional. It is a `dx_sbom` target, and every `*_deploy` macro takes
it. `dx deploy` prints `SBOM of <label>: <file> <n> packages`.

Every `*_deploy` macro ends with the same four attributes: `tags`,
`visibility`, `testonly` and `target_compatible_with`. Each one is passed on
to the `dx_deployment` target and to every helper target the macro makes, such
as the `_config`, `_pack` and archive targets.


## maven_deploy

Use `maven_deploy` to publish one jar and its pom to Maven Central.

```starlark
load("@rules_dx//deploy:defs.bzl", "maven_deploy")

maven_deploy(
    name = "maven_demo",
    group = "com.example",
    jar = ":maven_demo_lib",
    pom = "maven_demo-0.0.0.pom",
)
```

- `jar`, `pom`: required. The files to upload.
- `group`: required. The Maven group id.
- `artifact`: defaults to `name`.
- `sources`, `javadoc`: optional. One `.jar` each, staged as
  `<artifact>-<version>-sources.jar` and `-javadoc.jar`. Central needs both,
  so a live upload refuses before any network call when one is missing. A
  dry run works without them and prints `sources: yes|no` and
  `javadoc: yes|no`. Central accepts a jar that holds only a README.
- `version`: empty by default, which publishes the version in the pom. When
  you set it, it must equal the pom's `<version>`.
- `build_mode`: `debug`, `dev` or `release`. Defaults to `release`.
- `api_compat`, `api_baseline`: see [Public API compatibility](#public-api-compatibility).

The pom must match `group`, `artifact` and `version`. `groupId` may come
from `parent`. `packaging` must be absent or `jar`. The pom also needs
`name`, `description`, `url`, `licenses/license/name`,
`developers/developer`, `scm/url`, `scm/connection` and
`scm/developerConnection`. The error names every missing field.

Check it first. This needs no credentials and does not contact Central:

```sh
dx deploy //pkg:maven_demo
```

The dry run checks the pom and stages a local Maven repository under
`<artifact>-repo/`. Next to the jar and the pom it writes:

- `.md5`, `.sha1`, `.sha256` and `.sha512` files with bare lowercase hex.
- `.asc` signatures when `MAVEN_GPG_KEY` is set.
- `maven-metadata.xml` one level up.

The output ends with `signed: yes` or `signed: no`.

To sign, set these in your shell:

- `MAVEN_GPG_KEY`: an ASCII-armored OpenPGP secret key. Ed25519 is
  recommended. dx uses the signing subkey when the key has one.
- `MAVEN_GPG_PASSPHRASE`: unlocks `MAVEN_GPG_KEY`.

Before you publish, you need:

- A Central Portal account with a verified namespace.
- A Portal user token for `MAVEN_USERNAME` and `MAVEN_PASSWORD`.

Publish:

```sh
MAVEN_PUBLISH_APPROVED=1 \
MAVEN_USERNAME=... MAVEN_PASSWORD=... \
MAVEN_GPG_KEY=... MAVEN_GPG_PASSPHRASE=... \
dx deploy --apply //pkg:maven_demo
```

This builds the same signed repository, zips it without
`maven-metadata.xml`, and uploads it to the Central Portal Publisher API at
`https://central.sonatype.com/api/v1/publisher/` with
`publishingType=USER_MANAGED`. The output names the deployment id. The
deployment stays a draft until you validate and publish it in the Central
Portal. The Portal rejects unsigned bundles. It also rejects a version that
is already published, after the deployment id is printed.

A live run refuses, with exit code 1, when:

- The version is `0.0.0` or ends in `-SNAPSHOT`.
- `MAVEN_USERNAME` or `MAVEN_PASSWORD` is missing.
- `MAVEN_PUBLISH_APPROVED=1` is missing.
- `MAVEN_GPG_KEY` is missing and `MAVEN_ALLOW_UNSIGNED=1` is not set.
- `MAVEN_GPG_PASSPHRASE` is set without `MAVEN_GPG_KEY`.
- `MAVEN_GPG_KEY` is not an armored secret key, or `MAVEN_GPG_PASSPHRASE`
  does not unlock it.
- The Portal rejects the credentials (HTTP 401 or 403).
- The pom is missing Central fields or does not match the coordinates.

## npm_deploy

Use `npm_deploy` to pack files into an npm tarball and publish it to an npm
registry.

```starlark
load("@rules_dx//deploy:defs.bzl", "npm_deploy")

npm_deploy(
    name = "npm_demo",
    srcs = ["npm_demo/package.json"],
    package = "npm-demo",
)
```

- `package`: the npm package name, `name` or `@scope/name`. Defaults to `name`.
- `srcs`: files to pack. They must include `package.json`. Defaults to `[]`.
- `dist_tag`: the dist-tag. Defaults to `latest`.
- `registry_url`: defaults to `https://registry.npmjs.org`.
- `access`: `public` or `restricted`. Defaults to `public`. `restricted`
  needs a scoped package.
- `provenance`: `required` or `none`. Defaults to `required` for `public`
  and `none` for `restricted`. `required` needs `access = "public"` and
  `registry.npmjs.org`.
- `build_mode`: `debug`, `dev` or `release`. Defaults to `release`.
- `api_compat`, `api_baseline`: see [Public API compatibility](#public-api-compatibility).

Check it first. This needs no credentials:

```sh
dx deploy //pkg:npm_demo
```

The dry run stages the tarball and a local feed. It does not run `npm` or
contact the registry.

Before you publish, you need:

- `npm` on `PATH`.
- An `NPM_API_TOKEN` from the registry.
- For `provenance = "required"`, a CI OIDC identity
  (`ACTIONS_ID_TOKEN_REQUEST_URL` or `SIGSTORE_ID_TOKEN`).

Publish:

```sh
NPM_PUBLISH_APPROVED=1 NPM_API_TOKEN=... \
dx deploy --apply //pkg:npm_demo
```

If the version is already published, `npm publish` fails and the run exits 1.
`NPM_PUBLISH_DRY_RUN` is refused. Use `dx deploy` without `--apply` instead.

## cloudflare_pages_deploy

Use `cloudflare_pages_deploy` to publish a static site to an existing
Cloudflare Pages project. It uses a pinned Wrangler, the Cloudflare CLI, that
Bazel builds for you.

```starlark
load("@rules_dx//deploy:defs.bzl", "cloudflare_pages_deploy")

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
- `account_id`: required. 32 lowercase hex characters.
- `project`: required. The Pages project name. The project must already
  exist.
- `branch`: required. When it equals `production_branch`, the deploy goes to
  production. Any other branch is a preview.
- `production_branch`: required. Set it to the production branch configured
  on the Pages project.
- `wrangler`: optional. Defaults to the pinned Wrangler in rules_dx with its
  own Node runtime. Set it to your own executable target to override. dx
  never uses `npx` from your machine.
- `build_mode`: `debug`, `dev` or `release`. Defaults to `release`.

The rule does not create projects or set up DNS, Workers or Pages Functions.
A site that contains `_worker.js`, `functions/` or `_routes.json` fails the
check.

Check it first. This needs no token and uploads nothing:

```sh
dx deploy //pkg:site_deploy
```

The dry run builds the site and checks the files, the account, the project
and the branch. It prints:

```text
cloudflare_pages_deploy: checked my-site (branch feature/x, preview, build_mode: release); nothing uploaded
  account: 0123456789abcdef0123456789abcdef
  project: my-site
  branch: feature/x
  intent: preview
  files: 12 (48213 bytes)
  publish: CLOUDFLARE_PUBLISH_APPROVED=1 with CLOUDFLARE_API_TOKEN, then dx deploy --apply
```

Publish:

```sh
CLOUDFLARE_PUBLISH_APPROVED=1 \
CLOUDFLARE_API_TOKEN=... \
dx deploy --apply //pkg:site_deploy
```

The token needs the Cloudflare Pages edit permission. dx reads it only on a
live run and never writes it into the build. Wrangler runs without prompts,
telemetry or project discovery. On success it prints:

```text
cloudflare_pages_deploy: deployed my-site (branch feature/x, preview, build_mode: release)
  account: 0123456789abcdef0123456789abcdef
  project: my-site
  branch: feature/x
  intent: preview
  files: 12 (48213 bytes)
  deployment: <deployment id>
  url: https://<hash>.my-site.pages.dev
```

The run is refused when `CLOUDFLARE_PUBLISH_APPROVED=1` is missing, when
you pass arguments or `--dry-run`, or when `CLOUDFLARE_ACCOUNT_ID` is set to
a value other than `account_id`.

Exit codes: `0` published or checked, `1` any failure or refusal. When
Wrangler fails, the run exits with Wrangler's own code and prints its output
with the token removed.

## archive_deploy

Use `archive_deploy` to package one executable as a tarball plus a sha256
file. It needs no credentials.

```starlark
load("@rules_dx//deploy:defs.bzl", "archive_deploy")

archive_deploy(
    name = "tasks_deploy",
    app = ":tasks",
)
```

- `app`: required. One executable target. It is the only file in the
  tarball.
- `build_mode`: `debug`, `dev` or `release`. Defaults to `release`.

Build the files only:

```sh
bazel build //pkg:tasks_deploy_archive //pkg:tasks_deploy_checksum
```

`<name>_archive` builds `<name>.tar.gz` and `<name>_checksum` builds
`<name>.tar.gz.sha256`.

Copy both into the workspace root, or into one directory you name:

```sh
dx deploy --apply //pkg:tasks_deploy
dx deploy --apply //pkg:tasks_deploy -- /tmp/out
```

The run checks the tarball against its checksum before it copies. More than
one directory argument is refused. Exit codes: `0` released, `1` any failure
or refusal.

For an app built with GCC on Linux, `@rules_dx//cc:gcc_linux_glibc` is an
`os_supplied` runtime that records the pinned glibc floor. The
[`consumer_archive`](#consumer_archive) section explains runtimes.

Example: [shared-rust](../examples/shared-rust/README.md#deploy).

## consumer_archive

Use `consumer_archive` to ship an app to end users as one tar.gz or zip. The
archive holds the app, the files it needs at run time, resources, a `NOTICE`
and an `IDENTITY.txt`, plus a sha256 file. It only builds files. There is no
deploy run.

```starlark
load("@rules_dx//deploy:defs.bzl", "consumer_archive")

consumer_archive(
    name = "ledger_release",
    app = ":ledger",
    min_runtime = "glibc-2.34",
    notice = "NOTICE",
    platform = "@rules_dx//deploy:linux_amd64",
    resources = [":runtime"],
)
```

Build it:

```sh
bazel build //pkg:ledger_release //pkg:ledger_release_checksum
```

`<name>` builds `<name>.<format>` and `<name>_checksum` builds
`<name>.<format>.sha256`. The app has mode 0755 and every other file mode
0644, in both formats.

- `app`: required. One executable target, stored at the archive root.
- `min_runtime`: required. A non-empty requirement such as `glibc-2.34`.
- `notice`: required. Exactly one file, stored as `NOTICE`.
- `license_file`: optional. Exactly one file, stored as `LICENSE`.
- `platform`: a Bazel platform, the label that names an OS and CPU. Its os
  and cpu go into `IDENTITY.txt`. The os is `linux`, `macos` or `windows` and
  the cpu `x86_64` or `aarch64`. Defaults to the platform you build for, so
  it cannot disagree with `target_compatible_with`. The app and runtime are
  not rebuilt for it.
- `linkage`: `static` or `dynamic`. Defaults to `dynamic`.
- `runtime`: files the app needs at run time. Each lands at the archive root
  under its own file name, mode 0644. Native libraries (`.dll`, `.dylib`,
  `.so` or `.so.<n>`) must come through a `dx_native_runtime` target. A
  plain file with such a name fails the build. Defaults to `[]`.
- `format`: `tar.gz` or `zip`. Defaults to `tar.gz`.
- `resources`: targets whose files land under `resources/`. Each file must
  sit under a directory named after its target. Defaults to `[]`.
- `check_imports`: defaults to `True`. See below.
- `tags`: Bazel tags for every target the macro makes.
- `target_compatible_with`: platform constraints. An archive for another OS
  or CPU is skipped instead of built.

`IDENTITY.txt` lists `name`, `arch`, `os`, `linkage` and `min_runtime`.
Then it has one sorted `runtime: <name> <version> <redistribution>` line per
`dx_native_runtime` target in `runtime`.

A `dx_native_runtime` target records where a library comes from and whether
it may ship:

```starlark
load("@rules_dx//deploy:defs.bzl", "dx_native_runtime")

dx_native_runtime(
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
  empty `srcs`. `build_only` fails the build, because compiler and SDK
  files cannot ship.
- `version` and `source`: required and non-empty.
- `notice`: required for `app_local`. The `notice` output group carries it,
  so a `filegroup(output_group = "notice")` can feed a wheel's
  `extra_distinfo_files`.
- `provides`: only for `os_supplied`. Library names the OS supplies for this
  runtime, such as `libGL.so.1`. A trailing `*` matches any name with that
  prefix.

The build also checks which libraries the app and each shipped runtime file
load. Each one (ELF `DT_NEEDED`, Mach-O load command, PE import) must be
shipped in the archive, be a system library for that format, or be listed in
an `os_supplied` runtime's `provides`. The system libraries are the
`SYSTEM_IMPORTS` list in `deploy/private/src/imports.rs`. Otherwise the
build fails with `<member>: not shipped and not a system import: [...]`.
Files that are not binaries are skipped. Set `check_imports = False` to turn
the check off.

To ship the pinned GCC's `libstdc++.so.6` and `libgcc_s.so.1`, use
`@rules_dx//cc:gcc_linux_libstdcxx`. It is an `app_local` runtime with the
GCC Runtime Library Exception notice. Build the app with
`extra_features = ["dynamic_libstdcxx"]` on `cc_toolchain_binary`, which
adds `-Wl,-rpath,$ORIGIN`. List it in `runtime` next to
`@rules_dx//cc:gcc_linux_glibc`.

Examples: [Qt packaging](../qt/README.md#packaging) and
[qt-rust](../qt/tests/fixtures/qt_rust/app/BUILD.bazel).

## crates_deploy

Use `crates_deploy` to stage a crate as a local vendor tree and publish it to
crates.io.

```starlark
load("@rules_dx//deploy:defs.bzl", "crates_deploy")

crates_deploy(
    name = "crates_demo",
    crate = ":crates_demo_crate",
)
```

- `crate`: required. The crate files, including `Cargo.toml`.
- `package`: the crate name. Defaults to `name`.
- `version`: empty by default, which publishes the version in `Cargo.toml`.
  When you set it, it must equal that version.
- `build_mode`: `debug`, `dev` or `release`. Defaults to `release`.
- `api_compat`, `api_baseline`: see [Public API compatibility](#public-api-compatibility).

Check it first. This needs no credentials and uploads nothing:

```sh
dx deploy //pkg:crates_demo
```

The dry run checks the staged `Cargo.toml` and stages
`<name>-vendor/vendor/<name>/` and `<name>-vendor/registry/<name>/<version>/`.
It runs `cargo package --list` when `cargo` is on `PATH`, and prints a skip
line when it is not.

Before you publish, you need:

- `cargo` on `PATH`.
- A crates.io API token for `CRATES_API_TOKEN`.

Publish:

```sh
CRATES_PUBLISH_APPROVED=1 CRATES_API_TOKEN=... \
dx deploy --apply //pkg:crates_demo
```

If the version is already published, `cargo publish` fails with the
registry's message and the run exits 1.

A live run refuses, with exit code 1, when:

- The version is `0.0.0`.
- `CRATES_ALLOW_DIRTY=1` is set.
- `CRATES_API_TOKEN` is missing.
- `CRATES_PUBLISH_APPROVED=1` is missing.

## hex_deploy

Use `hex_deploy` to stage an Elixir or Erlang package and publish it to Hex.

```starlark
load("@rules_dx//deploy:defs.bzl", "hex_deploy")

hex_deploy(
    name = "hex_demo",
    srcs = [":hex_demo_package"],
    tool = "mix",
)
```

- `srcs`: required. The package files, including `mix.exs`, or
  `rebar.config` for `rebar3`.
- `package`: the Hex package name. Defaults to `name`.
- `tool`: `mix` or `rebar3`. Defaults to `mix`.
- `version`: empty by default, which publishes the version in `mix.exs`, or
  the `vsn` in the `.app.src` for `rebar3`. When you set it, it must equal
  that version.
- `build_mode`: `debug`, `dev` or `release`. Defaults to `release`.

Check it first. This needs no credentials and uploads nothing:

```sh
dx deploy //pkg:hex_demo
```

The dry run stages `<name>-hex/package/` and checks the version. It runs
`mix hex.build`, or `rebar3 hex build`, when the tool is on `PATH`, and
prints a skip line when it is not. `mix` also writes
`<name>-hex/<name>-<version>.tar`.

Before you publish, you need:

- `mix` or `rebar3` on `PATH`.
- A Hex API key for `HEX_API_TOKEN`.

Publish:

```sh
HEX_PUBLISH_APPROVED=1 HEX_API_TOKEN=... \
dx deploy --apply //pkg:hex_demo
```

This runs `mix hex.publish --yes`, or `rebar3 hex publish --yes`, in a
staged copy with an empty `HOME`. `HEX_API_TOKEN` is the only credential it
gets. These variables pass through, so point them at the pinned OTP:

- `ERL_LIBS`
- `ERL_ROOTDIR`
- `HEX_HOME`
- `MIX_HOME`

A live run refuses, with exit code 1, when:

- The version is `0.0.0`.
- `HEX_API_TOKEN` is missing.
- `HEX_PUBLISH_APPROVED=1` is missing.

## rubygems_deploy

Use `rubygems_deploy` to build a gem from its sources and push it to
rubygems.org.

```starlark
load("@rules_dx//deploy:defs.bzl", "rubygems_deploy")

rubygems_deploy(
    name = "rubygems_demo",
    gem = ":rubygems_demo_gem",
)
```

- `gem`: required. The gemspec and the files it packs, such as a
  `ruby_library`. The gemspec must be `<gem name>.gemspec`.
- `package`: the gem name. Defaults to `name`.
- `version`: empty by default, which publishes `spec.version` from the
  gemspec. When you set it, it must equal `spec.version`.
- `ruby`: optional. A pinned `ruby` executable, such as
  `@ruby//:ruby_file`. The `gem` next to it is used instead of the one on
  `PATH`.
- `ruby_files`: the files that `ruby` needs, such as `@ruby//:files`.
- `build_mode`: `debug`, `dev` or `release`. Defaults to `release`.

Check it first. This needs no credentials and pushes nothing:

```sh
dx deploy //pkg:rubygems_demo
```

The dry run needs `gem` on `PATH` unless `ruby` is set. It stages the
sources and runs `gem build`. It checks that the gemspec name and version
match the target, and that the gem packs exactly the sources, not counting
the gemspec. It writes `<name>-<version>.gem` to the output directory.

Before you publish, you need:

- `gem` on `PATH`, or `ruby` set.
- A rubygems.org API key for `RUBYGEMS_API_TOKEN`.

Publish:

```sh
RUBYGEMS_PUBLISH_APPROVED=1 RUBYGEMS_API_TOKEN=... \
dx deploy --apply //pkg:rubygems_demo
```

The token reaches `gem push` as `GEM_HOST_API_KEY`. If the version is
already pushed, rubygems.org refuses it. The run prints the last `gem`
output lines and exits 1.

A live run refuses, with exit code 1, when:

- The gemspec version is `0.0.0`.
- `RUBYGEMS_API_TOKEN` is missing.
- `RUBYGEMS_PUBLISH_APPROVED=1` is missing.

## github_deploy

Use `github_deploy` to stage pinned files for a draft GitHub Release and
create the draft.

```starlark
load("@rules_dx//deploy:defs.bzl", "github_deploy")

github_deploy(
    name = "github_demo",
    artifacts = [
        ":release_demo_archive",
        ":release_demo_checksum",
    ],
)
```

- `artifacts`: required. The files to attach. Asset names must be unique.
  An empty list creates a draft with release notes only.
- `release_tag`: the tag. Defaults to `v0.0.0-dryrun`.
- `tag_from_head`: `True` uses the tag that points at HEAD. Defaults to
  `False`. You cannot set both `release_tag` and `tag_from_head`.
- `notes`: `commits` or `none`. Defaults to `commits`.
- `notes_file`: a markdown file used as the release body. It needs
  `notes = "none"`.
- `notes_diff`: workspace paths, such as `["api/"]`. With
  `notes = "commits"` the body ends with their `git diff` since the previous
  tag, or their full content on the first release.
- `sign`: `auto`, `off` or `required`. Defaults to `auto`. A live run signs
  each asset with the pinned cosign and uploads `<asset>.sigstore.json` when
  `COSIGN_KEY` is set or the run has a GitHub OIDC token
  (`ACTIONS_ID_TOKEN_REQUEST_URL`). `required` fails without either.
- `registry_deploys`: `crates_deploy` and `maven_deploy` targets published
  in the same release. The release body gets an `## Attestations` section
  with one `gh attestation verify <file> -R <owner>/<repo>` line for each
  `.crate` and each `maven_deploy` file.
- `attach`: `True` also uploads the `maven_deploy` files as release assets.
  Defaults to `True`.
- `sbom`: also uploaded as a release asset.
- `build_mode`: `debug`, `dev` or `release`. Defaults to `release`.
- `api_compat`, `api_baseline`: see [Public API compatibility](#public-api-compatibility).
- `go_modules`: the workspace-relative module directories that `api_compat`
  checks. Empty means the `go.mod` at the root.

Check it first. This publishes nothing:

```sh
dx deploy //pkg:github_demo
```

The dry run stages `<name>-release/` with the assets, none for a notes-only
draft, and a `would-run.txt` that holds their sha256 and the
`gh release create` command. Its `sign:` line names the identity it would
sign with, or `none`, and it lists each `<asset>.sigstore.json` it would
upload.

It also prints each asset's size in the latest published release, read with
`gh release view`, so you can set a budget from it. Without `gh` or a
network, it prints why that release is unavailable and still succeeds.

Before you publish, you need:

- `gh` on `PATH` and logged in.
- The tag already pushed to GitHub.
- `GITHUB_REPOSITORY` set to `<owner>/<repo>` when `registry_deploys` is
  set.

Publish:

```sh
GITHUB_PUBLISH_APPROVED=1 dx deploy --apply //pkg:github_demo
```

This runs `gh release create <tag> <assets> --draft --verify-tag`.

A live run refuses, with exit code 1, when:

- The tag is `v0.0.0-dryrun` or `v0.0.0`.
- `GITHUB_PUBLISH_APPROVED=1` is missing.
- `gh` is not on `PATH`.
- `registry_deploys` is set and `GITHUB_REPOSITORY` is missing.
- `sign = "required"` and there is neither `COSIGN_KEY` nor an OIDC token.

## homebrew_tap_deploy

Use `homebrew_tap_deploy` to render a Homebrew formula from a
`github_deploy` release and push it to a tap repository.

```starlark
load("@rules_dx//deploy:defs.bzl", "homebrew_tap_deploy")

homebrew_tap_deploy(
    name = "tool_tap",
    release = ":github_demo",
    repo = "org/tool",
    tap = "org/homebrew-tap",
    formula = "tool",
)
```

- `release`: required. The `github_deploy` target. Its tag gives the
  version. Its `.tar.gz`, `.tgz` and `.zip` assets become the formula urls.
- `repo`: required. The GitHub `owner/repository` that hosts the release.
- `tap`: required. The GitHub `owner/repository` of the tap.
- `formula`: the formula name. Defaults to `name`.
- `binary`: the file at the top of each archive, installed into `bin`.
  Defaults to `formula`.
- `desc`: the formula description. Defaults to the formula name.
- `homepage`: the formula homepage. Defaults to `https://github.com/<repo>`.
- `build_mode`: `debug`, `dev` or `release`. Defaults to `release`.

Each asset name must name its platform:

- The OS: `darwin`, `macos` or `apple`, or `linux`.
- The CPU: `arm64` or `aarch64`, or `x86_64`, `amd64` or `x64`.

Other assets are ignored. The sha256 comes from the asset file. When the
release has a sibling `<asset>.sha256`, the two must match.

Check it first. This pushes nothing:

```sh
dx deploy //pkg:tool_tap
```

The dry run writes `<name>-tap/Formula/<formula>.rb` and `would-run.txt`
into the output tree. It runs `ruby -c` on the formula when `ruby` is on
`PATH`, and prints `syntax: skipped, ruby not on PATH` when it is not.

Before you publish, you need:

- `gh` on `PATH`.
- A token with write access to the tap for `HOMEBREW_TAP_API_TOKEN`.

Publish:

```sh
HOMEBREW_TAP_PUBLISH_APPROVED=1 HOMEBREW_TAP_API_TOKEN=... \
dx deploy --apply //pkg:tool_tap
```

This commits `Formula/<formula>.rb` to the tap's default branch through the
contents API.

A live run refuses, with exit code 1, when:

- The release tag is a placeholder, such as `v0.0.0-dryrun`.
- `HOMEBREW_TAP_API_TOKEN` is missing.
- `HOMEBREW_TAP_PUBLISH_APPROVED=1` is missing.

## linux_package_deploy

Use `linux_package_deploy` to build `.deb`, `.rpm` and `.apk` packages from
one binary with a pinned `nfpm`, and attach them to a draft GitHub Release
through `github_deploy`.

```starlark
load("@rules_dx//deploy:defs.bzl", "linux_package_deploy")

linux_package_deploy(
    name = "demo",
    binary = ":demo_bin",
    extra_files = {":demo_conf": "/etc/demo/demo.conf"},
    formats = ["deb", "rpm"],
    maintainer = "Demo <demo@example.com>",
    version = "1.2.3",
)
```

- `binary`: required. The executable to install.
- `version`: required. The package version, such as `1.2.3`. It must start
  with a digit.
- `install_path`: where `binary` is installed. Defaults to
  `/usr/bin/<package>`.
- `formats`: any of `deb`, `rpm` and `apk`. Defaults to `["deb", "rpm"]`.
- `platform`: a Linux x86_64 or arm64 Bazel platform label, recorded as the
  package architecture, `amd64` or `arm64`. Defaults to the current target
  platform. Other operating systems fail. The binary is not rebuilt for it.
- `package`: the package name. Defaults to `name`. When you set it, `name`
  need not be a valid package name.
- `maintainer`: `Name <email>`. Defaults to
  `unknown <unknown@example.com>`.
- `description`: defaults to the package name.
- `extra_files`: more files, mapped to absolute install paths. They are
  installed as `0644`.
- `release_tag`, `tag_from_head`, `notes`, `notes_file`, `build_mode`,
  `sbom`: as for [`github_deploy`](#github_deploy).

`nfpm` runs on Linux x86_64, Linux arm64, macOS arm64 and Windows x86_64
hosts.

Build the files only:

```sh
bazel build //pkg:demo_deb
```

This writes `demo_<version>_<arch>.deb`. Each format has its own
`<name>_<format>` target.

Check it first. This publishes nothing:

```sh
dx deploy //pkg:demo
```

The dry run stages the packages as `github_deploy` does.

Before you publish, you need what [`github_deploy`](#github_deploy) needs.

Publish:

```sh
GITHUB_PUBLISH_APPROVED=1 dx deploy --apply //pkg:demo
```

This attaches the packages to the draft release. A live run refuses what a
`github_deploy` live run refuses.

## pub_deploy

Use `pub_deploy` to stage a Dart or Flutter package and publish it to
pub.dev.

```starlark
load("@rules_dx//deploy:defs.bzl", "pub_deploy")

pub_deploy(
    name = "pub_demo",
    srcs = [":pub_demo_package"],
)
```

- `srcs`: required. The package files, including `pubspec.yaml` and
  `CHANGELOG.md`.
- `package`: the package name, in lowercase letters, digits and `_`.
  Defaults to `name`.
- `version`: empty by default, which publishes the version in
  `pubspec.yaml`. When you set it, it must equal that version. Either way,
  `CHANGELOG.md` needs a heading for it.
- `registry_url`: an https URL. Defaults to `https://pub.dev`.
- `build_mode`: `debug`, `dev` or `release`. Defaults to `release`.
- `api_compat`, `api_baseline`: see [Public API compatibility](#public-api-compatibility).
  Flutter packages skip the check.

Check it first. This needs no credentials and uploads nothing:

```sh
dx deploy //pkg:pub_demo
```

The dry run checks `pubspec.yaml` and `CHANGELOG.md` and stages
`<name>-package/`. It runs `dart pub publish --dry-run` on a copy when `dart`
is on `PATH`, and prints a skip line when it is not. A package with a
top-level `flutter:` section uses `flutter` instead of `dart`.

Before you publish, you need:

- `dart` on `PATH`, or `flutter` for a Flutter package.
- A pub.dev token for `PUB_API_TOKEN`.

Publish:

```sh
PUB_PUBLISH_APPROVED=1 PUB_API_TOKEN=... \
dx deploy --apply //pkg:pub_demo
```

This repeats the dry run, then runs `pub publish --force`. The token reaches
`dart pub token add --env-var`. It lands only in a temporary `PUB_CACHE`,
which is removed afterwards.

A live run refuses, with exit code 1, when:

- The version is `0.0.0` or `0.0.0-dryrun`.
- `PUB_API_TOKEN` is missing.
- `PUB_PUBLISH_APPROVED=1` is missing.
- `dart`, or `flutter` for a Flutter package, is not on `PATH`.

## nuget_deploy

Use `nuget_deploy` to stage one nupkg as a local folder feed and push it to a
NuGet source.

```starlark
load("@rules_dx//deploy:defs.bzl", "nuget_deploy")

nuget_deploy(
    name = "nuget_demo",
    nupkg = ":nuget_demo_nupkg",
)
```

- `nupkg`: required. One `.nupkg` file.
- `package`: the package id. Defaults to `name`. It must match the nuspec id.
- `version`: optional. It must match the nuspec version.
- `registry_url`: an https URL. Defaults to
  `https://api.nuget.org/v3/index.json`.
- `build_mode`: `debug`, `dev` or `release`. Defaults to `release`.
- `api_compat`, `api_baseline`: see [Public API compatibility](#public-api-compatibility).

Check it first. This needs no credentials and pushes nothing:

```sh
dx deploy //pkg:nuget_demo
```

The dry run checks the nuspec id and version against `package` and
`version`, and stages `<name>-feed/` with the nupkg. It does not run
`dotnet`.

Before you publish, you need:

- `dotnet` on `PATH`, with NuGet 7.6 or later (.NET SDK 10.0.300 or later).
- A NuGet API key for `NUGET_API_TOKEN`.

Publish:

```sh
NUGET_PUBLISH_APPROVED=1 NUGET_API_TOKEN=... \
dx deploy --apply //pkg:nuget_demo
```

This runs `dotnet nuget push --skip-duplicate`. The key reaches `dotnet`
through the environment, never the command line. If the version is already
on the source, the push is skipped. The run prints
`nuget_deploy: skipped <id> <version>: already on <source>` and exits 0.

A live run refuses, with exit code 1, when:

- The nuspec version is `0.0.0`.
- `NUGET_API_TOKEN` is missing.
- `NUGET_PUBLISH_APPROVED=1` is missing.
- `dotnet` is not on `PATH`.
- NuGet is older than 7.6.

## cpan_deploy

Use `cpan_deploy` to stage one distribution tarball and upload it to PAUSE,
the CPAN upload server.

```starlark
load("@rules_dx//deploy:defs.bzl", "cpan_deploy")

cpan_deploy(
    name = "cpan-demo",
    dist = ":cpan_demo_dist",
    package = "Cpan-Demo",
)
```

- `dist`: required. One `<Dist>-<version>.tar.gz` that holds
  `<Dist>-<version>/META.json` or `META.yml`.
- `package`: the distribution name. Defaults to `name`. It must match the
  file name.
- `version`: optional. It must match the version in the file name.
- `build_mode`: `debug`, `dev` or `release`. Defaults to `release`.

Check it first. This needs no credentials and uploads nothing:

```sh
dx deploy //pkg:cpan-demo
```

The dry run checks the name and version and stages `<name>-dist/` with the
tarball.

Before you publish, you need:

- Your PAUSE user name for `CPAN_API_USER`.
- Your PAUSE password for `CPAN_API_TOKEN`.

You do not need `perl`.

Publish:

```sh
CPAN_PUBLISH_APPROVED=1 CPAN_API_USER=... CPAN_API_TOKEN=... \
dx deploy --apply //pkg:cpan-demo
```

This posts the tarball to
`https://pause.perl.org/pause/authenquery?ACTION=add_uri` with HTTP basic
auth.

A live run refuses, with exit code 1, when:

- The version is `0.0.0` or `0.0.0-dryrun`.
- `CPAN_API_USER` or `CPAN_API_TOKEN` is missing.
- `CPAN_PUBLISH_APPROVED=1` is missing.

## conan_deploy

Use `conan_deploy` to stage one Conan recipe, then create it and upload it to
a Conan remote.

```starlark
load("@rules_dx//deploy:defs.bzl", "conan_deploy")

conan_deploy(
    name = "conan_demo",
    recipe = ":conan_demo_recipe",
    remote = "center2",
    remote_url = "https://center2.conan.io",
)
```

- `recipe`: required. The files, including `conanfile.py` and its sources.
- `remote`: required. The remote name, in lowercase letters, digits, `_`
  and `-`.
- `remote_url`: required. The https URL of the remote.
- `package`: the package name. Defaults to `name`. It must match `name` in
  `conanfile.py` when the recipe sets one.
- `version`: optional. It must match `version` in `conanfile.py`. You need it
  when the recipe sets none.
- `conan`, `conan_files`: optional. A `conan` executable and the files it
  needs. Defaults to the pinned conan on Linux x86_64 and arm64, macOS x86_64
  and arm64, and Windows x86_64.
- `build_mode`: `debug`, `dev` or `release`. Defaults to `release`.

Check it first. This needs no credentials and uploads nothing:

```sh
dx deploy //pkg:conan_demo
```

The dry run checks the recipe and stages `<name>-recipe/`.

Before you publish, you need:

- A remote user name for `CONAN_LOGIN_USERNAME`.
- Its password or token for `CONAN_PASSWORD`.

Publish:

```sh
CONAN_PUBLISH_APPROVED=1 CONAN_LOGIN_USERNAME=... CONAN_PASSWORD=... \
dx deploy --apply //pkg:conan_demo
```

This uses a private `CONAN_HOME` and runs `conan profile detect`,
`conan remote add`, `conan create` and `conan upload --confirm`. Only the
`conan upload` step gets the credentials, through its environment.

A live run refuses, with exit code 1, when:

- The version is `0.0.0` or `0.0.0-dryrun`.
- `CONAN_LOGIN_USERNAME` or `CONAN_PASSWORD` is missing.
- `CONAN_PUBLISH_APPROVED=1` is missing.

## hackage_deploy

Use `hackage_deploy` to stage one cabal sdist and upload it to Hackage.

```starlark
load("@rules_dx//deploy:defs.bzl", "hackage_deploy")

hackage_deploy(
    name = "hackage-demo",
    sdist = ":hackage_demo_sdist",
)
```

- `sdist`: required. One `<package>-<version>.tar.gz` made by `cabal sdist`.
- `package`: the package name. Defaults to `name`. It must match `name` in
  the `.cabal` file.
- `version`: optional. It must match `version` in the `.cabal` file.
- `build_mode`: `debug`, `dev` or `release`. Defaults to `release`.

Check it first. This needs no credentials and uploads nothing:

```sh
dx deploy //pkg:hackage-demo
```

Every run checks that the sdist holds `<package>-<version>/<package>.cabal`,
and that the file declares the name and version in the sdist file name. The
dry run then stages `<name>-sdist/` with the sdist.

Before you publish, you need:

- `cabal` on `PATH`.
- A Hackage API token for `HACKAGE_API_TOKEN`.

Publish:

```sh
HACKAGE_PUBLISH_APPROVED=1 HACKAGE_API_TOKEN=... \
dx deploy --apply //pkg:hackage-demo
```

This runs `cabal upload --publish <sdist>`. `CABAL_CONFIG` names a mode 0600
config in a private temp directory that holds the token, so the upload
ignores your own cabal config.

A live run refuses, with exit code 1, when:

- The version is `0.0.0` or `0.0.0-dryrun`.
- `HACKAGE_API_TOKEN` is missing, or holds whitespace or control characters.
- `HACKAGE_PUBLISH_APPROVED=1` is missing.
- `cabal` is not on `PATH`.

## luarocks_deploy

Use `luarocks_deploy` to stage one rockspec and upload it to LuaRocks.

```starlark
load("@rules_dx//deploy:defs.bzl", "luarocks_deploy")

luarocks_deploy(
    name = "luarocks_demo",
    rockspec = ":luarocks_demo_rockspec",
)
```

- `rockspec`: required. One `<package>-<version>.rockspec`.
- `package`: the rock name. Defaults to `name`. It must match `package` in
  the rockspec.
- `version`: optional. It must match `version` in the rockspec, with or
  without the `-<revision>` suffix.
- `build_mode`: `debug`, `dev` or `release`. Defaults to `release`.

Check it first. This needs no credentials and uploads nothing:

```sh
dx deploy //pkg:luarocks_demo
```

Every run checks that the rockspec file name is
`<package>-<version>.rockspec` for the package and version it declares. The
dry run then stages `<name>-rock/` with the rockspec. It does not run
`luarocks`.

Before you publish, you need:

- `luarocks` on `PATH`.
- A LuaRocks API key for `LUAROCKS_API_TOKEN`.

Publish:

```sh
LUAROCKS_PUBLISH_APPROVED=1 LUAROCKS_API_TOKEN=... \
dx deploy --apply //pkg:luarocks_demo
```

This runs `luarocks upload <rockspec>`, which packs the source rock and
uploads it. `LUAROCKS_CONFIG` names an empty config in a private temp
directory, next to a mode 0600 `upload_config.lua` that holds the key. The
upload ignores your own LuaRocks config and saves nothing to it.

A live run refuses, with exit code 1, when:

- The version without its `-<revision>` suffix is `0.0.0` or `0.0.0-dryrun`.
- `LUAROCKS_API_TOKEN` is missing, or holds whitespace, control characters,
  `"` or `\`.
- `LUAROCKS_PUBLISH_APPROVED=1` is missing.
- `luarocks` is not on `PATH`.

## Base images

Container images build on distroless base images, pinned by digest and
fetched with `rules_oci` and `rules_pkg`.

| Repository | Image |
| --- | --- |
| `@distroless_static` | `gcr.io/distroless/static-debian12` |
| `@distroless_cc` | `gcr.io/distroless/cc-debian12` |
| `@distroless_python` | `gcr.io/distroless/python3-debian12` |
| `@distroless_java` | `gcr.io/distroless/java21-debian12` |
| `@distroless_nodejs` | `gcr.io/distroless/nodejs22-debian12` |

Each pin is the multi-arch index digest. `@<name>_linux_amd64` and
`@<name>_linux_arm64` hold the single-platform images.

```sh
bazel build @distroless_static
```

The first build fetches the layers. Later builds use the Bazel cache and need
no network.

## oci_image_from_binary

Use `oci_image_from_binary` to build a container image from one binary, with
no Dockerfile. It only builds files. Publish the image with
[`oci_deploy`](#oci_deploy).

```starlark
load("@rules_dx//deploy:defs.bzl", "oci_deploy", "oci_image_from_binary")

oci_image_from_binary(
    name = "image",
    binary = ":hello",
)

oci_deploy(
    name = "publish",
    image_tar = ":image",
    platform = "@rules_dx//deploy:linux_amd64",
)
```

Build it:

```sh
bazel build //pkg:image
```

With one platform, `<name>` is an `oci_image`. With several, it is an
`oci_image_index` that builds the binary once per platform, and `oci_deploy`
takes one `platform` of it. The macro also makes:

- `<name>_load`: loads the image into a local container runtime when you
  `dx run` it. With several platforms it loads the first one.
- `<name>_binary`: the binary built for Linux.

The binary and its runfiles go in one layer under `/app`. The entrypoint is
`/app/<binary name>`.

- `binary`: required. A `rust_binary`, `go_binary`, `cc_binary` or zig
  binary, or the app a `python`, `java`, `node` or `dotnet` base runs.
- `base`: the base image. `static` or `cc`, the distroless images in
  [Base images](#base-images), or `python`, `java`, `node` or `dotnet`.
  Defaults to `static`.
- `main`: the entry script of a `python` or `node` base. Defaults to the
  binary's package and name with `.py` or `.js`, so `//app:server` runs
  `app/server.py` and `//app/server` runs `app/server/server.py`. A `dotnet`
  base defaults to `<pkg>/<name>/net10.0/<name>.dll`.
- `main_class`: the class of a `java` base. Defaults to the `main_class` of
  the `java_binary`, written to an argument file next to the jars.
- `runtime`: `True` or `False`, for the `python`, `java`, `node` and
  `dotnet` bases. Defaults to `False`. `True` replaces the base interpreter
  with a layer from the managed toolchain, installed under `/opt/<base>` on
  the distroless `cc` image. The dotnet runtime runs with invariant
  globalization.
- `platforms`: Bazel platform labels, `@rules_dx//deploy:linux_amd64` or
  `@rules_dx//deploy:linux_arm64`, each at most once. Defaults to
  `["@rules_dx//deploy:linux_amd64"]`.
- `structure_tests`: container-structure-test config files, `.yaml`, `.yml`
  or `.json`, that `dx typecheck` runs against the image. Defaults to `[]`.
- `tags`, `visibility`, `testonly`: passed on to the image targets.

The image targets are incompatible with a Windows target platform, so a
wildcard build on Windows skips them. A Windows exec platform still builds
the Linux binary of an image, the `<name>_binary` target, with the
Linux-target Rust and cc toolchains.

## oci_deploy

Use `oci_deploy` to stage one image as a local OCI layout and push it to a
container registry.

```starlark
load("@rules_dx//deploy:defs.bzl", "oci_deploy")

oci_deploy(
    name = "oci_demo",
    image_tar = ":oci_demo_tar",
    platform = "@rules_dx//deploy:linux_amd64",
)
```

- `image_tar`: required. One image: a `docker save` or OCI layout archive,
  `.tar` or `.tar.gz`, or an OCI layout directory such as a rules_oci
  `oci_image`.
- `platform`: required. The Bazel platform label of the image,
  `@rules_dx//deploy:linux_amd64` or `@rules_dx//deploy:linux_arm64`. The
  deploy records it as `linux/amd64` or `linux/arm64`.
- `image_tag`: defaults to `latest`. At most 128 characters, and it must
  not start with `.` or `-`.
- `registry`: defaults to `ghcr.io`. It may carry a path prefix, such as
  `ghcr.io/example`. The host is case-insensitive. The path after it must be
  lowercase with no empty segments.
- `repository`: lowercase OCI path segments. Defaults to `name`, so a
  mixed-case target name needs `repository` set.
- `build_mode`: `debug`, `dev` or `release`. Defaults to `release`.
- `size_budget`: optional, such as `500MB`. The build fails when the image,
  the archive file or the layout directory tree, is larger.
- `sbom`: optional. A `dx_sbom` target, attested to the pushed digest.
- `scan`: `off`, `warn` or `fail`. Defaults to `fail`. Scans the staged
  image for vulnerabilities with `trivy`. See below.
- `scan_severity`: the lowest severity the scan reports: `LOW`, `MEDIUM`,
  `HIGH` or `CRITICAL`. Defaults to `HIGH`.
- `sign`: `auto`, `off` or `required`. Defaults to `auto`. See below.

The scan runs `trivy`, from `DX_TRIVY` or else `PATH`, offline against a
pinned database. Findings that an exception in the workspace `security.toml`
covers are left out. With `warn`, the run prints the findings. With `fail`,
findings or a scan error make the run exit 1.

With `sign` set to `auto` or `required`, a live run signs the pushed image
with the pinned cosign when `COSIGN_KEY` is set or the run has a GitHub OIDC
token (`ACTIONS_ID_TOKEN_REQUEST_URL`). After the push it runs
`cosign sign --yes <registry>/<repository>@<digest>` and, with `sbom`,
`cosign attest --yes --type spdxjson --predicate <sbom> <ref>`. A keyless run
then checks both with `cosign verify` and `cosign verify-attestation`. Set
`COSIGN_REPOSITORY` to store the signatures in another repository.

Check it first. This needs no credentials and pushes nothing:

```sh
dx deploy //pkg:oci_demo
```

The dry run stages `<repository>-oci-layout/` with an `index.json` and
prints the image size in bytes. It prints the signing identity, or `none`,
and the cosign commands a live run would run. Then it runs the scan.

Before you publish, you need:

- A registry user name for `OCI_API_USER`.
- A registry token or password for `OCI_API_TOKEN`.

You do not need `docker`.

Publish:

```sh
OCI_PUBLISH_APPROVED=1 OCI_API_USER=... OCI_API_TOKEN=... \
dx deploy --apply //pkg:oci_demo
```

This stages the same layout into the output directory, scans it and uploads
it over HTTPS with the OCI Distribution API. It sends the credentials as a
Basic login, or trades them for a bearer token, as the registry asks. Layers
go up as staged, so uncompressed layers stay uncompressed. It prints
`oci_deploy: pushed <registry>/<repository>:<tag>@sha256:...`, the digest of
the staged manifest.

`docker.io` pushes to `registry-1.docker.io`, and a single-segment
repository gets `library/`. Plain-HTTP registries are not supported.

A live run refuses, with exit code 1, when:

- The tag is `0.0.0` or `0.0.0-dryrun`.
- `OCI_API_USER` or `OCI_API_TOKEN` is missing.
- `OCI_PUBLISH_APPROVED=1` is missing.
- `scan = "fail"` and the scan finds something or cannot run.
- `sign = "required"` and there is neither `COSIGN_KEY` nor an OIDC token.

## octopus_deploy

Use `octopus_deploy` to stage one package for Octopus Deploy, push it to an
Octopus server and create a release.

```starlark
load("@rules_dx//deploy:defs.bzl", "octopus_deploy")

octopus_deploy(
    name = "octopus_demo",
    channel = "Default",
    deploy_to = ["Production"],
    project = "octopus_demo",
    srcs = ":octopus_demo_package",
)
```

- `srcs`: required. One `.tar.gz` named `<package>.<version>.tar.gz`.
- `project`: required. The Octopus project.
- `package`: the Octopus package id. Defaults to `name`.
- `channel`: defaults to `Default`.
- `version`: empty by default, which takes the version from the archive file
  name. When you set it, it must match that version.
- `deploy_to`: the environments to deploy the release to. Defaults to `[]`.
- `space`: empty by default, which means the default space. `OCTOPUS_SPACE`
  overrides it.
- `registry_url`: the server, an https URL. Defaults to
  `https://octopus.example.invalid`. `OCTOPUS_URL` overrides it.
- `build_mode`: `debug`, `dev` or `release`. Defaults to `release`.

Check it first. This needs no credentials and pushes nothing:

```sh
dx deploy //pkg:octopus_demo
```

The dry run checks the package and the spec, and stages `<name>-drop/` with
the package and a `would-run.txt`. That file holds the `octo push` and
`octo create-release` commands. It runs no `octo` command.

Before you publish, you need:

- `octo` on `PATH`.
- An Octopus API key for `OCTOPUS_API_TOKEN`.
- The server URL in `OCTOPUS_URL`, unless `registry_url` already names it.

Publish:

```sh
OCTOPUS_PUBLISH_APPROVED=1 OCTOPUS_API_TOKEN=... \
OCTOPUS_URL=https://octopus.example.com \
dx deploy --apply //pkg:octopus_demo
```

This runs `octo push --package`, then `octo create-release` with a
`--deployTo` for each `deploy_to` entry. Deployments are queued, not awaited.
The key reaches `octo` as `OCTOPUS_CLI_API_KEY` in its environment, never the
command line.

If the version is already on the server, `octo push` fails, the run exits 1
and the hint names `FailIfExists`. That includes a rerun after a failed
`create-release`.

A live run refuses, with exit code 1, when:

- The version is `0.0.0` or `0.0.0-dryrun`.
- The server is the default `registry_url` and `OCTOPUS_URL` is not set.
- The server URL is not https, or holds a user or password.
- `OCTOPUS_API_TOKEN` is missing.
- `OCTOPUS_PUBLISH_APPROVED=1` is missing.
- `octo` is not on `PATH`.

## pypi_deploy

Use `pypi_deploy` to stage one wheel and an optional sdist as a local PEP 503
index, and upload them to PyPI with `twine`.

```starlark
load("@rules_dx//deploy:defs.bzl", "pypi_deploy")

pypi_deploy(
    name = "pypi_demo",
    sdist = ":pypi_demo_sdist",
    wheel = ":pypi_demo_wheel",
)
```

- `name`: the distribution name. Letters, digits, `.`, `_` and `-` only, and
  it must start and end with a letter or digit. It names the wheelhouse
  directory.
- `wheel`: required. One `.whl` file named
  `name-version-python-abi-platform.whl`. An optional build tag adds a sixth
  field. Native wheels come from `py_wheel`. `pypi_deploy` never rebuilds an
  extension from the sdist.
- `sdist`: optional. One `.tar.gz` file named `name-version.tar.gz`. Its
  project and version must match the wheel's.
- `registry_url`: an https URL. Defaults to
  `https://upload.pypi.org/legacy/`.
- `build_mode`: `debug`, `dev` or `release`. Defaults to `release`.
- `api_compat`, `api_baseline`: see [Public API compatibility](#public-api-compatibility).
- `api_package`: the import name the API check reads. Defaults to the
  distribution name.
- `api_src`: the workspace-relative source directory the API check reads at
  the git tag. `api_baseline = "git"` needs it.

Project names are PEP 503 normalized: lowercased, with each run of `.`, `_`
or `-` replaced by one `-`.

Check it first. This needs no credentials and uploads nothing:

```sh
dx deploy //pkg:pypi_demo
```

The dry run checks the wheel and sdist file names and that their project and
version match. It does not run `twine`. It writes this tree into the output
directory, see [Common behavior](#common-behavior):

```text
<name>-wheelhouse/
  <wheel file>
  <sdist file>          only when sdist is set
  simple-index/
    <project>/
      index.html
```

`index.html` has one link per file, sorted by file name. Each link points to
`../../<file>#sha256=<digest>`. The run prints:

```text
pypi_deploy: staged <name> (build_mode: <build_mode>)
  auth: <PYPI_API_TOKEN | trusted publishing | none>
  wheelhouse: <path>/<name>-wheelhouse
  index: <path>/<name>-wheelhouse/simple-index/<project>/index.html
```

Before you publish, you need:

- `twine` on `PATH`.
- A PyPI API token for `PYPI_API_TOKEN`, or a GitHub trusted-publisher
  identity: `ACTIONS_ID_TOKEN_REQUEST_URL` and
  `ACTIONS_ID_TOKEN_REQUEST_TOKEN`.

Publish:

```sh
PYPI_PUBLISH_APPROVED=1 PYPI_API_TOKEN=... \
dx deploy --apply //pkg:pypi_demo
```

`PYPI_API_TOKEN` reaches `twine` as the password, with user name
`__token__`, never on the command line. Without it, the run trades the
GitHub identity for an upload token at `https://pypi.org/_/oidc/mint-token`,
or `https://test.pypi.org/_/oidc/mint-token` for `test.pypi.org`. Any other
registry needs `PYPI_API_TOKEN`.

When the identity variables are set, the run first signs the staged files
with the `pypi-attestations` pinned in `core/quality/tools/python/uv.lock`.
It passes the `.publish.attestation` files to `twine upload --attestations`.

The live run does not write `<name>-wheelhouse/` and does not check file
names. It runs `twine check --strict`, then
`twine upload --non-interactive --repository-url <registry_url>` with the
wheel and sdist files. `twine` runs with a cleared environment. Only
`REQUESTS_CA_BUNDLE` and the keep-list in [Common behavior](#common-behavior)
pass through. On success it prints:

```text
pypi_deploy: uploaded <wheel file> to <registry_url>
```

If the version is already on PyPI, PyPI refuses the file. The run prints
`pypi_deploy: <file> version <version> is already on <url>; upload a new version`
and exits 1.

A live run refuses, with exit code 1, when:

- `PYPI_PUBLISH_APPROVED=1` is missing.
- Neither `PYPI_API_TOKEN` nor a trusted-publisher identity is set.
- It would sign on a windows_arm64 execution platform, where signing is
  unavailable.

Exit codes:

- `0`: the wheelhouse is staged, or the upload succeeds.
- `1`: invalid file names or a mismatched sdist, a refused live run, or a
  failed `twine` upload.

## helm_chart_deploy

Use `helm_chart_deploy` to package a `helm_library` with the pinned `helm`
and push it to an OCI registry.

```starlark
load("@rules_dx//deploy:defs.bzl", "helm_chart_deploy")

helm_chart_deploy(
    name = "helm_demo",
    chart = ":chart",
    registry = "ghcr.io/acme/charts",
)
```

- `chart`: required. A `helm_library` target with a `Chart.yaml`. Its files
  are copied to a staging directory.
- `registry`: required. A host plus an optional path, as for
  [`oci_deploy`](#oci_deploy): lowercase, with no empty segments.
- `repository`: a path below `registry`. Defaults to none.
- `version`: empty by default, which publishes the `version` in
  `Chart.yaml`. When you set it, it must equal that version.
- `build_mode`: `debug`, `dev` or `release`. Defaults to `release`.

It always uses the pinned helm from `//config:tool_helm`, with a cleared
environment.

Check it first. This needs no credentials and pushes nothing:

```sh
dx deploy //pkg:helm_demo
```

The dry run runs `helm lint`, then `helm package`, and prints the `.tgz`
path.

Before you publish, you need:

- A registry user name for `OCI_API_USER`.
- A registry token or password for `OCI_API_TOKEN`.

Publish:

```sh
HELM_PUBLISH_APPROVED=1 OCI_API_USER=... OCI_API_TOKEN=... \
dx deploy --apply //pkg:helm_demo
```

This logs in with `helm registry login` into a throwaway registry config,
runs `helm push <chart>.tgz oci://<registry>/<repository>`, and removes the
config.

A live run refuses, with exit code 1, when:

- The version is `0.0.0` or `0.0.0-dryrun`.
- `OCI_API_USER` or `OCI_API_TOKEN` is missing.
- `HELM_PUBLISH_APPROVED=1` is missing.

Exit codes:

- `0`: the chart is packaged, or pushed.
- `1`: a version mismatch, a lint or package failure, a refused live run, or
  a failed push.

## dx_split_debug_info

Use `dx_split_debug_info` to split the debug info out of a Linux `cc_binary`,
so you can ship the stripped binary and keep the symbols. It only builds
files.

```starlark
load("@rules_dx//deploy:defs.bzl", "dx_split_debug_info")

dx_split_debug_info(
    name = "app_split",
    binary = ":app",
)
```

Compile the binary with `-g` and do not strip it at link time. Build the
`symbols` output group:

```sh
bazel build --output_groups=symbols //pkg:app_split
```

The target itself builds the stripped binary. The `symbols` output group
holds:

- `app_split.debug`: the debug info.
- `app_split.symbols.json`: a manifest with the build-id, arch, sha256 of the
  stripped binary, debug file name and `llvm-objcopy` version.

The build fails if the stripped binary and the debug file differ in build-id
or arch.

- `binary`: required. The Linux `cc_binary`.
- `target_compatible_with`: defaults to Linux only.
- Bazel's common attributes, such as `tags` and `visibility`.

Look up an address with `llvm-symbolizer`:

```sh
llvm-symbolizer --obj=app_split.debug 0x401136
```

## dx_dsym

Use `dx_dsym` to pair the dSYM of a macOS `cc_binary` with that binary. It
only builds files.

```starlark
load("@rules_dx//deploy:defs.bzl", "dx_dsym")

dx_dsym(
    name = "app_dsym",
    binary = ":app",
)
```

Build the `symbols` output group:

```sh
bazel build --output_groups=symbols //pkg:app_dsym
```

The `binary` is built again with `apple_generate_dsym` set, so the link
writes the dSYM bundle. The `symbols` output group holds:

- `app_dsym.dSYM`: the bundle, with its DWARF file under
  `Contents/Resources/DWARF/`.
- `app_dsym.dsym.json`: a manifest with the Mach-O UUID, arch, sha256 of the
  binary, DWARF path and `dsymutil` version.

The build fails if the bundle has no DWARF file, or if its UUID or arch
differ from the binary.

- `binary`: required. The macOS `cc_binary`.
- `target_compatible_with`: defaults to macOS only.
- Bazel's common attributes, such as `tags` and `visibility`.

Look up an address in the bundle with `llvm-symbolizer`:

```sh
llvm-symbolizer --obj=app_dsym.dSYM/Contents/Resources/DWARF/app 0x100003f40
```

## dx_pdb

Use `dx_pdb` to pair the PDB of a Windows `cc_binary` with that binary. It
only builds files.

```starlark
load("@rules_dx//deploy:defs.bzl", "dx_pdb")

dx_pdb(
    name = "app_pdb",
    binary = ":app",
)
```

Build the `symbols` output group:

```sh
bazel build --output_groups=symbols //pkg:app_pdb
```

The `binary` is built again with the `generate_pdb_file` feature added, so
the link writes the PDB. The `symbols` output group holds:

- The PDB the link wrote, such as `app.pdb`.
- `app_pdb.pdb.json`: a manifest with the CodeView GUID and age, arch, sha256
  of the binary, PDB name, CodeView path and linker version.

The build fails if the PDB is not a PDB, or if its GUID, age or machine
differ from the binary.

- `binary`: required. The Windows `cc_binary`.
- `target_compatible_with`: defaults to Windows only.
- Bazel's common attributes, such as `tags` and `visibility`.

Look up an address, a relative virtual address, with `llvm-symbolizer`:

```sh
llvm-symbolizer --obj=app.exe --pdb=app.pdb --relative-address 0x1000
```

## opam_deploy

Use `opam_deploy` to open a pull request to `ocaml/opam-repository` that adds
one package version.

```starlark
load("@rules_dx//deploy:defs.bzl", "opam_deploy")

opam_deploy(
    name = "opam_demo",
    opam_file = "opam",
    package = "demo",
    version = "1.2.0",
)
```

- `package`: required. Letters, digits, `_`, `+` and `-`.
- `version`: required. Letters, digits, `.`, `_`, `+`, `~` and `-`.
- `opam_file`: required. A file named `opam`. A `name:` or `version:` field
  in it must equal `package` and `version`.
- `build_mode`: `debug`, `dev` or `release`. Defaults to `release`.

Check it first. This needs no credentials and contacts nothing:

```sh
dx deploy //pkg:opam_demo
```

The dry run lints the file with the pinned opam, a release download pinned by
sha256. It prints the target path
`packages/<package>/<package>.<version>/opam`.

Before you publish, you need:

- A GitHub token that can fork and open pull requests, for
  `OPAM_API_TOKEN`.

Publish:

```sh
OPAM_PUBLISH_APPROVED=1 OPAM_API_TOKEN=... \
dx deploy --apply //pkg:opam_demo
```

This forks `ocaml/opam-repository` and commits the file on a branch named
`<package>.<version>`. Characters outside `[A-Za-z0-9._-]`, such as `~`,
become `-` in the branch name. Then it opens the pull request against
`master`. A rerun updates the file on that branch and reports the pull
request that is already open.

A live run refuses, with exit code 1, when:

- `OPAM_API_TOKEN` is missing.
- `OPAM_PUBLISH_APPROVED=1` is missing.

## packagist_deploy

Use `packagist_deploy` to ask Packagist to update a package after you push
its git tag.

```starlark
load("@rules_dx//deploy:defs.bzl", "packagist_deploy")

packagist_deploy(
    name = "packagist_demo",
    composer_json = "composer.json",
    package = "acme/demo",
    repository_url = "https://github.com/acme/demo",
)
```

- `package`: required. `vendor/name`, lowercase. It must equal `name` in
  `composer_json`.
- `composer_json`: required. A file named `composer.json`.
- `repository_url`: required. The `https://` URL registered on Packagist.
- `build_mode`: `debug`, `dev` or `release`. Defaults to `release`.

Check it first. This needs no credentials and contacts nothing:

```sh
dx deploy //pkg:packagist_demo
```

The dry run checks the name and prints the request.

Before you publish, you need:

- The release tag pushed to the repository.
- Your Packagist user name for `PACKAGIST_USERNAME`.
- A Packagist API token for `PACKAGIST_API_TOKEN`.

Publish:

```sh
PACKAGIST_PUBLISH_APPROVED=1 PACKAGIST_USERNAME=... PACKAGIST_API_TOKEN=... \
dx deploy --apply //pkg:packagist_demo
```

This posts to `https://packagist.org/api/update-package`.

A live run refuses, with exit code 1, when:

- `PACKAGIST_USERNAME` or `PACKAGIST_API_TOKEN` is missing.
- `PACKAGIST_PUBLISH_APPROVED=1` is missing.

## Registries released by tag

Go, Julia, Swift and dub have no deploy rule. You release by pushing a git
tag. To add a GitHub release too, use `github_deploy` with `release_tag` set
to the tag.

- Go: push the tag `vX.Y.Z`. The module proxy fetches it from the
  repository.
- Julia: push the tag, then comment `@JuliaRegistrator register` on the
  commit.
- Swift: push a semver tag such as `1.2.0`. SwiftPM reads versions from tags.
- dub: push a semver tag such as `v1.2.0`. The dub registry reads versions
  from tags.

PHP, opam and CPAN have a deploy rule:

- PHP: run `composer validate`, push a tag, then run `packagist_deploy`.
- opam: run `opam_deploy`.
- CPAN: run `cpan_deploy`.

## Public API compatibility

`crates_deploy`, `maven_deploy`, `npm_deploy`, `pypi_deploy`, `nuget_deploy`,
`pub_deploy` and `github_deploy` check the public API the same way. Every run
compares the version you deploy with the highest earlier release and lists
the breaking changes.

- `api_compat`: `fail`, `warn` or `off`. Defaults to `fail`. With `fail`,
  breaking changes refuse the run unless the major version goes up. With
  `warn`, they are only printed.
- `api_baseline`: `registry` or `git`. Defaults to `registry`. `registry`
  reads the highest published version below the new one from the registry.
  `git` uses the highest `v*` tag below the new version.

A first release passes. When the earlier release cannot be fetched, the check
prints why and is skipped. The tools are pinned and come with the rule.

| Rule | Tool | `registry` reads | `git` reads |
| --- | --- | --- | --- |
| `crates_deploy` | cargo-semver-checks | crates.io | the crate directory at the tag |
| `maven_deploy` | japicmp | the jar from `maven-metadata.xml` | the jar of the tag's version |
| `npm_deploy` | api-extractor | the npm packument and tarball | the tarball of the tag's version |
| `pypi_deploy` | griffe | the PyPI JSON and its wheel | `api_src` at the tag |
| `nuget_deploy` | Microsoft.DotNet.ApiCompat | the feed's package versions and `.nupkg` | the `.nupkg` of the tag's version from the feed |
| `pub_deploy` | dart_apitool | the server's `api/packages/<name>` versions | the package directory at the tag |
| `github_deploy` (Go) | apidiff | proxy.golang.org | `go_modules` at the tag |

## Common behavior

Except for `cloudflare_pages_deploy`, a dry run writes into the workspace
root, or into the directory you pass as the first argument. Every deploy
target, `npm_deploy` included, rejects any other argument.

Deploy targets run through `dx deploy` or `bazel run`, which set
`DX_DEPLOY_CONFIG`.

A live run without its credentials or its `*_APPROVED=1` variable is refused.

Exit codes:

- `0`: staged or published.
- `1`: any failure or refusal.

Publisher tools (`twine`, `dotnet`, `cargo`, `gh`, `octo`, `npm`, `cabal`,
`luarocks`) run with a cleared environment plus a keep-list for each tool.
`wrangler` has its own, narrower list. Secrets reach a tool only through that
environment, stdin or a temp file, never its command line. On Unix the temp
file is mode 0600. On Windows it relies on the access list of your temp
directory. In `npm`, `twine`, `dotnet` and `octo` output, the token is
replaced with `[redacted]`, and the output appears when the tool exits.
