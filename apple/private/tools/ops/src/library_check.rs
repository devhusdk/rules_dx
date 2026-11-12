//! Checks that imported Mach-O libraries and framework binaries belong to one app cell.

use std::path::Path;

use crate::util::{Failure, Outcome, reject, usage};
use crate::xcode::{Tools, components};

/// What one Mach-O file declares: platforms and minimum OS versions, in load-command order.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Declared {
    pub platforms: Vec<String>,
    pub versions: Vec<String>,
}

/// Reads the build platforms and minimum versions out of `otool -l` text.
///
/// `version_min` is the platform number the old iOS version-min command stands for.
pub fn declared(loads: &str, version_min: &str) -> Declared {
    let mut found = Declared::default();
    let mut command = "";
    for line in loads.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        let [first, second, ..] = fields[..] else {
            continue;
        };
        if first == "cmd" {
            command = second;
        }
        match (command, first) {
            ("LC_BUILD_VERSION", "platform") => found.platforms.push(second.to_owned()),
            ("LC_BUILD_VERSION", "minos") => found.versions.push(second.to_owned()),
            ("LC_VERSION_MIN_IPHONEOS", "version") => {
                found.platforms.push(version_min.to_owned());
                found.versions.push(second.to_owned());
            }
            ("LC_VERSION_MIN_MACOSX", "version") => {
                found.platforms.push("1".to_owned());
                found.versions.push(second.to_owned());
            }
            _ => {}
        }
    }
    found
}

fn platform_name(number: &str) -> String {
    match number {
        "1" => "macosx".to_owned(),
        "2" => "iphoneos".to_owned(),
        "7" => "iphonesimulator".to_owned(),
        other => format!("platform {other}"),
    }
}

/// The newest of `versions`, or empty when there is none.
fn newest(versions: &[String]) -> &str {
    versions.iter().fold("", |best, version| {
        if best.is_empty() || components(version) > components(best) {
            version
        } else {
            best
        }
    })
}

/// Checks the inputs `args` name and writes the cell report on success.
///
/// Failures read `<what> ...`; the caller prefixes the kind.
pub fn run(tools: &Tools, args: &[String]) -> Outcome {
    let [
        kind,
        report,
        cell,
        sdk,
        platform,
        arch,
        deployment,
        xcode,
        sdk_version,
        inputs @ ..,
    ] = args
    else {
        return Err(usage(
            "usage: apple_ops library-check <kind> <report> <cell> <sdk> <platform> <arch> <deployment> <xcode> <sdk-version> <input>...",
        ));
    };
    let owner = kind.rsplit('_').next().unwrap_or(kind);
    let os = if sdk == "macosx" { "macOS" } else { "iOS" };
    let version_min = if arch == "x86_64" { "7" } else { "2" };

    let mut text = format!(
        "cell={cell}\nsdk={sdk}\nsdk_version={sdk_version}\ndeployment={deployment}\nxcode={xcode}\n"
    );
    for input in inputs {
        let name = Path::new(input)
            .file_name()
            .map_or_else(|| input.clone(), |name| name.to_string_lossy().into_owned());
        let (what, key) = if name.ends_with(".a") {
            (format!("library archive {name}"), "archive")
        } else {
            (format!("framework binary {name}"), "framework")
        };
        let lipo = tools.xcrun(&["lipo", "-archs", input]);
        if !lipo.success() {
            return Err(reject(format!("{what} is not a Mach-O file")));
        }
        let archs = lipo.stdout.trim_end_matches('\n');
        if !archs.split(' ').any(|have| have == arch) {
            return Err(reject(format!(
                "{what} carries {archs}, want {arch} for {cell}"
            )));
        }
        let otool = tools.xcrun(&["otool", "-arch", arch, "-l", input]);
        let loads = if otool.success() {
            otool.stdout
        } else {
            String::new()
        };
        let Declared {
            platforms,
            versions,
        } = declared(&loads, version_min);
        if platforms.is_empty() {
            return Err(reject(format!(
                "{what} declares no build platform; rebuild it for {sdk}"
            )));
        }
        let mut unique = platforms;
        unique.sort();
        unique.dedup();
        for found in &unique {
            if found != platform {
                return Err(reject(format!(
                    "{what} is built for {}, want {sdk} for {cell}",
                    platform_name(found)
                )));
            }
        }
        let newest = newest(&versions);
        if !newest.is_empty() && components(newest) > components(deployment) {
            return Err(reject(format!(
                "{what} needs {os} {newest}, above the {owner} deployment {deployment}"
            )));
        }
        text.push_str(&format!(
            "{key}={name} arch={arch} platform={sdk} minos={newest}\n"
        ));
    }
    std::fs::write(Path::new(report), text).map_err(|error: std::io::Error| -> Failure {
        usage(format!("cannot write {report}: {error}"))
    })?;
    Ok(0)
}
