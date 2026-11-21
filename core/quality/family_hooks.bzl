"""Hook helpers shared by the quality family files."""

load("//core/quality:pipeline.bzl", "drop_pipeline_tool")
load("//core/quality:real_core.bzl", "real_context")

visibility(["//bicep/...", "//cc/...", "//core/...", "//dart/...", "//dotnet/...", "//erlang/...", "//go/...", "//haskell/...", "//java/...", "//javascript/...", "//julia/...", "//kubernetes/...", "//lua/...", "//ocaml/...", "//ruby/...", "//rust/...", "//scala/...", "//terraform/..."])

def stage(files, inputs, rel, f):
    """Stages f at rel for the tool and lists it as an action input."""
    files.append((rel, f))
    inputs.append(f)

def without(resolved, tool):
    """Context that drops a tool from the pipeline."""
    return real_context(resolved = drop_pipeline_tool(resolved, tool))

def default_files_hook(extensions, only_capability = None, except_capability = None):
    def _hook(target, _ctx, _resolved, capability, _what, _shared):
        if only_capability != None and capability != only_capability:
            return real_context()
        if capability == except_capability:
            return real_context()
        return real_context(resolve_lists = [(target[DefaultInfo].files, extensions)])

    return _hook

def upstream_files(srcs, kind):
    """Source recognizer result for an upstream target's srcs, or an ownership error."""
    if srcs == None:
        return struct(files = [], error = "upstream " + kind + " target has no readable srcs; unsupported ownership is unchecked, not clean")
    return struct(files = list(srcs), error = "")
