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
- `version`: defaults to `0.0.0`.
- `repository_url`: defaults to the Central Portal OSSRH staging API below.
- `profile`: `debug`, `dev` or `release`. Defaults to `release`.

Default run stages a local file repository under `<artifact>-repo/`. No
credentials needed. It writes, next to the jar and the pom:

- `.md5`, `.sha1`, `.sha256` and `.sha512` files with bare lowercase hex.
- `.asc` detached signatures when `MAVEN_GPG_KEY` is set.
- `maven-metadata.xml` one level up.

Output ends with `signed: yes` or `signed: no`.

Signing env:

- `MAVEN_GPG_KEY`: an ASCII-armored OpenPGP secret key. Ed25519 is
  recommended. A signing subkey is used when the key has one.
- `MAVEN_GPG_PASSPHRASE`: unlocks `MAVEN_GPG_KEY`.

The pom must match `group`, `artifact` and `version` (`groupId` may come
from `parent`). `packaging` must be absent or `jar`. It must also have
`name`, `description`, `url`, `licenses/license/name`,
`developers/developer`, `scm/url`, `scm/connection` and
`scm/developerConnection`. The error names every missing field.

Live upload goes to the Central Portal OSSRH staging API:
`https://ossrh-staging-api.central.sonatype.com/service/local/staging/deploy/maven2/`

Setup:

- A Central Portal account with a verified namespace.
- A Portal user token for `MAVEN_USERNAME` and `MAVEN_PASSWORD`.
- `mvn` on `PATH`.

Run:

```
MAVEN_PUBLISH_LIVE=1 MAVEN_PUBLISH_APPROVED=1 \
MAVEN_USERNAME=... MAVEN_PASSWORD=... \
MAVEN_GPG_KEY=... MAVEN_GPG_PASSPHRASE=... \
bazel run //pkg:maven_demo
```

Upload only stages the deployment. Validate and publish it in the Central
Portal. Live upload sends the jar and the pom. Live signing runs the pinned
maven-gpg-plugin `sign-and-deploy-file` goal, which reads the same
`MAVEN_GPG_KEY` and `MAVEN_GPG_PASSPHRASE` env.

Refusals, all exit 1:

- Version `0.0.0` in a live run.
- Missing `MAVEN_USERNAME` or `MAVEN_PASSWORD`.
- Missing `MAVEN_PUBLISH_APPROVED=1`.
- No `MAVEN_GPG_KEY` without `MAVEN_ALLOW_UNSIGNED=1`.
- `MAVEN_GPG_PASSPHRASE` set without `MAVEN_GPG_KEY`.
- A `MAVEN_GPG_KEY` that is not an armored secret key, or a
  `MAVEN_GPG_PASSPHRASE` that does not unlock it.
- A `repository_url` on `oss.sonatype.org` or `s01.oss.sonatype.org`.
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

Default run stages the tarball and feed locally. No credentials needed.

Setup:

- `npm` on `PATH`.
- An `NPM_TOKEN` from the registry.
- `provenance = "required"` needs a CI OIDC identity
  (`ACTIONS_ID_TOKEN_REQUEST_URL` or `SIGSTORE_ID_TOKEN`).

Run:

```
NPM_PUBLISH_LIVE=1 NPM_PUBLISH_APPROVED=1 NPM_TOKEN=... \
bazel run //pkg:npm_demo
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
and the branch. Nothing is uploaded.

A run without `CLOUDFLARE_PAGES_LIVE=1` checks the same things and uploads
nothing. It needs no token:

```
dx deploy --apply //pkg:site_deploy
```

```
cloudflare_pages_deploy: checked my-site (branch feature/x, preview, profile: release); nothing uploaded
  account: 0123456789abcdef0123456789abcdef
  project: my-site
  branch: feature/x
  intent: preview
  files: 12 (48213 bytes)
  publish: set CLOUDFLARE_PAGES_LIVE=1 and CLOUDFLARE_PAGES_APPROVED=1 with CLOUDFLARE_API_TOKEN
```

Publish:

```
CLOUDFLARE_PAGES_LIVE=1 CLOUDFLARE_PAGES_APPROVED=1 \
CLOUDFLARE_API_TOKEN=... \
dx deploy --apply //pkg:site_deploy
```

`CLOUDFLARE_PAGES_LIVE=1` without `CLOUDFLARE_PAGES_APPROVED=1` is refused.
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
output. Arguments, `--dry-run` and `CLOUDFLARE_PAGES_DRY_RUN` are refused.
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

Packages one app with its resources, a `NOTICE` and an `IDENTITY.txt` as a
tarball plus a sha256 file. It builds files only and has no deploy run.

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
- `resources`: targets whose files land under `resources/`. Each file must
  sit under a directory named after its target. Defaults to `[]`.
- `notice`: required. Exactly one file, stored as `NOTICE`.
- `tags`: Bazel tags for every generated target.

`IDENTITY.txt` lists `name`, `arch`, `os`, `linkage` and `min_runtime`.
Outputs: `<name>_archive` builds `<name>.tar.gz` and `<name>_checksum`
builds `<name>.tar.gz.sha256`.

Examples: [Qt packaging](../../qt/README.md#packaging) and
[qt-rust](../../examples/qt-rust/app/BUILD.bazel).

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
CRATES_PUBLISH_LIVE=1 CRATES_PUBLISH_APPROVED=1 CARGO_REGISTRY_TOKEN=... \
dx deploy --apply //pkg:crates_demo
```

A live run refuses version `0.0.0` and `CRATES_ALLOW_DIRTY=1`.

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

- `artifacts`: required. At least one file. Asset names must be unique.
- `tag`: defaults to `v0.0.0-dryrun`.
- `profile`: `debug`, `dev` or `release`. Defaults to `release`.
- `tags`: Bazel tags for the generated targets. Defaults to `[]`.

Default run stages `<name>-release/` with the assets and a `would-run.txt`
holding their sha256 and the `gh release create` command. Nothing is
published. `GH_RELEASE_DRY_RUN=1` only changes the first output line.

Create the draft with `gh` on `PATH` and logged in:

```
GH_RELEASE_LIVE=1 GH_RELEASE_APPROVED=1 dx deploy --apply //pkg:github_demo
```

This runs `gh release create <tag> <assets> --draft --verify-tag`, so the
tag must already exist on GitHub. A live run refuses `v0.0.0-dryrun`.

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
NUGET_PUBLISH_LIVE=1 NUGET_PUBLISH_APPROVED=1 NUGET_API_KEY=... \
dx deploy --apply //pkg:nuget_demo
```

This runs `dotnet nuget push --skip-duplicate`. The key goes to `dotnet`
through the `NUGET_API_KEY` env var, never the command line, so NuGet 7.6+
(.NET SDK 10.0.300+) is required; older versions are refused. A live run refuses nuspec
version `0.0.0`.

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
  `.tar.gz`, holding one image.
- `platform`: required. `linux/amd64` or `linux/arm64`.
- `tag`: defaults to `latest`. At most 128 chars, not starting with `.` or `-`.
- `registry`: defaults to `ghcr.io`.
- `repository`: lowercase OCI path segments. Defaults to `name`, so a
  mixed-case target needs `repository` set.
- `profile`: `debug`, `dev` or `release`. Defaults to `release`.

Default run stages `<repository>-oci-layout/` with an `index.json`. No
credentials needed. `OCI_PUBLISH_DRY_RUN=1` also prints the push it would
run.

Push:

```
OCI_PUBLISH_LIVE=1 OCI_PUBLISH_APPROVED=1 \
OCI_REGISTRY_USER=... OCI_REGISTRY_TOKEN=... \
dx deploy --apply //pkg:oci_demo
```

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
OCTOPUS_PUBLISH_LIVE=1 OCTOPUS_PUBLISH_APPROVED=1 OCTOPUS_API_KEY=... \
OCTOPUS_URL=https://octopus.example.com \
dx deploy --apply //pkg:octopus_demo
```

This runs `octo push --package`, then `octo create-release` with a
`--deployTo` for each `deploy_to`. Deployments are queued, not awaited. A rerun
after a failed `create-release` fails at push. The key goes to `octo` as `OCTOPUS_CLI_API_KEY` in its
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
PROMOTION_LIVE=1 PROMOTION_APPROVED=1 dx deploy --apply //pkg:promotion_demo
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

Stages one wheel and an optional sdist as a local index and uploads them to
PyPI.

```starlark
load("@rules_dx//deploy/rules:pypi.bzl", "pypi_deploy")

pypi_deploy(
    name = "pypi_demo",
    sdist = ":pypi_demo_sdist",
    wheel = ":pypi_demo_wheel",
)
```

- `name`: the distribution name.
- `wheel`: required. One `.whl` file.
- `sdist`: optional. Must match the wheel's project and version.
- `repository_url`: defaults to `https://upload.pypi.org/legacy/`.
- `profile`: `debug`, `dev` or `release`. Defaults to `release`.

Default run stages `<name>-wheelhouse/` with a `simple-index/`. No
credentials needed.

Upload with `twine` on `PATH`:

```
PYPI_PUBLISH_LIVE=1 PYPI_PUBLISH_APPROVED=1 PYPI_API_TOKEN=... \
dx deploy --apply //pkg:pypi_demo
```

# Common behavior

Except for `cloudflare_pages_deploy`, a default run writes into the
workspace root, or into the directory given as the first argument. A live
run without its credentials or its `*_APPROVED=1` variable is refused. Exit
codes: `0` staged or published, `1` any failure or refusal.

Every launcher, `npm_deploy` included, rejects arguments beyond that output
directory. Publisher tools (`twine`, `dotnet`, `cargo`, `gh`, `mvn`, `docker`,
`octo`, `npm`) run with a cleared environment plus a per-tool keep-list.
Secrets reach them only through that environment, stdin or an owner-only temp
file, never argv. The exceptions are the promotion `PROMOTION_HEALTH_CMD`, the
`docker context inspect` endpoint probe, and `wrangler`, which has its own
narrower list.
