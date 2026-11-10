"""Pinned SimpleCov gems that Ruby tests load under `bazel coverage` (#3954)."""

load("//core:repo.bzl", "gem_extract")
load(":pins.bzl", "RUBY_BENCH_GEMS", "RUBY_COVERAGE_GEMS", "gem_dir", "gem_url")

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
        "    ], allow_empty = True),",
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

def _download_gems(ctx, gems):
    for gem in gems:
        url = gem_url(gem)
        gem_extract(
            ctx,
            url = url,
            sha256 = gem.sha256,
            canonical_id = "dx-ruby-coverage-gem:" + url,
            output = gem_dir(gem),
            required = ["lib/" + gem.entry],
        )
    ctx.file("BUILD.bazel", _build_file(gems))
    return ctx.repo_metadata(reproducible = True)

def _ruby_coverage_gems_impl(ctx):
    return _download_gems(ctx, RUBY_COVERAGE_GEMS)

def _ruby_bench_gems_impl(ctx):
    return _download_gems(ctx, RUBY_BENCH_GEMS)

ruby_coverage_gems = repository_rule(
    implementation = _ruby_coverage_gems_impl,
)

ruby_bench_gems = repository_rule(
    implementation = _ruby_bench_gems_impl,
)
