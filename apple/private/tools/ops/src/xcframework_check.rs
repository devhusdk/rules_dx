//! Checks the slices of one iOS XCFramework archive and publishes it.

use std::path::{Path, PathBuf};

use crate::util::{Failure, Outcome, reject, usage};
use crate::xcode::{PLUTIL, Tools, components};

pub const PREFIX: &str = "apple_ios_xcframework";

/// What `otool -l` declares for one slice.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Slice {
    pub platforms: Vec<String>,
    pub minos: String,
    pub install_name: String,
}

/// Reads the build platforms, newest minimum version and install name out of `otool -l` text.
pub fn parse_slice(loads: &str) -> Slice {
    let mut found = Slice::default();
    let mut command = "";
    for line in loads.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        let [first, second, ..] = fields[..] else {
            continue;
        };
        if first == "cmd" {
            command = second;
        }
        match first {
            "platform" => found.platforms.push(second.to_owned()),
            "minos" => {
                if found.minos.is_empty() || components(second) > components(&found.minos) {
                    second.clone_into(&mut found.minos);
                }
            }
            "name" if command == "LC_ID_DYLIB" && found.install_name.is_empty() => {
                second.clone_into(&mut found.install_name);
            }
            _ => {}
        }
    }
    found.platforms.sort();
    found.platforms.dedup();
    found
}

fn unpack(archive: &Path, tree: &Path) -> Result<(), String> {
    let file = std::fs::File::open(archive).map_err(|error| error.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|error| error.to_string())?;
    zip.extract(tree).map_err(|error| error.to_string())
}

struct Check<'a> {
    tools: &'a Tools<'a>,
    name: &'a str,
    deployment: &'a str,
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

    fn expect(&self, key: &str, plist: &Path, want: &str, what: &str) -> Result<(), Failure> {
        let got = self.value(key, plist).unwrap_or_default();
        if got == want {
            Ok(())
        } else {
            Err(reject(format!("{what} has {key} '{got}', want '{want}'")))
        }
    }

    fn slice(
        &self,
        cell: &str,
        arch: &str,
        binary: &Path,
        sdk: &str,
        code: &str,
    ) -> Result<Slice, Failure> {
        let otool = self
            .tools
            .xcrun(&["otool", "-arch", arch, "-l", &binary.to_string_lossy()]);
        let loads = if otool.success() {
            otool.stdout
        } else {
            String::new()
        };
        let slice = parse_slice(&loads);
        if slice.platforms != [code] {
            return Err(reject(format!(
                "slice {cell} declares build platform '{}', want {code} for {sdk}",
                slice.platforms.join(" ")
            )));
        }
        if slice.minos.is_empty() {
            return Err(reject(format!(
                "slice {cell} declares no minimum iOS version"
            )));
        }
        if components(&slice.minos) > components(self.deployment) {
            return Err(reject(format!(
                "slice {cell} needs iOS {}, above the framework deployment {}",
                slice.minos, self.deployment
            )));
        }
        let want = format!("@rpath/{0}.framework/{0}", self.name);
        if slice.install_name != want {
            return Err(reject(format!(
                "slice {cell} installs as '{}', want {want}",
                slice.install_name
            )));
        }
        Ok(slice)
    }
}

fn write(path: &Path, result: std::io::Result<()>) -> Result<(), Failure> {
    result.map_err(|error| usage(format!("cannot write {}: {error}", path.display())))
}

fn verify(tools: &Tools, args: &[String], work: &Path, tmp: &Path) -> Result<(), Failure> {
    let [
        report,
        out,
        archive,
        name,
        bundle_id,
        version,
        deployment,
        xcode,
        slices,
        resources,
        headers @ ..,
    ] = args
    else {
        return Err(usage(
            "usage: apple_ops xcframework-check <report> <out> <archive> <name> <bundle-id> <version> <deployment> <xcode> <slices> <resources> <header>...",
        ));
    };
    let archive_path = Path::new(archive);
    let archive_name = archive_path.file_name().map_or_else(
        || archive.clone(),
        |base| base.to_string_lossy().into_owned(),
    );
    if let Err(error) = std::fs::create_dir_all(work)
        .map_err(|error| error.to_string())
        .and_then(|()| unpack(archive_path, work))
    {
        return Err(reject(format!("{archive_name} does not unpack: {error}")));
    }
    let root = work.join(format!("{name}.xcframework"));
    let index_plist = root.join("Info.plist");
    if !index_plist.is_file() {
        return Err(reject(format!("{name}.xcframework has no Info.plist")));
    }
    let check = Check {
        tools,
        name,
        deployment,
    };

    let resource_names: Vec<&str> = resources.split('\n').collect();
    let mut text = format!(
        "xcframework={name}\nbundle_id={bundle_id}\nversion={version}\ndeployment={deployment}\nxcode={xcode}\nresources={}\n",
        resource_names.join(",")
    );
    let mut found: Vec<String> = Vec::new();
    let mut index = 0;
    while let Some(identifier) = check.value(
        &format!("AvailableLibraries.{index}.LibraryIdentifier"),
        &index_plist,
    ) {
        let entry = format!("AvailableLibraries.{index}");
        let platform = check
            .value(&format!("{entry}.SupportedPlatform"), &index_plist)
            .unwrap_or_default();
        if platform != "ios" {
            return Err(reject(format!(
                "library {identifier} is for '{platform}', want ios"
            )));
        }
        let variant = check
            .value(&format!("{entry}.SupportedPlatformVariant"), &index_plist)
            .unwrap_or_default();
        let (environment, sdk, code, supported) = match variant.as_str() {
            "" => ("device", "iphoneos", "2", "iPhoneOS"),
            "simulator" => ("simulator", "iphonesimulator", "7", "iPhoneSimulator"),
            _ => {
                return Err(reject(format!(
                    "library {identifier} has variant '{variant}', want device or simulator"
                )));
            }
        };
        let path = check
            .value(&format!("{entry}.LibraryPath"), &index_plist)
            .unwrap_or_default();
        if path != format!("{name}.framework") {
            return Err(reject(format!(
                "library {identifier} has path '{path}', want {name}.framework"
            )));
        }
        let framework: PathBuf = root.join(&identifier).join(&path);
        let binary = framework.join(name);
        if !binary.is_file() {
            return Err(reject(format!(
                "library {identifier} has no binary {path}/{name}"
            )));
        }
        let lipo = tools.xcrun(&["lipo", "-archs", &binary.to_string_lossy()]);
        if !lipo.success() {
            return Err(reject(format!(
                "binary {identifier}/{path}/{name} is not a Mach-O file"
            )));
        }
        let archs = lipo.stdout.trim_end_matches('\n').to_owned();
        let plist = framework.join("Info.plist");
        if !plist.is_file() {
            return Err(reject(format!(
                "library {identifier} has no {path}/Info.plist"
            )));
        }
        let what = format!("{identifier}/{path}");
        check.expect("CFBundleIdentifier", &plist, bundle_id, &what)?;
        check.expect("CFBundleExecutable", &plist, name, &what)?;
        check.expect("CFBundlePackageType", &plist, "FMWK", &what)?;
        check.expect("CFBundleShortVersionString", &plist, version, &what)?;
        check.expect("MinimumOSVersion", &plist, deployment, &what)?;
        check.expect("CFBundleSupportedPlatforms.0", &plist, supported, &what)?;
        let modulemap = framework.join("Modules/module.modulemap");
        if !modulemap.is_file() {
            return Err(reject(format!(
                "library {identifier} has no {path}/Modules/module.modulemap"
            )));
        }
        let module_text =
            String::from_utf8_lossy(&std::fs::read(&modulemap).unwrap_or_default()).into_owned();
        if !module_text.contains(&format!("framework module {name} ")) {
            return Err(reject(format!(
                "{identifier}/{path}/Modules/module.modulemap declares no framework module {name}"
            )));
        }
        for header in headers {
            if !framework.join("Headers").join(header).is_file() {
                return Err(reject(format!(
                    "library {identifier} misses public header Headers/{header}"
                )));
            }
        }
        for resource in resource_names.iter().filter(|name| !name.is_empty()) {
            if !framework.join(resource).is_file() {
                return Err(reject(format!(
                    "library {identifier} misses resource {path}/{resource}"
                )));
            }
        }
        let mut listed: Vec<String> = Vec::new();
        let mut position = 0;
        while let Some(arch) = check.value(
            &format!("{entry}.SupportedArchitectures.{position}"),
            &index_plist,
        ) {
            let cell = format!("{environment}/{arch}");
            if found.contains(&cell) {
                return Err(reject(format!(
                    "slice {cell} appears in more than one library"
                )));
            }
            found.push(cell.clone());
            listed.push(arch.clone());
            if !archs.split_whitespace().any(|have| have == arch) {
                return Err(reject(format!(
                    "binary {identifier}/{path}/{name} carries {archs}, want {arch} for {cell}"
                )));
            }
            let slice = check.slice(&cell, &arch, &binary, sdk, code)?;
            text.push_str(&format!(
                "slice={cell} library={identifier} platform={sdk} minos={} install_name={}\n",
                slice.minos, slice.install_name
            ));
            position += 1;
        }
        if listed.is_empty() {
            return Err(reject(format!(
                "library {identifier} lists no architectures"
            )));
        }
        for arch in archs.split_whitespace() {
            if !listed.iter().any(|have| have == arch) {
                return Err(reject(format!(
                    "binary {identifier}/{path}/{name} carries {arch}, which library {identifier} does not list"
                )));
            }
        }
        index += 1;
    }
    if index == 0 {
        return Err(reject(format!("{name}.xcframework lists no libraries")));
    }
    let declared: Vec<&str> = slices.split(',').filter(|cell| !cell.is_empty()).collect();
    for want in &declared {
        if !found.iter().any(|cell| cell == want) {
            return Err(reject(format!(
                "{name}.xcframework misses the declared slice {want}"
            )));
        }
    }
    for cell in &found {
        if !declared.contains(&cell.as_str()) {
            return Err(reject(format!(
                "{name}.xcframework carries the undeclared slice {cell}"
            )));
        }
    }

    let _ = std::fs::remove_dir_all(work);
    write(tmp, std::fs::write(tmp, text))?;
    write(Path::new(out), std::fs::copy(archive_path, out).map(|_| ()))?;
    write(Path::new(report), std::fs::rename(tmp, report))
}

/// Checks the XCFramework archive `args` name, publishes it and writes the slice report on success.
///
/// Failures read `<what> ...`; the caller prefixes [`PREFIX`].
pub fn run(tools: &Tools, args: &[String]) -> Outcome {
    let report = args.first().map_or("", String::as_str);
    let work = PathBuf::from(format!("{report}.unpacked"));
    let tmp = PathBuf::from(format!("{report}.tmp"));
    let _ = std::fs::remove_dir_all(&work);
    let result = verify(tools, args, &work, &tmp);
    let _ = std::fs::remove_dir_all(&work);
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
        if let Some(out) = args.get(1) {
            let _ = std::fs::remove_file(out);
        }
    }
    result.map(|()| 0)
}
