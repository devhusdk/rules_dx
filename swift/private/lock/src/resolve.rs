//! Resolves a Swift package set from its pins and the targets `swift package describe` prints.

use std::collections::BTreeMap;
use std::path::PathBuf;

use dx_lock::{Archive, Error, HttpFetch, Lock, Package, Resolution, Resolver, Scratch, Spec};
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
    manifest: String,
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
        Self {
            swift,
            manifest: spec.manifest.first().cloned().unwrap_or_default(),
            state: spec.state.first().cloned().unwrap_or_default(),
            download,
        }
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

    /// Writes the state file from the root manifest: each `exact:` GitHub requirement becomes the
    /// commit its tag names.
    fn write_state(&self, scratch: &Scratch) -> Result<(), Error> {
        let text = std::fs::read_to_string(scratch.set().join(&self.manifest))
            .map_err(|error| Error::new(format!("{}: {error}", self.manifest)))?;
        let at = |url: &str, message: String| {
            let line = text.lines().position(|line| line.contains(url)).map_or(1, |index| index + 1);
            Error::new(format!("{}:{line}: {message}", self.manifest))
        };
        let dumped = self
            .package_json(scratch, scratch.set(), "dump-package", &[])
            .map_err(|message| Error::new(format!("{}: {message}", self.manifest)))?;
        let mut pins = BTreeMap::new();
        for dependency in elements(&dumped["dependencies"]) {
            let source = &dependency["sourceControl"][0];
            let location = &source["location"]["remote"][0];
            let url = location["urlString"].as_str().or_else(|| location.as_str()).ok_or_else(|| {
                Error::new(format!("{}: a dependency is not a source control dependency", self.manifest))
            })?;
            let version = source["requirement"]["exact"][0]
                .as_str()
                .ok_or_else(|| at(url, format!("{url} is not an exact version requirement")))?;
            let (owner, repo) = github_repository(url)
                .ok_or_else(|| at(url, format!("{url} is not on github.com; only github.com is supported")))?;
            let reference = format!("https://api.github.com/repos/{owner}/{repo}/git/ref/tags/{version}");
            let object = self.api(&reference).map_err(|error| at(url, error.to_string()))?;
            let mut object = object["object"].clone();
            if object["type"] == "tag" {
                let tag = field(&object, "url").map_err(|error| at(url, error.to_string()))?.to_owned();
                object = self.api(&tag).map_err(|error| at(url, error.to_string()))?["object"].clone();
            }
            let revision = field(&object, "sha").map_err(|error| at(url, format!("{reference}: {error}")))?;
            pins.insert(repo.to_lowercase(), (url.to_owned(), version.to_owned(), revision.to_owned()));
        }
        let quoted = |text: &str| Value::from(text).to_string();
        let entries: Vec<String> = pins
            .iter()
            .map(|(identity, (url, version, revision))| {
                format!(
                    "    {{\n      \"identity\" : {},\n      \"kind\" : \"remoteSourceControl\",\n      \"location\" : {},\n      \"state\" : {{\n        \"revision\" : {},\n        \"version\" : {}\n      }}\n    }}",
                    quoted(identity),
                    quoted(url),
                    quoted(revision),
                    quoted(version)
                )
            })
            .collect();
        let state = format!("{{\n  \"pins\" : [\n{}\n  ],\n  \"version\" : 3\n}}\n", entries.join(",\n"));
        std::fs::write(scratch.set().join(&self.state), state).map_err(|error| Error::new(format!("{}: {error}", self.state)))
    }

    fn api(&self, url: &str) -> Result<Value, Error> {
        let bytes = self.download.bytes(url).map_err(|error| Error::new(format!("{url}: {error}")))?;
        serde_json::from_slice(&bytes).map_err(|error| Error::new(format!("{url}: {error}")))
    }

    fn pins(&self, scratch: &Scratch) -> Result<Vec<Pin>, Error> {
        let path = scratch.set().join(&self.state);
        if !path.exists() {
            self.write_state(scratch)?;
        }
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
            let repository = github_repository(location)
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

    fn package_json(&self, scratch: &Scratch, package: &std::path::Path, command: &str, args: &[&str]) -> Result<Value, String> {
        let output = scratch
            .command(&self.swift)
            .arg("package")
            .arg(command)
            .args(args)
            .current_dir(package)
            .output()
            .map_err(|error| format!("swift: {error}"))?;
        if !output.status.success() {
            return Err(format!("swift package {command} failed:\n{}", String::from_utf8_lossy(&output.stderr)));
        }
        serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())
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
        let dumped = self.package_json(scratch, &package, "dump-package", &[]).map_err(failed)?;
        if !elements(&dumped["dependencies"]).is_empty() {
            return Err(failed("declares dependencies on other packages; only packages without dependencies are supported".to_owned()));
        }
        let described = self.package_json(scratch, &package, "describe", &["--type", "json"]).map_err(failed)?;
        let (products, targets) = library_targets(&described).map_err(failed)?;
        Ok(Described { pin, sha256, products, targets })
    }
}

fn github_repository(location: &str) -> Option<(&str, &str)> {
    location
        .strip_prefix("https://github.com/")
        .map(|rest| rest.trim_end_matches('/').trim_end_matches(".git"))
        .and_then(|rest| rest.split_once('/'))
        .filter(|(owner, repo)| !owner.is_empty() && !repo.is_empty() && !repo.contains('/'))
}

fn elements(value: &Value) -> &[Value] {
    value.as_array().map_or(&[], Vec::as_slice)
}

/// The library products of a described package and the targets they need, by name.
///
/// A product whose targets include one that is not a Swift library, such as a C, macro or executable
/// target, is left out.
///
/// # Errors
/// Fails when a product has more than one target or a target is not described.
pub fn library_targets(described: &Value) -> Result<(BTreeMap<String, Vec<String>>, BTreeMap<String, Value>), String> {
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
    let mut kept = BTreeMap::new();
    let mut targets = BTreeMap::new();
    for (product, roots) in products {
        let mut closure = BTreeMap::new();
        let mut pending = roots.clone();
        let mut supported = true;
        while let Some(name) = pending.pop() {
            if closure.contains_key(&name) {
                continue;
            }
            let target = all.get(name.as_str()).ok_or(format!("target {name} is not described"))?;
            if target["module_type"] != "SwiftTarget" || target["type"] != "library" {
                supported = false;
                break;
            }
            pending.extend(strings(&target["target_dependencies"]));
            closure.insert(name, (*target).clone());
        }
        if supported {
            kept.insert(product, roots);
            targets.extend(closure);
        }
    }
    Ok((kept, targets))
}

/// The package of `old` that has the name, version and archive URL of `pin`.
fn unchanged<'l>(old: &'l Lock, pin: &Pin) -> Option<&'l Package> {
    old.packages.iter().find(|package| {
        package.name == pin.identity
            && package.version == pin.version
            && package.archives.len() == 1
            && package.archives[0].urls == [pin.url.as_str()]
            && package.archives[0].strip_prefix == pin.strip_prefix
    })
}

impl Resolver for Swift<'_> {
    fn resolve(&self, scratch: &Scratch, old: Option<&Lock>) -> Result<Resolution, Error> {
        let mut kept = Vec::new();
        let mut described = Vec::new();
        for pin in self.pins(scratch)? {
            match old.and_then(|lock| unchanged(lock, &pin)) {
                Some(package) => kept.push(package.clone()),
                None => described.push(self.describe(scratch, pin)?),
            }
        }
        let mut owners: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for entry in &described {
            for product in entry.products.keys() {
                owners.entry(product).or_default().push(&entry.pin.identity);
            }
        }
        for package in &kept {
            for product in package.extra.get("swift_products").and_then(Value::as_object).into_iter().flatten().map(|(name, _)| name) {
                owners.entry(product).or_default().push(&package.name);
            }
        }
        for package in &kept {
            for target in package.extra.get("swift_targets").and_then(Value::as_object).into_iter().flat_map(Map::values) {
                for dependency in strings(&target["product_deps"]) {
                    let found = dependency.split_once(':').is_some_and(|(owner, product)| {
                        owners.get(product).is_some_and(|all| all.contains(&owner))
                    });
                    if !found {
                        return Err(Error::new(format!(
                            "pin {}: {dependency} is gone; delete the lock to resolve every pin again",
                            package.name
                        )));
                    }
                }
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
        packages.extend(kept);
        Ok(Resolution { packages })
    }
}
