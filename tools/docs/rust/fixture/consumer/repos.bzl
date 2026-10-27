"""A consumer repository that documents its own plain rules_rust crate."""

_LIB_RS = '''//! Greetings a consumer of rules_dx documents with its own shard.

pub mod words;

pub use words::Greeter;

/// Greets one name with a fixed salutation.
pub fn greet(name: &str) -> String {
    format!("{}, {name}", salutation())
}

fn salutation() -> &'static str {
    "Hello"
}
'''

_WORDS_RS = '''//! Words a greeter puts together.

/// A greeter that remembers its salutation.
pub struct Greeter {
    salutation: String,
}

impl Greeter {
    /// Greets one name with the remembered salutation.
    pub fn greet(&self, name: &str) -> String {
        format!("{}, {name}", self.salutation)
    }
}
'''

_GUIDE_MD = """# Consumer Guide

Read the API reference of the `consumer_greeting` crate.
"""

_BOOK_TOML = '''[book]
title = "consumer"
authors = ["rules_dx fixture"]
language = "en"
src = "src"
'''

_BUILD_BAZEL = '''load("@rules_dx//tools/docs/rust:defs.bzl", "rust_doc_shard")
load("@rules_dx//tools/docs/site:site.bzl", "docs_site")
load("@rules_rust//rust:defs.bzl", "rust_library")

package(default_visibility = ["//visibility:public"])

rust_library(
    name = "consumer_greeting",
    srcs = [
        "src/lib.rs",
        "src/words.rs",
    ],
    crate_root = "src/lib.rs",
    edition = "2021",
)

rust_doc_shard(
    name = "api_docs",
    crate = ":consumer_greeting",
)

docs_site(
    name = "site",
    book_toml = "book.toml",
    prose = ["guide.md"],
    shards = [":api_docs"],
    source_url = "https://example.invalid/consumer/blob/main/",
)
'''

def _docs_consumer_repo_impl(ctx):
    """Writes the repository a consumer documenting its own crate would own."""
    ctx.file("BUILD.bazel", _BUILD_BAZEL)
    ctx.file("book.toml", _BOOK_TOML)
    ctx.file("guide.md", _GUIDE_MD)
    ctx.file("src/lib.rs", _LIB_RS)
    ctx.file("src/words.rs", _WORDS_RS)
    return ctx.repo_metadata(reproducible = True)

docs_consumer_repo = repository_rule(
    implementation = _docs_consumer_repo_impl,
)
