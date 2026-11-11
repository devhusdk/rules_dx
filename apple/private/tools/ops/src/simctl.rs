//! Boots, installs on and launches one named simulator.

use std::path::Path;

use crate::util::{Env, Failure, Outcome, Scratch, exit_code, missing, reject, usage};
use crate::xcode::{Tools, XCRUN};

pub const PREFIX: &str = "dx-apple-simctl";

/// The `(name, udid)` of a line of `simctl list devices`.
pub fn device(line: &str) -> Option<(&str, &str)> {
    let first = line.strip_prefix(' ')?.trim_start().chars().next()?;
    if matches!(first, '=' | '-') {
        return None;
    }
    let (rest, state) = line.trim().strip_suffix(')')?.rsplit_once(" (")?;
    if state.contains(['(', ')']) {
        return None;
    }
    let (name, udid) = rest.strip_suffix(')')?.rsplit_once(" (")?;
    let hex = !udid.is_empty() && udid.chars().all(|c| c.is_ascii_hexdigit() || c == '-');
    hex.then_some((name, udid))
}

/// The stdout of `xcrun simctl list <what>`, or why it failed.
pub fn list(tools: &Tools, what: &[&str], noun: &str) -> Result<String, Failure> {
    let mut args = vec!["simctl", "list"];
    args.extend_from_slice(what);
    let listed = tools.xcrun(&args);
    match listed.code {
        Some(0) => Ok(listed.stdout),
        None => Err(missing(format!(
            "cannot start {XCRUN}: {}; simulator operations need a macOS host with Xcode",
            listed.stderr.trim()
        ))),
        Some(code) => Err(Failure {
            code: exit_code(Some(code)),
            message: format!(
                "xcrun simctl cannot list {noun}: {}{}",
                listed.stdout, listed.stderr
            ),
        }),
    }
}

fn udid(tools: &Tools, env: Env) -> Result<String, Failure> {
    let want = env("DX_SIMCTL_DEVICE")
        .ok_or_else(|| usage("DX_SIMCTL_DEVICE names the simulator by name or UDID"))?;
    let listing = list(tools, &["devices", "available"], "simulators")?;
    let mut found: Vec<&str> = listing
        .lines()
        .filter_map(device)
        .filter(|(name, udid)| *name == want || *udid == want)
        .map(|(_, udid)| udid)
        .collect();
    match found.len() {
        0 => Err(usage(format!(
            "no available simulator named '{want}'; create one with xcrun simctl create"
        ))),
        1 => Ok(found.remove(0).to_owned()),
        count => Err(usage(format!(
            "'{want}' names {count} simulators; pass one UDID"
        ))),
    }
}

fn done(code: Option<i32>) -> Outcome {
    Ok(exit_code(code))
}

fn install(tools: &Tools, env: Env, udid: &str, app: &str) -> Outcome {
    let app_path = Path::new(app);
    if !app_path.exists() {
        return Err(usage("the built .ipa or .app is missing"));
    }
    if app.ends_with(".app") {
        return done(tools.xcrun_inherit(&["simctl", "install", udid, app]));
    }
    if !app.ends_with(".ipa") {
        return Err(usage(format!("{app} is neither an .ipa nor an .app")));
    }
    let work = Scratch::new(env, "simctl")?;
    let file = std::fs::File::open(app_path)
        .map_err(|error| reject(format!("cannot read {app}: {error}")))?;
    zip::ZipArchive::new(file)
        .and_then(|mut archive| archive.extract(work.path()))
        .map_err(|error| reject(format!("{app} does not unpack: {error}")))?;
    let mut bundles: Vec<_> = std::fs::read_dir(work.path().join("Payload"))
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|end| end == "app") && path.is_dir())
                .collect()
        })
        .unwrap_or_default();
    bundles.sort();
    let Some(bundle) = bundles.first() else {
        return Err(usage(format!("{app} holds no Payload/*.app bundle")));
    };
    let bundle = bundle.to_string_lossy();
    done(tools.xcrun_inherit(&["simctl", "install", udid, &bundle]))
}

/// Runs `simctl <op> [app]`.
pub fn run(tools: &Tools, args: &[String], env: Env) -> Outcome {
    let Some((op, rest)) = args.split_first() else {
        return Err(usage("usage: apple_ops simctl <boot|install|launch> [app]"));
    };
    let udid = udid(tools, env)?;
    match (op.as_str(), rest) {
        ("boot", []) => done(tools.xcrun_inherit(&["simctl", "bootstatus", &udid, "-b"])),
        ("install", [app]) => install(tools, env, &udid, app),
        ("launch", []) => {
            let bundle_id = env("DX_SIMCTL_BUNDLE_ID")
                .ok_or_else(|| usage("DX_SIMCTL_BUNDLE_ID names the installed app"))?;
            done(tools.xcrun_inherit(&[
                "simctl",
                "launch",
                "--terminate-running-process",
                &udid,
                &bundle_id,
            ]))
        }
        ("boot" | "install" | "launch", _) => {
            Err(usage(format!("unexpected arguments for simctl {op}")))
        }
        _ => Err(usage(format!("unknown op '{op}'"))),
    }
}
