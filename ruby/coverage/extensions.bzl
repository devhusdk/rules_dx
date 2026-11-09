"""Module extension for ruby/coverage."""

load(":pins.bzl", "RUBY_BENCH_REPOSITORY", "RUBY_COVERAGE_REPOSITORY")
load(":repos.bzl", "ruby_bench_gems", "ruby_coverage_gems")


def _ruby_coverage_impl(module_ctx):
    if len(module_ctx.modules) == 0:
        fail("ruby gems: the extension is only reachable from a module graph")
    ruby_coverage_gems(name = RUBY_COVERAGE_REPOSITORY)
    ruby_bench_gems(name = RUBY_BENCH_REPOSITORY)
    return module_ctx.extension_metadata(reproducible = True)


ruby_coverage = module_extension(
    doc = "Downloads the pinned, checksum-verified SimpleCov gems as @dx_ruby_coverage_gems and benchmark-ips as @dx_ruby_bench_gems.",
    implementation = _ruby_coverage_impl,
)
