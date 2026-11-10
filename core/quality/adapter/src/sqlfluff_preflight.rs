//! Offline check dx runs on a sqlfluff config before sqlfluff sees it.
//!
//! The dialect comes from the sql_library or the file suffix and reaches sqlfluff as `--dialect`,
//! so a config must not set one. Templaters that run project code or reach outside the config are refused.

/// The dialects sqlfluff 4.4.0 lists in `sqlfluff dialects`; update with the pin.
pub const DIALECTS: &[&str] = &[
    "ansi",
    "athena",
    "bigquery",
    "clickhouse",
    "databricks",
    "db2",
    "doris",
    "duckdb",
    "exasol",
    "flink",
    "greenplum",
    "hive",
    "impala",
    "mariadb",
    "materialize",
    "mysql",
    "oracle",
    "postgres",
    "redshift",
    "snowflake",
    "soql",
    "sparksql",
    "sqlite",
    "starrocks",
    "teradata",
    "trino",
    "tsql",
    "vertica",
];

/// The templaters that need no project code or dependency beyond the pinned wheels.
pub const TEMPLATERS: &[&str] = &["raw", "jinja", "placeholder"];

/// Keys that make a templater read files outside the bound config.
const BANNED_KEYS: &[&str] = &[
    "library_path",
    "load_macros_from_path",
    "loader_search_path",
];

/// Sections of templaters that run project code.
const BANNED_SECTIONS: &[&str] = &["sqlfluff:templater:dbt", "sqlfluff:templater:python"];

/// The sqlfluff dialect for a `sql_library` dialect name or file suffix, or the reason it has none.
pub fn dialect_from_sql_library(name: &str) -> Result<&'static str, String> {
    let dialect = match name {
        "bigquery" => "bigquery",
        "clickhouse" => "clickhouse",
        "db2" => "db2",
        "hive" => "hive",
        "mariadb" => "mariadb",
        "mysql" => "mysql",
        "plsql" => "oracle",
        "postgresql" => "postgres",
        "redshift" => "redshift",
        "snowflake" => "snowflake",
        "spark" => "sparksql",
        "sqlite" => "sqlite",
        "transactsql" => "tsql",
        "trino" => "trino",
        _ => {
            return Err(format!(
                "sqlfluff has no `{name}` dialect; use another dialect or tag the target no-lint"
            ));
        }
    };
    Ok(dialect)
}

/// The `sql_library` dialect a file name carries as `name.<dialect>.sql`, if any.
pub fn suffix_dialect(basename: &str) -> Option<&str> {
    let mut parts = basename.rsplit('.');
    if parts.next()? != "sql" {
        return None;
    }
    let suffix = parts.next()?;
    parts.next()?;
    dialect_from_sql_library(suffix).is_ok().then_some(suffix)
}

/// Refuses a sqlfluff config that sets a dialect or a templater dx does not allow.
pub fn check_config(text: &str) -> Result<(), String> {
    check(text)
}

fn check(text: &str) -> Result<(), String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut section = String::new();
    let mut templater: Option<String> = None;
    // Mirrors Python's configparser: a line is a continuation only when the section already has
    // a key and the line is indented more than that key.
    let mut in_key = false;
    let mut indent_level = 0usize;
    for (index, raw) in text.lines().enumerate() {
        let number = index + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        let indent = raw.len() - raw.trim_start().len();
        if in_key && indent > indent_level {
            continue;
        }
        indent_level = indent;
        if let Some(rest) = line.strip_prefix('[') {
            let Some(name) = rest.strip_suffix(']') else {
                return Err(format!(
                    "sqlfluff config line {number}: unterminated section"
                ));
            };
            section = name.trim().to_ascii_lowercase();
            in_key = false;
            if BANNED_SECTIONS.contains(&section.as_str()) {
                return Err(format!(
                    "sqlfluff config line {number}: [{section}] is not allowed: dx runs only the {} templaters",
                    TEMPLATERS.join(", ")
                ));
            }
            continue;
        }
        let Some(split) = line.find(['=', ':']) else {
            return Err(format!(
                "sqlfluff config line {number}: expected `key = value`"
            ));
        };
        in_key = true;
        let key = line[..split].trim().to_ascii_lowercase();
        let value = line[split + 1..].trim();
        if BANNED_KEYS.contains(&key.as_str()) {
            return Err(format!(
                "sqlfluff config line {number}: `{key}` is not allowed: dx does not load macros or libraries from the file system"
            ));
        }
        if section == "sqlfluff" {
            match key.as_str() {
                "dialect" => {
                    return Err(format!(
                        "sqlfluff config line {number}: `dialect` is not allowed: dx passes the dialect of the sql_library or the file suffix"
                    ));
                }
                "templater" => templater = Some(value.to_ascii_lowercase()),
                _ => {}
            }
        }
    }
    if let Some(name) = templater.as_deref()
        && !TEMPLATERS.contains(&name)
    {
        return Err(format!(
            "sqlfluff config templater `{name}` is not allowed: dx accepts only {}",
            TEMPLATERS.join(", ")
        ));
    }
    Ok(())
}

/// Refuses inline `-- sqlfluff:...` directives in a SQL file that set a banned key, because
/// sqlfluff applies them without the config checks above.
pub fn check_inline_config(sql: &str) -> Result<(), String> {
    for (index, raw) in sql.lines().enumerate() {
        let mut line = raw.trim_start();
        let Some(rest) = ["--", "#", "/*"]
            .iter()
            .find_map(|marker| line.strip_prefix(marker))
        else {
            continue;
        };
        line = rest.trim_start().trim_start_matches('-').trim_start();
        let lower = line.to_ascii_lowercase();
        if !lower.starts_with("sqlfluff") {
            continue;
        }
        if let Some(key) = BANNED_KEYS.iter().find(|key| lower.contains(*key)) {
            return Err(format!(
                "inline sqlfluff directive on line {}: `{key}` is not allowed: dx does not load macros or libraries from the file system",
                index + 1
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_config_without_a_dialect_passes() {
        assert!(check_config("").is_ok());
        assert!(check_config("\u{feff}[sqlfluff]\r\ntemplater = raw\r\n").is_ok());
        assert!(check_config(
            "# c\n[sqlfluff]\ntemplater = jinja\nexclude_rules =\n    LT01,\n    LT02\n[sqlfluff:rules:capitalisation.keywords]\ncapitalisation_policy = lower\n"
        )
        .is_ok());
        // A dialect under another section is not the one sqlfluff reads.
        assert!(check_config("[sqlfluff:core]\n").is_ok());
    }

    #[test]
    fn a_dialect_in_the_config_is_refused() {
        for text in [
            "[sqlfluff]\ndialect = ansi\n",
            "[sqlfluff]\nDialect: postgres\n",
            "[sqlfluff]\ndialect = \n",
        ] {
            let err = check_config(text).expect_err(text);
            assert!(err.contains("`dialect` is not allowed"), "{err}");
        }
    }

    #[test]
    fn sql_library_dialects_map_to_sqlfluff_names() {
        for (name, want) in [
            ("postgresql", "postgres"),
            ("spark", "sparksql"),
            ("transactsql", "tsql"),
            ("plsql", "oracle"),
            ("mysql", "mysql"),
        ] {
            assert_eq!(dialect_from_sql_library(name), Ok(want));
        }
        for name in ["n1ql", "singlestoredb", "db2i"] {
            let err = dialect_from_sql_library(name).expect_err(name);
            assert!(err.contains(name), "{err}");
        }
        for name in [
            "bigquery",
            "clickhouse",
            "db2",
            "hive",
            "mariadb",
            "redshift",
        ] {
            assert!(DIALECTS.contains(&dialect_from_sql_library(name).unwrap()));
        }
    }

    #[test]
    fn the_dialect_suffix_is_read_from_the_file_name() {
        assert_eq!(suffix_dialect("a.postgresql.sql"), Some("postgresql"));
        assert_eq!(suffix_dialect("a.sql"), None);
        assert_eq!(suffix_dialect("postgresql.sql"), None);
        assert_eq!(suffix_dialect("a.n1ql.sql"), None);
        assert_eq!(suffix_dialect("a.postgresql.txt"), None);
    }

    #[test]
    fn project_code_templaters_and_file_system_loaders_are_refused() {
        for templater in ["dbt", "python", "other"] {
            let text = format!("[sqlfluff]\ntemplater = {templater}\n");
            assert!(check_config(&text).is_err(), "{templater}");
        }
        for section in ["sqlfluff:templater:dbt", "sqlfluff:templater:python"] {
            let text = format!("[sqlfluff]\n[{section}]\nproject_dir = .\n");
            assert!(check_config(&text).is_err(), "{section}");
        }
        for key in [
            "library_path",
            "load_macros_from_path",
            "loader_search_path",
        ] {
            let text = format!("[sqlfluff]\n[sqlfluff:templater:jinja]\n{key} = macros\n");
            let err = check_config(&text).expect_err(key);
            assert!(err.contains(key), "{err}");
        }
    }

    #[test]
    fn indented_lines_without_a_key_above_are_keys() {
        // configparser reads an indented first line of a section as a key.
        let err = check_config("[sqlfluff]\n[sqlfluff:templater:jinja]\n  library_path = /x\n")
            .expect_err("library_path");
        assert!(err.contains("library_path"), "{err}");
        let err = check_config("[sqlfluff]\n  templater = python\n").expect_err("templater");
        assert!(err.contains("python"), "{err}");
        // Same indent as the key above is a new key, not a continuation.
        assert!(check_config("[sqlfluff]\n  exclude_rules = x\n  templater = python\n").is_err());
        // Deeper indent after a key stays a continuation.
        assert!(check_config("[sqlfluff]\nexclude_rules =\n  library_path = x\n").is_ok());
    }

    #[test]
    fn inline_directives_with_banned_keys_are_refused() {
        for line in [
            "-- sqlfluff:templater:jinja:library_path:/d",
            "--sqlfluff:library_path:/d",
            "  -- SQLFLUFF:templater:jinja:load_macros_from_path:m",
            "-- sqlfluff:loader_search_path:x",
        ] {
            let sql = format!("select 1\n{line}\n");
            assert!(check_inline_config(&sql).is_err(), "{line}");
        }
        assert!(
            check_inline_config("-- sqlfluff:disable:LT01\nselect 1 -- library_path\n").is_ok()
        );
        assert!(check_inline_config("-- library_path is a name\n").is_ok());
    }

    #[test]
    fn malformed_lines_are_refused() {
        assert!(check_config("[sqlfluff\n").is_err());
        assert!(check_config("[sqlfluff]\ndialect ansi\n").is_err());
    }
}
