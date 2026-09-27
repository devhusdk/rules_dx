"""RSpec 3.13.0 version pins."""

load("//modules:versions.bzl", _RUBY_VERSION = "RUBY_VERSION", _RULES_RUBY_VERSION = "RULES_RUBY_VERSION")

RULES_RUBY_VERSION = _RULES_RUBY_VERSION

RUBY_VERSION = _RUBY_VERSION

RSPEC_VERSION = "3.13.0"
RSPEC_CORE_VERSION = "3.13.0"
RSPEC_EXPECTATIONS_VERSION = "3.13.0"
RSPEC_MOCKS_VERSION = "3.13.0"
RSPEC_SUPPORT_VERSION = "3.13.1"
DIFF_LCS_VERSION = "1.5.0"

RSPEC_GEMS = [
    "rspec:3.13.0",
    "rspec-core:3.13.0",
    "rspec-expectations:3.13.0",
    "rspec-mocks:3.13.0",
    "rspec-support:3.13.1",
    "diff-lcs:1.5.0",
]

RSPEC_MAIN = "@bundle//bin:rspec"
RSPEC_KIND = "ruby_test"

RSPEC_FIXTURE_LIB = "//ruby/tests/fixtures/rspec:greeter_lib"
RSPEC_FIXTURE_TEST = "//ruby/tests/fixtures/rspec:greeter_spec"

RSPEC_REJECTED = "unpinned runner rejected: no floating version or head"

RSPEC_CURRENCY_RECHECK = "2026-09-22"
