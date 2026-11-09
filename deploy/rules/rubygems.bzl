"""Local-first RubyGems publisher for dx deploy."""

load(":config.bzl", "DEPLOY_TOOL", "deploy_attrs", "deploy_config", "deploy_identity", "deployment_budget", "rlocation_path")
load(":defs.bzl", "dx_deployment")
load(":validate.bzl", "ALNUM", "VERSION_CHARS", "only_chars")

_VALID_NAME_CHARS = ALNUM + "_.-"

_VALID_VERSION_CHARS = VERSION_CHARS

def rubygems_name_error(gem_name):
    """Validates one gem name value."""
    if type(gem_name) != "string" or gem_name == "":
        return ("rubygems_deploy: invalid gem name '" + str(gem_name) +
                "': want a non-empty name (for example 'rubygems_demo')")
    if not only_chars(gem_name, _VALID_NAME_CHARS):
        return ("rubygems_deploy: invalid gem name '" + gem_name +
                "': want only [A-Za-z0-9_.-]")
    return ""

def rubygems_version_error(version):
    """Validates one gem version value."""
    if type(version) != "string" or version == "":
        return ("rubygems_deploy: invalid version '" + str(version) +
                "': want a non-empty version (for example '0.0.0')")
    if not only_chars(version, _VALID_VERSION_CHARS):
        return ("rubygems_deploy: invalid version '" + version +
                "': want only [A-Za-z0-9._-+]")
    return ""

def _rubygems_config_impl(ctx):
    """Writes the deploy config for one RubyGems deployment."""
    gem_files = ctx.attr.gem[DefaultInfo].files.to_list()
    if len(gem_files) == 0:
        fail("rubygems_deploy " + str(ctx.label) + ": gem " +
             str(ctx.attr.gem.label) + " provides no files, want at least one source file")
    rlocs = [rlocation_path(ctx, f) for f in gem_files]
    ruby = ctx.file.ruby
    runfiles = ctx.runfiles(files = gem_files + ctx.files.ruby_files + ([ruby] if ruby else []))

    return deploy_config(
        ctx,
        "rubygems",
        {
            "gem_name": ctx.attr.gem_name,
            "gem_rlocs": rlocs,
            "ruby_rloc": rlocation_path(ctx, ruby) if ruby else "",
            "version": ctx.attr.version,
        },
        runfiles.merge(ctx.attr.gem[DefaultInfo].default_runfiles),
    )

_rubygems_config = rule(
    implementation = _rubygems_config_impl,
    doc = "Writes the deploy config for one RubyGems deployment.",
    attrs = {
        "gem": attr.label(
            doc = "Target whose files (the gemspec and its sources) are published by the deployment.",
            allow_files = True,
            mandatory = True,
        ),
        "gem_name": attr.string(
            doc = "Name the gem is published under on rubygems.org.",
            mandatory = True,
        ),
        "ruby": attr.label(
            doc = "Pinned ruby executable; its bin directory supplies gem. Empty takes gem from PATH.",
            allow_single_file = True,
            cfg = "exec",
        ),
        "ruby_files": attr.label(
            doc = "Files the pinned ruby needs at run time.",
            allow_files = True,
            cfg = "exec",
        ),
        "version": attr.string(
            doc = "Gem version the gemspec must declare, empty to publish the gemspec's version.",
        ),
    },
)

def rubygems_deploy(name, gem, package = "", version = "", ruby = None, ruby_files = None, build_mode = "release", sbom = None, size_budget = "", tags = [], visibility = None, testonly = False, target_compatible_with = []):
    """Publishes staged gem sources as a local-first RubyGems deployment."""
    attrs = deploy_attrs(tags, visibility, testonly, target_compatible_with)
    pkg, version = deploy_identity(name, package, version, rubygems_name_error, rubygems_version_error)

    config_target = name + "_config"
    _rubygems_config(
        name = config_target,
        gem = gem,
        gem_name = pkg,
        ruby = ruby,
        ruby_files = ruby_files,
        version = version,
        **attrs
    )

    dx_deployment(
        name = name,
        sbom = sbom,
        config = ":" + config_target,
        deploy = DEPLOY_TOOL,
        build_mode = build_mode,
        **(deployment_budget(name, size_budget, [gem]) | attrs)
    )
