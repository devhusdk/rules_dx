//! Rewrites absolute execroot paths in MSVC linker and archiver arguments to relative ones.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{ExitCode, ExitStatus};

/// Returns the nearest ancestor of `cwd`, or `cwd` itself, that holds `bazel-out`.
pub fn execroot(cwd: &Path) -> io::Result<PathBuf> {
    cwd.ancestors()
        .find(|dir| dir.join("bazel-out").is_dir())
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("no bazel-out in {} or above", cwd.display()),
            )
        })
}

/// Returns the prefix that leads from `cwd` back up to `root`.
pub fn up_prefix(cwd: &Path, root: &Path) -> String {
    let depth = cwd.components().count() - root.components().count();
    "../".repeat(depth)
}

/// Replaces the execroot at the start of a path with a relative prefix.
pub struct Rewriter {
    root: String,
    prefix: String,
}

impl Rewriter {
    pub fn new(root: &str, prefix: &str) -> Self {
        let root = root.strip_prefix(r"\\?\").unwrap_or(root);
        Self {
            root: root.trim_end_matches(is_sep_char).to_string(),
            prefix: prefix.to_string(),
        }
    }

    /// Finds the execroot above the current directory.
    pub fn from_cwd() -> io::Result<Self> {
        let cwd = std::env::current_dir()?;
        let root = execroot(&cwd)?;
        Ok(Self::new(&root.to_string_lossy(), &up_prefix(&cwd, &root)))
    }

    /// Replaces every `<root>\` and `<root>/` in `text`, ignoring ASCII case.
    pub fn rewrite(&self, text: &str) -> String {
        let bytes = text.as_bytes();
        let mut out = String::with_capacity(text.len());
        let mut start = 0;
        let mut i = 0;
        while i < bytes.len() {
            match self.match_at(bytes, i) {
                Some(end) => {
                    let head = &text[start..i];
                    out.push_str(head.strip_suffix(r"\\?\").unwrap_or(head));
                    out.push_str(&self.prefix);
                    start = end;
                    i = end;
                }
                None => i += 1,
            }
        }
        out.push_str(&text[start..]);
        out
    }

    fn match_at(&self, bytes: &[u8], i: usize) -> Option<usize> {
        let root = self.root.as_bytes();
        let end = i + root.len();
        if root.is_empty() || !is_sep(*bytes.get(end)?) {
            return None;
        }
        let same = root
            .iter()
            .zip(&bytes[i..end])
            .all(|(a, b)| a.eq_ignore_ascii_case(b) || (is_sep(*a) && is_sep(*b)));
        same.then_some(end + 1)
    }
}

fn is_sep(byte: u8) -> bool {
    byte == b'/' || byte == b'\\'
}

fn is_sep_char(c: char) -> bool {
    c == '/' || c == '\\'
}

/// How a response file was encoded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    Utf8,
    Utf8Bom,
    Utf16Le,
}

pub fn decode(bytes: &[u8]) -> io::Result<(String, Encoding)> {
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        let units: Vec<u16> = rest
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
            .collect();
        let text = String::from_utf16(&units).map_err(invalid)?;
        return Ok((text, Encoding::Utf16Le));
    }
    let (rest, encoding) = match bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        Some(rest) => (rest, Encoding::Utf8Bom),
        None => (bytes, Encoding::Utf8),
    };
    let text = String::from_utf8(rest.to_vec()).map_err(invalid)?;
    Ok((text, encoding))
}

pub fn encode(text: &str, encoding: Encoding) -> Vec<u8> {
    match encoding {
        Encoding::Utf8 => text.as_bytes().to_vec(),
        Encoding::Utf8Bom => [&[0xEF, 0xBB, 0xBF], text.as_bytes()].concat(),
        Encoding::Utf16Le => [0xFF, 0xFE]
            .into_iter()
            .chain(text.encode_utf16().flat_map(u16::to_le_bytes))
            .collect(),
    }
}

fn invalid(err: impl std::error::Error + Send + Sync + 'static) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, err)
}

/// Calls `edit` on each argument and on each line of every `@file` response file.
pub fn edit_args(
    args: &mut [String],
    mut edit: impl FnMut(&str) -> io::Result<String>,
) -> io::Result<()> {
    for arg in args.iter_mut() {
        match arg.strip_prefix('@') {
            Some(path) => edit_response_file(Path::new(path), &mut edit)?,
            None => *arg = edit(arg)?,
        }
    }
    Ok(())
}

fn edit_response_file(
    path: &Path,
    edit: &mut impl FnMut(&str) -> io::Result<String>,
) -> io::Result<()> {
    let (text, encoding) = decode(&fs::read(path)?)?;
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let body = line.trim_end_matches(['\r', '\n']);
        out.push_str(&edit_line(body, edit)?);
        out.push_str(&line[body.len()..]);
    }
    if out != text {
        fs::write(path, encode(&out, encoding))?;
    }
    Ok(())
}

fn edit_line(line: &str, edit: &mut impl FnMut(&str) -> io::Result<String>) -> io::Result<String> {
    match line
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
    {
        Some(quoted) => {
            let arg = edit(&quoted.replace("\\\"", "\""))?;
            Ok(format!("\"{}\"", arg.replace('"', "\\\"")))
        }
        None => edit(line),
    }
}

/// Splits an MSVC option such as `/DEF:` from its value; a plain path has no option.
pub fn split_option(arg: &str) -> (&str, &str) {
    if arg.starts_with(['/', '-']) {
        if let Some(colon) = arg.find(':') {
            if colon > 1 && arg[1..colon].bytes().all(|c| c.is_ascii_alphabetic()) {
                return arg.split_at(colon + 1);
            }
        }
    }
    ("", arg)
}

/// Returns the file name of a path that sits in a rustc temp dir such as `rustcPNODW9`.
pub fn rustc_temp_file(path: &str) -> Option<&str> {
    let mut parts = path.rsplit(is_sep_char);
    let name = parts.next()?;
    let tail = parts.next()?.strip_prefix("rustc")?;
    let random = tail.len() == 6 && tail.bytes().all(|c| c.is_ascii_alphanumeric());
    (random && !name.is_empty()).then_some(name)
}

/// Returns the directory that holds copies of rustc temp files for the link that writes `out`.
pub fn link_copy_dir(out: &str) -> String {
    let (dir, file) = match out.rfind(is_sep_char) {
        Some(i) => out.split_at(i + 1),
        None => ("", out),
    };
    let stem = file.rsplit_once('.').map_or(file, |(stem, _)| stem);
    format!("{dir}_dx_link/{stem}")
}

pub fn exit_code(status: ExitStatus) -> ExitCode {
    match status.code() {
        Some(0) => ExitCode::SUCCESS,
        Some(code) => ExitCode::from(u8::try_from(code).unwrap_or(1).max(1)),
        None => ExitCode::FAILURE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROOT: &str = r"C:\b\out\execroot\_main";

    fn scratch(name: &str) -> PathBuf {
        let base = std::env::var_os("TEST_TMPDIR").map_or_else(std::env::temp_dir, PathBuf::from);
        let dir = base.join(name);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn rewrites_backslash_paths() {
        let rewriter = Rewriter::new(ROOT, "");
        assert_eq!(
            rewriter.rewrite(r"/LIBPATH:c:\b\out\execroot\_main\bazel-out\x\ring.out_dir"),
            r"/LIBPATH:bazel-out\x\ring.out_dir"
        );
    }

    #[test]
    fn rewrites_forward_and_mixed_paths() {
        let rewriter = Rewriter::new(ROOT, "");
        assert_eq!(
            rewriter.rewrite("C:/b/out/execroot/_main/bazel-out/x/libdx_cli.rlib"),
            "bazel-out/x/libdx_cli.rlib"
        );
        assert_eq!(
            rewriter.rewrite(r"C:\b\out\execroot\_main\bazel-out/x/cli\rustcPNODW9\symbols.o"),
            r"bazel-out/x/cli\rustcPNODW9\symbols.o"
        );
    }

    #[test]
    fn rewrites_verbatim_paths_with_a_prefix() {
        let rewriter = Rewriter::new(&format!("{ROOT}\\"), "../../");
        assert_eq!(
            rewriter
                .rewrite(r"\\?\C:\b\out\execroot\_main\bazel-out\a.o C:\b\out\execroot\_main\b.o"),
            r"../../bazel-out\a.o ../../b.o"
        );
    }

    #[test]
    fn keeps_siblings_and_relative_paths() {
        let rewriter = Rewriter::new(ROOT, "");
        for text in [
            r"C:\b\out\execroot\_main2\a.o",
            "bazel-out/a.o",
            "C:/b/out/execroot/_main",
        ] {
            assert_eq!(rewriter.rewrite(text), text);
        }
    }

    #[test]
    fn response_files_keep_their_encoding() {
        let dir = scratch("rsp");
        let rewriter = Rewriter::new(ROOT, "");
        let text = "\"C:\\b\\out\\execroot\\_main\\bazel-out\\a.rlib\"\r\n\"/OUT:say \\\"hi\\\"\"\r\nkernel32.lib\n";
        let want = "\"bazel-out\\a.rlib\"\r\n\"/OUT:say \\\"hi\\\"\"\r\nkernel32.lib\n";
        for encoding in [Encoding::Utf8, Encoding::Utf8Bom, Encoding::Utf16Le] {
            let path = dir.join(format!("{encoding:?}.rsp"));
            fs::write(&path, encode(text, encoding)).unwrap();
            let mut seen = Vec::new();
            let mut args = vec![format!("@{}", path.display())];
            edit_args(&mut args, |arg| {
                seen.push(arg.to_string());
                Ok(rewriter.rewrite(arg))
            })
            .unwrap();
            assert_eq!(seen[1], "/OUT:say \"hi\"");
            assert_eq!(
                decode(&fs::read(&path).unwrap()).unwrap(),
                (want.to_string(), encoding)
            );
        }
    }

    #[test]
    fn rustc_temp_files_move_next_to_the_output() {
        let arg = r"bazel-out/x/bin/cli/cli\rustcPNODW9\symbols.o";
        assert_eq!(split_option(arg), ("", arg));
        assert_eq!(rustc_temp_file(arg), Some("symbols.o"));
        let (option, value) = split_option(r"/DEF:bazel-out\x\rustcAb12Cd\lib.def");
        assert_eq!((option, rustc_temp_file(value)), ("/DEF:", Some("lib.def")));
        assert_eq!(rustc_temp_file("bazel-out/x/rustc_ast/lib.o"), None);
        assert_eq!(
            link_copy_dir("bazel-out/x/bin/cli/cli/dx.exe"),
            "bazel-out/x/bin/cli/cli/_dx_link/dx"
        );
        assert_eq!(link_copy_dir("dx.dll"), "_dx_link/dx");
    }

    #[test]
    fn execroot_is_the_nearest_dir_with_bazel_out() {
        let dir = scratch("execroot");
        let cwd = dir.join("external").join("ring");
        fs::create_dir_all(dir.join("bazel-out")).unwrap();
        fs::create_dir_all(&cwd).unwrap();
        assert_eq!(execroot(&cwd).unwrap(), dir);
        assert_eq!(up_prefix(&cwd, &dir), "../../");
        assert_eq!(up_prefix(&dir, &dir), "");
    }

    #[test]
    fn missing_execroot_is_an_error() {
        let root = Path::new(std::path::MAIN_SEPARATOR_STR);
        let err = execroot(root).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
        assert!(err.to_string().contains("no bazel-out"), "{err}");
    }
}
