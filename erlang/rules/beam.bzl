"""The BeamInfo every BEAM language's app rule exports for dialyzer."""

load("@rules_erlang//:erlang_app_info.bzl", "ErlangAppInfo")
load(":providers.bzl", "BeamInfo")

def _beams(info):
    """The beams of one ErlangAppInfo: listed .beam files and ebin directories."""
    return [f for f in info.beam if f.is_directory or f.extension == "beam"]

def beam_info(upstream, plts, code = []):
    """Returns the BeamInfo of an ErlangAppInfo target."""
    info = upstream[ErlangAppInfo]
    return BeamInfo(
        app_name = info.app_name,
        owned = depset(_beams(info)),
        deps = depset([b for dep in info.deps for b in _beams(dep[ErlangAppInfo])]),
        extra_apps = info.extra_apps,
        plts = plts,
        code = code,
    )
