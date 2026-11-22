"""Public Python module extensions."""

load("@rules_py//uv:extensions.bzl", _uv = "uv")
load("//python/private:interpreters.bzl", _python = "python")

python = _python
uv = _uv
