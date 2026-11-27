pub const COVERAGE_EXCLUDED_TAGS: [&str; 4] = [
    "no-coverage",
    "dx-no-coverage-collector",
    "manual",
    "resources:nested_bazel:1",
];

const TAG_FILTERS: &str = "--test_tag_filters";

#[must_use]
pub fn takes_coverage(tags: &[String]) -> bool {
    !tags
        .iter()
        .any(|tag| COVERAGE_EXCLUDED_TAGS.contains(&tag.as_str()))
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TagFilter {
    included: Vec<String>,
    excluded: Vec<String>,
}

impl TagFilter {
    #[must_use]
    pub fn from_options(options: &[String]) -> TagFilter {
        let mut value: Option<&str> = None;
        let mut tokens = options.iter();
        while let Some(token) = tokens.next() {
            if let Some(inline) = token
                .strip_prefix(TAG_FILTERS)
                .and_then(|rest| rest.strip_prefix('='))
            {
                value = Some(inline);
            } else if token == TAG_FILTERS {
                value = tokens.next().map(String::as_str);
            }
        }
        TagFilter::parse(value.unwrap_or_default())
    }

    fn parse(value: &str) -> TagFilter {
        let mut filter = TagFilter::default();
        for tag in value
            .split(',')
            .map(str::trim)
            .filter(|tag| !tag.is_empty())
        {
            match tag.strip_prefix('-') {
                Some(excluded) => filter.excluded.push(excluded.to_owned()),
                None => filter.included.push(tag.to_owned()),
            }
        }
        filter
    }

    #[must_use]
    pub fn admits(&self, tags: &[String]) -> bool {
        let has = |wanted: &String| tags.iter().any(|tag| tag == wanted);
        (self.included.is_empty() || self.included.iter().any(has))
            && !self.excluded.iter().any(has)
    }

    fn render(&self) -> String {
        self.included
            .iter()
            .cloned()
            .chain(self.excluded.iter().map(|tag| format!("-{tag}")))
            .collect::<Vec<_>>()
            .join(",")
    }
}

#[must_use]
pub fn exclude_tags(options: &[String], excluded: &[&str]) -> Vec<String> {
    let mut filter = TagFilter::from_options(options);
    for tag in excluded {
        if !filter.excluded.iter().any(|have| have == tag) {
            filter.excluded.push((*tag).to_owned());
        }
    }
    let mut kept = Vec::with_capacity(options.len() + 1);
    let mut tokens = options.iter();
    while let Some(token) = tokens.next() {
        if token == TAG_FILTERS {
            tokens.next();
            continue;
        }
        if token
            .strip_prefix(TAG_FILTERS)
            .is_some_and(|rest| rest.starts_with('='))
        {
            continue;
        }
        kept.push(token.clone());
    }
    kept.push(format!("{TAG_FILTERS}={}", filter.render()));
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| (*item).to_owned()).collect()
    }

    #[test]
    fn coverage_skips_tests_with_an_excluded_tag() {
        assert!(takes_coverage(&strings(&["small", "network"])));
        for tag in COVERAGE_EXCLUDED_TAGS {
            assert!(!takes_coverage(&strings(&["a", tag])), "{tag}");
        }
        assert!(takes_coverage(&strings(&["no-coverage-please"])));
    }

    #[test]
    fn the_last_tag_filter_wins() {
        let filter = TagFilter::from_options(&strings(&[
            "--keep_going",
            "--test_tag_filters=-a",
            "--test_tag_filters",
            "b,-c",
        ]));
        assert!(filter.admits(&strings(&["b"])));
        assert!(!filter.admits(&strings(&["b", "c"])));
        assert!(!filter.admits(&strings(&["a"])));
        assert!(TagFilter::from_options(&[]).admits(&[]));
    }

    #[test]
    fn excluded_tags_merge_into_one_filter_after_the_other_options() {
        let merged = exclude_tags(
            &strings(&[
                "--test_tag_filters=-network-consumer,-audit",
                "--keep_going",
                "--test_tag_filters",
                "smoke,-audit",
            ]),
            &["manual", "audit"],
        );
        assert_eq!(
            merged,
            strings(&["--keep_going", "--test_tag_filters=smoke,-audit,-manual"])
        );
        assert_eq!(
            exclude_tags(&strings(&["--jobs=4"]), &["manual"]),
            strings(&["--jobs=4", "--test_tag_filters=-manual"])
        );
    }
}
