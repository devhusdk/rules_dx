def _fake_repo_impl(rctx):
    if rctx.attr.prev:
        rctx.read(rctx.attr.prev)
    rctx.file("data.txt", "%s %s\n" % (rctx.attr.name, rctx.attr.nonce))
    if rctx.attr.sleep:
        rctx.execute(["sleep", str(rctx.attr.sleep)])
    rctx.file("BUILD", 'exports_files(["data.txt"])\n')
    return rctx.repo_metadata(reproducible = True)

fake_repo = repository_rule(
    implementation = _fake_repo_impl,
    attrs = {"prev": attr.label(), "sleep": attr.int(), "nonce": attr.string()},
)

def _hub_impl(rctx):
    rctx.file("BUILD", 'filegroup(name = "all", srcs = %r, visibility = ["//visibility:public"])\n' % rctx.attr.srcs)

hub = repository_rule(implementation = _hub_impl, attrs = {"srcs": attr.string_list()})

def _repos_impl(mctx):
    n = int(mctx.getenv("REPRO_REPOS", "3"))
    chain = mctx.getenv("REPRO_CHAIN", "0") == "1"
    sleep = int(mctx.getenv("REPRO_SLEEP", "2"))
    nonce = mctx.getenv("REPRO_NONCE", "")
    for i in range(n):
        prev = "@r%d//:data.txt" % (i - 1) if chain and i > 0 else None
        fake_repo(name = "r%d" % i, prev = prev, sleep = sleep, nonce = nonce)
    hub(name = "all", srcs = ["@r%d//:data.txt" % i for i in range(n)])
    return mctx.extension_metadata(reproducible = True)

repos = module_extension(implementation = _repos_impl)
