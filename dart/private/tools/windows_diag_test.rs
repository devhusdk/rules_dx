use dx_testing::env_file;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const CLEAN: &str = "void main() {\n  print(1);\n}\n";
const ISOLATE: &str = "import 'dart:isolate';\nvoid child(int x) { print('child $x'); }\nFuture<void> main() async {\n  print('start');\n  await Isolate.spawn(child, 1);\n  await Future<void>.delayed(const Duration(seconds: 1));\n  print('done');\n}\n";
const SPAWN: &str = "import 'dart:io';\nFuture<void> main() async {\n  final p = await Process.start(Platform.resolvedExecutable, ['--version'], mode: ProcessStartMode.inheritStdio);\n  print('exit ${await p.exitCode}');\n}\n";

fn run(log: &mut String, label: &str, root: &Path, envs: &[(&str, String)], clear: bool, args: &[String]) {
    let bin = env_file("DART");
    let out = root.join(format!("{label}.out"));
    let err = root.join(format!("{label}.err"));
    let mut cmd = Command::new(&bin);
    if clear {
        cmd.env_clear();
    }
    for (k, v) in envs {
        cmd.env(k, v);
    }
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(std::fs::File::create(&out).expect("out"))
        .stderr(std::fs::File::create(&err).expect("err"));
    let start = Instant::now();
    let mut child = cmd.spawn().expect("spawn");
    let status = loop {
        if let Some(status) = child.try_wait().expect("wait") {
            break format!("{status}");
        }
        if start.elapsed() > Duration::from_secs(45) {
            let _ = child.kill();
            let _ = child.wait();
            break "HUNG".to_string();
        }
        std::thread::sleep(Duration::from_millis(200));
    };
    let o = std::fs::read_to_string(&out).unwrap_or_default();
    let e = std::fs::read_to_string(&err).unwrap_or_default();
    let tail = |s: &str| s.chars().rev().take(1500).collect::<Vec<_>>().into_iter().rev().collect::<String>();
    let _ = writeln!(
        log,
        "=== {label}: {status} in {:?}\n--- stdout\n{}\n--- stderr\n{}",
        start.elapsed(),
        tail(&o),
        tail(&e)
    );
}

#[test]
fn diagnose() {
    let tmp = PathBuf::from(std::env::var("TEST_TMPDIR").expect("TEST_TMPDIR"));
    let root = std::fs::canonicalize(&tmp).expect("canon").join("diag");
    std::fs::create_dir_all(root.join("lib")).expect("mkdir");
    std::fs::create_dir_all(root.join("home/AppData/Local")).expect("mkdir");
    std::fs::create_dir_all(root.join("home/AppData/Roaming")).expect("mkdir");
    std::fs::write(root.join("analysis_options.yaml"), "").expect("w");
    let file = root.join("lib/a.dart");
    std::fs::write(&file, CLEAN).expect("w");
    std::fs::write(root.join("iso.dart"), ISOLATE).expect("w");
    std::fs::write(root.join("spawn.dart"), SPAWN).expect("w");
    let mut log = String::new();
    let bin = env_file("DART");
    let _ = writeln!(log, "dart = {}", bin.display());
    if let Some(sdk) = bin.parent().and_then(Path::parent) {
        for sub in ["bin", "bin/snapshots"] {
            let names: Vec<String> = std::fs::read_dir(sdk.join(sub))
                .map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect())
                .unwrap_or_default();
            let _ = writeln!(log, "{sub}: {names:?}");
        }
    }
    let mut vars: Vec<String> = std::env::vars().map(|(k, _)| k).collect();
    vars.sort();
    let _ = writeln!(log, "test env keys: {vars:?}");
    let s = |p: &Path| p.display().to_string();
    let analyze = |cache: &str| {
        vec![
            "--suppress-analytics".to_string(),
            "analyze".into(),
            "--format=json".into(),
            format!("--cache={}", s(&root.join(cache))),
            s(&file),
        ]
    };
    let sysroot = std::env::var("SYSTEMROOT").unwrap_or_default();
    let temp = s(&root);
    let home = root.join("home");
    let base = vec![("DART_SUPPRESS_ANALYTICS", "true".to_string())];
    let mut sys = base.clone();
    sys.push(("SystemRoot", sysroot.clone()));
    sys.push(("TEMP", temp.clone()));
    sys.push(("TMP", temp.clone()));
    let mut profile = sys.clone();
    profile.push(("USERPROFILE", s(&home)));
    profile.push(("HOME", s(&home)));
    profile.push(("LOCALAPPDATA", s(&home.join("AppData/Local"))));
    profile.push(("APPDATA", s(&home.join("AppData/Roaming"))));
    let mut path = profile.clone();
    path.push(("PATH", format!("{sysroot}\\System32")));
    run(&mut log, "version_clear", &root, &base, true, &["--version".into()]);
    run(&mut log, "isolate_inherit", &root, &[], false, &[s(&root.join("iso.dart"))]);
    run(&mut log, "spawn_inherit", &root, &[], false, &[s(&root.join("spawn.dart"))]);
    run(&mut log, "analyze_inherit", &root, &[], false, &analyze("c1"));
    run(&mut log, "analyze_clear", &root, &base, true, &analyze("c2"));
    run(&mut log, "analyze_profile", &root, &profile, true, &analyze("c3"));
    run(&mut log, "analyze_path", &root, &path, true, &analyze("c4"));
    run(&mut log, "analyze_inherit_profile", &root, &profile, false, &analyze("c5"));
    let mut verbose = vec!["-v".to_string()];
    verbose.extend(analyze("c6"));
    run(&mut log, "analyze_inherit_verbose", &root, &[], false, &verbose);
    panic!("diagnostic report\n{log}");
}
