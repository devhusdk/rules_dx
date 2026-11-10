//! The modules one release publishes: core `rules_dx` and the modules under `families/`.

use crate::BCR_WANT_MODULE;
use crate::candidate::{call_end, string_fields};

/// One module of a release. The root is empty for core and `families/<f>` for a family.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseModule {
    pub name: String,
    pub root: String,
}

impl ReleaseModule {
    pub fn core() -> Self {
        Self {
            name: BCR_WANT_MODULE.to_owned(),
            root: String::new(),
        }
    }

    pub fn is_core(&self) -> bool {
        self.root.is_empty()
    }

    /// Returns the release asset name of this module's `source.json`.
    pub fn source_file(&self) -> String {
        if self.is_core() {
            "source.json".to_owned()
        } else {
            format!("{}.source.json", self.name)
        }
    }
}

type Call = Vec<(String, String)>;

fn calls(text: &str, function: &str) -> Vec<Call> {
    let mut found = Vec::new();
    for (at, _) in text.match_indices(&format!("{function}(")) {
        if at != 0 && text.as_bytes()[at - 1] != b'\n' {
            continue;
        }
        let open = at + function.len();
        if let Some(close) = call_end(text, open) {
            let fields = string_fields(&text[open + 1..close], open + 1);
            found.push(fields.into_iter().map(|f| (f.0, f.1)).collect());
        }
    }
    found
}

fn field<'a>(call: &'a Call, key: &str) -> Option<&'a str> {
    call.iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.as_str())
}

fn family_of(path: &str) -> Option<&str> {
    let rest = path.strip_prefix("families/")?;
    let (family, file) = rest.split_once('/')?;
    (file == "MODULE.bazel" && !family.is_empty()).then_some(family)
}

/// Returns core plus one module per `families/<f>/MODULE.bazel` in `files`.
///
/// `files` holds (path, text) pairs of the commit tree's MODULE.bazel files.
pub fn release_modules(files: &[(String, String)]) -> Result<Vec<ReleaseModule>, String> {
    let mut families: Vec<(&str, &str)> = files
        .iter()
        .filter_map(|(path, text)| family_of(path).map(|family| (family, text.as_str())))
        .collect();
    families.sort();
    let mut modules = vec![ReleaseModule::core()];
    for (family, text) in families {
        let want = format!("{BCR_WANT_MODULE}_{family}");
        let names: Vec<String> = calls(text, "module")
            .iter()
            .filter_map(|call| field(call, "name").map(str::to_owned))
            .collect();
        if names != [want.clone()] {
            return Err(format!(
                "release: families/{family}/MODULE.bazel module() name is {names:?}, want [\"{want}\"]"
            ));
        }
        modules.push(ReleaseModule {
            name: want,
            root: format!("families/{family}"),
        });
    }
    Ok(modules)
}

/// Fails when a module or a family's `rules_dx` dependency is not at `version`.
pub fn module_versions_error(files: &[(String, String)], version: &str) -> Result<(), String> {
    let hint = "land the version bump first";
    for (path, text) in files {
        let family = family_of(path).is_some();
        if !family && path != "MODULE.bazel" {
            continue;
        }
        let have = calls(text, "module")
            .iter()
            .find_map(|call| field(call, "version").map(str::to_owned))
            .unwrap_or_default();
        if have != version {
            return Err(format!(
                "release: {path} module() version is {have:?}, want {version:?}; {hint}"
            ));
        }
        if !family {
            continue;
        }
        let dep = calls(text, "bazel_dep")
            .iter()
            .find(|call| field(call, "name") == Some(BCR_WANT_MODULE))
            .and_then(|call| field(call, "version").map(str::to_owned))
            .unwrap_or_default();
        if dep != version {
            return Err(format!(
                "release: {path} bazel_dep on {BCR_WANT_MODULE} is at {dep:?}, want {version:?}; {hint}"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn family(name: &str, version: &str, dep: &str) -> String {
        format!(
            "module(\n    name = \"{name}\",\n    version = \"{version}\",\n)\n\nbazel_dep(name = \"rules_dx\", version = \"{dep}\")\n"
        )
    }

    fn core(version: &str) -> (String, String) {
        (
            "MODULE.bazel".to_owned(),
            format!("module(\n    name = \"rules_dx\",\n    version = \"{version}\",\n)\n"),
        )
    }

    fn tree(family_text: &str) -> Vec<(String, String)> {
        vec![
            core("1.2.3"),
            (
                "families/go/MODULE.bazel".to_owned(),
                family_text.to_owned(),
            ),
        ]
    }

    #[test]
    fn zero_families_is_core_only() {
        let files = vec![core("1.2.3")];
        assert_eq!(release_modules(&files), Ok(vec![ReleaseModule::core()]));
        module_versions_error(&files, "1.2.3").expect("versions");
    }

    #[test]
    fn a_family_with_matching_versions_is_a_module() {
        let files = tree(&family("rules_dx_go", "1.2.3", "1.2.3"));
        let modules = release_modules(&files).expect("modules");
        assert_eq!(modules.len(), 2);
        assert_eq!(modules[1].name, "rules_dx_go");
        assert_eq!(modules[1].root, "families/go");
        assert_eq!(modules[1].source_file(), "rules_dx_go.source.json");
        assert_eq!(modules[0].source_file(), "source.json");
        module_versions_error(&files, "1.2.3").expect("versions");
    }

    #[test]
    fn a_family_module_version_must_match_the_tag() {
        let files = tree(&family("rules_dx_go", "1.2.2", "1.2.3"));
        let error = module_versions_error(&files, "1.2.3").expect_err("version");
        assert!(
            error.contains("families/go/MODULE.bazel module() version"),
            "{error}"
        );
        assert!(error.ends_with("land the version bump first"), "{error}");
    }

    #[test]
    fn a_family_rules_dx_dep_must_match_the_tag() {
        let files = tree(&family("rules_dx_go", "1.2.3", "1.2.2"));
        let error = module_versions_error(&files, "1.2.3").expect_err("dep");
        assert!(error.contains("bazel_dep on rules_dx"), "{error}");
        assert!(error.ends_with("land the version bump first"), "{error}");
    }

    #[test]
    fn the_core_version_must_match_the_tag() {
        let error = module_versions_error(&[core("1.2.2")], "1.2.3").expect_err("core");
        assert!(error.contains("MODULE.bazel module() version"), "{error}");
    }

    #[test]
    fn a_family_must_be_named_after_its_directory() {
        let files = tree(&family("rules_dx_rust", "1.2.3", "1.2.3"));
        let error = release_modules(&files).expect_err("name");
        assert!(error.contains("want [\"rules_dx_go\"]"), "{error}");
    }
}
