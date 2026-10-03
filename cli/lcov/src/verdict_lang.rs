/// Line-comment syntax the coverage gate scans.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LineComment {
    SlashSlash,
    Hash,
}

/// Extensions the gate counts executable lines for, each with its line-comment syntax.
pub(crate) const COVERED_LANGUAGES: &[(&str, LineComment)] = &[
    (".rs", LineComment::SlashSlash),
    (".go", LineComment::SlashSlash),
    (".py", LineComment::Hash),
    (".js", LineComment::SlashSlash),
    (".jsx", LineComment::SlashSlash),
    (".mjs", LineComment::SlashSlash),
    (".cjs", LineComment::SlashSlash),
    (".ts", LineComment::SlashSlash),
    (".tsx", LineComment::SlashSlash),
    (".mts", LineComment::SlashSlash),
    (".cts", LineComment::SlashSlash),
    (".java", LineComment::SlashSlash),
    (".kt", LineComment::SlashSlash),
    (".scala", LineComment::SlashSlash),
    (".cs", LineComment::SlashSlash),
    (".fs", LineComment::SlashSlash),
    (".fsi", LineComment::SlashSlash),
    (".c", LineComment::SlashSlash),
    (".cc", LineComment::SlashSlash),
    (".cpp", LineComment::SlashSlash),
    (".cxx", LineComment::SlashSlash),
    (".h", LineComment::SlashSlash),
    (".hh", LineComment::SlashSlash),
    (".hpp", LineComment::SlashSlash),
    (".hxx", LineComment::SlashSlash),
];

/// Declaration stubs carry no executable lines, so the gate never counts them.
const DECLARATION_SUFFIXES: &[&str] = &[".d.ts", ".d.mts", ".d.cts"];

/// The comment syntax a covered language uses, or None when the gate does not count it.
pub(crate) fn line_comment_syntax(path: &str) -> Option<LineComment> {
    if DECLARATION_SUFFIXES
        .iter()
        .any(|suffix| path.ends_with(suffix))
    {
        return None;
    }
    COVERED_LANGUAGES
        .iter()
        .find(|(extension, _)| path.ends_with(extension))
        .map(|(_, syntax)| *syntax)
}

pub fn is_covered_language(path: &str) -> bool {
    line_comment_syntax(path).is_some()
}

pub(crate) fn is_starlark(path: &str) -> bool {
    path.ends_with(".bzl")
}
