"""Pinned Sorbet (sorbet-static) gems for the Ruby type check."""

SORBET_VERSION = "0.6.13599"

# Keyed by "<os>_<arch>" as the repository rule derives it from the host. The macOS gem is a
# universal (fat) binary, so both darwin cells share it. Sorbet ships no Windows build.
SORBET_CELLS = {
    "darwin_aarch64": struct(
        gem = "sorbet-static-" + SORBET_VERSION + "-universal-darwin.gem",
        sha256 = "c023f9bdc8b974640b9eb3960295ec8f41dcfa385c89a699ceec8c6ed328eea6",
    ),
    "darwin_x86_64": struct(
        gem = "sorbet-static-" + SORBET_VERSION + "-universal-darwin.gem",
        sha256 = "c023f9bdc8b974640b9eb3960295ec8f41dcfa385c89a699ceec8c6ed328eea6",
    ),
    "linux_aarch64": struct(
        gem = "sorbet-static-" + SORBET_VERSION + "-aarch64-linux.gem",
        sha256 = "797fb456360f91dd40c5eb1eaecee630742c2de112a76e3787506040fb750864",
    ),
    "linux_x86_64": struct(
        gem = "sorbet-static-" + SORBET_VERSION + "-x86_64-linux.gem",
        sha256 = "6fbb673e4d69b344bf5b6bec916820f87be08a351b2e4ff33df126345b1e8a70",
    ),
}

SORBET_BINARY = "libexec/sorbet"

def sorbet_url(gem):
    """Returns the rubygems download URL of one pinned gem file."""
    return "https://rubygems.org/downloads/" + gem
