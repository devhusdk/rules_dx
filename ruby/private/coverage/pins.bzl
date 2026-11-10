"""SimpleCov and benchmark-ips gem pins read from pins.json."""

load("@dx_pins_data//:pins.bzl", "DATA")

_ARCHIVES = DATA["dx_ruby_coverage"]["archives"]

def _gem(name, entry):
    pin = _ARCHIVES[name]
    return struct(name = name, version = pin["version"], sha256 = pin["sha256"], urls = pin["urls"], entry = entry)

SIMPLECOV_VERSION = _ARCHIVES["simplecov"]["version"]

RUBY_COVERAGE_GEMS = [
    _gem("simplecov", "simplecov.rb"),
    _gem("simplecov-lcov", "simplecov-lcov.rb"),
    _gem("docile", "docile.rb"),
    _gem("simplecov-html", "simplecov-html.rb"),
    _gem("simplecov_json_formatter", "simplecov_json_formatter.rb"),
]

RUBY_COVERAGE_REPOSITORY = "dx_ruby_coverage_gems"

RUBY_BENCH_GEMS = [
    _gem("benchmark-ips", "benchmark/ips.rb"),
]

RUBY_BENCH_REPOSITORY = "dx_ruby_bench_gems"

def gem_dir(gem):
    return gem.name + "-" + gem.version

RUBY_COVERAGE_CURRENCY_RECHECK = "2026-10-27"
