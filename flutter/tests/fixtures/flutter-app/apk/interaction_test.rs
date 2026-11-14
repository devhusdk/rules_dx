//! Taps Add item in the Flutter fixture app on the declared emulator.

use std::process::{Command, Output};
use std::thread::sleep;
use std::time::Duration;

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} is not set"))
}

fn pick(name: &str, keep: impl Fn(&str) -> bool) -> String {
    let paths = env(name);
    paths
        .split_whitespace()
        .find(|path| keep(path))
        .unwrap_or_else(|| panic!("{name} has no match in: {paths}"))
        .to_owned()
}

struct Device {
    adb: String,
    serial: String,
}

impl Device {
    fn run(&self, args: &[&str]) -> Output {
        Command::new(&self.adb)
            .env_clear()
            .env("HOME", env("TEST_TMPDIR"))
            .env("TMPDIR", env("TEST_TMPDIR"))
            .arg("-s")
            .arg(&self.serial)
            .args(args)
            .output()
            .unwrap_or_else(|error| panic!("run {}: {error}", self.adb))
    }

    fn text(&self, args: &[&str]) -> String {
        let output = self.run(args);
        let text = String::from_utf8_lossy(&output.stdout).replace('\r', "");
        assert!(
            output.status.success(),
            "adb {args:?} failed: {text}{}",
            String::from_utf8_lossy(&output.stderr)
        );
        text
    }

    fn wait_online(&self) {
        for _ in 0..60 {
            let output = self.run(&["get-state"]);
            if String::from_utf8_lossy(&output.stdout).trim() == "device" {
                return;
            }
            sleep(Duration::from_secs(1));
        }
        panic!("{} is not online", self.serial);
    }

    fn ui(&self) -> String {
        let dumped = self.run(&["shell", "uiautomator", "dump", "/sdcard/dx_ui.xml"]);
        if !dumped.status.success() {
            return String::new();
        }
        String::from_utf8_lossy(&self.run(&["shell", "cat", "/sdcard/dx_ui.xml"]).stdout)
            .replace('\r', "")
    }

    fn wait_for(&self, needle: &str) -> String {
        for _ in 0..60 {
            let ui = self.ui();
            if ui.contains(needle) {
                return ui;
            }
            sleep(Duration::from_secs(1));
        }
        let log = self.text(&["logcat", "-d", "-t", "80"]);
        panic!("the ui never showed {needle}:\n{log}");
    }
}

fn center(ui: &str, desc: &str) -> (i64, i64) {
    let node = ui
        .split('<')
        .find(|node| node.contains(&format!("content-desc=\"{desc}\"")))
        .unwrap_or_else(|| panic!("no {desc} node in the ui dump"));
    let bounds = node
        .split("bounds=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .unwrap_or_else(|| panic!("the {desc} node has no bounds"));
    let numbers: Vec<i64> = bounds
        .split(|c: char| !c.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .map(|part| part.parse().expect("bound"))
        .collect();
    assert_eq!(numbers.len(), 4, "bounds {bounds}");
    ((numbers[0] + numbers[2]) / 2, (numbers[1] + numbers[3]) / 2)
}

#[test]
fn tap_on_add_item_changes_the_count() {
    let serial_file = env("DX_ANDROID_SERIAL_FILE");
    let serial = std::fs::read_to_string(&serial_file)
        .unwrap_or_else(|error| panic!("read {serial_file}: {error}"))
        .trim()
        .to_owned();
    assert!(!serial.is_empty(), "no emulator serial is declared");
    let device = Device {
        adb: pick("DX_ADB", |path| path.ends_with("/adb")),
        serial,
    };
    device.wait_online();
    assert_eq!(
        device.text(&["shell", "getprop", "ro.kernel.qemu"]).trim(),
        "1",
        "{} is not an emulator",
        device.serial
    );
    assert_eq!(
        device
            .text(&["shell", "getprop", "ro.product.cpu.abi"])
            .trim(),
        "x86_64",
        "{} is not an x86_64 emulator",
        device.serial
    );

    let apk = pick("DX_APK", |path| {
        path.ends_with(".apk") && !path.ends_with("_unsigned.apk")
    });
    let app_id = env("DX_APK_APP_ID");
    let activity = env("DX_APK_ACTIVITY");
    device.text(&["install", "-r", &apk]);
    device.text(&["shell", "am", "force-stop", &app_id]);
    let started = device.text(&[
        "shell",
        "am",
        "start",
        "-W",
        "-n",
        &format!("{app_id}/{activity}"),
    ]);
    assert!(!started.contains("Error"), "{started}");

    let ui = device.wait_for("No items");
    let (x, y) = center(&ui, "Add item");
    device.text(&["shell", "input", "tap", &x.to_string(), &y.to_string()]);
    device.wait_for("One item");
}
