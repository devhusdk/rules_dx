#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

mod check;
pub mod entry;
mod frontmatter;

#[cfg(test)]
#[path = "lib_tests.rs"]
mod lib_tests;

pub use check::{
    check_markdown, kind_id, run_cli, CheckOutcome, Finding, FindingKind, MarkdownError,
};
pub use frontmatter::slug;
