use std::collections::BTreeMap;
use std::path::Path;

use crate::{DepInfo, DepcheckError};

use super::put;

/// Reads the `build-depends` of every component in a `.cabal` file. Test suites and
/// benchmarks are `dev`; `base` and the package's own library are not dependencies to check.
pub fn parse_haskell_manifest(path: &Path) -> Result<BTreeMap<String, DepInfo>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::ManifestIo)?;
    let mut package = String::new();
    let mut category = "prod";
    let mut in_depends = false;
    let mut field_indent = 0;
    let mut body = String::new();
    let mut entries: Vec<(String, &'static str)> = Vec::new();
    let mut flush = |body: &mut String, category: &'static str| {
        if !body.is_empty() {
            entries.push((std::mem::take(body), category));
        }
    };
    for rawline in text.lines() {
        let line = rawline.split("--").next().unwrap_or("");
        let item = line.trim();
        if item.is_empty() {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        if in_depends && indent > field_indent {
            body.push(' ');
            body.push_str(item);
            continue;
        }
        if in_depends {
            flush(&mut body, category);
            in_depends = false;
        }
        let lower = item.to_lowercase();
        if indent == 0 {
            if let Some(name) = lower.strip_prefix("name:") {
                package = name.trim().to_owned();
            } else if lower.starts_with("test-suite") || lower.starts_with("benchmark") {
                category = "dev";
            } else {
                category = "prod";
            }
        }
        if let Some(rest) = lower.strip_prefix("build-depends:") {
            in_depends = true;
            field_indent = indent;
            body = item[item.len() - rest.len()..].to_owned();
        }
    }
    flush(&mut body, category);
    let mut deps = BTreeMap::new();
    for (body, category) in entries {
        for entry in body.split(',') {
            let entry = entry.trim();
            let name_end = entry
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
                .unwrap_or(entry.len());
            let name = &entry[..name_end];
            if name.is_empty() || name == "base" || name.eq_ignore_ascii_case(&package) {
                continue;
            }
            put(
                &mut deps,
                name.to_owned(),
                DepInfo {
                    spec: haskell_spec(&entry[name_end..]),
                    category: category.to_owned(),
                    optional: false,
                    platform: false,
                    raw: name.to_owned(),
                    peer: false,
                    sha256: String::new(),
                    package: String::new(),
                },
            );
        }
    }
    Ok(deps)
}

/// Turns a cabal version range into the comma-separated form `satisfies_for` reads:
/// `^>=1.2` becomes `~1.2`, `==1.2.*` becomes `~1.2`, `&&` becomes `,`.
fn haskell_spec(range: &str) -> String {
    let range = range.trim();
    if range.is_empty() || range.contains("||") {
        return "*".to_owned();
    }
    let parts: Vec<String> = range
        .split("&&")
        .map(|part| {
            let part = part.trim().replace(' ', "");
            if let Some(rest) = part.strip_prefix("^>=") {
                format!("~{rest}")
            } else if let Some(rest) = part.strip_prefix("==") {
                match rest.strip_suffix(".*") {
                    Some(prefix) => format!("~{prefix}"),
                    None => format!("=={rest}"),
                }
            } else {
                part
            }
        })
        .collect();
    parts.join(", ")
}

/// Reads the pinned packages of `cabal.project.freeze` (`any.text ==2.0.2`) or
/// `stack.yaml.lock` (`hackage: text-2.0.2@sha256:...`).
pub fn parse_haskell_lock(path: &Path) -> Result<BTreeMap<String, String>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::LockIo)?;
    let freeze = regex::Regex::new(
        r"(?:^|[\s,])(?:any\.|\w+:)?([A-Za-z0-9][A-Za-z0-9-]*)\s*==\s*([0-9][0-9.]*)",
    )
    .map_err(DepcheckError::LockRegex)?;
    let stack =
        regex::Regex::new(r"hackage:\s*([A-Za-z0-9][A-Za-z0-9-]*)-([0-9][0-9.]*)(?:@|\s|$)")
            .map_err(DepcheckError::LockRegex)?;
    let mut pkgs = BTreeMap::new();
    for line in text.lines() {
        if line.trim_start().starts_with("--") {
            continue;
        }
        if let Some(caps) = stack.captures(line) {
            pkgs.insert(caps[1].to_owned(), caps[2].to_owned());
        } else {
            for caps in freeze.captures_iter(line) {
                pkgs.insert(caps[1].to_owned(), caps[2].to_owned());
            }
        }
    }
    Ok(pkgs)
}

/// Module prefixes a package exposes. Cabal packages carry no import list without the
/// solver's plan, so well known packages are listed and others fall back to their own name.
pub(crate) fn module_prefixes(package: &str) -> Vec<String> {
    let known: &[&str] = match package {
        "containers" => &[
            "Data.Map",
            "Data.Set",
            "Data.IntMap",
            "Data.IntSet",
            "Data.Sequence",
            "Data.Tree",
            "Data.Graph",
            "Data.Containers",
        ],
        "mtl" => &[
            "Control.Monad.State",
            "Control.Monad.Reader",
            "Control.Monad.Writer",
            "Control.Monad.Except",
            "Control.Monad.RWS",
            "Control.Monad.Error",
            "Control.Monad.Cont",
            "Control.Monad.Identity",
            "Control.Monad.Accum",
            "Control.Monad.Select",
        ],
        "transformers" => &[
            "Control.Monad.Trans",
            "Control.Applicative.Backwards",
            "Data.Functor.Reverse",
        ],
        "bytestring" => &["Data.ByteString"],
        "text" => &["Data.Text"],
        "unordered-containers" => &["Data.HashMap", "Data.HashSet"],
        "hashable" => &["Data.Hashable"],
        "time" => &["Data.Time"],
        "directory" => &["System.Directory"],
        "filepath" => &["System.FilePath"],
        "process" => &["System.Process"],
        "unix" => &["System.Posix"],
        "random" => &["System.Random"],
        "array" => &["Data.Array"],
        "stm" => &["Control.Concurrent.STM", "Control.Monad.STM"],
        "deepseq" => &["Control.DeepSeq"],
        "async" => &["Control.Concurrent.Async"],
        "aeson" => &["Data.Aeson"],
        "QuickCheck" => &["Test.QuickCheck"],
        "HUnit" => &["Test.HUnit"],
        "hspec" => &["Test.Hspec"],
        "tasty" => &["Test.Tasty"],
        "tasty-hunit" => &["Test.Tasty.HUnit"],
        "tasty-quickcheck" => &["Test.Tasty.QuickCheck"],
        "optparse-applicative" => &["Options.Applicative"],
        "megaparsec" => &["Text.Megaparsec"],
        "parsec" => &["Text.Parsec"],
        "lens" => &["Control.Lens"],
        "http-client" => &["Network.HTTP.Client"],
        "wai" => &["Network.Wai"],
        "warp" => &["Network.Wai.Handler.Warp"],
        "network" => &["Network.Socket"],
        "vector" => &["Data.Vector"],
        "scientific" => &["Data.Scientific"],
        _ => &[],
    };
    if !known.is_empty() {
        return known.iter().map(|module| (*module).to_owned()).collect();
    }
    let camel: String = package
        .split('-')
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().chain(chars).collect::<String>())
                .unwrap_or_default()
        })
        .collect();
    vec![camel]
}
