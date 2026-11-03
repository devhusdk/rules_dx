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

def _impl(ctx):
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
            asserts.true(env, "llvm" in cc, "CC must come from the llvm repo, got " + cc)
        linkopt_seen = linkopt_seen or "-Wl,-z,now" in argv
        copt_seen = copt_seen or "-DDX_CGO_COPT=1" in _after(argv, "-cflags")
    asserts.true(env, cc_seen, "expected CC in a compile action")
    asserts.true(env, copt_seen, "copts must reach the cgo compile")
    for _, argv, _ in links:
        extld = _after(argv, "-extld")
        asserts.true(env, "llvm" in extld, "-extld must come from the llvm repo, got " + extld)
    asserts.true(env, linkopt_seen, "clinkopts must reach the link")
    for _, argv, _ in compiles + links:
        for arg in argv:
            for part in arg.replace("=", " ").replace(",", " ").split(" "):
                asserts.false(env, part.startswith("/usr") or part.startswith("-I/usr"), "argument reads the host system: " + arg)
    return analysistest.end(env)

toolchain_test = analysistest.make(
    _impl,
    doc = "Asserts cgo compile and link use the llvm toolchain and keep copts and clinkopts.",
    extra_target_under_test_aspects = [_aspect],
)
