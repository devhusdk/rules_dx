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
