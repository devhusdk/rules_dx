"""Signing and attestation selection for releases."""

load("//deploy/release:cosign_pins.bzl", "COSIGN_VERSION")
load("//deploy/release:tool.bzl", "RELEASE_DEPLOY_TOOL")
load("//deploy/rules:config.bzl", "deploy_config", "rlocation_path")
load("//deploy/rules:defs.bzl", "dx_deployment")

SIGNING_TRUST_ROOT = "https://tuf-repo-cdn.sigstore.dev"
SIGNING_ISSUER = "https://token.actions.githubusercontent.com"

SIGNING_COSIGN_VERSION = COSIGN_VERSION
_COSIGN = Label("//deploy/release:cosign")
SIGNING_BUNDLE_MEDIA_TYPE = "application/vnd.dev.sigstore.bundle.v0.3+json"

def signing_cosign_error(version):
    """Validates the expected cosign CLI version pin."""
    if version != SIGNING_COSIGN_VERSION:
        return ("signing: invalid cosign version '" + str(version) +
                "': want '" + SIGNING_COSIGN_VERSION + "' (pinned)")
    return ""

def signing_bundle_media_error(media_type):
    """Validates the expected Sigstore bundle media type."""
    if media_type != SIGNING_BUNDLE_MEDIA_TYPE:
        return ("signing: invalid bundle media type '" + str(media_type) +
                "': want '" + SIGNING_BUNDLE_MEDIA_TYPE + "' (Sigstore bundle v0.3; 0.1/0.2 only if declared)")
    return ""

def signing_identity_error(identity, issuer):
    """Validates the expected certificate identity + issuer."""
    if type(identity) != "string" or identity == "":
        return ("signing: invalid identity '" + str(identity) +
                "': want the owner-approved release workflow identity")
    if issuer != SIGNING_ISSUER:
        return ("signing: invalid issuer '" + str(issuer) +
                "': want '" + SIGNING_ISSUER + "' (Sigstore keyless via GitHub OIDC)")
    return ""

def signing_bundle_names(name):
    """Returns the deterministic bundle output names."""
    return (name + ".bundle", name + ".attestation")

CosignInfo = provider(
    doc = "A pinned cosign binary and its sha256.",
    fields = {
        "file": "The cosign binary File.",
        "sha256": "The pinned sha256 of the binary.",
    },
)

def _cosign_binary_impl(ctx):
    """Pairs a pinned cosign binary with its sha256."""
    f = ctx.file.binary
    return [
        DefaultInfo(files = depset([f]), runfiles = ctx.runfiles(files = [f])),
        CosignInfo(file = f, sha256 = ctx.attr.sha256),
    ]

cosign_binary = rule(
    implementation = _cosign_binary_impl,
    doc = "Pairs a pinned cosign binary with its sha256.",
    attrs = {
        "binary": attr.label(
            doc = "Pinned cosign executable file provided by this target.",
            mandatory = True,
            allow_single_file = True,
        ),
        "sha256": attr.string(
            doc = "Expected sha256 digest of the pinned cosign binary.",
            mandatory = True,
        ),
    },
    provides = [CosignInfo],
)

def _signing_config_impl(ctx):
    """Writes the owner-gated signing deploy config."""
    asset_rlocs = []
    files = []
    runfiles = ctx.runfiles()
    for target in ctx.attr.artifacts:
        info = target[DefaultInfo]
        f = info.files_to_run.executable
        if f == None:
            fl = info.files.to_list()
            if len(fl) != 1:
                fail("signed_release " + str(ctx.label) + ": artifact " +
                     str(target.label) + " provides " +
                     str(len(fl)) + " files, want exactly one")
            f = fl[0]
        asset_rlocs.append(rlocation_path(ctx, f))
        files.append(f)
        runfiles = runfiles.merge(info.default_runfiles)
    cosign = ctx.attr.cosign[CosignInfo]
    runfiles = runfiles.merge(ctx.attr.cosign[DefaultInfo].default_runfiles)
    return deploy_config(
        ctx,
        "signing",
        {
            "asset_rlocs": asset_rlocs,
            "cosign_rloc": rlocation_path(ctx, cosign.file),
            "cosign_sha256": cosign.sha256,
            "cosign_version": ctx.attr.cosign_version,
            "identity": ctx.attr.identity,
            "issuer": ctx.attr.issuer,
        },
        runfiles.merge(ctx.runfiles(files = files + [cosign.file])),
    )

_signing_config = rule(
    implementation = _signing_config_impl,
    doc = "Writes the owner-gated signing deploy config.",
    attrs = {
        "artifacts": attr.label_list(
            doc = "Targets the signing deploy signs, one asset each.",
            mandatory = True,
        ),
        "cosign": attr.label(
            doc = "Pinned cosign binary target providing the signing tool.",
            mandatory = True,
            cfg = "exec",
            providers = [CosignInfo],
        ),
        "cosign_version": attr.string(
            doc = "Cosign release version recorded in the signing config.",
            mandatory = True,
        ),
        "identity": attr.string(
            doc = "Certificate identity the signatures must match.",
            mandatory = True,
        ),
        "issuer": attr.string(
            doc = "OIDC issuer the signing certificate must come from.",
            mandatory = True,
        ),
    },
)

def signed_release(name, artifacts, identity, issuer = SIGNING_ISSUER, profile = "release", cosign_version = SIGNING_COSIGN_VERSION):
    """Creates an owner-gated signing deploy target for pinned artifacts."""
    err = signing_identity_error(identity, issuer)
    if err != "":
        fail(err + " (in " + native.package_name() + ":" + name + ")")
    version_err = signing_cosign_error(cosign_version)
    if version_err != "":
        fail(version_err + " (in " + native.package_name() + ":" + name + ")")
    if len(artifacts) == 0:
        fail("signed_release " + native.package_name() + ":" + name + ": need at least one artifact")
    config_target = name + "_config"
    _signing_config(
        name = config_target,
        artifacts = artifacts,
        cosign = _COSIGN,
        cosign_version = cosign_version,
        identity = identity,
        issuer = issuer,
    )
    dx_deployment(
        name = name,
        config = ":" + config_target,
        deploy = RELEASE_DEPLOY_TOOL,
        profile = profile,
    )
