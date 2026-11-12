//! Checks one ad hoc signed macOS application archive and writes its identity.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::util::{Failure, Outcome, reject, usage};
use crate::xcode::{CODESIGN, PLUTIL, Tools, components};

pub const PREFIX: &str = "apple_macos_app";

const LOAD_COMMANDS: [&str; 4] = [
    "LC_LOAD_DYLIB",
    "LC_LOAD_WEAK_DYLIB",
    "LC_REEXPORT_DYLIB",
    "LC_LOAD_UPWARD_DYLIB",
];

const K: [u32; 64] = [
    0x428a_2f98,
    0x7137_4491,
    0xb5c0_fbcf,
    0xe9b5_dba5,
    0x3956_c25b,
    0x59f1_11f1,
    0x923f_82a4,
    0xab1c_5ed5,
    0xd807_aa98,
    0x1283_5b01,
    0x2431_85be,
    0x550c_7dc3,
    0x72be_5d74,
    0x80de_b1fe,
    0x9bdc_06a7,
    0xc19b_f174,
    0xe49b_69c1,
    0xefbe_4786,
    0x0fc1_9dc6,
    0x240c_a1cc,
    0x2de9_2c6f,
    0x4a74_84aa,
    0x5cb0_a9dc,
    0x76f9_88da,
    0x983e_5152,
    0xa831_c66d,
    0xb003_27c8,
    0xbf59_7fc7,
    0xc6e0_0bf3,
    0xd5a7_9147,
    0x06ca_6351,
    0x1429_2967,
    0x27b7_0a85,
    0x2e1b_2138,
    0x4d2c_6dfc,
    0x5338_0d13,
    0x650a_7354,
    0x766a_0abb,
    0x81c2_c92e,
    0x9272_2c85,
    0xa2bf_e8a1,
    0xa81a_664b,
    0xc24b_8b70,
    0xc76c_51a3,
    0xd192_e819,
    0xd699_0624,
    0xf40e_3585,
    0x106a_a070,
    0x19a4_c116,
    0x1e37_6c08,
    0x2748_774c,
    0x34b0_bcb5,
    0x391c_0cb3,
    0x4ed8_aa4a,
    0x5b9c_ca4f,
    0x682e_6ff3,
    0x748f_82ee,
    0x78a5_636f,
    0x84c8_7814,
    0x8cc7_0208,
    0x90be_fffa,
    0xa450_6ceb,
    0xbef9_a3f7,
    0xc671_78f2,
];

/// The lowercase hex SHA-256 of `data`.
pub fn sha256(data: &[u8]) -> String {
    let mut state: [u32; 8] = [
        0x6a09_e667,
        0xbb67_ae85,
        0x3c6e_f372,
        0xa54f_f53a,
        0x510e_527f,
        0x9b05_688c,
        0x1f83_d9ab,
        0x5be0_cd19,
    ];
    let mut padded = data.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&((data.len() as u64) * 8).to_be_bytes());
    let (blocks, _) = padded.as_chunks::<64>();
    for block in blocks {
        let mut w = [0u32; 64];
        let (words, _) = block.as_chunks::<4>();
        for (slot, word) in w.iter_mut().zip(words) {
            *slot = u32::from_be_bytes(*word);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ (!e & g);
            let t1 = h
                .wrapping_add(s1)
                .wrapping_add(choice)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (slot, add) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *slot = slot.wrapping_add(add);
        }
    }
    let mut hex = String::new();
    for word in state {
        let _ = write!(hex, "{word:08x}");
    }
    hex
}

/// What `otool -l` text declares: build platforms, minimum versions, rpaths and loaded libraries.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Loads {
    pub platforms: BTreeSet<String>,
    pub minos: Vec<String>,
    pub rpaths: Vec<String>,
    pub loads: Vec<String>,
}

/// Reads the load commands out of `otool -l` text.
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

/// Escapes `text` for a JSON string, dropping control characters.
pub fn esc(text: &str) -> String {
    let mut out = String::new();
    for char in text.chars().filter(|char| !char.is_control()) {
        match char {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            other => out.push(other),
        }
    }
    out
}

fn array<'a>(items: impl IntoIterator<Item = &'a str>) -> String {
    let quoted: Vec<String> = items
        .into_iter()
        .filter(|item| !item.is_empty())
        .map(|item| format!("\"{}\"", esc(item)))
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

    /// The linkage of one file, or `None` when it is not Mach-O.
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
            if rpath.starts_with('/') {
                return Err(reject(format!(
                    "{rel} has absolute rpath {rpath}; use @executable_path or @loader_path"
                )));
            }
            rpaths.insert(rpath.clone());
        }
        for load in &found.loads {
            let embedded = if load.starts_with("/usr/lib/") || load.starts_with("/System/Library/")
            {
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

fn unpack(archive: &Path, tree: &Path) -> Result<(), String> {
    let file = std::fs::File::open(archive).map_err(|error| error.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|error| error.to_string())?;
    zip.extract(tree).map_err(|error| error.to_string())
}

fn verify(tools: &Tools, args: &[String], scratch: &Path, tmp: &Path) -> Result<(), Failure> {
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
    unpack(archive_path, &tree).map_err(|error| {
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
    let sha = sha256(&bytes);

    let text = format!(
        "{{\n  \"identity\": {{\n    \"bundle_id\": \"{}\",\n    \"executable\": \"{}\",\n    \"version\": \"{}\",\n    \"deployment\": \"{}\"\n  }},\n  \"target\": {{\n    \"arch\": \"{}\",\n    \"platform\": \"macosx\",\n    \"sdk_version\": \"{}\",\n    \"xcode\": \"{}\",\n    \"minos\": \"{}\"\n  }},\n  \"linkage\": {{\n    \"rpaths\": {},\n    \"loads\": {}\n  }},\n  \"contents\": {{\n    \"frameworks\": {},\n    \"plugins\": {},\n    \"resources\": {}\n  }},\n  \"signing\": {{\n    \"signature\": \"adhoc\",\n    \"executable_sha256\": \"{}\"\n  }}\n}}\n",
        esc(bundle_id),
        esc(executable),
        esc(version),
        esc(deployment),
        esc(arch),
        esc(sdk_version),
        esc(xcode),
        esc(&minos),
        array(rpaths.iter().map(String::as_str)),
        array(loads.iter().map(String::as_str)),
        array(names(frameworks)),
        array(names(plugins)),
        array(names(resources)),
        esc(&sha),
    );
    let write = |path: &Path, from: Result<(), std::io::Error>| {
        from.map_err(|error| usage(format!("cannot write {}: {error}", path.display())))
    };
    write(tmp, std::fs::write(tmp, text))?;
    write(Path::new(out), std::fs::copy(archive_path, out).map(|_| ()))?;
    write(Path::new(identity), std::fs::rename(tmp, identity))
}

/// Checks the app archive `args` name, publishes the zip and writes the identity on success.
///
/// Failures read `<what> ...`; the caller prefixes [`PREFIX`].
pub fn run(tools: &Tools, args: &[String]) -> Outcome {
    let identity = args.first().map_or("", String::as_str);
    let scratch = PathBuf::from(format!("{identity}.scratch"));
    let tmp = PathBuf::from(format!("{identity}.tmp"));
    let _ = std::fs::remove_dir_all(&scratch);
    let result = verify(tools, args, &scratch, &tmp);
    let _ = std::fs::remove_dir_all(&scratch);
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
        if let Some(out) = args.get(1) {
            let _ = std::fs::remove_file(out);
        }
    }
    result.map(|()| 0)
}
