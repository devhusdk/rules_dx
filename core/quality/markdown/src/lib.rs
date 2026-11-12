mod check;
pub mod entry;
mod frontmatter;

#[cfg(test)]
#[path = "lib_tests.rs"]
mod lib_tests;

pub use check::{
    CheckOutcome, Finding, FindingKind, MarkdownError, check_markdown, kind_id, run_cli,
};
pub use frontmatter::slug;
