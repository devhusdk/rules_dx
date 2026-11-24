use std::io::{self, IsTerminal, Write};
use std::sync::Mutex;

const KEEP_BYTES: usize = 256 * 1024;

static TAIL: Mutex<Vec<u8>> = Mutex::new(Vec::new());

#[cfg(test)]
pub(crate) fn tail_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub(crate) fn clear() {
    TAIL.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clear();
}

pub fn last_bazel_stderr() -> String {
    let tail = TAIL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    String::from_utf8_lossy(&tail).into_owned()
}

pub(crate) fn wanted(program: &std::ffi::OsStr) -> bool {
    let name = std::path::Path::new(program)
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    name.starts_with("bazel")
}

pub(crate) fn copy_wanted(program: &std::ffi::OsStr) -> bool {
    copy_on(program, io::stderr().is_terminal(), cfg!(unix))
}

pub(crate) fn copy_on(program: &std::ffi::OsStr, terminal: bool, unix: bool) -> bool {
    wanted(program) && (unix || !terminal)
}

pub(crate) fn keep_terminal_ui(argv: &[String]) -> Vec<String> {
    let columns = terminal_columns();
    let no_color = no_color(
        std::env::var_os("NO_COLOR"),
        crate::rc::current().rc_color.chosen(argv),
    );
    keep_ui(argv, columns, no_color)
}

fn no_color(env: Option<std::ffi::OsString>, rc_chose_color: bool) -> bool {
    env.is_some_and(|value| !value.is_empty()) && !rc_chose_color
}

fn keep_ui(argv: &[String], columns: Option<u16>, no_color: bool) -> Vec<String> {
    let command = argv.iter().skip(1).position(|arg| !arg.starts_with('-'));
    let chosen = |flag: &str| {
        argv.iter()
            .take_while(|arg| *arg != "--")
            .any(|arg| arg.starts_with(flag))
    };
    let (Some(command), Some(columns)) = (command.map(|at| at + 1), columns) else {
        return argv.to_vec();
    };
    if !argv.first().is_some_and(|program| wanted(program.as_ref())) {
        return argv.to_vec();
    }
    let mut out = argv.iter().take(command + 1).cloned().collect::<Vec<_>>();
    if !chosen("--isatty") {
        out.push("--isatty=1".to_owned());
    }
    if !chosen("--terminal_columns") {
        out.push(format!("--terminal_columns={columns}"));
    }
    if no_color && !chosen("--color") {
        out.push("--color=no".to_owned());
    }
    out.extend(argv.iter().skip(command + 1).cloned());
    out
}

#[cfg(unix)]
fn terminal_columns() -> Option<u16> {
    if !io::stderr().is_terminal() {
        return None;
    }
    Some(match dx_os_sys::tty::columns(io::stderr()) {
        Ok(columns) if columns > 0 => columns,
        Ok(_) | Err(_) => 80,
    })
}

#[cfg(not(unix))]
fn terminal_columns() -> Option<u16> {
    None
}

pub(crate) struct Tee;

impl Write for Tee {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let written = io::stderr().write(buf)?;
        push(buf.get(..written).unwrap_or(buf));
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        io::stderr().flush()
    }
}

pub(crate) fn push(bytes: &[u8]) {
    let mut tail = TAIL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    tail.extend_from_slice(bytes);
    if tail.len() > KEEP_BYTES {
        let drop = tail.len() - KEEP_BYTES;
        tail.drain(..drop);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_the_last_bytes() {
        let _tail = tail_lock();
        clear();
        push(b"first ");
        push(&vec![b'x'; KEEP_BYTES]);
        let kept = last_bazel_stderr();
        assert_eq!(kept.len(), KEEP_BYTES);
        assert!(!kept.contains("first"));
        clear();
        assert_eq!(last_bazel_stderr(), "");
    }

    #[test]
    fn the_tee_writes_every_byte_through() {
        let _tail = tail_lock();
        let mut tee = Tee;
        assert_eq!(tee.write(b"tee\n").expect("write"), 4);
        tee.flush().expect("flush");
    }

    fn argv(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| (*word).to_owned()).collect()
    }

    #[test]
    fn terminal_ui_is_kept_after_the_command_word() {
        let args = argv(&["bazel", "--output_base=/o", "build", "//a", "--", "x"]);
        assert_eq!(
            keep_ui(&args, Some(120), false),
            [
                "bazel",
                "--output_base=/o",
                "build",
                "--isatty=1",
                "--terminal_columns=120",
                "//a",
                "--",
                "x"
            ]
        );
        assert!(
            !keep_ui(&args, Some(120), false)
                .iter()
                .any(|arg| arg.starts_with("--curses"))
        );
        assert_eq!(keep_ui(&args, None, false), args);
    }

    #[test]
    fn no_color_is_passed_on_unless_chosen() {
        let args = argv(&["bazel", "build", "//a"]);
        assert!(keep_ui(&args, Some(80), true).contains(&"--color=no".to_owned()));
        let chosen = argv(&["bazel", "build", "//a", "--color=yes"]);
        assert!(!keep_ui(&chosen, Some(80), true).contains(&"--color=no".to_owned()));
        assert!(no_color(Some("1".into()), false));
        assert!(!no_color(Some("1".into()), true));
        assert!(!no_color(Some("".into()), false));
        assert!(!no_color(None, false));
    }

    #[test]
    fn the_users_choice_and_other_programs_are_left_alone() {
        let args = argv(&[
            "bazel",
            "build",
            "//a",
            "--isatty=0",
            "--terminal_columns=50",
        ]);
        assert_eq!(keep_ui(&args, Some(120), false), args);
        let git = argv(&["git", "status"]);
        assert_eq!(keep_ui(&git, Some(120), false), git);
        let plain = argv(&["bazel", "build", "//a"]);
        let kept = keep_ui(&plain, Some(120), false);
        assert!(!kept.iter().any(|arg| arg.starts_with("--color=")));
        assert!(!kept.iter().any(|arg| arg.starts_with("--curses=")));
    }

    #[test]
    fn program_arguments_after_the_separator_are_not_the_users_choice() {
        let args = argv(&["bazel", "run", "//a", "--", "--isatty=0", "--color=yes"]);
        let kept = keep_ui(&args, Some(100), true);
        assert!(kept.contains(&"--isatty=1".to_owned()));
        assert!(kept.contains(&"--terminal_columns=100".to_owned()));
        assert!(kept.contains(&"--color=no".to_owned()));
    }

    #[test]
    fn windows_consoles_are_not_copied() {
        let bazel = std::ffi::OsStr::new("bazel");
        assert!(!copy_on(bazel, true, false));
        assert!(copy_on(bazel, true, true));
        assert!(copy_on(bazel, false, false));
        assert!(!copy_on(std::ffi::OsStr::new("git"), false, true));
    }

    #[test]
    fn only_bazel_programs_are_copied() {
        assert!(!wanted(std::ffi::OsStr::new("/usr/bin/git")));
    }
}
