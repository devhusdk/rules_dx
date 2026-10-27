"""SQL source ownership for dx quality."""

load("//libs/starlark:wrapper.bzl", "dx_quality_sources")
load(":providers.bzl", "SQL_DIALECTS", "SqlDialectInfo", "sql_dialect_error")

_EXTENSIONS = ["sql"]

def _sql_library_impl(ctx):
    dialect = ctx.attr.dialect
    error = sql_dialect_error([f.basename for f in ctx.files.srcs], dialect) if dialect else None
    if error:
        fail("%s: %s" % (ctx.label, error))
    providers = [
        DefaultInfo(files = depset(
            ctx.files.srcs,
            transitive = [dep[DefaultInfo].files for dep in ctx.attr.deps],
        )),
        dx_quality_sources(ctx.files.srcs, [("sql", _EXTENSIONS)], str(ctx.label)),
    ]
    if dialect:
        providers.append(SqlDialectInfo(dialect = dialect))
    return providers

sql_library = rule(
    implementation = _sql_library_impl,
    doc = "Owns the SQL files of one package for dx quality.",
    attrs = {
        "srcs": attr.label_list(allow_files = ["." + ext for ext in _EXTENSIONS], doc = "SQL files of this package."),
        "dialect": attr.string(default = "", values = [""] + SQL_DIALECTS, doc = "SQL dialect of every source; empty lets each file's dialect suffix decide."),
        "deps": attr.label_list(providers = [DefaultInfo], doc = "SQL libraries this package depends on."),
    },
)
