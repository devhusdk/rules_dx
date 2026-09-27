"""Pinned Ruby foundation."""

load(":versions.bzl", _RUBY_VERSION = "RUBY_VERSION", _RULES_RUBY_VERSION = "RULES_RUBY_VERSION")

RULES_RUBY_VERSION = _RULES_RUBY_VERSION
RUBY_VERSION = _RUBY_VERSION

GEMFILE = "//third_party/ruby:Gemfile"
GEMFILE_LOCK = "//third_party/ruby:Gemfile.lock"
BUNDLE_HUB = "@bundle"

RSPEC_VERSION = "3.13.0"

GEM_CHECKSUMS = {
    "diff-lcs-1.5.0": "49b934001c8c6aedb37ba19daec5c634da27b318a7a3c654ae979d6ba1929b67",
    "rspec-3.13.0": "d490914ac1d5a5a64a0e1400c1d54ddd2a501324d703b8cfe83f458337bab993",
    "rspec-core-3.13.0": "557792b4e88da883d580342b263d9652b6a10a12d5bda9ef967b01a48f15454c",
    "rspec-expectations-3.13.0": "621d48c62262f955421eaa418130744760802cad47e781df70dba4d9f897102e",
    "rspec-mocks-3.13.0": "735a891215758d77cdb5f4721fffc21078793959d1f0ee4a961874311d9b7f66",
    "rspec-support-3.13.1": "48877d4f15b772b7538f3693c22225f2eda490ba65a0515c4e7cf6f2f17de70f",
}
