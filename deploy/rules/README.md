# maven_deploy

Default run stages a local file repository. No credentials needed.

Live upload goes to the Central Portal OSSRH staging API:
`https://ossrh-staging-api.central.sonatype.com/service/local/staging/deploy/maven2/`

Setup:

- A Central Portal account with a verified namespace.
- A Portal user token for `MAVEN_USERNAME` and `MAVEN_PASSWORD`.
- `mvn` and `gpg` on `PATH`.

Run:

```
MAVEN_PUBLISH_LIVE=1 MAVEN_PUBLISH_APPROVED=1 \
MAVEN_USERNAME=... MAVEN_PASSWORD=... MAVEN_GPG_PASSPHRASE=... \
bazel run //pkg:maven_demo
```

Upload only stages the deployment. Validate and publish it in the Central
Portal. `oss.sonatype.org` and `s01.oss.sonatype.org` are rejected.
Unsigned uploads need `MAVEN_ALLOW_UNSIGNED=1`.

# npm_deploy

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
