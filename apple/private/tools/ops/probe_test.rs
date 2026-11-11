//! Tests the toolchain probe.

mod fake;

use dx_apple_ops::probe::{Executor, run};
use dx_apple_ops::util::Output;
use dx_apple_ops::xcode::{Tools, XCRUN};
use dx_test_scratch::scratch;
use fake::{Fake, args, env, fail, ok};

const COMPILER: &str = "/xcode/clang";

fn mac() -> Executor {
    Executor {
        system: "Darwin".to_owned(),
        arch: "arm64".to_owned(),
        macos: "15.1.1".to_owned(),
        macos_build: "24B91".to_owned(),
    }
}

fn scripted(compiler: Output) -> Fake {
    Fake::new(move |cmd| {
        if cmd.program == COMPILER {
            return compiler.clone();
        }
        assert_eq!(cmd.program, XCRUN);
        match cmd.args.join(" ").as_str() {
            "xcodebuild -license check" => ok(""),
            "xcodebuild -version" => ok("Xcode 16.4\nBuild version 16F6\n"),
            "--sdk iphonesimulator --show-sdk-version" => ok("18.5\n"),
            "--sdk iphonesimulator --show-sdk-build-version" => ok("22F76\n"),
            "--sdk iphonesimulator --show-sdk-path" => ok("/sdk/iPhoneSimulator18.5.sdk\n"),
            "--sdk iphonesimulator --find clang" => ok("/xcode/clang\n"),
            other => panic!("unexpected {other}"),
        }
    })
}

fn clang() -> Output {
    ok("Apple clang version 17.0.0 (clang-1700.0.13.5)\nTarget: arm64-apple-darwin24.1.1\n")
}

fn probe(
    fake: &Fake,
    executor: &Executor,
    xcode: &str,
    sdk_version: &str,
    floor: &str,
) -> (Result<u8, String>, std::path::PathBuf, dx_test_scratch::TempDir) {
    let root = scratch("probe");
    let out = root.path().join("report.txt");
    let text = out.to_string_lossy().into_owned();
    let tools = Tools {
        developer_dir: None,
        runner: fake,
    };
    let outcome = run(
        &tools,
        executor,
        &args(&[
            &text,
            "managed",
            "macos_arm64",
            xcode,
            "iphonesimulator",
            sdk_version,
            "17.0",
            floor,
            COMPILER,
            "grpcs://mac.example.com",
        ]),
        &env(&[("DEVELOPER_DIR", "/xcode/Developer")]),
    );
    (outcome.map_err(|failure| failure.message), out, root)
}

fn rejected(
    fake: &Fake,
    executor: &Executor,
    xcode: &str,
    sdk_version: &str,
    floor: &str,
    message: &str,
) {
    let (outcome, out, _root) = probe(fake, executor, xcode, sdk_version, floor);
    assert_eq!(outcome, Err(message.to_owned()));
    assert!(!out.exists(), "a failed probe leaves no report");
}

#[test]
fn a_linux_executor_never_stands_in_for_macos() {
    let linux = Executor {
        system: "Linux".to_owned(),
        arch: "x86_64".to_owned(),
        ..Executor::default()
    };
    rejected(
        &scripted(clang()),
        &linux,
        "16.4.0.16F6",
        "18.5",
        "14.5",
        "probe ran on Linux x86_64, not a macOS executor; the managed route needs macos_arm64 with Xcode 16.4.0.16F6",
    );
}

#[test]
fn an_intel_executor_is_not_the_arm64_host() {
    let intel = Executor {
        arch: "x86_64".to_owned(),
        ..mac()
    };
    rejected(
        &scripted(clang()),
        &intel,
        "16.4.0.16F6",
        "18.5",
        "14.5",
        "executor is x86_64, not the arm64 host macos_arm64",
    );
}

#[test]
fn a_macos_below_the_floor_is_rejected() {
    rejected(
        &scripted(clang()),
        &mac(),
        "16.4.0.16F6",
        "18.5",
        "15.2",
        "executor runs macOS 15.1.1, below 15.2",
    );
}

#[test]
fn an_unaccepted_license_is_rejected() {
    let fake = Fake::new(|_| fail(69, "Agreeing to the Xcode license\n"));
    rejected(
        &fake,
        &mac(),
        "16.4.0.16F6",
        "18.5",
        "14.5",
        "the Xcode license is not accepted on this executor; an administrator runs 'sudo xcodebuild -license accept' there: Agreeing to the Xcode license",
    );
}

#[test]
fn another_xcode_version_is_rejected() {
    rejected(
        &scripted(clang()),
        &mac(),
        "16.2.0.16C5032a",
        "18.5",
        "14.5",
        "executor runs Xcode 16.4 (16F6) but Bazel resolved 16.2.0.16C5032a",
    );
}

#[test]
fn another_xcode_build_is_rejected() {
    rejected(
        &scripted(clang()),
        &mac(),
        "16.4.0.16F7",
        "18.5",
        "14.5",
        "executor runs Xcode build 16F6 but Bazel resolved 16.4.0.16F7",
    );
}

#[test]
fn a_missing_sdk_is_rejected() {
    let fake = Fake::new(|cmd| match cmd.args.join(" ").as_str() {
        "xcodebuild -license check" => ok(""),
        "xcodebuild -version" => ok("Xcode 16.4\nBuild version 16F6\n"),
        _ => fail(1, "unknown SDK"),
    });
    rejected(
        &fake,
        &mac(),
        "16.4.0.16F6",
        "18.5",
        "14.5",
        "Xcode 16.4 has no iphonesimulator SDK",
    );
}

#[test]
fn another_sdk_version_is_rejected() {
    rejected(
        &scripted(clang()),
        &mac(),
        "16.4.0.16F6",
        "18.2",
        "14.5",
        "executor has the iphonesimulator 18.5 SDK but Bazel resolved 18.2",
    );
}

#[test]
fn a_compiler_that_is_not_apple_clang_is_rejected() {
    rejected(
        &scripted(ok("gcc (GCC) 14.2.0\n")),
        &mac(),
        "16.4.0.16F6",
        "18.5",
        "14.5",
        "the C/C++ toolchain compiler /xcode/clang reports 'gcc (GCC) 14.2.0', not Xcode's Apple clang; register @local_config_apple_cc_toolchains//:all ahead of other C/C++ toolchains",
    );
}

#[test]
fn a_matching_executor_gets_its_report() {
    let fake = scripted(clang());
    let (outcome, out, _root) = probe(&fake, &mac(), "16.4.0.16F6", "18.5", "14.5");
    assert_eq!(outcome, Ok(0));
    assert_eq!(
        std::fs::read_to_string(&out).unwrap(),
        "route=managed\nendpoint=grpcs://mac.example.com\nhost=macos_arm64\narch=arm64\nmacos=15.1.1\nmacos_build=24B91\nxcode=16.4\nxcode_build=16F6\ndeveloper_dir=/xcode/Developer\nsdk=iphonesimulator\nsdk_version=18.5\nsdk_build=22F76\nsdk_path=/sdk/iPhoneSimulator18.5.sdk\ndeployment=17.0\nclang=/xcode/clang\ncompiler=Apple clang version 17.0.0 (clang-1700.0.13.5)\nlicense=accepted\n"
    );
    for call in fake.calls.borrow().iter() {
        assert!(call.program == XCRUN || call.program == COMPILER);
    }
}

#[test]
fn a_compiler_that_prints_to_stderr_is_read() {
    let fake = scripted(fail(0, "Apple clang version 17.0.0\n"));
    let (outcome, _, _root) = probe(&fake, &mac(), "16.4.0.16F6", "18.5", "14.5");
    assert_eq!(outcome, Ok(0));
}
