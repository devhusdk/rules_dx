//! Task records and the plain-text board format every output loads.

use std::fmt;

pub const MAX_TITLE_LEN: usize = 80;
pub const MAX_TASKS: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Task {
    pub id: u32,
    pub title: String,
    pub done: bool,
    pub asset: Option<String>,
}

impl Task {
    pub fn new(id: u32, title: &str) -> Result<Self, TitleError> {
        Ok(Task {
            id,
            title: check_title(title)?,
            done: false,
            asset: None,
        })
    }

    pub fn line(&self) -> String {
        let mark = if self.done { 'x' } else { ' ' };
        match &self.asset {
            Some(asset) => format!("[{mark}] {} @{asset}", self.title),
            None => format!("[{mark}] {}", self.title),
        }
    }
}

impl fmt::Display for Task {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mark = if self.done { 'x' } else { ' ' };
        write!(f, "[{mark}] {} {}", self.id, self.title)?;
        if let Some(asset) = &self.asset {
            write!(f, " @{asset}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TitleError {
    Empty,
    TooLong,
    Reserved,
}

impl TitleError {
    pub fn kind(self) -> &'static str {
        match self {
            TitleError::Empty => "empty_title",
            TitleError::TooLong => "title_too_long",
            TitleError::Reserved => "reserved_character",
        }
    }
}

pub fn check_title(title: &str) -> Result<String, TitleError> {
    let title = title.trim();
    if title.is_empty() {
        return Err(TitleError::Empty);
    }
    if title.chars().count() > MAX_TITLE_LEN {
        return Err(TitleError::TooLong);
    }
    if title.contains(['@', '\n', '\r']) {
        return Err(TitleError::Reserved);
    }
    Ok(title.to_owned())
}

pub fn check_asset(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseError {
    MissingMarker(usize),
    Title(usize, TitleError),
    BadAsset(usize),
    TooManyTasks,
}

impl ParseError {
    pub fn kind(self) -> &'static str {
        match self {
            ParseError::MissingMarker(_) => "missing_marker",
            ParseError::Title(_, error) => error.kind(),
            ParseError::BadAsset(_) => "bad_asset",
            ParseError::TooManyTasks => "too_many_tasks",
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::MissingMarker(line) => {
                write!(f, "line {line} does not start with [ ] or [x]")
            }
            ParseError::Title(line, error) => write!(f, "line {line}: {}", error.kind()),
            ParseError::BadAsset(line) => write!(f, "line {line} names an invalid asset"),
            ParseError::TooManyTasks => write!(f, "board holds more than {MAX_TASKS} tasks"),
        }
    }
}

impl std::error::Error for ParseError {}

pub fn parse_board(text: &str) -> Result<Vec<Task>, ParseError> {
    let mut tasks = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let (done, rest) = if let Some(rest) = trimmed.strip_prefix("[ ]") {
            (false, rest)
        } else if let Some(rest) = trimmed.strip_prefix("[x]") {
            (true, rest)
        } else {
            return Err(ParseError::MissingMarker(line));
        };
        let (title, asset) = match rest.split_once('@') {
            Some((title, asset)) => {
                let asset = asset.trim();
                if !check_asset(asset) {
                    return Err(ParseError::BadAsset(line));
                }
                (title, Some(asset.to_owned()))
            }
            None => (rest, None),
        };
        if tasks.len() == MAX_TASKS {
            return Err(ParseError::TooManyTasks);
        }
        let id = u32::try_from(tasks.len() + 1).map_err(|_| ParseError::TooManyTasks)?;
        let mut task = Task::new(id, title).map_err(|error| ParseError::Title(line, error))?;
        task.done = done;
        task.asset = asset;
        tasks.push(task);
    }
    Ok(tasks)
}

pub fn format_board(tasks: &[Task]) -> String {
    tasks.iter().map(|task| task.line() + "\n").collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOARD: &str = "# release\n[x] Pin toolchains\n\n[ ] Draft notes @notes.txt\n";

    #[test]
    fn parses_markers_assets_and_ids() {
        let tasks = parse_board(BOARD).expect("board parses");
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].to_string(), "[x] 1 Pin toolchains");
        assert_eq!(tasks[1].to_string(), "[ ] 2 Draft notes @notes.txt");
        assert_eq!(tasks[1].asset.as_deref(), Some("notes.txt"));
    }

    #[test]
    fn format_round_trips() {
        let tasks = parse_board(BOARD).expect("board parses");
        let text = format_board(&tasks);
        assert_eq!(text, "[x] Pin toolchains\n[ ] Draft notes @notes.txt\n");
        assert_eq!(parse_board(&text), Ok(tasks));
    }

    #[test]
    fn rejects_malformed_lines() {
        assert_eq!(
            parse_board("[ ] ok\nnot a task"),
            Err(ParseError::MissingMarker(2))
        );
        assert_eq!(
            parse_board("[ ]   "),
            Err(ParseError::Title(1, TitleError::Empty))
        );
        assert_eq!(
            parse_board("[ ] notes @../secret"),
            Err(ParseError::BadAsset(1))
        );
        let long = format!("[ ] {}", "a".repeat(MAX_TITLE_LEN + 1));
        assert_eq!(
            parse_board(&long),
            Err(ParseError::Title(1, TitleError::TooLong))
        );
        let many = "[ ] task\n".repeat(MAX_TASKS + 1);
        assert_eq!(parse_board(&many), Err(ParseError::TooManyTasks));
    }

    #[test]
    fn titles_reject_reserved_characters() {
        assert_eq!(check_title("a @b"), Err(TitleError::Reserved));
        assert_eq!(check_title("  Ship  "), Ok("Ship".to_owned()));
        assert_eq!(TitleError::Reserved.kind(), "reserved_character");
    }

    #[test]
    fn asset_names_stay_flat() {
        assert!(check_asset("notes.txt"));
        assert!(check_asset("icon_2-x.png"));
        assert!(!check_asset(""));
        assert!(!check_asset(".."));
        assert!(!check_asset("dir/file"));
    }
}
