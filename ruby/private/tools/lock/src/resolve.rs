//! Resolves a Bundler gem set from its manifest through the rubygems.org compact index and writes its lock and Bundler state file.
#![deny(clippy::indexing_slicing, clippy::string_slice)]

use dx_lock::{Archive, Error, HttpFetch, Lock, Package, Resolution, Resolver, Scratch, Spec};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

/// The gem host.
pub const BASE: &str = "https://rubygems.org";

const PINS: &str = include_str!("../../../../pins.json");

/// The version of the pinned Ruby toolchain.
#[must_use]
pub fn pinned_ruby() -> String {
    serde_json::from_str::<serde_json::Value>(PINS)
        .ok()
        .and_then(|pins| {
            pins.pointer("/portable_ruby/version")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_default()
}

/// Downloads index answers and gems.
pub trait Download {
    /// The bytes at `url`.
    ///
    /// # Errors
    /// Fails when the download does.
    fn bytes(&self, url: &str) -> Result<Vec<u8>, Error>;
}

/// Downloads over HTTPS.
pub struct Registry(HttpFetch);

impl Registry {
    /// A downloader that refuses plain HTTP.
    #[must_use]
    pub fn new() -> Self {
        Self(HttpFetch::new())
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

impl Download for Registry {
    fn bytes(&self, url: &str) -> Result<Vec<u8>, Error> {
        self.0.bytes(url)
    }
}

/// The compact index URL of a gem.
#[must_use]
pub fn info_url(name: &str) -> String {
    format!("{BASE}/info/{name}")
}

/// The URL of a gem file.
#[must_use]
pub fn gem_url(name: &str, version: &str) -> String {
    format!("{BASE}/gems/{name}-{version}.gem")
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Segment {
    Number(u64),
    Text(String),
}

/// A `Gem::Version`.
#[derive(Clone, Debug)]
struct Version(Vec<Segment>);

impl Version {
    fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        if text.is_empty() || !text.chars().all(|c| c.is_ascii_alphanumeric() || c == '.') {
            return None;
        }
        let mut segments = Vec::new();
        let mut run = String::new();
        let flush = |run: &mut String, segments: &mut Vec<Segment>| -> Option<()> {
            if run.is_empty() {
                return Some(());
            }
            let segment = if run.bytes().all(|byte| byte.is_ascii_digit()) {
                Segment::Number(run.parse().ok()?)
            } else {
                Segment::Text(run.clone())
            };
            segments.push(segment);
            run.clear();
            Some(())
        };
        for c in text.chars() {
            let switch = run
                .chars()
                .last()
                .is_some_and(|last| last.is_ascii_digit() != c.is_ascii_digit());
            if c == '.' || switch {
                flush(&mut run, &mut segments)?;
            }
            if c != '.' {
                run.push(c);
            }
        }
        flush(&mut run, &mut segments)?;
        Some(Self(segments))
    }

    fn prerelease(&self) -> bool {
        self.0
            .iter()
            .any(|segment| matches!(segment, Segment::Text(_)))
    }

    fn bump(&self) -> Self {
        let mut release: Vec<u64> = self
            .0
            .iter()
            .map_while(|segment| match segment {
                Segment::Number(number) => Some(*number),
                Segment::Text(_) => None,
            })
            .collect();
        if release.len() > 1 {
            release.pop();
        }
        if let Some(last) = release.last_mut() {
            *last += 1;
        }
        Self(release.into_iter().map(Segment::Number).collect())
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        let zero = Segment::Number(0);
        for at in 0..self.0.len().max(other.0.len()) {
            let left = self.0.get(at).unwrap_or(&zero);
            let right = other.0.get(at).unwrap_or(&zero);
            let order = match (left, right) {
                (Segment::Number(a), Segment::Number(b)) => a.cmp(b),
                (Segment::Text(a), Segment::Text(b)) => a.cmp(b),
                (Segment::Text(_), Segment::Number(_)) => Ordering::Less,
                (Segment::Number(_), Segment::Text(_)) => Ordering::Greater,
            };
            if order != Ordering::Equal {
                return order;
            }
        }
        Ordering::Equal
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Version {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Version {}

/// One `Gem::Requirement` clause such as `~> 2.4.1`.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Requirement {
    op: String,
    version: String,
}

impl Requirement {
    /// Parses `<op> <version>`, where a bare version means `=`.
    ///
    /// # Errors
    /// Fails on an unknown operator or a version that is not a gem version.
    pub fn parse(text: &str) -> Result<Self, Error> {
        let text = text.trim();
        let ops = ["~>", ">=", "<=", "!=", ">", "<", "="];
        let (op, rest) = ops
            .iter()
            .find_map(|op| text.strip_prefix(op).map(|rest| (*op, rest)))
            .unwrap_or(("=", text));
        let version = rest.trim();
        if Version::parse(version).is_none() {
            return Err(Error::new(format!("{text:?} is not a gem requirement")));
        }
        Ok(Self {
            op: op.to_owned(),
            version: version.to_owned(),
        })
    }

    fn allows(&self, version: &Version) -> bool {
        let Some(base) = Version::parse(&self.version) else {
            return false;
        };
        match self.op.as_str() {
            "~>" => *version >= base && *version < base.bump(),
            ">=" => *version >= base,
            "<=" => *version <= base,
            "!=" => *version != base,
            ">" => *version > base,
            "<" => *version < base,
            _ => *version == base,
        }
    }

    fn names_prerelease(&self) -> bool {
        Version::parse(&self.version).is_some_and(|version| version.prerelease())
    }

    fn is_any(&self) -> bool {
        self.op == ">=" && Version::parse(&self.version) == Version::parse("0")
    }

    fn text(&self) -> String {
        format!("{} {}", self.op, self.version)
    }
}

/// The lockfile text of a requirement list: empty for none, else ` (<clauses>)`.
fn requirement_suffix(requirements: &[Requirement]) -> String {
    let mut clauses: Vec<String> = requirements
        .iter()
        .filter(|requirement| !requirement.is_any())
        .map(Requirement::text)
        .collect();
    if clauses.is_empty() {
        return String::new();
    }
    clauses.sort();
    clauses.reverse();
    clauses.dedup();
    format!(" ({})", clauses.join(", "))
}

/// One `gem` line of a Bundler manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Wanted {
    /// The requirement clauses.
    pub requirements: Vec<Requirement>,
    /// Whether every group of the line is `development` or `test`.
    pub dev: bool,
}

fn quoted(text: &str) -> Option<&str> {
    let text = text.trim();
    let inner = text
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .or_else(|| {
            text.strip_prefix('\'')
                .and_then(|rest| rest.strip_suffix('\''))
        })?;
    (!inner.contains(['"', '\''])).then_some(inner)
}

fn symbols(text: &str) -> Vec<String> {
    text.split(',')
        .map(|item| {
            item.trim()
                .trim_start_matches('[')
                .trim_end_matches(']')
                .trim()
                .trim_start_matches(':')
                .trim_matches(['"', '\''])
                .to_owned()
        })
        .filter(|item| !item.is_empty())
        .collect()
}

fn is_dev(groups: &[String]) -> bool {
    !groups.is_empty()
        && groups
            .iter()
            .all(|group| group == "development" || group == "test")
}

/// The `gem` lines of a Bundler manifest, by gem name.
///
/// # Errors
/// Fails on a source other than rubygems.org, on a git, path or gemspec dependency, and on a line it does not read.
pub fn parse_gemfile(text: &str) -> Result<BTreeMap<String, Wanted>, Error> {
    let mut gems = BTreeMap::new();
    let mut groups: Vec<Vec<String>> = Vec::new();
    for (number, line) in text.lines().enumerate() {
        let failed = |message: &str| Error::new(format!("line {}: {message}", number + 1));
        let line = line.split_once('#').map_or(line, |(code, _)| code).trim();
        if line.is_empty() {
            continue;
        }
        if line == "end" {
            if groups.pop().is_none() {
                return Err(failed("end without a group"));
            }
            continue;
        }
        let (word, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        match word {
            "source" => {
                let url = quoted(rest).ok_or_else(|| failed("source is not a quoted URL"))?;
                if url.trim_end_matches('/') != BASE {
                    return Err(failed(&format!("source {url} is not {BASE}")));
                }
            }
            "ruby" => {
                let pinned = pinned_ruby();
                let version = Version::parse(&pinned).ok_or_else(|| {
                    failed(&format!("the pinned Ruby {pinned:?} is not a version"))
                })?;
                for arg in rest.split(',') {
                    let Some(clause) = quoted(arg) else { continue };
                    let requirement =
                        Requirement::parse(clause).map_err(|error| failed(&error.to_string()))?;
                    if !requirement.allows(&version) {
                        return Err(failed(&format!(
                            "ruby {} does not allow the pinned Ruby {pinned}",
                            requirement.text()
                        )));
                    }
                }
            }
            "group" => {
                let names = rest
                    .strip_suffix("do")
                    .ok_or_else(|| failed("group without do"))?;
                groups.push(symbols(names));
            }
            "gem" => {
                let mut args = rest.split(',');
                let name = args
                    .next()
                    .and_then(quoted)
                    .ok_or_else(|| failed("gem has no quoted name"))?;
                let mut requirements = Vec::new();
                let mut own_groups = Vec::new();
                let mut rest = args.peekable();
                while let Some(arg) = rest.next() {
                    if let Some(clause) = quoted(arg) {
                        requirements.push(
                            Requirement::parse(clause)
                                .map_err(|error| failed(&error.to_string()))?,
                        );
                        continue;
                    }
                    let arg = arg.trim().trim_start_matches(':');
                    let (key, value) = arg
                        .split_once("=>")
                        .or_else(|| arg.split_once(':'))
                        .ok_or_else(|| failed(&format!("gem {name} has the argument {arg:?}")))?;
                    let key = key.trim();
                    match key {
                        "group" | "groups" => {
                            let mut value = value.to_owned();
                            if value.contains('[') {
                                while !value.contains(']') {
                                    let Some(next) = rest.next() else { break };
                                    value.push(',');
                                    value.push_str(next);
                                }
                            }
                            own_groups.extend(symbols(&value));
                        }
                        "require" | "platforms" | "platform" => {}
                        _ => {
                            return Err(failed(&format!(
                                "gem {name} uses {key}:, which is not supported; only rubygems.org gems can be locked"
                            )));
                        }
                    }
                }
                if requirements.is_empty() {
                    requirements.push(Requirement::parse(">= 0")?);
                }
                let all: Vec<String> = groups
                    .iter()
                    .flatten()
                    .chain(&own_groups)
                    .cloned()
                    .collect();
                gems.insert(
                    name.to_owned(),
                    Wanted {
                        requirements,
                        dev: is_dev(&all),
                    },
                );
            }
            _ => {
                return Err(failed(&format!(
                    "{word} is not supported; only source, ruby, group and gem lines can be locked"
                )));
            }
        }
    }
    if !groups.is_empty() {
        return Err(Error::new("a group has no end"));
    }
    Ok(gems)
}

/// One version of a gem in the compact index.
#[derive(Clone, Debug)]
struct Indexed {
    version: Version,
    text: String,
    deps: BTreeMap<String, Vec<Requirement>>,
    sha256: String,
    ruby: Vec<Requirement>,
    platform: Option<String>,
}

fn parse_info(name: &str, text: &str) -> Result<Vec<Indexed>, Error> {
    let failed = |line: &str| Error::new(format!("{}: unreadable line {line:?}", info_url(name)));
    let mut versions = Vec::new();
    for line in text.lines().skip_while(|line| *line != "---").skip(1) {
        let (head, tail) = line.split_once('|').ok_or_else(|| failed(line))?;
        let (full, deps) = head.split_once(' ').unwrap_or((head, ""));
        let (version, platform) = match full.split_once('-') {
            Some((version, platform)) => (version, Some(platform.to_owned())),
            None => (full, None),
        };
        let Some(parsed) = Version::parse(version) else {
            continue;
        };
        let mut needs = BTreeMap::new();
        for dep in deps.trim().split(',').filter(|dep| !dep.is_empty()) {
            let (dep, clauses) = dep.split_once(':').ok_or_else(|| failed(line))?;
            let clauses = clauses
                .split('&')
                .map(Requirement::parse)
                .collect::<Result<Vec<_>, _>>()?;
            needs.insert(dep.to_owned(), clauses);
        }
        let field = |key: &str| {
            tail.split(',')
                .find_map(|field| field.strip_prefix(key))
                .unwrap_or_default()
        };
        let sha256 = field("checksum:").to_owned();
        let ruby = field("ruby:")
            .split('&')
            .filter(|clause| !clause.trim().is_empty())
            .map(Requirement::parse)
            .collect::<Result<Vec<_>, _>>()?;
        versions.push(Indexed {
            version: parsed,
            text: version.to_owned(),
            deps: needs,
            sha256,
            ruby,
            platform,
        });
    }
    Ok(versions)
}

/// The most bytes read from one gem member.
const MEMBER_LIMIT: u64 = 64 << 20;

const GEM_LIMITS: dx_archive::Limits = dx_archive::Limits {
    max_entries: 1 << 16,
    max_entry_bytes: MEMBER_LIMIT,
    max_total_bytes: 256 << 20,
};

/// The import roots under the require paths of a gem file.
fn gem_provides(name: &str, bytes: &[u8]) -> Result<BTreeSet<String>, String> {
    let mut metadata = None;
    let mut data = None;
    dx_archive::visit(
        std::io::Cursor::new(bytes),
        dx_archive::Format::Tar,
        GEM_LIMITS,
        |path, _, body| {
            match path {
                "metadata.gz" => {
                    let text = dx_archive::gunzip(body, MEMBER_LIMIT)
                        .map_err(|error| std::io::Error::other(format!("metadata.gz: {error}")))?;
                    let text = String::from_utf8(text)
                        .map_err(|error| std::io::Error::other(format!("metadata.gz: {error}")))?;
                    metadata = Some(text);
                }
                "data.tar.gz" => {
                    let mut raw = Vec::new();
                    body.read_to_end(&mut raw)?;
                    data = Some(raw);
                }
                _ => {}
            }
            Ok(())
        },
    )
    .map_err(|error| error.to_string())?;
    let metadata = metadata.ok_or("has no metadata.gz")?;
    let data = data.ok_or("has no data.tar.gz")?;
    let mut require_paths = Vec::new();
    let mut lines = metadata
        .lines()
        .skip_while(|line| *line != "require_paths:");
    lines.next();
    for line in lines {
        let Some(item) = line.strip_prefix("- ") else {
            break;
        };
        require_paths.push(
            item.trim()
                .trim_matches(['"', '\''])
                .trim_end_matches('/')
                .to_owned(),
        );
    }
    if require_paths.is_empty() {
        require_paths.push("lib".to_owned());
    }
    let mut relative = Vec::new();
    dx_archive::visit(
        std::io::Cursor::new(data),
        dx_archive::Format::TarGz,
        GEM_LIMITS,
        |path, _, _| {
            for dir in &require_paths {
                if let Some(rest) = path.strip_prefix(&format!("{dir}/")) {
                    relative.push(rest.to_owned());
                }
            }
            Ok(())
        },
    )
    .map_err(|error| format!("data.tar.gz: {error}"))?;
    let files: BTreeSet<&str> = relative
        .iter()
        .filter_map(|rest| rest.strip_suffix(".rb"))
        .collect();
    let mut provides = BTreeSet::new();
    for rest in &relative {
        let root = match rest.split_once('/') {
            None => rest.strip_suffix(".rb"),
            Some((top, _)) if files.contains(top) => Some(top),
            Some((top, tail)) => match tail.split_once('/') {
                Some((sub, _)) => Some(rest.get(..top.len() + 1 + sub.len()).unwrap_or(top)),
                None => tail
                    .strip_suffix(".rb")
                    .map(|sub| rest.get(..top.len() + 1 + sub.len()).unwrap_or(top)),
            },
        };
        if let Some(root) = root.filter(|root| !root.is_empty()) {
            provides.insert(root.to_owned());
        }
    }
    if provides.is_empty() {
        provides.insert(name.to_owned());
    }
    Ok(provides)
}

/// The versions and `BUNDLED WITH` value of a previous Bundler state file.
fn previous_state(text: &str) -> (BTreeMap<String, String>, Option<String>) {
    let mut versions = BTreeMap::new();
    let mut bundled = None;
    let mut section = "";
    for line in text.lines() {
        if !line.starts_with(' ') {
            section = line.trim();
            continue;
        }
        if section == "BUNDLED WITH" {
            bundled = Some(line.trim().to_owned());
            continue;
        }
        if section != "GEM" || line.starts_with("      ") || !line.starts_with("    ") {
            continue;
        }
        if let Some((name, version)) = line
            .trim()
            .strip_suffix(')')
            .and_then(|line| line.split_once(" ("))
            && !version.contains('-')
        {
            versions.insert(name.to_owned(), version.to_owned());
        }
    }
    (versions, bundled)
}

/// The resolver of the ruby ecosystem.
pub struct Ruby<'a> {
    manifest: String,
    state: String,
    ruby: String,
    download: &'a dyn Download,
}

const ROUNDS: usize = 64;

impl<'a> Ruby<'a> {
    /// A resolver that downloads with `download`.
    #[must_use]
    pub fn new(spec: &Spec, download: &'a dyn Download) -> Self {
        Self {
            manifest: spec.manifest.first().cloned().unwrap_or_default(),
            state: spec.state.first().cloned().unwrap_or_default(),
            ruby: pinned_ruby(),
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

    fn index(
        &self,
        name: &str,
        listed: &mut BTreeMap<String, Vec<Indexed>>,
    ) -> Result<Vec<Indexed>, Error> {
        if let Some(found) = listed.get(name) {
            return Ok(found.clone());
        }
        let url = info_url(name);
        let body = self
            .download
            .bytes(&url)
            .map_err(|error| Error::new(format!("{url}: {error}")))?;
        let text =
            String::from_utf8(body).map_err(|error| Error::new(format!("{url}: {error}")))?;
        let found = parse_info(name, &text)?;
        listed.insert(name.to_owned(), found.clone());
        Ok(found)
    }

    fn pick(
        &self,
        name: &str,
        requirements: &[Requirement],
        selected: Option<&str>,
        listed: &mut BTreeMap<String, Vec<Indexed>>,
    ) -> Result<Indexed, Error> {
        let ruby = Version::parse(&self.ruby).ok_or_else(|| {
            Error::new(format!("the pinned Ruby {:?} is not a version", self.ruby))
        })?;
        let prerelease = requirements.iter().any(Requirement::names_prerelease);
        let matches = |entry: &Indexed| {
            (prerelease || !entry.version.prerelease())
                && requirements
                    .iter()
                    .all(|requirement| requirement.allows(&entry.version))
        };
        let runs = |entry: &Indexed| entry.ruby.iter().all(|clause| clause.allows(&ruby));
        let allowed = |entry: &Indexed| entry.platform.is_none() && matches(entry) && runs(entry);
        let versions = self.index(name, listed)?;
        let selected = selected.and_then(|text| {
            versions
                .iter()
                .find(|entry| entry.text == text && allowed(entry))
        });
        if let Some(found) = selected.or_else(|| {
            versions
                .iter()
                .filter(|entry| allowed(entry))
                .max_by(|a, b| a.version.cmp(&b.version))
        }) {
            return Ok(found.clone());
        }
        let wanted: Vec<String> = requirements.iter().map(Requirement::text).collect();
        let wanted = wanted.join(" and ");
        let newest = |entries: &mut dyn Iterator<Item = &Indexed>| {
            entries.max_by(|a, b| a.version.cmp(&b.version)).cloned()
        };
        if let Some(found) = newest(
            &mut versions
                .iter()
                .filter(|entry| entry.platform.is_none() && matches(entry)),
        ) {
            let needs: Vec<String> = found.ruby.iter().map(Requirement::text).collect();
            return Err(Error::new(format!(
                "no version of {name} satisfies {wanted} on the pinned Ruby {}; {name} {} needs ruby {}",
                self.ruby,
                found.text,
                needs.join(" and ")
            )));
        }
        if let Some(found) = newest(&mut versions.iter().filter(|entry| matches(entry))) {
            let platforms: BTreeSet<&str> = versions
                .iter()
                .filter(|entry| entry.text == found.text)
                .filter_map(|entry| entry.platform.as_deref())
                .collect();
            return Err(Error::new(format!(
                "no version of {name} satisfies {wanted} on the ruby platform; {name} {} ships only platform builds ({}), and the gem hub builds every gem from its ruby platform source",
                found.text,
                platforms.into_iter().collect::<Vec<_>>().join(", ")
            )));
        }
        Err(Error::new(format!(
            "no version of {name} satisfies {wanted}"
        )))
    }

    fn describe(&self, name: &str, found: &Indexed, old: Option<&Lock>) -> Result<Package, Error> {
        let url = gem_url(name, &found.text);
        if let Some(package) = old.and_then(|lock| {
            lock.packages.iter().find(|package| {
                package.name == name
                    && package.version == found.text
                    && matches!(package.archives.as_slice(), [archive] if archive.urls == [url.clone()] && archive.sha256 == found.sha256)
            })
        }) {
            return Ok(Package {
                extra: gem_deps(found),
                ..package.clone()
            });
        }
        let failed = |message: String| Error::new(format!("gem {name} {}: {message}", found.text));
        let bytes = self
            .download
            .bytes(&url)
            .map_err(|error| failed(format!("{url}: {error}")))?;
        let digest = dx_digest::sha256_hex(&bytes);
        if !found.sha256.is_empty() && digest != found.sha256 {
            return Err(failed(format!(
                "{url} has the sha256 {digest}, the index states {}",
                found.sha256
            )));
        }
        let provides =
            gem_provides(name, &bytes).map_err(|error| failed(format!("{url}: {error}")))?;
        Ok(Package {
            name: name.to_owned(),
            version: found.text.clone(),
            archives: vec![Archive {
                urls: vec![url],
                sha256: digest,
                ..Archive::default()
            }],
            deps: found.deps.keys().cloned().collect(),
            provides: provides.into_iter().collect(),
            dev: false,
            extra: gem_deps(found),
        })
    }
}

/// The `gem_deps` extra of a gem: each dependency mapped to its requirement clauses.
fn gem_deps(found: &Indexed) -> BTreeMap<String, serde_json::Value> {
    if found.deps.is_empty() {
        return BTreeMap::new();
    }
    let deps = found
        .deps
        .iter()
        .map(|(dep, requirements)| {
            let suffix = requirement_suffix(requirements);
            let clauses = suffix
                .strip_prefix(" (")
                .and_then(|rest| rest.strip_suffix(')'))
                .unwrap_or("");
            (dep.clone(), serde_json::Value::from(clauses))
        })
        .collect();
    BTreeMap::from([("gem_deps".to_owned(), serde_json::Value::Object(deps))])
}

fn state_text(
    root: &BTreeMap<String, Wanted>,
    chosen: &BTreeMap<String, Indexed>,
    bundled: Option<&str>,
) -> String {
    let mut text = format!("GEM\n  remote: {BASE}/\n  specs:\n");
    for (name, found) in chosen {
        text.push_str(&format!("    {name} ({})\n", found.text));
        for (dep, requirements) in &found.deps {
            text.push_str(&format!(
                "      {dep}{}\n",
                requirement_suffix(requirements)
            ));
        }
    }
    text.push_str("\nPLATFORMS\n  ruby\n\nDEPENDENCIES\n");
    for (name, wanted) in root {
        text.push_str(&format!(
            "  {name}{}\n",
            requirement_suffix(&wanted.requirements)
        ));
    }
    text.push_str("\nCHECKSUMS\n");
    for (name, found) in chosen {
        text.push_str(&format!(
            "  {name} ({}) sha256={}\n",
            found.text, found.sha256
        ));
    }
    if let Some(bundled) = bundled {
        text.push_str(&format!("\nBUNDLED WITH\n   {bundled}\n"));
    }
    text
}

impl Resolver for Ruby<'_> {
    fn resolve(&self, scratch: &Scratch, old: Option<&Lock>) -> Result<Resolution, Error> {
        let read = |name: &str| std::fs::read_to_string(scratch.set().join(name));
        let manifest = read(&self.manifest)
            .map_err(|error| Error::new(format!("{}: {error}", self.manifest)))?;
        let root = parse_gemfile(&manifest)
            .map_err(|error| Error::new(format!("{}: {error}", self.manifest)))?;
        let (previous, bundled) = read(&self.state)
            .map(|text| previous_state(&text))
            .unwrap_or_default();
        let mut listed = BTreeMap::new();
        let mut chosen: BTreeMap<String, Indexed> = BTreeMap::new();
        let mut settled = false;
        for _ in 0..ROUNDS {
            let mut wanted: BTreeMap<String, Vec<Requirement>> = root
                .iter()
                .map(|(name, entry)| (name.clone(), entry.requirements.clone()))
                .collect();
            let mut queue: Vec<String> = root.keys().cloned().collect();
            let mut seen = BTreeSet::new();
            while let Some(name) = queue.pop() {
                if !seen.insert(name.clone()) {
                    continue;
                }
                let Some(found) = chosen.get(&name) else {
                    continue;
                };
                for (dep, requirements) in &found.deps {
                    let entry = wanted.entry(dep.clone()).or_default();
                    for requirement in requirements {
                        if !entry.contains(requirement) {
                            entry.push(requirement.clone());
                        }
                    }
                    queue.push(dep.clone());
                }
            }
            let mut next = BTreeMap::new();
            for (name, requirements) in &wanted {
                let selected = chosen
                    .get(name)
                    .map(|found| found.text.as_str())
                    .or_else(|| previous.get(name).map(String::as_str));
                next.insert(
                    name.clone(),
                    self.pick(name, requirements, selected, &mut listed)?,
                );
            }
            let same = next.len() == chosen.len()
                && next.iter().all(|(name, found)| {
                    chosen.get(name).is_some_and(|was| was.text == found.text)
                });
            chosen = next;
            if same {
                settled = true;
                break;
            }
        }
        if !settled {
            return Err(Error::new(format!(
                "{} has no resolution after {ROUNDS} rounds",
                self.manifest
            )));
        }
        let mut packages = BTreeMap::new();
        for (name, found) in &mut chosen {
            let package = self.describe(name, found, old)?;
            if let Some(archive) = package.archives.first() {
                found.sha256.clone_from(&archive.sha256);
            }
            packages.insert(name.clone(), package);
        }
        let mut runtime = BTreeSet::new();
        let mut queue: Vec<String> = root
            .iter()
            .filter(|(_, wanted)| !wanted.dev)
            .map(|(name, _)| name.clone())
            .collect();
        while let Some(name) = queue.pop() {
            if runtime.insert(name.clone())
                && let Some(package) = packages.get(&name)
            {
                queue.extend(package.deps.iter().cloned());
            }
        }
        for (name, package) in &mut packages {
            package.dev = !runtime.contains(name);
        }
        std::fs::write(
            scratch.set().join(&self.state),
            state_text(&root, &chosen, bundled.as_deref()),
        )
        .map_err(|error| Error::new(format!("{}: {error}", self.state)))?;
        Ok(Resolution {
            groups: BTreeMap::new(),
            packages: packages.into_values().collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allows(requirement: &str, version: &str) -> bool {
        Requirement::parse(requirement)
            .expect("requirement")
            .allows(&Version::parse(version).expect("version"))
    }

    #[test]
    fn pessimistic_requirements_bump_the_second_to_last_segment() {
        assert!(allows("~> 2.4.1", "2.4.9"));
        assert!(!allows("~> 2.4.1", "2.5.0"));
        assert!(allows("~> 2.3", "2.9"));
        assert!(!allows("~> 2.3", "3.0"));
        assert!(allows("~> 3", "3.9"));
        assert!(!allows("~> 3", "4.0"));
    }

    #[test]
    fn prereleases_sort_before_their_release() {
        assert!(Version::parse("1.0.0.rc1") < Version::parse("1.0.0"));
        assert!(Version::parse("1.0") == Version::parse("1.0.0"));
        assert!(Version::parse("1.10") > Version::parse("1.9"));
    }

    #[test]
    fn lockfile_clauses_are_sorted_like_bundler() {
        let requirements = vec![
            Requirement::parse("< 2.0").expect("clause"),
            Requirement::parse(">= 1.2.0").expect("clause"),
        ];
        assert_eq!(requirement_suffix(&requirements), " (>= 1.2.0, < 2.0)");
        assert_eq!(
            requirement_suffix(&[Requirement::parse(">= 0").expect("clause")]),
            ""
        );
    }
}
