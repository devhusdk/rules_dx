"""Pinned SimpleCov gems that Ruby tests load under `bazel coverage` (#3954)."""

load(":pins.bzl", "RUBY_COVERAGE_GEMS", "RUBY_COVERAGE_REPOSITORY", "gem_dir", "gem_url")

def _build_file(gems):
    dirs = [gem_dir(gem) for gem in gems]
    lines = [
        "filegroup(",
        "    name = \"lib\",",
        "    srcs = glob([",
    ]
    for d in dirs:
        lines.append("        \"" + d + "/lib/**\",")
    lines += [
        "        \"simplecov-html-*/public/**\",",
        "        \"simplecov-html-*/views/**\",",
        "    ]),",
        "    visibility = [\"//visibility:public\"],",
        ")",
        "",
        "exports_files(",
        "    glob([\"*/LICENSE*\"]) + [",
    ]
    for gem in gems:
        lines.append("        \"" + gem_dir(gem) + "/lib/" + gem.entry + "\",")
    lines += [
        "    ],",
        "    visibility = [\"//visibility:public\"],",
        ")",
        "",
    ]
    return "\n".join(lines)

def _ruby_coverage_gems_impl(ctx):
    for gem in RUBY_COVERAGE_GEMS:
        url = gem_url(gem)
        tmp = "_gem_" + gem.name
        ctx.download_and_extract(
            url = url,
            output = tmp,
            sha256 = gem.sha256,
            type = "tar",
            canonical_id = "dx-ruby-coverage-gem:" + url,
        )
        data = tmp + "/data.tar.gz"
        if not ctx.path(data).exists:
            fail("ruby coverage: '" + url + "' holds no data.tar.gz")
        ctx.extract(archive = data, output = gem_dir(gem))
        entry = gem_dir(gem) + "/lib/" + gem.entry
        if not ctx.path(entry).exists:
            fail("ruby coverage: '" + url + "' holds no lib/" + gem.entry)
        ctx.delete(tmp)
    ctx.file("BUILD.bazel", _build_file(RUBY_COVERAGE_GEMS))
    return ctx.repo_metadata(reproducible = True)

_ruby_coverage_gems = repository_rule(
    implementation = _ruby_coverage_gems_impl,
)

def _ruby_coverage_impl(module_ctx):
    if len(module_ctx.modules) == 0:
        fail("ruby coverage: the extension is only reachable from a module graph")
    _ruby_coverage_gems(name = RUBY_COVERAGE_REPOSITORY)
    return module_ctx.extension_metadata(reproducible = True)

ruby_coverage = module_extension(
    doc = "Downloads the pinned, checksum-verified SimpleCov gems as @dx_ruby_coverage_gems.",
    implementation = _ruby_coverage_impl,
)
