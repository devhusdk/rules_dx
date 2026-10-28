//! Feeds the Vue fixture shard to the site aggregator.

fn fixture() -> documentation_ir::proto::DocIr {
    let rel = std::env::var("DX_FIXTURE_SHARD").expect("DX_FIXTURE_SHARD");
    let bytes = std::fs::read(dx_testing::resolve_runfiles(&rel)).expect("read the shard");
    documentation_ir::decode_shard(&bytes).expect("decode")
}

#[test]
fn components_render_with_member_groups_and_search_records() {
    let rendered = dx_site_aggregate::render(&[fixture()], "").expect("render");
    assert!(
        rendered.api.contains("## Vue components `ui`"),
        "{}",
        rendered.api
    );
    let page = &rendered.pages["vue/ui/Button.md"];
    assert!(
        page.starts_with("# `Button.vue`\n\n```ts\ncomponent Button\n```\n"),
        "{page}"
    );
    for group in ["Props: ", "Events: ", "Slots: ", "Exposed: "] {
        assert!(
            page.contains(&format!("\n{group}")),
            "{group} missing in {page}"
        );
    }
    let prop = &rendered.pages["vue/ui/Button.prop_label.md"];
    assert!(prop.contains("```ts\nlabel: string\n```"), "{prop}");
    assert!(
        prop.contains("Member of: [`Button.vue`](Button.md)"),
        "{prop}"
    );
    assert_eq!(
        rendered.summary.matches("    - [").count(),
        10,
        "{}",
        rendered.summary
    );
    assert!(
        rendered.records.contains("Button.vue prop_label"),
        "{}",
        rendered.records
    );
}
