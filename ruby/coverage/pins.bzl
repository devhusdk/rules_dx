"""SimpleCov 0.22.0 pins for Ruby coverage under `bazel coverage` (#3954)."""

SIMPLECOV_VERSION = "0.22.0"

# simplecov 0.22.0's runtime dependencies plus the LCOV formatter rules_ruby's
# coverage helper loads. Each entry: gem name, version, sha256 of the .gem,
# and the file under lib/ that `require` resolves.
RUBY_COVERAGE_GEMS = [
    struct(name = "simplecov", version = SIMPLECOV_VERSION, sha256 = "fe2622c7834ff23b98066bb0a854284b2729a569ac659f82621fc22ef36213a5", entry = "simplecov.rb"),
    struct(name = "simplecov-lcov", version = "0.9.0", sha256 = "7a77a31e200a595ed4b0249493056efd0c920601f53d2ef135ca34ee796346cd", entry = "simplecov-lcov.rb"),
    struct(name = "docile", version = "1.4.1", sha256 = "96159be799bfa73cdb721b840e9802126e4e03dfc26863db73647204c727f21e", entry = "docile.rb"),
    struct(name = "simplecov-html", version = "0.13.2", sha256 = "bd0b8e54e7c2d7685927e8d6286466359b6f16b18cb0df47b508e8d73c777246", entry = "simplecov-html.rb"),
    struct(name = "simplecov_json_formatter", version = "0.1.4", sha256 = "529418fbe8de1713ac2b2d612aa3daa56d316975d307244399fa4838c601b428", entry = "simplecov_json_formatter.rb"),
]

RUBY_COVERAGE_REPOSITORY = "dx_ruby_coverage_gems"

def gem_url(gem):
    return "https://rubygems.org/downloads/" + gem.name + "-" + gem.version + ".gem"

def gem_dir(gem):
    return gem.name + "-" + gem.version

RUBY_COVERAGE_CURRENCY_RECHECK = "2026-10-27"
