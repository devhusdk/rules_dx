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

fn update_rejects(set: &str, package: &str) -> bool {
    matches!(
        parse_selector(&format!("{set}:{package}")),
        Err(SelectorError::InvalidPackage { .. })
    )
}

fn bump_rejects(set: &str, package: &str) -> bool {
    matches!(
        BumpRequest::parse(&format!("{set}:{package}"), VERSION),
        Err(BumpError::InvalidPackage { .. })
    )
}

#[test]
fn update_and_bump_reject_the_same_package_identities() {
    let mut shared = 0;
    for set in BumpSet::ALL {
        let name = set.name();
        if SetId::parse(name).is_none() {
            continue;
        }
        shared += 1;
        for package in IDENTITIES {
            assert_eq!(
                update_rejects(name, package),
                bump_rejects(name, package),
                "{name}:{package}: dx update and dx bump disagree"
            );
        }
    }
    assert_eq!(shared, 5, "cargo, npm, go, maven, and nuget are shared");
}

#[test]
fn the_shared_set_corpus_rejects_and_accepts_both_ways() {
    for set in ["cargo", "npm", "go", "maven", "nuget"] {
        let accepted = IDENTITIES
            .iter()
            .filter(|package| !update_rejects(set, package))
            .count();
        let refused = IDENTITIES.len() - accepted;
        assert!(accepted > 0, "{set}: corpus rejects every identity");
        assert!(refused > 0, "{set}: corpus accepts every identity");
    }
}
