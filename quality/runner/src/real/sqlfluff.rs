use std::ffi::OsString;

use super::*;

/// The scratch directory sqlfluff gets as its home.
pub(super) const SQLFLUFF_HOME_REL: &str = ".dx-sqlfluff-home";

/// The dialect sqlfluff uses when no `.sqlfluff` file sets one.
pub(super) const SQLFLUFF_DEFAULT_DIALECT: &str = "ansi";

/// The sqlfluff dialect for a `sql_library` dialect.
pub(super) fn sqlfluff_dialect(dialect: Option<&str>) -> &'static str {
    match dialect {
        Some("bigquery") => "bigquery",
        Some("clickhouse") => "clickhouse",
        Some("db2" | "db2i") => "db2",
        Some("hive") => "hive",
        Some("mariadb") => "mariadb",
        Some("mysql") => "mysql",
        Some("plsql") => "oracle",
        Some("postgresql") => "postgres",
        Some("redshift") => "redshift",
        Some("snowflake") => "snowflake",
        Some("spark") => "sparksql",
        Some("sqlite") => "sqlite",
        Some("transactsql" | "tsql") => "tsql",
        Some("trino") => "trino",
        _ => SQLFLUFF_DEFAULT_DIALECT,
    }
}

/// Writes the lowest-precedence user config with the default dialect and returns the variable that makes it sqlfluff's home.
pub(super) fn sqlfluff_home(
    tool_id: &str,
    tool: &RealTool,
    scratch: &Scratch,
) -> Result<(OsString, OsString), RunnerError> {
    let home = scratch.root().join(SQLFLUFF_HOME_REL);
    let config_dir = home.join(".config").join("sqlfluff");
    std::fs::create_dir_all(&config_dir)
        .map_err(|err| execution(tool_id, format!("sqlfluff home: {err}")))?;
    let defaults = format!(
        "[sqlfluff]\ndialect = {}\n",
        sqlfluff_dialect(tool.dialect.as_deref())
    );
    std::fs::write(config_dir.join(".sqlfluff"), defaults)
        .map_err(|err| execution(tool_id, format!("sqlfluff defaults: {err}")))?;
    Ok((OsString::from("DX_SQLFLUFF_HOME"), home.into_os_string()))
}
