use dx_release_tools::render_bcr_source_for_digest;

fn template(name: &str) -> String {
    dx_testing::read_runfiles(&format!(".bcr/{name}"))
}

fn fill(text: &str, version: &str) -> String {
    text.replace("{OWNER}", "devhusdk")
        .replace("{REPO}", "rules_dx")
        .replace("{VERSION}", version)
        .replace("{TAG}", &format!("v{version}"))
}

fn json(text: &str) -> serde_json::Value {
    serde_json::from_str(text).expect("template is JSON")
}

#[test]
fn metadata_template_names_homepage_maintainer_and_repository() {
    let meta = json(&fill(&template("metadata.template.json"), "1.2.3"));
    assert_eq!(meta["homepage"], "https://github.com/devhusdk/rules_dx");
    assert_eq!(meta["repository"][0], "github:devhusdk/rules_dx");
    assert_eq!(meta["maintainers"][0]["github"], "devhusdk");
}

#[test]
fn source_template_matches_the_bcr_source_output() {
    let digest = "0".repeat(64);
    let want = json(&render_bcr_source_for_digest("rules_dx", "1.2.3", &digest).unwrap());
    let got = json(&fill(&template("source.template.json"), "1.2.3"));
    assert_eq!(got["url"], want["url"]);
    assert_eq!(got["strip_prefix"], want["strip_prefix"]);
}

#[test]
fn templates_carry_the_placeholders_bcr_substitutes() {
    let text = template("source.template.json");
    for key in ["{OWNER}", "{REPO}", "{VERSION}", "{TAG}"] {
        assert!(text.contains(key), "source.template.json lacks {key}");
    }
    json(
        &template("metadata.template.json")
            .replace("{OWNER}", "o")
            .replace("{REPO}", "r"),
    );
}

#[test]
fn presubmit_covers_five_platforms_and_the_dx_commands() {
    let text = template("presubmit.yml");
    for want in [
        "debian11",
        "ubuntu2204",
        "macos_arm64",
        "windows",
        "ubuntu2204_arm64",
        "@rules_dx//:dx",
        "--help",
        "//:hello_test",
    ] {
        assert!(text.contains(want), "presubmit.yml lacks {want}");
    }
}

#[test]
fn every_family_template_directory_is_complete() {
    let dir = dx_testing::resolve_runfiles(".bcr/presubmit.yml").with_file_name("families");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    for entry in entries {
        let family = entry.expect("entry").path();
        for file in [
            "metadata.template.json",
            "source.template.json",
            "presubmit.yml",
        ] {
            assert!(
                family.join(file).is_file(),
                "{} lacks {file}",
                family.display()
            );
        }
    }
}

#[test]
fn config_lists_the_root_and_every_family_with_templates() {
    let config = template("config.yml");
    let mut roots: Vec<String> = config
        .lines()
        .filter_map(|line| line.trim().strip_prefix("- "))
        .map(|root| root.trim_matches('"').to_owned())
        .collect();
    roots.sort();
    let dir = dx_testing::resolve_runfiles(".bcr/presubmit.yml").with_file_name("families");
    let mut want = vec![".".to_owned()];
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries {
            let name = entry.expect("entry").file_name();
            want.push(format!("families/{}", name.to_string_lossy()));
        }
    }
    want.sort();
    assert!(config.starts_with("moduleRoots:"));
    assert_eq!(roots, want);
}
