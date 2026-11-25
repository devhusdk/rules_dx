//! The `<lang>/tests/consumer` fixtures of rules_dx. The root module ignores those directories, so
//! no package owns their files; the `@dx_consumer_fixtures` repository rule snapshots each one into
//! `<lang>.json` and all of them into `consumer.json`, and the tests read those files.

use std::path::Path;

use super::rule_graph::apparent_repo_matches;

const REPO: &str = "dx_consumer_fixtures";
const EVERY_LANGUAGE: &str = "consumer.json";

/// The language whose consumer fixture holds `path`: a file under `<lang>/tests/consumer/` next to
/// a `<lang>/lang.json`. Callers check that rules_dx is the root module.
pub(crate) fn language<'a>(workspace: &Path, path: &'a str) -> Option<&'a str> {
    let (lang, rest) = path.split_once('/')?;
    let file = rest.strip_prefix("tests/consumer/")?;
    if file.is_empty() || !workspace.join(lang).join("lang.json").is_file() {
        return None;
    }
    Some(lang)
}

/// The snapshot files of `@dx_consumer_fixtures` that hold the fixture of `lang`, among the
/// external labels of the rule graph, by apparent or canonical repository name.
pub(crate) fn seeds<'a>(external_labels: impl Iterator<Item = &'a str>, lang: &str) -> Vec<String> {
    let own = format!("{lang}.json");
    let mut seeds: Vec<String> = external_labels
        .filter(|input| {
            input
                .strip_prefix('@')
                .map(|repo| repo.strip_prefix('@').unwrap_or(repo))
                .and_then(|label| label.split_once("//:"))
                .is_some_and(|(repo, file)| {
                    apparent_repo_matches(repo, REPO) && (file == own || file == EVERY_LANGUAGE)
                })
        })
        .map(str::to_owned)
        .collect();
    seeds.sort();
    seeds
}

#[cfg(test)]
mod tests {
    use super::*;
    use dx_test_scratch::write;

    #[test]
    fn a_file_under_a_language_consumer_directory_names_its_language() {
        let scratch = dx_test_scratch::scratch("dx-consumer-fixture-language-").unwrap();
        let workspace = scratch.path();
        write(workspace, "lua/lang.json", "{}\n").unwrap();
        write(workspace, "lua/tests/consumer/hello.lua", "x\n").unwrap();
        assert_eq!(
            language(workspace, "lua/tests/consumer/hello.lua"),
            Some("lua")
        );
        assert_eq!(
            language(workspace, "lua/tests/consumer/sub/a.lua"),
            Some("lua")
        );
        assert_eq!(language(workspace, "lua/tests/other/hello.lua"), None);
        assert_eq!(language(workspace, "lua/tests/consumer/"), None);
        assert_eq!(language(workspace, "go/tests/consumer/main.go"), None);
        assert_eq!(language(workspace, "x/lua/tests/consumer/hello.lua"), None);
    }

    #[test]
    fn seeds_are_the_language_snapshot_and_the_snapshot_of_every_language() {
        let inputs = [
            "@dx_consumer_fixtures//:go.json",
            "@dx_consumer_fixtures//:lua.json",
            "@@+consumer_fixtures+dx_consumer_fixtures//:consumer.json",
            "@@+consumer_fixtures+other_fixtures//:lua.json",
            "@other//:lua.json",
            "@dx_consumer_fixtures//sub:lua.json",
            "//lua/tests/consumer:lua.json",
        ];
        assert_eq!(
            seeds(inputs.into_iter(), "lua"),
            [
                "@@+consumer_fixtures+dx_consumer_fixtures//:consumer.json",
                "@dx_consumer_fixtures//:lua.json",
            ]
        );
    }
}
