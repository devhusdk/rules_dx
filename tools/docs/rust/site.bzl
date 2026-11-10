"""Docs site rules whose libraries may be Rust crates."""

load("//tools/docs/site:site.bzl", "docs_site_targets", "make_docs_api")
load(":aspect.bzl", "real_rust_docs_aspect")

rust_docs_api = make_docs_api([real_rust_docs_aspect])

def rust_docs_site(name, libraries, prose, book_toml, prose_route = "prose.md", source_url = ""):
    """Renders one prose page and the API reference of some Rust libraries."""
    docs_site_targets(rust_docs_api, name, libraries, prose, book_toml, prose_route, source_url)
