"""Public Astro API."""

load(
    "//astro/private:defs.bzl",
    _astro_library = "astro_library",
    _astro_build = "astro_build",
    _astro_test = "astro_test",
)

astro_library = _astro_library
astro_build = _astro_build
astro_test = _astro_test
