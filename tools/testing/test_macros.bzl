load("//rust:defs.bzl", "rust_test")

def dx_format_test(name, tool, srcs, args, config = None, data = [], env = {}, **kwargs):
    env = dict(env)
    env["DX_FORMAT_TOOL"] = "$(rootpath %s)" % tool
    env["DX_FORMAT_ARGS"] = " ".join(args)
    env["DX_FORMAT_SRCS"] = " ".join(["$(rootpaths %s)" % src for src in srcs])
    extra = [tool] + srcs
    if config:
        env["DX_FORMAT_CONFIG"] = "$(rootpath %s)" % config
        extra.append(config)
    rust_test(
        name = name,
        srcs = [Label("//tools/testing:format_test.rs")],
        data = extra + data,
        env = env,
        deps = [Label("//tools/testing:dx_testing")],
        **kwargs
    )

def dx_diagnostics_test(name, language, data, env, **kwargs):
    rust_test(
        name = name,
        srcs = [Label("//tools/testing:%s_diagnostics_test.rs" % language)],
        data = data,
        env = env,
        deps = [Label("//tools/testing:dx_testing")],
        **kwargs
    )

CLANG_FORMAT_ARGS = ["--dry-run", "--Werror", "--style=file:{config}", "{src}"]
