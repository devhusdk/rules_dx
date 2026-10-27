"""Signing and attestation selection for releases."""

load("//deploy/release:cosign_pins.bzl", "COSIGN_VERSION")
load("//deploy/rules:defs.bzl", "dx_deployment")
load("//deploy/rules:launcher.bzl", "rlocation_path")
load("//rust/rules:defs.bzl", "rust_binary")

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
        "binary": attr.label(mandatory = True, allow_single_file = True),
        "sha256": attr.string(mandatory = True),
    },
    provides = [CosignInfo],
)

def _signing_launcher_impl(ctx):
    """Writes the owner-gated signing deploy launcher Rust source."""
    asset_rlocs = []
    for target in ctx.attr.artifacts:
        info = target[DefaultInfo]
        f = info.files_to_run.executable
        if f == None:
            files = info.files.to_list()
            if len(files) != 1:
                fail("signed_release " + str(ctx.label) + ": artifact " +
                     str(target.label) + " provides " +
                     str(len(files)) + " files, want exactly one")
            f = files[0]
        rloc = rlocation_path(ctx, f)
        for banned in ["\"", "\\", "\n"]:
            if banned in rloc:
                fail("signed_release " + str(ctx.label) + ": rlocation '" + rloc +
                     "' is not launcher-safe")
        asset_rlocs.append(rloc)
    cosign = ctx.attr.cosign[CosignInfo]
    cosign_rloc = rlocation_path(ctx, cosign.file)
    for value in [ctx.attr.identity, ctx.attr.issuer, ctx.attr.cosign_version, cosign.sha256, cosign_rloc]:
        for banned in ["\"", "\\", "\n"]:
            if banned in value:
                fail("signed_release " + str(ctx.label) + ": value '" + value +
                     "' is not launcher-safe")
    rloc_list = ", ".join(["\"" + r + "\"" for r in asset_rlocs])
    launcher = ctx.actions.declare_file(ctx.label.name + ".rs")
    ctx.actions.write(
        output = launcher,
        content = """// Deploy launcher for `signed_release`. Generated. Do not edit.
fn run() -> i32 {
    const IDENTITY: &str = \"""" + ctx.attr.identity + """\";
    const ISSUER: &str = \"""" + ctx.attr.issuer + """\";
    const COSIGN_RLOC: &str = \"""" + cosign_rloc + """\";
    const COSIGN_VERSION: &str = \"""" + ctx.attr.cosign_version + """\";
    const COSIGN_SHA256: &str = \"""" + cosign.sha256 + """\";
    const ASSET_RLOCS: &[&str] = &[""" + rloc_list + """];
    if std::env::args_os().len() > 2 {
        eprintln!("signing: this deploy target takes at most an output directory; artifacts are pinned at analysis time");
        return 1;
    }
    let argv: Vec<String> = std::env::args().collect();
    let outdir = dx_deploy_tools::deploy_outdir(&argv);
    let dry = std::env::var("RELEASE_SIGN_DRY_RUN").unwrap_or_default() == "1";
    let runfiles = match runfiles::Runfiles::create() {
        Ok(runfiles) => runfiles,
        Err(error) => {
            eprintln!("signing: cannot load runfiles: {error}");
            return 1;
        }
    };
    let mut assets = Vec::with_capacity(ASSET_RLOCS.len());
    for rloc in ASSET_RLOCS {
        match runfiles.rlocation(rloc) {
            Some(path) => assets.push(path.to_string_lossy().into_owned()),
            None => {
                eprintln!("signing: runfile not found for '{rloc}'");
                return 1;
            }
        }
    }
    let Some(cosign) = runfiles.rlocation(COSIGN_RLOC) else {
        eprintln!("signing: runfile not found for '{COSIGN_RLOC}'");
        return 1;
    };
    let cosign = cosign.to_string_lossy().into_owned();
    let pin = dx_release_tools::CosignPin {
        path: &cosign,
        version: COSIGN_VERSION,
        sha256: COSIGN_SHA256,
    };
    match dx_release_tools::signing_run(IDENTITY, ISSUER, &assets, dry, &pin, &outdir) {
        Ok(text) => {
            print!("{text}");
            0
        }
        Err(diagnostic) => {
            eprintln!("{diagnostic}");
            1
        }
    }
}
fn main() {
    std::process::exit(run());
}
""",
    )
    return [DefaultInfo(files = depset([launcher]))]

_signing_launcher = rule(
    implementation = _signing_launcher_impl,
    attrs = {
        "artifacts": attr.label_list(mandatory = True),
        "cosign": attr.label(mandatory = True, providers = [CosignInfo]),
        "cosign_version": attr.string(mandatory = True),
        "identity": attr.string(mandatory = True),
        "issuer": attr.string(mandatory = True),
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
    program_target = name + "_program"
    launcher_target = program_target + "_launcher"
    _signing_launcher(
        name = launcher_target,
        artifacts = artifacts,
        cosign = _COSIGN,
        cosign_version = cosign_version,
        identity = identity,
        issuer = issuer,
    )
    rust_binary(
        name = program_target,
        srcs = [":" + launcher_target],
        crate_name = program_target.replace("-", "_"),
        data = artifacts + [_COSIGN],
        edition = "2021",
        deps = [
            "//deploy/release:dx_release_tools",
            "//deploy/rules:dx_deploy_tools",
            "@rules_rust//rust/runfiles",
        ],
    )
    dx_deployment(
        name = name,
        deploy = ":" + program_target,
        profile = profile,
    )
