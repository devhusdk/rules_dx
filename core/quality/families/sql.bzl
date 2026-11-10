"""SQL real quality aspects."""

load("//core/quality:real_core.bzl", "real_config", "real_context", "real_make_aspect")
load("//sql/private:providers.bzl", "SqlDialectInfo")

TOOLS = {
    "sqlfluff": {
        "attr": attr.label(default = "//core/quality/tools/python:sqlfluff", cfg = "exec", executable = True),
        "kind": "executable",
        "resources": "medium",
        "runfiles_env": True,
        "shard": "sql",
    },
}

SHARD_SPECS = {
    "real_sql_format": {
        "capability": "format",
        "doc": "Additive SQL format family aspect (sqlfluff).",
        "shard": "sql",
        "suffix": "-sql",
    },
    "real_sql_lint": {
        "capability": "lint",
        "doc": "Additive SQL lint family aspect (sqlfluff).",
        "shard": "sql",
        "suffix": "-sql",
    },
}

# sql_library dialect names to sqlfluff's. A dialect missing here has no sqlfluff dialect.
_SQLFLUFF_DIALECTS = {
    "bigquery": "bigquery",
    "clickhouse": "clickhouse",
    "db2": "db2",
    "hive": "hive",
    "mariadb": "mariadb",
    "mysql": "mysql",
    "plsql": "oracle",
    "postgresql": "postgres",
    "redshift": "redshift",
    "snowflake": "snowflake",
    "spark": "sparksql",
    "sqlite": "sqlite",
    "transactsql": "tsql",
    "trino": "trino",
}

def _sqlfluff_hook(target, _ctx, _resolved, _capability, what, _shared):
    if SqlDialectInfo not in target:
        return real_context()
    dialect = target[SqlDialectInfo].dialect
    if dialect not in _SQLFLUFF_DIALECTS:
        fail("%s: sqlfluff has no %s dialect; use another dialect or tag the target no-lint" % (what, dialect))
    return real_context(options = {"dialect": _SQLFLUFF_DIALECTS[dialect]})

HOOK_ORDER = ["sqlfluff"]

HOOKS = {"sqlfluff": _sqlfluff_hook}

_CFG = real_config(
    tools = TOOLS,
    shard_specs = SHARD_SPECS,
    hook_order = HOOK_ORDER,
    context_hooks = HOOKS,
    source_recognizers = [],
    harness_sources = lambda _target, _capability, direct_sources: direct_sources,
    toolchain = None,
    check_policy = False,
)

real_sql_format_aspect = real_make_aspect(_CFG, "real_sql_format")

real_sql_lint_aspect = real_make_aspect(_CFG, "real_sql_lint")
