"""SQL dialect provider and suffix check for dx quality."""

SQL_DIALECTS = [
    "bigquery",
    "clickhouse",
    "db2",
    "db2i",
    "hive",
    "mariadb",
    "mysql",
    "n1ql",
    "plsql",
    "postgresql",
    "redshift",
    "singlestoredb",
    "snowflake",
    "spark",
    "sqlite",
    "transactsql",
    "trino",
    "tsql",
]

SqlDialectInfo = provider(
    doc = "The SQL dialect every source of one sql_library is formatted as.",
    fields = {"dialect": "A name from SQL_DIALECTS."},
)

def sql_dialect_error(basenames, dialect):
    """Returns the message for the first file whose dialect suffix differs from dialect, or None."""
    for basename in basenames:
        parts = basename.split(".")
        if len(parts) < 3 or parts[-1] != "sql":
            continue
        suffix = parts[-2]
        if suffix in SQL_DIALECTS and suffix != dialect:
            return "%s has the %s dialect suffix but dialect = %s" % (basename, suffix, dialect)
    return None
