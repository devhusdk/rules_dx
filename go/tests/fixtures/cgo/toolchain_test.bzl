"""Analysis test for the C and C++ toolchain used by cgo targets."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

GoCgoActionsInfo = provider(
    doc = "Go compile and link actions reached from one target.",
    fields = {"actions": "List of (mnemonic, argv, env) tuples."},
)

_EDGES = ["upstream", "embed", "deps"]

def _aspect_impl(target, ctx):
    actions = []
    for action in target.actions:
        if action.mnemonic.startswith("GoCompilePkg") or action.mnemonic == "GoLink":
            actions.append((action.mnemonic, action.argv or [], action.env or {}))
    for edge in _EDGES:
        value = getattr(ctx.rule.attr, edge, None)
        for dep in value if type(value) == "list" else [value]:
            if dep != None and GoCgoActionsInfo in dep:
                actions.extend(dep[GoCgoActionsInfo].actions)
    return [GoCgoActionsInfo(actions = actions)]

_aspect = aspect(
    implementation = _aspect_impl,
    attr_aspects = _EDGES,
)

def _after(argv, flag):
    if flag in argv:
        return argv[argv.index(flag) + 1]
    return ""

_GCC = "dx_gcc_linux_x86_64"

def _assert_compiler(env, gcc, value, what):
    if gcc:
        asserts.true(env, _GCC in value, what + " must come from " + _GCC + ", got " + value)
    else:
        asserts.true(env, "llvm" in value and "dx_gcc" not in value, what + " must come from the llvm repo, got " + value)

def _impl(ctx):
    gcc = ctx.attr.compiler == "gcc"
    env = analysistest.begin(ctx)
    actions = analysistest.target_under_test(env)[GoCgoActionsInfo].actions
    compiles = [a for a in actions if a[0].startswith("GoCompilePkg")]
    links = [a for a in actions if a[0] == "GoLink"]
    asserts.true(env, len(compiles) > 0, "expected a Go compile action")
    asserts.true(env, len(links) > 0, "expected a Go link action")
    cc_seen = False
    copt_seen = False
    linkopt_seen = False
    for _, argv, action_env in compiles:
        cc = action_env.get("CC", "")
        if cc:
            cc_seen = True
            _assert_compiler(env, gcc, cc, "CC")
        linkopt_seen = linkopt_seen or "-Wl,-z,now" in argv
        copt_seen = copt_seen or "-DDX_CGO_COPT=1" in _after(argv, "-cflags")
    asserts.true(env, cc_seen, "expected CC in a compile action")
    asserts.true(env, copt_seen, "copts must reach the cgo compile")
    for _, argv, _ in links:
        extld = _after(argv, "-extld")
        _assert_compiler(env, gcc, extld, "-extld")
        asserts.equals(env, gcc, "-l:libstdc++.a" in " ".join(argv).replace(",", " "), "static libstdc++ must be linked only under gcc")
    asserts.true(env, linkopt_seen, "clinkopts must reach the link")
    for _, argv, _ in compiles + links:
        for arg in argv:
            for part in arg.replace("=", " ").replace(",", " ").split(" "):
                asserts.false(env, part.startswith("/usr") or part.startswith("-I/usr"), "argument reads the host system: " + arg)
    return analysistest.end(env)

def _make(compiler):
    return analysistest.make(
        _impl,
        doc = "Asserts cgo compile and link use the %s toolchain and keep copts and clinkopts." % compiler,
        attrs = {"compiler": attr.string(default = compiler)},
        extra_target_under_test_aspects = [_aspect],
        config_settings = {str(Label("//cc:compiler")): compiler},
    )

_llvm_test = _make("llvm")
_gcc_test = _make("gcc")

def cgo_toolchain_tests(name, target_under_test):
    """Declares the LLVM and GCC toolchain tests and a suite over them.

    Args:
        name: Suite name; tests are named name_llvm and name_gcc.
        target_under_test: Go target built with cgo.
    """
    _llvm_test(
        name = name + "_llvm",
        target_compatible_with = ["@platforms//os:linux"],
        target_under_test = target_under_test,
    )
    _gcc_test(
        name = name + "_gcc",
        target_compatible_with = ["@platforms//os:linux", "@platforms//cpu:x86_64"],
        target_under_test = target_under_test,
    )
    native.test_suite(name = name, tests = [name + "_llvm", name + "_gcc"])
