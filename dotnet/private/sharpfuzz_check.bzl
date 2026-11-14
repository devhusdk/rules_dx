"""Analysis test for SharpFuzz instrumentation across dx library forwarders."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("@rules_dotnet//dotnet/private:providers.bzl", "DotnetAssemblyRuntimeInfo")

def _instrumented(runtime):
    return [lib.short_path for lib in runtime.libs if not lib.is_source and lib.dirname.endswith("_sharpfuzz")]

def _first_party(runtimes):
    return [runtime for runtime in runtimes if runtime.nuget_info == None]

def _assembly(runtime):
    return runtime.libs[0].basename if runtime.libs else runtime.name

def _transitive_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    runtimes = _first_party(target[DotnetAssemblyRuntimeInfo].deps.to_list())
    by_assembly = {}
    for runtime in runtimes:
        by_assembly.setdefault(_assembly(runtime), []).append(runtime)
    chain = ctx.attr.chain
    asserts.equals(env, sorted(chain), sorted(by_assembly.keys()))
    for index, assembly in enumerate(chain):
        found = by_assembly.get(assembly, [])
        asserts.equals(env, 1, len(found), "runtimes with " + assembly)
        if len(found) != 1:
            continue
        asserts.equals(env, len(found[0].libs), len(_instrumented(found[0])), "instrumented assemblies of " + assembly)
        below = sorted([_assembly(runtime) for runtime in _first_party(found[0].deps.to_list()) if _instrumented(runtime)])
        asserts.equals(env, sorted(chain[index + 1:]), below, "instrumented runtime deps of " + assembly)
    return analysistest.end(env)

_transitive_test = analysistest.make(
    _transitive_impl,
    attrs = {"chain": attr.string_list(doc = "Assembly file names from the harness dep down to its deepest library.")},
)

def dx_sharpfuzz_transitive_test(name, harness, chain):
    """Asserts every first-party assembly in chain under a fuzz harness is instrumented."""
    _transitive_test(name = name, size = "small", target_under_test = harness, chain = chain)
