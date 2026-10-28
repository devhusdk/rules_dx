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
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut section = String::new();
    let mut dialect: Option<String> = None;
    let mut templater: Option<String> = None;
    for (index, raw) in text.lines().enumerate() {
        let number = index + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        // Continuation lines of a multi-line value belong to the previous key.
        if raw.starts_with([' ', '\t']) {
            continue;
        }
        if let Some(rest) = line.strip_prefix('[') {
            let Some(name) = rest.strip_suffix(']') else {
                return Err(format!(
                    "sqlfluff config line {number}: unterminated section"
                ));
            };
            section = name.trim().to_ascii_lowercase();
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
        let key = line[..split].trim().to_ascii_lowercase();
        let value = line[split + 1..].trim();
        if BANNED_KEYS.contains(&key.as_str()) {
            return Err(format!(
                "sqlfluff config line {number}: `{key}` is not allowed: dx does not load macros or libraries from the file system"
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
        None | Some("") => {
            return Err(format!(
                "sqlfluff config must set `dialect` under [sqlfluff]; one of {}",
                DIALECTS.join(", ")
            ))
        }
        Some(name) if !DIALECTS.contains(&name) => {
            return Err(format!(
                "sqlfluff config dialect `{name}` is unknown to sqlfluff 4.4.0; use one of {}",
                DIALECTS.join(", ")
            ))
        }
        Some(_) => {}
    }
    if let Some(name) = templater.as_deref() {
        if !TEMPLATERS.contains(&name) {
            return Err(format!(
                "sqlfluff config templater `{name}` is not allowed: dx accepts only {}",
                TEMPLATERS.join(", ")
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn malformed_lines_are_refused() {
        assert!(check_config("[sqlfluff\ndialect = ansi\n").is_err());
        assert!(check_config("[sqlfluff]\ndialect ansi\n").is_err());
    }
}
