//! Resolves the `br:` module references of the `*.bicep` files of a set to their OCI manifests and layers.
#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::collections::BTreeMap;

use dx_lock::{Archive, Error, HttpFetch, Lock, Package, Resolution, Resolver, Scratch, Spec};

/// The media type of a Bicep module manifest.
pub const MANIFEST: &str = "application/vnd.oci.image.manifest.v1+json";

/// The media type of the compiled template layer of a Bicep module.
pub const LAYER: &str = "application/vnd.ms.bicep.module.layer.v1+json";

/// The registry the `br/public:` alias names.
pub const PUBLIC: &str = "mcr.microsoft.com/bicep";

/// Reads module manifests from a registry.
pub trait Registry {
    /// The manifest bytes at `url`.
    ///
    /// # Errors
    /// Fails when the download does.
    fn manifest(&self, url: &str) -> Result<Vec<u8>, Error>;
}

/// Reads manifests over HTTPS.
pub struct Site(HttpFetch);

impl Site {
    /// A registry client that refuses plain HTTP.
    #[must_use]
    pub fn new() -> Self {
        Self(HttpFetch::new())
    }
}

impl Default for Site {
    fn default() -> Self {
        Self::new()
    }
}

impl Registry for Site {
    fn manifest(&self, url: &str) -> Result<Vec<u8>, Error> {
        self.0.accepting(url, MANIFEST)
    }
}

/// One module reference: `HOST/PATH` and its tag.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Reference {
    /// `HOST/PATH`, the host in lowercase.
    pub name: String,
    /// The tag.
    pub tag: String,
}

impl Reference {
    fn parse(text: &str) -> Result<Self, Error> {
        let rest = match (text.strip_prefix("br:"), text.strip_prefix("br/public:")) {
            (Some(rest), _) => rest.to_owned(),
            (_, Some(rest)) => format!("{PUBLIC}/{rest}"),
            _ => return Err(Error::new(format!("{text:?} is not a br: reference"))),
        };
        let Some((name, tag)) = rest.rsplit_once(':') else {
            return Err(Error::new(format!("{text} has no :<tag>")));
        };
        let Some((host, path)) = name.split_once('/') else {
            return Err(Error::new(format!("{text} has no repository path")));
        };
        let segment = |part: &str| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && part
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        };
        if !segment(host) || !path.split('/').all(segment) {
            return Err(Error::new(format!(
                "{text}: host and path take letters, digits, '.', '-' and '_' only"
            )));
        }
        if tag.is_empty()
            || !tag
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        {
            return Err(Error::new(format!("{text} has no valid tag")));
        }
        Ok(Self {
            name: format!("{}/{path}", host.to_ascii_lowercase()),
            tag: tag.to_owned(),
        })
    }

    fn registry(&self) -> (&str, &str) {
        self.name.split_once('/').unwrap_or((&self.name, ""))
    }

    /// The URL of a manifest by tag or digest.
    #[must_use]
    pub fn manifest_url(&self, reference: &str) -> String {
        let (host, path) = self.registry();
        format!("https://{host}/v2/{path}/manifests/{reference}")
    }

    /// The URL of a blob by digest.
    #[must_use]
    pub fn blob_url(&self, digest: &str) -> String {
        let (host, path) = self.registry();
        format!("https://{host}/v2/{path}/blobs/{digest}")
    }
}

/// The single-quoted strings of Bicep source outside comments.
fn strings(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '/' if chars.peek() == Some(&'/') => {
                for next in chars.by_ref() {
                    if next == '\n' {
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut star = false;
                for next in chars.by_ref() {
                    if star && next == '/' {
                        break;
                    }
                    star = next == '*';
                }
            }
            '\'' => {
                let mut value = String::new();
                while let Some(next) = chars.next() {
                    match next {
                        '\\' => value.extend(chars.next()),
                        '\'' => break,
                        other => value.push(other),
                    }
                }
                out.push(value);
            }
            _ => {}
        }
    }
    out
}

/// The module references of Bicep source, in source order.
///
/// # Errors
/// Fails on a `br:` or `br/public:` string that is not a valid reference.
pub fn references(text: &str) -> Result<Vec<Reference>, Error> {
    strings(text)
        .iter()
        .filter(|value| value.starts_with("br:") || value.starts_with("br/public:"))
        .map(|value| Reference::parse(value))
        .collect()
}

fn hex(digest: &str) -> Option<&str> {
    digest
        .strip_prefix("sha256:")
        .filter(|hex| dx_lock::is_sha256(hex))
}

/// The sha256 of the one Bicep module layer of a manifest.
///
/// # Errors
/// Fails when the manifest is not an OCI manifest with exactly one Bicep module layer.
pub fn module_layer(name: &str, manifest: &[u8]) -> Result<String, Error> {
    let value: serde_json::Value = serde_json::from_slice(manifest)
        .map_err(|error| Error::new(format!("{name}: manifest is not JSON: {error}")))?;
    let layers: Vec<&serde_json::Value> = value
        .get("layers")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| Error::new(format!("{name}: manifest has no layers")))?
        .iter()
        .filter(|layer| layer.get("mediaType").and_then(serde_json::Value::as_str) == Some(LAYER))
        .collect();
    let [layer] = layers.as_slice() else {
        return Err(Error::new(format!(
            "{name}: manifest has {} layers of type {LAYER}, expected 1",
            layers.len()
        )));
    };
    layer
        .get("digest")
        .and_then(serde_json::Value::as_str)
        .and_then(hex)
        .map(str::to_owned)
        .ok_or_else(|| Error::new(format!("{name}: layer {LAYER} has no sha256 digest")))
}

/// The resolver of the bicep ecosystem.
pub struct Bicep<'a> {
    registry: &'a dyn Registry,
}

impl<'a> Bicep<'a> {
    /// A resolver that reads manifests from `registry`.
    #[must_use]
    pub fn new(registry: &'a dyn Registry) -> Self {
        Self { registry }
    }

    /// The lock step values of the registry entry.
    #[must_use]
    pub fn spec(hub: &dx_families::Hub) -> Spec {
        Spec {
            ecosystem: hub.ecosystem.clone(),
            manifest: vec![hub.manifest.clone()],
            state: hub.state.iter().cloned().collect(),
        }
    }

    fn package(&self, reference: &Reference) -> Result<Package, Error> {
        let name = &reference.name;
        let manifest = self
            .registry
            .manifest(&reference.manifest_url(&reference.tag))
            .map_err(|error| Error::new(format!("{name}:{}: {error}", reference.tag)))?;
        let digest = dx_digest::sha256_hex(&manifest);
        let layer = module_layer(name, &manifest)?;
        Ok(Package {
            archives: vec![Archive {
                urls: vec![reference.blob_url(&format!("sha256:{layer}"))],
                sha256: layer,
                ..Archive::default()
            }],
            extra: BTreeMap::from([(
                "bicep_manifest".to_owned(),
                serde_json::json!({
                    "sha256": digest,
                    "urls": [reference.manifest_url(&format!("sha256:{digest}"))],
                }),
            )]),
            provides: vec![format!("br:{name}:{}", reference.tag)],
            name: name.clone(),
            version: reference.tag.clone(),
            ..Package::default()
        })
    }
}

fn read_references(scratch: &Scratch) -> Result<Vec<Reference>, Error> {
    let entries = std::fs::read_dir(scratch.set())
        .map_err(|error| Error::new(format!("{}: {error}", scratch.set().display())))?;
    let mut files: Vec<_> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "bicep"))
        .collect();
    files.sort();
    let mut tags: BTreeMap<String, String> = BTreeMap::new();
    for file in files {
        let shown = file.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let text = std::fs::read_to_string(&file)
            .map_err(|error| Error::new(format!("{shown}: {error}")))?;
        for reference in references(&text).map_err(|error| Error::new(format!("{shown}: {error}")))? {
            match tags.get(&reference.name) {
                Some(tag) if *tag != reference.tag => {
                    return Err(Error::new(format!(
                        "{shown}: {} is referenced with the tags {tag} and {}",
                        reference.name, reference.tag
                    )));
                }
                _ => {
                    tags.insert(reference.name, reference.tag);
                }
            }
        }
    }
    Ok(tags
        .into_iter()
        .map(|(name, tag)| Reference { name, tag })
        .collect())
}

impl Resolver for Bicep<'_> {
    fn resolve(&self, scratch: &Scratch, old: Option<&Lock>) -> Result<Resolution, Error> {
        let mut packages = Vec::new();
        for reference in read_references(scratch)? {
            let kept = old.and_then(|lock| {
                lock.packages.iter().find(|package| {
                    package.name == reference.name && package.version == reference.tag
                })
            });
            packages.push(match kept {
                Some(package) => package.clone(),
                None => self.package(&reference)?,
            });
        }
        Ok(Resolution { packages })
    }
}
