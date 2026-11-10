"""Public Vue API."""

load(
    "//vue/private:defs.bzl",
    _vue_library = "vue_library",
    _vue_build = "vue_build",
    _vue_test = "vue_test",
)

vue_library = _vue_library
vue_build = _vue_build
vue_test = _vue_test
