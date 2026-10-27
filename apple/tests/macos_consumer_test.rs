mod consumer;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use consumer::{consumer, fails_with, module, read, write};
use dx_nested as nested;
use dx_testing::Run;

const TOOLCHAINS: &str = r#"
apple_cc_configure = use_extension("@xcode_support//crosstool:setup.bzl", "apple_cc_configure_extension")
use_repo(apple_cc_configure, "local_config_apple_cc_toolchains")

register_toolchains("@local_config_apple_cc_toolchains//:all")
"#;

const BAZELRC: &str = "build:sim --platforms=@xcode_support//platforms:ios_sim_arm64\nbuild:sim --ios_minimum_os=17.0\nbuild:mac --platforms=@xcode_support//platforms:macos_arm64\nbuild:mac --macos_minimum_os=14.0\n";

const BUILD: &str = r#"load("@dx//apple/provisioning:defs.bzl", "apple_provisioning")
load("@dx//apple/provisioning:probe.bzl", "apple_toolchain_probe")

apple_provisioning(
    name = "simulator",
    accept_licenses = ["apple-xcode-license"],
    deployment = "17.0",
    host = "macos_arm64",
    route = "local",
    sdk = "iphonesimulator",
    xcode_version = "XCODE",
)

apple_toolchain_probe(
    name = "simulator_probe",
    provisioning = ":simulator",
    target_compatible_with = ["@platforms//os:ios"],
)

apple_provisioning(
    name = "mac",
    accept_licenses = ["apple-xcode-license"],
    deployment = "14.0",
    host = "macos_arm64",
    route = "local",
    sdk = "macosx",
    xcode_version = "XCODE",
)

apple_toolchain_probe(
    name = "mac_probe",
    provisioning = ":mac",
    target_compatible_with = ["@platforms//os:macos"],
)
"#;

const LEDGER_BUILD: &str = r#"load("@rules_cc//cc:defs.bzl", "cc_binary", "cc_library", "cc_test")

cc_library(
    name = "ledger",
    srcs = ["ledger.c"],
    hdrs = ["ledger.h"],
)

cc_binary(
    name = "ledger_cli",
    srcs = ["main.c"],
    target_compatible_with = ["@platforms//os:macos"],
    deps = [":ledger"],
)

cc_test(
    name = "ledger_test",
    srcs = ["ledger_test.c"],
    target_compatible_with = ["@platforms//os:macos"],
    deps = [":ledger"],
)
"#;

const LEDGER_H: &str = r#"#pragma once

typedef struct {
  const char *name;
  long cents;
} ledger_entry;

long ledger_total(const ledger_entry *entries, int count);
const char *ledger_platform(void);
"#;

const LEDGER_C: &str = r#"#include "ledger/ledger.h"

#include <TargetConditionals.h>

long ledger_total(const ledger_entry *entries, int count) {
  long total = 0;
  for (int i = 0; i < count; i++) {
    total += entries[i].cents;
  }
  return total;
}

const char *ledger_platform(void) {
#if TARGET_OS_SIMULATOR
  return "iphonesimulator";
#elif TARGET_OS_OSX
  return "macosx";
#else
  return "other";
#endif
}
"#;

const MAIN_C: &str = r#"#include <stdio.h>

#include "ledger/ledger.h"

int main(void) {
  ledger_entry entries[] = {{"tea", 450}, {"bread", 325}, {"jam", 600}};
  printf("%s 3 entries %ld cents\n", ledger_platform(), ledger_total(entries, 3));
  return 0;
}
"#;

const LEDGER_TEST_C: &str = r#"#include <string.h>

#include "ledger/ledger.h"

int main(void) {
  ledger_entry entries[] = {{"a", 1}, {"b", -3}, {"c", 10}};
  if (ledger_total(entries, 3) != 8) {
    return 1;
  }
  if (ledger_total(entries, 0) != 0) {
    return 2;
  }
  return strcmp(ledger_platform(), "macosx") == 0 ? 0 : 3;
}
"#;

fn tool(program: &str, args: &[&str], envs: &[(&str, &str)]) -> String {
    let output = Command::new(program)
        .args(args)
        .envs(envs.iter().copied())
        .output()
        .unwrap_or_else(|error| panic!("{program}: {error}"));
    assert!(
        output.status.success(),
        "{program} {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn installed_xcode() -> String {
    let version = tool("xcodebuild", &["-version"], &[]);
    let first = version.lines().next().unwrap_or_default();
    let number = first
        .strip_prefix("Xcode ")
        .unwrap_or_else(|| panic!("xcodebuild -version: {version}"));
    assert!(
        number.starts_with("16."),
        "the qualified cell runs Xcode 16; this executor selects {number}"
    );
    number.to_owned()
}

fn other_xcode(default: &str) -> (String, PathBuf) {
    let mut found = Vec::new();
    for entry in std::fs::read_dir("/Applications").unwrap_or_else(|error| panic!("{error}")) {
        let path = entry.unwrap_or_else(|error| panic!("{error}")).path();
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        if !name.starts_with("Xcode") || !name.ends_with(".app") || path.is_symlink() {
            continue;
        }
        let plist = path.join("Contents/Info.plist");
        let version = tool(
            "/usr/libexec/PlistBuddy",
            &[
                "-c",
                "Print :CFBundleShortVersionString",
                &plist.to_string_lossy(),
            ],
            &[],
        );
        if version.starts_with("16.") && version != default {
            found.push((version, path));
        }
    }
    found.sort();
    found.into_iter().next().unwrap_or_else(|| {
        panic!("the qualified executor installs a second Xcode 16 beside {default}")
    })
}

fn keep(name: &str, text: &str) {
    if let Ok(dir) = std::env::var("TEST_UNDECLARED_OUTPUTS_DIR") {
        let test = std::thread::current().name().unwrap_or("main").to_owned();
        write(&Path::new(&dir).join(test).join(name), text);
    }
}

fn bazel(root: &Path, command: &str, targets: &[&str], flags: &[&str]) -> Run {
    let args: Vec<&str> = std::iter::once(command)
        .chain(flags.iter().copied())
        .chain(targets.iter().copied())
        .collect();
    let run = nested::bazel_in(root, &args);
    let name: String = [command]
        .iter()
        .chain(flags)
        .flat_map(|part| part.chars().chain(['-']))
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    keep(&format!("bazel-{name}log.txt"), &run.combined());
    run
}

fn report(path: &Path) -> BTreeMap<String, String> {
    let text = read(path);
    keep(
        &path.file_name().unwrap_or_default().to_string_lossy(),
        &text,
    );
    text.lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect()
}

fn field<'a>(report: &'a BTreeMap<String, String>, key: &str) -> &'a str {
    report
        .get(key)
        .map(String::as_str)
        .unwrap_or_else(|| panic!("report has no {key}: {report:?}"))
}

fn build_version(binary: &Path) -> String {
    let load = tool("otool", &["-l", &binary.to_string_lossy()], &[]);
    keep(
        &format!(
            "{}.otool.txt",
            binary.file_name().unwrap_or_default().to_string_lossy()
        ),
        &load,
    );
    load
}

fn ledger(xcode: &str) -> dx_nested::Case {
    let root = consumer("apple-macos-consumer", TOOLCHAINS, BAZELRC);
    write(&root.join("BUILD.bazel"), &BUILD.replace("XCODE", xcode));
    write(&root.join("ledger/BUILD.bazel"), LEDGER_BUILD);
    write(&root.join("ledger/ledger.h"), LEDGER_H);
    write(&root.join("ledger/ledger.c"), LEDGER_C);
    write(&root.join("ledger/main.c"), MAIN_C);
    write(&root.join("ledger/ledger_test.c"), LEDGER_TEST_C);
    root
}

#[test]
fn simulator_and_macos_cells_build_with_the_selected_xcode() {
    let xcode = installed_xcode();
    let developer_dir = tool("xcode-select", &["-p"], &[]);
    let root = ledger(&xcode);

    let sim = bazel(&root, "build", &["//..."], &["--config=sim"]);
    assert_eq!(sim.status.code(), Some(0), "{}", sim.combined());
    let probe = report(&root.join("bazel-bin/simulator_probe.txt"));
    let sdk_version = tool(
        "xcrun",
        &["--sdk", "iphonesimulator", "--show-sdk-version"],
        &[],
    );
    assert_eq!(field(&probe, "xcode"), xcode, "{probe:?}");
    assert_eq!(field(&probe, "developer_dir"), developer_dir, "{probe:?}");
    assert_eq!(field(&probe, "sdk"), "iphonesimulator", "{probe:?}");
    assert_eq!(field(&probe, "sdk_version"), sdk_version, "{probe:?}");
    assert_eq!(field(&probe, "deployment"), "17.0", "{probe:?}");
    assert_eq!(field(&probe, "arch"), "arm64", "{probe:?}");
    assert_eq!(field(&probe, "route"), "local", "{probe:?}");
    assert_eq!(field(&probe, "license"), "accepted", "{probe:?}");
    assert!(
        field(&probe, "compiler").starts_with("Apple clang"),
        "{probe:?}"
    );
    assert!(
        field(&probe, "sdk_path").starts_with(&developer_dir),
        "{probe:?}"
    );
    let archive = build_version(&root.join("bazel-bin/ledger/libledger.a"));
    assert!(archive.contains("platform 7"), "{archive}");
    assert!(archive.contains("minos 17.0"), "{archive}");
    assert!(archive.contains(&format!("sdk {sdk_version}")), "{archive}");

    let mac = bazel(&root, "build", &["//..."], &["--config=mac"]);
    assert_eq!(mac.status.code(), Some(0), "{}", mac.combined());
    let probe = report(&root.join("bazel-bin/mac_probe.txt"));
    let sdk_version = tool("xcrun", &["--sdk", "macosx", "--show-sdk-version"], &[]);
    assert_eq!(field(&probe, "xcode"), xcode, "{probe:?}");
    assert_eq!(field(&probe, "sdk"), "macosx", "{probe:?}");
    assert_eq!(field(&probe, "sdk_version"), sdk_version, "{probe:?}");
    assert_eq!(field(&probe, "deployment"), "14.0", "{probe:?}");
    assert!(
        field(&probe, "compiler").starts_with("Apple clang"),
        "{probe:?}"
    );
    let binary = root.join("bazel-bin/ledger/ledger_cli");
    let load = build_version(&binary);
    assert!(load.contains("platform 1"), "{load}");
    assert!(load.contains("minos 14.0"), "{load}");
    assert!(load.contains(&format!("sdk {sdk_version}")), "{load}");
    assert_eq!(
        tool(&binary.to_string_lossy(), &[], &[]),
        "macosx 3 entries 1375 cents"
    );

    let tested = bazel(&root, "test", &["//ledger:ledger_test"], &["--config=mac"]);
    assert_eq!(tested.status.code(), Some(0), "{}", tested.combined());
    assert!(
        tested.combined().contains("//ledger:ledger_test"),
        "{}",
        tested.combined()
    );
}

#[test]
fn a_second_installed_xcode_is_selected_and_never_substituted() {
    let default = installed_xcode();
    let default_dir = tool("xcode-select", &["-p"], &[]);
    let (xcode, app) = other_xcode(&default);
    let developer_dir = app.join("Contents/Developer");
    let developer = developer_dir.to_string_lossy().into_owned();
    let root = ledger(&xcode);
    let selected = format!("--xcode_version={xcode}");

    let sim = bazel(
        &root,
        "build",
        &["//:simulator_probe", "//ledger:ledger"],
        &["--config=sim", selected.as_str()],
    );
    assert_eq!(sim.status.code(), Some(0), "{}", sim.combined());
    let probe = report(&root.join("bazel-bin/simulator_probe.txt"));
    let sdk_version = tool(
        "xcrun",
        &["--sdk", "iphonesimulator", "--show-sdk-version"],
        &[("DEVELOPER_DIR", developer.as_str())],
    );
    assert_eq!(field(&probe, "xcode"), xcode, "{probe:?}");
    assert_eq!(field(&probe, "developer_dir"), developer, "{probe:?}");
    assert_ne!(developer, default_dir);
    assert_eq!(field(&probe, "sdk_version"), sdk_version, "{probe:?}");
    let archive = build_version(&root.join("bazel-bin/ledger/libledger.a"));
    assert!(archive.contains(&format!("sdk {sdk_version}")), "{archive}");

    let fallback = bazel(&root, "build", &["//:simulator_probe"], &["--config=sim"]);
    fails_with(
        &fallback,
        &format!("apple: provisioning selects Xcode {xcode} but Bazel resolved {default}"),
    );
}

#[test]
fn a_non_xcode_compiler_is_rejected() {
    let xcode = installed_xcode();
    let root = ledger(&xcode);
    write(&root.join("MODULE.bazel"), &module(""));

    let mac = bazel(&root, "build", &["//:mac_probe"], &["--config=mac"]);
    fails_with(&mac, "not Xcode's Apple clang");
}
