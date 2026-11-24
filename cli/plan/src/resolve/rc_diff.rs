const NO_BUILD_EFFECT: &[&str] = &[
    "announce_rc",
    "bes_backend",
    "bes_header",
    "bes_instance_name",
    "bes_keywords",
    "bes_lifecycle_events",
    "bes_results_url",
    "bes_timeout",
    "bes_upload_mode",
    "build_event_binary_file",
    "build_event_json_file",
    "build_event_publish_all_actions",
    "build_event_text_file",
    "build_metadata",
    "color",
    "curses",
    "disk_cache",
    "experimental_disk_cache_gc_max_age",
    "experimental_disk_cache_gc_max_size",
    "experimental_repository_cache_hardlinks",
    "experimental_ui_max_stdouterr_bytes",
    "jobs",
    "local_cpu_resources",
    "local_ram_resources",
    "local_resources",
    "max_idle_secs",
    "progress_report_interval",
    "remote_cache",
    "remote_header",
    "remote_instance_name",
    "remote_max_connections",
    "remote_retries",
    "remote_timeout",
    "repository_cache",
    "show_progress",
    "show_progress_rate_limit",
    "show_result",
    "show_timestamps",
    "terminal_columns",
    "test_summary",
    "ui_actions_shown",
    "ui_event_filters",
    "verbose_failures",
];

fn joined_lines(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut pending = String::new();
    for line in text.lines() {
        if let Some(head) = line.strip_suffix('\\') {
            pending.push_str(head);
            pending.push(' ');
            continue;
        }
        pending.push_str(line);
        lines.push(std::mem::take(&mut pending));
    }
    if !pending.is_empty() {
        lines.push(pending);
    }
    lines
}

fn no_build_effect(flag: &str) -> bool {
    let name = flag.split_once('=').map_or(flag, |(name, _)| name);
    NO_BUILD_EFFECT.contains(&name)
        || name
            .strip_prefix("no")
            .is_some_and(|name| NO_BUILD_EFFECT.contains(&name))
}

fn effective(text: &str) -> Vec<Vec<String>> {
    joined_lines(text)
        .iter()
        .map(|line| {
            line.split_whitespace()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .filter(|words| words.first().is_some_and(|first| !first.starts_with('#')))
        .filter(|words| !inert(words))
        .collect()
}

fn inert(words: &[String]) -> bool {
    let Some((command, flags)) = words.split_first() else {
        return true;
    };
    if matches!(command.as_str(), "import" | "try-import") || flags.is_empty() {
        return false;
    }
    let mut previous_inert = false;
    for word in flags {
        match word.strip_prefix("--") {
            Some(flag) => {
                if !no_build_effect(flag) {
                    return false;
                }
                previous_inert = !flag.contains('=');
            }
            None if previous_inert => previous_inert = false,
            None => return false,
        }
    }
    true
}

pub(crate) fn changes_builds(base: &str, head: &str) -> bool {
    effective(base) != effective(head)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comments_layout_and_inert_flags_do_not_change_builds() {
        let base = "# ci\nbuild --config=strict\nbuild --jobs=8\ncommon --color=yes\n";
        for head in [
            "build   --config=strict\n# other\n\nbuild --jobs 16\n",
            "build --config=strict\nbuild --disk_cache=/tmp/x --remote_timeout=60\n",
            "build \\\n  --config=strict\ncommon --nocolor --noshow_progress\n",
        ] {
            assert!(!changes_builds(base, head), "{head}");
        }
    }

    #[test]
    fn flags_order_and_imports_change_builds() {
        let base = "build --copt=-O1\nbuild --copt=-O2\n";
        for head in [
            "build --copt=-O2\nbuild --copt=-O1\n",
            "build --copt=-O1\nbuild --copt=-O2\nbuild --jobs=4 --compilation_mode=opt\n",
            "build --copt=-O1\nbuild --copt=-O2\ntry-import %workspace%/user.bazelrc\n",
            "build --copt=-O1\nbuild --copt=-O2\nbuild:ci --jobs 4 opt\n",
            "build --copt=-O1\nbuild --copt=-O2\nbuild\n",
        ] {
            assert!(changes_builds(base, head), "{head}");
        }
    }
}
