//! The thrift compiler rejects an include outside the declared inputs.

use std::fs;
use std::process::Command;

use dx_path::Resolver;

fn runfile(resolver: &Resolver, env: &str) -> std::path::PathBuf {
    let key = std::env::var(env).expect(env);
    resolver.lookup(&key).expect("runfile")
}

#[test]
fn undeclared_include_is_rejected() {
    let exe = std::env::current_exe().expect("current_exe");
    let resolver = Resolver::for_binary(&exe).expect("runfiles");
    let thrift = runfile(&resolver, "THRIFT_COMPILER");
    let source = runfile(&resolver, "UNDECLARED_THRIFT");

    let work = std::path::PathBuf::from(std::env::var("TEST_TMPDIR").expect("TEST_TMPDIR"));
    let package = work.join("thrift/tests");
    fs::create_dir_all(&package).expect("mkdir");
    fs::copy(&source, package.join("undeclared.thrift")).expect("copy");
    let out = work.join("out");
    fs::create_dir_all(&out).expect("mkdir out");

    let output = Command::new(&thrift)
        .env_clear()
        .args(["--gen", "cpp", "-I"])
        .arg(&work)
        .arg("-out")
        .arg(&out)
        .arg(package.join("undeclared.thrift"))
        .output()
        .expect("spawn thrift");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !output.status.success(),
        "thrift accepted an include outside the declared inputs"
    );
    assert!(
        text.contains("Could not find include file"),
        "unexpected failure: {text}"
    );
}
