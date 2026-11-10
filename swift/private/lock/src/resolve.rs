//! Resolves a Swift package set from its pins and the targets `swift package describe` prints.

use std::collections::BTreeMap;
use std::path::PathBuf;

use dx_lock::{Archive, Error, HttpFetch, Package, Resolution, Resolver, Scratch, Spec};
use serde_json::{Map, Value, json};

/// Downloads archives.
pub trait Download {
    /// The bytes at `url`.
    ///
    /// # Errors
    /// Fails when the download does.
    fn bytes(&self, url: &str) -> Result<Vec<u8>, Error>;
}

/// Downloads over HTTPS.
pub struct Github(HttpFetch);

impl Github {
    /// A downloader that refuses plain HTTP.
    #[must_use]
    pub fn new() -> Self {
        Self(HttpFetch::new())
    }
}

impl Default for Github {
    fn default() -> Self {
        Self::new()
    }
}

impl Download for Github {
    fn bytes(&self, url: &str) -> Result<Vec<u8>, Error> {
        self.0.bytes(url)
    }
}

/// The resolver of the swiftpm ecosystem.
pub struct Swift<'a> {
    swift: PathBuf,
    state: String,
    download: &'a dyn Download,
}

/// One pin of the state file.
struct Pin {
    identity: String,
    version: String,
    url: String,
    strip_prefix: String,
}

/// What `swift package describe` printed for one pin.
struct Described {
    pin: Pin,
    sha256: String,
    products: BTreeMap<String, Vec<String>>,
    targets: BTreeMap<String, Value>,
}

fn field<'v>(value: &'v Value, key: &str) -> Result<&'v str, Error> {
    value[key].as_str().ok_or_else(|| Error::new(format!("{key} is missing")))
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item.as_str().map(str::to_owned))
        .collect()
}

impl<'a> Swift<'a> {
    /// A resolver that runs `swift` and downloads with `download`.
    #[must_use]
    pub fn new(swift: PathBuf, spec: &Spec, download: &'a dyn Download) -> Self {
        Self { swift, state: spec.state.first().cloned().unwrap_or_default(), download }
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

    fn pins(&self, scratch: &Scratch) -> Result<Vec<Pin>, Error> {
        let path = scratch.set().join(&self.state);
        let text = std::fs::read_to_string(&path)
            .map_err(|error| Error::new(format!("{}: {error}", self.state)))?;
        let state: Value = serde_json::from_str(&text).map_err(|error| Error::new(format!("{}: {error}", self.state)))?;
        let version = state["version"].as_u64().unwrap_or(0);
        if version != 2 && version != 3 {
            return Err(Error::new(format!("{} has version {version}; want 2 or 3", self.state)));
        }
        let mut pins = Vec::new();
        for pin in state["pins"].as_array().into_iter().flatten() {
            let identity = field(pin, "identity")?.to_owned();
            let location = field(pin, "location")?;
            let revision = field(&pin["state"], "revision")?;
            let repository = location
                .strip_prefix("https://github.com/")
                .map(|rest| rest.trim_end_matches('/').trim_end_matches(".git"))
                .and_then(|rest| rest.split_once('/'))
                .filter(|(owner, repo)| !owner.is_empty() && !repo.is_empty() && !repo.contains('/'))
                .ok_or_else(|| Error::new(format!("pin {identity} is at {location}; only github.com is supported")))?;
            pins.push(Pin {
                version: pin["state"]["version"].as_str().unwrap_or(revision).to_owned(),
                url: format!("https://github.com/{}/{}/archive/{revision}.tar.gz", repository.0, repository.1),
                strip_prefix: format!("{}-{revision}", repository.1),
                identity,
            });
        }
        Ok(pins)
    }

    fn describe(&self, scratch: &Scratch, pin: Pin) -> Result<Described, Error> {
        let identity = &pin.identity;
        let failed = |message: String| Error::new(format!("pin {identity}: {message}"));
        let bytes = self.download.bytes(&pin.url).map_err(|error| failed(format!("{}: {error}", pin.url)))?;
        let sha256 = dx_digest::sha256_hex(&bytes);
        let root = scratch.tmp().join(identity);
        std::fs::create_dir_all(&root).map_err(|error| failed(error.to_string()))?;
        let source = root.join("source");
        tar::Archive::new(flate2::read::GzDecoder::new(bytes.as_slice()))
            .unpack(&source)
            .map_err(|error| failed(error.to_string()))?;
        let package = source.join(&pin.strip_prefix);
        let output = scratch
            .command(&self.swift)
            .args(["package", "describe", "--type", "json"])
            .current_dir(&package)
            .output()
            .map_err(|error| failed(format!("swift: {error}")))?;
        if !output.status.success() {
            return Err(failed(format!("swift package describe failed:\n{}", String::from_utf8_lossy(&output.stderr))));
        }
        let described: Value = serde_json::from_slice(&output.stdout).map_err(|error| failed(error.to_string()))?;
        let (products, targets) = library_targets(&described).map_err(failed)?;
        Ok(Described { pin, sha256, products, targets })
    }
}

/// The library products of a described package and the targets they need, by name.
fn library_targets(described: &Value) -> Result<(BTreeMap<String, Vec<String>>, BTreeMap<String, Value>), String> {
    let mut products = BTreeMap::new();
    for product in described["products"].as_array().into_iter().flatten() {
        if product["type"].get("library").is_none() {
            continue;
        }
        let name = product["name"].as_str().ok_or("a product has no name")?;
        let targets = strings(&product["targets"]);
        if targets.len() != 1 {
            return Err(format!("product {name} has {} targets; want 1", targets.len()));
        }
        products.insert(name.to_owned(), targets);
    }
    let all: BTreeMap<&str, &Value> = described["targets"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|target| Some((target["name"].as_str()?, target)))
        .collect();
    let mut targets = BTreeMap::new();
    let mut pending: Vec<String> = products.values().flatten().cloned().collect();
    while let Some(name) = pending.pop() {
        if targets.contains_key(&name) {
            continue;
        }
        let target = all.get(name.as_str()).ok_or(format!("target {name} is not described"))?;
        if target["module_type"] != "SwiftTarget" || target["type"] != "library" {
            return Err(format!("target {name} is not a Swift library"));
        }
        pending.extend(strings(&target["target_dependencies"]));
        targets.insert(name, (*target).clone());
    }
    Ok((products, targets))
}

impl Resolver for Swift<'_> {
    fn resolve(&self, scratch: &Scratch) -> Result<Resolution, Error> {
        let mut described = Vec::new();
        for pin in self.pins(scratch)? {
            described.push(self.describe(scratch, pin)?);
        }
        let mut owners: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for entry in &described {
            for product in entry.products.keys() {
                owners.entry(product).or_default().push(&entry.pin.identity);
            }
        }
        let mut packages = Vec::new();
        for entry in &described {
            let identity = &entry.pin.identity;
            let mut deps = Vec::new();
            let mut targets = Map::new();
            for (name, target) in &entry.targets {
                let mut product_deps = Vec::new();
                for product in strings(&target["product_dependencies"]) {
                    let owner = match owners.get(product.as_str()).map(Vec::as_slice) {
                        Some([owner]) => *owner,
                        Some(many) => {
                            return Err(Error::new(format!(
                                "pin {identity}: product {product} is in {}",
                                many.join(" and ")
                            )));
                        }
                        None => return Err(Error::new(format!("pin {identity}: product {product} is in no pin"))),
                    };
                    product_deps.push(format!("{owner}:{product}"));
                    deps.push(owner.to_owned());
                }
                let path = field(target, "path").map_err(|error| Error::new(format!("pin {identity}: {error}")))?;
                let srcs: Vec<String> =
                    strings(&target["sources"]).iter().map(|source| format!("{path}/{source}")).collect();
                targets.insert(
                    name.clone(),
                    json!({
                        "deps": strings(&target["target_dependencies"]),
                        "module": field(target, "c99name").map_err(|error| Error::new(format!("pin {identity}: {error}")))?,
                        "path": path,
                        "product_deps": product_deps,
                        "srcs": srcs,
                    }),
                );
            }
            let provides = entry
                .targets
                .values()
                .filter_map(|target| target["c99name"].as_str().map(str::to_owned))
                .collect();
            let mut extra = BTreeMap::new();
            extra.insert("swift_products".to_owned(), json!(entry.products));
            extra.insert("swift_targets".to_owned(), Value::Object(targets));
            packages.push(Package {
                name: identity.clone(),
                version: entry.pin.version.clone(),
                archives: vec![Archive {
                    urls: vec![entry.pin.url.clone()],
                    sha256: entry.sha256.clone(),
                    strip_prefix: entry.pin.strip_prefix.clone(),
                    platform: String::new(),
                }],
                deps,
                provides,
                dev: false,
                extra,
            });
        }
        Ok(Resolution { packages })
    }
}
