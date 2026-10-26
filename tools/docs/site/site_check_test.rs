use std::path::PathBuf;
use std::process::Command;

use documentation_ir::proto::{DocIr, Symbol};
use documentation_ir::{encode_shard, SCHEMA_MAJOR};

const ID: &str = "rust:docs_fixture:shapes.Circle.area";
const PAGE: &str = "api/rust/docs_fixture/shapes.Circle.area.md";

fn shard(ids: &[&str]) -> Vec<u8> {
    let shard = DocIr {
        schema_major: SCHEMA_MAJOR,
        language: "rust".to_string(),
        package: "docs_fixture".to_string(),
        symbols: ids
            .iter()
            .map(|id| Symbol {
                id: id.to_string(),
                ..Symbol::default()
            })
            .collect(),
        ..DocIr::default()
    };
    encode_shard(&shard).unwrap_or_else(|error| panic!("encode shard: {error:?}"))
}

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
        site.write_bytes("shard.ir.pb", &shard(&[ID]));
        site.write(
            "api.md",
            &format!("# API Reference\n\n- [`docs_fixture::shapes::Circle::area`]({PAGE})\n"),
        );
        site.write("guide.md", "# Guide\n\n## Getting Started\n\nText.\n");
        site
    }

    fn write(&self, rel: &str, text: &str) {
        self.write_bytes(rel, text.as_bytes());
    }

    fn write_bytes(&self, rel: &str, bytes: &[u8]) {
        let path = self.root.join(rel);
        let parent = path
            .parent()
            .unwrap_or_else(|| panic!("{} has a parent", path.display()));
        std::fs::create_dir_all(parent)
            .unwrap_or_else(|error| panic!("create {}: {error}", parent.display()));
        std::fs::write(&path, bytes)
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
            "shard.ir.pb",
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
         [the symbol page](api/rust/docs_fixture/shapes.Circle.area.md)\n\
         [another guide](other.md)\n\
         [a directory](cli/)\n\
         [an anchor](#getting-started)\n\
         [a fragment](other.md#usage)\n\
         [remote](https://example.com/docs)\n\
         [mail](mailto:docs@example.com)\n\
         [reference style][symbol]\n",
    );
    site.write("other.md", "# Other\n\n## Usage\n\nText.\n");
    site.write("cli/README.md", "# CLI\n\n[the licence](../LICENSE)\n");
    site.write("LICENSE", "Apache 2.0\n");
    site.write(
        "symbol.md",
        "# Symbols\n\n[symbol]: api/rust/docs_fixture/shapes.Circle.area.md\n",
    );
    site.check(&[
        "--book",
        "book.toml",
        "--api",
        "api.md",
        "--shard",
        "shard.ir.pb",
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
        "# Guide\n\nSee [gone](api/rust/docs_fixture/shapes.Circle.gone.md).\n",
    );
    site.standard()
        .expect_refusal("dangling link 'api/rust/docs_fixture/shapes.Circle.gone.md'");
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
        "shard.ir.pb",
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
    site.write_bytes("shard.ir.pb", &shard(&[]));
    site.standard().expect_refusal("shard names no symbols");
}

#[test]
fn a_shard_that_does_not_decode_fails() {
    let site = Site::new("dx-site-check-bad-shard");
    site.write("shard.ir.pb", "schema_major: 1\n");
    site.standard()
        .expect_refusal("shard 'shard.ir.pb' is not a valid DocIr shard");
}

#[test]
fn a_reexport_without_its_own_api_page_fails() {
    let site = Site::new("dx-site-check-alias");
    let mut ir = DocIr {
        schema_major: SCHEMA_MAJOR,
        language: "rust".to_string(),
        package: "docs_fixture".to_string(),
        ..DocIr::default()
    };
    ir.symbols.push(Symbol {
        id: ID.to_string(),
        relations: Some(documentation_ir::proto::Relations {
            reexported_as: vec!["rust:docs_fixture:area".to_string()],
            ..Default::default()
        }),
        ..Symbol::default()
    });
    let bytes = encode_shard(&ir).unwrap_or_else(|error| panic!("encode shard: {error:?}"));
    site.write_bytes("shard.ir.pb", &bytes);
    site.standard()
        .expect_refusal("missing API page for rust:docs_fixture:area");
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
        "shard.ir.pb",
    ])
    .expect_rejection("--prose FILE is required");
}

fn api_args() -> Vec<&'static str> {
    vec![
        "--book",
        "book.toml",
        "--api",
        "api.md",
        "--shard",
        "shard.ir.pb",
    ]
}

#[test]
fn a_link_to_a_page_in_another_directory_fails() {
    let site = Site::new("dx-site-check-otherdir");
    site.write("docs/a/guide.md", "# Guide\n\n[setup](setup.md)\n");
    site.write("docs/b/setup.md", "# Setup\n");
    let mut args = api_args();
    args.extend(["--prose", "docs/a/guide.md", "--prose", "docs/b/setup.md"]);
    site.check(&args).expect_refusal("dangling link 'setup.md'");
}

#[test]
fn a_parent_link_to_a_missing_root_file_fails() {
    let site = Site::new("dx-site-check-parent");
    site.write("docs/guide.md", "# Guide\n\n[top](../README.md)\n");
    site.write("docs/README.md", "# Docs\n");
    site.write("top.md", "# Top\n");
    let mut args = api_args();
    args.extend([
        "--prose",
        "docs/guide.md",
        "--prose",
        "docs/README.md",
        "--prose",
        "top.md",
    ]);
    site.check(&args)
        .expect_refusal("dangling link '../README.md'");
}

#[test]
fn a_fragment_only_on_another_page_fails() {
    let site = Site::new("dx-site-check-fragment");
    site.write("guide.md", "# Guide\n\n[x](other.md#install)\n");
    site.write("other.md", "# Other\n");
    site.write("third.md", "# Third\n\n## Install\n");
    let mut args = api_args();
    args.extend([
        "--prose", "guide.md", "--prose", "other.md", "--prose", "third.md",
    ]);
    site.check(&args)
        .expect_refusal("dangling fragment 'other.md#install'");
}
