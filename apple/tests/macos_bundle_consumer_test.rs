mod consumer;
mod macos;

use std::path::Path;
use std::process::Command;

use consumer::{consumer, fails_with, read, write};
use dx_nested::Case;
use dx_testing::mkscratch;
use macos::{bazel, installed_xcode, keep, passes, tool};

const MODULE_EXTRA: &str = r#"
bazel_dep(name = "rules_rust", version = "0.75.0")
bazel_dep(name = "rules_apple", version = "5.2.0")
bazel_dep(name = "bazel_skylib", version = "1.9.2")

apple_cc_configure = use_extension("@xcode_support//crosstool:setup.bzl", "apple_cc_configure_extension")
use_repo(apple_cc_configure, "local_config_apple_cc_toolchains")

register_toolchains("@local_config_apple_cc_toolchains//:all")

rust = use_extension("@rules_rust//rust:extensions.bzl", "rust")
rust.toolchain(
    edition = "2021",
    extra_target_triples = ["aarch64-apple-darwin"],
    versions = ["1.98.1"],
)
use_repo(rust, "rust_toolchains")

register_toolchains("@rust_toolchains//:all")
"#;

const BAZELRC: &str = "build --macos_cpus=arm64\nbuild --//config:currency=EUR\n";

const OFFLINE: [&str; 3] = [
    "--nofetch",
    "--repository_disable_download",
    "--sandbox_default_allow_network=false",
];

const CONFIG_BUILD: &str = r#"load("@bazel_skylib//rules:common_settings.bzl", "string_flag")

package(default_visibility = ["//visibility:public"])

string_flag(
    name = "currency",
    build_setting_default = "USD",
    values = [
        "EUR",
        "USD",
    ],
)

config_setting(
    name = "eur",
    flag_values = {":currency": "EUR"},
)
"#;

const CORE_BUILD: &str = r#"load("@rules_rust//rust:defs.bzl", "rust_static_library")

rust_static_library(
    name = "ledger_core",
    srcs = ["ledger.rs"],
    edition = "2021",
    visibility = ["//visibility:public"],
)
"#;

const LEDGER_RS: &str = r#"use std::ffi::{c_char, CStr};

#[no_mangle]
pub extern "C" fn ledger_total(text: *const c_char) -> i64 {
    if text.is_null() {
        return -1;
    }
    let Ok(text) = unsafe { CStr::from_ptr(text) }.to_str() else {
        return -1;
    };
    let mut total = 0;
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let Some((_, cents)) = line.rsplit_once(' ') else {
            return -1;
        };
        let Ok(cents) = cents.parse::<i64>() else {
            return -1;
        };
        total += cents;
    }
    total
}
"#;

const NATIVE_BUILD: &str = r#"load("@rules_cc//cc:cc_library.bzl", "cc_library")

cc_library(
    name = "native",
    srcs = ["native.cc"],
    defines = select({
        "//config:eur": ["LEDGER_CURRENCY=\"EUR\""],
        "//conditions:default": ["LEDGER_CURRENCY=\"USD\""],
    }),
    visibility = ["//visibility:public"],
)
"#;

const NATIVE_CC: &str = r#"extern "C" const char *ledger_currency() {
    return LEDGER_CURRENCY;
}
"#;

const FMT_BUILD: &str = r#"load("@rules_apple//apple:macos.bzl", "macos_dynamic_framework")
load("@rules_cc//cc:objc_library.bzl", "objc_library")

objc_library(
    name = "fmt_lib",
    srcs = ["fmt.c"],
    alwayslink = True,
)

macos_dynamic_framework(
    name = "fmt",
    bundle_id = "com.example.ledger.fmt",
    bundle_name = "LedgerFmt",
    infoplists = ["fmt_Info.plist"],
    minimum_os_version = "14.0",
    visibility = ["//visibility:public"],
    deps = [":fmt_lib"],
)
"#;

const FMT_C: &str = r#"const char *ledger_fmt(void) {
    return "fmt-framework";
}
"#;

const PLUGIN_BUILD: &str = r#"load("@rules_apple//apple:macos.bzl", "macos_bundle")
load("@rules_cc//cc:objc_library.bzl", "objc_library")

objc_library(
    name = "plugin_lib",
    srcs = ["audit.c"],
    alwayslink = True,
)

macos_bundle(
    name = "plugin",
    bundle_id = "com.example.ledger.audit",
    bundle_name = "Audit",
    infoplists = ["plugin_Info.plist"],
    minimum_os_version = "14.0",
    visibility = ["//visibility:public"],
    deps = [":plugin_lib"],
)
"#;

const AUDIT_C: &str = r#"const char *ledger_audit(void) {
    return "audit-plugin";
}
"#;

const APP_BUILD: &str = r#"load("@dx//apple/macos:app.bzl", "apple_macos_app")

apple_macos_app(
    name = "ledger",
    srcs = ["main.cc"],
    bundle_id = "com.example.ledger",
    deployment = "14.0",
    frameworks = ["//ledger/fmt"],
    library = "//ledger/core:ledger_core",
    plugins = ["//ledger/plugin"],
    resources = ["entries.txt"],
    version = "2.3",
    deps = ["//ledger/native"],
)
"#;

const ENTRIES: &str = "rent 1000\nfood 375\n";

const MAIN_CC: &str = r#"#include <dlfcn.h>
#include <mach-o/dyld.h>

#include <climits>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <sstream>
#include <string>

extern "C" long long ledger_total(const char *text);
extern "C" const char *ledger_currency();
extern "C" const char *ledger_fmt(void);

int main() {
    char raw[PATH_MAX];
    uint32_t size = sizeof(raw);
    char real[PATH_MAX];
    if (_NSGetExecutablePath(raw, &size) != 0 || realpath(raw, real) == nullptr) {
        return 2;
    }
    std::string path(real);
    std::string macos = path.substr(0, path.rfind('/'));
    std::string contents = macos + "/..";
    std::ifstream file(contents + "/Resources/entries.txt");
    std::stringstream text;
    text << file.rdbuf();
    std::string body = text.str();
    std::string first = body.substr(0, body.find('\n'));
    std::printf("total=%lld\n", ledger_total(body.c_str()));
    std::printf("resource=%s\n", first.c_str());
    std::printf("framework=%s\n", ledger_fmt());
    std::printf("currency=%s\n", ledger_currency());
    std::string plugin = contents + "/PlugIns/Audit.bundle/Contents/MacOS/Audit";
    void *handle = dlopen(plugin.c_str(), RTLD_NOW);
    if (handle == nullptr) {
        std::printf("plugin-error=%s\n", dlerror());
        return 3;
    }
    auto audit = reinterpret_cast<const char *(*)(void)>(dlsym(handle, "ledger_audit"));
    if (audit == nullptr) {
        return 4;
    }
    std::printf("plugin=%s\n", audit());
    return 0;
}
"#;

const RAN: &str =
    "total=1375\nresource=rent 1000\nframework=fmt-framework\ncurrency=EUR\nplugin=audit-plugin\n";

fn plist_xml(kind: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n  <key>CFBundleExecutable</key>\n  <string>$(EXECUTABLE_NAME)</string>\n  <key>CFBundleIdentifier</key>\n  <string>$(PRODUCT_BUNDLE_IDENTIFIER)</string>\n  <key>CFBundleName</key>\n  <string>$(PRODUCT_NAME)</string>\n  <key>CFBundlePackageType</key>\n  <string>{kind}</string>\n</dict>\n</plist>\n"
    )
}

const BAD_BUILD: &str = r#"load("@dx//apple/macos:app.bzl", "apple_macos_app")
load("@rules_cc//cc:cc_import.bzl", "cc_import")

cc_import(
    name = "x86_import",
    static_library = "libx86.a",
)

cc_import(
    name = "extra_import",
    shared_library = "libextra.dylib",
)

cc_import(
    name = "host_import",
    shared_library = "libhost.dylib",
)

filegroup(
    name = "empty_plugin",
    srcs = [],
)

apple_macos_app(
    name = "wrong_slice",
    srcs = ["main.c"],
    bundle_id = "com.example.bad",
    deployment = "14.0",
    library = ":x86_import",
    version = "1.0",
)

apple_macos_app(
    name = "wrong_arch",
    srcs = ["main.c"],
    arch = "x86_64",
    bundle_id = "com.example.bad",
    deployment = "14.0",
    version = "1.0",
)

apple_macos_app(
    name = "unembedded",
    srcs = ["main.c"],
    bundle_id = "com.example.bad",
    deployment = "14.0",
    version = "1.0",
    deps = [":extra_import"],
)

apple_macos_app(
    name = "host_load",
    srcs = ["main.c"],
    bundle_id = "com.example.bad",
    deployment = "14.0",
    version = "1.0",
    deps = [":host_import"],
)

apple_macos_app(
    name = "missing_resource",
    srcs = ["main.c"],
    bundle_id = "com.example.bad",
    deployment = "14.0",
    resources = ["missing.txt"],
    version = "1.0",
)

apple_macos_app(
    name = "empty_plugin_app",
    srcs = ["main.c"],
    bundle_id = "com.example.bad",
    deployment = "14.0",
    plugins = [":empty_plugin"],
    version = "1.0",
)
"#;

fn ledger() -> Case {
    let root = consumer("apple-macos-bundle", MODULE_EXTRA, BAZELRC);
    write(&root.join("config/BUILD.bazel"), CONFIG_BUILD);
    write(&root.join("ledger/core/BUILD.bazel"), CORE_BUILD);
    write(&root.join("ledger/core/ledger.rs"), LEDGER_RS);
    write(&root.join("ledger/native/BUILD.bazel"), NATIVE_BUILD);
    write(&root.join("ledger/native/native.cc"), NATIVE_CC);
    write(&root.join("ledger/fmt/BUILD.bazel"), FMT_BUILD);
    write(&root.join("ledger/fmt/fmt.c"), FMT_C);
    write(&root.join("ledger/fmt/fmt_Info.plist"), &plist_xml("FMWK"));
    write(&root.join("ledger/plugin/BUILD.bazel"), PLUGIN_BUILD);
    write(&root.join("ledger/plugin/audit.c"), AUDIT_C);
    write(
        &root.join("ledger/plugin/plugin_Info.plist"),
        &plist_xml("BNDL"),
    );
    write(&root.join("ledger/app/BUILD.bazel"), APP_BUILD);
    write(&root.join("ledger/app/main.cc"), MAIN_CC);
    write(&root.join("ledger/app/entries.txt"), ENTRIES);
    root
}

fn output(program: &str, args: &[&str]) -> String {
    let output = Command::new(program)
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("{program}: {error}"));
    assert!(
        output.status.success(),
        "{program} {args:?}: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn plist(file: &Path, key: &str) -> String {
    tool(
        "plutil",
        &["-extract", key, "raw", "-o", "-", &file.to_string_lossy()],
    )
}

fn lipo(binary: &Path) -> String {
    tool("xcrun", &["lipo", "-archs", &binary.to_string_lossy()])
}

fn executable(bundle: &Path, name: &str) -> std::path::PathBuf {
    for base in [
        bundle.join("Versions/Current"),
        bundle.join("Contents"),
        bundle.to_path_buf(),
    ] {
        for candidate in [
            base.join("MacOS").join(name),
            base.join(name),
            base.join("Contents/MacOS").join(name),
        ] {
            if candidate.is_file() {
                return candidate;
            }
        }
    }
    panic!("no executable {name} under {bundle:?}");
}

fn identity_has(identity: &str, field: &str) {
    assert!(identity.contains(field), "want {field} in:\n{identity}");
}

/// Builds fully offline; `--sandbox_default_allow_network=false` covers sandboxed actions only.
#[test]
fn a_macos_bundle_builds_offline_and_runs_outside_the_trees() {
    installed_xcode();
    let root = ledger();

    passes(&bazel(&root, "fetch", &["//..."], &[]));
    passes(&bazel(&root, "build", &["//ledger/app:ledger"], &OFFLINE));

    let identity = read(&root.join("bazel-bin/ledger/app/ledger.app.identity.json"));
    keep("ledger.app.identity.json", &identity);
    identity_has(&identity, "\"bundle_id\": \"com.example.ledger\"");
    identity_has(&identity, "\"version\": \"2.3\"");
    identity_has(&identity, "\"deployment\": \"14.0\"");
    identity_has(&identity, "\"frameworks\": [\"LedgerFmt\"]");
    identity_has(&identity, "\"plugins\": [\"Audit.bundle\"]");
    identity_has(&identity, "\"signature\": \"adhoc\"");

    let scratch =
        mkscratch("apple-macos-bundle-run").unwrap_or_else(|error| panic!("scratch: {error}"));
    let zipped = scratch.join("ledger.zip");
    std::fs::copy(root.join("bazel-bin/ledger/app/ledger.zip"), &zipped)
        .unwrap_or_else(|error| panic!("copy zip: {error}"));
    tool(
        "ditto",
        &[
            "-x",
            "-k",
            &zipped.to_string_lossy(),
            &scratch.to_string_lossy(),
        ],
    );
    let app = scratch.join("ledger.app");
    let info = app.join("Contents/Info.plist");
    assert_eq!(plist(&info, "CFBundleIdentifier"), "com.example.ledger");
    assert_eq!(plist(&info, "LSMinimumSystemVersion"), "14.0");
    assert_eq!(plist(&info, "CFBundlePackageType"), "APPL");

    let binary = app.join("Contents/MacOS/ledger");
    let framework = executable(
        &app.join("Contents/Frameworks/LedgerFmt.framework"),
        "LedgerFmt",
    );
    let plugin = executable(&app.join("Contents/PlugIns/Audit.bundle"), "Audit");
    for file in [&binary, &framework, &plugin] {
        assert_eq!(lipo(file), "arm64", "{file:?}");
    }

    let loads = tool("xcrun", &["otool", "-l", &binary.to_string_lossy()]);
    keep("ledger.otool-l.txt", &loads);
    assert!(loads.contains("platform 1"), "{loads}");
    assert!(loads.contains("minos 14.0"), "{loads}");
    assert!(loads.contains("@executable_path/../Frameworks"), "{loads}");
    for file in [&binary, &framework, &plugin] {
        let linked = tool("xcrun", &["otool", "-L", &file.to_string_lossy()]);
        keep(
            &format!(
                "{}.otool-L.txt",
                file.file_name().unwrap_or_default().to_string_lossy()
            ),
            &linked,
        );
        for line in linked.lines().skip(1) {
            let load = line.trim().split(" (").next().unwrap_or_default();
            assert!(
                [
                    "/usr/lib/",
                    "/System/Library/",
                    "@rpath/",
                    "@executable_path/",
                    "@loader_path/"
                ]
                .iter()
                .any(|prefix| load.starts_with(prefix)),
                "{file:?} loads {load}"
            );
        }
    }
    let signature = output("codesign", &["-dv", &app.to_string_lossy()]);
    keep("ledger.codesign.txt", &signature);
    assert!(signature.contains("Signature=adhoc"), "{signature}");

    let mut command = Command::new(&binary);
    command.current_dir(&scratch);
    for (key, _) in std::env::vars() {
        if key.starts_with("DYLD_") {
            command.env_remove(key);
        }
    }
    let ran = command
        .output()
        .unwrap_or_else(|error| panic!("ledger: {error}"));
    let stdout = String::from_utf8_lossy(&ran.stdout).into_owned();
    keep(
        "ledger.run.txt",
        &format!(
            "exit={:?}\n{stdout}{}",
            ran.status.code(),
            String::from_utf8_lossy(&ran.stderr)
        ),
    );
    assert_eq!(ran.status.code(), Some(0), "{stdout}");
    assert_eq!(stdout, RAN);

    passes(&bazel(&root, "run", &["//ledger/app:ledger"], &OFFLINE));
}

#[test]
fn missing_parts_and_wrong_arch_fail_clearly() {
    installed_xcode();
    let root = ledger();
    let scratch =
        mkscratch("apple-macos-bundle-bad").unwrap_or_else(|error| panic!("scratch: {error}"));
    let bad = root.join("bad");
    write(&bad.join("BUILD.bazel"), BAD_BUILD);
    write(&bad.join("main.c"), "int main(void) { return 0; }\n");
    write(&scratch.join("x.c"), "int dx_part(void) { return 1; }\n");
    let source = scratch.join("x.c").to_string_lossy().into_owned();
    let object = scratch.join("x86.o").to_string_lossy().into_owned();
    tool(
        "xcrun",
        &["clang", "-arch", "x86_64", "-c", &source, "-o", &object],
    );
    tool(
        "xcrun",
        &[
            "libtool",
            "-static",
            "-o",
            &bad.join("libx86.a").to_string_lossy(),
            &object,
        ],
    );
    let host_name = scratch.join("libhost.dylib").to_string_lossy().into_owned();
    for (file, install_name) in [
        ("libhost.dylib", host_name.as_str()),
        ("libextra.dylib", "@rpath/libextra.dylib"),
    ] {
        tool(
            "xcrun",
            &[
                "clang",
                "-arch",
                "arm64",
                "-dynamiclib",
                "-install_name",
                install_name,
                &source,
                "-o",
                &bad.join(file).to_string_lossy(),
            ],
        );
    }

    let run = bazel(
        &root,
        "build",
        &[
            "//bad:wrong_slice",
            "//bad:wrong_arch",
            "//bad:unembedded",
            "//bad:host_load",
            "//bad:missing_resource",
            "//bad:empty_plugin_app",
        ],
        &["--keep_going"],
    );
    for needle in [
        "carries x86_64, want arm64",
        "arch 'x86_64' is outside macos: want arm64",
        "loads @rpath/libextra.dylib, which is missing from the bundle",
        "loads host runtime",
        "missing.txt",
        "places nothing under PlugIns",
    ] {
        fails_with(&run, needle);
    }
    for name in [
        "wrong_slice",
        "wrong_arch",
        "unembedded",
        "host_load",
        "missing_resource",
        "empty_plugin_app",
    ] {
        let zipped = root.join(format!("bazel-bin/bad/{name}.zip"));
        assert!(!zipped.exists(), "{zipped:?} was built");
    }
}
