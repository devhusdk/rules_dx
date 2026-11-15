//! Resolves a Swift package set from its pins and the targets `swift package describe` prints.
#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::collections::{BTreeMap, BTreeSet};
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
    settings: BTreeMap<String, Settings>,
}

/// The unconditional settings of one target of a dumped package.
#[derive(Default)]
struct Settings {
    defines: Vec<String>,
    c_defines: Vec<String>,
    header_paths: Vec<String>,
    include: String,
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
        for dependency in elements(dumped.get("dependencies")) {
            let source = dependency.pointer("/sourceControl/0");
            let url = source
                .and_then(|source| source.pointer("/location/remote/0"))
                .and_then(|location| {
                    location
                        .get("urlString")
                        .and_then(Value::as_str)
                        .or_else(|| location.as_str())
                })
                .ok_or_else(|| {
                    Error::new(format!(
                        "{}: a dependency is not a source control dependency",
                        self.manifest
                    ))
                })?;
            let version = source
                .and_then(|source| source.pointer("/requirement/exact/0"))
                .and_then(Value::as_str)
                .ok_or_else(|| at(url, format!("{url} is not an exact version requirement")))?;
            let (owner, repo) = github_repository(url).ok_or_else(|| {
                at(
                    url,
                    format!("{url} is not on github.com; only github.com is supported"),
                )
            })?;
            let reference =
                format!("https://api.github.com/repos/{owner}/{repo}/git/ref/tags/{version}");
            let object = self
                .api(&reference)
                .map_err(|error| at(url, error.to_string()))?;
            let mut object = object
                .get("object")
                .cloned()
                .ok_or_else(|| at(url, format!("{reference} has no object")))?;
            if object.get("type").and_then(Value::as_str) == Some("tag") {
                let tag = field(&object, "url")
                    .map_err(|error| at(url, error.to_string()))?
                    .to_owned();
                object = self
                    .api(&tag)
                    .map_err(|error| at(url, error.to_string()))?
                    .get("object")
                    .cloned()
                    .ok_or_else(|| at(url, format!("{tag} has no object")))?;
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
        let state: Value = serde_json::from_str(&text)
            .map_err(|error| Error::new(format!("{}: {error}", self.state)))?;
        let version = state.get("version").and_then(Value::as_u64).unwrap_or(0);
        if version != 2 && version != 3 {
            return Err(Error::new(format!("{} has version {version}; want 2 or 3", self.state)));
        }
        let mut pins = Vec::new();
        for pin in elements(state.get("pins")) {
            let identity = field(pin, "identity")?.to_owned();
            let location = field(pin, "location")?;
            let pin_state = pin
                .get("state")
                .ok_or_else(|| Error::new(format!("pin {identity} has no state")))?;
            let revision = field(pin_state, "revision")?;
            let repository = github_repository(location).ok_or_else(|| {
                Error::new(format!(
                    "pin {identity} is at {location}; only github.com is supported"
                ))
            })?;
            pins.push(Pin {
                version: pin_state
                    .get("version")
                    .and_then(Value::as_str)
                    .unwrap_or(revision)
                    .to_owned(),
                url: format!(
                    "https://github.com/{}/{}/archive/{revision}.tar.gz",
                    repository.0, repository.1
                ),
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
            .arg("--disable-sandbox")
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
        let dumped = self
            .package_json(scratch, &package, "dump-package", &[])
            .map_err(failed)?;
        if !elements(dumped.get("dependencies")).is_empty() {
            return Err(failed("declares dependencies on other packages; only packages without dependencies are supported".to_owned()));
        }
        let described = self.package_json(scratch, &package, "describe", &["--type", "json"]).map_err(failed)?;
        let (settings, conditional) = target_settings(&dumped).map_err(failed)?;
        let (products, targets) = library_targets(&described, &conditional).map_err(failed)?;
        Ok(Described { pin, sha256, products, targets, settings })
    }
}

fn github_repository(location: &str) -> Option<(&str, &str)> {
    location
        .strip_prefix("https://github.com/")
        .map(|rest| rest.trim_end_matches('/').trim_end_matches(".git"))
        .and_then(|rest| rest.split_once('/'))
        .filter(|(owner, repo)| !owner.is_empty() && !repo.is_empty() && !repo.contains('/'))
}

fn set_field(value: &mut Value, key: &str, field: Value) {
    if let Some(map) = value.as_object_mut() {
        map.insert(key.to_owned(), field);
    }
}

fn elements(value: Option<&Value>) -> &[Value] {
    value.and_then(Value::as_array).map_or(&[], Vec::as_slice)
}

fn setting_values(setting: &Value, kind: &str) -> Vec<String> {
    let value = setting
        .get("kind")
        .and_then(|kinds| kinds.get(kind))
        .and_then(|kind| kind.get("_0"));
    value
        .map(strings)
        .unwrap_or_default()
        .into_iter()
        .chain(value.and_then(Value::as_str).map(str::to_owned))
        .collect()
}

/// The defines and header paths that apply to every platform and configuration, by target name,
/// and the names of targets with a setting that applies to some platforms or configurations only.
///
/// # Errors
/// Fails when a target has no name.
fn target_settings(dumped: &Value) -> Result<(BTreeMap<String, Settings>, BTreeSet<String>), String> {
    let mut all = BTreeMap::new();
    let mut conditional = BTreeSet::new();
    for target in elements(dumped.get("targets")) {
        let name = target["name"].as_str().ok_or("a target has no name")?;
        let mut settings = Settings {
            include: target["publicHeadersPath"].as_str().unwrap_or("include").to_owned(),
            ..Settings::default()
        };
        for setting in elements(target.get("settings")) {
            let tool = setting["tool"].as_str().unwrap_or_default();
            if !matches!(tool, "swift" | "c" | "cxx") {
                continue;
            }
            let defines = setting_values(setting, "define");
            let paths = setting_values(setting, "headerSearchPath");
            if defines.is_empty() && paths.is_empty() {
                continue;
            }
            if !setting["condition"].is_null() {
                conditional.insert(name.to_owned());
                continue;
            }
            if tool == "swift" {
                settings.defines.extend(defines.into_iter().map(|define| define.split('=').next().unwrap_or_default().to_owned()));
            } else {
                settings.c_defines.extend(defines);
            }
            settings.header_paths.extend(paths);
        }
        settings.defines.sort();
        settings.defines.dedup();
        settings.c_defines.sort();
        settings.c_defines.dedup();
        all.insert(name.to_owned(), settings);
    }
    Ok((all, conditional))
}

/// The library products of a described package and the targets they need, by name. A product
/// whose targets include one that is not a Swift or C library, such as a macro, plugin or
/// executable target, or one named in `unsupported`, is left out.
///
/// # Errors
/// Fails when a product has no name or a target is not described.
pub fn library_targets(
    described: &Value,
    unsupported: &BTreeSet<String>,
) -> Result<(BTreeMap<String, Vec<String>>, BTreeMap<String, Value>), String> {
    let all: BTreeMap<&str, &Value> = described["targets"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|target| Some((target["name"].as_str()?, target)))
        .collect();
    let mut products = BTreeMap::new();
    let mut targets = BTreeMap::new();
    'products: for product in described["products"].as_array().into_iter().flatten() {
        if product["type"].get("library").is_none() {
            continue;
        }
        let name = product["name"].as_str().ok_or("a product has no name")?;
        let roots = strings(&product["targets"]);
        let mut pending = roots.clone();
        let mut seen = BTreeMap::new();
        while let Some(target_name) = pending.pop() {
            if seen.contains_key(&target_name) {
                continue;
            }
            let target = all.get(target_name.as_str()).ok_or(format!("target {target_name} is not described"))?;
            let module_type = target["module_type"].as_str().unwrap_or_default();
            if !matches!(module_type, "SwiftTarget" | "ClangTarget")
                || target["type"] != "library"
                || unsupported.contains(&target_name)
            {
                continue 'products;
            }
            pending.extend(strings(&target["target_dependencies"]));
            seen.insert(target_name, (*target).clone());
        }
        products.insert(name.to_owned(), roots);
        targets.extend(seen);
    }
    Ok((products, targets))
}

/// The package of `old` that has the name, version and archive URL of `pin`.
fn unchanged<'l>(old: &'l Lock, pin: &Pin) -> Option<&'l Package> {
    old.packages.iter().find(|package| {
        package.name == pin.identity
            && package.version == pin.version
            && matches!(
                package.archives.as_slice(),
                [archive] if archive.urls == [pin.url.as_str()] && archive.strip_prefix == pin.strip_prefix
            )
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
            let entry_settings = &entry.settings;
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
                        None => {
                            return Err(Error::new(format!(
                                "pin {identity}: product {product} is in no pin"
                            )));
                        }
                    };
                    product_deps.push(format!("{owner}:{product}"));
                    deps.push(owner.to_owned());
                }
                let path = field(target, "path").map_err(|error| Error::new(format!("pin {identity}: {error}")))?;
                let srcs: Vec<String> =
                    strings(&target["sources"]).iter().map(|source| format!("{path}/{source}")).collect();
                let mut entry = json!({
                    "deps": strings(&target["target_dependencies"]),
                    "module": field(target, "c99name").map_err(|error| Error::new(format!("pin {identity}: {error}")))?,
                    "path": path,
                    "product_deps": product_deps,
                    "srcs": srcs,
                });
                if target["module_type"] == "ClangTarget" {
                    set_field(&mut entry, "kind", json!("clang"));
                }
                if let Some(settings) = entry_settings.get(name) {
                    if target["module_type"] == "ClangTarget" {
                        set_field(
                            &mut entry,
                            "include",
                            json!(format!("{path}/{}", settings.include)),
                        );
                    }
                    let defines = if target["module_type"] == "ClangTarget" { &settings.c_defines } else { &settings.defines };
                    if !defines.is_empty() {
                        set_field(&mut entry, "defines", json!(defines));
                    }
                    if !settings.header_paths.is_empty() {
                        set_field(
                            &mut entry,
                            "header_paths",
                            json!(
                                settings
                                    .header_paths
                                    .iter()
                                    .map(|header| format!("{path}/{header}"))
                                    .collect::<Vec<_>>()
                            ),
                        );
                    }
                }
                targets.insert(name.clone(), entry);
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
                    kind: String::new(),
                    platform: String::new(),
                }],
                deps,
                provides,
                dev: false,
                extra,
            });
        }
        packages.extend(kept);
        Ok(Resolution {
            groups: std::collections::BTreeMap::new(),
            packages,
            ..Resolution::default()
        })
    }
}
