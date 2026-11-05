use super::parse;
use dx_testing::cli::strings;

const TUTORIAL: &str = include_str!("../../../../docs/tutorial.md");
const PROGRAMS: [&str; 10] = [
    "curl", "gh", "chmod", "mkdir", "cd", "cp", "git", "bazel", "echo", "tar",
];

#[test]
fn tutorial_commands_parse_or_use_allowed_programs() {
    let mut in_sh = false;
    for (index, line) in TUTORIAL.lines().enumerate() {
        let number = index + 1;
        if line.starts_with("```") {
            in_sh = line == "```sh";
            continue;
        }
        if !in_sh || line.trim().is_empty() {
            continue;
        }
        let words: Vec<&str> = line.split_whitespace().collect();
        if words[0] == "dx" {
            if let Err(error) = parse(&strings(&words[1..])) {
                panic!("tutorial.md:{number}: {line}: {error:?}");
            }
        } else {
            assert!(
                PROGRAMS.contains(&words[0]),
                "tutorial.md:{number}: unknown program in {line}"
            );
        }
    }
}

#[test]
fn tutorial_python_imports_name_the_module() {
    for line in TUTORIAL.lines().filter(|line| line.starts_with("from ")) {
        let module = line.split_whitespace().nth(1).unwrap();
        assert!(module.contains('.'), "ambiguous import: {line}");
    }
}

fn tutorial_python_files() -> Vec<(String, String)> {
    let mut files = Vec::new();
    let mut path = None;
    let mut body: Option<String> = None;
    for line in TUTORIAL.lines() {
        if let Some(text) = body.as_mut() {
            if line.starts_with("```") {
                files.push((path.take().unwrap(), body.take().unwrap()));
            } else {
                text.push_str(line);
                text.push('\n');
            }
        } else if line == "```python" {
            body = path.is_some().then(String::new);
        } else if let Some(name) = line
            .strip_prefix("Create `")
            .and_then(|rest| rest.strip_suffix("`."))
        {
            path = Some(name.to_string());
        }
    }
    files
}

#[test]
fn tutorial_python_imports_resolve_to_tutorial_files() {
    let files = tutorial_python_files();
    assert!(files.len() >= 4, "found {} python files", files.len());
    for (file, body) in &files {
        for line in body.lines().filter(|line| line.starts_with("from ")) {
            let words: Vec<&str> = line.split_whitespace().collect();
            let target = format!("{}.py", words[1].replace('.', "/"));
            let (_, source) = files
                .iter()
                .find(|(path, _)| *path == target)
                .unwrap_or_else(|| panic!("{file}: {line}: no tutorial file {target}"));
            let name = words[3];
            assert!(
                source.contains(&format!("def {name}(")),
                "{file}: {target} does not define {name}"
            );
        }
    }
}
