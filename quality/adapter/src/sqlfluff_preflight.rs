//! Offline check dx runs on a sqlfluff config before sqlfluff sees it.
//!
//! The config is the only place the SQL dialect comes from, so it must name one sqlfluff knows.
//! Templaters that run project code or reach outside the config are refused.

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

/// Refuses a sqlfluff config without a known dialect, or with a templater dx does not allow.
pub fn check_config(text: &str) -> Result<(), String> {
    scan_config(text, true, "sqlfluff config")
}

/// Refuses a nested `.sqlfluff` that sets an unknown dialect or a templater dx does not allow.
/// The dialect is optional because the bound config below it supplies one, and `sql_file_exts` is
/// refused because it would filter the files of the group out of the run.
pub fn check_nested_config(text: &str) -> Result<(), String> {
    scan_config(text, false, "nested sqlfluff config")
}

fn scan_config(text: &str, require_dialect: bool, what: &str) -> Result<(), String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut section = String::new();
    let mut dialect: Option<String> = None;
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
                return Err(format!("{what} line {number}: unterminated section"));
            };
            section = name.trim().to_ascii_lowercase();
            in_key = false;
            if BANNED_SECTIONS.contains(&section.as_str()) {
                return Err(format!(
                    "{what} line {number}: [{section}] is not allowed: dx runs only the {} templaters",
                    TEMPLATERS.join(", ")
                ));
            }
            continue;
        }
        let Some(split) = line.find(['=', ':']) else {
            return Err(format!("{what} line {number}: expected `key = value`"));
        };
        in_key = true;
        let key = line[..split].trim().to_ascii_lowercase();
        let value = line[split + 1..].trim();
        if BANNED_KEYS.contains(&key.as_str()) {
            return Err(format!(
                "{what} line {number}: `{key}` is not allowed: dx does not load macros or libraries from the file system"
            ));
        }
        if !require_dialect && key == "sql_file_exts" {
            return Err(format!(
                "{what} line {number}: `sql_file_exts` is not allowed in a nested config"
            ));
        }
        if section == "sqlfluff" {
            match key.as_str() {
                "dialect" => dialect = Some(value.to_ascii_lowercase()),
                "templater" => templater = Some(value.to_ascii_lowercase()),
                _ => {}
            }
        }
    }
    match dialect.as_deref() {
        None | Some("") if !require_dialect => {}
        None | Some("") => {
            return Err(format!(
                "{what} must set `dialect` under [sqlfluff]; one of {}",
                DIALECTS.join(", ")
            ))
        }
        Some(name) if !DIALECTS.contains(&name) => {
            return Err(format!(
                "{what} dialect `{name}` is unknown to sqlfluff 4.4.0; use one of {}",
                DIALECTS.join(", ")
            ))
        }
        Some(_) => {}
    }
    if let Some(name) = templater.as_deref() {
        if !TEMPLATERS.contains(&name) {
            return Err(format!(
                "{what} templater `{name}` is not allowed: dx accepts only {}",
                TEMPLATERS.join(", ")
            ));
        }
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
    fn a_nested_config_needs_no_dialect_but_must_be_safe() {
        assert!(check_nested_config("[sqlfluff]\ntemplater = placeholder\n").is_ok());
        assert!(check_nested_config("").is_ok());
        assert!(check_nested_config("[sqlfluff]\ndialect = tsql\n").is_ok());
        let err = check_nested_config("[sqlfluff]\ndialect = postgresql\n").expect_err("unknown");
        assert!(err.contains("nested sqlfluff config dialect"), "{err}");
        assert!(check_nested_config("[sqlfluff]\ntemplater = dbt\n").is_err());
        assert!(check_nested_config("[sqlfluff:templater:python]\nx = 1\n").is_err());
        assert!(check_nested_config("[sqlfluff:templater:jinja]\nlibrary_path = l\n").is_err());
        let err = check_nested_config("[sqlfluff]\nsql_file_exts = .txt\n").expect_err("exts");
        assert!(err.contains("sql_file_exts"), "{err}");
        assert!(check_config("[sqlfluff]\nsql_file_exts = .sql,.txt\ndialect = ansi\n").is_ok());
    }

    #[test]
    fn a_known_dialect_passes() {
        assert!(check_config("[sqlfluff]\ndialect = postgres\n").is_ok());
        assert!(check_config("\u{feff}[sqlfluff]\r\ndialect: ANSI\r\ntemplater = raw\r\n").is_ok());
        assert!(check_config(
            "# c\n[sqlfluff]\ntemplater = jinja\ndialect = tsql\nexclude_rules =\n    LT01,\n    LT02\n[sqlfluff:rules:capitalisation.keywords]\ncapitalisation_policy = lower\n"
        )
        .is_ok());
    }

    #[test]
    fn a_missing_or_unknown_dialect_is_refused() {
        let err = check_config("[sqlfluff]\ntemplater = raw\n").expect_err("no dialect");
        assert!(err.contains("must set `dialect`"), "{err}");
        assert!(check_config("").is_err());
        assert!(check_config("[sqlfluff]\ndialect =\n").is_err());
        // The sql_library spelling is not sqlfluff's.
        let err = check_config("[sqlfluff]\ndialect = postgresql\n").expect_err("unknown");
        assert!(err.contains("unknown"), "{err}");
        // A dialect under another section does not count.
        assert!(check_config("[sqlfluff:core]\ndialect = ansi\n").is_err());
    }

    #[test]
    fn project_code_templaters_and_file_system_loaders_are_refused() {
        for templater in ["dbt", "python", "other"] {
            let text = format!("[sqlfluff]\ndialect = ansi\ntemplater = {templater}\n");
            assert!(check_config(&text).is_err(), "{templater}");
        }
        for section in ["sqlfluff:templater:dbt", "sqlfluff:templater:python"] {
            let text = format!("[sqlfluff]\ndialect = ansi\n[{section}]\nproject_dir = .\n");
            assert!(check_config(&text).is_err(), "{section}");
        }
        for key in [
            "library_path",
            "load_macros_from_path",
            "loader_search_path",
        ] {
            let text =
                format!("[sqlfluff]\ndialect = ansi\n[sqlfluff:templater:jinja]\n{key} = macros\n");
            let err = check_config(&text).expect_err(key);
            assert!(err.contains(key), "{err}");
        }
    }

    #[test]
    fn indented_lines_without_a_key_above_are_keys() {
        // configparser reads an indented first line of a section as a key.
        let err = check_config(
            "[sqlfluff]\ndialect = ansi\n[sqlfluff:templater:jinja]\n  library_path = /x\n",
        )
        .expect_err("library_path");
        assert!(err.contains("library_path"), "{err}");
        let err = check_config("[sqlfluff]\n  templater = python\ndialect = ansi\n")
            .expect_err("templater");
        assert!(err.contains("python"), "{err}");
        // Same indent as the key above is a new key, not a continuation.
        assert!(check_config("[sqlfluff]\n  dialect = ansi\n  templater = python\n").is_err());
        // Deeper indent after a key stays a continuation.
        assert!(
            check_config("[sqlfluff]\ndialect = ansi\nexclude_rules =\n  library_path = x\n")
                .is_ok()
        );
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
        assert!(check_config("[sqlfluff\ndialect = ansi\n").is_err());
        assert!(check_config("[sqlfluff]\ndialect ansi\n").is_err());
    }
}
