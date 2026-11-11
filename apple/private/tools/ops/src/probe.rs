//! Records the executor's Xcode, SDK, compiler and macOS identities, failing on any mismatch.

use std::path::Path;

use crate::util::{Env, Failure, Outcome, Output, reject, usage};
use crate::xcode::{Tools, components};

pub const PREFIX: &str = "apple";

/// The machine the probe runs on.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Executor {
    pub system: String,
    pub arch: String,
    pub macos: String,
    pub macos_build: String,
}

fn field(raw: &[libc::c_char]) -> String {
    let bytes: Vec<u8> = raw
        .iter()
        .take_while(|byte| **byte != 0)
        .map(|byte| *byte as u8)
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

#[cfg(target_os = "macos")]
fn sysctl(name: &str) -> String {
    let Ok(key) = std::ffi::CString::new(name) else {
        return String::new();
    };
    let mut size: libc::size_t = 0;
    // SAFETY: a null buffer asks sysctlbyname for the size of the value.
    let rc = unsafe {
        libc::sysctlbyname(key.as_ptr(), std::ptr::null_mut(), &mut size, std::ptr::null_mut(), 0)
    };
    if rc != 0 || size == 0 {
        return String::new();
    }
    let mut buffer = vec![0u8; size];
    // SAFETY: the buffer holds `size` bytes.
    let rc = unsafe {
        libc::sysctlbyname(
            key.as_ptr(),
            buffer.as_mut_ptr().cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if rc != 0 {
        return String::new();
    }
    buffer.truncate(size);
    let end = buffer.iter().position(|byte| *byte == 0).unwrap_or(buffer.len());
    String::from_utf8_lossy(&buffer[..end]).into_owned()
}

#[cfg(not(target_os = "macos"))]
fn sysctl(_name: &str) -> String {
    String::new()
}

/// The machine this process runs on.
pub fn executor() -> Executor {
    // SAFETY: utsname is plain bytes, and uname fills it.
    let mut name: libc::utsname = unsafe { std::mem::zeroed() };
    // SAFETY: name is a valid utsname.
    let rc = unsafe { libc::uname(&mut name) };
    let (system, arch) = if rc == 0 {
        (field(&name.sysname), field(&name.machine))
    } else {
        (String::new(), String::new())
    };
    let darwin = system == "Darwin";
    Executor {
        macos: if darwin { sysctl("kern.osproductversion") } else { String::new() },
        macos_build: if darwin { sysctl("kern.osversion") } else { String::new() },
        system,
        arch,
    }
}

fn text(output: &Output) -> String {
    format!("{}{}", output.stdout, output.stderr)
        .trim_end_matches('\n')
        .to_owned()
}

fn line(output: &Output, prefix: &str) -> String {
    output
        .stdout
        .lines()
        .find_map(|line| line.strip_prefix(prefix))
        .map(|rest| rest.split_whitespace().next().unwrap_or("").to_owned())
        .unwrap_or_default()
}

fn xcrun(tools: &Tools, args: &[&str]) -> Result<String, Failure> {
    let output = tools.xcrun(args);
    if output.success() {
        Ok(output.stdout.trim_end_matches('\n').to_owned())
    } else {
        Err(reject(format!("xcrun {} failed: {}", args.join(" "), text(&output))))
    }
}

/// Runs the probe `args` name and writes the report on success.
pub fn run(tools: &Tools, executor: &Executor, args: &[String], env: Env) -> Outcome {
    let [out, route, host, xcode, sdk, sdk_version, deployment, macos_floor, compiler, rest @ ..] =
        args
    else {
        return Err(usage(
            "usage: apple_ops probe <out> <route> <host> <xcode> <sdk> <sdk-version> <deployment> <macos-floor> <compiler> [endpoint]",
        ));
    };
    let endpoint = rest.first().map_or("", String::as_str);
    let Executor {
        system,
        arch,
        macos,
        macos_build,
    } = executor;

    if system != "Darwin" {
        return Err(reject(format!(
            "probe ran on {system} {arch}, not a macOS executor; the {route} route needs {host} with Xcode {xcode}"
        )));
    }
    if host == "macos_arm64" && arch != "arm64" {
        return Err(reject(format!("executor is {arch}, not the arm64 host {host}")));
    }
    if components(macos) < components(macos_floor) {
        return Err(reject(format!("executor runs macOS {macos}, below {macos_floor}")));
    }

    let license = tools.xcrun(&["xcodebuild", "-license", "check"]);
    if !license.success() {
        return Err(reject(format!(
            "the Xcode license is not accepted on this executor; an administrator runs 'sudo xcodebuild -license accept' there: {}",
            text(&license)
        )));
    }

    let developer_dir = env("DEVELOPER_DIR");
    let version = tools.xcrun(&["xcodebuild", "-version"]);
    if !version.success() {
        return Err(reject(format!(
            "xcodebuild failed under DEVELOPER_DIR={}",
            developer_dir.as_deref().unwrap_or("unset")
        )));
    }
    let actual = line(&version, "Xcode ");
    let build = line(&version, "Build version ");
    if components(&actual) != components(xcode) {
        return Err(reject(format!(
            "executor runs Xcode {actual} ({build}) but Bazel resolved {xcode}"
        )));
    }
    if let Some(resolved) = xcode.split('.').nth(3).filter(|resolved| !resolved.is_empty())
        && *resolved != build
    {
        return Err(reject(format!(
            "executor runs Xcode build {build} but Bazel resolved {xcode}"
        )));
    }

    let probed = tools.xcrun(&["--sdk", sdk, "--show-sdk-version"]);
    if !probed.success() {
        return Err(reject(format!("Xcode {actual} has no {sdk} SDK")));
    }
    let sdk_actual = probed.stdout.trim_end_matches('\n').to_owned();
    if components(&sdk_actual) != components(sdk_version) {
        return Err(reject(format!(
            "executor has the {sdk} {sdk_actual} SDK but Bazel resolved {sdk_version}"
        )));
    }
    let sdk_build = xcrun(tools, &["--sdk", sdk, "--show-sdk-build-version"])?;
    let sdk_path = xcrun(tools, &["--sdk", sdk, "--show-sdk-path"])?;
    let clang = xcrun(tools, &["--sdk", sdk, "--find", "clang"])?;

    let reported = tools.tool(compiler, &["--version"]);
    let identity = reported
        .stdout
        .lines()
        .next()
        .or_else(|| reported.stderr.lines().next())
        .unwrap_or("");
    if !identity.starts_with("Apple clang") {
        return Err(reject(format!(
            "the C/C++ toolchain compiler {compiler} reports '{identity}', not Xcode's Apple clang; register @local_config_apple_cc_toolchains//:all ahead of other C/C++ toolchains"
        )));
    }

    let developer_dir = developer_dir.unwrap_or_default();
    let report = format!(
        "route={route}\nendpoint={endpoint}\nhost={host}\narch={arch}\nmacos={macos}\nmacos_build={macos_build}\nxcode={actual}\nxcode_build={build}\ndeveloper_dir={developer_dir}\nsdk={sdk}\nsdk_version={sdk_actual}\nsdk_build={sdk_build}\nsdk_path={sdk_path}\ndeployment={deployment}\nclang={clang}\ncompiler={identity}\nlicense=accepted\n"
    );
    std::fs::write(Path::new(out), report)
        .map_err(|error| usage(format!("cannot write {out}: {error}")))?;
    Ok(0)
}
