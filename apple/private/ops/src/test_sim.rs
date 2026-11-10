//! Creates and deletes the simulator one test run uses.

use crate::simctl::{device, list};
use crate::util::{Env, Failure, Outcome, exit_code, usage};
use crate::xcode::Tools;

pub const PREFIX: &str = "dx-apple-test-simulator";

/// The process group a simulator name records, when `name` is one dx created.
pub fn owner(name: &str) -> Option<i32> {
    let (head, group) = name.strip_prefix("dx ")?.rsplit_once(" pgid ")?;
    if group.is_empty() || !group.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let (kind, version) = head.rsplit_once(" iOS ")?;
    let numeric = !version.is_empty() && version.chars().all(|c| c.is_ascii_digit() || c == '.');
    if kind.is_empty() || !numeric {
        return None;
    }
    group.parse().ok()
}

fn quiet(tools: &Tools, args: &[&str]) {
    let _ = tools.xcrun(args);
}

fn sweep(tools: &Tools, alive: &dyn Fn(i32) -> bool) {
    let Ok(listing) = list(tools, &["devices", "available"], "simulators") else {
        return;
    };
    for (name, udid) in listing.lines().filter_map(device) {
        if owner(name).is_some_and(|group| !alive(group)) {
            quiet(tools, &["simctl", "shutdown", udid]);
            quiet(tools, &["simctl", "delete", udid]);
        }
    }
}

/// Creates and boots one simulator; its UDID is the first line of the returned stdout.
pub fn create(
    tools: &Tools,
    env: Env,
    group: i32,
    alive: &dyn Fn(i32) -> bool,
) -> Result<String, Failure> {
    let (Some(device_type), Some(os)) = (env("SIMULATOR_DEVICE_TYPE"), env("SIMULATOR_OS_VERSION"))
    else {
        return Err(usage(
            "SIMULATOR_DEVICE_TYPE and SIMULATOR_OS_VERSION name the simulator to create",
        ));
    };
    if env("SIMULATOR_REUSE_SIMULATOR").as_deref() == Some("1") {
        return Err(usage(
            "test simulators are never reused; SIMULATOR_REUSE_SIMULATOR must be unset",
        ));
    }
    let runtime = format!(
        "com.apple.CoreSimulator.SimRuntime.iOS-{}",
        os.replace('.', "-")
    );
    let runtimes = list(tools, &["runtimes", "available"], "runtimes")?;
    if !runtimes
        .lines()
        .any(|line| line.split_whitespace().last() == Some(runtime.as_str()))
    {
        return Err(usage(format!(
            "no available iOS {os} simulator runtime {runtime}; install it with xcodebuild -downloadPlatform iOS -buildVersion {os}"
        )));
    }
    sweep(tools, alive);
    let name = format!("dx {device_type} iOS {os} pgid {group}");
    let created = tools.xcrun(&["simctl", "create", &name, &device_type, &runtime]);
    if !created.success() {
        return Err(Failure {
            code: exit_code(created.code),
            message: format!(
                "xcrun simctl create failed: {}{}",
                created.stdout, created.stderr
            ),
        });
    }
    let udid = created.stdout.trim().to_owned();
    let booted = tools.xcrun(&["simctl", "bootstatus", &udid, "-b"]);
    eprint!("{}{}", booted.stdout, booted.stderr);
    if !booted.success() {
        quiet(tools, &["simctl", "delete", &udid]);
        return Err(Failure {
            code: exit_code(booted.code),
            message: format!("simulator {udid} did not boot"),
        });
    }
    Ok(udid)
}

/// Shuts down and deletes the simulator `SIMULATOR_UDID` names; a failed delete only warns.
pub fn delete(tools: &Tools, env: Env) -> Outcome {
    let udid =
        env("SIMULATOR_UDID").ok_or_else(|| usage("SIMULATOR_UDID names the simulator to delete"))?;
    quiet(tools, &["simctl", "shutdown", &udid]);
    let deleted = tools.xcrun(&["simctl", "delete", &udid]);
    if !deleted.success() {
        eprint!("{}{}", deleted.stdout, deleted.stderr);
        eprintln!(
            "{PREFIX}: warning: could not delete simulator {udid}; the next test run on this host sweeps it"
        );
    }
    Ok(0)
}
