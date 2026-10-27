mod consumer;

use consumer::{consumer, fails_with, write};
use dx_nested::Case;
use dx_testing::{resolve_runfiles, Run};

const SECRET: &str = "tok-7f3a9c41e2";

const BUILD: &str = r#"load("@dx//apple/provisioning:defs.bzl", "apple_provisioning")
load("@dx//apple/provisioning:probe.bzl", "apple_toolchain_probe")
load("@xcode_support//xcode:xcode_config.bzl", "xcode_config")
load("@xcode_support//xcode:xcode_version.bzl", "xcode_version")

apple_provisioning(
    name = "local",
    accept_licenses = ["apple-xcode-license"],
    deployment = "17.0",
    host = "macos_arm64",
    route = "local",
    sdk = "iphonesimulator",
    xcode_version = "16.4",
)

apple_toolchain_probe(
    name = "local_probe",
    provisioning = ":local",
)

apple_provisioning(
    name = "managed",
    accept_licenses = ["apple-xcode-license"],
    credential_file = "secrets/mac.json",
    deployment = "17.0",
    endpoint = "grpcs://mac.example.com",
    host = "macos_arm64",
    route = "managed",
    sdk = "iphonesimulator",
    xcode_version = "16.4",
)

apple_toolchain_probe(
    name = "managed_probe",
    provisioning = ":managed",
)

xcode_version(
    name = "executor_xcode",
    aliases = ["16.4"],
    default_ios_sdk_version = "18.5",
    default_macos_sdk_version = "15.5",
    version = "16.4.0.16F6",
)

xcode_config(
    name = "executor_xcodes",
    default = ":executor_xcode",
    versions = [":executor_xcode"],
)
"#;

const BAZELRC: &str = "build:sim --platforms=@xcode_support//platforms:ios_sim_arm64\nbuild:sim --ios_minimum_os=17.0\n";

pub fn dx(root: &Case, command: &str, args: &[&str], bazel: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let mut argv: Vec<&str> = vec![command, "--workspace", workspace.as_str()];
    argv.extend_from_slice(args);
    if !bazel.is_empty() {
        argv.push("--");
        argv.extend_from_slice(bazel);
    }
    root.run(&resolve_runfiles(&rel), &argv, &[])
}

#[test]
fn unsupported_routes_fail_with_explicit_diagnostics_and_keep_secrets_out() {
    let root = consumer("apple-linux-consumer", "", BAZELRC);
    write(&root.join("BUILD.bazel"), BUILD);
    write(
        &root.join("secrets/mac.json"),
        &format!("{{\"token\": \"{SECRET}\"}}\n"),
    );
    let mut outputs = Vec::new();

    let local = dx(&root, "build", &["//:local_probe"], &["--config=sim"]);
    fails_with(
        &local,
        "apple: no Xcode resolved for this build; the local route needs a macos_arm64 host with Xcode 16.4 installed",
    );
    outputs.push(local.combined());

    let unconfigured = dx(&root, "build", &["//:managed_probe"], &["--config=sim"]);
    fails_with(
        &unconfigured,
        "apple: no Xcode resolved for this build; the managed route needs --xcode_version_config naming the Xcode on grpcs://mac.example.com",
    );
    outputs.push(unconfigured.combined());

    let no_executor = dx(
        &root,
        "build",
        &["//:managed_probe"],
        &["--config=sim", "--xcode_version_config=//:executor_xcodes"],
    );
    fails_with(
        &no_executor,
        "apple: no C/C++ toolchain targets iphonesimulator from this executor; the managed route needs a macos_arm64 execution platform with Xcode",
    );
    outputs.push(no_executor.combined());

    let host = dx(
        &root,
        "build",
        &["//:managed_probe"],
        &["--xcode_version_config=//:executor_xcodes"],
    );
    fails_with(
        &host,
        "apple: provisioning selects iphonesimulator but this build targets no Apple platform; build with --platforms=@@apple_support+//platforms:ios_sim_arm64",
    );
    outputs.push(host.combined());

    write(
        &root.join("BUILD.bazel"),
        &BUILD.replace(
            "    route = \"local\",\n",
            "    allow_download = True,\n    route = \"local\",\n",
        ),
    );
    let download = dx(&root, "build", &["//:local"], &[]);
    fails_with(
        &download,
        "apple: Xcode has no unattended download; use the local route or a managed endpoint",
    );
    outputs.push(download.combined());

    write(
        &root.join("BUILD.bazel"),
        &BUILD.replace(
            "    credential_file = \"secrets/mac.json\",\n",
            &format!("    api_token = \"{SECRET}\",\n"),
        ),
    );
    let inline = dx(&root, "build", &["//:managed"], &[]);
    fails_with(
        &inline,
        "apple: api_token must stay empty; pass credential_file instead",
    );
    fails_with(
        &inline,
        "apple: managed route needs credential_file; credentials never travel inline",
    );
    outputs.push(inline.combined());

    for output in outputs {
        assert!(!output.contains(SECRET), "{output}");
    }
}
