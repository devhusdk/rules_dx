"""SQL real quality aspects."""

load("//core/quality:real_core.bzl", "real_config", "real_context", "real_make_aspect", "real_make_probe_aspect")
load("//sql/private:providers.bzl", "SqlDialectInfo")

TOOLS = {
    "sqlfluff": {
        "attr": attr.label(default = "//config:tool_sqlfluff", cfg = "exec", executable = True),
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
    },
    "real_sql_lint": {
        "capability": "lint",
        "doc": "Additive SQL lint family aspect (sqlfluff).",
        "shard": "sql",
    },
}

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
)

real_sql_format_aspect = real_make_aspect(_CFG, "real_sql_format")

real_sql_lint_aspect = real_make_aspect(_CFG, "real_sql_lint")

SqlShardProbeInfo = provider(
    doc = "Shard names the sql shard probe found for one target and capability.",
    fields = {
        "capability": "lint, format or typecheck",
        "shards": "sorted shard names whose tool filter keeps a resolved stage",
    },
)

QUALITY_CFG = _CFG

real_sql_format_probe_aspect = real_make_probe_aspect(_CFG, "format", "sql", SqlShardProbeInfo)

real_sql_lint_probe_aspect = real_make_probe_aspect(_CFG, "lint", "sql", SqlShardProbeInfo)
