//! Checks the Vue, Svelte, Astro and MDX fixture shards and the converter's failure handling.

use std::path::PathBuf;
use std::process::Command;

use documentation_ir::proto::{DocIr, Symbol, SymbolKind};
use documentation_ir::{decode_shard, validate_shard};

fn runfile(var: &str) -> PathBuf {
    let rel = std::env::var(var).unwrap_or_else(|_| panic!("{var} must name a runfile"));
    dx_testing::resolve_runfiles(&rel)
}

fn shard(var: &str) -> DocIr {
    let bytes = std::fs::read(runfile(var)).expect("read a fixture shard");
    decode_shard(&bytes).expect("the fixture shard decodes")
}

fn find<'a>(shard: &'a DocIr, id: &str) -> &'a Symbol {
    shard
        .symbols
        .iter()
        .find(|s| s.id == id)
        .unwrap_or_else(|| panic!("missing {id}"))
}

fn line_of(source: &str, needle: &str) -> u64 {
    let at = source
        .find(needle)
        .unwrap_or_else(|| panic!("{needle} not in source"));
    1 + source[..at].matches('\n').count() as u64
}

#[test]
fn the_fixture_shard_has_the_exact_symbols() {
    let shard = shard("DX_FIXTURE_SHARD");
    validate_shard(&shard).expect("validate_shard passes");
    assert_eq!(
        (shard.language.as_str(), shard.package.as_str()),
        ("vue", "ui")
    );
    let ids: Vec<&str> = shard.symbols.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "vue:ui:Badge",
            "vue:ui:Badge.prop_text",
            "vue:ui:Button",
            "vue:ui:Button.event_click",
            "vue:ui:Button.event_update~3AmodelValue",
            "vue:ui:Button.exposed_clicks",
            "vue:ui:Button.exposed_focus",
            "vue:ui:Button.prop_disabled",
            "vue:ui:Button.prop_label",
            "vue:ui:Button.prop_modelValue",
            "vue:ui:Button.prop_size",
            "vue:ui:Button.slot_default",
        ]
    );
}

#[test]
fn kinds_signatures_and_docs_match_the_sources() {
    let shard = shard("DX_FIXTURE_SHARD");
    let sig = |id: &str| find(&shard, id).signature_text.clone();
    assert_eq!(sig("vue:ui:Button"), "component Button");
    assert_eq!(sig("vue:ui:Button.prop_label"), "label: string");
    assert_eq!(
        sig("vue:ui:Button.prop_size"),
        "size?: number | undefined = 12"
    );
    assert_eq!(
        sig("vue:ui:Button.prop_disabled"),
        "disabled?: boolean | undefined = false"
    );
    assert_eq!(
        sig("vue:ui:Button.event_click"),
        "(event: \"click\", id: number): void"
    );
    assert_eq!(sig("vue:ui:Button.exposed_focus"), "focus: () => void");
    assert_eq!(sig("vue:ui:Button.exposed_clicks"), "clicks: number");
    assert_eq!(
        find(&shard, "vue:ui:Button.exposed_focus").kind,
        SymbolKind::Method as i32
    );
    assert_eq!(
        find(&shard, "vue:ui:Button.exposed_clicks").kind,
        SymbolKind::Field as i32
    );
    assert_eq!(
        find(&shard, "vue:ui:Button.slot_default").kind,
        SymbolKind::Slot as i32
    );
    assert_eq!(
        find(&shard, "vue:ui:Badge").doc_markdown,
        "A small status badge."
    );
    assert_eq!(
        find(&shard, "vue:ui:Button.prop_label").doc_markdown,
        "Text shown inside the button."
    );
}

#[test]
fn source_lines_follow_the_file_bytes_including_non_ascii_comments() {
    let shard = shard("DX_FIXTURE_SHARD");
    let source =
        std::fs::read_to_string(runfile("DX_FIXTURE_BUTTON_SOURCE")).expect("read Button.vue");
    assert!(
        source.contains("Größe"),
        "the fixture keeps its non-ASCII comment"
    );
    for (id, needle) in [
        ("vue:ui:Button.prop_label", "  label: string;"),
        ("vue:ui:Button.prop_size", "  size?: number;"),
        ("vue:ui:Button.prop_disabled", "  disabled?: boolean;"),
    ] {
        let reference = find(&shard, id).source.clone().expect("source");
        assert_eq!(
            reference.file, "tools/docs/components/fixture/vue/src/Button.vue",
            "{id}"
        );
        assert_eq!(reference.line, line_of(&source, needle), "{id}");
    }
}

#[test]
fn the_configuration_variation_is_observable() {
    let strict = shard("DX_FIXTURE_SHARD");
    let loose = shard("DX_FIXTURE_LOOSE_SHARD");
    let ids = |s: &DocIr| s.symbols.iter().map(|x| x.id.clone()).collect::<Vec<_>>();
    assert_eq!(ids(&strict), ids(&loose));
    assert_eq!(
        find(&loose, "vue:ui:Button.prop_size").signature_text,
        "size?: number = 12"
    );
    let config = |s: &DocIr| {
        String::from_utf8(find(s, "vue:ui:Button").extensions[0].value.clone()).expect("utf8")
    };
    assert!(
        config(&strict).starts_with("tsconfig=tsconfig.json;"),
        "{}",
        config(&strict)
    );
    assert!(
        config(&loose).starts_with("tsconfig=tsconfig.loose.json;"),
        "{}",
        config(&loose)
    );
    assert!(
        config(&strict)
            .contains(";meta-json=dx-component-meta/1;vue-component-meta=3.3.14;typescript=5.9.3")
    );
}

#[test]
fn the_converter_fails_without_writing_on_bad_metadata() {
    let scratch = dx_test_scratch::scratch("component_extract");
    let dir = scratch.path();
    let meta = std::fs::read_to_string(runfile("DX_FIXTURE_META")).expect("meta");
    let cases = [
        ("corrupt", "{ not json".to_owned()),
        (
            "schema",
            meta.replace("dx-component-meta/1", "dx-component-meta/9"),
        ),
        (
            "framework",
            meta.replace("\"framework\": \"vue\"", "\"framework\": \"svelte\""),
        ),
        (
            "empty",
            "{\"schema\":\"dx-component-meta/1\",\"framework\":\"vue\",\"components\":[]}"
                .to_owned(),
        ),
    ];
    for (name, body) in cases {
        let input = dir.join(format!("{name}.json"));
        let out = dir.join(format!("{name}.pb"));
        std::fs::write(&input, body).expect("write");
        let status = Command::new(runfile("DX_COMPONENT_EXTRACT"))
            .args(["--meta"])
            .arg(&input)
            .arg("--out")
            .arg(&out)
            .args(["--language", "vue", "--package", "ui"])
            .output()
            .expect("run");
        assert_eq!(
            status.status.code(),
            Some(1),
            "{name}: {}",
            String::from_utf8_lossy(&status.stderr)
        );
        assert!(!out.exists(), "{name} must not write a stub");
    }
}

#[test]
fn the_svelte_shard_has_exact_symbols_signatures_docs_and_lines() {
    let shard = shard("DX_SVELTE_SHARD");
    validate_shard(&shard).expect("validate_shard passes");
    assert_eq!(
        (shard.language.as_str(), shard.package.as_str()),
        ("svelte", "ui")
    );
    let ids: Vec<&str> = shard.symbols.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "svelte:ui:Card",
            "svelte:ui:Card.prop_onselect",
            "svelte:ui:Card.prop_rows",
            "svelte:ui:Card.prop_title",
            "svelte:ui:Card.slot_children",
            "svelte:ui:Card.slot_footer",
            "svelte:ui:Tag",
            "svelte:ui:Tag.prop_label",
        ]
    );
    let sig = |id: &str| find(&shard, id).signature_text.clone();
    assert_eq!(sig("svelte:ui:Card.prop_title"), "title: string");
    assert_eq!(sig("svelte:ui:Card.prop_rows"), "rows?: number = 3");
    assert_eq!(
        find(&shard, "svelte:ui:Card.prop_title").doc_markdown,
        "Heading of the card."
    );
    assert_eq!(
        find(&shard, "svelte:ui:Card.slot_footer").kind,
        SymbolKind::Slot as i32
    );
    let source =
        std::fs::read_to_string(runfile("DX_SVELTE_CARD_SOURCE")).expect("read Card.svelte");
    for (id, needle) in [
        ("svelte:ui:Card.prop_title", "    title: string;"),
        ("svelte:ui:Card.prop_rows", "    rows?: number;"),
        ("svelte:ui:Card.prop_onselect", "    onselect?:"),
        ("svelte:ui:Card.slot_children", "    children: Snippet;"),
        ("svelte:ui:Card.slot_footer", "    footer?: Snippet"),
    ] {
        let reference = find(&shard, id).source.clone().expect("source");
        assert_eq!(
            reference.file, "tools/docs/components/fixture/svelte/src/Card.svelte",
            "{id}"
        );
        assert_eq!(reference.line, line_of(&source, needle), "{id}");
    }
}

#[test]
fn the_astro_shard_has_exact_symbols_signatures_docs_and_lines() {
    let shard = shard("DX_ASTRO_SHARD");
    validate_shard(&shard).expect("validate_shard passes");
    assert_eq!(
        (shard.language.as_str(), shard.package.as_str()),
        ("astro", "ui")
    );
    let ids: Vec<&str> = shard.symbols.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "astro:ui:Hero",
            "astro:ui:Hero.prop_links",
            "astro:ui:Hero.prop_title",
            "astro:ui:Hero.slot_default",
            "astro:ui:Hero.slot_footer",
            "astro:ui:Note",
            "astro:ui:Note.prop_text",
        ]
    );
    let sig = |id: &str| find(&shard, id).signature_text.clone();
    assert_eq!(sig("astro:ui:Hero.prop_title"), "title: string");
    assert_eq!(sig("astro:ui:Hero.prop_links"), "links?: number = 3");
    assert_eq!(
        find(&shard, "astro:ui:Hero.prop_title").doc_markdown,
        "Heading of the hero."
    );
    assert_eq!(
        find(&shard, "astro:ui:Hero.slot_footer").kind,
        SymbolKind::Slot as i32
    );
    let source = std::fs::read_to_string(runfile("DX_ASTRO_HERO_SOURCE")).expect("read Hero.astro");
    for (id, needle) in [
        ("astro:ui:Hero.prop_title", "  title: string;"),
        ("astro:ui:Hero.prop_links", "  links?: number;"),
        ("astro:ui:Hero.slot_default", "<slot />"),
        ("astro:ui:Hero.slot_footer", "<slot name=\"footer\" />"),
    ] {
        let reference = find(&shard, id).source.clone().expect("source");
        assert_eq!(
            reference.file, "tools/docs/components/fixture/astro/src/Hero.astro",
            "{id}"
        );
        assert_eq!(reference.line, line_of(&source, needle), "{id}");
    }
}

#[test]
fn the_mdx_shard_has_exact_symbols_signatures_docs_and_lines() {
    let shard = shard("DX_MDX_SHARD");
    validate_shard(&shard).expect("validate_shard passes");
    assert_eq!(
        (shard.language.as_str(), shard.package.as_str()),
        ("mdx", "ui")
    );
    let ids: Vec<&str> = shard.symbols.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "mdx:ui:Intro",
            "mdx:ui:Intro.exposed_greet",
            "mdx:ui:Intro.exposed_limit",
            "mdx:ui:Intro.prop_size",
            "mdx:ui:Intro.prop_title",
            "mdx:ui:Outro",
            "mdx:ui:Outro.exposed_farewell",
        ]
    );
    let sig = |id: &str| find(&shard, id).signature_text.clone();
    assert_eq!(sig("mdx:ui:Intro.exposed_limit"), "limit: number");
    assert_eq!(
        sig("mdx:ui:Intro.exposed_greet"),
        "greet: (name: string) => string"
    );
    assert_eq!(
        find(&shard, "mdx:ui:Intro.exposed_greet").doc_markdown,
        "Greets someone.\n\n**@param** name Who to greet.\n\n**@returns**"
    );
    let source = std::fs::read_to_string(runfile("DX_MDX_INTRO_SOURCE")).expect("read Intro.mdx");
    for (id, needle) in [
        ("mdx:ui:Intro.exposed_limit", "export const limit"),
        ("mdx:ui:Intro.exposed_greet", "export function greet"),
        ("mdx:ui:Intro.prop_title", "props.title"),
        ("mdx:ui:Intro.prop_size", "props.size"),
    ] {
        let reference = find(&shard, id).source.clone().expect("source");
        assert_eq!(
            reference.file, "tools/docs/components/fixture/mdx/src/Intro.mdx",
            "{id}"
        );
        assert_eq!(reference.line, line_of(&source, needle), "{id}");
    }
    for key in ["author", "Welcome", "prose"] {
        assert!(
            shard.symbols.iter().all(|s| !s.id.contains(key)),
            "{key} became a symbol"
        );
    }
}
