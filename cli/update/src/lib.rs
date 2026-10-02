#![cfg_attr(not(test), deny(clippy::expect_used, clippy::unwrap_used))]

pub mod backend;
pub mod manifest;
pub mod outcome;
pub mod recovery;
pub mod report;
pub mod selector;
pub mod sets;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateRequest {
    pub selectors: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateSelection {
    AllSets,
    Selected(Vec<String>),
}

impl UpdateRequest {
    pub fn plan(args: &[String]) -> UpdateRequest {
        UpdateRequest {
            selectors: args.to_vec(),
        }
    }

    pub fn selection(&self) -> UpdateSelection {
        if self.selectors.is_empty() {
            UpdateSelection::AllSets
        } else {
            UpdateSelection::Selected(self.selectors.clone())
        }
    }

    pub fn requires_confirmation() -> bool {
        false
    }

    pub fn is_mutating() -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(words: &[&str]) -> Vec<String> {
        words.iter().copied().map(str::to_owned).collect()
    }

    #[test]
    fn bare_update_selects_all_sets() {
        let request = UpdateRequest::plan(&[]);
        assert!(request.selectors.is_empty());
        assert_eq!(request.selection(), UpdateSelection::AllSets);
    }

    #[test]
    fn selectors_pass_through_verbatim() {
        let request = UpdateRequest::plan(&args(&["cargo-lock", "npm-root:react"]));
        assert_eq!(
            request.selection(),
            UpdateSelection::Selected(vec!["cargo-lock".to_owned(), "npm-root:react".to_owned(),])
        );
    }

    #[test]
    fn single_selector_narrows_without_rewriting() {
        let request = UpdateRequest::plan(&args(&["cargo-lock"]));
        assert_eq!(
            request.selection(),
            UpdateSelection::Selected(vec!["cargo-lock".to_owned()])
        );
    }

    #[test]
    fn update_applies_without_confirmation_and_mutates() {
        assert!(!UpdateRequest::requires_confirmation());
        assert!(UpdateRequest::is_mutating());
    }
}
