"""Gem layout of the pinned SimpleCov and benchmark-ips gems (#3954); versions and digests are in pins.json."""

load("@dx_pins_data//:pins.bzl", "PINS")

RUBY_COVERAGE_REPOSITORY = "dx_ruby_coverage_gems"

RUBY_BENCH_REPOSITORY = "dx_ruby_bench_gems"

RUBY_COVERAGE_CURRENCY_RECHECK = "2026-10-27"

# Gem name to the file under lib/ that `require` resolves, in load order.
_COVERAGE_ENTRIES = [
    ("simplecov", "simplecov.rb"),
    ("simplecov-lcov", "simplecov-lcov.rb"),
    ("docile", "docile.rb"),
    ("simplecov-html", "simplecov-html.rb"),
    ("simplecov_json_formatter", "simplecov_json_formatter.rb"),
]

_BENCH_ENTRIES = [("benchmark-ips", "benchmark/ips.rb")]

def _gems(entries):
    archives = PINS["dx_ruby_gems"]["archives"]
    return [
        struct(
            name = name,
            dir = name + "-" + archives[name]["version"],
            entry = entry,
            sha256 = archives[name]["sha256"],
            url = archives[name]["urls"][0],
            version = archives[name]["version"],
        )
        for name, entry in entries
    ]

RUBY_COVERAGE_GEMS = _gems(_COVERAGE_ENTRIES)

RUBY_BENCH_GEMS = _gems(_BENCH_ENTRIES)
