//! Installs and launches the Flutter fixture app on the booted host template simulator and uninstalls it afterwards.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread::sleep;
use std::time::{Duration, Instant};

use dx_apple_ops::simctl::device;
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
}

impl Simulators {
    fn simctl(&self, args: &[&str]) -> (bool, String) {
        let output = Command::new(XCRUN)
            .env("DEVELOPER_DIR", &self.developer_dir)
            .arg("simctl")
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
    let line = format!("{name}: {:.1}s\n", start.elapsed().as_secs_f64());
    eprint!("{line}");
    if let Ok(dir) = std::env::var("TEST_UNDECLARED_OUTPUTS_DIR") {
        let path = Path::new(&dir).join("steps.txt");
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .unwrap_or_else(|error| panic!("open {}: {error}", path.display()));
        std::io::Write::write_all(&mut file, line.as_bytes())
            .unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
    }
}

struct Installed<'a> {
    simulators: &'a Simulators,
    udid: String,
    bundle_id: String,
}

impl Drop for Installed<'_> {
    fn drop(&mut self) {
        drop(
            self.simulators
                .simctl(&["terminate", &self.udid, &self.bundle_id]),
        );
        drop(
            self.simulators
                .simctl(&["uninstall", &self.udid, &self.bundle_id]),
        );
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
    dx_archive::extract(
        Path::new(app),
        dx_archive::Format::Zip,
        scratch,
        dx_archive::Limits::default(),
    )
    .unwrap_or_else(|error| panic!("unpack {app}: {error}"));
    std::fs::read_dir(scratch.join("Payload"))
        .unwrap_or_else(|error| panic!("{app} has no Payload: {error}"))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|end| end == "app"))
        .unwrap_or_else(|| panic!("{app} holds no Payload/*.app"))
}

fn template(simulators: &Simulators, device_type: &str, runtime: &str, runtime_id: &str) -> String {
    let name = format!("dx-template {device_type} iOS {runtime}");
    let listing = simulators.must(&["list", "devices"]);
    let found: Vec<(&str, bool)> = listing
        .lines()
        .filter_map(|line| device(line).map(|(device_name, udid)| (device_name, udid, line)))
        .filter(|(device_name, _, _)| *device_name == name.as_str())
        .map(|(_, udid, line)| (udid, line.trim_end().ends_with("(Booted)")))
        .collect();
    let create = format!(
        "udid=$(xcrun simctl create {name:?} {device_type:?} {runtime_id}) && \
         xcrun simctl bootstatus \"$udid\" -b"
    );
    match found.as_slice() {
        [(udid, true)] => (*udid).to_owned(),
        [(udid, false)] => panic!(
            "the simulator {name:?} ({udid}) is not booted; boot it with \
             xcrun simctl bootstatus {udid} -b"
        ),
        [] => panic!("no simulator named {name:?} on {runtime_id}; create it with {create}"),
        _ => panic!("more than one simulator is named {name:?} on {runtime_id}"),
    }
}

#[test]
fn the_app_launches_on_the_booted_template() {
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
    let simulators = Simulators { developer_dir };

    let runtime_id = format!(
        "com.apple.CoreSimulator.SimRuntime.iOS-{}",
        runtime.replace('.', "-")
    );
    let device_type = env("DX_SIMCTL_DEVICE_TYPE");
    let udid = template(&simulators, &device_type, &runtime, &runtime_id);
    step(start, "find");
    let deadline = Instant::now() + Duration::from_secs(40);

    let tmp = PathBuf::from(env("TEST_TMPDIR"));
    let scratch = tmp.join("app");
    std::fs::create_dir_all(&scratch).expect("create the app scratch directory");
    let app = bundle(&env("DX_SIMCTL_APP"), &scratch);
    let app = app.to_string_lossy();
    let bundle_id = env("DX_SIMCTL_BUNDLE_ID");
    let device = Installed {
        simulators: &simulators,
        udid,
        bundle_id: bundle_id.clone(),
    };
    simulators.until_ready(&["install", &device.udid, &app], deadline);
    step(start, "install");

    let launched = simulators.until_ready(&["launch", &device.udid, &bundle_id], deadline);
    step(start, "launch");
    assert!(
        launched.starts_with(&format!("{bundle_id}: ")),
        "unexpected launch output: {launched}"
    );
    let service = format!("UIKitApplication:{bundle_id}");
    let running = Instant::now() + Duration::from_secs(5);
    loop {
        let services = simulators.must(&["spawn", &device.udid, "launchctl", "list"]);
        if services.contains(&service) {
            break;
        }
        assert!(
            Instant::now() < running,
            "{bundle_id} is not running five seconds after launch:\n{services}"
        );
        sleep(Duration::from_millis(500));
    }
    step(start, "running");
}
