load("//libs/starlark:cells.bzl", "cells_compatible_with")
load("//libs/starlark:wrapper.bzl", "dx_binary_forward_kwargs", "dx_test_forward_kwargs", "dx_upstream_tags")
load("//ocaml/toolchain:pins.bzl", "OCAML_CELLS")

OCAML_COMPATIBLE_WITH = cells_compatible_with(OCAML_CELLS, "//ocaml/toolchain", sorted(OCAML_CELLS.keys()))

COVERAGE_ACTIVE = str(Label("//tools/coverage/ocaml:coverage_active"))

BISECT_DRIVER = str(Label("//tools/coverage/ocaml:bisect_driver"))

BISECT_RUNTIME = str(Label("//tools/coverage/ocaml:bisect_runtime"))

OCAML_LINK_OPTS = ["-without-runtime"]

LOCAL_TAGS = ["no-remote-exec"]

def ocaml_upstream_tags(tags):
    """Returns private upstream tags that keep OCaml actions on this host."""
    out = dx_upstream_tags(tags)
    return out + [tag for tag in LOCAL_TAGS if tag not in out]

def ocaml_with_defaults(kwargs):
    """Returns kwargs with the qualified OCaml cell as the default."""
    out = dict(kwargs)
    if out.get("target_compatible_with") == None:
        out["target_compatible_with"] = OCAML_COMPATIBLE_WITH
    return out

def ocaml_link_kwargs(kwargs):
    """Returns binary kwargs that link plain bytecode for the runfiles ocamlrun when the emitter is vm; native links take no extra opts."""
    vm_opts = select({
        Label("//ocaml/toolchain:emitter_vm"): OCAML_LINK_OPTS,
        "//conditions:default": [],
    })
    out = dict(kwargs)
    opts = out.get("opts")
    out["opts"] = vm_opts if opts == None else opts + vm_opts
    if out.get("vm_linkage") == None:
        out["vm_linkage"] = "dynamic"
    return out

TEST_ONLY_ATTRS = ["env", "env_inherit", "flaky", "local", "shard_count", "size", "testonly", "timeout"]

RUN_ENV_ATTRS = ["env", "env_inherit"]

def ocaml_test_upstream_kwargs(kwargs):
    """Returns the test kwargs the upstream executable accepts."""
    return {key: value for key, value in kwargs.items() if key not in TEST_ONLY_ATTRS}

def ocaml_test_forward_kwargs(kwargs):
    """Returns the test forwarder kwargs, including env and env_inherit, which the forwarder applies itself."""
    out = dx_test_forward_kwargs(kwargs)
    for key in RUN_ENV_ATTRS:
        if kwargs.get(key) != None:
            out[key] = kwargs[key]
    return out

def ocaml_binary_upstream_kwargs(kwargs):
    """Returns the binary kwargs the upstream executable accepts."""
    return {key: value for key, value in kwargs.items() if key not in RUN_ENV_ATTRS}

def ocaml_binary_forward_kwargs(kwargs):
    """Returns the binary forwarder kwargs, including env and env_inherit, which the forwarder applies itself."""
    out = dx_binary_forward_kwargs(kwargs)
    for key in RUN_ENV_ATTRS:
        if kwargs.get(key) != None:
            out[key] = kwargs[key]
    return out

def ocaml_default_module_name(name, sig):
    """Returns the module_name default: the target name unless sig names a signature target."""
    if sig == None or (type(sig) == "string" and sig.endswith(".mli")):
        return name
    return None

def ocaml_coverage_kwargs(kwargs, struct, package, coverage_ppx = False):
    """Returns kwargs that run the bisect_ppx instrumenter over struct when `bazel coverage` runs on Linux x86_64."""
    if type(struct) != "string" or not struct.endswith(".ml"):
        return kwargs
    own_ppx = kwargs.get("ppx") != None
    if own_ppx and not coverage_ppx:
        return kwargs
    out = dict(kwargs)
    if not own_ppx:
        out["ppx"] = select({COVERAGE_ACTIVE: BISECT_DRIVER, "//conditions:default": None})
    loc_args = select({COVERAGE_ACTIVE: ["-loc-filename", package + "/" + struct], "//conditions:default": []})
    out["ppx_args"] = (out.get("ppx_args") or []) + loc_args
    out["deps"] = (out.get("deps") or []) + select({COVERAGE_ACTIVE: [BISECT_RUNTIME], "//conditions:default": []})
    out["opts"] = (out.get("opts") or []) + select({COVERAGE_ACTIVE: ["-w", "-33"], "//conditions:default": []})
    return out
