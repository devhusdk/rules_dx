#![expect(
    clippy::disallowed_methods,
    reason = "tests run local programs under the Bazel test timeout"
)]

use std::path::Path;
use std::process::Command;

use dx_testing::read_runfiles;

const LOCAL_HELPER: &str = "tools/buildbuddy-credential-helper.sh";

fn keep_system_vars(command: &mut Command) {
    for (key, value) in std::env::vars_os() {
        if key
            .to_string_lossy()
            .to_ascii_lowercase()
            .starts_with("system")
        {
            command.env(key, value);
        }
    }
}

fn run_node(dir: &Path, env: &[(&str, &str)]) -> (i32, String) {
    let mut command = Command::new(dx_testing::env_file("DX_NODE"));
    command
        .arg(dx_testing::env_file("DX_SCAN_CJS"))
        .current_dir(dir)
        .env_clear();
    keep_system_vars(&mut command);
    for (key, value) in env {
        let key = if *key == "DX_UPLOAD_PATH" {
            "INPUT_PATH"
        } else {
            key
        };
        command.env(key, value);
    }
    let done = command.output().expect("run node");
    let mut output = String::from_utf8_lossy(&done.stdout).into_owned();
    output.push_str(&String::from_utf8_lossy(&done.stderr));
    (done.status.code().unwrap_or(-1), output)
}

fn run_scans(dir: &Path, env: &[(&str, &str)]) -> Vec<(&'static str, i32, String)> {
    let (code, output) = run_node(dir, env);
    vec![("node", code, output)]
}

fn assert_no_secret_printed(name: &str, output: &str) {
    for secret in ["k3y", "s3cr3t"] {
        assert!(
            !output.contains(secret),
            "{name} printed {secret}: {output}"
        );
    }
}

type CacheCase<'a> = (
    &'a str,
    &'a [(&'a str, &'a str)],
    Option<&'a str>,
    i32,
    Option<&'a str>,
);

#[test]
fn scan_finds_every_cache_secret() {
    let key = ("BUILDBUDDY_API_KEY", "k3y");
    let header = ("REMOTE_CACHE_HEADER", "x-t=s3cr3t");
    let empty = ("REMOTE_CACHE_HEADER", "x-t=");
    let no_equals = ("REMOTE_CACHE_HEADER", "novalue");
    let no_value = Some("has no value");
    let cases: &[CacheCase<'_>] = &[
        ("nothing set", &[], Some("k3y s3cr3t"), 0, None),
        ("key found", &[key], Some("has k3y"), 1, None),
        ("key clean", &[key], Some("clean"), 0, None),
        ("header value found", &[header], Some("has s3cr3t"), 1, None),
        ("header name only", &[header], Some("x-t"), 0, None),
        ("header empty value", &[empty], Some("x"), 1, no_value),
        ("header without =", &[no_equals], Some("x"), 1, no_value),
        ("key with header", &[key, header], Some("has k3y"), 1, None),
        ("path missing", &[key, header], None, 0, None),
    ];
    for (name, env, content, status, expected) in cases {
        let dir = dx_test_scratch::scratch("secret-scan").unwrap();
        let upload = dir.path().join("upload");
        if let Some(content) = content {
            std::fs::create_dir_all(&upload).expect("create upload dir");
            std::fs::write(upload.join("report.txt"), content).expect("write upload");
        }
        let upload = upload.to_str().expect("utf-8 path");
        let mut env = env.to_vec();
        env.push(("DX_UPLOAD_PATH", upload));
        for (runner, code, output) in run_scans(dir.path(), &env) {
            assert_eq!(code, *status, "{runner} {name}: {output}");
            if let Some(expected) = expected {
                assert!(output.contains(expected), "{runner} {name}: {output}");
            }
            assert_no_secret_printed(name, &output);
        }
    }
}

fn gzip_with_node(text: &str) -> Vec<u8> {
    let mut command = Command::new(dx_testing::env_file("DX_NODE"));
    command
        .args([
            "-e",
            "process.stdout.write(require('zlib').gzipSync(process.argv[1]))",
            text,
        ])
        .env_clear();
    keep_system_vars(&mut command);
    let done = command.output().expect("run node");
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    done.stdout
}

#[test]
fn scan_reads_inside_gzip_files() {
    for (name, content, status) in [("gzip with key", "has k3y", 1), ("gzip clean", "clean", 0)] {
        let dir = dx_test_scratch::scratch("secret-scan-gzip").unwrap();
        let upload = dir.path().join("upload");
        std::fs::create_dir_all(&upload).expect("create upload dir");
        std::fs::write(upload.join("trace.json.gz"), gzip_with_node(content)).expect("write gzip");
        let env = [
            ("BUILDBUDDY_API_KEY", "k3y"),
            ("DX_UPLOAD_PATH", upload.to_str().expect("utf-8 path")),
        ];
        for (runner, code, output) in run_scans(dir.path(), &env) {
            assert_eq!(code, status, "{runner} {name}: {output}");
            assert_no_secret_printed(name, &output);
        }
    }
}

#[cfg(unix)]
#[test]
fn scan_follows_symlinks() {
    let cases = [
        ("link to a secret", "../outside.txt", None),
        ("dangling link", "../missing", Some("failed")),
    ];
    for (name, target, expected) in cases {
        let dir = dx_test_scratch::scratch("secret-scan-link").unwrap();
        let upload = dir.path().join("upload");
        std::fs::create_dir_all(&upload).expect("create upload dir");
        std::fs::write(dir.path().join("outside.txt"), "has k3y").expect("write outside");
        std::os::unix::fs::symlink(target, upload.join("link")).expect("symlink");
        let env = [
            ("BUILDBUDDY_API_KEY", "k3y"),
            ("DX_UPLOAD_PATH", upload.to_str().expect("utf-8 path")),
        ];
        for (runner, code, output) in run_scans(dir.path(), &env) {
            assert_eq!(code, 1, "{runner} {name}: {output}");
            if let Some(expected) = expected {
                assert!(output.contains(expected), "{runner} {name}: {output}");
            }
            assert_no_secret_printed(name, &output);
        }
    }
}

fn helper_path(written: &str) -> String {
    written
        .trim()
        .rsplit('=')
        .next()
        .expect("helper path")
        .to_owned()
}

fn run_helper_node(dir: &Path, env: &[(&str, &str)]) -> (i32, String, String) {
    let temp = dir.join("temp");
    std::fs::create_dir_all(&temp).expect("create temp");
    let github_env = dir.join("github_env");
    let mut command = Command::new(dx_testing::env_file("DX_NODE"));
    command
        .arg(dx_testing::env_file("DX_HELPER_JS"))
        .current_dir(dir)
        .env_clear()
        .env("RUNNER_OS", "Linux")
        .env("RUNNER_TEMP", &temp)
        .env("GITHUB_ENV", &github_env);
    keep_system_vars(&mut command);
    for (key, value) in env {
        command.env(key, value);
    }
    let done = command.output().expect("run node");
    let mut output = String::from_utf8_lossy(&done.stdout).into_owned();
    output.push_str(&String::from_utf8_lossy(&done.stderr));
    let written = std::fs::read_to_string(&github_env).unwrap_or_default();
    (done.status.code().unwrap_or(-1), output, written)
}

type HelperCase<'a> = (&'a str, &'a [(&'a str, &'a str)], i32, Option<&'a str>);

#[test]
fn helper_action_writes_a_working_helper() {
    let key = ("BUILDBUDDY_API_KEY", "k3y");
    let cache = (
        "DX_REMOTE_CACHE_ARG",
        "--remote_cache=grpcs://remote.example.com",
    );
    let header = ("REMOTE_CACHE_HEADER", "x-t=s3cr3t");
    let own_host = (
        "DX_REMOTE_CACHE_ARG",
        "--remote_cache=grpcs://u@cache.example.com:443",
    );
    let cases: &[HelperCase<'_>] = &[
        ("key only", &[key], 0, Some("remote.buildbuddy.io=")),
        (
            "key with a custom cache",
            &[key, cache],
            0,
            Some(
                "--remote_cache=grpcs://remote.example.com --credential_helper=remote.buildbuddy.io=",
            ),
        ),
        (
            "header",
            &[header, own_host],
            0,
            Some("--credential_helper=cache.example.com="),
        ),
        (
            "header wins over key",
            &[key, header, own_host],
            0,
            Some("--credential_helper=cache.example.com="),
        ),
        ("no equals", &[("REMOTE_CACHE_HEADER", "novalue")], 1, None),
        ("quote", &[("REMOTE_CACHE_HEADER", "x=a\"b")], 1, None),
        ("percent", &[("REMOTE_CACHE_HEADER", "x=a%b")], 1, None),
        ("nothing set", &[], 0, None),
    ];
    for (name, env, status, args) in cases {
        let dir = dx_test_scratch::scratch("helper-action-node").unwrap();
        let (code, output, written) = run_helper_node(dir.path(), env);
        assert_eq!(code, *status, "{name}: {output}");
        assert_no_secret_printed(name, &output);
        if *status != 0 {
            assert!(output.contains("::error::"), "{name}: {output}");
        }
        let Some(args) = args else {
            assert!(written.is_empty(), "{name} wrote {written:?}");
            continue;
        };
        assert!(
            written.starts_with("BB_ARGS=") && written.contains(args),
            "{name}: {written}"
        );
        let helper = std::fs::read_to_string(helper_path(&written)).expect("read helper");
        assert_eq!(helper, read_runfiles(LOCAL_HELPER), "{name}");
    }
}
