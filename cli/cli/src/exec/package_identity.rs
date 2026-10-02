use dx_bump::{BumpError, BumpRequest, BumpSet};
use dx_update::selector::{parse_selector, SelectorError};
use dx_update::sets::SetId;

const VERSION: &str = "1.2.3";

const IDENTITIES: &[&str] = &[
    "anyhow",
    "serde_json",
    "rules_dx",
    "a-b_c",
    "",
    "bad name",
    "bad!name",
    "with/slash",
    "with:colon",
    "astro",
    "@astrojs/compiler",
    "@scope/name.with-dots_and_underscores",
    "@scope",
    "@/name",
    "@scope/",
    "@scope/bad!",
    "@scope/a/b",
    "github.com/google/go-cmp",
    "gopkg.in/yaml.v3",
    "example.com/a~b+c",
    "/leading",
    "trailing/",
    "double//slash",
    "junit:junit",
    "org.junit.jupiter:junit-jupiter-api",
    "com.example:my_artifact-1.0",
    ":artifact",
    "group:",
    "group:art:ifact",
    "gr oup:artifact",
    "group:arti fact",
    "gr@oup:artifact",
    "group:artifact!",
    "gr/oup:artifact",
    "FSharp.Core",
    "Newtonsoft.Json",
];

fn update_refusal(set: &str, package: &str) -> Option<&'static str> {
    match parse_selector(&format!("{set}:{package}")) {
        Err(SelectorError::InvalidPackage { reason, .. }) => Some(reason),
        _ => None,
    }
}

fn bump_refusal(set: &str, package: &str) -> Option<&'static str> {
    match BumpRequest::parse(&format!("{set}:{package}"), VERSION) {
        Err(BumpError::InvalidPackage { reason, .. }) => Some(reason),
        _ => None,
    }
}

/// `BumpRequest::parse` refuses an empty or colon-bearing identity before the shared check.
fn bump_precheck(set: &str, package: &str) -> bool {
    package.is_empty() || (set != "maven" && package.contains(':'))
}

#[test]
fn update_and_bump_refuse_a_shared_identity_the_same_way() {
    let mut shared = 0;
    for set in BumpSet::ALL {
        let name = set.name();
        if SetId::parse(name).is_none() {
            continue;
        }
        shared += 1;
        for package in IDENTITIES {
            let update = update_refusal(name, package);
            let bump = bump_refusal(name, package);
            assert_eq!(
                update.is_some(),
                bump.is_some(),
                "{name}:{package}: dx update and dx bump disagree"
            );
            if !bump_precheck(name, package) {
                assert_eq!(
                    update, bump,
                    "{name}:{package}: dx update and dx bump name the refusal differently"
                );
            }
        }
    }
    assert_eq!(shared, 5, "cargo, npm, go, maven, and nuget are shared");
}

#[test]
fn the_shared_set_corpus_refuses_some_and_accepts_some() {
    for set in ["cargo", "npm", "go", "maven", "nuget"] {
        let accepted = IDENTITIES
            .iter()
            .filter(|package| update_refusal(set, package).is_none())
            .count();
        let refused = IDENTITIES.len() - accepted;
        assert!(accepted > 0, "{set}: corpus refuses every identity");
        assert!(refused > 0, "{set}: corpus accepts every identity");
    }
}

#[test]
fn every_shared_set_names_its_own_characters() {
    let said: Vec<(&str, &str)> = ["cargo", "npm", "go", "maven", "nuget"]
        .iter()
        .map(|set| (*set, update_refusal(set, "bad!name").unwrap_or("accepted")))
        .collect();
    assert_eq!(
        said,
        vec![
            ("cargo", "cargo crate names use [A-Za-z0-9_-] only"),
            ("npm", "npm names use [A-Za-z0-9_.-] only"),
            ("go", "go module paths use [A-Za-z0-9/_.-~+] only"),
            ("maven", "maven identities are group:artifact"),
            ("nuget", "nuget ids use [A-Za-z0-9_.-] only"),
        ]
    );
}
