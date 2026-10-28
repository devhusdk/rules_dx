//! Tests for the API aggregator.

use super::{api_path, code, parse, relative, render, title, Options};
use documentation_ir::proto::{DocIr, Relations, SourceRef, Symbol, SymbolKind};
use documentation_ir::{SCHEMA_MAJOR, SCHEMA_MINOR};

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

fn symbol(id: &str, signature: &str, doc: &str, line: u64) -> Symbol {
    Symbol {
        id: id.to_string(),
        kind: SymbolKind::Function as i32,
        signature_text: signature.to_string(),
        doc_markdown: doc.to_string(),
        source: Some(SourceRef {
            file: "pkg/src/lib.rs".to_string(),
            line,
            generated: false,
        }),
        ..Symbol::default()
    }
}

fn shard() -> DocIr {
    let mut circle = symbol(
        "rust:demo:shapes.Circle",
        "pub struct Circle { .. }",
        "A circle.\n\n# Examples\n\n```\n# let x = 1;\n```",
        4,
    );
    circle.relations = Some(Relations {
        implements: vec!["rust:core:clone.Clone".to_string()],
        reexported_as: vec!["rust:demo:Circle".to_string()],
        ..Relations::default()
    });
    let mut area = symbol(
        "rust:demo:shapes.Circle.area",
        "pub fn area(&self) -> f64",
        "Area.",
        9,
    );
    area.relations = Some(Relations {
        member_of: vec!["rust:demo:shapes.Circle".to_string()],
        ..Relations::default()
    });
    let mut generated = symbol(
        "rust:demo:VERSION",
        "pub const VERSION: &str = \"1\";",
        "",
        2,
    );
    if let Some(source) = generated.source.as_mut() {
        source.file = "pkg/build.out_dir/gen.rs".to_string();
        source.generated = true;
    }
    DocIr {
        schema_major: SCHEMA_MAJOR,
        schema_minor: SCHEMA_MINOR,
        language: "rust".to_string(),
        package: "demo".to_string(),
        symbols: vec![
            generated,
            symbol(
                "rust:demo:add",
                "pub fn add(a: u64) -> u64",
                "Adds.\n\nMore.",
                7,
            ),
            circle,
            area,
        ],
    }
}

#[test]
fn parse_needs_a_shard_and_every_output() {
    let full = args(&[
        "--shard",
        "a.pb",
        "--shard",
        "b.pb",
        "--source-url",
        "https://x/",
        "--summary",
        "s",
        "--api",
        "a",
        "--api-tree",
        "t",
        "--records",
        "r",
    ]);
    let options = parse(&full).expect("full");
    assert_eq!(
        options,
        Options {
            shards: vec!["a.pb".to_string(), "b.pb".to_string()],
            source_url: "https://x/".to_string(),
            summary: "s".to_string(),
            api: "a".to_string(),
            api_tree: "t".to_string(),
            records: "r".to_string(),
        }
    );
    assert_eq!(parse(&full[4..]), Err("--shard is required".to_string()));
    assert_eq!(parse(&full[..12]), Err("--records is required".to_string()));
    assert_eq!(
        parse(&args(&["--x", "y"])),
        Err("unknown argument '--x'".to_string())
    );
}

#[test]
fn ids_map_to_pages_titles_and_relative_links() {
    assert_eq!(
        api_path("rust:demo:shapes.Circle"),
        "api/rust/demo/shapes.Circle.md"
    );
    assert_eq!(title("rust:demo:shapes.Circle"), "demo::shapes::Circle");
    assert_eq!(title("python:demo:Service.create"), "demo.Service.create");
    assert_eq!(relative("api/rust/demo/a.md", "api/rust/demo/b.md"), "b.md");
    assert_eq!(
        relative("api/rust/demo/a.md", "api/rust/other/b.md"),
        "../other/b.md"
    );
    assert_eq!(
        relative("api.md", "api/rust/demo/b.md"),
        "api/rust/demo/b.md"
    );
}

#[test]
fn every_symbol_and_alias_gets_a_page_index_entry_summary_line_and_record() {
    let rendered = render(&[shard()], "https://example.com/blob/main/").expect("render");
    let pages: Vec<&str> = rendered.pages.keys().map(String::as_str).collect();
    assert_eq!(
        pages,
        vec![
            "rust/demo/Circle.md",
            "rust/demo/VERSION.md",
            "rust/demo/add.md",
            "rust/demo/shapes.Circle.area.md",
            "rust/demo/shapes.Circle.md",
        ]
    );
    let add = &rendered.pages["rust/demo/add.md"];
    assert_eq!(
        add,
        "# `demo::add`\n\n```rust,noplayground\npub fn add(a: u64) -> u64\n```\n\nAdds.\n\nMore.\n\nSource: [pkg/src/lib.rs:7](https://example.com/blob/main/pkg/src/lib.rs#L7)\n"
    );
    let circle = &rendered.pages["rust/demo/shapes.Circle.md"];
    assert!(circle.contains("\n### Examples\n"), "{circle}");
    assert!(circle.contains("```\n# let x = 1;\n```"), "{circle}");
    assert!(
        circle.contains("Implements: `core::clone::Clone`"),
        "{circle}"
    );
    assert!(
        circle.contains("Also exported as: [`demo::Circle`](Circle.md)"),
        "{circle}"
    );
    assert!(
        circle.contains("Members: [`demo::shapes::Circle::area`](shapes.Circle.area.md)"),
        "{circle}"
    );
    let alias = &rendered.pages["rust/demo/Circle.md"];
    assert!(
        alias.contains("Re-export of [`demo::shapes::Circle`](shapes.Circle.md)."),
        "{alias}"
    );
    let area = &rendered.pages["rust/demo/shapes.Circle.area.md"];
    assert!(
        area.contains("Member of: [`demo::shapes::Circle`](shapes.Circle.md)"),
        "{area}"
    );
    let generated = &rendered.pages["rust/demo/VERSION.md"];
    assert!(
        generated.contains("Source: `pkg/build.out_dir/gen.rs:2` (generated)"),
        "{generated}"
    );

    assert!(rendered
        .api
        .starts_with("# API Reference\n\n## Rust crate `demo`\n\n"));
    assert!(rendered
        .api
        .contains("- [`demo::add`](api/rust/demo/add.md): Adds.\n"));
    assert!(rendered
        .api
        .contains("- [`demo::Circle`](api/rust/demo/Circle.md): A circle.\n"));
    assert!(rendered
        .summary
        .starts_with("- [API](api.md)\n    - [demo::Circle](api/rust/demo/Circle.md)\n"));
    assert_eq!(rendered.summary.lines().count(), 6);

    let records: serde_json::Value = serde_json::from_str(&rendered.records).expect("records");
    let urls: Vec<&str> = records
        .as_array()
        .expect("array")
        .iter()
        .map(|r| r["url"].as_str().expect("url"))
        .collect();
    assert_eq!(
        urls,
        vec![
            "api/rust/demo/Circle.html",
            "api/rust/demo/VERSION.html",
            "api/rust/demo/add.html",
            "api/rust/demo/shapes.Circle.area.html",
            "api/rust/demo/shapes.Circle.html",
        ]
    );
    assert_eq!(records[2]["title"], "demo::add");
    assert_eq!(records[2]["body"], "Adds.");
}

#[test]
fn a_symbol_without_docs_says_undocumented() {
    let mut bare = symbol("rust:demo:bare", "pub fn bare()", "", 3);
    bare.relations = Some(Relations {
        reexported_as: vec!["rust:demo:alias".to_string()],
        ..Relations::default()
    });
    let blank = symbol("rust:demo:blank", "pub fn blank()", " \n\t\n", 5);
    let mut ir = shard();
    ir.symbols = vec![bare, blank];
    let rendered = render(&[ir], "").expect("render");
    let bare_page = &rendered.pages["rust/demo/bare.md"];
    assert_eq!(
        bare_page,
        "# `demo::bare`\n\n```rust,noplayground\npub fn bare()\n```\n\nUndocumented.\n\nSource: `pkg/src/lib.rs:3`\n\nAlso exported as: [`demo::alias`](alias.md)\n"
    );
    assert!(
        rendered.pages["rust/demo/blank.md"].contains("\n\nUndocumented.\n\nSource:"),
        "{}",
        rendered.pages["rust/demo/blank.md"]
    );
    let alias = &rendered.pages["rust/demo/alias.md"];
    assert!(alias.contains("Re-export of"), "{alias}");
    assert!(alias.contains("\n\nUndocumented.\n\n"), "{alias}");
    assert!(!rendered.api.contains("Undocumented"), "{}", rendered.api);
    assert!(!rendered.records.contains("Undocumented"));
}

#[test]
fn without_a_source_url_the_source_is_plain_text() {
    let rendered = render(&[shard()], "").expect("render");
    assert!(rendered.pages["rust/demo/add.md"].contains("Source: `pkg/src/lib.rs:7`\n"));
}

#[test]
fn duplicate_pages_and_empty_shards_fail() {
    assert_eq!(
        render(&[shard(), shard()], ""),
        Err("two shards define the API page for 'rust:demo:VERSION'".to_string())
    );
    let mut empty = shard();
    empty.symbols.clear();
    assert_eq!(
        render(&[empty], ""),
        Err("the shards name no symbols".to_string())
    );
}

#[test]
fn go_symbols_render_with_a_go_fence_under_their_import_path() {
    let mut deposit = symbol(
        "go:example.com/ledger/money:Account.Deposit",
        "func (a *Account) Deposit(amount Amount)",
        "Deposit adds money.",
        47,
    );
    deposit.kind = SymbolKind::Method as i32;
    deposit.relations = Some(Relations {
        member_of: vec!["go:example.com/ledger/money:Account".to_string()],
        ..Relations::default()
    });
    let account = symbol(
        "go:example.com/ledger/money:Account",
        "type Account struct{}",
        "Account holds money.",
        35,
    );
    let shard = DocIr {
        schema_major: SCHEMA_MAJOR,
        schema_minor: SCHEMA_MINOR,
        language: "go".to_string(),
        package: "example.com/ledger/money".to_string(),
        symbols: vec![account, deposit],
    };
    let rendered = render(&[shard], "https://example.com/blob/main/").expect("render");
    let page = &rendered.pages["go/example.com/ledger/money/Account.Deposit.md"];
    assert!(
        page.starts_with("# `example.com/ledger/money.Account.Deposit`\n\n```go\nfunc (a *Account) Deposit(amount Amount)\n```\n"),
        "{page}"
    );
    assert!(
        page.contains("Member of: [`example.com/ledger/money.Account`](Account.md)"),
        "{page}"
    );
    assert!(
        rendered
            .api
            .contains("## Go package `example.com/ledger/money`"),
        "{}",
        rendered.api
    );
    assert!(
        rendered
            .records
            .contains("api/go/example.com/ledger/money/Account.html"),
        "{}",
        rendered.records
    );
}

fn ruby_shard() -> DocIr {
    let mut circle = symbol(
        "ruby:geometry:Geometry.Circle",
        "class Geometry::Circle < Geometry::Shape",
        "A circle.",
        16,
    );
    circle.display_name = "Geometry::Circle".to_string();
    circle.relations = Some(Relations {
        implements: vec!["ruby:geometry:Geometry.Sized".to_string()],
        reexported_as: vec!["ruby:geometry:Circle".to_string()],
        ..Relations::default()
    });
    let mut empty = symbol(
        "ruby:geometry:Geometry.Circle.empty~3F",
        "def empty?",
        "Reports whether the circle has no area.",
        51,
    );
    empty.display_name = "Geometry::Circle#empty?".to_string();
    empty.relations = Some(Relations {
        member_of: vec!["ruby:geometry:Geometry.Circle".to_string()],
        ..Relations::default()
    });
    let mut tick = symbol(
        "ruby:geometry:Geometry.Circle.~60",
        "def `(command)",
        "Runs.",
        70,
    );
    tick.display_name = "Geometry::Circle#`".to_string();
    let mut sized = symbol(
        "ruby:geometry:Geometry.Sized",
        "module Geometry::Sized",
        "Sized.",
        9,
    );
    sized.display_name = "Geometry::Sized".to_string();
    DocIr {
        schema_major: SCHEMA_MAJOR,
        schema_minor: SCHEMA_MINOR,
        language: "ruby".to_string(),
        package: "geometry".to_string(),
        symbols: vec![circle, empty, tick, sized],
    }
}

#[test]
fn display_names_title_ruby_pages_links_and_records() {
    let rendered = render(&[ruby_shard()], "https://example.com/blob/main/").expect("render");
    let page = &rendered.pages["ruby/geometry/Geometry.Circle.empty~3F.md"];
    assert!(
        page.starts_with("# `Geometry::Circle#empty?`\n\n```ruby\ndef empty?\n```\n"),
        "{page}"
    );
    assert!(
        page.contains("Member of: [`Geometry::Circle`](Geometry.Circle.md)"),
        "{page}"
    );
    let circle = &rendered.pages["ruby/geometry/Geometry.Circle.md"];
    assert!(
        circle.contains("Implements: [`Geometry::Sized`](Geometry.Sized.md)"),
        "{circle}"
    );
    assert!(
        circle.contains("[`Geometry::Circle#empty?`](Geometry.Circle.empty~3F.md)"),
        "{circle}"
    );
    assert!(
        rendered.api.contains("## Ruby library `geometry`"),
        "{}",
        rendered.api
    );
    assert!(
        rendered.api.contains(
            "- [`Geometry::Circle#empty?`](api/ruby/geometry/Geometry.Circle.empty~3F.md): Reports"
        ),
        "{}",
        rendered.api
    );
    assert!(
        rendered.summary.contains(
            "    - [Geometry::Circle#empty?](api/ruby/geometry/Geometry.Circle.empty~3F.md)\n"
        ),
        "{}",
        rendered.summary
    );
    let records: serde_json::Value = serde_json::from_str(&rendered.records).expect("records");
    let titles: Vec<&str> = records
        .as_array()
        .expect("records")
        .iter()
        .map(|r| r["title"].as_str().expect("title"))
        .collect();
    assert!(titles.contains(&"Geometry::Circle#empty?"), "{titles:?}");
}

#[test]
fn alias_pages_fall_back_to_the_id_title() {
    let rendered = render(&[ruby_shard()], "").expect("render");
    let alias = &rendered.pages["ruby/geometry/Circle.md"];
    assert!(alias.starts_with("# `geometry.Circle`\n"), "{alias}");
    assert!(
        alias.contains("Re-export of [`Geometry::Circle`](Geometry.Circle.md)."),
        "{alias}"
    );
}

#[test]
fn a_title_with_a_backtick_gets_a_longer_code_fence() {
    assert_eq!(code("plain"), "`plain`");
    assert_eq!(code("a`b"), "``a`b``");
    assert_eq!(code("Foo#`"), "`` Foo#` ``");
    assert_eq!(code("``x"), "``` ``x ```");
    let rendered = render(&[ruby_shard()], "").expect("render");
    let page = &rendered.pages["ruby/geometry/Geometry.Circle.~60.md"];
    assert!(page.starts_with("# `` Geometry::Circle#` ``\n"), "{page}");
    assert!(
        rendered
            .summary
            .contains("- [Geometry::Circle#\\`](api/ruby/geometry/Geometry.Circle.~60.md)"),
        "{}",
        rendered.summary
    );
}

fn java_shard(doc: &str) -> DocIr {
    DocIr {
        schema_major: SCHEMA_MAJOR,
        schema_minor: SCHEMA_MINOR,
        language: "java".to_string(),
        package: "demo".to_string(),
        symbols: vec![
            symbol("java:demo:a.Circle", "public class Circle", doc, 3),
            symbol("java:demo:a.Shape", "public interface Shape", "", 1),
        ],
    }
}

#[test]
fn dx_links_resolve_to_relative_pages_and_unknown_ids_become_code() {
    let doc = "See [Shape](dx:java:demo:a.Shape) and [Gone](dx:java:demo:a.Gone).\n\n\
               `[x](dx:java:demo:a.Shape)`\n\n```\n[y](dx:java:demo:a.Shape)\n```";
    let rendered = render(&[java_shard(doc)], "").expect("renders");
    let page = &rendered.pages["java/demo/a.Circle.md"];
    assert!(page.contains("[Shape](a.Shape.md)"), "{page}");
    assert!(page.contains("`demo.a.Gone`"), "{page}");
    assert!(page.contains("`[x](dx:java:demo:a.Shape)`"), "{page}");
    assert!(page.contains("\n[y](dx:java:demo:a.Shape)\n"), "{page}");
    assert!(page.contains("```java\npublic class Circle\n```"), "{page}");
}

#[test]
fn pages_that_differ_only_in_case_fail() {
    let mut shard = java_shard("");
    shard.symbols[1].id = "java:demo:a.circle".to_string();
    let error = render(&[shard], "").expect_err("collision");
    assert!(error.contains("differ only in case"), "{error}");
    assert!(
        error.contains("a.Circle") && error.contains("a.circle"),
        "{error}"
    );
}

fn vue_shard() -> DocIr {
    let member = |name: &str, kind: SymbolKind, signature: &str| {
        let mut s = symbol(&format!("vue:ui:Button.{name}"), signature, "", 5);
        s.kind = kind as i32;
        s.display_name = format!("Button.vue {name}");
        s.relations = Some(Relations {
            member_of: vec!["vue:ui:Button".to_string()],
            ..Relations::default()
        });
        s
    };
    let mut button = symbol("vue:ui:Button", "component Button", "A button.", 1);
    button.kind = SymbolKind::Component as i32;
    button.display_name = "Button.vue".to_string();
    DocIr {
        schema_major: SCHEMA_MAJOR,
        schema_minor: SCHEMA_MINOR,
        language: "vue".to_string(),
        package: "ui".to_string(),
        symbols: vec![
            button,
            member("event_click", SymbolKind::Event, "click: [number]"),
            member("exposed_focus", SymbolKind::Method, "focus: () => void"),
            member("prop_label", SymbolKind::Property, "label: string"),
            member("prop_size", SymbolKind::Property, "size?: number = 1"),
            member("slot_default", SymbolKind::Slot, "default: {}"),
        ],
    }
}

#[test]
fn vue_components_group_their_members_by_kind() {
    let rendered = render(&[vue_shard()], "").expect("render");
    assert!(
        rendered.api.contains("## Vue components `ui`"),
        "{}",
        rendered.api
    );
    let page = &rendered.pages["vue/ui/Button.md"];
    assert_eq!(
        page,
        "# `Button.vue`\n\n```ts\ncomponent Button\n```\n\nA button.\n\nSource: `pkg/src/lib.rs:1`\n\nProps: [`Button.vue prop_label`](Button.prop_label.md), [`Button.vue prop_size`](Button.prop_size.md)\n\nEvents: [`Button.vue event_click`](Button.event_click.md)\n\nSlots: [`Button.vue slot_default`](Button.slot_default.md)\n\nExposed: [`Button.vue exposed_focus`](Button.exposed_focus.md)\n"
    );
    let prop = &rendered.pages["vue/ui/Button.prop_label.md"];
    assert!(
        prop.contains("```ts\nlabel: string\n```")
            && prop.contains("Member of: [`Button.vue`](Button.md)"),
        "{prop}"
    );
    assert_eq!(rendered.summary.matches("    - [").count(), 6);
    let records: serde_json::Value = serde_json::from_str(&rendered.records).expect("records");
    assert_eq!(records.as_array().expect("records").len(), 6);
}

#[test]
fn python_symbols_render_with_a_python_fence_and_title() {
    let shard = DocIr {
        schema_major: SCHEMA_MAJOR,
        schema_minor: SCHEMA_MINOR,
        language: "python".to_string(),
        package: "mylib".to_string(),
        symbols: vec![symbol(
            "python:mylib:greet",
            "def greet(name: str) -> str",
            "Greets.",
            3,
        )],
    };
    let rendered = render(&[shard], "https://example.com/blob/main/").expect("render");
    let page = &rendered.pages["python/mylib/greet.md"];
    assert!(
        page.contains("```python\ndef greet(name: str) -> str\n```\n"),
        "{page}"
    );
    let all: String = rendered.pages.values().cloned().collect();
    assert!(all.contains("Python package `mylib`"), "{all}");
}
