//! Installs and launches the Flutter fixture app on a simulator this test creates.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread::sleep;
use std::time::{Duration, Instant};

use dx_apple_ops::xcode::{APPLICATIONS, XCRUN, developer_dir};

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} is not set"))
}

fn declared(text: &str, key: &str) -> String {
    text.lines()
        .find_map(|line| line.strip_prefix(&format!("{key}=")))
        .unwrap_or_else(|| panic!("the declared simulator file has no {key}"))
        .trim()
        .to_owned()
}

struct Simulators {
    developer_dir: PathBuf,
    set: PathBuf,
}

impl Simulators {
    fn simctl(&self, args: &[&str]) -> (bool, String) {
        let output = Command::new(XCRUN)
            .env("DEVELOPER_DIR", &self.developer_dir)
            .arg("simctl")
            .arg("--set")
            .arg(&self.set)
            .args(args)
            .output()
            .unwrap_or_else(|error| panic!("run {XCRUN}: {error}"));
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        (output.status.success(), text)
    }

    fn must(&self, args: &[&str]) -> String {
        let (ok, text) = self.simctl(args);
        assert!(ok, "simctl {args:?} failed: {text}");
        text
    }

    fn until_ready(&self, args: &[&str], deadline: Instant) -> String {
        loop {
            let (ok, text) = self.simctl(args);
            if ok {
                return text;
            }
            assert!(Instant::now() < deadline, "simctl {args:?} failed: {text}");
            sleep(Duration::from_millis(500));
        }
    }
}

fn step(start: Instant, name: &str) {
    eprintln!("{name}: {:.1}s", start.elapsed().as_secs_f64());
}

struct Device<'a> {
    simulators: &'a Simulators,
    udid: String,
}

impl Drop for Device<'_> {
    fn drop(&mut self) {
        drop(self.simulators.simctl(&["shutdown", &self.udid]));
        drop(self.simulators.simctl(&["delete", &self.udid]));
    }
}

fn bundle(app: &str, scratch: &Path) -> PathBuf {
    if Path::new(app).extension().is_some_and(|end| end == "app") {
        return PathBuf::from(app);
    }
    assert!(
        Path::new(app).extension().is_some_and(|end| end == "ipa"),
        "{app} is neither an .app nor an .ipa"
    );
    let file = std::fs::File::open(app).unwrap_or_else(|error| panic!("open {app}: {error}"));
    zip::ZipArchive::new(file)
        .and_then(|mut archive| archive.extract(scratch))
        .unwrap_or_else(|error| panic!("unpack {app}: {error}"));
    std::fs::read_dir(scratch.join("Payload"))
        .unwrap_or_else(|error| panic!("{app} has no Payload: {error}"))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|end| end == "app"))
        .unwrap_or_else(|| panic!("{app} holds no Payload/*.app"))
}

#[test]
fn the_app_launches_on_a_fresh_simulator() {
    let start = Instant::now();
    let file = env("DX_IOS_SIMULATOR_FILE");
    let text =
        std::fs::read_to_string(&file).unwrap_or_else(|error| panic!("read {file}: {error}"));
    let runtime = declared(&text, "runtime");
    assert!(!runtime.is_empty(), "no simulator runtime is declared");
    let xcode = declared(&text, "xcode");
    let developer_dir = developer_dir(
        &xcode,
        std::env::var("DEVELOPER_DIR").ok().as_deref(),
        Path::new(APPLICATIONS),
    )
    .unwrap_or_else(|failure| panic!("{}", failure.message));

    let tmp = PathBuf::from(env("TEST_TMPDIR"));
    let set = tmp.join("simulators");
    std::fs::create_dir_all(&set).expect("create the simulator set");
    let simulators = Simulators { developer_dir, set };

    let runtime_id = format!(
        "com.apple.CoreSimulator.SimRuntime.iOS-{}",
        runtime.replace('.', "-")
    );
    let device_type = env("DX_SIMCTL_DEVICE_TYPE");
    let created = simulators.must(&["create", "dx-flutter-smoke", &device_type, &runtime_id]);
    let device = Device {
        simulators: &simulators,
        udid: created.trim().to_owned(),
    };
    step(start, "create");
    simulators.must(&["boot", &device.udid]);
    step(start, "boot");
    let deadline = Instant::now() + Duration::from_secs(40);

    let scratch = tmp.join("app");
    std::fs::create_dir_all(&scratch).expect("create the app scratch directory");
    let app = bundle(&env("DX_SIMCTL_APP"), &scratch);
    let app = app.to_string_lossy();
    simulators.until_ready(&["install", &device.udid, &app], deadline);
    step(start, "install");

    let bundle_id = env("DX_SIMCTL_BUNDLE_ID");
    let launched = simulators.until_ready(&["launch", &device.udid, &bundle_id], deadline);
    step(start, "launch");
    assert!(
        launched.starts_with(&format!("{bundle_id}: ")),
        "unexpected launch output: {launched}"
    );
    sleep(Duration::from_secs(5));
    let services = simulators.must(&["spawn", &device.udid, "launchctl", "list"]);
    assert!(
        services.contains(&format!("UIKitApplication:{bundle_id}")),
        "{bundle_id} is not running five seconds after launch:\n{services}"
    );
}
