"""Explicit-apply Cloudflare Pages publisher for dx deploy."""

load("@bazel_skylib//lib:shell.bzl", "shell")
load(":config.bzl", "DEPLOY_TOOL", "deploy_attrs", "deploy_config", "deployment_budget", "rlocation_path")
load(":deployment.bzl", "dx_deployment")
load(":validate_cloudflare.bzl", "cloudflare_pages_error")

_VERIFY = Label("//deploy/rules:cloudflare_verify")

def cloudflare_pages_intent(branch, production_branch):
    """Returns 'production' or 'preview' for a branch."""
    return "production" if branch == production_branch else "preview"

def _cloudflare_config_impl(ctx):
    """Writes the deploy config for one Pages deployment."""
    site = ctx.attr.site[DefaultInfo].files.to_list()
    if len(site) != 1 or not site[0].is_directory:
        fail("cloudflare_pages_deploy " + str(ctx.label) + ": site " +
             str(ctx.attr.site.label) + " must provide exactly one directory " +
             "(for example a javascript_web_bundle)")
    wrangler_info = ctx.attr.wrangler[DefaultInfo]
    wrangler = wrangler_info.files_to_run.executable
    if wrangler == None:
        fail("cloudflare_pages_deploy " + str(ctx.label) + ": wrangler " +
             str(ctx.attr.wrangler.label) + " has no executable")
    return deploy_config(
        ctx,
        "cloudflare_pages",
        {
            "account_id": ctx.attr.account_id,
            "branch": ctx.attr.branch,
            "deploy_name": ctx.attr.deploy_name,
            "production_branch": ctx.attr.production_branch,
            "project": ctx.attr.project,
            "site_rloc": rlocation_path(ctx, site[0]),
            "wrangler_rloc": rlocation_path(ctx, wrangler),
        },
        ctx.runfiles(files = [site[0], wrangler]).merge(wrangler_info.default_runfiles),
    )

_cloudflare_config = rule(
    doc = "Writes the deploy config for one Pages deployment.",
    implementation = _cloudflare_config_impl,
    attrs = {
        "account_id": attr.string(mandatory = True, doc = "Cloudflare account id."),
        "branch": attr.string(mandatory = True, doc = "Branch to deploy."),
        "deploy_name": attr.string(mandatory = True, doc = "Deploy target name."),
        "production_branch": attr.string(mandatory = True, doc = "Project production branch."),
        "project": attr.string(mandatory = True, doc = "Pages project name."),
        "site": attr.label(mandatory = True, allow_files = True, doc = "Static site directory."),
        "wrangler": attr.label(mandatory = True, cfg = "exec", executable = True, doc = "Pinned Wrangler executable."),
    },
)

def cloudflare_pages_deploy(name, account_id, project, branch, production_branch, site, wrangler = None, build_mode = "release", sbom = None, size_budget = "", tags = [], visibility = None, testonly = False, target_compatible_with = []):
    """Publishes a static bundle directory to an existing Cloudflare Pages project."""
    attrs = deploy_attrs(tags, visibility, testonly, target_compatible_with)
    wrangler = wrangler or str(Label("//quality/tools/javascript/bin:wrangler"))
    error = cloudflare_pages_error(account_id, project, branch, production_branch)
    if error != "":
        fail(error + " (in " + native.package_name() + ":" + name + ")")

    check_target = name + "_check"
    config_target = name + "_config"

    native.genrule(
        name = check_target,
        srcs = [site],
        outs = [name + ".pages.json"],
        tools = [_VERIFY],
        cmd = "$(location " + str(_VERIFY) + ") " + " ".join([
            shell.quote(account_id),
            shell.quote(project),
            shell.quote(branch),
            shell.quote(production_branch),
            "$(execpath " + site + ")",
            "$@",
        ]),
        **attrs
    )

    _cloudflare_config(
        name = config_target,
        account_id = account_id,
        branch = branch,
        deploy_name = name,
        production_branch = production_branch,
        project = project,
        site = site,
        wrangler = wrangler,
        **attrs
    )

    dx_deployment(
        name = name,
        sbom = sbom,
        config = ":" + config_target,
        data = [":" + check_target],
        deploy = DEPLOY_TOOL,
        build_mode = build_mode,
        **(deployment_budget(name, size_budget, [site]) | attrs)
    )
