//! The license ids of `<lang>/lang.json` `setup.licenses` and the `dx_licenses.accept` tags of a
//! `MODULE.bazel`.

use std::collections::BTreeSet;
use std::path::Path;

/// The `use_extension` line that binds the `dx_licenses` extension.
pub const LICENSE_EXTENSION_LINE: &str =
    "dx_licenses = use_extension(\"@rules_dx//core:extensions.bzl\", \"dx_licenses\")";

/// A license id the workspace needs and the language that needs it.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct NeededLicense {
    pub id: String,
    pub language: String,
}

/// Every id a `lang.json` lists.
pub fn known_license_ids() -> BTreeSet<String> {
    dx_families::licenses()
        .iter()
        .flat_map(|entry| entry.licenses.iter().map(|license| license.id.clone()))
        .collect()
}

/// The `<os>_<arch>` name of the running host, like `linux_x86_64`.
pub fn host_platform() -> Option<&'static str> {
    crate::status::host_name(std::env::consts::OS, std::env::consts::ARCH)
}

/// The ids that the languages found under `root` need on `platform`.
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

fn strip_comment(line: &str) -> &str {
    let mut quoted = false;
    for (index, ch) in line.char_indices() {
        match ch {
            '"' => quoted = !quoted,
            '#' if !quoted => return &line[..index],
            _ => {}
        }
    }
    line
}

/// The text of the call that starts at `open`, the index just after its `(`.
fn call_body(text: &str, open: usize) -> &str {
    let mut depth = 1usize;
    for (index, ch) in text[open..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return &text[open..open + index];
                }
            }
            _ => {}
        }
    }
    &text[open..]
}

fn quoted(text: &str) -> Vec<String> {
    text.split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect()
}

/// The names bound to the `dx_licenses` extension by `use_extension`.
fn extension_names(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("use_extension(") {
        let open = at + "use_extension(".len();
        let body = call_body(rest, open);
        let start = rest[..at].rfind('\n').map_or(0, |line| line + 1);
        if let Some((name, _)) = rest[start..at].split_once('=')
            && quoted(body).iter().any(|word| word == "dx_licenses")
        {
            names.push(name.trim().to_owned());
        }
        rest = &rest[open..];
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
        while let Some(at) = rest.find(&call) {
            let open = at + call.len();
            let boundary = rest[..at]
                .chars()
                .next_back()
                .is_none_or(|ch| !(ch.is_alphanumeric() || ch == '_' || ch == '.'));
            if boundary {
                ids.extend(quoted(call_body(rest, open)));
            }
            rest = &rest[open..];
        }
    }
    ids
}

/// The lines that accept `ids` in `module`: the `use_extension` line when no name binds the
/// extension, then one `accept` tag. Empty when `ids` is empty.
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
    let name = match extension_names(&text).into_iter().next() {
        Some(name) => name,
        None => {
            lines.push_str(LICENSE_EXTENSION_LINE);
            lines.push('\n');
            "dx_licenses".to_owned()
        }
    };
    let list: Vec<String> = ids.iter().map(|id| format!("\"{id}\"")).collect();
    lines.push_str(&format!("{name}.accept(ids = [{}])\n", list.join(", ")));
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
    fn render_adds_the_extension_only_when_unbound() {
        assert_eq!(
            render_license_accept("module(name = \"m\")\n", &set(&["cuda-eula"])),
            format!("{LICENSE_EXTENSION_LINE}\ndx_licenses.accept(ids = [\"cuda-eula\"])\n")
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
        let scratch = dx_test_scratch::scratch("dx-adopt-licenses-");
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
