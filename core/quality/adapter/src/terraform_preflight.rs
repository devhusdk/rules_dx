use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

pub const RULE_MODULE_SOURCE: &str = "dx/terraform-module-source";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub line: u64,
    pub column: u64,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleSource {
    pub value: Option<String>,
    pub line: u64,
    pub column: u64,
}

#[must_use]
pub fn is_terraform(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    name.ends_with(".tf") || name.ends_with(".tf.json")
}

#[must_use]
pub fn is_terraform_test(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    name.ends_with(".tftest.hcl") || name.ends_with(".tftest.json")
}

const TEST_DIR: &str = "tests";

pub fn module_sources(name: &str, bytes: &[u8]) -> Result<Vec<ModuleSource>, String> {
    let text = std::str::from_utf8(bytes).map_err(|err| format!("not valid UTF-8: {err}"))?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    if name.ends_with(".tftest.json") {
        json_test_sources(text)
    } else if name.ends_with(".tftest.hcl") {
        hcl_sources(text, &["run", "module"])
    } else if name.ends_with(".tf.json") {
        json_sources(text)
    } else {
        hcl_sources(text, &["module"])
    }
}

#[must_use]
pub fn local_path(source: &str) -> Option<&str> {
    let local = source.starts_with("./") || source.starts_with("../");
    let plain = !source.contains(['\\', ':', '?', '$', '%']) && !source.contains("//");
    (local && plain).then_some(source)
}

pub fn check_file(root: &Path, file: &Path) -> Result<Option<Problem>, String> {
    let root = normalize(root).ok_or_else(|| format!("bad scan root {}", root.display()))?;
    let Some(file) = normalize(file) else {
        return Err(format!("bad Terraform path {}", file.display()));
    };
    let Some(dir) = file.parent() else {
        return Err(format!(
            "Terraform file {} has no directory",
            file.display()
        ));
    };
    check_sources(&root, &file, dir)
}

pub fn check_init_dir(root: &Path, dir: &Path) -> Result<Vec<(PathBuf, Problem)>, String> {
    let root = normalize(root).ok_or_else(|| format!("bad scan root {}", root.display()))?;
    let Some(dir) = normalize(dir) else {
        return Err(format!("bad Terraform directory {}", dir.display()));
    };
    let mut files = files_in(&dir, |path| is_terraform(path) || is_terraform_test(path))?;
    let tests = dir.join(TEST_DIR);
    if tests.is_dir() {
        files.extend(files_in(&tests, is_terraform_test)?);
    }
    let mut out = Vec::new();
    for file in files {
        if let Some(problem) = check_sources(&root, &file, &dir)? {
            out.push((file, problem));
        }
    }
    Ok(out)
}

fn check_sources(root: &Path, file: &Path, dir: &Path) -> Result<Option<Problem>, String> {
    let bytes = std::fs::read(file).map_err(|err| format!("read {}: {err}", file.display()))?;
    let name = file.to_string_lossy();
    let sources = match module_sources(&name, &bytes) {
        Ok(sources) => sources,
        Err(err) => {
            return Ok(Some(at(
                1,
                1,
                format!("dx cannot read the module sources: {err}"),
            )));
        }
    };
    let mut seen = BTreeSet::new();
    let mut queue: Vec<(PathBuf, u64, u64)> = Vec::new();
    for source in sources {
        match follow(root, dir, &source) {
            Ok(next) => queue.push((next, source.line, source.column)),
            Err(message) => return Ok(Some(at(source.line, source.column, message))),
        }
    }
    while let Some((module, line, column)) = queue.pop() {
        if !seen.insert(module.clone()) || !module.is_dir() {
            continue;
        }
        for path in files_in(&module, is_terraform)? {
            let shown = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .display()
                .to_string();
            let bytes =
                std::fs::read(&path).map_err(|err| format!("read {}: {err}", path.display()))?;
            let sources = match module_sources(&path.to_string_lossy(), &bytes) {
                Ok(sources) => sources,
                Err(err) => {
                    return Ok(Some(at(
                        line,
                        column,
                        format!("dx cannot read the module sources of {shown}: {err}"),
                    )));
                }
            };
            for source in sources {
                match follow(root, &module, &source) {
                    Ok(next) => queue.push((next, line, column)),
                    Err(message) => {
                        return Ok(Some(at(
                            line,
                            column,
                            format!("{shown}:{}: {message}", source.line),
                        )));
                    }
                }
            }
        }
    }
    Ok(None)
}

fn at(line: u64, column: u64, message: String) -> Problem {
    Problem {
        line,
        column,
        message,
    }
}

fn follow(root: &Path, dir: &Path, source: &ModuleSource) -> Result<PathBuf, String> {
    let Some(value) = &source.value else {
        return Err(
            "module source must be a plain string: dx never downloads Terraform modules".to_owned(),
        );
    };
    let Some(rel) = local_path(value) else {
        return Err(format!(
            "module source {value:?} is not a local ./ or ../ path: dx never downloads Terraform modules"
        ));
    };
    match normalize(&dir.join(rel)) {
        Some(target) if target.starts_with(root) => Ok(target),
        _ => Err(format!(
            "module source {value:?} leaves the files dx scans: list the module's files in the target"
        )),
    }
}

fn normalize(path: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    return None;
                }
            }
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => out.push(part),
        }
    }
    Some(out)
}

fn files_in(dir: &Path, keep: fn(&Path) -> bool) -> Result<Vec<PathBuf>, String> {
    let entries = std::fs::read_dir(dir).map_err(|err| format!("read {}: {err}", dir.display()))?;
    let mut files = Vec::new();
    for entry in entries {
        let path = entry
            .map_err(|err| format!("read {}: {err}", dir.display()))?
            .path();
        if keep(&path) && path.is_file() {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

fn json_sources(text: &str) -> Result<Vec<ModuleSource>, String> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|err| format!("not valid JSON: {err}"))?;
    let mut out = Vec::new();
    if let Some(modules) = value.get("module") {
        for body in json_bodies(modules) {
            if let Some(source) = body.get("source") {
                out.push(ModuleSource {
                    value: source.as_str().map(str::to_owned),
                    line: 1,
                    column: 1,
                });
            }
        }
    }
    Ok(out)
}

fn json_test_sources(text: &str) -> Result<Vec<ModuleSource>, String> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|err| format!("not valid JSON: {err}"))?;
    let mut out = Vec::new();
    if let Some(runs) = value.get("run") {
        for run in json_bodies(runs) {
            let modules: Vec<&serde_json::Value> = match run.get("module") {
                Some(serde_json::Value::Array(items)) => items.iter().collect(),
                Some(module) => vec![module],
                None => Vec::new(),
            };
            for source in modules
                .into_iter()
                .filter_map(|module| module.get("source"))
            {
                out.push(ModuleSource {
                    value: source.as_str().map(str::to_owned),
                    line: 1,
                    column: 1,
                });
            }
        }
    }
    Ok(out)
}

fn json_bodies(modules: &serde_json::Value) -> Vec<&serde_json::Map<String, serde_json::Value>> {
    let mut out = Vec::new();
    let tops: Vec<&serde_json::Value> = match modules {
        serde_json::Value::Array(items) => items.iter().collect(),
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_)
        | serde_json::Value::Object(_) => vec![modules],
    };
    for top in tops {
        let Some(named) = top.as_object() else {
            continue;
        };
        for body in named.values() {
            match body {
                serde_json::Value::Object(map) => out.push(map),
                serde_json::Value::Array(items) => {
                    out.extend(items.iter().filter_map(serde_json::Value::as_object));
                }
                serde_json::Value::Null
                | serde_json::Value::Bool(_)
                | serde_json::Value::Number(_)
                | serde_json::Value::String(_) => {}
            }
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Ident(String),
    Str(Option<String>),
    Heredoc,
    Open,
    Close,
    Equals,
    Newline,
    Other,
}

struct Lexer {
    chars: Vec<char>,
    pos: usize,
    line: u64,
    column: u64,
}

impl Lexer {
    fn peek(&self, ahead: usize) -> Option<char> {
        self.chars.get(self.pos + ahead).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let next = self.chars.get(self.pos).copied()?;
        self.pos += 1;
        if next == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(next)
    }

    fn fail(&self, what: &str) -> String {
        format!("{what} at line {}", self.line)
    }

    fn tokens(mut self) -> Result<Vec<(Token, u64, u64)>, String> {
        let mut out = Vec::new();
        while let Some(next) = self.peek(0) {
            let (line, column) = (self.line, self.column);
            let token = match next {
                '\n' => {
                    self.bump();
                    Token::Newline
                }
                ' ' | '\t' | '\r' => {
                    self.bump();
                    continue;
                }
                '#' => {
                    self.line_comment();
                    continue;
                }
                '/' if self.peek(1) == Some('/') => {
                    self.line_comment();
                    continue;
                }
                '/' if self.peek(1) == Some('*') => {
                    if self.block_comment()? {
                        Token::Newline
                    } else {
                        continue;
                    }
                }
                '"' => Token::Str(self.string()?),
                '<' if self.peek(1) == Some('<') => {
                    self.heredoc()?;
                    Token::Heredoc
                }
                '{' => {
                    self.bump();
                    Token::Open
                }
                '}' => {
                    self.bump();
                    Token::Close
                }
                '=' if self.peek(1) != Some('=') && self.peek(1) != Some('>') => {
                    self.bump();
                    Token::Equals
                }
                c if c.is_alphabetic() || c == '_' => Token::Ident(self.ident()),
                _ => {
                    self.bump();
                    Token::Other
                }
            };
            out.push((token, line, column));
        }
        Ok(out)
    }

    fn line_comment(&mut self) {
        while let Some(next) = self.peek(0) {
            if next == '\n' {
                return;
            }
            self.bump();
        }
    }

    fn block_comment(&mut self) -> Result<bool, String> {
        let start = self.line;
        self.bump();
        self.bump();
        loop {
            match self.bump() {
                Some('*') if self.peek(0) == Some('/') => {
                    self.bump();
                    return Ok(self.line != start);
                }
                Some(_) => {}
                None => return Err(self.fail("unterminated comment")),
            }
        }
    }

    fn ident(&mut self) -> String {
        let mut out = String::new();
        while let Some(next) = self.peek(0) {
            if next.is_alphanumeric() || next == '_' || next == '-' {
                out.push(next);
                self.bump();
            } else {
                break;
            }
        }
        out
    }

    fn string(&mut self) -> Result<Option<String>, String> {
        self.bump();
        let mut out = String::new();
        let mut plain = true;
        loop {
            match self.bump() {
                Some('"') => return Ok(plain.then_some(out)),
                Some('\\') => match self.bump() {
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some('r') => out.push('\r'),
                    Some(c @ ('"' | '\\')) => out.push(c),
                    Some(_) => plain = false,
                    None => return Err(self.fail("unterminated string")),
                },
                Some(c @ ('$' | '%')) if self.peek(0) == Some(c) && self.peek(1) == Some('{') => {
                    self.bump();
                    out.push(c);
                }
                Some('$' | '%') if self.peek(0) == Some('{') => {
                    self.bump();
                    plain = false;
                    self.template()?;
                }
                Some('\n') | None => return Err(self.fail("unterminated string")),
                Some(c) => out.push(c),
            }
        }
    }

    fn template(&mut self) -> Result<(), String> {
        let mut depth = 1_usize;
        while depth > 0 {
            match self.peek(0) {
                Some('"') => {
                    self.string()?;
                }
                Some('{') => {
                    self.bump();
                    depth += 1;
                }
                Some('}') => {
                    self.bump();
                    depth -= 1;
                }
                Some(_) => {
                    self.bump();
                }
                None => return Err(self.fail("unterminated template")),
            }
        }
        Ok(())
    }

    fn heredoc(&mut self) -> Result<(), String> {
        self.bump();
        self.bump();
        if self.peek(0) == Some('-') {
            self.bump();
        }
        let marker = self.ident();
        if marker.is_empty() {
            return Err(self.fail("heredoc without a marker"));
        }
        while self.peek(0).is_some_and(|c| c != '\n') {
            self.bump();
        }
        loop {
            if self.bump().is_none() {
                return Err(self.fail("unterminated heredoc"));
            }
            let mut text = String::new();
            while let Some(next) = self.peek(0) {
                if next == '\n' {
                    break;
                }
                text.push(next);
                self.bump();
            }
            if text.trim() == marker {
                return Ok(());
            }
        }
    }
}

fn hcl_sources(text: &str, path: &[&str]) -> Result<Vec<ModuleSource>, String> {
    let lexer = Lexer {
        chars: text.chars().collect(),
        pos: 0,
        line: 1,
        column: 1,
    };
    let tokens = lexer.tokens()?;
    let mut out = Vec::new();
    let mut blocks: Vec<Option<&str>> = Vec::new();
    let mut header: Option<(&str, usize)> = None;
    let mut statement_start = true;
    let mut index = 0;
    while let Some((token, line, column)) = tokens.get(index) {
        let at_start = statement_start;
        statement_start = matches!(token, Token::Newline | Token::Open | Token::Close);
        match token {
            Token::Open => {
                let kind = header
                    .take()
                    .filter(|(_, open)| *open == index)
                    .map(|(kind, _)| kind);
                blocks.push(kind);
            }
            Token::Close => {
                if blocks.pop().is_none() {
                    return Err(format!("unbalanced braces at line {line}"));
                }
            }
            Token::Ident(word)
                if at_start
                    && word == "source"
                    && matches!(tokens.get(index + 1), Some((Token::Equals, _, _))) =>
            {
                let inside = blocks.len() == path.len()
                    && blocks
                        .iter()
                        .zip(path)
                        .all(|(kind, want)| *kind == Some(*want));
                if inside {
                    let value = match (tokens.get(index + 2), tokens.get(index + 3)) {
                        (
                            Some((Token::Str(value), _, _)),
                            None | Some((Token::Newline | Token::Close, _, _)),
                        ) => value.clone(),
                        _ => None,
                    };
                    out.push(ModuleSource {
                        value,
                        line: *line,
                        column: *column,
                    });
                }
            }
            Token::Ident(word) if at_start => {
                let mut ahead = index + 1;
                while matches!(
                    tokens.get(ahead),
                    Some((Token::Str(_) | Token::Ident(_), _, _))
                ) {
                    ahead += 1;
                }
                if matches!(tokens.get(ahead), Some((Token::Open, _, _))) {
                    header = Some((word.as_str(), ahead));
                }
            }
            Token::Ident(_)
            | Token::Str(_)
            | Token::Heredoc
            | Token::Equals
            | Token::Newline
            | Token::Other => {}
        }
        index += 1;
    }
    if !blocks.is_empty() {
        return Err("unbalanced braces at end of file".to_owned());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{ModuleSource, check_file, check_init_dir, local_path, module_sources};

    fn values(name: &str, text: &str) -> Vec<Option<String>> {
        module_sources(name, text.as_bytes())
            .expect(text)
            .into_iter()
            .map(|source| source.value)
            .collect()
    }

    fn some(value: &str) -> Option<String> {
        Some(value.to_owned())
    }

    #[test]
    fn hcl_module_sources_are_found() {
        let text = r#"
# module "commented" { source = "git::https://example.com/x.git" }
/* module "block" {
  source = "hashicorp/consul/aws"
} */
resource "aws_s3_bucket" "b" {
  bucket = "module { source = \"x\" }"
  source = "not a module"
}

module "vpc" {
  source  = "terraform-aws-modules/vpc/aws"
  version = "5.0.0"
  tags = {
    source = "nested map key"
  }
}

module "local" { source = "./modules/local" }
module "parent" {
  // comment
  source = "../shared"
}
"#;
        assert_eq!(
            values("main.tf", text),
            vec![
                some("terraform-aws-modules/vpc/aws"),
                some("./modules/local"),
                some("../shared"),
            ]
        );
    }

    #[test]
    fn hcl_templates_and_expressions_are_not_plain() {
        let text = "module \"a\" {\n  source = \"./${var.dir}\"\n}\nmodule \"b\" {\n  source = local.dir\n}\nmodule \"c\" {\n  source = \"./a\" # ok\n}\nmodule \"d\" {\n  source = \"$${x}\"\n}\n";
        assert_eq!(
            values("main.tf", text),
            vec![None, None, some("./a"), some("${x}")]
        );
    }

    #[test]
    fn hcl_heredocs_hide_their_text() {
        let text = "locals {\n  doc = <<-EOT\n    module \"x\" {\n      source = \"git::x\"\n    }\n  EOT\n}\nmodule \"y\" {\n  source = \"./y\"\n}\n";
        assert_eq!(values("main.tf", text), vec![some("./y")]);
    }

    #[test]
    fn hcl_positions_point_at_source() {
        let sources =
            module_sources("m.tf", b"module \"a\" {\n  source = \"x\"\n}\n").expect("parse");
        assert_eq!(
            sources,
            vec![ModuleSource {
                value: some("x"),
                line: 2,
                column: 3
            }]
        );
    }

    #[test]
    fn bad_hcl_is_refused() {
        for text in [
            "module \"a\" {\n  source = \"./a\n}\n",
            "module \"a\" {\n",
            "}\n",
            "/* open",
            "x = <<EOT\nno end\n",
        ] {
            module_sources("main.tf", text.as_bytes()).expect_err(text);
        }
        module_sources("main.tf", &[0xff]).expect_err("utf8");
    }

    #[test]
    fn json_module_sources_are_found() {
        let text = r#"{"module": {"a": {"source": "./a"}, "b": [{"source": "hashicorp/x/aws"}], "c": {"source": 3}}, "resource": {"source": "y"}}"#;
        assert_eq!(
            values("main.tf.json", text),
            vec![some("./a"), some("hashicorp/x/aws"), None]
        );
        let list = r#"{"module": [{"a": {"source": "../a"}}]}"#;
        assert_eq!(values("main.tf.json", list), vec![some("../a")]);
        module_sources("main.tf.json", b"{").expect_err("bad json");
    }

    #[test]
    fn only_plain_local_paths_are_local() {
        for local in ["./a", "../a/b", "./"] {
            assert!(local_path(local).is_some(), "{local}");
        }
        for remote in [
            "hashicorp/consul/aws",
            "git::https://example.com/x.git",
            "github.com/acme/x",
            "s3::https://s3.amazonaws.com/b/x.zip",
            "./a//sub",
            ".\\a",
            "./${x}",
            "/abs/path",
            "a",
            "",
        ] {
            assert!(local_path(remote).is_none(), "{remote}");
        }
    }

    #[test]
    fn check_file_follows_local_modules() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        let write = |rel: &str, text: &str| {
            let path = root.join(rel);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            std::fs::write(path, text).expect("write");
        };
        write(
            "app/main.tf",
            "module \"net\" {\n  source = \"../modules/net\"\n}\n",
        );
        write(
            "modules/net/main.tf",
            "module \"sub\" {\n  source = \"./sub\"\n}\n",
        );
        write(
            "modules/net/sub/main.tf.json",
            r#"{"module": {"x": {"source": "../../net"}}}"#,
        );
        assert_eq!(
            check_file(root, &root.join("app/main.tf")).expect("check"),
            None
        );

        write(
            "modules/net/sub/extra.tf",
            "module \"r\" {\n  source = \"hashicorp/consul/aws\"\n}\n",
        );
        let problem = check_file(root, &root.join("app/main.tf"))
            .expect("check")
            .expect("problem");
        assert_eq!((problem.line, problem.column), (2, 3));
        assert!(
            problem.message.contains("extra.tf:2"),
            "{}",
            problem.message
        );
        assert!(
            problem.message.contains("hashicorp/consul/aws"),
            "{}",
            problem.message
        );

        write("top.tf", "module \"out\" {\n  source = \"../outside\"\n}\n");
        let problem = check_file(root, &root.join("top.tf"))
            .expect("check")
            .expect("problem");
        assert!(
            problem.message.contains("leaves the files"),
            "{}",
            problem.message
        );

        write(
            "missing.tf",
            "module \"m\" {\n  source = \"./not-staged\"\n}\n",
        );
        assert_eq!(
            check_file(root, &root.join("missing.tf")).expect("check"),
            None
        );

        write("bad.tf", "module \"m\" {\n");
        let problem = check_file(root, &root.join("bad.tf"))
            .expect("check")
            .expect("problem");
        assert!(
            problem.message.contains("cannot read"),
            "{}",
            problem.message
        );

        check_file(root, &root.join("absent.tf")).expect_err("absent file");
    }

    #[test]
    fn test_file_run_module_sources_are_found() {
        let text = r#"
module "top" { source = "not/in/a/run" }
run "a" {
  command = plan
  module {
    source  = "hashicorp/consul/aws"
    version = "1.0.0"
  }
  variables {
    source = "a variable"
  }
}
run "b" {
  module { source = "./setup" }
}
"#;
        assert_eq!(
            values("a.tftest.hcl", text),
            vec![some("hashicorp/consul/aws"), some("./setup")]
        );
        let json = r#"{"run": {"a": {"module": {"source": "x/y/aws"}}, "b": [{"module": [{"source": "./s"}]}], "c": {}}}"#;
        assert_eq!(
            values("a.tftest.json", json),
            vec![some("x/y/aws"), some("./s")]
        );
    }

    #[test]
    fn check_init_dir_reads_every_file_init_reads() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        let write = |rel: &str, text: &str| {
            let path = root.join(rel);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            std::fs::write(path, text).expect("write");
        };
        write("infra/main.tf", "resource \"terraform_data\" \"x\" {}\n");
        write(
            "infra/setup/main.tf",
            "resource \"terraform_data\" \"y\" {}\n",
        );
        write(
            "infra/tests/local.tftest.hcl",
            "run \"x\" {\n  module {\n    source = \"./setup\"\n  }\n}\n",
        );
        write("infra/notes.txt", "module \"m\" { source = \"x/y/aws\" }\n");
        write(
            "infra/tests/deeper/skip.tftest.hcl",
            "run \"x\" {\n  module {\n    source = \"x/y/aws\"\n  }\n}\n",
        );
        assert_eq!(
            check_init_dir(root, &root.join("infra")).expect("check"),
            Vec::new()
        );

        write(
            "infra/dep.tf",
            "module \"consul\" {\n  source = \"hashicorp/consul/aws\"\n}\n",
        );
        write(
            "infra/root.tftest.json",
            r#"{"run": {"x": {"module": {"source": "x/y/aws"}}}}"#,
        );
        write(
            "infra/tests/remote.tftest.hcl",
            "run \"x\" {\n  module {\n    source = \"x/y/aws\"\n  }\n}\n",
        );
        write(
            "infra/setup/nested.tf",
            "module \"n\" {\n  source = \"git::https://example.com/n.git\"\n}\n",
        );
        let found: Vec<(String, u64, u64)> = check_init_dir(root, &root.join("infra"))
            .expect("check")
            .into_iter()
            .map(|(file, problem)| {
                let rel = file.strip_prefix(root).expect("under root");
                (
                    rel.to_string_lossy().replace('\\', "/"),
                    problem.line,
                    problem.column,
                )
            })
            .collect();
        assert_eq!(
            found,
            vec![
                ("infra/dep.tf".to_owned(), 2, 3),
                ("infra/root.tftest.json".to_owned(), 1, 1),
                ("infra/tests/local.tftest.hcl".to_owned(), 3, 5),
                ("infra/tests/remote.tftest.hcl".to_owned(), 3, 5),
            ]
        );

        check_init_dir(root, &root.join("absent")).expect_err("absent dir");
    }
}
