//! Checks the slices of one iOS XCFramework archive and writes its report.

use std::path::{Path, PathBuf};

use crate::util::{Failure, Outcome, reject, usage};
use crate::xcode::{PLUTIL, Tools, components};

pub const PREFIX: &str = "apple_ios_xcframework";

/// What `otool -l` text declares for one slice.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Slice {
    pub platforms: Vec<String>,
    pub minos: String,
    pub install_name: String,
}

/// Reads the build platforms, the newest minimum version and the install name out of `otool -l` text.
pub fn parse_slice(text: &str) -> Slice {
    let mut found = Slice::default();
    let mut command = "";
    let mut install = None;
    for line in text.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        let [first, second, ..] = fields[..] else {
            continue;
        };
        match first {
            "cmd" => command = second,
            "platform" => {
                if !found.platforms.iter().any(|have| have == second) {
                    found.platforms.push(second.to_owned());
                }
            }
            "minos" => {
                if found.minos.is_empty() || components(second) > components(&found.minos) {
                    found.minos = second.to_owned();
                }
            }
            "name" if command == "LC_ID_DYLIB" && install.is_none() => {
                install = Some(second.to_owned());
            }
            _ => {}
        }
    }
    found.platforms.sort();
    found.install_name = install.unwrap_or_default();
    found
}

struct Check<'a> {
    tools: &'a Tools<'a>,
    root_plist: PathBuf,
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

    fn root(&self, key: &str) -> Option<String> {
        self.value(key, &self.root_plist)
    }
}

fn expect(check: &Check, key: &str, plist: &Path, want: &str, what: &str) -> Result<(), Failure> {
    let got = check.value(key, plist).unwrap_or_default();
    if got == want {
        Ok(())
    } else {
        Err(reject(format!("{what} has {key} '{got}', want '{want}'")))
    }
}

fn unpack(archive: &Path, tree: &Path) -> Result<(), String> {
    let file = std::fs::File::open(archive).map_err(|error| error.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|error| error.to_string())?;
    zip.extract(tree).map_err(|error| error.to_string())
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
            "usage: apple_ops ios-xcframework-check <report> <zip> <archive> <name> <bundle-id> <version> <deployment> <xcode> <slices> <resources> <header>...",
        ));
    };
    std::fs::create_dir_all(work)
        .map_err(|error| usage(format!("cannot create {}: {error}", work.display())))?;
    let archive_path = Path::new(archive);
    unpack(archive_path, work).map_err(|error| {
        let last = error.lines().last().unwrap_or_default();
        reject(format!(
            "{} does not unpack: {last}",
            archive_path
                .file_name()
                .map_or_else(String::new, |base| base.to_string_lossy().into_owned())
        ))
    })?;
    let root = work.join(format!("{name}.xcframework"));
    let root_plist = root.join("Info.plist");
    if !root_plist.is_file() {
        return Err(reject(format!("{name}.xcframework has no Info.plist")));
    }
    let resource_names: Vec<&str> = resources.lines().collect();
    let mut text = format!(
        "xcframework={name}\nbundle_id={bundle_id}\nversion={version}\ndeployment={deployment}\nxcode={xcode}\nresources={}\n",
        resource_names.join(",")
    );
    let check = Check { tools, root_plist };
    let mut found: Vec<String> = Vec::new();
    let mut index = 0;
    while let Some(identifier) =
        check.root(&format!("AvailableLibraries.{index}.LibraryIdentifier"))
    {
        let entry = format!("AvailableLibraries.{index}");
        let platform = check
            .root(&format!("{entry}.SupportedPlatform"))
            .unwrap_or_default();
        if platform != "ios" {
            return Err(reject(format!(
                "library {identifier} is for '{platform}', want ios"
            )));
        }
        let variant = check
            .root(&format!("{entry}.SupportedPlatformVariant"))
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
            .root(&format!("{entry}.LibraryPath"))
            .unwrap_or_default();
        if path != format!("{name}.framework") {
            return Err(reject(format!(
                "library {identifier} has path '{path}', want {name}.framework"
            )));
        }
        let framework = root.join(&identifier).join(&path);
        let binary = framework.join(name);
        if !binary.is_file() {
            return Err(reject(format!(
                "library {identifier} has no binary {path}/{name}"
            )));
        }
        let binary_text = binary.to_string_lossy().into_owned();
        let lipo = tools.xcrun(&["lipo", "-archs", &binary_text]);
        if !lipo.success() {
            return Err(reject(format!(
                "binary {identifier}/{path}/{name} is not a Mach-O file"
            )));
        }
        let archs_text = lipo.stdout.trim_end_matches('\n').to_owned();
        let archs: Vec<&str> = archs_text.split_whitespace().collect();
        let plist = framework.join("Info.plist");
        if !plist.is_file() {
            return Err(reject(format!(
                "library {identifier} has no {path}/Info.plist"
            )));
        }
        let what = format!("{identifier}/{path}");
        expect(&check, "CFBundleIdentifier", &plist, bundle_id, &what)?;
        expect(&check, "CFBundleExecutable", &plist, name, &what)?;
        expect(&check, "CFBundlePackageType", &plist, "FMWK", &what)?;
        expect(&check, "CFBundleShortVersionString", &plist, version, &what)?;
        expect(&check, "MinimumOSVersion", &plist, deployment, &what)?;
        expect(
            &check,
            "CFBundleSupportedPlatforms.0",
            &plist,
            supported,
            &what,
        )?;
        let modulemap = framework.join("Modules/module.modulemap");
        if !modulemap.is_file() {
            return Err(reject(format!(
                "library {identifier} has no {path}/Modules/module.modulemap"
            )));
        }
        let declared = std::fs::read_to_string(&modulemap).unwrap_or_default();
        if !declared.contains(&format!("framework module {name} ")) {
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
        while let Some(arch) = check.root(&format!("{entry}.SupportedArchitectures.{position}")) {
            let cell = format!("{environment}/{arch}");
            if found.contains(&cell) {
                return Err(reject(format!(
                    "slice {cell} appears in more than one library"
                )));
            }
            found.push(cell.clone());
            if !archs.contains(&arch.as_str()) {
                return Err(reject(format!(
                    "binary {identifier}/{path}/{name} carries {archs_text}, want {arch} for {cell}"
                )));
            }
            let otool = tools.xcrun(&["otool", "-arch", &arch, "-l", &binary_text]);
            let slice = parse_slice(if otool.success() { &otool.stdout } else { "" });
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
            if components(&slice.minos) > components(deployment) {
                return Err(reject(format!(
                    "slice {cell} needs iOS {}, above the framework deployment {deployment}",
                    slice.minos
                )));
            }
            let want_install = format!("@rpath/{name}.framework/{name}");
            if slice.install_name != want_install {
                return Err(reject(format!(
                    "slice {cell} installs as '{}', want {want_install}",
                    slice.install_name
                )));
            }
            text.push_str(&format!(
                "slice={cell} library={identifier} platform={sdk} minos={} install_name={}\n",
                slice.minos, slice.install_name
            ));
            listed.push(arch);
            position += 1;
        }
        if listed.is_empty() {
            return Err(reject(format!(
                "library {identifier} lists no architectures"
            )));
        }
        for arch in &archs {
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
        if !found.iter().any(|have| have == want) {
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
    let write = |path: &Path, from: Result<(), std::io::Error>| {
        from.map_err(|error| usage(format!("cannot write {}: {error}", path.display())))
    };
    write(tmp, std::fs::write(tmp, text))?;
    write(Path::new(out), std::fs::copy(archive_path, out).map(|_| ()))?;
    write(Path::new(report), std::fs::rename(tmp, report))
}

/// Checks the XCFramework archive `args` name, publishes the zip and writes the report on success.
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
