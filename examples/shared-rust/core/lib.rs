//! Shared task and asset board driven by every product adapter.

use std::fmt;

pub use shared_tasks_model::{format_board, parse_board, ParseError, Task, TitleError};

pub const CORE_VERSION: &str = "2.0.0";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandError {
    Empty,
    Unknown(String),
    MissingArgument(&'static str),
    BadId(String),
    NoSuchTask(u32),
    AlreadyDone(u32),
    NotDone(u32),
    Title(TitleError),
    BadAsset(String),
    Full,
}

impl CommandError {
    pub fn kind(&self) -> &'static str {
        match self {
            CommandError::Empty => "empty_command",
            CommandError::Unknown(_) => "unknown_command",
            CommandError::MissingArgument(_) => "missing_argument",
            CommandError::BadId(_) => "bad_id",
            CommandError::NoSuchTask(_) => "no_such_task",
            CommandError::AlreadyDone(_) => "already_done",
            CommandError::NotDone(_) => "not_done",
            CommandError::Title(error) => error.kind(),
            CommandError::BadAsset(_) => "bad_asset",
            CommandError::Full => "board_full",
        }
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandError::Empty => write!(f, "command is empty"),
            CommandError::Unknown(verb) => write!(f, "unknown command {verb:?}"),
            CommandError::MissingArgument(what) => write!(f, "missing {what}"),
            CommandError::BadId(raw) => write!(f, "{raw:?} is not a task id"),
            CommandError::NoSuchTask(id) => write!(f, "no task {id}"),
            CommandError::AlreadyDone(id) => write!(f, "task {id} is already done"),
            CommandError::NotDone(id) => write!(f, "task {id} is not done"),
            CommandError::Title(error) => write!(f, "invalid title: {}", error.kind()),
            CommandError::BadAsset(name) => write!(f, "{name:?} is not a flat asset name"),
            CommandError::Full => {
                write!(f, "board holds {} tasks", shared_tasks_model::MAX_TASKS)
            }
        }
    }
}

impl std::error::Error for CommandError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Added(u32),
    Done(u32),
    Reopened(u32),
    Attached(u32, String),
    Removed(u32),
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Event::Added(id) => write!(f, "added {id}"),
            Event::Done(id) => write!(f, "done {id}"),
            Event::Reopened(id) => write!(f, "reopened {id}"),
            Event::Attached(id, asset) => write!(f, "attached {asset} to {id}"),
            Event::Removed(id) => write!(f, "removed {id}"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Summary {
    pub total: usize,
    pub done: usize,
    pub assets: usize,
}

impl fmt::Display for Summary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "tasks={} done={} open={} assets={}",
            self.total,
            self.done,
            self.total - self.done,
            self.assets
        )
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Board {
    tasks: Vec<Task>,
    next_id: u32,
}

impl Board {
    pub fn load(text: &str) -> Result<Self, ParseError> {
        let tasks = parse_board(text)?;
        let next_id = tasks.iter().map(|task| task.id).max().unwrap_or(0) + 1;
        Ok(Board { tasks, next_id })
    }

    pub fn tasks(&self) -> &[Task] {
        &self.tasks
    }

    pub fn save(&self) -> String {
        format_board(&self.tasks)
    }

    pub fn summary(&self) -> Summary {
        Summary {
            total: self.tasks.len(),
            done: self.tasks.iter().filter(|task| task.done).count(),
            assets: self
                .tasks
                .iter()
                .filter(|task| task.asset.is_some())
                .count(),
        }
    }

    pub fn render(&self) -> Vec<String> {
        self.tasks.iter().map(Task::to_string).collect()
    }

    pub fn apply(&mut self, command: &str) -> Result<Event, CommandError> {
        let command = command.trim();
        let (verb, rest) = command
            .split_once(char::is_whitespace)
            .map_or((command, ""), |(verb, rest)| (verb, rest.trim()));
        match verb {
            "" => Err(CommandError::Empty),
            "add" => self.add(rest),
            "done" => {
                let task = self.find(rest)?;
                if task.done {
                    return Err(CommandError::AlreadyDone(task.id));
                }
                task.done = true;
                Ok(Event::Done(task.id))
            }
            "reopen" => {
                let task = self.find(rest)?;
                if !task.done {
                    return Err(CommandError::NotDone(task.id));
                }
                task.done = false;
                Ok(Event::Reopened(task.id))
            }
            "attach" => {
                let (id, asset) = rest
                    .split_once(char::is_whitespace)
                    .ok_or(CommandError::MissingArgument("asset name"))?;
                let asset = asset.trim();
                if !shared_tasks_model::check_asset(asset) {
                    return Err(CommandError::BadAsset(asset.to_owned()));
                }
                let task = self.find(id)?;
                task.asset = Some(asset.to_owned());
                Ok(Event::Attached(task.id, asset.to_owned()))
            }
            "remove" => {
                let id = parse_id(rest)?;
                let index = self
                    .tasks
                    .iter()
                    .position(|task| task.id == id)
                    .ok_or(CommandError::NoSuchTask(id))?;
                self.tasks.remove(index);
                Ok(Event::Removed(id))
            }
            other => Err(CommandError::Unknown(other.to_owned())),
        }
    }

    fn add(&mut self, title: &str) -> Result<Event, CommandError> {
        if self.tasks.len() >= shared_tasks_model::MAX_TASKS {
            return Err(CommandError::Full);
        }
        let task = Task::new(self.next_id, title).map_err(CommandError::Title)?;
        let id = task.id;
        self.tasks.push(task);
        self.next_id += 1;
        Ok(Event::Added(id))
    }

    fn find(&mut self, raw: &str) -> Result<&mut Task, CommandError> {
        let id = parse_id(raw)?;
        self.tasks
            .iter_mut()
            .find(|task| task.id == id)
            .ok_or(CommandError::NoSuchTask(id))
    }
}

fn parse_id(raw: &str) -> Result<u32, CommandError> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(CommandError::MissingArgument("task id"));
    }
    raw.parse().map_err(|_| CommandError::BadId(raw.to_owned()))
}

pub fn asset_digest(bytes: &[u8]) -> String {
    format!("adler32:{:08x}", adler2::adler32_slice(bytes))
}

pub fn asset_report(name: &str, bytes: &[u8]) -> String {
    format!("asset {name} bytes={} {}", bytes.len(), asset_digest(bytes))
}

pub fn reply(board: &mut Board, command: &str) -> String {
    match board.apply(command) {
        Ok(event) => format!("ok:{event}"),
        Err(error) => format!("err:{}", error.kind()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOARD: &str = "[x] Pin toolchains\n[ ] Draft notes @notes.txt\n[ ] Record evidence\n";

    fn board() -> Board {
        Board::load(BOARD).expect("seed board loads")
    }

    #[test]
    fn summary_counts_state() {
        assert_eq!(
            board().summary().to_string(),
            "tasks=3 done=1 open=2 assets=1"
        );
    }

    #[test]
    fn commands_change_the_board() {
        let mut board = board();
        assert_eq!(board.apply("add Ship release"), Ok(Event::Added(4)));
        assert_eq!(board.apply("done 2"), Ok(Event::Done(2)));
        assert_eq!(
            board.apply("attach 4 build-log.txt"),
            Ok(Event::Attached(4, "build-log.txt".to_owned()))
        );
        assert_eq!(board.apply("reopen 1"), Ok(Event::Reopened(1)));
        assert_eq!(board.apply("remove 3"), Ok(Event::Removed(3)));
        assert_eq!(
            board.render(),
            vec![
                "[ ] 1 Pin toolchains",
                "[x] 2 Draft notes @notes.txt",
                "[ ] 4 Ship release @build-log.txt",
            ]
        );
        assert_eq!(
            board.summary().to_string(),
            "tasks=3 done=1 open=2 assets=2"
        );
    }

    #[test]
    fn ids_stay_unique_after_removal() {
        let mut board = board();
        assert_eq!(board.apply("remove 3"), Ok(Event::Removed(3)));
        assert_eq!(board.apply("add Again"), Ok(Event::Added(4)));
    }

    #[test]
    fn invalid_commands_report_kinds() {
        let mut board = board();
        let kinds: Vec<&str> = [
            "",
            "launch 1",
            "done",
            "done one",
            "done 9",
            "done 1",
            "reopen 2",
            "add   ",
            "add a @b",
            "attach 2",
            "attach 2 ../x",
        ]
        .iter()
        .map(|command| board.apply(command).expect_err("command is invalid").kind())
        .collect();
        assert_eq!(
            kinds,
            vec![
                "empty_command",
                "unknown_command",
                "missing_argument",
                "bad_id",
                "no_such_task",
                "already_done",
                "not_done",
                "empty_title",
                "reserved_character",
                "missing_argument",
                "bad_asset",
            ]
        );
        assert_eq!(
            board,
            self::board(),
            "failed commands leave the board alone"
        );
    }

    #[test]
    fn full_board_rejects_new_tasks() {
        let mut board = Board::load(&"[ ] task\n".repeat(shared_tasks_model::MAX_TASKS))
            .expect("full board loads");
        assert_eq!(board.apply("add one more"), Err(CommandError::Full));
    }

    #[test]
    fn save_round_trips() {
        let mut board = board();
        board.apply("done 3").expect("done");
        let reloaded = Board::load(&board.save()).expect("saved board loads");
        assert_eq!(reloaded.render(), board.render());
    }

    #[test]
    fn digest_uses_the_pinned_adler32() {
        assert_eq!(asset_digest(b""), "adler32:00000001");
        assert_eq!(asset_digest(b"Wikipedia"), "adler32:11e60398");
        assert_eq!(
            asset_report("a.txt", b"Wikipedia"),
            "asset a.txt bytes=9 adler32:11e60398"
        );
    }

    #[test]
    fn reply_formats_events_and_errors() {
        let mut board = board();
        assert_eq!(reply(&mut board, "add Ship"), "ok:added 4");
        assert_eq!(reply(&mut board, "done 7"), "err:no_such_task");
    }
}
