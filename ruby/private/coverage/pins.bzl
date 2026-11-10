"""SimpleCov 0.22.0 pins for Ruby coverage under `bazel coverage` (#3954); the gem data is in pins.json."""

load("@dx_pins_data//:pins.bzl", "DATA")

_ARCHIVES = DATA["dx_ruby_coverage"]["archives"]

def _gem(name, entry):
    pin = _ARCHIVES[name]
    return struct(name = name, version = pin["version"], sha256 = pin["sha256"], urls = pin["urls"], entry = entry)

SIMPLECOV_VERSION = _ARCHIVES["simplecov"]["version"]

# simplecov's runtime dependencies plus the LCOV formatter rules_ruby's coverage helper loads.
# entry is the file under lib/ that `require` resolves.
RUBY_COVERAGE_GEMS = [
    _gem("simplecov", "simplecov.rb"),
    _gem("simplecov-lcov", "simplecov-lcov.rb"),
    _gem("docile", "docile.rb"),
    _gem("simplecov-html", "simplecov-html.rb"),
    _gem("simplecov_json_formatter", "simplecov_json_formatter.rb"),
]

RUBY_COVERAGE_REPOSITORY = "dx_ruby_coverage_gems"

# benchmark-ips for ruby_bench; pure Ruby, no runtime dependencies.
RUBY_BENCH_GEMS = [
    _gem("benchmark-ips", "benchmark/ips.rb"),
]

RUBY_BENCH_REPOSITORY = "dx_ruby_bench_gems"

def gem_dir(gem):
    return gem.name + "-" + gem.version

RUBY_COVERAGE_CURRENCY_RECHECK = "2026-10-27"
