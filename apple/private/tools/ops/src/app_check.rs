use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::util::{Failure, Outcome, reject, unzip, usage};
use crate::xcode::{CODESIGN, PLUTIL, Tools, components};

pub const PREFIX: &str = "apple_macos_app";

const LOAD_COMMANDS: [&str; 4] = [
    "LC_LOAD_DYLIB",
    "LC_LOAD_WEAK_DYLIB",
    "LC_REEXPORT_DYLIB",
    "LC_LOAD_UPWARD_DYLIB",
];

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Loads {
    pub platforms: BTreeSet<String>,
    pub minos: Vec<String>,
    pub rpaths: Vec<String>,
    pub loads: Vec<String>,
}

#[must_use]
pub fn parse_loads(text: &str) -> Loads {
    let mut found = Loads::default();
    let mut command = "";
    for line in text.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        let [first, second, ..] = fields[..] else {
            continue;
        };
        if first == "cmd" {
            command = second;
        }
        match first {
            "platform" => {
                found.platforms.insert(second.to_owned());
            }
            "minos" => found.minos.push(second.to_owned()),
            "path" if command == "LC_RPATH" => found.rpaths.push(second.to_owned()),
            "name" if LOAD_COMMANDS.contains(&command) => found.loads.push(second.to_owned()),
            _ => {}
        }
    }
    found
}

fn newest(versions: &[String]) -> &str {
    versions.iter().fold("", |best, version| {
        if best.is_empty() || components(version) > components(best) {
            version
        } else {
            best
        }
    })
}

fn quote(text: &str) -> String {
    serde_json::Value::from(text).to_string()
}

fn array<'a>(items: impl IntoIterator<Item = &'a str>) -> String {
    let quoted: Vec<String> = items
        .into_iter()
        .filter(|item| !item.is_empty())
        .map(quote)
        .collect();
    format!("[{}]", quoted.join(", "))
}

fn names(list: &str) -> Vec<&str> {
    list.lines().filter(|item| !item.is_empty()).collect()
}

fn files_below(dir: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => files_below(&path, found),
            Ok(kind) if kind.is_file() => found.push(path),
            _ => {}
        }
    }
}

struct Check<'a> {
    tools: &'a Tools<'a>,
    arch: &'a str,
    deployment: &'a str,
    name: String,
    root: PathBuf,
    contents: PathBuf,
}

impl Check<'_> {
    fn value(&self, key: &str, plist: &Path) -> Option<String> {
        let out = self.tools.tool(
            PLUTIL,
            &["-extract", key, "raw", "-o", "-", &plist.to_string_lossy()],
        );
        out.success()
            .then(|| out.stdout.trim_end_matches('\n').to_owned())
    }

    fn expect(&self, key: &str, plist: &Path, want: &str) -> Result<(), Failure> {
        let got = self.value(key, plist).unwrap_or_default();
        if got == want {
            Ok(())
        } else {
            Err(reject(format!(
                "{} has {key} '{got}', want '{want}'",
                self.name
            )))
        }
    }

    fn framework(&self, framework: &str) -> Result<(), Failure> {
        let bundle = self
            .contents
            .join(format!("Frameworks/{framework}.framework"));
        if !bundle.is_dir() {
            return Err(reject(format!(
                "declared framework {framework}.framework is missing from the bundle"
            )));
        }
        let mut home = None;
        for base in [bundle.join("Versions/Current"), bundle.clone()] {
            for candidate in [base.join("Resources/Info.plist"), base.join("Info.plist")] {
                if home.is_none() && candidate.is_file() {
                    home = Some((candidate, base.clone()));
                }
            }
        }
        let Some((plist, base)) = home else {
            return Err(reject(format!(
                "framework {framework}.framework has no Info.plist"
            )));
        };
        let inner = self.value("CFBundleExecutable", &plist).unwrap_or_default();
        if inner.is_empty() || !base.join(&inner).is_file() {
            return Err(reject(format!(
                "framework {framework}.framework has no executable '{inner}'"
            )));
        }
        Ok(())
    }

    fn mach_o(
        &self,
        file: &Path,
        rpaths: &mut BTreeSet<String>,
        loads: &mut BTreeSet<String>,
    ) -> Result<Option<String>, Failure> {
        let path = file.to_string_lossy();
        let rel = format!(
            "{}/{}",
            self.name,
            file.strip_prefix(&self.root).unwrap_or(file).display()
        );
        let lipo = self.tools.xcrun(&["lipo", "-archs", &path]);
        if !lipo.success() {
            return Ok(None);
        }
        let archs = lipo.stdout.trim_end_matches('\n');
        if archs != self.arch {
            return Err(reject(format!(
                "{rel} carries '{archs}', want {}",
                self.arch
            )));
        }
        let otool = self
            .tools
            .xcrun(&["otool", "-arch", self.arch, "-l", &path]);
        let found = parse_loads(if otool.success() { &otool.stdout } else { "" });
        let platforms: Vec<&str> = found.platforms.iter().map(String::as_str).collect();
        if platforms != ["1"] {
            return Err(reject(format!(
                "{rel} declares build platform '{}', want 1 for macosx",
                platforms.join(" ")
            )));
        }
        let minos = newest(&found.minos);
        if minos.is_empty() {
            return Err(reject(format!("{rel} declares no minimum macOS version")));
        }
        if components(minos) > components(self.deployment) {
            return Err(reject(format!(
                "{rel} needs macOS {minos}, above the app deployment {}",
                self.deployment
            )));
        }
        for rpath in &found.rpaths {
            if rpath.starts_with('/') && !system_path(rpath) {
                return Err(reject(format!(
                    "{rel} has absolute rpath {rpath}; use @executable_path or @loader_path"
                )));
            }
            rpaths.insert(rpath.clone());
        }
        for load in &found.loads {
            let embedded = if system_path(load) {
                None
            } else if let Some(rest) = load.strip_prefix("@rpath/") {
                Some(self.contents.join("Frameworks").join(rest))
            } else if let Some(rest) = load.strip_prefix("@executable_path/") {
                Some(self.contents.join("MacOS").join(rest))
            } else if let Some(rest) = load.strip_prefix("@loader_path/") {
                Some(file.parent().unwrap_or(file).join(rest))
            } else {
                return Err(reject(format!(
                    "{rel} loads host runtime {load}; embed it under Frameworks"
                )));
            };
            if embedded.is_some_and(|path| !path.exists()) {
                return Err(reject(format!(
                    "{rel} loads {load}, which is missing from the bundle"
                )));
            }
            loads.insert(load.clone());
        }
        Ok(Some(minos.to_owned()))
    }
}

fn system_path(path: &str) -> bool {
    path == "/usr/lib"
        || path == "/System/Library"
        || path.starts_with("/usr/lib/")
        || path.starts_with("/System/Library/")
}

fn verify(tools: &Tools<'_>, args: &[String], scratch: &Path) -> Result<(), Failure> {
    let [
        identity,
        out,
        archive,
        name,
        extension,
        executable,
        bundle_id,
        version,
        deployment,
        arch,
        xcode,
        sdk_version,
        frameworks,
        plugins,
        resources,
    ] = args
    else {
        return Err(usage(
            "usage: apple_ops macos-app-check <identity> <zip> <archive> <name> <extension> <executable> <bundle-id> <version> <deployment> <arch> <xcode> <sdk-version> <frameworks> <plugins> <resources>",
        ));
    };
    let tree = scratch.join("tree");
    std::fs::create_dir_all(&tree)
        .map_err(|error| usage(format!("cannot create {}: {error}", tree.display())))?;
    let archive_path = Path::new(archive);
    unzip(archive_path, &tree).map_err(|error| {
        let error = error.to_string();
        let last = error.lines().last().unwrap_or_default();
        reject(format!(
            "{} does not unpack: {last}",
            archive_path
                .file_name()
                .map_or_else(String::new, |base| base.to_string_lossy().into_owned())
        ))
    })?;
    let bundle = format!("{name}{extension}");
    let root = tree.join(&bundle);
    let contents = root.join("Contents");
    if !contents.is_dir() {
        return Err(reject(format!("{bundle} has no Contents directory")));
    }
    let plist = contents.join("Info.plist");
    if !plist.is_file() {
        return Err(reject(format!("{bundle} has no Contents/Info.plist")));
    }
    let check = Check {
        tools,
        arch,
        deployment,
        name: bundle.clone(),
        root: root.clone(),
        contents: contents.clone(),
    };
    check.expect("CFBundleIdentifier", &plist, bundle_id)?;
    check.expect("CFBundleExecutable", &plist, executable)?;
    check.expect("CFBundleShortVersionString", &plist, version)?;
    check.expect("CFBundleVersion", &plist, version)?;
    check.expect("LSMinimumSystemVersion", &plist, deployment)?;
    check.expect("CFBundlePackageType", &plist, "APPL")?;
    check.expect("CFBundleSupportedPlatforms.0", &plist, "MacOSX")?;
    let main = contents.join("MacOS").join(executable);
    if !main.is_file() {
        return Err(reject(format!(
            "{bundle} has no Contents/MacOS/{executable}"
        )));
    }
    for framework in names(frameworks) {
        check.framework(framework)?;
    }
    for plugin in names(plugins) {
        if !contents.join("PlugIns").join(plugin).exists() {
            return Err(reject(format!(
                "declared plugin {plugin} is missing from the bundle"
            )));
        }
    }
    for resource in names(resources) {
        if !contents.join("Resources").join(resource).exists() {
            return Err(reject(format!(
                "declared resource {resource} is missing from the bundle"
            )));
        }
    }

    let mut files = Vec::new();
    files_below(&contents, &mut files);
    files.sort();
    let mut rpaths = BTreeSet::new();
    let mut loads = BTreeSet::new();
    let mut minimums: Vec<String> = Vec::new();
    for file in &files {
        if let Some(minos) = check.mach_o(file, &mut rpaths, &mut loads)? {
            minimums.push(minos);
        }
    }
    let minos = newest(&minimums).to_owned();
    if minos.is_empty() {
        return Err(reject(format!("{bundle} holds no Mach-O file")));
    }

    let signed = tools.tool(CODESIGN, &["-dv", &root.to_string_lossy()]);
    let signature = format!("{}\n{}", signed.stdout, signed.stderr)
        .lines()
        .find_map(|line| {
            line.split_once('=')
                .filter(|(key, _)| *key == "Signature")
                .map(|(_, value)| value.to_owned())
        })
        .unwrap_or_default();
    if signature != "adhoc" {
        let shown = if signature.is_empty() {
            "none"
        } else {
            &signature
        };
        return Err(reject(format!(
            "{bundle} has signature '{shown}', want adhoc"
        )));
    }
    let bytes = std::fs::read(&main)
        .map_err(|error| usage(format!("cannot read {}: {error}", main.display())))?;
    let sha = dx_digest::sha256_hex(&bytes);

    let text = format!(
        "{{\n  \"identity\": {{\n    \"bundle_id\": {},\n    \"executable\": {},\n    \"version\": {},\n    \"deployment\": {}\n  }},\n  \"target\": {{\n    \"arch\": {},\n    \"platform\": \"macosx\",\n    \"sdk_version\": {},\n    \"xcode\": {},\n    \"minos\": {}\n  }},\n  \"linkage\": {{\n    \"rpaths\": {},\n    \"loads\": {}\n  }},\n  \"contents\": {{\n    \"frameworks\": {},\n    \"plugins\": {},\n    \"resources\": {}\n  }},\n  \"signing\": {{\n    \"signature\": \"adhoc\",\n    \"executable_sha256\": {}\n  }}\n}}\n",
        quote(bundle_id),
        quote(executable),
        quote(version),
        quote(deployment),
        quote(arch),
        quote(sdk_version),
        quote(xcode),
        quote(&minos),
        array(rpaths.iter().map(String::as_str)),
        array(loads.iter().map(String::as_str)),
        array(names(frameworks)),
        array(names(plugins)),
        array(names(resources)),
        quote(&sha),
    );
    let write = |path: &Path, from: Result<(), std::io::Error>| {
        from.map_err(|error| usage(format!("cannot write {}: {error}", path.display())))
    };
    write(Path::new(out), std::fs::copy(archive_path, out).map(|_| ()))?;
    write(
        Path::new(identity),
        dx_atomic_fs::write_scratch(Path::new(identity), text.as_bytes()),
    )
}

/// # Errors
/// Returns an error when the operation fails.
pub fn run(tools: &Tools<'_>, args: &[String]) -> Outcome {
    let identity = args.first().map_or("", String::as_str);
    let scratch = PathBuf::from(format!("{identity}.scratch"));
    dx_atomic_fs::discard_dir(&scratch);
    let result = verify(tools, args, &scratch);
    dx_atomic_fs::discard_dir(&scratch);
    if result.is_err()
        && let Some(out) = args.get(1)
    {
        dx_atomic_fs::discard_file(out);
    }
    result.map(|()| 0)
}
