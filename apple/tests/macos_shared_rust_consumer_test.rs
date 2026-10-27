mod consumer;
mod macos;

use std::path::{Path, PathBuf};

use consumer::{consumer, fails_with, read, write};
use dx_nested::{self as nested, Case};
use dx_testing::{mkscratch, run, Run};
use macos::{bazel, installed_xcode, keep, passed, passes, runtime_for, tool, wait_for, Simulator};

const POCKET_EXTRA: &str = r#"
bazel_dep(name = "rules_rust", version = "0.74.0")

apple_cc_configure = use_extension("@xcode_support//crosstool:setup.bzl", "apple_cc_configure_extension")
use_repo(apple_cc_configure, "local_config_apple_cc_toolchains")

register_toolchains("@local_config_apple_cc_toolchains//:all")

rust = use_extension("@rules_rust//rust:extensions.bzl", "rust")
rust.toolchain(
    edition = "2021",
    extra_target_triples = [
        "aarch64-apple-ios",
        "aarch64-apple-ios-sim",
    ],
    versions = ["1.98.0"],
)
use_repo(rust, "rust_toolchains")

register_toolchains("@rust_toolchains//:all")
"#;

const DESK_EXTRA: &str = r#"
bazel_dep(name = "rules_apple", version = "5.2.0", repo_name = "apple_rules")

apple_cc_configure = use_extension("@xcode_support//crosstool:setup.bzl", "apple_cc_configure_extension")
use_repo(apple_cc_configure, "local_config_apple_cc_toolchains")

register_toolchains("@local_config_apple_cc_toolchains//:all")
"#;

const BAZELRC: &str = "build --ios_multi_cpus=sim_arm64\nbuild:release --compilation_mode=opt\n";

const RUSTC: &str = "rustc 1.98.0";

const POCKET_ID: &str = "com.example.pocket";

const FRAMEWORK_ID: &str = "com.example.pockettasks";

const POCKET_BUILD: &str = r#"load("@dx//apple/ios:app.bzl", "apple_ios_app", "apple_ios_app_test", "apple_ios_framework")
load("@dx//apple/ios:simulator.bzl", "apple_simulator_boot", "apple_simulator_launch")
load("@rules_cc//cc:objc_library.bzl", "objc_library")
load("@rules_rust//rust:defs.bzl", "rust_binary", "rust_static_library")

rust_binary(
    name = "pocket_cli",
    srcs = ["pocket_cli.rs"],
    crate_features = ["pocket_board"],
    edition = "2021",
    rustc_flags = ["--cfg=consumer_apple"],
    deps = ["@dx//examples/shared-rust:tasks_core"],
)

rust_static_library(
    name = "pocket_rust",
    srcs = ["pocket.rs"],
    crate_features = ["pocket_board"],
    edition = "2021",
    rustc_flags = ["--cfg=consumer_apple"],
    deps = ["@dx//examples/shared-rust:tasks_core"],
)

objc_library(
    name = "pocket_headers",
    hdrs = [
        "PocketAppDelegate.h",
        "pocket_api.h",
    ],
)

apple_ios_app(
    name = "pocket_app",
    srcs = [
        "PocketAppDelegate.m",
        "main.m",
    ],
    arch = "arm64",
    bundle_id = "com.example.pocket",
    deployment = "17.0",
    environment = "simulator",
    library = ":pocket_rust",
    resources = [
        "board.txt",
        "notes.txt",
    ],
    version = "3.1",
    xcode_version = "XCODE",
    deps = [":pocket_headers"],
)

apple_ios_app_test(
    name = "pocket_app_test",
    size = "large",
    srcs = ["PocketAppTests.m"],
    app = ":pocket_app",
    deployment = "17.0",
    device_type = "iPhone 16",
    os_version = "RUNTIME",
    deps = [":pocket_headers"],
)

apple_ios_framework(
    name = "pocket_framework",
    bundle_id = "com.example.pockettasks",
    bundle_name = "PocketTasks",
    deployment = "17.0",
    hdrs = ["pocket_api.h"],
    library = ":pocket_rust",
    resources = [
        "board.txt",
        "notes.txt",
    ],
    slices = [
        "device/arm64",
        "simulator/arm64",
    ],
    version = "3.1",
    xcode_version = "XCODE",
)

apple_simulator_boot(
    name = "boot",
    simulator = "SIMULATOR",
)

apple_simulator_launch(
    name = "launch",
    bundle_id = "com.example.pocket",
    simulator = "SIMULATOR",
)

apple_ios_app(
    name = "pocket_device_app",
    srcs = [
        "PocketAppDelegate.m",
        "main.m",
    ],
    arch = "arm64",
    bundle_id = "com.example.pocket",
    deployment = "17.0",
    environment = "device",
    library = ":pocket_rust",
    resources = [
        "board.txt",
        "notes.txt",
    ],
    tags = ["manual"],
    version = "3.1",
    xcode_version = "XCODE",
    deps = [":pocket_headers"],
)

apple_ios_app(
    name = "pocket_export_app",
    srcs = [
        "PocketAppDelegate.m",
        "main.m",
    ],
    arch = "arm64",
    bundle_id = "com.example.pocket",
    deployment = "17.0",
    environment = "simulator",
    export_ipa = True,
    library = ":pocket_rust",
    tags = ["manual"],
    version = "3.1",
    xcode_version = "XCODE",
    deps = [":pocket_headers"],
)
"#;

const POCKET_RS: &str = r#"//! Consumer-owned iOS adapter over the shared task board.

#[cfg(not(all(consumer_apple, feature = "pocket_board")))]
compile_error!("consumer features and rustc flags did not reach the adapter");

use std::ffi::{c_char, CStr, CString};
use std::sync::{Mutex, MutexGuard};

use shared_tasks::{asset_report, reply, Board, CORE_VERSION};

static BOARD: Mutex<Option<Board>> = Mutex::new(None);

fn board() -> MutexGuard<'static, Option<Board>> {
    BOARD.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn owned(value: String) -> *mut c_char {
    CString::new(value).map_or(std::ptr::null_mut(), CString::into_raw)
}

unsafe fn text<'a>(value: *const c_char) -> Option<&'a str> {
    if value.is_null() {
        return None;
    }
    CStr::from_ptr(value).to_str().ok()
}

#[no_mangle]
pub unsafe extern "C" fn pocket_load(board_text: *const c_char) -> i32 {
    let Some(loaded) = text(board_text).and_then(|text| Board::load(text).ok()) else {
        return -1;
    };
    let count = i32::try_from(loaded.tasks().len()).unwrap_or(i32::MAX);
    *board() = Some(loaded);
    count
}

#[no_mangle]
pub unsafe extern "C" fn pocket_apply(command: *const c_char) -> *mut c_char {
    let mut guard = board();
    match (text(command), guard.as_mut()) {
        (Some(command), Some(current)) => owned(reply(current, command)),
        _ => owned("err:no_board".to_owned()),
    }
}

#[no_mangle]
pub extern "C" fn pocket_summary() -> *mut c_char {
    match board().as_ref() {
        Some(current) => owned(current.summary().to_string()),
        None => owned("err:no_board".to_owned()),
    }
}

#[no_mangle]
pub extern "C" fn pocket_saved() -> *mut c_char {
    match board().as_ref() {
        Some(current) => owned(current.save()),
        None => owned(String::new()),
    }
}

#[no_mangle]
pub unsafe extern "C" fn pocket_asset(name: *const c_char, data: *const u8, len: usize) -> *mut c_char {
    let Some(name) = text(name) else {
        return owned("err:bad_asset".to_owned());
    };
    if data.is_null() && len != 0 {
        return owned("err:bad_asset".to_owned());
    }
    let bytes = if data.is_null() {
        &[][..]
    } else {
        std::slice::from_raw_parts(data, len)
    };
    owned(asset_report(name, bytes))
}

#[no_mangle]
pub extern "C" fn pocket_build() -> *mut c_char {
    let platform = if cfg!(all(target_os = "ios", target_abi = "sim")) {
        "iphonesimulator"
    } else if cfg!(target_os = "ios") {
        "iphoneos"
    } else {
        "other"
    };
    let profile = if cfg!(debug_assertions) { "debug" } else { "release" };
    owned(format!(
        "core={CORE_VERSION} platform={platform} profile={profile} features=pocket_board"
    ))
}

#[no_mangle]
pub unsafe extern "C" fn pocket_free(value: *mut c_char) {
    if !value.is_null() {
        drop(CString::from_raw(value));
    }
}
"#;

const POCKET_CLI_RS: &str = r#"//! Consumer-owned macOS CLI over the shared task board.

#[cfg(not(all(consumer_apple, feature = "pocket_board")))]
compile_error!("consumer features and rustc flags did not reach the CLI");

use std::path::Path;
use std::process::ExitCode;

use shared_tasks::{asset_report, reply, Board, CORE_VERSION};

fn read(path: &str) -> Result<Vec<u8>, ExitCode> {
    std::fs::read(path).map_err(|error| {
        eprintln!("err:unreadable: {path}: {error}");
        ExitCode::from(2)
    })
}

fn run(args: &[String]) -> Result<bool, ExitCode> {
    let mut assets = Vec::new();
    let mut rest = Vec::new();
    for arg in args {
        match arg.strip_prefix("--asset=") {
            Some(path) => assets.push(path),
            None => rest.push(arg.as_str()),
        }
    }
    let Some((board_path, commands)) = rest.split_first() else {
        eprintln!("usage: pocket_cli [--asset=PATH]... BOARD [COMMAND]...");
        return Err(ExitCode::from(2));
    };
    let text = String::from_utf8(read(board_path)?).map_err(|_| {
        eprintln!("err:not_utf8: {board_path}");
        ExitCode::from(2)
    })?;
    let mut board = Board::load(&text).map_err(|error| {
        eprintln!("err:{}: {error}", error.kind());
        ExitCode::from(2)
    })?;
    let platform = if cfg!(target_os = "macos") { "macos" } else { "other" };
    let profile = if cfg!(debug_assertions) { "debug" } else { "release" };
    println!("core={CORE_VERSION} platform={platform} profile={profile} features=pocket_board");
    println!("loaded={}", board.tasks().len());
    let replies: Vec<String> = commands
        .iter()
        .map(|command| reply(&mut board, command))
        .collect();
    println!("replies={}", replies.join(";"));
    println!("summary={}", board.summary());
    for path in assets {
        let bytes = read(path)?;
        let name = Path::new(path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(path);
        println!("{}", asset_report(name, &bytes));
    }
    Ok(replies.iter().all(|reply| reply.starts_with("ok:")))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(code) => code,
    }
}
"#;

const POCKET_H: &str = r#"#pragma once

#include <stddef.h>
#include <stdint.h>

int32_t pocket_load(const char *board);
char *pocket_apply(const char *command);
char *pocket_summary(void);
char *pocket_saved(void);
char *pocket_asset(const char *name, const uint8_t *data, size_t len);
char *pocket_build(void);
void pocket_free(char *value);
"#;

const DELEGATE_H: &str = r#"#import <UIKit/UIKit.h>

@interface PocketAppDelegate : UIResponder <UIApplicationDelegate>
@property(nonatomic, strong) UIWindow *window;
- (int32_t)loadBoard;
- (NSString *)apply:(NSString *)command;
- (NSString *)summary;
- (NSString *)saved;
- (NSString *)notesReport;
- (NSString *)build;
- (NSURL *)stateURL;
@end
"#;

const DELEGATE_M: &str = r#"#import "pocket/PocketAppDelegate.h"

#import "pocket/pocket_api.h"

static NSString *Take(char *value) {
  if (value == NULL) {
    return nil;
  }
  NSString *text = [NSString stringWithUTF8String:value];
  pocket_free(value);
  return text;
}

@implementation PocketAppDelegate {
  UILabel *_label;
}

- (int32_t)loadBoard {
  NSString *path = [[NSBundle mainBundle] pathForResource:@"board" ofType:@"txt"];
  NSString *text = path == nil ? nil : [NSString stringWithContentsOfFile:path
                                                                  encoding:NSUTF8StringEncoding
                                                                     error:NULL];
  return text == nil ? -2 : pocket_load(text.UTF8String);
}

- (NSString *)apply:(NSString *)command {
  return Take(pocket_apply(command.UTF8String));
}

- (NSString *)summary {
  return Take(pocket_summary());
}

- (NSString *)saved {
  return Take(pocket_saved());
}

- (NSString *)notesReport {
  NSString *path = [[NSBundle mainBundle] pathForResource:@"notes" ofType:@"txt"];
  NSData *data = path == nil ? nil : [NSData dataWithContentsOfFile:path];
  if (data == nil) {
    return @"missing notes.txt";
  }
  return Take(pocket_asset("notes.txt", data.bytes, data.length));
}

- (NSString *)build {
  return Take(pocket_build());
}

- (NSURL *)stateURL {
  NSURL *documents = [[NSFileManager defaultManager] URLsForDirectory:NSDocumentDirectory
                                                            inDomains:NSUserDomainMask]
                         .firstObject;
  return [documents URLByAppendingPathComponent:@"pocket-state.txt"];
}

- (BOOL)application:(UIApplication *)application
    didFinishLaunchingWithOptions:(NSDictionary *)launchOptions {
  int32_t loaded = [self loadBoard];
  NSArray<NSString *> *replies = @[
    [self apply:@"add Ship the simulator build"],
    [self apply:@"done 1"],
    [self apply:@"done 9"],
  ];
  NSString *state = [NSString stringWithFormat:@"%@\nloaded=%d\nreplies=%@\nsummary=%@\n%@\n",
                                               [self build], loaded,
                                               [replies componentsJoinedByString:@";"],
                                               [self summary], [self notesReport]];
  [state writeToURL:self.stateURL atomically:YES encoding:NSUTF8StringEncoding error:NULL];
  UIViewController *root = [[UIViewController alloc] init];
  _label = [[UILabel alloc] initWithFrame:CGRectMake(20, 80, 280, 40)];
  _label.text = [self summary];
  [root.view addSubview:_label];
  self.window = [[UIWindow alloc] initWithFrame:[[UIScreen mainScreen] bounds]];
  self.window.rootViewController = root;
  [self.window makeKeyAndVisible];
  return YES;
}

@end
"#;

const MAIN_M: &str = r#"#import <UIKit/UIKit.h>

#import "pocket/PocketAppDelegate.h"

int main(int argc, char *argv[]) {
  @autoreleasepool {
    return UIApplicationMain(argc, argv, nil, NSStringFromClass([PocketAppDelegate class]));
  }
}
"#;

const TESTS_M: &str = r#"#import <XCTest/XCTest.h>

#import "pocket/PocketAppDelegate.h"
#import "pocket/pocket_api.h"

@interface PocketAppTests : XCTestCase
@end

@implementation PocketAppTests

- (void)testSharedCoreRepliesLikeTheNativeCli {
  PocketAppDelegate *pocket = [[PocketAppDelegate alloc] init];
  XCTAssertEqual([pocket loadBoard], 2);
  XCTAssertEqualObjects([pocket apply:@"add Ship the simulator build"], @"ok:added 3");
  XCTAssertEqualObjects([pocket apply:@"done 1"], @"ok:done 1");
  XCTAssertEqualObjects([pocket apply:@"done 9"], @"err:no_such_task");
  XCTAssertEqualObjects([pocket apply:@"attach 3 ../escape.txt"], @"err:bad_asset");
  XCTAssertEqualObjects([pocket summary], @"tasks=3 done=2 open=1 assets=1");
  XCTAssertEqualObjects([pocket saved], @"[x] Review the shared core @notes.txt\n"
                                         @"[x] Wire the iOS consumer\n"
                                         @"[ ] Ship the simulator build\n");
}

- (void)testInvalidBoardKeepsTheLoadedOne {
  PocketAppDelegate *pocket = [[PocketAppDelegate alloc] init];
  XCTAssertEqual([pocket loadBoard], 2);
  XCTAssertEqual(pocket_load("no marker"), -1);
  XCTAssertEqualObjects([pocket summary], @"tasks=2 done=1 open=1 assets=1");
}

- (void)testBundledNotesDigestMatchesTheNativeCli {
  PocketAppDelegate *pocket = [[PocketAppDelegate alloc] init];
  XCTAssertEqualObjects([pocket notesReport], @"asset notes.txt bytes=20 adler32:4fce0732");
}

- (void)testConsumerFeaturesReachTheSimulatorBuild {
  NSString *build = [[[PocketAppDelegate alloc] init] build];
  XCTAssertTrue([build hasPrefix:@"core=2.0.0 platform=iphonesimulator profile="], @"%@", build);
  XCTAssertTrue([build hasSuffix:@" features=pocket_board"], @"%@", build);
}

- (void)testHostAppWroteItsLaunchState {
  id delegate = UIApplication.sharedApplication.delegate;
  XCTAssertTrue([delegate isKindOfClass:[PocketAppDelegate class]]);
  NSString *state = [NSString stringWithContentsOfURL:[delegate stateURL]
                                             encoding:NSUTF8StringEncoding
                                                error:NULL];
  XCTAssertTrue([state containsString:@"\nloaded=2\nreplies=ok:added 3;ok:done 1;err:no_such_task\n"], @"%@", state);
  XCTAssertTrue([state hasSuffix:@"\nsummary=tasks=3 done=2 open=1 assets=1\nasset notes.txt bytes=20 adler32:4fce0732\n"], @"%@", state);
}

@end
"#;

const BOARD: &str =
    "# pocket board\n[ ] Review the shared core @notes.txt\n[x] Wire the iOS consumer\n";

const NOTES: &str = "Pocket-owned notes.\n";

const RELEASE_STATE: &str = "core=2.0.0 platform=iphonesimulator profile=release features=pocket_board\nloaded=2\nreplies=ok:added 3;ok:done 1;err:no_such_task\nsummary=tasks=3 done=2 open=1 assets=1\nasset notes.txt bytes=20 adler32:4fce0732\n";

const NATIVE_STATE: &str = "core=2.0.0 platform=macos profile=release features=pocket_board\nloaded=2\nreplies=ok:added 3;ok:done 1;err:no_such_task\nsummary=tasks=3 done=2 open=1 assets=1\nasset notes.txt bytes=20 adler32:4fce0732\n";

const DESK_BUILD: &str = r#"load("@apple_rules//apple:apple.bzl", "apple_dynamic_xcframework_import")
load("@dx//apple/ios:app.bzl", "apple_ios_app", "apple_ios_app_test")
load("@dx//apple/ios:simulator.bzl", "apple_simulator_boot", "apple_simulator_install", "apple_simulator_launch")
load("@rules_cc//cc:objc_library.bzl", "objc_library")

apple_dynamic_xcframework_import(
    name = "pocket_tasks",
    xcframework_imports = glob(["PocketTasks.xcframework/**"]),
)

objc_library(
    name = "desk_headers",
    hdrs = ["DeskAppDelegate.h"],
)

apple_ios_app(
    name = "desk_app",
    srcs = [
        "DeskAppDelegate.m",
        "main.m",
    ],
    arch = "arm64",
    bundle_id = "com.example.desk",
    deployment = "17.0",
    environment = "simulator",
    frameworks = [":pocket_tasks"],
    version = "1.2",
    xcode_version = "XCODE",
    deps = [":desk_headers"],
)

apple_ios_app_test(
    name = "desk_app_test",
    size = "large",
    srcs = ["DeskAppTests.m"],
    app = ":desk_app",
    deployment = "17.0",
    device_type = "iPhone 16",
    os_version = "RUNTIME",
    deps = [
        ":desk_headers",
        ":pocket_tasks",
    ],
)

apple_simulator_boot(
    name = "boot",
    simulator = "SIMULATOR",
)

apple_simulator_install(
    name = "install",
    app = ":desk_app",
    simulator = "SIMULATOR",
)

apple_simulator_launch(
    name = "launch",
    bundle_id = "com.example.desk",
    simulator = "SIMULATOR",
)
"#;

const DESK_DELEGATE_H: &str = r#"#import <UIKit/UIKit.h>

@interface DeskAppDelegate : UIResponder <UIApplicationDelegate>
@property(nonatomic, strong) UIWindow *window;
- (NSBundle *)tasksBundle;
- (int32_t)loadBoard;
- (NSString *)apply:(NSString *)command;
- (NSString *)summary;
- (NSString *)notesReport;
- (NSString *)build;
- (NSURL *)stateURL;
@end
"#;

const DESK_DELEGATE_M: &str = r#"#import "desk/DeskAppDelegate.h"

#import <PocketTasks/pocket_api.h>

static NSString *Take(char *value) {
  if (value == NULL) {
    return nil;
  }
  NSString *text = [NSString stringWithUTF8String:value];
  pocket_free(value);
  return text;
}

@implementation DeskAppDelegate

- (NSBundle *)tasksBundle {
  return [NSBundle bundleWithIdentifier:@"com.example.pockettasks"];
}

- (int32_t)loadBoard {
  NSString *path = [self.tasksBundle pathForResource:@"board" ofType:@"txt"];
  NSString *text = path == nil ? nil : [NSString stringWithContentsOfFile:path
                                                                  encoding:NSUTF8StringEncoding
                                                                     error:NULL];
  return text == nil ? -2 : pocket_load(text.UTF8String);
}

- (NSString *)apply:(NSString *)command {
  return Take(pocket_apply(command.UTF8String));
}

- (NSString *)summary {
  return Take(pocket_summary());
}

- (NSString *)notesReport {
  NSString *path = [self.tasksBundle pathForResource:@"notes" ofType:@"txt"];
  NSData *data = path == nil ? nil : [NSData dataWithContentsOfFile:path];
  if (data == nil) {
    return @"missing notes.txt";
  }
  return Take(pocket_asset("notes.txt", data.bytes, data.length));
}

- (NSString *)build {
  return Take(pocket_build());
}

- (NSURL *)stateURL {
  NSURL *documents = [[NSFileManager defaultManager] URLsForDirectory:NSDocumentDirectory
                                                            inDomains:NSUserDomainMask]
                         .firstObject;
  return [documents URLByAppendingPathComponent:@"desk-state.txt"];
}

- (BOOL)application:(UIApplication *)application
    didFinishLaunchingWithOptions:(NSDictionary *)launchOptions {
  int32_t loaded = [self loadBoard];
  NSArray<NSString *> *replies = @[
    [self apply:@"add Ship the framework build"],
    [self apply:@"remove 2"],
  ];
  NSString *state = [NSString stringWithFormat:@"%@\nloaded=%d\nreplies=%@\nsummary=%@\n%@\n",
                                               [self build], loaded,
                                               [replies componentsJoinedByString:@";"],
                                               [self summary], [self notesReport]];
  [state writeToURL:self.stateURL atomically:YES encoding:NSUTF8StringEncoding error:NULL];
  UIViewController *root = [[UIViewController alloc] init];
  self.window = [[UIWindow alloc] initWithFrame:[[UIScreen mainScreen] bounds]];
  self.window.rootViewController = root;
  [self.window makeKeyAndVisible];
  return YES;
}

@end
"#;

const DESK_MAIN_M: &str = r#"#import <UIKit/UIKit.h>

#import "desk/DeskAppDelegate.h"

int main(int argc, char *argv[]) {
  @autoreleasepool {
    return UIApplicationMain(argc, argv, nil, NSStringFromClass([DeskAppDelegate class]));
  }
}
"#;

const DESK_TESTS_M: &str = r#"#import <XCTest/XCTest.h>

#import <PocketTasks/pocket_api.h>

#import "desk/DeskAppDelegate.h"

@interface DeskAppTests : XCTestCase
@end

@implementation DeskAppTests

- (void)testFrameworkCoreAppliesCommandsToItsBundledBoard {
  DeskAppDelegate *desk = [[DeskAppDelegate alloc] init];
  XCTAssertEqual([desk loadBoard], 2);
  XCTAssertEqualObjects([desk apply:@"add Ship the framework build"], @"ok:added 3");
  XCTAssertEqualObjects([desk apply:@"remove 2"], @"ok:removed 2");
  XCTAssertEqualObjects([desk apply:@"reopen 1"], @"err:not_done");
  XCTAssertEqualObjects([desk summary], @"tasks=2 done=0 open=2 assets=1");
}

- (void)testFrameworkBundleCarriesTheSharedResources {
  DeskAppDelegate *desk = [[DeskAppDelegate alloc] init];
  NSBundle *tasks = desk.tasksBundle;
  XCTAssertNotNil(tasks);
  XCTAssertTrue([tasks.bundlePath hasSuffix:@"Frameworks/PocketTasks.framework"], @"%@", tasks.bundlePath);
  XCTAssertEqualObjects([desk notesReport], @"asset notes.txt bytes=20 adler32:4fce0732");
}

- (void)testFrameworkKeepsTheProducerReleaseBuild {
  XCTAssertEqualObjects([[[DeskAppDelegate alloc] init] build],
                        @"core=2.0.0 platform=iphonesimulator profile=release features=pocket_board");
}

@end
"#;

const DESK_STATE: &str = "core=2.0.0 platform=iphonesimulator profile=release features=pocket_board\nloaded=2\nreplies=ok:added 3;ok:removed 2\nsummary=tasks=2 done=0 open=2 assets=1\nasset notes.txt bytes=20 adler32:4fce0732\n";

const SYMBOLS: [&str; 7] = [
    "_pocket_load",
    "_pocket_apply",
    "_pocket_summary",
    "_pocket_saved",
    "_pocket_asset",
    "_pocket_build",
    "_pocket_free",
];

fn container(sim: &Simulator, bundle_id: &str, kind: &str) -> PathBuf {
    PathBuf::from(tool(
        "xcrun",
        &["simctl", "get_app_container", &sim.udid, bundle_id, kind],
    ))
}

fn unzip(archive: &Path, into: &Path) {
    std::fs::create_dir_all(into).unwrap_or_else(|error| panic!("mkdir {into:?}: {error}"));
    tool(
        "unzip",
        &[
            "-q",
            &archive.to_string_lossy(),
            "-d",
            &into.to_string_lossy(),
        ],
    );
}

fn plist(file: &Path, key: &str) -> String {
    tool(
        "plutil",
        &["-extract", key, "raw", "-o", "-", &file.to_string_lossy()],
    )
}

fn field(loads: &str, name: &str) -> Vec<String> {
    let mut found: Vec<String> = loads
        .lines()
        .map(str::split_whitespace)
        .filter_map(|mut words| match (words.next(), words.next()) {
            (Some(key), Some(value)) if key == name => Some(value.to_owned()),
            _ => None,
        })
        .collect();
    found.sort();
    found.dedup();
    found
}
fn rustc_identity(root: &Path) -> String {
    const RUSTC_FILES: &str = "@rules_rust//rust/toolchain:current_rustc_files";
    passes(&bazel(root, "build", &[RUSTC_FILES], &[]));
    let query = bazel(root, "cquery", &[RUSTC_FILES], &["--output=files"]);
    passes(&query);
    let file = query
        .stdout
        .lines()
        .next_back()
        .unwrap_or_else(|| panic!("no consumer rustc: {}", query.combined()));
    let info = nested::bazel_in(root, &["info", "execution_root"]);
    passes(&info);
    let execroot = info.stdout.trim();
    tool(
        &Path::new(execroot).join(file).to_string_lossy(),
        &["--version"],
    )
}

fn executor(xcode: &str) -> String {
    format!(
        "host_os=macOS {} ({})\nhost_arch={}\nxcode={xcode} ({})\nsdk_iphoneos={}\nsdk_iphonesimulator={}\n",
        tool("sw_vers", &["-productVersion"]),
        tool("sw_vers", &["-buildVersion"]),
        tool("uname", &["-m"]),
        tool("xcodebuild", &["-version"])
            .lines()
            .last()
            .unwrap_or_default()
            .trim_start_matches("Build version "),
        tool("xcrun", &["--sdk", "iphoneos", "--show-sdk-version"]),
        tool("xcrun", &["--sdk", "iphonesimulator", "--show-sdk-version"]),
    )
}

fn pocket(xcode: &str, runtime: &str, simulator: &str) -> Case {
    let root = consumer("apple-shared-rust", POCKET_EXTRA, BAZELRC);
    write(&root.join("BUILD.bazel"), "");
    write(
        &root.join("pocket/BUILD.bazel"),
        &POCKET_BUILD
            .replace("XCODE", xcode)
            .replace("RUNTIME", runtime)
            .replace("SIMULATOR", simulator),
    );
    write(&root.join("pocket/pocket.rs"), POCKET_RS);
    write(&root.join("pocket/pocket_cli.rs"), POCKET_CLI_RS);
    write(&root.join("pocket/pocket_api.h"), POCKET_H);
    write(&root.join("pocket/PocketAppDelegate.h"), DELEGATE_H);
    write(&root.join("pocket/PocketAppDelegate.m"), DELEGATE_M);
    write(&root.join("pocket/main.m"), MAIN_M);
    write(&root.join("pocket/PocketAppTests.m"), TESTS_M);
    write(&root.join("pocket/board.txt"), BOARD);
    write(&root.join("pocket/notes.txt"), NOTES);
    assert!(
        !root.join("pocket/core").exists()
            && !POCKET_RS.contains("fn apply(")
            && !POCKET_CLI_RS.contains("fn apply("),
        "the consumer reuses the shared core instead of copying it"
    );
    root
}

fn inspect_binary(binary: &Path, arch: &str, platform: &str) -> String {
    let path = binary.to_string_lossy().into_owned();
    let loads = tool("xcrun", &["otool", "-arch", arch, "-l", &path]);
    assert_eq!(field(&loads, "platform"), [platform], "{loads}");
    assert_eq!(field(&loads, "minos"), ["17.0"], "{loads}");
    let symbols = tool("xcrun", &["nm", "-gU", "-arch", arch, &path]);
    for symbol in SYMBOLS {
        assert!(
            symbols.contains(&format!(" {symbol}")),
            "{symbol} missing:\n{symbols}"
        );
    }
    loads
}

fn pocket_cli(binary: &Path, args: &[&str]) -> (Run, String) {
    let shown = format!("pocket_cli {}", args.join(" "));
    let ran = run(binary, args, &[]).unwrap_or_else(|error| panic!("{shown}: {error}"));
    keep(
        &format!("native-{}.txt", args.len()),
        &format!(
            "$ {shown}\nexit={:?}\n{}",
            ran.status.code(),
            ran.combined()
        ),
    );
    (ran, shown)
}

#[test]
fn the_shared_core_runs_as_a_relocated_native_macos_release_cli() {
    let xcode = installed_xcode();
    let sdk_version = tool("xcrun", &["--sdk", "iphonesimulator", "--show-sdk-version"]);
    let runtime = runtime_for(&sdk_version);
    let root = pocket(&xcode, &runtime, "unused");

    passes(&bazel(
        &root,
        "build",
        &["//pocket:pocket_cli"],
        &["--config=release"],
    ));
    let relocated =
        mkscratch("apple-shared-rust-native").unwrap_or_else(|error| panic!("scratch: {error}"));
    let binary = relocated.join("pocket_cli");
    std::fs::copy(root.join("bazel-bin/pocket/pocket_cli"), &binary)
        .unwrap_or_else(|error| panic!("copy pocket_cli: {error}"));
    let board = relocated.join("board.txt");
    let notes = relocated.join("notes.txt");
    write(&board, BOARD);
    write(&notes, NOTES);
    let board = board.to_string_lossy().into_owned();
    let asset = format!("--asset={}", notes.to_string_lossy());

    let path = binary.to_string_lossy().into_owned();
    assert_eq!(tool("xcrun", &["lipo", "-archs", &path]), "arm64");
    let loads = tool("xcrun", &["otool", "-l", &path]);
    assert_eq!(field(&loads, "platform"), ["1"], "{loads}");
    keep("pocket_cli.otool.txt", &loads);

    let mut commands = Vec::new();
    let (rejected, shown) = pocket_cli(
        &binary,
        &[
            &asset,
            &board,
            "add Ship the simulator build",
            "done 1",
            "done 9",
        ],
    );
    assert_eq!(rejected.status.code(), Some(1), "{}", rejected.combined());
    assert_eq!(rejected.stdout, NATIVE_STATE);
    commands.push(format!("{shown} exit=1"));

    let (accepted, shown) = pocket_cli(
        &binary,
        &[&asset, &board, "add Ship the simulator build", "done 1"],
    );
    assert_eq!(accepted.status.code(), Some(0), "{}", accepted.combined());
    assert!(
        accepted
            .stdout
            .contains("\nreplies=ok:added 3;ok:done 1\nsummary=tasks=3 done=2 open=1 assets=1\nasset notes.txt bytes=20 adler32:4fce0732\n"),
        "{}",
        accepted.stdout
    );
    commands.push(format!("{shown} exit=0"));

    let missing = relocated.join("missing.txt").to_string_lossy().into_owned();
    let (unreadable, shown) = pocket_cli(&binary, &[&missing]);
    assert_eq!(
        unreadable.status.code(),
        Some(2),
        "{}",
        unreadable.combined()
    );
    assert!(
        unreadable
            .stderr
            .contains(&format!("err:unreadable: {missing}")),
        "{}",
        unreadable.stderr
    );
    commands.push(format!("{shown} exit=2"));

    let (usage, shown) = pocket_cli(&binary, &[]);
    assert_eq!(usage.status.code(), Some(2), "{}", usage.combined());
    assert!(
        usage
            .stderr
            .contains("usage: pocket_cli [--asset=PATH]... BOARD [COMMAND]..."),
        "{}",
        usage.stderr
    );
    commands.push(format!("{shown} exit=2"));

    let rustc = rustc_identity(&root);
    assert!(rustc.starts_with(RUSTC), "{rustc}");
    keep(
        "native-evidence.txt",
        &format!(
            "{}rustc={rustc}\nbuild=bazel build --config=release //pocket:pocket_cli\nslice=macos/arm64 platform=1 profile=release executed=relocated-cli\n{}\nstate={}\n",
            executor(&xcode),
            commands
                .iter()
                .map(|command| format!("command={command}"))
                .collect::<Vec<_>>()
                .join("\n"),
            NATIVE_STATE.trim_end().replace('\n', ";")
        ),
    );
}

#[test]
fn the_shared_core_runs_in_a_relocated_release_simulator_app() {
    let xcode = installed_xcode();
    let sdk_version = tool("xcrun", &["--sdk", "iphonesimulator", "--show-sdk-version"]);
    let runtime = runtime_for(&sdk_version);
    let simulator = Simulator::create("dx-pocket", &runtime);
    let root = pocket(&xcode, &runtime, &simulator.name);

    passes(&bazel(
        &root,
        "build",
        &["//pocket:pocket_app"],
        &["--config=release"],
    ));
    let cell = read(&root.join("bazel-bin/pocket/pocket_app_library.txt"));
    keep("pocket_app_library.txt", &cell);
    assert!(cell.contains("cell=simulator/arm64\n"), "{cell}");
    assert!(cell.contains("sdk=iphonesimulator\n"), "{cell}");
    assert!(cell.contains(&format!("xcode={xcode}")), "{cell}");
    assert!(
        cell.lines().any(|line| line.starts_with("archive=")
            && line.contains("pocket_rust")
            && line.contains(" arch=arm64 platform=iphonesimulator minos=")),
        "{cell}"
    );

    let relocated =
        mkscratch("apple-shared-rust-release").unwrap_or_else(|error| panic!("scratch: {error}"));
    let ipa = relocated.join("pocket_app.ipa");
    std::fs::copy(root.join("bazel-bin/pocket/pocket_app.ipa"), &ipa)
        .unwrap_or_else(|error| panic!("copy ipa: {error}"));
    unzip(&ipa, &relocated);
    let app = relocated.join("Payload/pocket_app.app");
    let info = app.join("Info.plist");
    assert_eq!(plist(&info, "CFBundleIdentifier"), POCKET_ID);
    assert_eq!(plist(&info, "CFBundleShortVersionString"), "3.1");
    assert_eq!(plist(&info, "MinimumOSVersion"), "17.0");
    assert_eq!(plist(&info, "DTPlatformName"), "iphonesimulator");
    assert_eq!(read(&app.join("board.txt")), BOARD);
    assert_eq!(read(&app.join("notes.txt")), NOTES);
    let binary = app.join("pocket_app");
    assert_eq!(
        tool("xcrun", &["lipo", "-archs", &binary.to_string_lossy()]),
        "arm64"
    );
    keep(
        "pocket_app.otool.txt",
        &inspect_binary(&binary, "arm64", "7"),
    );

    let tested = bazel(
        &root,
        "test",
        &["//pocket:pocket_app_test"],
        &["--test_output=all"],
    );
    passes(&tested);
    for case in [
        "testSharedCoreRepliesLikeTheNativeCli",
        "testInvalidBoardKeepsTheLoadedOne",
        "testBundledNotesDigestMatchesTheNativeCli",
        "testConsumerFeaturesReachTheSimulatorBuild",
        "testHostAppWroteItsLaunchState",
    ] {
        assert!(
            passed(&tested, case),
            "{case} did not pass:\n{}",
            tested.combined()
        );
    }

    passes(&bazel(&root, "run", &["//pocket:boot"], &[]));
    tool(
        "xcrun",
        &["simctl", "install", &simulator.udid, &app.to_string_lossy()],
    );
    assert!(
        container(&simulator, POCKET_ID, "app")
            .join("board.txt")
            .is_file(),
        "the relocated bundle did not install"
    );
    passes(&bazel(&root, "run", &["//pocket:launch"], &[]));
    let state =
        wait_for(&container(&simulator, POCKET_ID, "data").join("Documents/pocket-state.txt"));
    assert_eq!(state, RELEASE_STATE);

    let device = bazel(
        &root,
        "build",
        &["//pocket:pocket_device_app"],
        &["--ios_multi_cpus=arm64"],
    );
    fails_with(
        &device,
        "apple_app: device distribution needs provisioning_profile; credentials travel by file only",
    );
    let export = bazel(&root, "build", &["//pocket:pocket_export_app"], &[]);
    fails_with(
        &export,
        "apple_app: export is a distinct explicitly applied operation; Check never exports an IPA",
    );
    for unsigned in ["pocket_device_app", "pocket_export_app"] {
        assert!(
            !root
                .join(format!("bazel-bin/pocket/{unsigned}.ipa"))
                .exists(),
            "{unsigned} produced an app without explicit signing"
        );
    }

    let rustc = rustc_identity(&root);
    assert!(rustc.starts_with(RUSTC), "{rustc}");
    keep(
        "pocket-evidence.txt",
        &format!(
            "{}rustc={rustc}\nruntime=iOS {runtime}\ndevice_type=iPhone 16\nsimulator={}\nslice=simulator/arm64 platform=iphonesimulator minos=17.0 profile=release executed=xctest,relocated-launch\nstate={}\nslice=device/arm64 executed=pending: needs a physical device\nsigning=pending: needs a consumer provisioning profile and identity\n",
            executor(&xcode),
            simulator.udid,
            state.trim_end().replace('\n', ";")
        ),
    );
}

#[test]
fn the_shared_core_ships_as_an_xcframework_to_an_independent_host_app() {
    let xcode = installed_xcode();
    let sdk_version = tool("xcrun", &["--sdk", "iphonesimulator", "--show-sdk-version"]);
    let runtime = runtime_for(&sdk_version);
    let producer = pocket(&xcode, &runtime, "unused");

    passes(&bazel(
        &producer,
        "build",
        &["//pocket:pocket_framework"],
        &["--config=release"],
    ));
    let report = read(&producer.join("bazel-bin/pocket/pocket_framework_xcframework.txt"));
    keep("pocket_framework_xcframework.txt", &report);
    assert!(report.contains("xcframework=PocketTasks\n"), "{report}");
    assert!(
        report.contains("resources=board.txt,notes.txt\n"),
        "{report}"
    );
    for (cell, library, platform) in [
        ("device/arm64", "ios-arm64", "iphoneos"),
        ("simulator/arm64", "ios-arm64-simulator", "iphonesimulator"),
    ] {
        assert!(
            report.contains(&format!(
                "slice={cell} library={library} platform={platform} minos=17.0 install_name=@rpath/PocketTasks.framework/PocketTasks\n"
            )),
            "{report}"
        );
    }

    let shipped = mkscratch("apple-shared-rust-xcframework")
        .unwrap_or_else(|error| panic!("scratch: {error}"));
    let zip = shipped.join("PocketTasks.xcframework.zip");
    std::fs::copy(
        producer.join("bazel-bin/pocket/pocket_framework.xcframework.zip"),
        &zip,
    )
    .unwrap_or_else(|error| panic!("copy xcframework: {error}"));
    unzip(&zip, &shipped);
    let xcframework = shipped.join("PocketTasks.xcframework");
    let mut slices = Vec::new();
    for (library, arch, platform, supported) in [
        ("ios-arm64", "arm64", "2", "iPhoneOS"),
        ("ios-arm64-simulator", "arm64", "7", "iPhoneSimulator"),
    ] {
        let framework = xcframework.join(library).join("PocketTasks.framework");
        let info = framework.join("Info.plist");
        assert_eq!(plist(&info, "CFBundleIdentifier"), FRAMEWORK_ID);
        assert_eq!(plist(&info, "CFBundlePackageType"), "FMWK");
        assert_eq!(plist(&info, "CFBundleShortVersionString"), "3.1");
        assert_eq!(plist(&info, "MinimumOSVersion"), "17.0");
        assert_eq!(plist(&info, "CFBundleSupportedPlatforms.0"), supported);
        assert_eq!(read(&framework.join("board.txt")), BOARD);
        assert_eq!(read(&framework.join("notes.txt")), NOTES);
        assert_eq!(read(&framework.join("Headers/pocket_api.h")), POCKET_H);
        let binary = framework.join("PocketTasks");
        assert_eq!(
            tool("xcrun", &["lipo", "-archs", &binary.to_string_lossy()]),
            arch
        );
        let loads = inspect_binary(&binary, arch, platform);
        assert!(
            loads.contains("name @rpath/PocketTasks.framework/PocketTasks (offset"),
            "{loads}"
        );
        keep(&format!("{library}.otool.txt"), &loads);
        let sdk = field(&loads, "sdk").join(",");
        slices.push(format!(
            "slice={library} arch={arch} platform={platform} minos=17.0 sdk={sdk}"
        ));
    }

    let rustc = rustc_identity(&producer);
    assert!(rustc.starts_with(RUSTC), "{rustc}");

    let simulator = Simulator::create("dx-desk", &runtime);
    let desk = consumer("apple-shared-rust-host", DESK_EXTRA, BAZELRC);
    write(&desk.join("BUILD.bazel"), "");
    unzip(&zip, &desk.join("desk"));
    write(
        &desk.join("desk/BUILD.bazel"),
        &DESK_BUILD
            .replace("XCODE", &xcode)
            .replace("RUNTIME", &runtime)
            .replace("SIMULATOR", &simulator.name),
    );
    write(&desk.join("desk/DeskAppDelegate.h"), DESK_DELEGATE_H);
    write(&desk.join("desk/DeskAppDelegate.m"), DESK_DELEGATE_M);
    write(&desk.join("desk/main.m"), DESK_MAIN_M);
    write(&desk.join("desk/DeskAppTests.m"), DESK_TESTS_M);
    assert!(
        !read(&desk.join("MODULE.bazel")).contains("rules_rust")
            && !desk.join("desk/pocket.rs").exists(),
        "the host app must see only the shipped framework"
    );

    passes(&bazel(&desk, "build", &["//desk:desk_app"], &[]));
    let cell = read(&desk.join("bazel-bin/desk/desk_app_library.txt"));
    keep("desk_app_library.txt", &cell);
    assert!(
        cell.contains("framework=PocketTasks arch=arm64 platform=iphonesimulator minos=17.0\n"),
        "{cell}"
    );
    let ipa =
        mkscratch("apple-shared-rust-desk").unwrap_or_else(|error| panic!("scratch: {error}"));
    unzip(&desk.join("bazel-bin/desk/desk_app.ipa"), &ipa);
    let app = ipa.join("Payload/desk_app.app");
    let embedded = app.join("Frameworks/PocketTasks.framework");
    assert_eq!(read(&embedded.join("board.txt")), BOARD);
    assert_eq!(read(&embedded.join("notes.txt")), NOTES);
    let binary = app.join("desk_app").to_string_lossy().into_owned();
    let linked = tool("xcrun", &["otool", "-L", &binary]);
    assert!(
        linked.contains("@rpath/PocketTasks.framework/PocketTasks"),
        "{linked}"
    );
    let undefined = tool("xcrun", &["nm", "-u", &binary]);
    assert!(undefined.contains("_pocket_load"), "{undefined}");

    let tested = bazel(
        &desk,
        "test",
        &["//desk:desk_app_test"],
        &["--test_output=all"],
    );
    passes(&tested);
    for case in [
        "testFrameworkCoreAppliesCommandsToItsBundledBoard",
        "testFrameworkBundleCarriesTheSharedResources",
        "testFrameworkKeepsTheProducerReleaseBuild",
    ] {
        assert!(
            passed(&tested, case),
            "{case} did not pass:\n{}",
            tested.combined()
        );
    }

    passes(&bazel(&desk, "run", &["//desk:boot"], &[]));
    passes(&bazel(&desk, "run", &["//desk:install"], &[]));
    passes(&bazel(&desk, "run", &["//desk:launch"], &[]));
    let state = wait_for(
        &container(&simulator, "com.example.desk", "data").join("Documents/desk-state.txt"),
    );
    assert_eq!(state, DESK_STATE);

    keep(
        "framework-evidence.txt",
        &format!(
            "{}rustc={rustc}\nruntime=iOS {runtime}\ndevice_type=iPhone 16\nsimulator={}\n{}\nexecuted=ios-arm64-simulator: xctest,launch\nstate={}\nexecuted=ios-arm64: pending: needs a physical device\nsigning=pending: needs a consumer provisioning profile and identity\n",
            executor(&xcode),
            simulator.udid,
            slices.join("\n"),
            state.trim_end().replace('\n', ";")
        ),
    );
}
