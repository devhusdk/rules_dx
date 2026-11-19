//! The license ids of `<lang>/lang.json` `setup.licenses` and the `dx_licenses.accept` tags of a
//! `MODULE.bazel`.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::LazyLock;

/// The `use_extension` line that binds the `dx_licenses` extension.
pub const LICENSE_EXTENSION_LINE: &str =
    "dx_licenses = use_extension(\"@rules_dx//core:extensions.bzl\", \"dx_licenses\")";

/// `core/licenses/pins.json`, the one source of each license's terms hash.
const LICENSE_TERMS: &str = include_str!("../../../core/licenses/pins.json");

/// The decoded `LICENSE_TERMS`, an object from license id to its `url` and `terms_sha256`.
static TERMS: LazyLock<serde_json::Value> =
    LazyLock::new(|| serde_json::from_str(LICENSE_TERMS).unwrap_or_default());

/// The `terms_sha256` that `core/licenses/pins.json` pins for `id`.
#[must_use]
pub fn terms_sha256(id: &str) -> Option<&'static str> {
    TERMS.get(id)?.get("terms_sha256")?.as_str()
}

/// Whether `word` is a sha256 hex digest, the value side of a `terms_sha256` entry.
fn is_sha256(word: &str) -> bool {
    word.len() == 64 && word.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// A license id the workspace needs and the language that needs it.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct NeededLicense {
    pub id: String,
    pub language: String,
}

/// Every id a `lang.json` lists.
#[must_use]
pub fn known_license_ids() -> BTreeSet<String> {
    dx_families::licenses()
        .iter()
        .flat_map(|entry| entry.licenses.iter().map(|license| license.id.clone()))
        .collect()
}

/// The `<os>_<arch>` name of the running host, like `linux_x86_64`.
#[must_use]
pub fn host_platform() -> Option<&'static str> {
    crate::status::host_name(std::env::consts::OS, std::env::consts::ARCH)
}

/// The ids that the languages found under `root` need on `platform`.
#[must_use]
pub fn needed_licenses(root: &Path, platform: &str) -> BTreeSet<NeededLicense> {
    let marks = crate::families::file_extensions(root);
    dx_families::licenses()
        .iter()
        .filter(|entry| entry.marks.iter().any(|mark| marks.contains(mark)))
        .flat_map(|entry| {
            entry
                .licenses
                .iter()
                .filter(|license| license.platforms.iter().any(|p| p == platform))
                .map(|license| NeededLicense {
                    id: license.id.clone(),
                    language: entry.language.clone(),
                })
        })
        .collect()
}

/// A scan over Starlark string literals: `"` or `'` opens one, the same quote closes it, and a
/// backslash escapes the next character, so `'it"s'` and `"it's"` each read as one string.
#[derive(Clone, Copy, Default)]
struct Strings {
    open: Option<char>,
    escaped: bool,
}

impl Strings {
    /// Reads `ch` and tells whether it is code, outside every string and its quotes.
    fn code(&mut self, ch: char) -> bool {
        match self.open {
            Some(_) if self.escaped => self.escaped = false,
            Some(_) if ch == '\\' => self.escaped = true,
            Some(quote) if ch == quote => self.open = None,
            Some(_) => {}
            None if ch == '"' || ch == '\'' => self.open = Some(ch),
            None => return true,
        }
        false
    }
}

fn strip_comment(line: &str) -> &str {
    let mut strings = Strings::default();
    for (index, ch) in line.char_indices() {
        if strings.code(ch) && ch == '#' {
            return line.get(..index).unwrap_or(line);
        }
    }
    line
}

/// The text of the call whose body is `text`, the text just after its `(`.
fn call_body(text: &str) -> &str {
    let mut depth = 1usize;
    for (index, ch) in text.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return text.get(..index).unwrap_or_default();
                }
            }
            _ => {}
        }
    }
    text
}

/// The string literals in `text`, single- or double-quoted, each escape kept as the bare escaped
/// character.
fn quoted(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut strings = Strings::default();
    let mut current: Option<String> = None;
    for ch in text.chars() {
        let was_open = strings.open.is_some();
        let escaping = strings.escaped;
        strings.code(ch);
        match (was_open, strings.open.is_some()) {
            (false, true) => current = Some(String::new()),
            (true, false) => found.extend(current.take()),
            (true, true) if escaping || ch != '\\' => {
                if let Some(value) = current.as_mut() {
                    value.push(ch);
                }
            }
            _ => {}
        }
    }
    found
}

/// The names bound to the `dx_licenses` extension by `use_extension`.
fn extension_names(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut rest = text;
    while let Some((before, after)) = rest.split_once("use_extension(") {
        let body = call_body(after);
        let start = before.rfind('\n').map_or(0, |line| line + 1);
        if let Some((name, _)) = before.get(start..).unwrap_or_default().split_once('=')
            && quoted(body).iter().any(|word| word == "dx_licenses")
        {
            names.push(name.trim().to_owned());
        }
        rest = after;
    }
    names
}

/// The ids of every `dx_licenses.accept` tag in `module`, the text of a `MODULE.bazel`.
pub fn accepted_license_ids(module: &str) -> BTreeSet<String> {
    let text: String = module
        .lines()
        .map(strip_comment)
        .collect::<Vec<_>>()
        .join("\n");
    let mut ids = BTreeSet::new();
    for name in extension_names(&text) {
        let call = format!("{name}.accept(");
        let mut rest = text.as_str();
        while let Some((before, after)) = rest.split_once(call.as_str()) {
            let boundary = before
                .chars()
                .next_back()
                .is_none_or(|ch| !(ch.is_alphanumeric() || ch == '_' || ch == '.'));
            if boundary {
                ids.extend(
                    quoted(call_body(after))
                        .into_iter()
                        .filter(|word| !is_sha256(word)),
                );
            }
            rest = after;
        }
    }
    ids
}

/// The lines that accept `ids` in `module`: the `use_extension` line when no name binds the
/// extension, then one `accept` tag that pins the current terms hash of each id, so a later
/// change of the terms asks for a new acceptance. Empty when `ids` is empty.
pub fn render_license_accept(module: &str, ids: &BTreeSet<String>) -> String {
    if ids.is_empty() {
        return String::new();
    }
    let text: String = module
        .lines()
        .map(strip_comment)
        .collect::<Vec<_>>()
        .join("\n");
    let mut lines = String::new();
    let name = if let Some(name) = extension_names(&text).into_iter().next() {
        name
    } else {
        lines.push_str(LICENSE_EXTENSION_LINE);
        lines.push('\n');
        "dx_licenses".to_owned()
    };
    let list: Vec<String> = ids.iter().map(|id| format!("\"{id}\"")).collect();
    let pins: Vec<String> = ids
        .iter()
        .filter_map(|id| terms_sha256(id).map(|hash| format!("\"{id}\": \"{hash}\"")))
        .collect();
    if pins.is_empty() {
        lines.push_str(&format!("{name}.accept(ids = [{}])\n", list.join(", ")));
    } else {
        lines.push_str(&format!(
            "{name}.accept(ids = [{}], terms_sha256 = {{{}}})\n",
            list.join(", "),
            pins.join(", ")
        ));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(ids: &[&str]) -> BTreeSet<String> {
        ids.iter().map(|id| (*id).to_owned()).collect()
    }

    #[test]
    fn accepted_ids_read_every_tag_of_the_bound_name() {
        let module = "dx_licenses_dev = use_extension(\n    \"//core:extensions.bzl\",\n    \"dx_licenses\",\n    dev_dependency = True,\n)\ndx_licenses_dev.accept(ids = [\n    \"cuda-eula\",  # gpu\n])\ndx_licenses_dev.accept(ids = [\"android-sdk-license\"])\n# dx_licenses_dev.accept(ids = [\"windows-sdk-buildtools\"])\nother.accept(ids = [\"apple-xcode-license\"])\n";
        assert_eq!(
            accepted_license_ids(module),
            set(&["android-sdk-license", "cuda-eula"])
        );
    }

    #[test]
    fn single_quoted_strings_bind_and_accept() {
        let module = "lic = use_extension('@rules_dx//core:extensions.bzl', 'dx_licenses')\nlic.accept(ids = ['cuda-eula'])  # it's \"gpu\"\n# lic.accept(ids = ['android-sdk-license'])\n";
        assert_eq!(accepted_license_ids(module), set(&["cuda-eula"]));
        assert_eq!(
            render_license_accept(module, &set(&["a"])),
            "lic.accept(ids = [\"a\"])\n"
        );
    }

    #[test]
    fn quoted_reads_escapes_and_the_other_quote() {
        assert_eq!(
            quoted(r#"'a"b', "c'd", 'e\'f', "g\\""#),
            ["a\"b", "c'd", "e'f", "g\\"]
        );
        assert_eq!(strip_comment(r#"x = 'a#b' # c"#), "x = 'a#b' ");
    }

    #[test]
    fn every_known_id_has_a_terms_hash() {
        for id in known_license_ids() {
            assert!(terms_sha256(&id).is_some_and(is_sha256), "{id}");
        }
        assert_eq!(terms_sha256("nope"), None);
        let pinned: BTreeSet<String> = TERMS
            .as_object()
            .expect("core/licenses/pins.json is an object")
            .keys()
            .cloned()
            .collect();
        assert_eq!(pinned, known_license_ids());
    }

    #[test]
    fn accepted_ids_skip_the_terms_hashes() {
        let hash = "a".repeat(64);
        let module = format!(
            "{LICENSE_EXTENSION_LINE}\ndx_licenses.accept(ids = [\"cuda-eula\"], terms_sha256 = {{\"cuda-eula\": \"{hash}\"}})\n"
        );
        assert_eq!(accepted_license_ids(&module), set(&["cuda-eula"]));
    }

    #[test]
    fn render_adds_the_extension_only_when_unbound() {
        assert_eq!(
            render_license_accept("module(name = \"m\")\n", &set(&["cuda-eula"])),
            format!(
                "{LICENSE_EXTENSION_LINE}\ndx_licenses.accept(ids = [\"cuda-eula\"], terms_sha256 = {{\"cuda-eula\": \"{}\"}})\n",
                terms_sha256("cuda-eula").expect("cuda-eula terms")
            )
        );
        assert_eq!(
            render_license_accept(
                "lic = use_extension(\"@rules_dx//core:extensions.bzl\", \"dx_licenses\")\n",
                &set(&["a", "b"])
            ),
            "lic.accept(ids = [\"a\", \"b\"])\n"
        );
        assert_eq!(render_license_accept("", &BTreeSet::new()), "");
    }

    #[test]
    fn needed_licenses_follow_the_files_and_the_platform() {
        let scratch = dx_test_scratch::scratch("dx-adopt-licenses-").unwrap();
        let root = scratch.path();
        std::fs::write(root.join("main.cc"), "int main() {}\n").expect("write");
        let needed = needed_licenses(root, "windows_x86_64");
        assert!(
            needed.contains(&NeededLicense {
                id: "visual-studio-build-tools".to_owned(),
                language: "cc".to_owned(),
            }),
            "{needed:?}"
        );
        assert!(needed_licenses(root, "linux_x86_64").is_empty());
        assert!(known_license_ids().contains("visual-studio-build-tools"));
    }
}
