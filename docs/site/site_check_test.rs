use std::path::PathBuf;
use std::process::Command;

const ID: &str = "python:demo:AccountService.create";

struct Run {
    code: i32,
    stderr: String,
}

impl Run {
    fn expect_ok(&self) -> &Self {
        assert_eq!(
            self.code, 0,
            "expected the tree to pass, stderr {}",
            self.stderr
        );
        self
    }

    fn expect_refusal(&self, detail: &str) -> &Self {
        assert_eq!(self.code, 1, "expected a refusal naming {detail:?}");
        assert!(
            self.stderr.contains(detail),
            "stderr must name {detail:?}, got {}",
            self.stderr
        );
        self
    }

    fn expect_rejection(&self, detail: &str) -> &Self {
        assert_eq!(self.code, 2, "expected a usage rejection for {detail:?}");
        assert!(
            self.stderr.contains(detail),
            "stderr must name {detail:?}, got {}",
            self.stderr
        );
        self
    }
}

struct Site {
    root: PathBuf,
}

impl Site {
    fn new(name: &str) -> Self {
        let root =
            dx_testing::mkscratch(name).unwrap_or_else(|error| panic!("test scratch: {error}"));
        let site = Self { root };
        site.write(
            "book.toml",
            "[book]\ntitle = \"demo\"\nauthors = [\"rules_dx\"]\n",
        );
        site.write(
            "shard.ir.textproto",
            &format!("symbols {{\n  id: \"{ID}\"\n}}\n"),
        );
        site.write("api.md", &format!("# API Reference\n\n## {ID}\n"));
        site.write("guide.md", "# Guide\n\n## Getting Started\n\nText.\n");
        site
    }

    fn write(&self, rel: &str, text: &str) {
        let path = self.root.join(rel);
        let parent = path
            .parent()
            .unwrap_or_else(|| panic!("{} has a parent", path.display()));
        std::fs::create_dir_all(parent)
            .unwrap_or_else(|error| panic!("create {}: {error}", parent.display()));
        std::fs::write(&path, text)
            .unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
    }

    fn check(&self, args: &[&str]) -> Run {
        let output = Command::new(script())
            .args(args)
            .current_dir(&self.root)
            .output()
            .unwrap_or_else(|error| panic!("run site_check: {error}"));
        Run {
            code: output.status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }

    fn standard(&self) -> Run {
        self.check(&[
            "--book",
            "book.toml",
            "--api",
            "api.md",
            "--shard",
            "shard.ir.textproto",
            "--prose",
            "guide.md",
        ])
    }
}

fn script() -> PathBuf {
    let rel = std::env::var("DX_SITE_CHECK").expect("DX_SITE_CHECK must name the checker");
    dx_testing::resolve_runfiles(&rel)
}

#[test]
fn the_whole_site_passes() {
    let site = Site::new("dx-site-check");
    site.standard().expect_ok();
}

#[test]
fn every_link_form_a_real_site_uses_resolves() {
    let site = Site::new("dx-site-check-links");
    site.write(
        "guide.md",
        "# Guide\n\n## Getting Started\n\n\
         [API index](api.md)\n\
         [summary](SUMMARY.md)\n\
         [the symbol page](api/python/demo/AccountService.create.md)\n\
         [another guide](other.md)\n\
         [a directory](cli/)\n\
         [the licence](../LICENSE)\n\
         [an anchor](#getting-started)\n\
         [a fragment](other.md#usage)\n\
         [remote](https://example.com/docs)\n\
         [mail](mailto:docs@example.com)\n\
         [reference style][symbol]\n",
    );
    site.write("other.md", "# Other\n\n## Usage\n\nText.\n");
    site.write("cli/README.md", "# CLI\n\nText.\n");
    site.write("LICENSE", "Apache 2.0\n");
    site.write(
        "symbol.md",
        "# Symbols\n\n[symbol]: api/python/demo/AccountService.create.md\n",
    );
    site.check(&[
        "--book",
        "book.toml",
        "--api",
        "api.md",
        "--shard",
        "shard.ir.textproto",
        "--prose",
        "guide.md",
        "--prose",
        "other.md",
        "--prose",
        "cli/README.md",
        "--prose",
        "symbol.md",
        "--data",
        "LICENSE",
    ])
    .expect_ok();
}

#[test]
fn a_dangling_page_fails_with_the_target_named() {
    let site = Site::new("dx-site-check-page");
    site.write("guide.md", "# Guide\n\nSee [gone](missing.md).\n");
    site.standard().expect_refusal("dangling link 'missing.md'");
}

#[test]
fn a_dangling_symbol_page_fails_even_when_the_api_lists_another_symbol() {
    let site = Site::new("dx-site-check-symbol");
    site.write(
        "guide.md",
        "# Guide\n\nSee [gone](api/python/demo/AccountService.gone.md).\n",
    );
    site.standard()
        .expect_refusal("dangling link 'api/python/demo/AccountService.gone.md'");
}

#[test]
fn a_dangling_anchor_fails() {
    let site = Site::new("dx-site-check-anchor");
    site.write("guide.md", "# Guide\n\nSee [gone](#nothing-here).\n");
    site.standard()
        .expect_refusal("dangling anchor '#nothing-here'");
}

#[test]
fn a_dangling_fragment_fails() {
    let site = Site::new("dx-site-check-fragment");
    site.write(
        "guide.md",
        "# Guide\n\nSee [gone](guide.md#nothing-here).\n",
    );
    site.standard()
        .expect_refusal("dangling fragment 'guide.md#nothing-here'");
}

#[test]
fn a_reference_style_dangling_link_fails() {
    let site = Site::new("dx-site-check-reference");
    site.write(
        "guide.md",
        "# Guide\n\nSee [gone][ref].\n\n[ref]: missing.md\n",
    );
    site.standard().expect_refusal("dangling link 'missing.md'");
}

#[test]
fn an_empty_link_target_fails() {
    let site = Site::new("dx-site-check-empty");
    site.write("guide.md", "# Guide\n\nSee [gone]().\n");
    site.standard().expect_refusal("empty link target");
}

#[test]
fn prose_without_a_title_fails_even_beside_titled_prose() {
    let site = Site::new("dx-site-check-prose");
    site.write("other.md", "No title here.\n");
    site.check(&[
        "--book",
        "book.toml",
        "--api",
        "api.md",
        "--shard",
        "shard.ir.textproto",
        "--prose",
        "guide.md",
        "--prose",
        "other.md",
    ])
    .expect_refusal("prose missing title 'other.md'");
}

#[test]
fn a_book_without_a_title_fails() {
    let site = Site::new("dx-site-check-book");
    site.write("book.toml", "[book]\nauthors = [\"rules_dx\"]\n");
    site.standard().expect_refusal("book has no title");
}

#[test]
fn an_api_page_missing_a_symbol_fails() {
    let site = Site::new("dx-site-check-api");
    site.write("api.md", "# API Reference\n");
    site.standard()
        .expect_refusal(&format!("missing API page for {ID}"));
}

#[test]
fn a_shard_without_symbols_fails() {
    let site = Site::new("dx-site-check-shard");
    site.write("shard.ir.textproto", "schema_major: 1\n");
    site.standard().expect_refusal("shard names no symbols");
}

#[test]
fn an_unusable_invocation_is_rejected_before_any_check() {
    let site = Site::new("dx-site-check-usage");
    site.check(&["--nope", "book.toml"])
        .expect_rejection("unknown argument '--nope'");
    site.check(&["--book"])
        .expect_rejection("--book needs a FILE");
    site.check(&["--api", "api.md"])
        .expect_rejection("--book FILE is required");
    site.check(&["--book", "book.toml", "--api", "api.md"])
        .expect_rejection("--shard FILE is required");
    site.check(&[
        "--book",
        "book.toml",
        "--api",
        "api.md",
        "--shard",
        "shard.ir.textproto",
    ])
    .expect_rejection("--prose FILE is required");
}
