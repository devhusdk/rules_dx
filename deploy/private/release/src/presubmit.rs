/// Platforms of the machine this binary runs on, as named in presubmit.yml.
pub fn host_platforms() -> &'static [&'static str] {
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        &["debian11", "ubuntu2204"]
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        &["ubuntu2204_arm64"]
    } else if cfg!(target_os = "macos") {
        &["macos_arm64"]
    } else if cfg!(target_os = "windows") {
        &["windows"]
    } else {
        &[]
    }
}

struct Task {
    platform: String,
    lists: Vec<(String, Vec<String>)>,
}

fn tasks(presubmit: &str) -> Vec<Task> {
    let mut tasks: Vec<Task> = Vec::new();
    let mut in_tasks = false;
    let mut key = String::new();
    for line in presubmit.lines() {
        let trimmed = line.trim();
        let indent = line.len() - line.trim_start().len();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if indent == 2 {
            in_tasks = trimmed == "tasks:";
        } else if !in_tasks {
        } else if indent == 4 {
            tasks.push(Task {
                platform: String::new(),
                lists: Vec::new(),
            });
        } else if let Some(task) = tasks.last_mut() {
            if let Some(item) = trimmed.strip_prefix("- ") {
                if let Some((_, items)) = task.lists.iter_mut().find(|(name, _)| *name == key) {
                    items.push(item.trim_matches('"').to_owned());
                }
            } else if let Some(value) = trimmed.strip_prefix("platform:") {
                value.trim().clone_into(&mut task.platform);
            } else if let Some(name) = trimmed.strip_suffix(':') {
                key = name.to_owned();
                task.lists.push((key.clone(), Vec::new()));
            }
        }
    }
    tasks
}

fn selected<'a>(tasks: &'a [Task], host: &'a [&str]) -> impl Iterator<Item = &'a Task> {
    tasks.iter().filter(move |task| {
        task.platform == "${{ platform }}" || host.contains(&task.platform.as_str())
    })
}

/// The (verb, target) pairs of the tasks that run on `host`, in file order
/// without duplicates. The verb is `run`, `test` or `build`.
pub fn host_targets(presubmit: &str, host: &[&str]) -> Vec<(String, String)> {
    let tasks = tasks(presubmit);
    let mut pairs: Vec<(String, String)> = Vec::new();
    for task in selected(&tasks, host) {
        for (list, verb) in [
            ("run_targets", "run"),
            ("test_targets", "test"),
            ("build_targets", "build"),
        ] {
            for (name, items) in &task.lists {
                if name != list {
                    continue;
                }
                for item in items {
                    let pair = (verb.to_owned(), item.clone());
                    if !pairs.contains(&pair) {
                        pairs.push(pair);
                    }
                }
            }
        }
    }
    pairs
}

/// The `run_flags` of the tasks that run on `host`.
pub fn host_run_flags(presubmit: &str, host: &[&str]) -> Vec<String> {
    let tasks = tasks(presubmit);
    let mut flags: Vec<String> = Vec::new();
    for task in selected(&tasks, host) {
        for (name, items) in &task.lists {
            if name == "run_flags" {
                flags.extend(items.iter().cloned());
            }
        }
    }
    flags
}
